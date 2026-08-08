use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{Envelope, Ping};
use portalis_nexus_protocol::{CURRENT_PROTOCOL_VERSION, new_message_id};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientProtocol {
    version: u32,
}

impl Default for ClientProtocol {
    fn default() -> Self {
        Self {
            version: CURRENT_PROTOCOL_VERSION,
        }
    }
}

impl ClientProtocol {
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub fn ping(&self, nonce: u64, sent_at_unix_ms: u64) -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms,
            payload: Some(Payload::Ping(Ping { nonce })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_valid_ping_for_current_protocol() {
        let client = ClientProtocol::default();
        let envelope = client.ping(42, 1000);

        assert_eq!(client.version(), CURRENT_PROTOCOL_VERSION);
        assert_eq!(envelope.sent_at_unix_ms, 1000);
        assert_eq!(envelope.validate(), Ok(()));
        assert_eq!(envelope.payload, Some(Payload::Ping(Ping { nonce: 42 })));
    }
}
