//! Short-lived, source-address-bound `BitTorrent` peer discovery.

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::{Mutex, MutexGuard};

use portalis_nexus_protocol::{
    INFO_HASH_V1_BYTES, INFO_HASH_V2_BYTES, MAX_SWARM_CANDIDATES, SWARM_LEASE_SECONDS,
};
use thiserror::Error;

use crate::ports::DeviceId;
use crate::presence::ConnectionId;

const NANOS_PER_SECOND: u64 = 1_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

impl AddressFamily {
    #[must_use]
    pub const fn matches(self, address: IpAddr) -> bool {
        matches!(
            (self, address),
            (Self::Ipv4, IpAddr::V4(_)) | (Self::Ipv6, IpAddr::V6(_))
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerLease {
    pub info_hash: Vec<u8>,
    pub device_id: DeviceId,
    pub connection_id: ConnectionId,
    pub address: IpAddr,
    pub port: u16,
    pub family: AddressFamily,
    pub transport_capabilities: u32,
    pub announced_at_unix_ns: u64,
    pub expires_at_unix_ns: u64,
    generation: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct PeerAnnouncement<'a> {
    pub info_hash: &'a [u8],
    pub device_id: DeviceId,
    pub connection_id: ConnectionId,
    pub address: IpAddr,
    pub port: u32,
    pub family: AddressFamily,
    pub transport_capabilities: u32,
    pub requested_lease_seconds: u32,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SwarmError {
    #[error("info_hash must contain 20 or 32 bytes")]
    InvalidInfoHash,
    #[error("listen_port must be between 1 and 65535")]
    InvalidPort,
    #[error("the announced address family does not match the observed source address")]
    AddressFamilyMismatch,
}

#[derive(Debug, Default)]
pub struct SwarmRegistry {
    leases: Mutex<HashMap<Vec<u8>, HashMap<DeviceId, PeerLease>>>,
}

impl SwarmRegistry {
    /// # Errors
    /// Returns [`SwarmError`] when the hash, port, or claimed family is invalid.
    pub fn announce(
        &self,
        announcement: PeerAnnouncement<'_>,
        now_unix_ns: u64,
    ) -> Result<PeerLease, SwarmError> {
        validate_info_hash(announcement.info_hash)?;
        let port = u16::try_from(announcement.port)
            .ok()
            .filter(|port| *port != 0)
            .ok_or(SwarmError::InvalidPort)?;
        if !announcement.family.matches(announcement.address) {
            return Err(SwarmError::AddressFamilyMismatch);
        }
        let duration = u64::from(announcement.requested_lease_seconds)
            .clamp(1, SWARM_LEASE_SECONDS)
            .saturating_mul(NANOS_PER_SECOND);
        let mut leases = self.lock();
        let peers = leases.entry(announcement.info_hash.to_vec()).or_default();
        let generation = peers
            .get(&announcement.device_id)
            .map_or(1, |lease| lease.generation.saturating_add(1));
        let lease = PeerLease {
            info_hash: announcement.info_hash.to_vec(),
            device_id: announcement.device_id,
            connection_id: announcement.connection_id,
            address: announcement.address,
            port,
            family: announcement.family,
            transport_capabilities: announcement.transport_capabilities,
            announced_at_unix_ns: now_unix_ns,
            expires_at_unix_ns: now_unix_ns.saturating_add(duration),
            generation,
        };
        peers.insert(announcement.device_id, lease.clone());
        Ok(lease)
    }

    /// Returns current candidates. Expiration is checked on every read, so it
    /// remains correct even if an optional deadline wake-up is delayed.
    ///
    /// # Errors
    /// Returns [`SwarmError`] when the info hash is malformed.
    pub fn lookup(
        &self,
        info_hash: &[u8],
        requester: DeviceId,
        family: Option<AddressFamily>,
        transport_capabilities: u32,
        now_unix_ns: u64,
    ) -> Result<Vec<PeerLease>, SwarmError> {
        validate_info_hash(info_hash)?;
        let mut leases = self.lock();
        let Some(peers) = leases.get_mut(info_hash) else {
            return Ok(Vec::new());
        };
        peers.retain(|_, lease| lease.expires_at_unix_ns > now_unix_ns);
        let mut candidates: Vec<_> = peers
            .values()
            .filter(|lease| lease.device_id != requester)
            .cloned()
            .collect();
        candidates.sort_by_key(|lease| {
            let family_penalty = family.is_some_and(|wanted| wanted != lease.family);
            let transport_penalty = transport_capabilities != 0
                && lease.transport_capabilities & transport_capabilities == 0;
            (
                family_penalty,
                transport_penalty,
                std::cmp::Reverse(lease.announced_at_unix_ns),
                mix(lease),
            )
        });

        // First take one candidate per public network prefix, then fill any
        // remaining slots. This avoids returning 32 peers behind one router
        // when the swarm has broader reachability available.
        let mut selected = Vec::with_capacity(candidates.len().min(MAX_SWARM_CANDIDATES));
        let mut prefixes = HashSet::new();
        for lease in &candidates {
            if prefixes.insert(prefix(lease.address)) {
                selected.push(lease.clone());
                if selected.len() == MAX_SWARM_CANDIDATES {
                    return Ok(selected);
                }
            }
        }
        for lease in candidates {
            if !selected
                .iter()
                .any(|picked| picked.device_id == lease.device_id)
            {
                selected.push(lease);
                if selected.len() == MAX_SWARM_CANDIDATES {
                    break;
                }
            }
        }
        Ok(selected)
    }

    /// # Errors
    /// Returns [`SwarmError`] when the info hash is malformed.
    pub fn withdraw(&self, info_hash: &[u8], device_id: DeviceId) -> Result<(), SwarmError> {
        validate_info_hash(info_hash)?;
        let mut leases = self.lock();
        if let Some(peers) = leases.get_mut(info_hash) {
            peers.remove(&device_id);
            if peers.is_empty() {
                leases.remove(info_hash);
            }
        }
        Ok(())
    }

    pub fn remove_connection(&self, connection_id: ConnectionId) {
        self.lock().retain(|_, peers| {
            peers.retain(|_, lease| lease.connection_id != connection_id);
            !peers.is_empty()
        });
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().values().map(HashMap::len).sum()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Vec<u8>, HashMap<DeviceId, PeerLease>>> {
        self.leases
            .lock()
            .expect("the swarm registry is not poisoned")
    }
}

fn validate_info_hash(info_hash: &[u8]) -> Result<(), SwarmError> {
    if matches!(info_hash.len(), INFO_HASH_V1_BYTES | INFO_HASH_V2_BYTES) {
        Ok(())
    } else {
        Err(SwarmError::InvalidInfoHash)
    }
}

#[derive(Hash, PartialEq, Eq)]
enum NetworkPrefix {
    V4([u8; 3]),
    V6([u8; 8]),
}

fn prefix(address: IpAddr) -> NetworkPrefix {
    match address {
        IpAddr::V4(address) => {
            let bytes = address.octets();
            NetworkPrefix::V4([bytes[0], bytes[1], bytes[2]])
        }
        IpAddr::V6(address) => {
            let bytes = address.octets();
            NetworkPrefix::V6(bytes[..8].try_into().expect("an IPv6 /64 is eight bytes"))
        }
    }
}

fn mix(lease: &PeerLease) -> u64 {
    lease
        .device_id
        .iter()
        .chain(lease.info_hash.iter())
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    const HASH: [u8; 20] = [1; 20];
    const NOW: u64 = 1_700_000_000_000_000_000;

    fn announce(registry: &SwarmRegistry, seed: u8, address: [u8; 4], now: u64) {
        registry
            .announce(
                PeerAnnouncement {
                    info_hash: &HASH,
                    device_id: [seed; 32],
                    connection_id: [seed; 16],
                    address: IpAddr::V4(Ipv4Addr::from(address)),
                    port: 6881,
                    family: AddressFamily::Ipv4,
                    transport_capabilities: 1,
                    requested_lease_seconds: 90,
                },
                now,
            )
            .expect("announced");
    }

    #[test]
    fn current_peers_are_found_and_expired_peers_disappear() {
        let registry = SwarmRegistry::default();
        announce(&registry, 1, [10, 0, 0, 1], NOW);
        announce(&registry, 2, [10, 0, 1, 1], NOW);

        assert_eq!(
            registry.lookup(&HASH, [1; 32], None, 1, NOW).unwrap().len(),
            1
        );
        assert!(
            registry
                .lookup(
                    &HASH,
                    [9; 32],
                    None,
                    1,
                    NOW + SWARM_LEASE_SECONDS * NANOS_PER_SECOND
                )
                .unwrap()
                .is_empty()
        );
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn refreshes_replace_a_device_lease_and_disconnect_removes_it() {
        let registry = SwarmRegistry::default();
        announce(&registry, 1, [10, 0, 0, 1], NOW);
        announce(&registry, 1, [10, 0, 0, 2], NOW + 1);
        assert_eq!(registry.len(), 1);

        registry.remove_connection([1; 16]);
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn rejects_untrusted_endpoint_fields() {
        let registry = SwarmRegistry::default();
        assert_eq!(
            registry.announce(
                PeerAnnouncement {
                    info_hash: &[1; 19],
                    device_id: [1; 32],
                    connection_id: [1; 16],
                    address: "127.0.0.1".parse().unwrap(),
                    port: 6881,
                    family: AddressFamily::Ipv4,
                    transport_capabilities: 1,
                    requested_lease_seconds: 90,
                },
                NOW,
            ),
            Err(SwarmError::InvalidInfoHash)
        );
        assert_eq!(
            registry.announce(
                PeerAnnouncement {
                    info_hash: &HASH,
                    device_id: [1; 32],
                    connection_id: [1; 16],
                    address: "::1".parse().unwrap(),
                    port: 6881,
                    family: AddressFamily::Ipv4,
                    transport_capabilities: 1,
                    requested_lease_seconds: 90,
                },
                NOW,
            ),
            Err(SwarmError::AddressFamilyMismatch)
        );
        for port in [0, 65_536] {
            assert_eq!(
                registry.announce(
                    PeerAnnouncement {
                        info_hash: &HASH,
                        device_id: [1; 32],
                        connection_id: [1; 16],
                        address: "127.0.0.1".parse().unwrap(),
                        port,
                        family: AddressFamily::Ipv4,
                        transport_capabilities: 1,
                        requested_lease_seconds: 90,
                    },
                    NOW,
                ),
                Err(SwarmError::InvalidPort)
            );
        }
    }

    #[test]
    fn lookup_is_bounded_diverse_and_compatible_first() {
        let registry = SwarmRegistry::default();
        assert!(registry.is_empty());
        assert!(
            registry
                .lookup(&HASH, [0; 32], None, 0, NOW)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            registry.lookup(&[1; 19], [0; 32], None, 0, NOW),
            Err(SwarmError::InvalidInfoHash)
        );

        for seed in 1..=40 {
            announce(&registry, seed, [10, 1, 1, seed], NOW + u64::from(seed));
        }
        registry
            .announce(
                PeerAnnouncement {
                    info_hash: &HASH,
                    device_id: [50; 32],
                    connection_id: [50; 16],
                    address: "2001:db8::1".parse().unwrap(),
                    port: 6881,
                    family: AddressFamily::Ipv6,
                    transport_capabilities: 2,
                    requested_lease_seconds: 90,
                },
                NOW + 50,
            )
            .unwrap();

        let found = registry
            .lookup(&HASH, [0; 32], Some(AddressFamily::Ipv6), 2, NOW)
            .unwrap();
        assert_eq!(found.len(), MAX_SWARM_CANDIDATES);
        assert_eq!(found[0].family, AddressFamily::Ipv6);
        assert_eq!(found[0].transport_capabilities, 2);
    }

    /// With enough distinct networks the diversity pass alone fills the
    /// response, so no two candidates sit behind the same router — the case
    /// the second pass exists to fall back from, not the common one.
    #[test]
    fn a_full_response_of_distinct_networks_needs_no_second_pass() {
        let registry = SwarmRegistry::default();
        for seed in 1..=40 {
            announce(&registry, seed, [10, 1, seed, 1], NOW + u64::from(seed));
        }

        let found = registry.lookup(&HASH, [0; 32], None, 0, NOW).unwrap();

        assert_eq!(found.len(), MAX_SWARM_CANDIDATES);
        let networks: HashSet<_> = found.iter().map(|lease| prefix(lease.address)).collect();
        assert_eq!(
            networks.len(),
            MAX_SWARM_CANDIDATES,
            "every candidate came from a different network"
        );
    }

    #[test]
    fn withdrawal_is_idempotent_and_removes_the_last_peer() {
        let registry = SwarmRegistry::default();
        assert_eq!(
            registry.withdraw(&[1; 19], [1; 32]),
            Err(SwarmError::InvalidInfoHash)
        );
        assert_eq!(registry.withdraw(&HASH, [1; 32]), Ok(()));
        announce(&registry, 1, [10, 0, 0, 1], NOW);
        assert_eq!(registry.withdraw(&HASH, [1; 32]), Ok(()));
        assert!(registry.is_empty());
    }
}
