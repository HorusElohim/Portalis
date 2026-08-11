//! One de-duplicated peer set from every discovery path.

use std::collections::HashSet;
use std::net::IpAddr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CandidateSource {
    Direct,
    Nexus,
    Tracker,
    Dht,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerCandidate {
    pub address: IpAddr,
    pub port: u16,
    pub transport_capabilities: u32,
    pub source: CandidateSource,
}

/// Prefers direct and deterministic Nexus candidates while retaining unique
/// tracker and DHT endpoints as fallbacks.
#[must_use]
pub fn merge_candidates(candidates: impl IntoIterator<Item = PeerCandidate>) -> Vec<PeerCandidate> {
    let mut candidates: Vec<_> = candidates.into_iter().collect();
    candidates.sort_by_key(|candidate| candidate.source);
    let mut seen = HashSet::new();
    candidates.retain(|candidate| seen.insert((candidate.address, candidate.port)));
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_every_source_without_repeating_an_endpoint() {
        let address = "192.0.2.1".parse().expect("address");
        let merged = merge_candidates([
            PeerCandidate {
                address,
                port: 6881,
                transport_capabilities: 1,
                source: CandidateSource::Dht,
            },
            PeerCandidate {
                address,
                port: 6881,
                transport_capabilities: 1,
                source: CandidateSource::Nexus,
            },
            PeerCandidate {
                address: "198.51.100.2".parse().expect("address"),
                port: 6881,
                transport_capabilities: 1,
                source: CandidateSource::Tracker,
            },
        ]);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].source, CandidateSource::Nexus);
    }
}
