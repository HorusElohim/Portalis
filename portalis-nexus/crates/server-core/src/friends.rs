//! Handle resolution, friend commands, and friend listing.

use portalis_nexus_protocol::v1::{FriendAction, FriendshipState};
use thiserror::Error;

use crate::friendship::{
    FriendshipEdge, FriendshipError, FriendshipRecord, Transition, apply as apply_action,
};
use crate::handle::{Handle, HandleError};
use crate::ports::{Clock, FriendRepository, RepositoryError, UserDirectory, UserId, UserRecord};

/// How many times a command re-reads and re-applies after losing a race.
///
/// Each retry means another side changed the edge first. A handful is enough
/// for two people acting at once; beyond that something is wrong.
pub const COMMAND_ATTEMPTS: usize = 3;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum FriendError {
    #[error(transparent)]
    Friendship(#[from] FriendshipError),
    #[error(transparent)]
    Handle(#[from] HandleError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("no user has that handle")]
    UnknownUser,
    #[error("the friendship kept changing while the command was applied")]
    Contended,
}

/// One side of a friendship, as the asking user sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FriendSummary {
    pub peer: UserRecord,
    pub state: FriendshipState,
    /// Whether the asking user sent the request, which decides who may answer.
    pub requested_by_me: bool,
}

/// Applies the friend rules over injected storage and time.
pub struct FriendService<S, C> {
    store: S,
    clock: C,
}

impl<S, C> FriendService<S, C>
where
    S: FriendRepository + UserDirectory,
    C: Clock,
{
    pub const fn new(store: S, clock: C) -> Self {
        Self { store, clock }
    }

    /// Finds the user behind a handle a person typed.
    ///
    /// # Errors
    ///
    /// Returns [`FriendError`] when the handle is malformed, no user holds it,
    /// or storage fails.
    pub async fn resolve_handle(&self, handle: &str) -> Result<UserRecord, FriendError> {
        let parsed = Handle::parse(handle)?;
        self.store
            .find_user_by_handle(parsed.normalized_username(), parsed.discriminator())
            .await?
            .ok_or(FriendError::UnknownUser)
    }

    /// Applies one friend action, retrying if another side wrote first.
    ///
    /// # Errors
    ///
    /// Returns [`FriendError`] when the action is not allowed from the current
    /// state, the peer is unknown, storage fails, or the edge kept changing.
    ///
    /// # Panics
    ///
    /// Panics if the state machine reports no change with no friendship
    /// stored, which it never does: every unchanged outcome names a state
    /// only an existing edge can hold.
    pub async fn command(
        &self,
        actor: UserId,
        peer: UserId,
        action: FriendAction,
    ) -> Result<FriendshipRecord, FriendError> {
        let edge = FriendshipEdge::between(actor, peer)?;
        if self.store.find_user(peer).await?.is_none() {
            return Err(FriendError::UnknownUser);
        }

        for _ in 0..COMMAND_ATTEMPTS {
            let current = self.store.find_friendship(edge).await?;
            let Transition::Move {
                state,
                requested_by,
                expected_version,
            } = apply_action(current.as_ref(), actor, action)?
            else {
                // Already where the action would take it. Repeating a command
                // must not bump the version or fail. Only an existing
                // friendship can report no change.
                return Ok(current.expect("an unchanged transition implies a friendship"));
            };

            let now = self.clock.now_unix_ms();
            let updated = FriendshipRecord {
                edge,
                requested_by,
                state,
                version: expected_version + 1,
                created_at_unix_ms: current
                    .as_ref()
                    .map_or(now, |record| record.created_at_unix_ms),
                updated_at_unix_ms: now,
            };
            match self
                .store
                .save_friendship(updated.clone(), expected_version)
                .await
            {
                Ok(()) => return Ok(updated),
                Err(RepositoryError::VersionConflict) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(FriendError::Contended)
    }

    /// Lists every friendship joining `user`, with the peer behind each.
    ///
    /// # Errors
    ///
    /// Returns [`FriendError`] when storage fails.
    pub async fn list(&self, user: UserId) -> Result<Vec<FriendSummary>, FriendError> {
        let friendships = self.store.list_friendships(user).await?;
        let mut summaries = Vec::with_capacity(friendships.len());
        for friendship in friendships {
            // A friendship cannot outlive its users, so a missing peer means
            // the edge is stale; skip it rather than failing the whole list.
            if let Some(peer) = self.store.find_user(friendship.edge.peer_of(user)).await? {
                summaries.push(FriendSummary {
                    peer,
                    state: friendship.state,
                    requested_by_me: friendship.requested_by == user,
                });
            }
        }
        Ok(summaries)
    }

    /// Whether these two users may see each other's presence.
    ///
    /// # Errors
    ///
    /// Returns [`FriendError`] when storage fails.
    pub async fn are_friends(&self, one: UserId, other: UserId) -> Result<bool, FriendError> {
        let Ok(edge) = FriendshipEdge::between(one, other) else {
            // A user is always allowed to see their own presence.
            return Ok(true);
        };
        Ok(self
            .store
            .find_friendship(edge)
            .await?
            .is_some_and(|record| record.is_accepted()))
    }
}

#[cfg(test)]
mod tests {
    use crate::memory::{FixedClock, InMemoryIdentities};

    use super::*;

    const NOW: u64 = 1_700_000_000_000;

    /// One store type across every test, since `FriendService` is generic and
    /// each instantiation is measured as its own set of regions.
    type TestService = FriendService<TestStore, FixedClock>;

    /// Which read or write should fail, so degraded paths are exercised.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    enum Fault {
        #[default]
        None,
        FindUser,
        FindHandle,
        FindFriendship,
        List,
        /// A write fails outright, as opposed to losing a race.
        Save,
        /// Every write loses its race, as if another side always wrote first.
        AlwaysContended,
    }

    #[derive(Default)]
    struct TestStore {
        inner: InMemoryIdentities,
        fault: Fault,
    }

    impl TestStore {
        fn hits(&self, operation: Fault) -> Option<RepositoryError> {
            (self.fault == operation)
                .then(|| RepositoryError::Unavailable(format!("{operation:?}")))
        }
    }

    impl UserDirectory for TestStore {
        fn find_user(
            &self,
            user_id: UserId,
        ) -> impl std::future::Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send
        {
            let failure = self.hits(Fault::FindUser);
            let inner = self.inner.find_user(user_id);
            async move {
                match failure {
                    Some(error) => Err(error),
                    None => inner.await,
                }
            }
        }

        fn find_user_by_handle(
            &self,
            normalized_username: &str,
            discriminator: &str,
        ) -> impl std::future::Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send
        {
            let failure = self.hits(Fault::FindHandle);
            let inner = self
                .inner
                .find_user_by_handle(normalized_username, discriminator);
            async move {
                match failure {
                    Some(error) => Err(error),
                    None => inner.await,
                }
            }
        }
    }

    impl FriendRepository for TestStore {
        fn find_friendship(
            &self,
            edge: FriendshipEdge,
        ) -> impl std::future::Future<Output = Result<Option<FriendshipRecord>, RepositoryError>> + Send
        {
            let failure = self.hits(Fault::FindFriendship);
            let inner = self.inner.find_friendship(edge);
            async move {
                match failure {
                    Some(error) => Err(error),
                    None => inner.await,
                }
            }
        }

        fn save_friendship(
            &self,
            record: FriendshipRecord,
            expected_version: u64,
        ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
            // The write must not happen at all when it is meant to lose its
            // race: performing it and then reporting a conflict would let the
            // next attempt read a record that was never committed.
            let contended = self.fault == Fault::AlwaysContended;
            let broken = self.hits(Fault::Save);
            let inner = (!contended && broken.is_none())
                .then(|| self.inner.save_friendship(record, expected_version));
            async move {
                match (inner, broken) {
                    (Some(write), _) => write.await,
                    (None, Some(error)) => Err(error),
                    (None, None) => Err(RepositoryError::VersionConflict),
                }
            }
        }

        fn list_friendships(
            &self,
            user: UserId,
        ) -> impl std::future::Future<Output = Result<Vec<FriendshipRecord>, RepositoryError>> + Send
        {
            let failure = self.hits(Fault::List);
            let inner = self.inner.list_friendships(user);
            async move {
                match failure {
                    Some(error) => Err(error),
                    None => inner.await,
                }
            }
        }
    }

    fn user(id: u8, username: &str, discriminator: &str) -> UserRecord {
        UserRecord {
            user_id: [id; 16],
            username: username.to_owned(),
            normalized_username: username.to_lowercase(),
            discriminator: discriminator.to_owned(),
            created_at_unix_ms: NOW,
        }
    }

    /// A service holding Ada (1) and Grace (2).
    fn service() -> TestService {
        service_with(Fault::None)
    }

    fn service_with(fault: Fault) -> TestService {
        let store = TestStore {
            fault,
            ..TestStore::default()
        };
        store
            .inner
            .store_user(user(1, "Ada", "7Q2XZ"))
            .expect("Ada");
        store
            .inner
            .store_user(user(2, "Grace", "ABCDE"))
            .expect("Grace");
        FriendService::new(store, FixedClock::new(NOW))
    }

    const ADA: UserId = [1; 16];
    const GRACE: UserId = [2; 16];

    #[tokio::test]
    async fn resolves_a_handle_however_it_was_typed() {
        let service = service();

        let found = service.resolve_handle("ada#7Q2XZ").await.expect("Ada");
        assert_eq!(found.user_id, ADA);
        // Casing is normalized on both halves of the handle.
        assert_eq!(
            service
                .resolve_handle("ADA#7q2xz")
                .await
                .expect("Ada")
                .user_id,
            ADA
        );
    }

    #[tokio::test]
    async fn refuses_handles_that_are_malformed_or_unclaimed() {
        let service = service();

        assert_eq!(
            service.resolve_handle("ada").await,
            Err(FriendError::Handle(HandleError::Malformed))
        );
        assert_eq!(
            service.resolve_handle("nobody#7Q2XZ").await,
            Err(FriendError::UnknownUser)
        );
    }

    #[tokio::test]
    async fn two_users_become_friends() {
        let service = service();

        let requested = service
            .command(ADA, GRACE, FriendAction::Request)
            .await
            .expect("request sent");
        assert_eq!(requested.state, FriendshipState::Pending);
        assert_eq!(requested.requested_by, ADA);
        assert_eq!(requested.version, 1);
        assert!(!service.are_friends(ADA, GRACE).await.expect("checked"));

        let accepted = service
            .command(GRACE, ADA, FriendAction::Accept)
            .await
            .expect("request accepted");

        assert_eq!(accepted.state, FriendshipState::Accepted);
        assert_eq!(accepted.version, 2);
        assert_eq!(accepted.created_at_unix_ms, requested.created_at_unix_ms);
        assert!(service.are_friends(ADA, GRACE).await.expect("checked"));
        assert!(service.are_friends(GRACE, ADA).await.expect("checked"));
    }

    #[tokio::test]
    async fn repeating_a_command_does_not_bump_the_version() {
        let service = service();
        service
            .command(ADA, GRACE, FriendAction::Request)
            .await
            .expect("request sent");

        let repeated = service
            .command(ADA, GRACE, FriendAction::Request)
            .await
            .expect("repeat accepted");

        assert_eq!(repeated.state, FriendshipState::Pending);
        assert_eq!(repeated.version, 1, "a repeat must not count as a change");
    }

    #[tokio::test]
    async fn each_side_sees_who_asked() {
        let service = service();
        service
            .command(ADA, GRACE, FriendAction::Request)
            .await
            .expect("request sent");

        let ada_sees = service.list(ADA).await.expect("Ada's friends");
        let grace_sees = service.list(GRACE).await.expect("Grace's friends");

        assert_eq!(ada_sees.len(), 1);
        assert_eq!(ada_sees[0].peer.user_id, GRACE);
        assert_eq!(ada_sees[0].peer.username, "Grace");
        assert_eq!(ada_sees[0].state, FriendshipState::Pending);
        assert!(ada_sees[0].requested_by_me);

        assert_eq!(grace_sees.len(), 1);
        assert_eq!(grace_sees[0].peer.user_id, ADA);
        assert!(!grace_sees[0].requested_by_me);
    }

    #[tokio::test]
    async fn removing_ends_the_friendship_for_both() {
        let service = service();
        service
            .command(ADA, GRACE, FriendAction::Request)
            .await
            .expect("request sent");
        service
            .command(GRACE, ADA, FriendAction::Accept)
            .await
            .expect("accepted");

        let removed = service
            .command(GRACE, ADA, FriendAction::Remove)
            .await
            .expect("removed");

        assert_eq!(removed.state, FriendshipState::Removed);
        assert!(!service.are_friends(ADA, GRACE).await.expect("checked"));
        assert!(!service.are_friends(GRACE, ADA).await.expect("checked"));
    }

    #[tokio::test]
    async fn a_user_may_always_see_their_own_presence() {
        assert!(service().are_friends(ADA, ADA).await.expect("checked"));
    }

    #[tokio::test]
    async fn commands_need_a_real_and_different_peer() {
        let service = service();

        assert_eq!(
            service.command(ADA, ADA, FriendAction::Request).await,
            Err(FriendError::Friendship(FriendshipError::SelfFriendship))
        );
        assert_eq!(
            service.command(ADA, [9; 16], FriendAction::Request).await,
            Err(FriendError::UnknownUser)
        );
    }

    #[tokio::test]
    async fn only_the_recipient_may_accept() {
        let service = service();
        service
            .command(ADA, GRACE, FriendAction::Request)
            .await
            .expect("request sent");

        assert_eq!(
            service.command(ADA, GRACE, FriendAction::Accept).await,
            Err(FriendError::Friendship(FriendshipError::NotTheRecipient))
        );
    }

    #[tokio::test]
    async fn an_action_that_changes_nothing_before_any_edge_exists_is_refused() {
        let service = service();

        // Remove has no meaning without a friendship, and reports why.
        assert_eq!(
            service.command(ADA, GRACE, FriendAction::Remove).await,
            Err(FriendError::Friendship(FriendshipError::NotPermitted {
                action: "remove",
                state: "not a friendship",
            }))
        );
    }

    #[tokio::test]
    async fn a_stale_peer_is_skipped_rather_than_failing_the_list() {
        let store = TestStore::default();
        store
            .inner
            .store_user(user(1, "Ada", "7Q2XZ"))
            .expect("Ada");
        // An edge whose other side was never stored.
        store
            .inner
            .save_friendship(
                FriendshipRecord::requested(
                    FriendshipEdge::between(ADA, [9; 16]).expect("distinct"),
                    ADA,
                    NOW,
                ),
                0,
            )
            .await
            .expect("edge stored");
        let service = FriendService::new(store, FixedClock::new(NOW));

        assert!(service.list(ADA).await.expect("listed").is_empty());
    }

    #[tokio::test]
    async fn a_command_that_keeps_losing_its_race_gives_up() {
        let service = service_with(Fault::AlwaysContended);

        assert_eq!(
            service.command(ADA, GRACE, FriendAction::Request).await,
            Err(FriendError::Contended),
            "after {COMMAND_ATTEMPTS} attempts the command stops retrying"
        );
    }

    #[tokio::test]
    async fn storage_failures_are_reported_rather_than_hidden() {
        for (fault, act) in [
            (Fault::FindHandle, "find-handle"),
            (Fault::FindUser, "find-user"),
            (Fault::FindFriendship, "find-friendship"),
            (Fault::Save, "save"),
            (Fault::List, "list"),
        ] {
            let service = service_with(fault);
            let unavailable =
                FriendError::Repository(RepositoryError::Unavailable(format!("{fault:?}")));

            let outcome = match fault {
                Fault::FindHandle => service.resolve_handle("ada#7Q2XZ").await.map(|_| ()),
                Fault::List => service.list(ADA).await.map(|_| ()),
                _ => service
                    .command(ADA, GRACE, FriendAction::Request)
                    .await
                    .map(|_| ()),
            };

            assert_eq!(outcome, Err(unavailable), "for {act}");
        }
    }

    #[tokio::test]
    async fn a_failed_friendship_read_is_reported_when_checking_friends() {
        let service = service_with(Fault::FindFriendship);

        assert!(service.are_friends(ADA, GRACE).await.is_err());
    }

    #[tokio::test]
    async fn a_failed_peer_lookup_is_reported_when_listing() {
        // Seeded directly, because reaching this state through commands would
        // need the very lookup that is set to fail.
        let store = TestStore {
            fault: Fault::FindUser,
            ..TestStore::default()
        };
        store
            .inner
            .store_user(user(1, "Ada", "7Q2XZ"))
            .expect("Ada");
        store
            .inner
            .store_user(user(2, "Grace", "ABCDE"))
            .expect("Grace");
        store
            .inner
            .save_friendship(
                FriendshipRecord::requested(
                    FriendshipEdge::between(ADA, GRACE).expect("distinct"),
                    ADA,
                    NOW,
                ),
                0,
            )
            .await
            .expect("edge stored");
        let service = FriendService::new(store, FixedClock::new(NOW));

        assert_eq!(
            service.list(ADA).await,
            Err(FriendError::Repository(RepositoryError::Unavailable(
                "FindUser".to_owned()
            )))
        );
    }
}
