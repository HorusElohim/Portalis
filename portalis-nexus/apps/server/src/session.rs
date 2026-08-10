//! Per-connection authentication state.
//!
//! A connection starts unauthenticated, holding the one challenge it was
//! greeted with. Spending that challenge is what lets a command bind the
//! connection to an identity. Deciding *which* command runs is the dispatch
//! layer's job, not this type's.

use portalis_nexus_protocol::v1::ServerHello;
use portalis_nexus_protocol::{CURRENT_PROTOCOL_VERSION, SessionBinding};
use portalis_nexus_server_core::{ChallengeError, Identity, IssuedChallenge};

/// One connection's view of who it is.
#[derive(Debug)]
pub struct Session {
    challenge: IssuedChallenge,
    identity: Option<Identity>,
}

impl Session {
    /// Starts a session from the hello the connection was greeted with.
    ///
    /// The challenge's issue time is read from the hello rather than the clock,
    /// because the client signs the timestamp the hello carried. Taking a
    /// second clock reading here would make every signature on this connection
    /// fail whenever the two readings differed at all.
    ///
    /// # Panics
    ///
    /// Panics when the hello was not built by this server, which would mean
    /// its fixed-size fields are the wrong length.
    #[must_use]
    pub fn new(hello: &ServerHello) -> Self {
        let connection_id = hello
            .connection_id
            .as_slice()
            .try_into()
            .expect("a server-built hello has a fixed-size connection id");
        let challenge = hello
            .challenge
            .as_slice()
            .try_into()
            .expect("a server-built hello has a fixed-size challenge");
        Self {
            challenge: IssuedChallenge::new(connection_id, challenge, hello.server_time_unix_ns),
            identity: None,
        }
    }

    /// The identifier this connection was greeted with.
    #[must_use]
    pub fn connection_id(&self) -> [u8; portalis_nexus_protocol::CONNECTION_ID_BYTES] {
        *self.challenge.connection_id()
    }

    #[must_use]
    pub fn identity(&self) -> Option<&Identity> {
        self.identity.as_ref()
    }

    #[must_use]
    pub fn is_authenticated(&self) -> bool {
        self.identity.is_some()
    }

    /// Binds this connection to a verified identity.
    pub fn bind(&mut self, identity: Identity) {
        self.identity = Some(identity);
    }

    /// Spends the connection's challenge for one signed attempt.
    ///
    /// The signature stands in for the challenge itself: the client never
    /// echoes the challenge back, it signs a payload built from it, so the
    /// bytes checked here are the ones the connection was issued.
    ///
    /// # Errors
    ///
    /// Returns [`ChallengeError`] when the challenge was already spent, has
    /// expired, or the request carried no signature at all.
    pub fn spend(&mut self, signature: &[u8], now_unix_ns: u64) -> Result<(), ChallengeError> {
        if signature.is_empty() {
            return Err(ChallengeError::Mismatch);
        }
        let issued = *self.challenge.challenge();
        self.challenge.consume(&issued, now_unix_ns)
    }

    /// The facts a signature on this connection is bound to.
    #[must_use]
    pub fn binding<'a>(&'a self, server_authority: &'a str) -> SessionBinding<'a> {
        SessionBinding {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            server_authority,
            connection_id: self.challenge.connection_id(),
            challenge: self.challenge.challenge(),
            server_time_unix_ns: self.challenge.issued_at_unix_ns(),
        }
    }

    #[cfg(test)]
    pub(crate) fn challenge_issued_at(&self) -> u64 {
        self.challenge.issued_at_unix_ns()
    }
}

#[cfg(test)]
mod tests {
    use portalis_nexus_protocol::CHALLENGE_LIFETIME_NS;
    use portalis_nexus_protocol::v1::ProtocolRange;
    use portalis_nexus_server_core::ProtocolPolicy;

    use super::*;
    use crate::messages::hello_payload;

    const NOW: u64 = 1_700_000_000_000_000_000;

    fn greeting() -> ServerHello {
        let policy = ProtocolPolicy::new(CURRENT_PROTOCOL_VERSION, CURRENT_PROTOCOL_VERSION)
            .expect("valid protocol range");
        hello_payload(&policy, NOW)
    }

    #[test]
    fn the_challenge_is_issued_at_the_time_the_hello_carries() {
        let hello = greeting();

        let session = Session::new(&hello);

        // The client signs the hello's timestamp, so the session must verify
        // against that exact value rather than a second clock reading.
        assert_eq!(session.challenge_issued_at(), hello.server_time_unix_ns);
        assert!(!session.is_authenticated());
        assert!(session.identity().is_none());
    }

    #[test]
    fn a_signature_binds_to_this_connection_and_challenge() {
        let hello = greeting();
        let session = Session::new(&hello);

        let binding = session.binding("nexus.portalis.test");

        assert_eq!(binding.connection_id, hello.connection_id.as_slice());
        assert_eq!(binding.challenge, hello.challenge.as_slice());
        assert_eq!(binding.server_time_unix_ns, NOW);
        assert_eq!(binding.protocol_version, CURRENT_PROTOCOL_VERSION);
        assert_eq!(binding.server_authority, "nexus.portalis.test");
        assert_eq!(
            session.binding("nexus.portalis.test"),
            SessionBinding {
                protocol_version: CURRENT_PROTOCOL_VERSION,
                server_authority: "nexus.portalis.test",
                connection_id: &hello.connection_id,
                challenge: &hello.challenge,
                server_time_unix_ns: NOW,
            }
        );
        let _ = ProtocolRange::default();
    }

    #[test]
    fn a_challenge_is_spent_once_and_expires() {
        let hello = greeting();
        let mut session = Session::new(&hello);

        assert_eq!(session.spend(&[1], NOW), Ok(()));
        assert_eq!(session.spend(&[1], NOW), Err(ChallengeError::AlreadyUsed));

        let mut fresh = Session::new(&hello);
        assert_eq!(
            fresh.spend(&[1], NOW + CHALLENGE_LIFETIME_NS + 1),
            Err(ChallengeError::Expired {
                age_ns: CHALLENGE_LIFETIME_NS + 1
            })
        );

        // An unsigned request must not cost the connection its challenge.
        let mut unsigned = Session::new(&hello);
        assert_eq!(unsigned.spend(&[], NOW), Err(ChallengeError::Mismatch));
        assert_eq!(unsigned.spend(&[1], NOW), Ok(()));
    }
}
