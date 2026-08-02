//! One pass over the difference between what is true here and what is true
//! elsewhere, run again and again.
//!
//! Every sync bug this project has had was a one-shot fired *by* an event: the
//! join that synced once, the tap that fetched once, the address learned by an
//! exchange and never re-checked. This loop acts on the difference instead, so
//! a pass that achieves nothing costs nothing and the next one tries again.
//!
//! See `docs/future-engine.md`.

use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Notify;

use crate::collab_sync::{
    forget_sync_peer, known_sync_peers, note_peer_result, sync_with, PREFERRED_SYNC_PORT,
    PRUNE_AFTER_FAILURES,
};
use crate::log::clog;

/// Long enough to be free on a phone, short enough that a collaborator's
/// addition shows up while you are still looking at the collection.
const INTERVAL: std::time::Duration = std::time::Duration::from_secs(45);

/// Whether anyone is looking. Told to us by the app's lifecycle rather than
/// inferred: this used to be a timestamp refreshed as a side effect of the UI
/// polling for collections, so the backend's network behaviour depended on how
/// often a screen happened to redraw.
static ACTIVE: AtomicBool = AtomicBool::new(true);

/// Wakes the loop before its next interval. A user who just joined should not
/// wait up to forty-five seconds to find out whether it worked.
static WAKE: Notify = Notify::const_new();

pub(crate) fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::Relaxed);
}

/// Converge as soon as possible — after a join, a fetch or a manual sync.
pub(crate) fn now() {
    WAKE.notify_one();
}

pub(crate) fn start() {
    clog!("converge", "every {INTERVAL:?} while in use, and on demand");
    tokio::spawn(async {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(INTERVAL) => {}
                _ = WAKE.notified() => {}
            }
            if ACTIVE.load(Ordering::Relaxed) {
                tick().await;
            }
        }
    });
}

/// One pass: bring every set's truth level with its peers, then chase whatever
/// is still wanted. Fetches come second because a sync is what teaches us where
/// a peer's content lives, which is exactly what a stalled fetch was missing.
async fn tick() {
    for (name, key) in sets() {
        reconcile(&name, &key).await;
    }
    crate::collections::pursue_fetches().await;
}

fn sets() -> Vec<(String, String)> {
    crate::collab_store::read_store(|collections| {
        Ok(collections
            .iter()
            .map(|c| (c.name.clone(), c.rendezvous_key().to_hex()))
            .collect())
    })
    .unwrap_or_default()
}

async fn reconcile(name: &str, key: &str) {
    let addresses = candidates(key);
    if addresses.is_empty() {
        return;
    }
    let round = attempt(key, &addresses).await;
    clog!("converge", "{name:?} — {}/{} reachable", round.reachable, addresses.len());
    round.prune(key);
}

/// Every remembered address, plus each of their hosts on the preferred port.
///
/// A saved address can be dead while the same device is very much alive on a
/// different one — remembered before the listener had a stable port, or from a
/// run that fell back to an ephemeral one. Without this, two devices that both
/// restarted could never find each other again without re-exchanging an invite.
fn candidates(key: &str) -> Vec<String> {
    let known = known_sync_peers(key);
    let mut all: std::collections::BTreeSet<String> = known.iter().cloned().collect();
    all.extend(known.iter().filter_map(|a| a.rsplit_once(':')).map(|(host, _)| {
        format!("{host}:{PREFERRED_SYNC_PORT}")
    }));
    all.into_iter().collect()
}

/// Contacts every candidate, not just the first that answers: two
/// collaborators can hold different entries, so stopping early leaves the rest
/// un-merged. Failures are expected and bounded by the connect timeout.
async fn attempt(key: &str, addresses: &[String]) -> Round {
    let mut round = Round::default();
    for address in addresses {
        let reached = sync_with(key, address).await.is_ok();
        round.record(address, reached, note_peer_result(key, address, reached));
    }
    round
}

#[derive(Default)]
struct Round {
    reachable: usize,
    stale: Vec<String>,
}

impl Round {
    fn record(&mut self, address: &str, reached: bool, failures: u32) {
        match reached {
            true => self.reachable += 1,
            false if failures >= PRUNE_AFTER_FAILURES => self.stale.push(address.to_string()),
            false => {}
        }
    }

    /// Forgetting only happens once something else is proven to work: if every
    /// address is failing the network is down, not the addresses wrong, and
    /// dropping them all would orphan the collection permanently.
    fn prune(self, key: &str) {
        if self.reachable == 0 {
            return;
        }
        for address in self.stale {
            forget_sync_peer(key, &address);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round_forgets_nothing_unless_something_else_worked() {
        let mut all_failing = Round::default();
        all_failing.record("a:1", false, PRUNE_AFTER_FAILURES);
        assert_eq!(all_failing.stale.len(), 1);
        assert_eq!(all_failing.reachable, 0);

        // Nothing reachable means the network is down, not the address wrong.
        // `prune` consuming self is what stops this being forgotten anyway.
        all_failing.prune("key");
        assert!(known_sync_peers("key").is_empty());
    }

    #[test]
    fn candidates_add_the_preferred_port_for_every_known_host() {
        let _temp = crate::paths::redirect_to_temp();
        let key = "d".repeat(64);
        crate::collab_sync::remember_sync_peers(&key, ["10.0.0.4:61638".to_string()]);

        let tried = candidates(&key);

        assert!(tried.contains(&"10.0.0.4:61638".to_string()));
        assert!(tried.contains(&format!("10.0.0.4:{PREFERRED_SYNC_PORT}")));
    }
}
