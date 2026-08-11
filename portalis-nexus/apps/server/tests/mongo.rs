//! The durable store, against a real `MongoDB` replica set.
//!
//! These need Docker. Without it they are skipped rather than failed, so
//! `cargo test` still passes on a machine that cannot run containers; CI has
//! Docker and runs them for real.

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use ed25519_dalek::{Signer, SigningKey};
use portalis_nexus_protocol::{
    CURRENT_PROTOCOL_VERSION, MAX_KEY_ENVELOPES_PER_PAGE, SHARE_ID_BYTES, SessionBinding,
    authentication_payload, derive_device_id, format_id, link_device_payload, new_message_id,
    registration_payload,
};
use portalis_nexus_server::{MongoStore, NexusStore};
use portalis_nexus_server_core::{
    AuthenticationRequest, DeviceId, DeviceRecord, EnvelopeRepository, FixedClock,
    FriendRepository, FriendshipEdge, FriendshipRecord, FriendshipState, IdentityError,
    IdentityRepository, IdentityService, KeyEnvelopeRecord, LinkDeviceRequest, RegistrationRequest,
    RepositoryError, ScriptedRandom, ShareId, ShareMembershipRecord, ShareRecord, ShareRepository,
    ShareSnapshotRecord, UserDirectory, UserId, UserRecord,
};
use testcontainers_modules::mongo::Mongo;
use testcontainers_modules::testcontainers::ContainerAsync;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use tokio::sync::{Mutex, OwnedMutexGuard};
use tokio::time::timeout;

/// How long a store waits for a server before calling it an outage.
///
/// The driver's default is thirty seconds. Nothing here should ever wait that
/// long: the container is already up before a store connects, and the tests
/// that stop it on purpose would otherwise spend minutes proving a point that
/// takes a moment.
const SELECTION_TIMEOUT_MS: u32 = 1_000;
/// Docker can stall while pulling or starting a replica set. Fail one test
/// promptly instead of leaving every parallel test blocked behind it.
const CONTAINER_START_TIMEOUT: Duration = Duration::from_secs(20);
const CONTAINER_STOP_TIMEOUT: Duration = Duration::from_secs(10);
const STORE_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DOCKER_INFO_TIMEOUT: Duration = Duration::from_secs(2);

const NOW: u64 = 1_700_000_000_000_000_000;
const ADA: UserId = [1; 16];
const GRACE: UserId = [2; 16];
const ENCRYPTION_KEY: [u8; 32] = [6; 32];
const RECIPIENT: DeviceId = [7; 32];
const OTHER_RECIPIENT: DeviceId = [8; 32];

/// Whether a Docker daemon is reachable at all.
///
/// Only its absence justifies skipping. A daemon that is present but cannot
/// start the container is a real failure, and saying otherwise would let this
/// suite report success while testing nothing.
fn docker_is_available() -> bool {
    let Ok(mut child) = std::process::Command::new("docker")
        .arg("info")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return false;
    };
    let deadline = Instant::now() + DOCKER_INFO_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

fn docker_failure() -> &'static OnceLock<String> {
    static FAILURE: OnceLock<String> = OnceLock::new();
    &FAILURE
}

fn remember_docker_failure(reason: String) {
    let _ = docker_failure().set(reason);
}

/// The environment variable naming an already-running `MongoDB`.
///
/// Set it to use a replica set this suite did not start, which is how CI runs
/// against a service container and how a developer without a working Docker
/// can still exercise these tests.
const EXTERNAL_URI: &str = "PORTALIS_NEXUS_TEST_MONGODB_URI";

/// A database name no other test will use.
///
/// An external server is shared between tests, so each needs its own database
/// or they would see each other's users.
fn scratch_database() -> String {
    format!(
        "nexus_test_{}",
        format_id(&new_message_id()).replace('-', "")
    )
}

/// A running `MongoDB`, and a store connected to it.
struct Running {
    /// Kept alive for the test's duration; dropping it stops the container.
    container: Option<ContainerAsync<Mongo>>,
    store: MongoStore,
    uri: String,
    database: String,
    /// Container-backed tests are exclusive because each replica set is a
    /// heavy Docker resource. Kept until the test drops `Running`.
    _docker_guard: Option<OwnedMutexGuard<()>>,
}

impl Running {
    /// Connects a second store to the same database, as a restarted server
    /// would. Nothing is carried over in memory, so whatever the reconnected
    /// store can see was genuinely durable.
    async fn restart(&self) -> MongoStore {
        connect_store(
            &self.uri,
            &self.database,
            "the store reconnects to the same database",
        )
        .await
    }
}

