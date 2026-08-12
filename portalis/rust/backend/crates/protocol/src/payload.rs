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
    use crate::v1;

    /// Every variant, because the point of this mapping is that a payload
    /// added later cannot slip through unnamed into a trace.
    #[test]
    #[expect(clippy::too_many_lines, reason = "one line per payload, by design")]
    fn names_every_payload_without_exposing_their_fields() {
        let cases: &[(Payload, &str)] = &[
            (
                Payload::ServerHello(v1::ServerHello::default()),
                "server_hello",
            ),
            (
                Payload::RegisterUser(v1::RegisterUser::default()),
                "register_user",
            ),
            (
                Payload::AuthenticateDevice(v1::AuthenticateDevice::default()),
                "authenticate_device",
            ),
            (
                Payload::Authenticated(v1::Authenticated::default()),
                "authenticated",
            ),
            (Payload::Ping(v1::Ping::default()), "ping"),
            (Payload::Pong(v1::Pong::default()), "pong"),
            (Payload::Ack(v1::Ack::default()), "ack"),
            (
                Payload::ProtocolError(v1::ProtocolError::default()),
                "protocol_error",
            ),
            (
                Payload::LinkDevice(v1::LinkDevice::default()),
                "link_device",
            ),
            (
                Payload::DeviceLinked(v1::DeviceLinked::default()),
                "device_linked",
            ),
            (
                Payload::ResolveHandleRequest(v1::ResolveHandleRequest::default()),
                "resolve_handle",
            ),
            (
                Payload::ResolveHandleResponse(v1::ResolveHandleResponse::default()),
                "handle_resolved",
            ),
            (
                Payload::FriendCommand(v1::FriendCommand::default()),
                "friend_command",
            ),
            (
                Payload::FriendEvent(v1::FriendEvent::default()),
                "friend_event",
            ),
            (
                Payload::ListFriendsRequest(v1::ListFriendsRequest::default()),
                "list_friends",
            ),
            (
                Payload::ListFriendsResponse(v1::ListFriendsResponse::default()),
                "friends_listed",
            ),
            (
                Payload::PresenceEvent(v1::PresenceEvent::default()),
                "presence_event",
            ),
            (
                Payload::PutKeyEnvelope(v1::PutKeyEnvelope::default()),
                "put_key_envelope",
            ),
            (
                Payload::KeyEnvelopePut(v1::KeyEnvelopePut::default()),
                "key_envelope_put",
            ),
            (
                Payload::ListKeyEnvelopesRequest(v1::ListKeyEnvelopesRequest::default()),
                "list_key_envelopes",
            ),
            (
                Payload::ListKeyEnvelopesResponse(v1::ListKeyEnvelopesResponse::default()),
                "key_envelopes_listed",
            ),
            (
                Payload::RevokeShareAccess(v1::RevokeShareAccess::default()),
                "revoke_share_access",
            ),
            (
                Payload::ShareAccessRevoked(v1::ShareAccessRevoked::default()),
                "share_access_revoked",
            ),
            (
                Payload::PublishShare(v1::PublishShare::default()),
                "publish_share",
            ),
            (
                Payload::SharePublished(v1::SharePublished::default()),
                "share_published",
            ),
            (
                Payload::ListSharesRequest(v1::ListSharesRequest::default()),
                "list_shares",
            ),
            (
                Payload::ListSharesResponse(v1::ListSharesResponse::default()),
                "shares_listed",
            ),
            (
                Payload::FetchShareRequest(v1::FetchShareRequest::default()),
                "fetch_share",
            ),
            (
                Payload::FetchShareResponse(v1::FetchShareResponse::default()),
                "share_fetched",
            ),
            (
                Payload::GrantShareAccess(v1::GrantShareAccess::default()),
                "grant_share_access",
            ),
            (
                Payload::ShareAccessGranted(v1::ShareAccessGranted::default()),
                "share_access_granted",
            ),
            (
                Payload::ShareEvent(v1::ShareEvent::default()),
                "share_event",
            ),
            (
                Payload::ShareHandoff(v1::ShareHandoff::default()),
                "share_handoff",
            ),
            (
                Payload::AnnouncePeer(v1::AnnouncePeer::default()),
                "announce_peer",
            ),
            (
                Payload::PeerAnnounced(v1::PeerAnnounced::default()),
                "peer_announced",
            ),
            (
                Payload::LookupPeersRequest(v1::LookupPeersRequest::default()),
                "lookup_peers",
            ),
            (
                Payload::LookupPeersResponse(v1::LookupPeersResponse::default()),
                "peers_found",
            ),
            (
                Payload::WithdrawPeer(v1::WithdrawPeer::default()),
                "withdraw_peer",
            ),
            (
                Payload::PeerWithdrawn(v1::PeerWithdrawn::default()),
                "peer_withdrawn",
            ),
        ];

        for (payload, expected) in cases {
            assert_eq!(payload_name(Some(payload)), *expected);
        }
        assert_eq!(payload_name(None), "missing");
        assert_eq!(
            cases.len(),
            39,
            "a new payload needs a name and a case here"
        );
    }
}
