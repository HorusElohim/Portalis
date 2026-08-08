//! The friendship state machine, held on one canonical edge.
//!
//! Two users share a single edge rather than a row each, so the two sides can
//! never disagree about whether they are friends. The pair is stored sorted,
//! which makes the edge's identity independent of who asked first.

use portalis_nexus_protocol::v1::{FriendAction, FriendshipState};
use thiserror::Error;

use crate::ports::UserId;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum FriendshipError {
    #[error("a user cannot befriend themselves")]
    SelfFriendship,
    #[error("no action was given")]
    UnspecifiedAction,
    #[error("cannot {action} a friendship that is {state}")]
    NotPermitted {
        action: &'static str,
        state: &'static str,
    },
    #[error("only the other side may answer a friend request")]
    NotTheRecipient,
}

/// The two users a friendship joins, in a fixed order.
///
/// Sorting means `between(a, b)` and `between(b, a)` name the same edge, so a
/// single unique index over the pair keeps one row per friendship.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FriendshipEdge {
    user_low: UserId,
    user_high: UserId,
}

impl FriendshipEdge {
    /// Names the edge between two users.
    ///
    /// # Errors
    ///
    /// Returns [`FriendshipError::SelfFriendship`] when both sides are the
    /// same user.
    pub fn between(one: UserId, other: UserId) -> Result<Self, FriendshipError> {
        match one.cmp(&other) {
            std::cmp::Ordering::Equal => Err(FriendshipError::SelfFriendship),
            std::cmp::Ordering::Less => Ok(Self {
                user_low: one,
                user_high: other,
            }),
            std::cmp::Ordering::Greater => Ok(Self {
                user_low: other,
                user_high: one,
            }),
        }
    }

    #[must_use]
    pub fn user_low(&self) -> UserId {
        self.user_low
    }

    #[must_use]
    pub fn user_high(&self) -> UserId {
        self.user_high
    }

    /// The other user on this edge.
    #[must_use]
    pub fn peer_of(&self, user: UserId) -> UserId {
        if user == self.user_low {
            self.user_high
        } else {
            self.user_low
        }
    }

    #[must_use]
    pub fn joins(&self, user: UserId) -> bool {
        user == self.user_low || user == self.user_high
    }
}

/// A durable friendship, carrying the version its next write must match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FriendshipRecord {
    pub edge: FriendshipEdge,
    /// Who sent the request that produced the current state, which decides
    /// who is allowed to answer it.
    pub requested_by: UserId,
    pub state: FriendshipState,
    /// Bumped on every accepted transition, so a concurrent write that read an
    /// older value is rejected rather than silently overwriting it.
    pub version: u64,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

impl FriendshipRecord {
    /// Starts a friendship at its first request.
    #[must_use]
    pub fn requested(edge: FriendshipEdge, requested_by: UserId, at_unix_ms: u64) -> Self {
        Self {
            edge,
            requested_by,
            state: FriendshipState::Pending,
            version: 1,
            created_at_unix_ms: at_unix_ms,
            updated_at_unix_ms: at_unix_ms,
        }
    }

    #[must_use]
    pub fn is_accepted(&self) -> bool {
        self.state == FriendshipState::Accepted
    }
}

/// What applying an action to a friendship should do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Transition {
    /// The friendship moves, and the write must match `expected_version`.
    Move {
        state: FriendshipState,
        requested_by: UserId,
        expected_version: u64,
    },
    /// The friendship is already where the action would take it. Repeating a
    /// command must not fail or bump the version.
    Unchanged,
}

/// Decides what `action` by `actor` does to `current`.
///
/// Passing `None` for `current` means the two users have no edge yet.
///
/// # Errors
///
/// Returns [`FriendshipError`] when the action makes no sense for the current
/// state, or when the wrong side tried to answer a request.
pub fn apply(
    current: Option<&FriendshipRecord>,
    actor: UserId,
    action: FriendAction,
) -> Result<Transition, FriendshipError> {
    let state = current.map_or(FriendshipState::None, |record| record.state);
    let version = current.map_or(0, |record| record.version);
    let requester = current.map(|record| record.requested_by);

    match action {
        FriendAction::Unspecified => Err(FriendshipError::UnspecifiedAction),
        FriendAction::Request => Ok(request(state, actor, requester, version)),
        FriendAction::Accept => answer(state, actor, requester, version, FriendshipState::Accepted),
        FriendAction::Reject => answer(state, actor, requester, version, FriendshipState::Rejected),
        FriendAction::Remove => remove(state, actor, requester, version),
    }
}