/// Starts a `MongoDB` replica set, or connects to one already running.
///
/// A replica set rather than a standalone, because registration writes a user
/// and its device in one transaction and transactions need one.
///
/// Returns `None` only when Docker is unavailable; anything else panics.
async fn mongo() -> Option<Running> {
    if let Ok(uri) = std::env::var(EXTERNAL_URI) {
        let database = scratch_database();
        let store = connect_store(&uri, &database, "the configured MongoDB is reachable").await;
        return Some(Running {
            container: None,
            store,
            uri,
            database,
            _docker_guard: None,
        });
    }

    if let Some(reason) = docker_failure().get() {
        eprintln!("skipping: Docker was already unavailable ({reason})");
        return None;
    }

    let docker_guard = docker_gate().lock_owned().await;
    let container = match timeout(CONTAINER_START_TIMEOUT, Mongo::repl_set().start()).await {
        Ok(Ok(container)) => container,
        Ok(Err(error)) if docker_is_available() => {
            remember_docker_failure(error.to_string());
            panic!("Docker is running but MongoDB would not start: {error}")
        }
        Ok(Err(error)) => {
            remember_docker_failure(error.to_string());
            eprintln!("skipping: no Docker and no {EXTERNAL_URI} ({error})");
            return None;
        }
        Err(_) => {
            let reason = format!(
                "MongoDB replica set did not start within {} seconds",
                CONTAINER_START_TIMEOUT.as_secs()
            );
            remember_docker_failure(reason.clone());
            panic!("{reason}");
        }
    };
    let port = timeout(CONTAINER_START_TIMEOUT, container.get_host_port_ipv4(27017))
        .await
        .unwrap_or_else(|_| {
            panic!(
                "MongoDB replica set did not publish its port within {} seconds",
                CONTAINER_START_TIMEOUT.as_secs()
            )
        })
        .expect("the container publishes its port");
    let uri = format!(
        "mongodb://127.0.0.1:{port}/?directConnection=true&serverSelectionTimeoutMS={SELECTION_TIMEOUT_MS}"
    );
    let database = scratch_database();
    let store = connect_store(&uri, &database, "the store connects to its container").await;
    Some(Running {
        container: Some(container),
        store,
        uri,
        database,
        _docker_guard: Some(docker_guard),
    })
}

/// One local Docker daemon is shared by the test process. Starting many
/// replica sets concurrently makes Docker's readiness wait appear as a hung
/// test on smaller developer machines.
fn docker_gate() -> Arc<Mutex<()>> {
    static GATE: OnceLock<Arc<Mutex<()>>> = OnceLock::new();
    Arc::clone(GATE.get_or_init(|| Arc::new(Mutex::new(()))))
}

async fn connect_store(uri: &str, database: &str, expectation: &str) -> MongoStore {
    timeout(STORE_CONNECT_TIMEOUT, MongoStore::connect(uri, database))
        .await
        .unwrap_or_else(|_| {
            panic!(
                "{expectation} within {} seconds",
                STORE_CONNECT_TIMEOUT.as_secs()
            )
        })
        .unwrap_or_else(|error| panic!("{expectation}: {error}"))
}

async fn stop_container(container: &ContainerAsync<Mongo>) {
    timeout(CONTAINER_STOP_TIMEOUT, container.stop())
        .await
        .unwrap_or_else(|_| {
            panic!(
                "MongoDB container did not stop within {} seconds",
                CONTAINER_STOP_TIMEOUT.as_secs()
            )
        })
        .expect("the container stops");
}

/// Runs a test against a real store, or skips when Docker is unavailable.
macro_rules! with_mongo {
    ($store:ident, $body:block) => {
        let Some(running) = mongo().await else {
            return;
        };
        let $store = &running.store;
        $body
    };
}

fn user(id: UserId, username: &str, discriminator: &str) -> UserRecord {
    UserRecord {
        user_id: id,
        username: username.to_owned(),
        normalized_username: username.to_lowercase(),
        discriminator: discriminator.to_owned(),
        created_at_unix_ns: NOW,
    }
}

fn device(seed: u8, owner: UserId) -> DeviceRecord {
    DeviceRecord {
        device_id: [seed; 32],
        user_id: owner,
        public_key: [seed; 32],
        encryption_public_key: [seed; 32],
        created_at_unix_ns: NOW,
        last_authenticated_at_unix_ns: None,
        revoked_at_unix_ns: None,
    }
}

/// Share IDs that sort in the order they are created, so a page boundary can
/// be asserted rather than guessed at.
fn share_id(index: u16) -> ShareId {
    let mut id = [0; SHARE_ID_BYTES];
    id[SHARE_ID_BYTES - 2..].copy_from_slice(&index.to_be_bytes());
    id
}

