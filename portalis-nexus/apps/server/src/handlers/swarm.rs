//! Source-address-bound swarm lease commands.

use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_protocol::v1::{
    AddressFamily as WireFamily, AnnouncePeer, Envelope, LookupPeersRequest, LookupPeersResponse,
    PeerAnnounced, PeerEndpoint, PeerWithdrawn, ProtocolErrorCode, WithdrawPeer,
};
use portalis_nexus_server_core::{AddressFamily, PeerAnnouncement, SwarmError};

use crate::messages::{protocol_error, reply_with};
use crate::session::Session;
use crate::state::AppState;

pub(crate) fn announce(
    session: &Session,
    state: &AppState,
    request: &Envelope,
    announce: &AnnouncePeer,
    now: u64,
) -> Envelope {
    let Some(identity) = session.identity() else {
        return unauthenticated(request, now);
    };
    let Ok(family) = family(announce.address_family) else {
        return malformed(request, "address_family is required", now);
    };
    match state.swarm().announce(
        PeerAnnouncement {
            info_hash: &announce.info_hash,
            device_id: identity.device.device_id,
            connection_id: session.connection_id(),
            address: session.observed_ip(),
            port: announce.listen_port,
            family,
            transport_capabilities: announce.transport_capabilities,
            requested_lease_seconds: announce.requested_lease_seconds,
        },
        now,
    ) {
        Ok(lease) => reply_with(
            request,
            Payload::PeerAnnounced(PeerAnnounced {
                info_hash: lease.info_hash,
                expires_at_unix_ns: lease.expires_at_unix_ns,
            }),
            now,
        ),
        Err(error) => rejection(request, &error, now),
    }
}

pub(crate) fn lookup(
    session: &Session,
    state: &AppState,
    request: &Envelope,
    lookup: &LookupPeersRequest,
    now: u64,
) -> Envelope {
    let Some(identity) = session.identity() else {
        return unauthenticated(request, now);
    };
    let wanted_family = match WireFamily::try_from(lookup.address_family) {
        Ok(WireFamily::Unspecified) => None,
        Ok(WireFamily::Ipv4) => Some(AddressFamily::Ipv4),
        Ok(WireFamily::Ipv6) => Some(AddressFamily::Ipv6),
        Err(_) => return malformed(request, "address_family is invalid", now),
    };
    match state.swarm().lookup(
        &lookup.info_hash,
        identity.device.device_id,
        wanted_family,
        lookup.transport_capabilities,
        now,
    ) {
        Ok(peers) => reply_with(
            request,
            Payload::LookupPeersResponse(LookupPeersResponse {
                info_hash: lookup.info_hash.clone(),
                peers: peers
                    .into_iter()
                    .map(|peer| PeerEndpoint {
                        device_id: peer.device_id.to_vec(),
                        ip_address: match peer.address {
                            std::net::IpAddr::V4(address) => address.octets().to_vec(),
                            std::net::IpAddr::V6(address) => address.octets().to_vec(),
                        },
                        port: u32::from(peer.port),
                        address_family: match peer.family {
                            AddressFamily::Ipv4 => WireFamily::Ipv4 as i32,
                            AddressFamily::Ipv6 => WireFamily::Ipv6 as i32,
                        },
                        transport_capabilities: peer.transport_capabilities,
                        expires_at_unix_ns: peer.expires_at_unix_ns,
                    })
                    .collect(),
            }),
            now,
        ),
        Err(error) => rejection(request, &error, now),
    }
}

pub(crate) fn withdraw(
    session: &Session,
    state: &AppState,
    request: &Envelope,
    withdraw: &WithdrawPeer,
    now: u64,
) -> Envelope {
    let Some(identity) = session.identity() else {
        return unauthenticated(request, now);
    };
    match state
        .swarm()
        .withdraw(&withdraw.info_hash, identity.device.device_id)
    {
        Ok(()) => reply_with(
            request,
            Payload::PeerWithdrawn(PeerWithdrawn {
                info_hash: withdraw.info_hash.clone(),
            }),
            now,
        ),
        Err(error) => rejection(request, &error, now),
    }
}