/// A request may start from nothing, or restart after a refusal or removal.
///
/// Every state has an answer, so this never fails.
fn request(
    state: FriendshipState,
    actor: UserId,
    requester: Option<UserId>,
    version: u64,
) -> Transition {
    match state {
        FriendshipState::Unspecified
        | FriendshipState::None
        | FriendshipState::Rejected
        | FriendshipState::Removed => Transition::Move {
            state: FriendshipState::Pending,
            requested_by: actor,
            expected_version: version,
        },
        // Re-sending your own pending request changes nothing. The other side
        // sending one back is taken as accepting yours.
        FriendshipState::Pending if requester == Some(actor) => Transition::Unchanged,
        FriendshipState::Pending => Transition::Move {
            state: FriendshipState::Accepted,
            requested_by: requester.unwrap_or(actor),
            expected_version: version,
        },
        FriendshipState::Accepted => Transition::Unchanged,
    }
}

/// Only the side that did not ask may accept or reject.
fn answer(
    state: FriendshipState,
    actor: UserId,
    requester: Option<UserId>,
    version: u64,
    outcome: FriendshipState,
) -> Result<Transition, FriendshipError> {
    if state == outcome {
        return Ok(Transition::Unchanged);
    }
    if state != FriendshipState::Pending {
        return Err(FriendshipError::NotPermitted {
            action: name_of_state(outcome),
            state: name_of_state(state),
        });
    }
    if requester == Some(actor) {
        return Err(FriendshipError::NotTheRecipient);
    }
    Ok(Transition::Move {
        state: outcome,
        requested_by: requester.unwrap_or(actor),
        expected_version: version,
    })
}

/// Either side may remove an accepted friendship; the asker may cancel a
/// request they have not had answered.
fn remove(
    state: FriendshipState,
    actor: UserId,
    requester: Option<UserId>,
    version: u64,
) -> Result<Transition, FriendshipError> {
    match state {
        FriendshipState::Removed => Ok(Transition::Unchanged),
        FriendshipState::Accepted | FriendshipState::Pending => Ok(Transition::Move {
            state: FriendshipState::Removed,
            requested_by: requester.unwrap_or(actor),
            expected_version: version,
        }),
        other => Err(FriendshipError::NotPermitted {
            action: "remove",
            state: name_of_state(other),
        }),
    }
}