fn key_envelope(share: ShareId, recipient: DeviceId, ciphertext: &[u8]) -> KeyEnvelopeRecord {
    KeyEnvelopeRecord {
        share_id: share,
        recipient_device_id: recipient,
        ephemeral_public_key: ENCRYPTION_KEY,
        ciphertext: ciphertext.to_vec(),
        created_at_unix_ns: NOW,
    }
}

fn unavailable<T: std::fmt::Debug>(outcome: &Result<T, RepositoryError>) -> bool {
    matches!(outcome, Err(RepositoryError::Unavailable(_)))
}

#[tokio::test]
async fn a_registration_round_trips() {
    with_mongo!(store, {
        let ada = user(ADA, "Ada", "7Q2XZ");

        store
            .insert_registration(ada.clone(), device(1, ADA))
            .await
            .expect("registration stored");

        assert_eq!(store.find_user(ADA).await, Ok(Some(ada.clone())));
        assert_eq!(
            store.find_user_by_handle("ada", "7Q2XZ").await,
            Ok(Some(ada))
        );
        assert_eq!(
            store.find_device([1; 32]).await,
            Ok(Some(device(1, ADA))),
            "binary identifiers survive the round trip"
        );
        assert_eq!(store.find_user([9; 16]).await, Ok(None));
        assert_eq!(store.find_device([9; 32]).await, Ok(None));
    });
}

/// `NexusStore::Mongo` only wraps `MongoStore`; this proves the wrapping
/// itself, not the storage it delegates to. Every trait method needs its own
/// arm in `store.rs`'s dispatch, and a match arm nobody calls is exactly the
/// kind of gap a backend swap would hit first in production, not in a test.
#[tokio::test]
async fn nexus_store_delegates_every_operation_to_mongo() {
    let Some(running) = mongo().await else {
        return;
    };
    let store = NexusStore::mongo(running.store.clone());
    assert_eq!(store.kind(), "mongodb");

    // A no-op for a durable store: only the in-memory backend can be forced
    // into a fault for testing.
    store.set_unavailable(true);

    let ada = user(ADA, "Ada", "7Q2XZ");
    store
        .insert_registration(ada.clone(), device(1, ADA))
        .await
        .expect("registration stored");
    assert_eq!(store.find_user(ADA).await, Ok(Some(ada.clone())));
    assert_eq!(
        store.find_user_by_handle("ada", "7Q2XZ").await,
        Ok(Some(ada))
    );
    assert_eq!(store.find_device([1; 32]).await, Ok(Some(device(1, ADA))));

    store.touch_device([1; 32], NOW + 5).await.expect("touched");
    store
        .revoke_device([1; 32], NOW + 9)
        .await
        .expect("revoked");
    let revoked = store
        .find_device([1; 32])
        .await
        .expect("stored")
        .expect("present");
    assert!(revoked.is_revoked());

    store.link_device(device(2, ADA)).await.expect("linked");
    assert_eq!(store.find_device([2; 32]).await, Ok(Some(device(2, ADA))));
    assert_eq!(
        store.link_device(device(2, ADA)).await,
        Err(RepositoryError::DeviceExists)
    );

    let edge = FriendshipEdge::between(ADA, GRACE).expect("distinct users");
    store
        .save_friendship(FriendshipRecord::requested(edge, ADA, NOW), 0)
        .await
        .expect("stored");
    assert!(store.find_friendship(edge).await.expect("stored").is_some());
    assert_eq!(store.list_friendships(ADA).await.expect("stored").len(), 1);
}

#[tokio::test]
async fn the_unique_index_decides_who_claims_a_handle() {
    with_mongo!(store, {
        store
            .insert_registration(user(ADA, "Ada", "7Q2XZ"), device(1, ADA))
            .await
            .expect("first registration");

        // Same handle, different user and device.
        let clash = store
            .insert_registration(user(GRACE, "Ada", "7Q2XZ"), device(2, GRACE))
            .await;

        assert_eq!(clash, Err(RepositoryError::HandleTaken));
        assert_eq!(
            store.find_device([2; 32]).await,
            Ok(None),
            "the transaction rolled back, so no device was enrolled"
        );
        assert_eq!(store.find_user(GRACE).await, Ok(None));
    });
}

#[tokio::test]
async fn a_device_can_only_be_enrolled_once() {
    with_mongo!(store, {
        store
            .insert_registration(user(ADA, "Ada", "7Q2XZ"), device(1, ADA))
            .await
            .expect("first registration");

        let clash = store
            .insert_registration(user(GRACE, "Grace", "ABCDE"), device(1, GRACE))
            .await;

        assert_eq!(clash, Err(RepositoryError::DeviceExists));
        assert_eq!(
            store.find_user(GRACE).await,
            Ok(None),
            "the rejected device took its user with it"
        );
    });
}

