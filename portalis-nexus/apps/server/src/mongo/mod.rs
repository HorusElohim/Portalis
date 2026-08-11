//! The durable identity and friend store.
//!
//! Uniqueness is enforced by indexes rather than by reading first: two servers
//! racing to claim the same handle both write, and exactly one wins. The same
//! is true of a friendship's version, which is why every write here filters on
//! the value it read.

use futures_util::TryStreamExt as _;
use mongodb::bson::{Document, doc};
use mongodb::error::{ErrorKind, WriteFailure};
use mongodb::options::{ClientOptions, IndexOptions};
use mongodb::{Client, ClientSession, Collection, Database, IndexModel};
use portalis_nexus_server_core::{
    DeviceId, DeviceRecord, EnvelopeRepository, FriendRepository, FriendshipEdge, FriendshipRecord,
    IdentityRepository, KeyEnvelopePage, KeyEnvelopeRecord, RepositoryError, ShareId,
    ShareMembershipRecord, ShareRecord, ShareRepository, ShareSnapshotRecord, UserDirectory,
    UserId, UserRecord,
};
use tracing::debug;

mod documents;

use documents::{
    DeviceDocument, FriendshipDocument, KeyEnvelopeDocument, ShareDocument,
    ShareMembershipDocument, ShareSnapshotDocument, UserDocument, binary, signed,
};

/// The duplicate-key code every unique index reports.
const DUPLICATE_KEY: i32 = 11_000;

/// How many times an envelope upsert retries a lost insert race before
/// reporting it. One retry is enough: the row exists after the first loss, so
/// the second attempt replaces it rather than inserting again.
const PUT_ENVELOPE_ATTEMPTS: u8 = 2;

const USERS: &str = "users";
const DEVICES: &str = "devices";
const FRIENDSHIPS: &str = "friendships";
const KEY_ENVELOPES: &str = "key_envelopes";
const SHARES: &str = "shares";
const SHARE_SNAPSHOTS: &str = "share_snapshots";
const SHARE_MEMBERSHIPS: &str = "share_memberships";

/// Identity and friend storage backed by `MongoDB`.
#[derive(Clone, Debug)]
pub struct MongoStore {
    database: Database,
    client: Client,
}

impl MongoStore {
    /// Connects to `uri` and prepares the indexes this server relies on.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryError::Unavailable`] when the server cannot be
    /// reached or the indexes cannot be created.
    pub async fn connect(uri: &str, database: &str) -> Result<Self, RepositoryError> {
        let options = ClientOptions::parse(uri).await.map_err(unavailable)?;
        let client = Client::with_options(options).map_err(unavailable)?;
        let store = Self {
            database: client.database(database),
            client,
        };
        store.prepare().await?;
        Ok(store)
    }

    /// Builds a store pointed at a server that is not there, without
    /// contacting anything.
    ///
    /// The driver connects lazily, so this is the one way a unit test can
    /// hold a `MongoStore` at all: every operation on it gives up quickly and
    /// reports an outage. Tests that need real storage use the replica set in
    /// `tests/mongo.rs` instead.
    #[cfg(test)]
    pub(crate) fn disconnected() -> Self {
        let options = ClientOptions::builder()
            .hosts(vec![mongodb::options::ServerAddress::Tcp {
                // Port 1 is reserved, so nothing can be listening on it.
                host: "127.0.0.1".to_owned(),
                port: Some(1),
            }])
            .direct_connection(true)
            .server_selection_timeout(std::time::Duration::from_millis(10))
            .build();
        let client = Client::with_options(options).expect("the options are valid");
        Self {
            database: client.database("nexus_disconnected"),
            client,
        }
    }