fn name_of_state(state: FriendshipState) -> &'static str {
    match state {
        FriendshipState::Unspecified => "unspecified",
        FriendshipState::None => "not a friendship",
        FriendshipState::Pending => "pending",
        FriendshipState::Accepted => "accepted",
        FriendshipState::Rejected => "rejected",
        FriendshipState::Removed => "removed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADA: UserId = [1; 16];
    const GRACE: UserId = [2; 16];
    const NOW: u64 = 1_700_000_000_000;

    fn edge() -> FriendshipEdge {
        FriendshipEdge::between(ADA, GRACE).expect("distinct users")
    }

    fn record(state: FriendshipState, requested_by: UserId, version: u64) -> FriendshipRecord {
        FriendshipRecord {
            edge: edge(),
            requested_by,
            state,
            version,
            created_at_unix_ms: NOW,
            updated_at_unix_ms: NOW,
        }
    }

    fn moves_to(state: FriendshipState, requested_by: UserId, version: u64) -> Transition {
        Transition::Move {
            state,
            requested_by,
            expected_version: version,
        }
    }

    #[test]
    fn an_edge_names_the_same_pair_whichever_way_it_is_built() {
        let forward = FriendshipEdge::between(ADA, GRACE).expect("distinct users");
        let backward = FriendshipEdge::between(GRACE, ADA).expect("distinct users");

        assert_eq!(forward, backward);
        assert_eq!(forward.user_low(), ADA);
        assert_eq!(forward.user_high(), GRACE);
        assert_eq!(forward.peer_of(ADA), GRACE);
        assert_eq!(forward.peer_of(GRACE), ADA);
        assert!(forward.joins(ADA) && forward.joins(GRACE));
        assert!(!forward.joins([9; 16]));
    }

    #[test]
    fn nobody_can_befriend_themselves() {
        assert_eq!(
            FriendshipEdge::between(ADA, ADA),
            Err(FriendshipError::SelfFriendship)
        );
    }

    #[test]
    fn a_request_starts_a_pending_friendship() {
        assert_eq!(
            apply(None, ADA, FriendAction::Request),
            Ok(moves_to(FriendshipState::Pending, ADA, 0))
        );

        let pending = FriendshipRecord::requested(edge(), ADA, NOW);
        assert_eq!(pending.state, FriendshipState::Pending);
        assert_eq!(pending.version, 1);
        assert!(!pending.is_accepted());
    }

    #[test]
    fn repeating_your_own_request_changes_nothing() {
        let pending = record(FriendshipState::Pending, ADA, 1);

        assert_eq!(
            apply(Some(&pending), ADA, FriendAction::Request),
            Ok(Transition::Unchanged)
        );
    }

    #[test]
    fn a_request_back_is_taken_as_accepting() {
        let pending = record(FriendshipState::Pending, ADA, 1);

        assert_eq!(
            apply(Some(&pending), GRACE, FriendAction::Request),
            Ok(moves_to(FriendshipState::Accepted, ADA, 1))
        );
    }

    #[test]
    fn only_the_other_side_may_answer_a_request() {
        let pending = record(FriendshipState::Pending, ADA, 1);

        assert_eq!(
            apply(Some(&pending), GRACE, FriendAction::Accept),
            Ok(moves_to(FriendshipState::Accepted, ADA, 1))
        );
        assert_eq!(
            apply(Some(&pending), GRACE, FriendAction::Reject),
            Ok(moves_to(FriendshipState::Rejected, ADA, 1))
        );
        assert_eq!(
            apply(Some(&pending), ADA, FriendAction::Accept),
            Err(FriendshipError::NotTheRecipient)
        );
        assert_eq!(
            apply(Some(&pending), ADA, FriendAction::Reject),
            Err(FriendshipError::NotTheRecipient)
        );
    }

    #[test]
    fn answering_twice_is_idempotent() {
        let accepted = record(FriendshipState::Accepted, ADA, 2);
        let rejected = record(FriendshipState::Rejected, ADA, 2);

        assert_eq!(
            apply(Some(&accepted), GRACE, FriendAction::Accept),
            Ok(Transition::Unchanged)
        );
        assert_eq!(
            apply(Some(&rejected), GRACE, FriendAction::Reject),
            Ok(Transition::Unchanged)
        );
        assert_eq!(
            apply(Some(&accepted), GRACE, FriendAction::Request),
            Ok(Transition::Unchanged)
        );
    }

    #[test]
    fn an_answer_needs_something_to_answer() {
        let accepted = record(FriendshipState::Accepted, ADA, 2);

        assert_eq!(
            apply(Some(&accepted), GRACE, FriendAction::Reject),
            Err(FriendshipError::NotPermitted {
                action: "rejected",
                state: "accepted",
            })
        );
        assert_eq!(
            apply(None, GRACE, FriendAction::Accept),
            Err(FriendshipError::NotPermitted {
                action: "accepted",
                state: "not a friendship",
            })
        );
    }

    #[test]
    fn either_side_may_remove_and_the_asker_may_cancel() {
        let accepted = record(FriendshipState::Accepted, ADA, 2);
        let pending = record(FriendshipState::Pending, ADA, 1);

        assert_eq!(
            apply(Some(&accepted), ADA, FriendAction::Remove),
            Ok(moves_to(FriendshipState::Removed, ADA, 2))
        );
        assert_eq!(
            apply(Some(&accepted), GRACE, FriendAction::Remove),
            Ok(moves_to(FriendshipState::Removed, ADA, 2))
        );
        assert_eq!(
            apply(Some(&pending), ADA, FriendAction::Remove),
            Ok(moves_to(FriendshipState::Removed, ADA, 1))
        );
        assert_eq!(
            apply(
                Some(&record(FriendshipState::Removed, ADA, 3)),
                ADA,
                FriendAction::Remove
            ),
            Ok(Transition::Unchanged)
        );
        assert_eq!(
            apply(None, ADA, FriendAction::Remove),
            Err(FriendshipError::NotPermitted {
                action: "remove",
                state: "not a friendship",
            })
        );
    }

    #[test]
    fn a_refusal_or_removal_can_be_asked_again() {
        for state in [FriendshipState::Rejected, FriendshipState::Removed] {
            assert_eq!(
                apply(Some(&record(state, ADA, 2)), GRACE, FriendAction::Request),
                Ok(moves_to(FriendshipState::Pending, GRACE, 2)),
                "{state:?} should be re-requestable"
            );
        }
    }

    #[test]
    fn an_unset_action_is_refused() {
        assert_eq!(
            apply(None, ADA, FriendAction::Unspecified),
            Err(FriendshipError::UnspecifiedAction)
        );
        // A record left at the zero value behaves as no friendship.
        assert_eq!(
            apply(
                Some(&record(FriendshipState::Unspecified, ADA, 1)),
                ADA,
                FriendAction::Request
            ),
            Ok(moves_to(FriendshipState::Pending, ADA, 1))
        );
    }

    #[test]
    fn accepting_a_removed_friendship_is_refused() {
        let removed = record(FriendshipState::Removed, ADA, 3);

        assert_eq!(
            apply(Some(&removed), GRACE, FriendAction::Accept),
            Err(FriendshipError::NotPermitted {
                action: "accepted",
                state: "removed",
            })
        );
    }

    #[test]
    fn names_every_state_for_its_error_message() {
        for (state, name) in [
            (FriendshipState::Unspecified, "unspecified"),
            (FriendshipState::None, "not a friendship"),
            (FriendshipState::Pending, "pending"),
            (FriendshipState::Accepted, "accepted"),
            (FriendshipState::Rejected, "rejected"),
            (FriendshipState::Removed, "removed"),
        ] {
            assert_eq!(name_of_state(state), name);
        }
    }
}