#[tokio::test]
async fn authentication_and_revocation_are_recorded() {
    with_mongo!(store, {
        store
            .insert_registration(user(ADA, "Ada", "7Q2XZ"), device(1, ADA))
            .await
            .expect("registration stored");

        store.touch_device([1; 32], NOW + 5).await.expect("touched");
        store
            .revoke_device([1; 32], NOW + 9)
            .await
            .expect("revoked");

        let stored = store
            .find_device([1; 32])
            .await
            .expect("stored")
            .expect("present");
        assert_eq!(stored.last_authenticated_at_unix_ns, Some(NOW + 5));
        assert_eq!(stored.revoked_at_unix_ns, Some(NOW + 9));
        assert!(stored.is_revoked());

        // Updating a device that is not there is a no-op, not an error.
        assert_eq!(store.touch_device([9; 32], NOW).await, Ok(()));
        assert_eq!(store.revoke_device([9; 32], NOW).await, Ok(()));
    });
}

#[tokio::test]
async fn a_friendship_write_must_match_the_stored_version() {
    with_mongo!(store, {
        let edge = FriendshipEdge::between(ADA, GRACE).expect("distinct users");
        let requested = FriendshipRecord::requested(edge, ADA, NOW);

        assert_eq!(store.save_friendship(requested.clone(), 0).await, Ok(()));
        assert_eq!(
            store.save_friendship(requested.clone(), 0).await,
            Err(RepositoryError::VersionConflict),
            "a second writer that read nothing must not overwrite"
        );

        let accepted = FriendshipRecord {
            state: FriendshipState::Accepted,
            version: 2,
            updated_at_unix_ns: NOW + 1,
            ..requested
        };
        assert_eq!(store.save_friendship(accepted.clone(), 1).await, Ok(()));
        assert_eq!(
            store.save_friendship(accepted.clone(), 1).await,
            Err(RepositoryError::VersionConflict),
            "the version moved, so a stale write is refused"
        );

        assert_eq!(store.find_friendship(edge).await, Ok(Some(accepted)));
    });
}

#[tokio::test]
async fn a_friendship_is_found_from_either_side() {
    with_mongo!(store, {
        let edge = FriendshipEdge::between(ADA, GRACE).expect("distinct users");
        store
            .save_friendship(FriendshipRecord::requested(edge, ADA, NOW), 0)
            .await
            .expect("stored");

        // Naming the edge the other way round finds the same row.
        let reversed = FriendshipEdge::between(GRACE, ADA).expect("distinct users");
        assert!(
            store
                .find_friendship(reversed)
                .await
                .expect("read")
                .is_some()
        );

        for user in [ADA, GRACE] {
            let listed = store.list_friendships(user).await.expect("listed");
            assert_eq!(listed.len(), 1, "both sides see one friendship");
            assert_eq!(listed[0].edge, edge);
        }
        assert_eq!(store.list_friendships([9; 16]).await, Ok(Vec::new()));
    });
}

/// One row per share and recipient device, enforced by the unique index.
///
/// Rotating a share key pushes a second envelope for the same pair; if that
/// piled up beside its predecessor rather than replacing it, a device would
/// fetch a stale key beside the current one with nothing to tell them apart.
#[tokio::test]
async fn a_rotated_key_replaces_the_envelope_it_supersedes() {
    with_mongo!(store, {
        let share = share_id(0);
        store
            .put_key_envelope(key_envelope(share, RECIPIENT, b"first"))
            .await
            .expect("envelope stored");

        let page = store
            .list_key_envelopes(RECIPIENT, None)
            .await
            .expect("listed");
        assert_eq!(
            page.envelopes,
            vec![key_envelope(share, RECIPIENT, b"first")],
            "binary identifiers and ciphertext survive the round trip"
        );
        assert_eq!(page.next_after_share_id, None);

        store
            .put_key_envelope(key_envelope(share, RECIPIENT, b"rotated"))
            .await
            .expect("rotated envelope stored");

        let page = store
            .list_key_envelopes(RECIPIENT, None)
            .await
            .expect("listed");
        assert_eq!(
            page.envelopes,
            vec![key_envelope(share, RECIPIENT, b"rotated")],
            "the rotated key replaced its predecessor rather than joining it"
        );
    });
}

