//! Payload names safe to use in operational telemetry.

use crate::v1::envelope::Payload;

/// Returns only the protobuf variant name, never any payload content.
///
/// Keeping this mapping beside the protocol gives clients and servers the
/// same stable tracing vocabulary without risking capsules, key envelopes,
/// handoff ciphertext, challenges, or private metadata in logs.
#[must_use]
pub const fn payload_name(payload: Option<&Payload>) -> &'static str {
    match payload {
        None => "missing",
        Some(Payload::ServerHello(_)) => "server_hello",
        Some(Payload::RegisterUser(_)) => "register_user",
        Some(Payload::AuthenticateDevice(_)) => "authenticate_device",
        Some(Payload::Authenticated(_)) => "authenticated",
        Some(Payload::Ping(_)) => "ping",
        Some(Payload::Pong(_)) => "pong",
        Some(Payload::Ack(_)) => "ack",
        Some(Payload::ProtocolError(_)) => "protocol_error",
        Some(Payload::LinkDevice(_)) => "link_device",
        Some(Payload::DeviceLinked(_)) => "device_linked",
        Some(Payload::ResolveHandleRequest(_)) => "resolve_handle",
        Some(Payload::ResolveHandleResponse(_)) => "handle_resolved",
        Some(Payload::FriendCommand(_)) => "friend_command",
        Some(Payload::FriendEvent(_)) => "friend_event",
        Some(Payload::ListFriendsRequest(_)) => "list_friends",
        Some(Payload::ListFriendsResponse(_)) => "friends_listed",
        Some(Payload::PresenceEvent(_)) => "presence_event",
        Some(Payload::PutKeyEnvelope(_)) => "put_key_envelope",
        Some(Payload::KeyEnvelopePut(_)) => "key_envelope_put",
        Some(Payload::ListKeyEnvelopesRequest(_)) => "list_key_envelopes",
        Some(Payload::ListKeyEnvelopesResponse(_)) => "key_envelopes_listed",
        Some(Payload::RevokeShareAccess(_)) => "revoke_share_access",
        Some(Payload::ShareAccessRevoked(_)) => "share_access_revoked",
        Some(Payload::PublishShare(_)) => "publish_share",
        Some(Payload::SharePublished(_)) => "share_published",
        Some(Payload::ListSharesRequest(_)) => "list_shares",
        Some(Payload::ListSharesResponse(_)) => "shares_listed",
        Some(Payload::FetchShareRequest(_)) => "fetch_share",
        Some(Payload::FetchShareResponse(_)) => "share_fetched",
        Some(Payload::GrantShareAccess(_)) => "grant_share_access",
        Some(Payload::ShareAccessGranted(_)) => "share_access_granted",
        Some(Payload::ShareEvent(_)) => "share_event",
        Some(Payload::ShareHandoff(_)) => "share_handoff",
        Some(Payload::AnnouncePeer(_)) => "announce_peer",
        Some(Payload::PeerAnnounced(_)) => "peer_announced",
        Some(Payload::LookupPeersRequest(_)) => "lookup_peers",
        Some(Payload::LookupPeersResponse(_)) => "peers_found",
        Some(Payload::WithdrawPeer(_)) => "withdraw_peer",
        Some(Payload::PeerWithdrawn(_)) => "peer_withdrawn",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::Ping;

    #[test]
    fn names_payloads_without_exposing_their_fields() {
        let payload = Payload::Ping(Ping { nonce: 42 });

        assert_eq!(payload_name(Some(&payload)), "ping");
        assert_eq!(payload_name(None), "missing");
    }
}
