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
    DeviceId, DeviceRecord, FriendRepository, FriendshipEdge, FriendshipRecord, IdentityRepository,
    RepositoryError, UserDirectory, UserId, UserRecord,
};
use tracing::debug;

mod documents;

use documents::{DeviceDocument, FriendshipDocument, UserDocument, binary, millis};

/// The duplicate-key code every unique index reports.
const DUPLICATE_KEY: i32 = 11_000;

const USERS: &str = "users";
const DEVICES: &str = "devices";
const FRIENDSHIPS: &str = "friendships";

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
                    .keys(doc! { "user_id": 1, "revoked_at_unix_ms": 1 })
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
        at_unix_ms: u64,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let store = self.clone();
        async move {
            store
                .devices()
                .update_one(
                    doc! { "_id": binary(&device_id) },
                    doc! { "$set": { "last_authenticated_at_unix_ms": millis(at_unix_ms) } },
                )
                .await
                .map_err(unavailable)?;
            Ok(())
        }
    }

    fn revoke_device(
        &self,
        device_id: DeviceId,
        at_unix_ms: u64,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        let store = self.clone();
        async move {
            store
                .devices()
                .update_one(
                    doc! { "_id": binary(&device_id) },
                    doc! { "$set": { "revoked_at_unix_ms": millis(at_unix_ms) } },
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
            filter.insert("version", millis(expected_version));
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

/// Names one edge, whichever side asked.
fn edge_filter(edge: FriendshipEdge) -> Document {
    doc! {
        "user_low": binary(&edge.user_low()),
        "user_high": binary(&edge.user_high()),
    }
}