#[tokio::test]
async fn envelopes_are_listed_only_for_the_device_they_name() {
    with_mongo!(store, {
        let share = share_id(0);
        store
            .put_key_envelope(key_envelope(share, RECIPIENT, b"theirs"))
            .await
            .expect("stored");
        store
            .put_key_envelope(key_envelope(share, OTHER_RECIPIENT, b"someone else's"))
            .await
            .expect("stored");

        let page = store
            .list_key_envelopes(RECIPIENT, None)
            .await
            .expect("listed");
        assert_eq!(
            page.envelopes,
            vec![key_envelope(share, RECIPIENT, b"theirs")]
        );

        // A device nobody has pushed to sees nothing rather than everything.
        let page = store
            .list_key_envelopes([9; 32], None)
            .await
            .expect("listed");
        assert_eq!(page.envelopes, Vec::new());
        assert_eq!(page.next_after_share_id, None);
    });
}

/// The page boundary, against a server that does the sorting.
///
/// Listing orders by share ID in the database and resumes with a `$gt`
/// cursor, so this is where `MongoDB`'s ordering of binary values has to agree
/// with the bytewise order the page type assumes; `KeyEnvelopePage` debug
/// asserts that agreement, and a debug build would trip on it here.
#[tokio::test]
async fn a_full_page_reports_where_the_next_one_starts() {
    with_mongo!(store, {
        let total = MAX_KEY_ENVELOPES_PER_PAGE + 1;
        for index in 0..total {
            let index = u16::try_from(index).expect("the page limit fits");
            store
                .put_key_envelope(key_envelope(share_id(index), RECIPIENT, b"cipher"))
                .await
                .expect("stored");
        }

        let first = store
            .list_key_envelopes(RECIPIENT, None)
            .await
            .expect("listed");
        assert_eq!(first.envelopes.len(), MAX_KEY_ENVELOPES_PER_PAGE);
        assert_eq!(
            first.envelopes.first().expect("a full page").share_id,
            share_id(0),
            "the page starts at the lowest share ID"
        );
        let last_on_page =
            u16::try_from(MAX_KEY_ENVELOPES_PER_PAGE - 1).expect("the page limit fits");
        assert_eq!(
            first.envelopes.last().expect("a full page").share_id,
            share_id(last_on_page),
            "the page is ordered by share ID, not by insertion"
        );
        assert_eq!(first.next_after_share_id, Some(share_id(last_on_page)));

        let second = store
            .list_key_envelopes(RECIPIENT, first.next_after_share_id)
            .await
            .expect("listed");
        let remaining = u16::try_from(MAX_KEY_ENVELOPES_PER_PAGE).expect("the page limit fits");
        assert_eq!(
            second.envelopes.len(),
            1,
            "the cursor resumes after the last row of the first page"
        );
        assert_eq!(
            second.envelopes.first().expect("one row").share_id,
            share_id(remaining)
        );
        assert_eq!(second.next_after_share_id, None, "nothing follows");
    });
}

#[tokio::test]
async fn indexes_are_created_more_than_once_without_complaint() {
    with_mongo!(store, {
        // Startup runs this every time, so it has to be idempotent.
        assert_eq!(store.prepare().await, Ok(()));
        assert_eq!(store.prepare().await, Ok(()));
    });
}

/// A standalone `MongoDB` supports everything registration needs except
/// transactions, so this is the one server failure the "unreachable" test
/// above cannot reach: the session opens fine, but starting a transaction on
/// it fails. A real deployment pointed at the wrong kind of server hits
/// exactly this.
#[tokio::test]
async fn a_standalone_server_cannot_start_the_registration_transaction() {
    if let Some(reason) = docker_failure().get() {
        eprintln!("skipping: Docker was already unavailable ({reason})");
        return;
    }
    let _docker_guard = docker_gate().lock_owned().await;
    let container = match timeout(
        CONTAINER_START_TIMEOUT,
        testcontainers_modules::mongo::Mongo::default().start(),
    )
    .await
    {
        Ok(Ok(container)) => container,
        Ok(Err(error)) if docker_is_available() => {
            remember_docker_failure(error.to_string());
            panic!("Docker is running but MongoDB would not start: {error}")
        }
        Ok(Err(error)) => {
            remember_docker_failure(error.to_string());
            eprintln!("skipping: no Docker available ({error})");
            return;
        }
        Err(_) => {
            let reason = format!(
                "MongoDB standalone server did not start within {} seconds",
                CONTAINER_START_TIMEOUT.as_secs()
            );
            remember_docker_failure(reason.clone());
            panic!("{reason}");
        }
    };
    let port = timeout(CONTAINER_START_TIMEOUT, container.get_host_port_ipv4(27017))
        .await
        .unwrap_or_else(|_| {
            panic!(
                "MongoDB standalone server did not publish its port within {} seconds",
                CONTAINER_START_TIMEOUT.as_secs()
            )
        })
        .expect("the container publishes its port");
    let uri = format!(
        "mongodb://127.0.0.1:{port}/?directConnection=true&serverSelectionTimeoutMS={SELECTION_TIMEOUT_MS}"
    );
    let database = scratch_database();
    let store = connect_store(&uri, &database, "indexes do not need a replica set").await;

    let outcome = store
        .insert_registration(user(ADA, "Ada", "7Q2XZ"), device(1, ADA))
        .await;

    assert!(
        matches!(outcome, Err(RepositoryError::Unavailable(_))),
        "expected an outage, got {outcome:?}"
    );
}

