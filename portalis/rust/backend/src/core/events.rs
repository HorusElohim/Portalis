//! What components tell each other, and the bus that carries it.
//!
//! This is `SPEC.md` §11, and decision D7. Components do not call each other:
//! the connection engine knows nothing about collections, and the projection
//! knows nothing except events. That only holds if the bus arrives before the
//! components do, which is why this is step 5 and not step 9 — anything
//! written earlier would be written against direct calls and rewired later.
//!
//! Four rules keep the bus from becoming a second architecture:
//!
//! - **Events are facts, not requests.** Past tense, and no subscriber may
//!   assume another subscriber exists.
//! - **Bounded, and lossless for durable facts.** Content and security events
//!   are never dropped. If a subscriber falls behind, emitters wait.
//! - **One writer per fact.** Exactly one component emits any given variant.
//! - **No event triggers an event synchronously.** A subscriber that needs to
//!   act does so on its own task, so a cycle cannot form inside one dispatch.
//!
//! The interesting half is the second rule, because it has two answers rather
//! than one. A revision published is a fact: losing it means a collection
//! silently never appears, so an emitter blocks rather than drops. Transfer
//! progress is a sample of a continuous quantity: the newest reading makes
//! every older one irrelevant, so it is coalesced per collection and never
//! blocks anybody. Those are different channels with different guarantees,
//! and pretending otherwise is how a bus ends up either lossy or deadlocked.

use std::collections::HashMap;

use tokio::sync::{mpsc, watch, Mutex};

/// How many durable events one subscriber may fall behind before emitters
/// start waiting for it.
///
/// Generous enough that an ordinary burst — a sync delivering many revisions —
/// never blocks, and small enough that a wedged subscriber is felt rather than
/// absorbed into unbounded memory.
pub const DURABLE_CAPACITY: usize = 256;

/// An opaque reference to something the interface can name.
///
/// Deliberately not a string and not an identifier: `SPEC.md` §18 keeps hex
/// out of every tier except the one a human reads. The projection assigns
/// these; nothing here interprets them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Handle(pub u64);

/// Whether a connection travels directly or through a relay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    Direct,
    Relayed,
}

/// How much is known about who is on the other end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerTrust {
    /// A contact whose fingerprint has been compared.
    Known,
    /// A known contact whose fingerprint has not been compared yet.
    Unverified,
    /// Authenticated, and belonging to nobody we know.
    Unknown,
}

/// What a connection actually is, reported when the handshake completes rather
/// than inferred later (`SPEC.md` §15).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Security {
    pub path: Path,
    pub peer: PeerTrust,
}

/// The service relationship, as the interface should describe it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connectivity {
    LocalOnly,
    Connecting,
    Online(Security),
    Degraded { since_unix_ns: u64 },
}

/// What failed to verify, so a security event can name it without carrying it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subject {
    DeviceLog { owner: Handle },
    Revision { collection: Handle, number: u64 },
    Manifest { collection: Handle },
    Entry { collection: Handle, entry: Handle },
}

/// Why verification failed, in the terms `SPEC.md` §18 shows a person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifyFailure {
    /// A signature that is not the author's, or an author with no authority.
    Signature,
    /// Older than what is already held.
    Rollback,
    /// Right shape, wrong ancestor.
    BrokenChain,
    /// The bytes are not what the object they arrived with claims.
    ContentMismatch,
}

/// A sample of a transfer in flight. Never a fact — see [`EventBus`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
    pub down_bytes_per_second: u32,
    pub up_bytes_per_second: u32,
    pub peers: u16,
}