    /// Creates the indexes that make handles, devices, and friendships unique.
    ///
    /// Index creation is idempotent, so this runs on every start. Built from
    /// one shared collection-and-index list rather than one call per index,
    /// so a server that goes away mid-`prepare` is exercised through a single
    /// error path instead of one copy per index.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryError::Unavailable`] when an index cannot be built.
    pub async fn prepare(&self) -> Result<(), RepositoryError> {
        let indexes = [
            // One handle per user: this is what makes allocation safe to
            // retry rather than scan.
            (
                USERS,
                unique(doc! { "normalized_username": 1, "discriminator": 1 }),
            ),
            // Listing a user's devices, and finding the live ones.
            (
                DEVICES,
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "revoked_at_unix_ns": 1 })
                    .build(),
            ),
            // One row per friendship, whichever side asked.
            (FRIENDSHIPS, unique(doc! { "user_low": 1, "user_high": 1 })),
            // Listing from either side of the edge.
            (
                FRIENDSHIPS,
                IndexModel::builder()
                    .keys(doc! { "user_low": 1, "state": 1 })
                    .build(),
            ),
            (
                FRIENDSHIPS,
                IndexModel::builder()
                    .keys(doc! { "user_high": 1, "state": 1 })
                    .build(),
            ),
            // One envelope per share and recipient device, so a rotated key
            // replaces its predecessor rather than piling up beside it.
            (
                KEY_ENVELOPES,
                unique(doc! { "share_id": 1, "recipient_device_id": 1 }),
            ),
            // A device fetching everything addressed to it.
            (
                KEY_ENVELOPES,
                IndexModel::builder()
                    .keys(doc! { "recipient_device_id": 1 })
                    .build(),
            ),
            (
                SHARES,
                IndexModel::builder()
                    .keys(doc! { "owner_user_id": 1 })
                    .build(),
            ),
            (
                SHARE_SNAPSHOTS,
                unique(doc! { "share_id": 1, "revision": 1 }),
            ),
            (
                SHARE_MEMBERSHIPS,
                unique(doc! { "share_id": 1, "user_id": 1 }),
            ),
            (
                SHARE_MEMBERSHIPS,
                IndexModel::builder().keys(doc! { "user_id": 1 }).build(),
            ),
        ];
        for (collection, index) in indexes {
            self.database
                .collection::<Document>(collection)
                .create_index(index)
                .await
                .map_err(unavailable)?;
        }
        debug!("identity indexes are ready");
        Ok(())
    }

    fn users(&self) -> Collection<UserDocument> {
        self.database.collection(USERS)
    }

    fn devices(&self) -> Collection<DeviceDocument> {
        self.database.collection(DEVICES)
    }

    fn friendships(&self) -> Collection<FriendshipDocument> {
        self.database.collection(FRIENDSHIPS)
    }

    fn key_envelopes(&self) -> Collection<KeyEnvelopeDocument> {
        self.database.collection(KEY_ENVELOPES)
    }

    fn shares(&self) -> Collection<ShareDocument> {
        self.database.collection(SHARES)
    }

    fn share_snapshots(&self) -> Collection<ShareSnapshotDocument> {
        self.database.collection(SHARE_SNAPSHOTS)
    }

    fn share_memberships(&self) -> Collection<ShareMembershipDocument> {
        self.database.collection(SHARE_MEMBERSHIPS)
    }

    /// Writes a user and its first device inside one transaction.
    async fn write_registration(
        &self,
        session: &mut ClientSession,
        user: &UserRecord,
        device: &DeviceRecord,
    ) -> Result<(), RepositoryError> {
        self.users()
            .insert_one(UserDocument::from_record(user))
            .session(&mut *session)
            .await
            .map_err(|error| classify(&error, RepositoryError::HandleTaken))?;
        self.devices()
            .insert_one(DeviceDocument::from_record(device))
            .session(&mut *session)
            .await
            .map_err(|error| classify(&error, RepositoryError::DeviceExists))?;
        Ok(())
    }
}

/// A unique index over `keys`.
fn unique(keys: Document) -> IndexModel {
    IndexModel::builder()
        .keys(keys)
        .options(IndexOptions::builder().unique(true).build())
        .build()
}

/// Reports a driver failure as an outage.
///
/// By value so it can be passed to `map_err` by name at every call site.
#[allow(clippy::needless_pass_by_value)]
fn unavailable(error: mongodb::error::Error) -> RepositoryError {
    RepositoryError::Unavailable(error.to_string())
}

/// Turns a duplicate-key failure into `duplicate`, and anything else into an
/// outage. A unique index rejecting a write is the store working, not failing.
fn classify(error: &mongodb::error::Error, duplicate: RepositoryError) -> RepositoryError {
    if is_duplicate_key(error) {
        return duplicate;
    }
    RepositoryError::Unavailable(error.to_string())
}

/// Every write here is a single document (`insert_one` or `update_one`), so
/// only `WriteError` is checked; a bulk failure shape can't occur.
fn is_duplicate_key(error: &mongodb::error::Error) -> bool {
    matches!(
        error.kind.as_ref(),
        ErrorKind::Write(WriteFailure::WriteError(write)) if write.code == DUPLICATE_KEY
    )
}