/// Every read and write reports the same outage once the server is gone
/// mid-session, not just the ones the tests above happen to reach: a caller
/// should never see a hang, a panic, or a silently empty result in its place.
#[tokio::test]
async fn every_operation_reports_an_outage_once_the_server_is_gone() {
    let Some(running) = mongo().await else {
        return;
    };
    let Some(container) = running.container.as_ref() else {
        eprintln!("skipping: needs a container this test can stop");
        return;
    };

    // Real data first, so reads and updates have something to look for right
    // up until the moment the server disappears.
    running
        .store
        .insert_registration(user(ADA, "Ada", "7Q2XZ"), device(1, ADA))
        .await
        .expect("registration stored");
    let edge = FriendshipEdge::between(ADA, GRACE).expect("distinct users");
    running
        .store
        .save_friendship(FriendshipRecord::requested(edge, ADA, NOW), 0)
        .await
        .expect("stored");

    let store = &running.store;
    stop_container(container).await;

    let find_user = store.find_user(ADA).await;
    assert!(unavailable(&find_user), "find_user: {find_user:?}");

    let find_user_by_handle = store.find_user_by_handle("ada", "7Q2XZ").await;
    assert!(
        unavailable(&find_user_by_handle),
        "find_user_by_handle: {find_user_by_handle:?}"
    );

    let find_device = store.find_device([1; 32]).await;
    assert!(unavailable(&find_device), "find_device: {find_device:?}");

    let touch_device = store.touch_device([1; 32], NOW + 1).await;
    assert!(unavailable(&touch_device), "touch_device: {touch_device:?}");

    let revoke_device = store.revoke_device([1; 32], NOW + 1).await;
    assert!(
        unavailable(&revoke_device),
        "revoke_device: {revoke_device:?}"
    );

    let find_friendship = store.find_friendship(edge).await;
    assert!(
        unavailable(&find_friendship),
        "find_friendship: {find_friendship:?}"
    );

    let accepted = FriendshipRecord {
        state: FriendshipState::Accepted,
        version: 2,
        updated_at_unix_ns: NOW + 1,
        ..FriendshipRecord::requested(edge, ADA, NOW)
    };
    let save_friendship = store.save_friendship(accepted, 1).await;
    assert!(
        unavailable(&save_friendship),
        "save_friendship: {save_friendship:?}"
    );

    let list_friendships = store.list_friendships(ADA).await;
    assert!(
        unavailable(&list_friendships),
        "list_friendships: {list_friendships:?}"
    );

    // Registration cannot even open the session its transaction needs.
    let insert_registration = store
        .insert_registration(user(GRACE, "Grace", "ABCDE"), device(2, GRACE))
        .await;
    assert!(
        unavailable(&insert_registration),
        "insert_registration: {insert_registration:?}"
    );

    let put_key_envelope = store
        .put_key_envelope(key_envelope(share_id(0), RECIPIENT, b"cipher"))
        .await;
    assert!(
        unavailable(&put_key_envelope),
        "put_key_envelope: {put_key_envelope:?}"
    );

    let list_key_envelopes = store.list_key_envelopes(RECIPIENT, None).await;
    assert!(
        unavailable(&list_key_envelopes),
        "list_key_envelopes: {list_key_envelopes:?}"
    );
}

/// A connection string that is not a connection string. Misconfiguration
/// should read as an outage like any other, rather than panicking at startup.
#[tokio::test]
async fn a_malformed_connection_string_is_refused() {
    let outcome = MongoStore::connect("not-a-connection-string", "nexus_test").await;

    assert!(
        matches!(outcome, Err(RepositoryError::Unavailable(_))),
        "expected an outage, got {outcome:?}"
    );
}

/// A connection string that parses but describes something impossible: a
/// direct connection cannot name two servers. The driver rejects this when
/// the client is built rather than when it is first used.
#[tokio::test]
async fn contradictory_connection_options_are_refused() {
    let outcome = MongoStore::connect(
        "mongodb://127.0.0.1:27017,127.0.0.1:27018/?directConnection=true",
        "nexus_test",
    )
    .await;

    assert!(
        matches!(outcome, Err(RepositoryError::Unavailable(_))),
        "expected an outage, got {outcome:?}"
    );
}

