//! Key-envelope push and fetch.

use portalis_nexus_protocol::SHARE_ID_BYTES;
use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{
    Envelope, KeyEnvelope, KeyEnvelopePut, ListKeyEnvelopesRequest, ListKeyEnvelopesResponse,
    ProtocolErrorCode, PutKeyEnvelope,
};
use portalis_nexus_server_core::{
    DeviceId, EnvelopeError, KeyEnvelopeRecord, PutKeyEnvelopeRequest,
};

use crate::identity::{DefaultStore, NexusEnvelopes};
use crate::messages::{protocol_error, reply_with};
use crate::session::Session;

/// Stores a sealed share key for one of this user's own devices.
pub(crate) async fn put(
    session: &Session,
    envelopes: &NexusEnvelopes<DefaultStore>,
    request: &Envelope,
    put: &PutKeyEnvelope,
    now_unix_ms: u64,
) -> Envelope {
    let Some(identity) = session.identity() else {
        return unauthenticated(request, now_unix_ms);
    };
    let Ok(share_id) = <[u8; SHARE_ID_BYTES]>::try_from(put.share_id.as_slice()) else {
        return malformed(request, "share_id must name a share", now_unix_ms);
    };
    let Ok(recipient_device_id) = DeviceId::try_from(put.recipient_device_id.as_slice()) else {
        return malformed(
            request,
            "recipient_device_id must name a device",
            now_unix_ms,
        );
    };

    let outcome = envelopes
        .put_key_envelope(
            identity.user.user_id,
            PutKeyEnvelopeRequest {
                share_id,
                recipient_device_id,
                ephemeral_public_key: &put.ephemeral_public_key,
                ciphertext: &put.ciphertext,
            },
        )
        .await;
    match outcome {
        Ok(()) => reply_with(
            request,
            Payload::KeyEnvelopePut(KeyEnvelopePut {
                share_id: share_id.to_vec(),
                recipient_device_id: recipient_device_id.to_vec(),
            }),
            now_unix_ms,
        ),
        Err(error) => rejection(request, &error, now_unix_ms),
    }
}

/// Returns every envelope addressed to this connection's own device.
///
/// A connection only ever asks for its own: the device is taken from the
/// authenticated session rather than the request, so there is nothing to
/// authorize beyond having authenticated at all.
pub(crate) async fn list(
    session: &Session,
    envelopes: &NexusEnvelopes<DefaultStore>,
    request: &Envelope,
    list: &ListKeyEnvelopesRequest,
    now_unix_ms: u64,
) -> Envelope {
    let Some(identity) = session.identity() else {
        return unauthenticated(request, now_unix_ms);
    };
    let after_share_id = if list.after_share_id.is_empty() {
        None
    } else {
        let Ok(after_share_id) = <[u8; SHARE_ID_BYTES]>::try_from(list.after_share_id.as_slice())
        else {
            return malformed(request, "after_share_id must name a share", now_unix_ms);
        };
        Some(after_share_id)
    };
    match envelopes
        .list_key_envelopes(&identity.device, after_share_id)
        .await
    {
        Ok(found) => reply_with(
            request,
            Payload::ListKeyEnvelopesResponse(ListKeyEnvelopesResponse {
                envelopes: found.envelopes.into_iter().map(sealed).collect(),
                next_after_share_id: found
                    .next_after_share_id
                    .map_or_else(Vec::new, |share_id| share_id.to_vec()),
            }),
            now_unix_ms,
        ),
        Err(error) => rejection(request, &error, now_unix_ms),
    }
}

fn sealed(record: KeyEnvelopeRecord) -> KeyEnvelope {
    KeyEnvelope {
        share_id: record.share_id.to_vec(),
        ephemeral_public_key: record.ephemeral_public_key.to_vec(),
        ciphertext: record.ciphertext,
    }
}

fn unauthenticated(request: &Envelope, now_unix_ms: u64) -> Envelope {
    protocol_error(
        ProtocolErrorCode::Unauthenticated,
        request.message_id.clone(),
        "authenticate before using key envelopes".to_owned(),
        now_unix_ms,
    )
}

fn malformed(request: &Envelope, message: &str, now_unix_ms: u64) -> Envelope {
    protocol_error(
        ProtocolErrorCode::InvalidMessage,
        request.message_id.clone(),
        message.to_owned(),
        now_unix_ms,
    )
}

/// Maps an envelope failure onto the wire, keeping storage detail off it.
fn rejection(request: &Envelope, error: &EnvelopeError, now_unix_ms: u64) -> Envelope {
    let code = match error {
        EnvelopeError::InvalidEphemeralKeyLength { .. }
        | EnvelopeError::CiphertextTooLarge { .. } => ProtocolErrorCode::InvalidMessage,
        EnvelopeError::UnknownRecipient
        | EnvelopeError::NotYourDevice
        | EnvelopeError::RecipientRevoked => ProtocolErrorCode::Unauthorized,
        EnvelopeError::Repository(_) => ProtocolErrorCode::Internal,
    };
    let message = match error {
        EnvelopeError::Repository(_) => "the identity store is unavailable".to_owned(),
        other => other.to_string(),
    };
    protocol_error(code, request.message_id.clone(), message, now_unix_ms)
}