impl UserDirectory for MongoStore {
    fn find_user(
        &self,
        user_id: UserId,
    ) -> impl std::future::Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send {
        let store = self.clone();
        async move {
            let found = store
                .users()
                .find_one(doc! { "_id": binary(&user_id) })
                .await
                .map_err(unavailable)?;
            Ok(found.and_then(UserDocument::into_record))
        }
    }

    fn find_user_by_handle(
        &self,
        normalized_username: &str,
        discriminator: &str,
    ) -> impl std::future::Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send {
        let store = self.clone();
        let filter = doc! {
            "normalized_username": normalized_username,
            "discriminator": discriminator,
        };
        async move {
            let found = store.users().find_one(filter).await.map_err(unavailable)?;
            Ok(found.and_then(UserDocument::into_record))
        }
    }
}

impl IdentityRepository for MongoStore {
    fn insert_registration(
        &self,
        user: UserRecord,
        device: DeviceRecord,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let store = self.clone();
        async move {
            let mut session = store.client.start_session().await.map_err(unavailable)?;
            session.start_transaction().await.map_err(unavailable)?;

            match store.write_registration(&mut session, &user, &device).await {
                Ok(()) => session.commit_transaction().await.map_err(unavailable),
                Err(error) => {
                    // Abort so a rejected handle leaves no user behind; the
                    // caller retries with another discriminator.
                    let _ = session.abort_transaction().await;
                    Err(error)
                }
            }
        }
    }

    fn find_device(
        &self,
        device_id: DeviceId,
    ) -> impl std::future::Future<Output = Result<Option<DeviceRecord>, RepositoryError>> + Send
    {
        let store = self.clone();
        async move {
            let found = store
                .devices()
                .find_one(doc! { "_id": binary(&device_id) })
                .await
                .map_err(unavailable)?;
            Ok(found.and_then(DeviceDocument::into_record))
        }
    }

    fn link_device(
        &self,
        device: DeviceRecord,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let store = self.clone();
        async move {
            store
                .devices()
                .insert_one(DeviceDocument::from_record(&device))
                .await
                .map(|_| ())
                .map_err(|error| classify(&error, RepositoryError::DeviceExists))
        }
    }

    fn touch_device(
        &self,
        device_id: DeviceId,
        at_unix_ns: u64,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let store = self.clone();
        async move {
            store
                .devices()
                .update_one(
                    doc! { "_id": binary(&device_id) },
                    doc! { "$set": { "last_authenticated_at_unix_ns": signed(at_unix_ns) } },
                )
                .await
                .map_err(unavailable)?;
            Ok(())
        }
    }

    fn revoke_device(
        &self,
        device_id: DeviceId,
        at_unix_ns: u64,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let store = self.clone();
        async move {
            store
                .devices()
                .update_one(
                    doc! { "_id": binary(&device_id) },
                    doc! { "$set": { "revoked_at_unix_ns": signed(at_unix_ns) } },
                )
                .await
                .map_err(unavailable)?;
            Ok(())
        }
    }
}

impl FriendRepository for MongoStore {
    fn find_friendship(
        &self,
        edge: FriendshipEdge,
    ) -> impl std::future::Future<Output = Result<Option<FriendshipRecord>, RepositoryError>> + Send
    {
        let store = self.clone();
        async move {
            let found = store
                .friendships()
                .find_one(edge_filter(edge))
                .await
                .map_err(unavailable)?;
            Ok(found.and_then(FriendshipDocument::into_record))
        }
    }

    fn save_friendship(
        &self,
        record: FriendshipRecord,
        expected_version: u64,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let store = self.clone();
        async move {
            let document = FriendshipDocument::from_record(&record);
            if expected_version == 0 {
                // No edge yet: the unique index decides who created it.
                return store
                    .friendships()
                    .insert_one(document)
                    .await
                    .map(|_| ())
                    .map_err(|error| classify(&error, RepositoryError::VersionConflict));
            }

            let mut filter = edge_filter(record.edge);
            filter.insert("version", signed(expected_version));
            let outcome = store
                .friendships()
                .replace_one(filter, document)
                .await
                .map_err(unavailable)?;
            if outcome.matched_count == 0 {
                // Someone wrote first; the caller re-reads and re-applies.
                return Err(RepositoryError::VersionConflict);
            }
            Ok(())
        }
    }

    fn list_friendships(
        &self,
        user: UserId,
    ) -> impl std::future::Future<Output = Result<Vec<FriendshipRecord>, RepositoryError>> + Send
    {
        let store = self.clone();
        async move {
            let filter = doc! {
                "$or": [
                    { "user_low": binary(&user) },
                    { "user_high": binary(&user) },
                ]
            };
            let found: Vec<_> = store
                .friendships()
                .find(filter)
                .await
                .map_err(unavailable)?
                .try_collect()
                .await
                .map_err(unavailable)?;
            Ok(found
                .into_iter()
                .filter_map(FriendshipDocument::into_record)
                .collect())
        }
    }
}