#[tokio::test]
async fn an_unreachable_server_is_reported_rather_than_hung() {
    // No container: a port nothing is listening on, with a short timeout.
    let outcome = MongoStore::connect(
        "mongodb://127.0.0.1:1/?directConnection=true&serverSelectionTimeoutMS=500",
        "nexus_test",
    )
    .await;

    assert!(
        matches!(outcome, Err(RepositoryError::Unavailable(_))),
        "expected an outage, got {outcome:?}"
    );
}

/// `classify` exists so a unique index rejecting a write reads as a lost
/// race, not an outage. This is the other half of that promise: a write that
/// fails for any other reason must still read as an outage, not as a
/// conflict nobody actually lost.
#[tokio::test]
async fn a_write_that_loses_its_server_is_reported_as_an_outage() {
    let Some(running) = mongo().await else {
        return;
    };
    let Some(container) = running.container.as_ref() else {
        eprintln!("skipping: needs a container this test can stop");
        return;
    };

    let store = &running.store;
    stop_container(container).await;

    let edge = FriendshipEdge::between(ADA, GRACE).expect("distinct users");
    let outcome = store
        .save_friendship(FriendshipRecord::requested(edge, ADA, NOW), 0)
        .await;

    assert!(
        matches!(outcome, Err(RepositoryError::Unavailable(_))),
        "expected an outage, got {outcome:?}"
    );
}

/// The server identity used when signing, which a signature is bound to.
const AUTHORITY: &str = "nexus.test";

fn binding(challenge: &[u8; 32]) -> SessionBinding<'_> {
    SessionBinding {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        server_authority: AUTHORITY,
        connection_id: &[4; 16],
        challenge,
        server_time_unix_ns: NOW,
    }
}

/// The identity rules applied over a durable store.
fn service(store: MongoStore) -> IdentityService<MongoStore, FixedClock, ScriptedRandom> {
    IdentityService::new(store, FixedClock::new(NOW), ScriptedRandom::new(&[9]))
}

fn registration<'a>(
    signer: &SigningKey,
    username: &'a str,
    challenge: &'a [u8; 32],
    public_key: &'a mut [u8; 32],
    signature: &'a mut [u8; 64],
) -> RegistrationRequest<'a> {
    *public_key = signer.verifying_key().to_bytes();
    let payload = registration_payload(&binding(challenge), username, public_key, &ENCRYPTION_KEY);
    *signature = signer.sign(&payload).to_bytes();
    RegistrationRequest {
        binding: binding(challenge),
        requested_username: username,
        device_public_key: public_key,
        encryption_public_key: &ENCRYPTION_KEY,
        signature,
    }
}

fn authentication<'a>(
    signer: &SigningKey,
    challenge: &'a [u8; 32],
    public_key: &'a mut [u8; 32],
    signature: &'a mut [u8; 64],
) -> AuthenticationRequest<'a> {
    *public_key = signer.verifying_key().to_bytes();
    let payload = authentication_payload(&binding(challenge), public_key);
    *signature = signer.sign(&payload).to_bytes();
    AuthenticationRequest {
        binding: binding(challenge),
        device_public_key: public_key,
        signature,
    }
}

/// The claim that only a durable store can make.
///
/// The second service shares nothing with the first but the database, so an
/// identity it can authenticate was read back from disk rather than remembered.
#[tokio::test]
async fn an_identity_outlives_the_process_that_registered_it() {
    let Some(running) = mongo().await else {
        return;
    };
    let signer = SigningKey::from_bytes(&[7; 32]);
    let (mut public, mut signature) = ([0; 32], [0; 64]);

    let registered = service(running.store.clone())
        .register(registration(
            &signer,
            "Ada",
            &[1; 32],
            &mut public,
            &mut signature,
        ))
        .await
        .expect("registration succeeds");

    let authenticated = service(running.restart().await)
        .authenticate(authentication(
            &signer,
            &[2; 32],
            &mut public,
            &mut signature,
        ))
        .await
        .expect("the stored identity still authenticates");

    assert_eq!(authenticated.user.user_id, registered.user.user_id);
    assert_eq!(authenticated.user.username, "Ada");
    assert_eq!(
        authenticated.user.discriminator,
        registered.user.discriminator
    );
    assert_eq!(authenticated.device.device_id, registered.device.device_id);
}

