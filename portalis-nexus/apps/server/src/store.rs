//! The store this server reads and writes.
//!
//! One enum rather than a generic parameter: every layer above the ports is
//! concrete, and swapping the backend should not change a handler's type. Each
//! method dispatches once and awaits the arm it chose.

use portalis_nexus_server_core::{
    DeviceId, DeviceRecord, EnvelopeRepository, FriendRepository, FriendshipEdge, FriendshipRecord,
    IdentityRepository, InMemoryIdentities, KeyEnvelopePage, KeyEnvelopeRecord, RepositoryError,
    ShareId, UserDirectory, UserId, UserRecord,
};

use crate::mongo::MongoStore;

/// Where durable identity and friend state lives.
#[derive(Debug)]
pub enum NexusStore {
    /// Held in memory and lost on restart. The default for local runs, the
    /// demo, and tests.
    Memory(InMemoryIdentities),
    /// Durable, indexed, and transactional.
    Mongo(Box<MongoStore>),
}

impl Default for NexusStore {
    fn default() -> Self {
        Self::Memory(InMemoryIdentities::default())
    }
}

impl NexusStore {
    #[must_use]
    pub fn mongo(store: MongoStore) -> Self {
        Self::Mongo(Box::new(store))
    }

    /// Makes an in-memory store fail, for exercising degraded paths. Has no
    /// effect on a durable store, which fails for real reasons.
    pub fn set_unavailable(&self, unavailable: bool) {
        if let Self::Memory(memory) = self {
            memory.set_unavailable(unavailable);
        }
    }

    /// Names the backend, for logs and readiness reporting.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Memory(_) => "memory",
            Self::Mongo(_) => "mongodb",
        }
    }
}

impl UserDirectory for NexusStore {
    async fn find_user(&self, user_id: UserId) -> Result<Option<UserRecord>, RepositoryError> {
        match self {
            Self::Memory(store) => store.find_user(user_id).await,
            Self::Mongo(store) => store.find_user(user_id).await,
        }
    }

    async fn find_user_by_handle(
        &self,
        normalized_username: &str,
        discriminator: &str,
    ) -> Result<Option<UserRecord>, RepositoryError> {
        match self {
            Self::Memory(store) => {
                store
                    .find_user_by_handle(normalized_username, discriminator)
                    .await
            }
            Self::Mongo(store) => {
                store
                    .find_user_by_handle(normalized_username, discriminator)
                    .await
            }
        }
    }
}

impl IdentityRepository for NexusStore {
    async fn insert_registration(
        &self,
        user: UserRecord,
        device: DeviceRecord,
    ) -> Result<(), RepositoryError> {
        match self {
            Self::Memory(store) => store.insert_registration(user, device).await,
            Self::Mongo(store) => store.insert_registration(user, device).await,
        }
    }

    async fn find_device(
        &self,
        device_id: DeviceId,
    ) -> Result<Option<DeviceRecord>, RepositoryError> {
        match self {
            Self::Memory(store) => store.find_device(device_id).await,
            Self::Mongo(store) => store.find_device(device_id).await,
        }
    }

    async fn link_device(&self, device: DeviceRecord) -> Result<(), RepositoryError> {
        match self {
            Self::Memory(store) => store.link_device(device).await,
            Self::Mongo(store) => store.link_device(device).await,
        }
    }

    async fn touch_device(
        &self,
        device_id: DeviceId,
        at_unix_ns: u64,
    ) -> Result<(), RepositoryError> {
        match self {
            Self::Memory(store) => store.touch_device(device_id, at_unix_ns).await,
            Self::Mongo(store) => store.touch_device(device_id, at_unix_ns).await,
        }
    }

    async fn revoke_device(
        &self,
        device_id: DeviceId,
        at_unix_ns: u64,
    ) -> Result<(), RepositoryError> {
        match self {
            Self::Memory(store) => store.revoke_device(device_id, at_unix_ns).await,
            Self::Mongo(store) => store.revoke_device(device_id, at_unix_ns).await,
        }
    }
}

impl FriendRepository for NexusStore {
    async fn find_friendship(
        &self,
        edge: FriendshipEdge,
    ) -> Result<Option<FriendshipRecord>, RepositoryError> {
        match self {
            Self::Memory(store) => store.find_friendship(edge).await,
            Self::Mongo(store) => store.find_friendship(edge).await,
        }
    }

    async fn save_friendship(
        &self,
        record: FriendshipRecord,
        expected_version: u64,
    ) -> Result<(), RepositoryError> {
        match self {
            Self::Memory(store) => store.save_friendship(record, expected_version).await,
            Self::Mongo(store) => store.save_friendship(record, expected_version).await,
        }
    }

