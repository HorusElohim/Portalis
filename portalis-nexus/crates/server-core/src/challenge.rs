//! The one challenge a connection may sign, and the rules for spending it.
//!
//! A challenge lives on its connection rather than in a shared replay cache.
//! The server issues exactly one per `ServerHello`, so "used once" is a fact
//! about that connection, not a lookup that has to be coordinated between
//! server processes.

use portalis_nexus_protocol::{CHALLENGE_BYTES, CHALLENGE_LIFETIME_MS, CONNECTION_ID_BYTES};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ChallengeError {
    #[error("this challenge was already used")]
    AlreadyUsed,
    #[error("this challenge expired {age_ms}ms after it was issued")]
    Expired { age_ms: u64 },
    #[error("this challenge is dated in the future")]
    NotYetIssued,
    #[error("the signed challenge does not match the one issued")]
    Mismatch,
}

/// The challenge issued to one connection, and whether it has been spent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssuedChallenge {
    connection_id: [u8; CONNECTION_ID_BYTES],
    challenge: [u8; CHALLENGE_BYTES],
    issued_at_unix_ms: u64,
    consumed: bool,
}

impl IssuedChallenge {
    #[must_use]
    pub fn new(
        connection_id: [u8; CONNECTION_ID_BYTES],
        challenge: [u8; CHALLENGE_BYTES],
        issued_at_unix_ms: u64,
    ) -> Self {
        Self {
            connection_id,
            challenge,
            issued_at_unix_ms,
            consumed: false,
        }
    }

    #[must_use]
    pub fn connection_id(&self) -> &[u8; CONNECTION_ID_BYTES] {
        &self.connection_id
    }

    #[must_use]
    pub fn challenge(&self) -> &[u8; CHALLENGE_BYTES] {
        &self.challenge
    }

    #[must_use]
    pub fn issued_at_unix_ms(&self) -> u64 {
        self.issued_at_unix_ms
    }

    #[must_use]
    pub fn is_consumed(&self) -> bool {
        self.consumed
    }

    /// Spends the challenge for one signed request.
    ///
    /// Consuming on success *and* on mismatch is deliberate: a wrong guess
    /// still costs the attempt, so a connection cannot probe repeatedly.
    ///
    /// # Errors
    ///
    /// Returns [`ChallengeError`] when the challenge was already spent, has
    /// expired, is dated in the future, or does not match what was signed.
    pub fn consume(&mut self, signed: &[u8], now_unix_ms: u64) -> Result<(), ChallengeError> {
        if self.consumed {
            return Err(ChallengeError::AlreadyUsed);
        }
        let Some(age_ms) = now_unix_ms.checked_sub(self.issued_at_unix_ms) else {
            return Err(ChallengeError::NotYetIssued);
        };
        if age_ms > CHALLENGE_LIFETIME_MS {
            return Err(ChallengeError::Expired { age_ms });
        }
        self.consumed = true;
        if signed != self.challenge {
            return Err(ChallengeError::Mismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISSUED_AT: u64 = 1_700_000_000_000;

    fn issued() -> IssuedChallenge {
        IssuedChallenge::new([1; CONNECTION_ID_BYTES], [2; CHALLENGE_BYTES], ISSUED_AT)
    }

    #[test]
    fn exposes_what_it_was_issued_with() {
        let challenge = issued();

        assert_eq!(challenge.connection_id(), &[1; CONNECTION_ID_BYTES]);
        assert_eq!(challenge.challenge(), &[2; CHALLENGE_BYTES]);
        assert_eq!(challenge.issued_at_unix_ms(), ISSUED_AT);
        assert!(!challenge.is_consumed());
    }

    #[test]
    fn accepts_the_matching_challenge_once() {
        let mut challenge = issued();

        assert_eq!(
            challenge.consume(&[2; CHALLENGE_BYTES], ISSUED_AT + 1),
            Ok(())
        );

        assert!(challenge.is_consumed());
        assert_eq!(
            challenge.consume(&[2; CHALLENGE_BYTES], ISSUED_AT + 2),
            Err(ChallengeError::AlreadyUsed),
            "a replayed signature must not be accepted twice"
        );
    }

    #[test]
    fn a_wrong_guess_still_spends_the_attempt() {
        let mut challenge = issued();

        assert_eq!(
            challenge.consume(&[9; CHALLENGE_BYTES], ISSUED_AT),
            Err(ChallengeError::Mismatch)
        );
        assert_eq!(
            challenge.consume(&[2; CHALLENGE_BYTES], ISSUED_AT),
            Err(ChallengeError::AlreadyUsed),
            "a connection must not be able to keep guessing"
        );
    }

    #[test]
    fn expires_after_its_lifetime() {
        let mut challenge = issued();
        assert_eq!(
            challenge
                .clone()
                .consume(&[2; CHALLENGE_BYTES], ISSUED_AT + CHALLENGE_LIFETIME_MS),
            Ok(()),
            "the boundary itself is still valid"
        );

        assert_eq!(
            challenge.consume(&[2; CHALLENGE_BYTES], ISSUED_AT + CHALLENGE_LIFETIME_MS + 1),
            Err(ChallengeError::Expired {
                age_ms: CHALLENGE_LIFETIME_MS + 1
            })
        );
        assert!(
            !challenge.is_consumed(),
            "an expired challenge is rejected before it is spent"
        );
    }

    #[test]
    fn rejects_a_clock_that_runs_backwards() {
        let mut challenge = issued();

        assert_eq!(
            challenge.consume(&[2; CHALLENGE_BYTES], ISSUED_AT - 1),
            Err(ChallengeError::NotYetIssued)
        );
    }
}