impl EnvelopeRepository for MongoStore {
    fn put_key_envelope(
        &self,
        envelope: KeyEnvelopeRecord,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let store = self.clone();
        async move {
            let mut remaining = PUT_ENVELOPE_ATTEMPTS;
            loop {
                let filter = doc! {
                    "share_id": binary(&envelope.share_id),
                    "recipient_device_id": binary(&envelope.recipient_device_id),
                };
                let outcome = store
                    .key_envelopes()
                    .replace_one(filter, KeyEnvelopeDocument::from_record(&envelope))
                    // A rotated key replaces its predecessor, and the first
                    // push for a device has nothing to replace.
                    .upsert(true)
                    .await;
                let Err(error) = outcome else {
                    return Ok(());
                };
                remaining -= 1;
                // Two writers can both find no row and both insert, and the
                // unique index rejects the loser. Retrying finds the row the
                // winner wrote and replaces it, which is what the caller
                // asked for either way. Reporting an outage instead would
                // lose a rotated key while telling the caller it was stored.
                if remaining == 0 || !is_duplicate_key(&error) {
                    return Err(unavailable(error));
                }
            }
        }
    }

    fn list_key_envelopes(
        &self,
        recipient_device_id: DeviceId,
        after_share_id: Option<ShareId>,
    ) -> impl std::future::Future<Output = Result<KeyEnvelopePage, RepositoryError>> + Send {
        let store = self.clone();
        async move {
            let mut filter = doc! { "recipient_device_id": binary(&recipient_device_id) };
            if let Some(after_share_id) = after_share_id {
                filter.insert("share_id", doc! { "$gt": binary(&after_share_id) });
            }
            let query_limit =
                i64::try_from(portalis_nexus_protocol::MAX_KEY_ENVELOPES_PER_PAGE + 1)
                    .map_err(|error| RepositoryError::Unavailable(error.to_string()))?;
            let found: Vec<_> = store
                .key_envelopes()
                .find(filter)
                .sort(doc! { "share_id": 1 })
                .limit(query_limit)
                .await
                .map_err(unavailable)?
                .try_collect()
                .await
                .map_err(unavailable)?;
            Ok(KeyEnvelopePage::from_sorted(
                found
                    .into_iter()
                    .filter_map(KeyEnvelopeDocument::into_record)
                    .collect(),
            ))
        }
    }
}

impl ShareRepository for MongoStore {
    fn find_share(
        &self,
        share_id: ShareId,
    ) -> impl std::future::Future<Output = Result<Option<ShareRecord>, RepositoryError>> + Send
    {
        let store = self.clone();
        async move {
            let found = store
                .shares()
                .find_one(doc! { "_id": binary(&share_id) })
                .await
                .map_err(unavailable)?;
            Ok(found.and_then(ShareDocument::into_record))
        }
    }

    fn save_publication(
        &self,
        share: ShareRecord,
        snapshot: ShareSnapshotRecord,
        expected_revision: Option<u64>,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let store = self.clone();
        async move {
            let mut session = store.client.start_session().await.map_err(unavailable)?;
            session.start_transaction().await.map_err(unavailable)?;
            let write = async {
                store
                    .share_snapshots()
                    .insert_one(ShareSnapshotDocument::from_record(&snapshot))
                    .session(&mut session)
                    .await
                    .map_err(|error| classify(&error, RepositoryError::VersionConflict))?;

                if let Some(expected) = expected_revision {
                    let outcome = store
                        .shares()
                        .replace_one(
                            doc! {
                                "_id": binary(&share.share_id),
                                "revision": signed(expected),
                            },
                            ShareDocument::from_record(&share),
                        )
                        .session(&mut session)
                        .await
                        .map_err(unavailable)?;
                    if outcome.matched_count == 0 {
                        return Err(RepositoryError::VersionConflict);
                    }
                } else {
                    store
                        .shares()
                        .insert_one(ShareDocument::from_record(&share))
                        .session(&mut session)
                        .await
                        .map_err(|error| classify(&error, RepositoryError::VersionConflict))?;
                }
                Ok(())
            }
            .await;
            match write {
                Ok(()) => session.commit_transaction().await.map_err(unavailable),
                Err(error) => {
                    let _ = session.abort_transaction().await;
                    Err(error)
                }
            }
        }
    }