    async fn list_friendships(
        &self,
        user: UserId,
    ) -> Result<Vec<FriendshipRecord>, RepositoryError> {
        match self {
            Self::Memory(store) => store.list_friendships(user).await,
            Self::Mongo(store) => store.list_friendships(user).await,
        }
    }
}

impl EnvelopeRepository for NexusStore {
    async fn put_key_envelope(&self, envelope: KeyEnvelopeRecord) -> Result<(), RepositoryError> {
        match self {
            Self::Memory(store) => store.put_key_envelope(envelope).await,
            Self::Mongo(store) => store.put_key_envelope(envelope).await,
        }
    }

    async fn list_key_envelopes(
        &self,
        recipient_device_id: DeviceId,
        after_share_id: Option<ShareId>,
    ) -> Result<KeyEnvelopePage, RepositoryError> {
        match self {
            Self::Memory(store) => {
                store
                    .list_key_envelopes(recipient_device_id, after_share_id)
                    .await
            }
            Self::Mongo(store) => {
                store
                    .list_key_envelopes(recipient_device_id, after_share_id)
                    .await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADA: UserId = [1; 16];
    const GRACE: UserId = [2; 16];

    fn user() -> UserRecord {
        UserRecord {
            user_id: ADA,
            username: "Ada".to_owned(),
            normalized_username: "ada".to_owned(),
            discriminator: "7Q2XZ".to_owned(),
            created_at_unix_ns: 0,
        }
    }

    fn device() -> DeviceRecord {
        DeviceRecord {
            device_id: [1; 32],
            user_id: ADA,
            public_key: [1; 32],
            encryption_public_key: [2; 32],
            created_at_unix_ns: 0,
            last_authenticated_at_unix_ns: None,
            revoked_at_unix_ns: None,
        }
    }

    fn key_envelope() -> KeyEnvelopeRecord {
        KeyEnvelopeRecord {
            share_id: [3; 16],
            recipient_device_id: [1; 32],
            ephemeral_public_key: [4; 32],
            ciphertext: b"sealed".to_vec(),
            created_at_unix_ns: 0,
        }
    }

    fn unavailable<T: std::fmt::Debug>(outcome: &Result<T, RepositoryError>) -> bool {
        matches!(outcome, Err(RepositoryError::Unavailable(_)))
    }

    /// Async because the driver's client spawns onto the Tokio runtime as it
    /// is built, even though nothing here contacts a server.
    #[tokio::test]
    async fn each_backend_names_itself() {
        assert_eq!(NexusStore::default().kind(), "memory");
        assert_eq!(
            NexusStore::mongo(MongoStore::disconnected()).kind(),
            "mongodb"
        );
    }

    #[tokio::test]
    async fn a_forced_fault_reaches_the_in_memory_backend_only() {
        let memory = NexusStore::default();
        memory.set_unavailable(true);
        assert!(unavailable(&memory.find_user(ADA).await));

        memory.set_unavailable(false);
        assert!(
            !unavailable(&memory.find_user(ADA).await),
            "clearing the fault puts the store back in service"
        );

        // The durable backend has no fault to inject; asking is a no-op
        // rather than an error.
        NexusStore::mongo(MongoStore::disconnected()).set_unavailable(true);
    }

    /// Every method must reach the durable backend and hand its outage back
    /// unchanged. A missing arm would show up here as a success, or as an
    /// answer from the wrong store.
    #[tokio::test]
    async fn every_method_dispatches_to_the_durable_backend() {
        let store = NexusStore::mongo(MongoStore::disconnected());
        let edge = FriendshipEdge::between(ADA, GRACE).expect("distinct users");

        assert!(unavailable(&store.find_user(ADA).await));
        assert!(unavailable(
            &store.find_user_by_handle("ada", "7Q2XZ").await
        ));
        assert!(unavailable(
            &store.insert_registration(user(), device()).await
        ));
        assert!(unavailable(&store.find_device([1; 32]).await));
        assert!(unavailable(&store.link_device(device()).await));
        assert!(unavailable(&store.touch_device([1; 32], 1).await));
        assert!(unavailable(&store.revoke_device([1; 32], 1).await));
        assert!(unavailable(&store.find_friendship(edge).await));
        assert!(unavailable(
            &store
                .save_friendship(FriendshipRecord::requested(edge, ADA, 0), 0)
                .await
        ));
        assert!(unavailable(&store.list_friendships(ADA).await));
        assert!(unavailable(&store.put_key_envelope(key_envelope()).await));
        assert!(unavailable(&store.list_key_envelopes([1; 32], None).await));
    }
}
