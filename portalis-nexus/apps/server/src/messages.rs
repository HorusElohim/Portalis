//! Envelope construction and inbound message dispatch.
//!
//! Every decision a socket makes lives here as a pure function, so the socket
//! module is left with nothing but plumbing.

use axum::extract::ws::Message;
use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{
    Envelope, Ping, Pong, ProtocolError, ProtocolErrorCode, ServerHello,
};
use portalis_nexus_protocol::{decode_frame, encode_frame, new_challenge, new_message_id};
use portalis_nexus_server_core::ProtocolPolicy;

/// What a socket owes its peer after one inbound WebSocket message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SocketReply {
    Send(Message),
    Idle,
    Close,
}

#[must_use]
pub fn server_hello(protocol_policy: &ProtocolPolicy, server_time_unix_ms: u64) -> Envelope {
    hello_envelope(
        hello_payload(protocol_policy, server_time_unix_ms),
        server_time_unix_ms,
    )
}

/// Wraps an already-built hello, so a caller can read its connection ID first.
#[must_use]
pub fn hello_envelope(hello: ServerHello, sent_at_unix_ms: u64) -> Envelope {
    Envelope {
        message_id: new_message_id(),
        correlation_id: Vec::new(),
        sent_at_unix_ms,
        payload: Some(Payload::ServerHello(hello)),
    }
}

#[must_use]
pub fn hello_payload(protocol_policy: &ProtocolPolicy, server_time_unix_ms: u64) -> ServerHello {
    ServerHello {
        connection_id: new_message_id(),
        challenge: new_challenge(),
        server_time_unix_ms,
        supported_protocols: Some(*protocol_policy.supported()),
    }
}

#[must_use]
pub fn response_for(envelope: &Envelope, sent_at_unix_ms: u64) -> Envelope {
    match &envelope.payload {
        Some(Payload::Ping(Ping { nonce })) => Envelope {
            message_id: new_message_id(),
            correlation_id: envelope.message_id.clone(),
            sent_at_unix_ms,
            payload: Some(Payload::Pong(Pong { nonce: *nonce })),
        },
        _ => protocol_error(
            envelope.message_id.clone(),
            "only Ping is accepted before authentication".to_owned(),
            sent_at_unix_ms,
        ),
    }
}

#[must_use]
pub fn protocol_error(correlation_id: Vec<u8>, message: String, sent_at_unix_ms: u64) -> Envelope {
    Envelope {
        message_id: new_message_id(),
        correlation_id,
        sent_at_unix_ms,
        payload: Some(Payload::ProtocolError(ProtocolError {
            code: ProtocolErrorCode::InvalidMessage as i32,
            message,
            retry_after_ms: None,
            retryable: false,
        })),
    }
}

/// Encodes one server-generated envelope as a bounded binary frame.
///
/// # Panics
///
/// Panics when the envelope is invalid or exceeds the frame limit, which would
/// mean the server itself built a message the protocol forbids.
#[must_use]
pub fn binary_frame(envelope: &Envelope) -> Message {
    let frame = encode_frame(envelope).expect("server-generated envelopes are valid and bounded");
    Message::Binary(frame.into())
}

/// Maps one inbound WebSocket message to the reply queued for its peer.
#[must_use]
pub fn reply_to(message: &Message, sent_at_unix_ms: u64) -> SocketReply {
    match message {
        Message::Binary(frame) => {
            let response = match decode_frame(frame) {
                Ok(envelope) => response_for(&envelope, sent_at_unix_ms),
                Err(error) => protocol_error(Vec::new(), error.to_string(), sent_at_unix_ms),
            };
            SocketReply::Send(binary_frame(&response))
        }
        Message::Text(_) => SocketReply::Send(binary_frame(&protocol_error(
            Vec::new(),
            "expected a binary protobuf envelope".to_owned(),
            sent_at_unix_ms,
        ))),
        Message::Ping(payload) => SocketReply::Send(Message::Pong(payload.clone())),
        Message::Pong(_) => SocketReply::Idle,
        Message::Close(_) => SocketReply::Close,
    }
}

#[cfg(test)]
mod tests {
    use portalis_nexus_protocol::CURRENT_PROTOCOL_VERSION;

    use super::*;

    fn policy() -> ProtocolPolicy {
        ProtocolPolicy::new(CURRENT_PROTOCOL_VERSION, CURRENT_PROTOCOL_VERSION)
            .expect("valid protocol range")
    }

    /// Decodes a binary reply's payload, or `None` when the reply is not one.
    fn replied_payload(reply: SocketReply) -> Option<Payload> {
        let SocketReply::Send(Message::Binary(frame)) = reply else {
            return None;
        };
        decode_frame(&frame)
            .expect("server replies are valid frames")
            .payload
    }

    fn rejection(message: &str) -> Payload {
        Payload::ProtocolError(ProtocolError {
            code: ProtocolErrorCode::InvalidMessage as i32,
            message: message.to_owned(),
            retry_after_ms: None,
            retryable: false,
        })
    }

    #[test]
    fn server_hello_is_valid_for_the_current_protocol() {
        let hello = hello_payload(&policy(), 42);

        assert_eq!(hello.server_time_unix_ms, 42);
        assert_eq!(
            portalis_nexus_protocol::validate_server_hello(&hello),
            Ok(())
        );
        assert_eq!(server_hello(&policy(), 42).sent_at_unix_ms, 42);
    }

    #[test]
    fn responds_to_ping_and_rejects_other_messages() {
        let request = Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: 1,
            payload: Some(Payload::Ping(Ping { nonce: 7 })),
        };
        let response = response_for(&request, 2);
        assert_eq!(response.correlation_id, request.message_id);
        assert_eq!(response.sent_at_unix_ms, 2);
        assert_eq!(response.payload, Some(Payload::Pong(Pong { nonce: 7 })));

        let rejected = response_for(&response, 3);
        assert_eq!(rejected.correlation_id, response.message_id);
        assert_eq!(
            rejected.payload,
            Some(rejection("only Ping is accepted before authentication"))
        );
    }

    #[test]
    fn replies_to_pings_and_rejects_non_protobuf_messages() {
        let ping = Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            sent_at_unix_ms: 1,
            payload: Some(Payload::Ping(Ping { nonce: 7 })),
        };
        assert_eq!(
            replied_payload(reply_to(&binary_frame(&ping), 2)),
            Some(Payload::Pong(Pong { nonce: 7 }))
        );
        assert_eq!(
            replied_payload(reply_to(&Message::Binary(vec![0xff].into()), 2)),
            Some(rejection("frame is not a valid protobuf envelope"))
        );
        assert_eq!(
            replied_payload(reply_to(&Message::Text("hello".into()), 2)),
            Some(rejection("expected a binary protobuf envelope"))
        );
    }

    #[test]
    fn answers_websocket_control_frames() {
        assert_eq!(
            reply_to(&Message::Ping(vec![1, 2].into()), 1),
            SocketReply::Send(Message::Pong(vec![1, 2].into()))
        );
        assert_eq!(
            reply_to(&Message::Pong(vec![].into()), 1),
            SocketReply::Idle
        );
        assert_eq!(reply_to(&Message::Close(None), 1), SocketReply::Close);
        assert_eq!(replied_payload(SocketReply::Close), None);
    }
}