    fn find_snapshot(
        &self,
        share_id: ShareId,
        revision: u64,
    ) -> impl std::future::Future<Output = Result<Option<ShareSnapshotRecord>, RepositoryError>> + Send
    {
        let store = self.clone();
        async move {
            let found = store
                .share_snapshots()
                .find_one(doc! {
                    "share_id": binary(&share_id),
                    "revision": signed(revision),
                })
                .await
                .map_err(unavailable)?;
            Ok(found.and_then(ShareSnapshotDocument::into_record))
        }
    }

    fn grant_share_access(
        &self,
        membership: ShareMembershipRecord,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let store = self.clone();
        async move {
            store
                .share_memberships()
                .replace_one(
                    doc! {
                        "share_id": binary(&membership.share_id),
                        "user_id": binary(&membership.user_id),
                    },
                    ShareMembershipDocument::from_record(&membership),
                )
                .upsert(true)
                .await
                .map_err(unavailable)?;
            Ok(())
        }
    }

    fn revoke_share_access(
        &self,
        share_id: ShareId,
        user_id: UserId,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let store = self.clone();
        async move {
            // Deleting nothing is success: the edge is gone either way, which
            // is what the caller asked for.
            store
                .share_memberships()
                .delete_one(doc! {
                    "share_id": binary(&share_id),
                    "user_id": binary(&user_id),
                })
                .await
                .map_err(unavailable)?;
            Ok(())
        }
    }

    fn has_share_access(
        &self,
        share_id: ShareId,
        user_id: UserId,
    ) -> impl std::future::Future<Output = Result<bool, RepositoryError>> + Send {
        let store = self.clone();
        async move {
            if store
                .shares()
                .find_one(doc! { "_id": binary(&share_id), "owner_user_id": binary(&user_id) })
                .await
                .map_err(unavailable)?
                .is_some()
            {
                return Ok(true);
            }
            Ok(store
                .share_memberships()
                .find_one(doc! { "share_id": binary(&share_id), "user_id": binary(&user_id) })
                .await
                .map_err(unavailable)?
                .is_some())
        }
    }

    fn list_authorized_shares(
        &self,
        user_id: UserId,
    ) -> impl std::future::Future<Output = Result<Vec<ShareRecord>, RepositoryError>> + Send {
        let store = self.clone();
        async move {
            let memberships: Vec<_> = store
                .share_memberships()
                .find(doc! { "user_id": binary(&user_id) })
                .await
                .map_err(unavailable)?
                .try_collect()
                .await
                .map_err(unavailable)?;
            let share_ids: Vec<_> = memberships
                .into_iter()
                .filter_map(ShareMembershipDocument::into_record)
                .map(|membership| binary(&membership.share_id))
                .collect();
            let filter = doc! {
                "$or": [
                    { "owner_user_id": binary(&user_id) },
                    { "_id": { "$in": share_ids } },
                ]
            };
            let found: Vec<_> = store
                .shares()
                .find(filter)
                .sort(doc! { "_id": 1 })
                .limit(
                    i64::try_from(portalis_nexus_protocol::MAX_SHARES_PER_RESPONSE)
                        .map_err(|error| RepositoryError::Unavailable(error.to_string()))?,
                )
                .await
                .map_err(unavailable)?
                .try_collect()
                .await
                .map_err(unavailable)?;
            Ok(found
                .into_iter()
                .filter_map(ShareDocument::into_record)
                .collect())
        }
    }

    fn list_share_members(
        &self,
        share_id: ShareId,
    ) -> impl std::future::Future<Output = Result<Vec<UserId>, RepositoryError>> + Send {
        let store = self.clone();
        async move {
            let mut members: Vec<_> = store
                .share_memberships()
                .find(doc! { "share_id": binary(&share_id) })
                .await
                .map_err(unavailable)?
                .try_collect()
                .await
                .map_err(unavailable)?;
            let mut users: Vec<_> = members
                .drain(..)
                .filter_map(ShareMembershipDocument::into_record)
                .map(|membership| membership.user_id)
                .collect();
            if let Some(owner) = store.find_share(share_id).await?.map(|share| share.owner) {
                users.push(owner);
            }
            users.sort_unstable();
            users.dedup();
            Ok(users)
        }
    }
}

/// Names one edge, whichever side asked.
fn edge_filter(edge: FriendshipEdge) -> Document {
    doc! {
        "user_low": binary(&edge.user_low()),
        "user_high": binary(&edge.user_high()),
    }
}