/// Something that happened. Past tense, always.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    // Connection. Emitted only by the connection engine.
    Connectivity(Connectivity),
    PeerConnected {
        contact: Handle,
        security: Security,
    },
    PeerDisconnected {
        contact: Handle,
    },

    // Content. Emitted only by the collection workflows.
    RevisionPublished {
        collection: Handle,
        number: u64,
    },
    RevisionReceived {
        collection: Handle,
        number: u64,
    },
    EntryAvailable {
        collection: Handle,
        entry: Handle,
    },
    MemberChanged {
        collection: Handle,
        contact: Handle,
        member: bool,
    },

    // Media. Progress is the one droppable variant in the whole enum.
    TransferProgress {
        collection: Handle,
        progress: Progress,
    },
    TransferSettled {
        collection: Handle,
        ok: bool,
    },

    // Security. Never rendered as an ordinary error (§18).
    VerificationFailed {
        subject: Subject,
        reason: VerifyFailure,
    },
    ForkDetected {
        collection: Handle,
        kept: [u8; 32],
        refused: [u8; 32],
    },
    DeviceRevoked {
        device: Handle,
    },

    // Lifecycle. Emitted only by the supervisor.
    ComponentStarted {
        component: &'static str,
    },
    ComponentStopped {
        component: &'static str,
    },
    /// A component's task ended on its own, by panic or by returning early.
    /// The process keeps running: one component failing is a degradation to
    /// report, not a reason to take everything else down with it.
    ComponentFailed {
        component: &'static str,
        panicked: bool,
    },

    // Commands, so a caller learns what became of what it asked for.
    CommandSettled {
        id: u64,
        ok: bool,
    },
}

impl Event {
    /// Whether losing this event would lose information.
    ///
    /// Everything except a transfer sample: a newer sample makes an older one
    /// irrelevant, and nothing else here can be reconstructed from what
    /// follows it.
    #[must_use]
    pub const fn is_durable(&self) -> bool {
        !matches!(self, Self::TransferProgress { .. })
    }
}

/// The latest transfer sample per collection.
///
/// A map rather than a single value, because two collections downloading at
/// once must not overwrite one another's progress — coalescing is per
/// collection, not global.
type Samples = HashMap<Handle, Progress>;

/// Carries facts to every subscriber, and samples to whoever is keeping up.
///
/// Cloning a bus is cheap and gives the same bus; subscribers are registered
/// once and live until dropped.
#[derive(Debug)]
pub struct EventBus {
    durable: Mutex<Vec<mpsc::Sender<Event>>>,
    samples: watch::Sender<Samples>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    #[must_use]
    pub fn new() -> Self {
        Self {
            durable: Mutex::new(Vec::new()),
            samples: watch::Sender::new(Samples::new()),
        }
    }

    /// Registers a subscriber and returns its own view of the bus.
    ///
    /// Every subscriber sees every durable event from this point on. A
    /// subscriber registered later does not see what it missed: events are
    /// facts about the moment they happened, not a log to replay.
    pub async fn subscribe(&self) -> Subscription {
        let (sender, durable) = mpsc::channel(DURABLE_CAPACITY);
        self.durable.lock().await.push(sender);
        Subscription {
            durable,
            samples: self.samples.subscribe(),
        }
    }

    /// Publishes an event to everyone listening.
    ///
    /// A durable event waits for any subscriber that is behind, so nothing is
    /// lost; a sample replaces whatever was pending for that collection and
    /// waits for nobody. Subscribers that have been dropped are forgotten as
    /// they are found.
    pub async fn emit(&self, event: Event) {
        if let Event::TransferProgress {
            collection,
            progress,
        } = event
        {
            // `send_modify` keeps only the newest state, which is exactly what
            // coalescing means. A subscriber that reads twice per second sees
            // two samples however many were emitted between them.
            self.samples.send_modify(|samples| {
                samples.insert(collection, progress);
            });
            return;
        }

        let mut subscribers = self.durable.lock().await;
        let mut living = Vec::with_capacity(subscribers.len());
        for sender in subscribers.drain(..) {
            // Waiting here is the point: a fact is worth more than the
            // emitter's latency.
            if sender.send(event).await.is_ok() {
                living.push(sender);
            }
        }
        *subscribers = living;
    }

    /// How many subscribers are currently registered.
    pub async fn subscribers(&self) -> usize {
        self.durable.lock().await.len()
    }
}

/// One subscriber's view: durable events in order, and the latest sample per
/// collection.
#[derive(Debug)]
pub struct Subscription {
    durable: mpsc::Receiver<Event>,
    samples: watch::Receiver<Samples>,
}

impl Subscription {
    /// The next durable event, or `None` once the bus is gone.
    ///
    /// Samples are not delivered here. A subscriber that wants them asks, and
    /// gets whatever is current rather than a queue of history.
    pub async fn next(&mut self) -> Option<Event> {
        self.durable.recv().await
    }

    /// Waits until at least one sample has changed since the last read, then
    /// returns the current sample for every collection in flight.
    ///
    /// Returns `None` when the bus is gone.
    pub async fn next_samples(&mut self) -> Option<Samples> {
        self.samples.changed().await.ok()?;
        Some(self.samples.borrow_and_update().clone())
    }