fn family(value: i32) -> Result<AddressFamily, ()> {
    match WireFamily::try_from(value) {
        Ok(WireFamily::Ipv4) => Ok(AddressFamily::Ipv4),
        Ok(WireFamily::Ipv6) => Ok(AddressFamily::Ipv6),
        _ => Err(()),
    }
}

fn unauthenticated(request: &Envelope, now: u64) -> Envelope {
    protocol_error(
        ProtocolErrorCode::Unauthenticated,
        request.message_id.clone(),
        "authenticate before using swarm discovery".to_owned(),
        now,
    )
}

fn malformed(request: &Envelope, message: &str, now: u64) -> Envelope {
    protocol_error(
        ProtocolErrorCode::InvalidMessage,
        request.message_id.clone(),
        message.to_owned(),
        now,
    )
}

fn rejection(request: &Envelope, error: &SwarmError, now: u64) -> Envelope {
    malformed(request, &error.to_string(), now)
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use portalis_nexus_protocol::v1::{Ping, ProtocolError};
    use portalis_nexus_protocol::{USER_ID_BYTES, new_message_id};
    use portalis_nexus_server_core::{
        DeviceRecord, Identity, IdentityRepository, ProtocolPolicy, UserRecord,
    };

    use super::*;

    const NOW: u64 = 1_700_000_000_000_000_000;
    const HASH: [u8; 20] = [7; 20];

    fn request() -> Envelope {
        Envelope {
            message_id: new_message_id(),
            correlation_id: Vec::new(),
            timestamp_unix_ns: NOW,
            payload: Some(Payload::Ping(Ping { nonce: 1 })),
        }
    }

    fn refusal(reply: &Envelope) -> Option<(ProtocolErrorCode, String)> {
        match &reply.payload {
            Some(Payload::ProtocolError(ProtocolError { code, message, .. })) => Some((
                ProtocolErrorCode::try_from(*code).unwrap_or(ProtocolErrorCode::Unspecified),
                message.clone(),
            )),
            _ => None,
        }
    }

    fn lease_of(reply: &Envelope) -> Option<PeerAnnounced> {
        match &reply.payload {
            Some(Payload::PeerAnnounced(announced)) => Some(announced.clone()),
            _ => None,
        }
    }

    fn peers_of(reply: &Envelope) -> Option<LookupPeersResponse> {
        match &reply.payload {
            Some(Payload::LookupPeersResponse(response)) => Some(response.clone()),
            _ => None,
        }
    }

    fn withdrawal_of(reply: &Envelope) -> Option<PeerWithdrawn> {
        match &reply.payload {
            Some(Payload::PeerWithdrawn(withdrawn)) => Some(withdrawn.clone()),
            _ => None,
        }
    }

    fn anonymous() -> Session {
        let policy = ProtocolPolicy::new(1, 1).expect("range");
        Session::new(&crate::messages::hello_payload(&policy, NOW))
    }

    /// A seeder whose source address the socket observed, which is the only
    /// address discovery will ever hand out for it.
    async fn seeder(state: &AppState, seed: u8, observed: IpAddr) -> Session {
        let user_id = [seed; USER_ID_BYTES];
        let user = UserRecord {
            user_id,
            username: format!("user{seed}"),
            normalized_username: format!("user{seed}"),
            discriminator: "7Q2XZ".to_owned(),
            created_at_unix_ns: NOW,
        };
        let device = DeviceRecord {
            device_id: [seed; 32],
            user_id,
            public_key: [seed; 32],
            encryption_public_key: [seed; 32],
            created_at_unix_ns: NOW,
            last_authenticated_at_unix_ns: Some(NOW),
            revoked_at_unix_ns: None,
        };
        state
            .store()
            .insert_registration(user.clone(), device.clone())
            .await
            .expect("seeded");

        let policy = ProtocolPolicy::new(1, 1).expect("range");
        let mut session =
            Session::new(&crate::messages::hello_payload(&policy, NOW)).with_observed_ip(observed);
        session.bind(Identity { user, device });
        session
    }

    fn announcement(family: WireFamily) -> AnnouncePeer {
        AnnouncePeer {
            info_hash: HASH.to_vec(),
            listen_port: 6881,
            address_family: family as i32,
            transport_capabilities: 1,
            requested_lease_seconds: 90,
        }
    }

    #[test]
    fn every_command_refuses_an_unauthenticated_connection() {
        let state = AppState::default();
        let session = anonymous();

        let replies = [
            announce(
                &session,
                &state,
                &request(),
                &announcement(WireFamily::Ipv4),
                NOW,
            ),
            lookup(
                &session,
                &state,
                &request(),
                &LookupPeersRequest {
                    info_hash: HASH.to_vec(),
                    address_family: WireFamily::Unspecified as i32,
                    transport_capabilities: 0,
                },
                NOW,
            ),
            withdraw(
                &session,
                &state,
                &request(),
                &WithdrawPeer {
                    info_hash: HASH.to_vec(),
                },
                NOW,
            ),
        ];

        for reply in &replies {
            let (code, message) = refusal(reply).expect("a refusal");
            assert_eq!(code, ProtocolErrorCode::Unauthenticated);
            assert_eq!(message, "authenticate before using swarm discovery");
        }
    }

    /// A seeder must say which family it listens on, because the server pairs
    /// that with the address it observed rather than one the client supplied.
    #[tokio::test]
    async fn an_announcement_needs_a_family_that_matches_the_observed_address() {
        let state = AppState::default();
        let session = seeder(&state, 1, IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))).await;

        let unspecified = announce(
            &session,
            &state,
            &request(),
            &announcement(WireFamily::Unspecified),
            NOW,
        );
        assert_eq!(
            refusal(&unspecified).expect("a refusal"),
            (
                ProtocolErrorCode::InvalidMessage,
                "address_family is required".to_owned()
            )
        );

        let out_of_range = announce(
            &session,
            &state,
            &request(),
            &AnnouncePeer {
                address_family: 99,
                ..announcement(WireFamily::Ipv4)
            },
            NOW,
        );
        assert_eq!(
            refusal(&out_of_range).expect("a refusal").0,
            ProtocolErrorCode::InvalidMessage
        );

        // Claiming IPv6 from an observed IPv4 socket is refused by the domain.
        let mismatched = announce(
            &session,
            &state,
            &request(),
            &announcement(WireFamily::Ipv6),
            NOW,
        );
        assert_eq!(
            refusal(&mismatched).expect("a refusal"),
            (
                ProtocolErrorCode::InvalidMessage,
                "the announced address family does not match the observed source address"
                    .to_owned()
            )
        );
    }

    /// Two seeders announce and a third finds them at the addresses the
    /// sockets observed, never at one they asked to advertise.
    #[tokio::test]
    async fn peers_are_discovered_at_their_observed_addresses() {
        let state = AppState::default();
        let four = seeder(&state, 1, IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))).await;
        let six = seeder(
            &state,
            2,
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
        )
        .await;
        let asker = seeder(&state, 3, IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1))).await;

        let announced = announce(
            &four,
            &state,
            &request(),
            &announcement(WireFamily::Ipv4),
            NOW,
        );
        let lease = lease_of(&announced).expect("a lease");
        assert_eq!(lease.info_hash, HASH.to_vec());
        assert!(lease.expires_at_unix_ns > NOW, "a lease expires later");
        announce(
            &six,
            &state,
            &request(),
            &announcement(WireFamily::Ipv6),
            NOW,
        );

        let found = lookup(
            &asker,
            &state,
            &request(),
            &LookupPeersRequest {
                info_hash: HASH.to_vec(),
                address_family: WireFamily::Unspecified as i32,
                transport_capabilities: 0,
            },
            NOW,
        );
        let response = peers_of(&found).expect("peers");

        assert_eq!(response.info_hash, HASH.to_vec());
        assert_eq!(response.peers.len(), 2);
        let mut addresses: Vec<_> = response
            .peers
            .iter()
            .map(|peer| (peer.ip_address.clone(), peer.address_family, peer.port))
            .collect();
        addresses.sort_unstable();
        assert_eq!(
            addresses,
            vec![
                (
                    Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)
                        .octets()
                        .to_vec(),
                    WireFamily::Ipv6 as i32,
                    6881
                ),
                (
                    Ipv4Addr::new(203, 0, 113, 7).octets().to_vec(),
                    WireFamily::Ipv4 as i32,
                    6881
                ),
            ]
        );
    }

    /// Asking for one family narrows the answer; an unknown one is refused
    /// rather than silently treated as "any".
    #[tokio::test]
    async fn a_lookup_may_ask_for_one_family_but_not_an_unknown_one() {
        let state = AppState::default();
        let four = seeder(&state, 1, IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))).await;
        announce(
            &four,
            &state,
            &request(),
            &announcement(WireFamily::Ipv4),
            NOW,
        );
        let asker = seeder(&state, 3, IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1))).await;

        for family in [WireFamily::Ipv4, WireFamily::Ipv6] {
            let reply = lookup(
                &asker,
                &state,
                &request(),
                &LookupPeersRequest {
                    info_hash: HASH.to_vec(),
                    address_family: family as i32,
                    transport_capabilities: 1,
                },
                NOW,
            );
            // Preference, not exclusion: the one lease is returned either way.
            assert_eq!(peers_of(&reply).expect("peers").peers.len(), 1);
        }

        let unknown = lookup(
            &asker,
            &state,
            &request(),
            &LookupPeersRequest {
                info_hash: HASH.to_vec(),
                address_family: 99,
                transport_capabilities: 0,
            },
            NOW,
        );
        assert_eq!(
            refusal(&unknown).expect("a refusal"),
            (
                ProtocolErrorCode::InvalidMessage,
                "address_family is invalid".to_owned()
            )
        );
    }

    /// Withdrawing takes a seeder out of the swarm before its lease expires,
    /// and a malformed hash is refused by every command that carries one.
    #[tokio::test]
    async fn withdrawing_removes_the_lease_and_malformed_hashes_are_refused() {
        let state = AppState::default();
        let session = seeder(&state, 1, IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))).await;
        announce(
            &session,
            &state,
            &request(),
            &announcement(WireFamily::Ipv4),
            NOW,
        );

        let withdrawn = withdraw(
            &session,
            &state,
            &request(),
            &WithdrawPeer {
                info_hash: HASH.to_vec(),
            },
            NOW,
        );
        let gone = withdrawal_of(&withdrawn).expect("a withdrawal");
        assert_eq!(gone.info_hash, HASH.to_vec());
        assert!(state.swarm().is_empty(), "the last lease is gone");

        // Each matcher recognises only its own answer.
        let other = request();
        assert!(lease_of(&other).is_none());
        assert!(peers_of(&other).is_none());
        assert!(withdrawal_of(&other).is_none());
        assert!(refusal(&other).is_none());

        let short = vec![7; 19];
        for reply in [
            announce(
                &session,
                &state,
                &request(),
                &AnnouncePeer {
                    info_hash: short.clone(),
                    ..announcement(WireFamily::Ipv4)
                },
                NOW,
            ),
            lookup(
                &session,
                &state,
                &request(),
                &LookupPeersRequest {
                    info_hash: short.clone(),
                    address_family: WireFamily::Unspecified as i32,
                    transport_capabilities: 0,
                },
                NOW,
            ),
            withdraw(
                &session,
                &state,
                &request(),
                &WithdrawPeer { info_hash: short },
                NOW,
            ),
        ] {
            let (code, message) = refusal(&reply).expect("a refusal");
            assert_eq!(code, ProtocolErrorCode::InvalidMessage);
            assert_eq!(message, "info_hash must contain 20 or 32 bytes");
        }
    }
}