/// Revocation has to outlive the process too, or a restart would readmit a
/// device its owner had already disowned.
#[tokio::test]
async fn a_revoked_device_stays_revoked_across_a_restart() {
    let Some(running) = mongo().await else {
        return;
    };
    let signer = SigningKey::from_bytes(&[8; 32]);
    let (mut public, mut signature) = ([0; 32], [0; 64]);

    let registered = service(running.store.clone())
        .register(registration(
            &signer,
            "Grace",
            &[1; 32],
            &mut public,
            &mut signature,
        ))
        .await
        .expect("registration succeeds");

    service(running.restart().await)
        .revoke_device(registered.device.device_id)
        .await
        .expect("revocation succeeds");

    let refused = service(running.restart().await)
        .authenticate(authentication(
            &signer,
            &[3; 32],
            &mut public,
            &mut signature,
        ))
        .await
        .expect_err("a revoked device cannot authenticate");

    assert_eq!(refused, IdentityError::DeviceRevoked);
}

/// The claim only a durable store can make, applied to linking: a device
/// approved by one process authenticates from a second one holding nothing
/// but the database.
#[tokio::test]
async fn a_linked_device_outlives_the_process_that_linked_it() {
    let Some(running) = mongo().await else {
        return;
    };
    let approver = SigningKey::from_bytes(&[9; 32]);
    let (mut approver_public, mut approver_signature) = ([0; 32], [0; 64]);
    service(running.store.clone())
        .register(registration(
            &approver,
            "Ada",
            &[1; 32],
            &mut approver_public,
            &mut approver_signature,
        ))
        .await
        .expect("registration succeeds");

    let candidate = SigningKey::from_bytes(&[10; 32]);
    let candidate_public = candidate.verifying_key().to_bytes();
    let candidate_encryption_key = [11; 32];
    let payload = link_device_payload(AUTHORITY, &candidate_public, &candidate_encryption_key);
    let approval = approver.sign(&payload).to_bytes();

    let linked = service(running.restart().await)
        .link_device(
            derive_device_id(&approver_public),
            AUTHORITY,
            LinkDeviceRequest {
                candidate_signing_public_key: &candidate_public,
                candidate_encryption_public_key: &candidate_encryption_key,
                approval_signature: &approval,
            },
        )
        .await
        .expect("linking succeeds");

    let (mut public, mut signature) = (candidate_public, [0; 64]);
    let authenticated = service(running.restart().await)
        .authenticate(authentication(
            &candidate,
            &[2; 32],
            &mut public,
            &mut signature,
        ))
        .await
        .expect("the linked device authenticates from a fresh process");

    assert_eq!(authenticated.device.device_id, linked.device.device_id);
    assert_eq!(authenticated.user.user_id, linked.user.user_id);
    assert_eq!(
        authenticated.device.encryption_public_key,
        candidate_encryption_key
    );
}

#[tokio::test]
async fn share_history_membership_and_head_cas_survive_a_restart() {
    let Some(running) = mongo().await else {
        return;
    };
    let share_id = [55; 16];
    let snapshot_id = [56; 32];
    let share = ShareRecord {
        share_id,
        owner: ADA,
        revision: 1,
        snapshot_id,
        capsule: b"encrypted capsule".to_vec(),
        capsule_signature: vec![9; 64],
        created_at_unix_ns: NOW,
        updated_at_unix_ns: NOW,
    };
    let snapshot = ShareSnapshotRecord {
        share_id,
        revision: 1,
        snapshot_id,
        capsule: share.capsule.clone(),
        capsule_signature: share.capsule_signature.clone(),
        created_at_unix_ns: NOW,
    };

    running
        .store
        .save_publication(share.clone(), snapshot.clone(), None)
        .await
        .expect("first publication");
    running
        .store
        .grant_share_access(ShareMembershipRecord {
            share_id,
            user_id: GRACE,
            granted_at_unix_ns: NOW,
        })
        .await
        .expect("membership stored");

    let restarted = running.restart().await;
    assert_eq!(
        restarted.find_share(share_id).await,
        Ok(Some(share.clone()))
    );
    assert_eq!(
        restarted.find_snapshot(share_id, 1).await,
        Ok(Some(snapshot))
    );
    assert!(restarted.has_share_access(share_id, GRACE).await.unwrap());
    assert_eq!(
        restarted
            .list_authorized_shares(GRACE)
            .await
            .expect("listed"),
        vec![share.clone()]
    );

    let stale = ShareRecord {
        revision: 2,
        snapshot_id: [57; 32],
        ..share
    };
    let stale_snapshot = ShareSnapshotRecord {
        share_id,
        revision: 2,
        snapshot_id: [57; 32],
        capsule: b"next".to_vec(),
        capsule_signature: vec![8; 64],
        created_at_unix_ns: NOW + 1,
    };
    assert_eq!(
        restarted
            .save_publication(stale, stale_snapshot, Some(0))
            .await,
        Err(RepositoryError::VersionConflict)
    );
    assert!(
        restarted
            .find_snapshot(share_id, 2)
            .await
            .unwrap()
            .is_none(),
        "the transaction leaves no orphan snapshot after a lost head CAS"
    );
}