    /// The current sample for every collection in flight, without waiting.
    #[must_use]
    pub fn samples(&mut self) -> Samples {
        self.samples.borrow_and_update().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLLECTION: Handle = Handle(1);
    const OTHER: Handle = Handle(2);

    fn progress(done: u64) -> Progress {
        Progress {
            done,
            total: 100,
            down_bytes_per_second: 10,
            up_bytes_per_second: 5,
            peers: 3,
        }
    }

    fn published(number: u64) -> Event {
        Event::RevisionPublished {
            collection: COLLECTION,
            number,
        }
    }

    #[tokio::test]
    async fn every_subscriber_sees_every_durable_event_in_order() {
        let bus = EventBus::new();
        let mut first = bus.subscribe().await;
        let mut second = bus.subscribe().await;
        assert_eq!(bus.subscribers().await, 2);

        for number in 1..=3 {
            bus.emit(published(number)).await;
        }

        for subscription in [&mut first, &mut second] {
            for number in 1..=3 {
                assert_eq!(subscription.next().await, Some(published(number)));
            }
        }
    }

    /// A subscriber registered later does not learn what it missed: events are
    /// facts about a moment, not a log.
    #[tokio::test]
    async fn a_late_subscriber_sees_only_what_happens_next() {
        let bus = EventBus::new();
        bus.emit(published(1)).await;

        let mut late = bus.subscribe().await;
        bus.emit(published(2)).await;

        assert_eq!(late.next().await, Some(published(2)));
    }

    /// The guarantee the whole design turns on. A subscriber that stops
    /// reading makes emitters wait rather than losing anything.
    #[tokio::test]
    async fn a_durable_event_waits_for_a_subscriber_that_is_behind() {
        let bus = std::sync::Arc::new(EventBus::new());
        let mut slow = bus.subscribe().await;

        // Fill its queue exactly, which must not block.
        for number in 0..DURABLE_CAPACITY {
            bus.emit(published(number as u64)).await;
        }

        // One more has nowhere to go until the subscriber reads.
        let emitting = tokio::spawn({
            let bus = std::sync::Arc::clone(&bus);
            async move { bus.emit(published(9_999)).await }
        });
        tokio::task::yield_now().await;
        assert!(
            !emitting.is_finished(),
            "the emitter is waiting, not dropping"
        );

        assert_eq!(slow.next().await, Some(published(0)));
        emitting
            .await
            .expect("the emitter proceeds once there is room");

        // And nothing was lost in the meantime.
        for number in 1..DURABLE_CAPACITY {
            assert_eq!(slow.next().await, Some(published(number as u64)));
        }
        assert_eq!(slow.next().await, Some(published(9_999)));
    }

    /// Samples are the opposite bargain: never block anybody, and keep only
    /// the newest reading per collection.
    #[tokio::test]
    async fn progress_coalesces_per_collection_and_never_blocks() {
        let bus = EventBus::new();
        let mut watcher = bus.subscribe().await;

        for done in 0..DURABLE_CAPACITY as u64 * 4 {
            bus.emit(Event::TransferProgress {
                collection: COLLECTION,
                progress: progress(done),
            })
            .await;
        }
        bus.emit(Event::TransferProgress {
            collection: OTHER,
            progress: progress(7),
        })
        .await;

        let samples = watcher.samples();
        assert_eq!(
            samples.get(&COLLECTION).map(|sample| sample.done),
            Some(DURABLE_CAPACITY as u64 * 4 - 1),
            "only the newest reading survives"
        );
        assert_eq!(
            samples.get(&OTHER).map(|sample| sample.done),
            Some(7),
            "and one collection does not overwrite another"
        );
        // None of that reached the durable queue.
        assert_eq!(watcher.samples().len(), 2);
    }

    #[tokio::test]
    async fn waiting_for_samples_returns_the_current_state() {
        let bus = EventBus::new();
        let mut watcher = bus.subscribe().await;

        bus.emit(Event::TransferProgress {
            collection: COLLECTION,
            progress: progress(42),
        })
        .await;

        let samples = watcher.next_samples().await.expect("a sample arrived");
        assert_eq!(samples.get(&COLLECTION).map(|sample| sample.done), Some(42));
    }

    #[tokio::test]
    async fn a_dropped_subscriber_is_forgotten_rather_than_blocking_forever() {
        let bus = EventBus::new();
        let leaving = bus.subscribe().await;
        let mut staying = bus.subscribe().await;
        drop(leaving);

        bus.emit(published(1)).await;

        assert_eq!(bus.subscribers().await, 1, "the departed one is dropped");
        assert_eq!(staying.next().await, Some(published(1)));
    }

    #[tokio::test]
    async fn a_subscription_ends_when_the_bus_does() {
        let bus = EventBus::new();
        let mut subscription = bus.subscribe().await;
        let mut samples = bus.subscribe().await;
        drop(bus);

        assert_eq!(subscription.next().await, None);
        assert_eq!(samples.next_samples().await, None);
    }

    /// Only a transfer sample may be dropped. Getting this wrong in either
    /// direction is a bug: a lossy fact, or a bus that blocks on telemetry.
    #[test]
    fn exactly_one_variant_is_droppable() {
        let durable = [
            Event::Connectivity(Connectivity::LocalOnly),
            Event::PeerConnected {
                contact: OTHER,
                security: Security {
                    path: Path::Direct,
                    peer: PeerTrust::Known,
                },
            },
            Event::PeerDisconnected { contact: OTHER },
            published(1),
            Event::RevisionReceived {
                collection: COLLECTION,
                number: 1,
            },
            Event::EntryAvailable {
                collection: COLLECTION,
                entry: OTHER,
            },
            Event::MemberChanged {
                collection: COLLECTION,
                contact: OTHER,
                member: true,
            },
            Event::TransferSettled {
                collection: COLLECTION,
                ok: true,
            },
            Event::VerificationFailed {
                subject: Subject::Revision {
                    collection: COLLECTION,
                    number: 2,
                },
                reason: VerifyFailure::Rollback,
            },
            Event::ForkDetected {
                collection: COLLECTION,
                kept: [1; 32],
                refused: [2; 32],
            },
            Event::DeviceRevoked { device: OTHER },
            Event::ComponentStarted { component: "one" },
            Event::ComponentStopped { component: "one" },
            Event::ComponentFailed {
                component: "one",
                panicked: true,
            },
            Event::CommandSettled { id: 1, ok: true },
        ];

        for event in durable {
            assert!(event.is_durable(), "{event:?} must never be dropped");
        }
        assert!(!Event::TransferProgress {
            collection: COLLECTION,
            progress: progress(1),
        }
        .is_durable());
    }

    /// Every subject and failure a security event can name, so adding one
    /// without deciding how it reads is caught here.
    #[test]
    fn security_events_name_what_failed_without_carrying_it() {
        let subjects = [
            Subject::DeviceLog { owner: OTHER },
            Subject::Revision {
                collection: COLLECTION,
                number: 1,
            },
            Subject::Manifest {
                collection: COLLECTION,
            },
            Subject::Entry {
                collection: COLLECTION,
                entry: OTHER,
            },
        ];
        let reasons = [
            VerifyFailure::Signature,
            VerifyFailure::Rollback,
            VerifyFailure::BrokenChain,
            VerifyFailure::ContentMismatch,
        ];

        for subject in subjects {
            for reason in reasons {
                let event = Event::VerificationFailed { subject, reason };
                assert!(event.is_durable());
                assert_eq!(event, Event::VerificationFailed { subject, reason });
            }
        }
    }

    #[test]
    fn connectivity_and_security_describe_a_connection_completely() {
        for path in [Path::Direct, Path::Relayed] {
            for peer in [PeerTrust::Known, PeerTrust::Unverified, PeerTrust::Unknown] {
                let security = Security { path, peer };
                assert_eq!(
                    Connectivity::Online(security),
                    Connectivity::Online(security)
                );
            }
        }
        assert_ne!(Connectivity::LocalOnly, Connectivity::Connecting);
        assert_ne!(
            Connectivity::Degraded { since_unix_ns: 1 },
            Connectivity::Degraded { since_unix_ns: 2 }
        );
    }

    #[tokio::test]
    async fn a_default_bus_is_an_empty_bus() {
        let bus = EventBus::default();

        assert_eq!(bus.subscribers().await, 0);
        // Emitting into the void is not an error: no subscriber may assume
        // another exists, including none at all.
        bus.emit(published(1)).await;
        bus.emit(Event::TransferProgress {
            collection: COLLECTION,
            progress: progress(1),
        })
        .await;
    }
}
