//! Step 5 — a core that starts, talks over a bus, survives a panic, and stops.
//!
//! Components do not call each other (D7). They emit facts and subscribe to
//! them, which is the only way the connection engine stays ignorant of
//! collections and the projection stays ignorant of everything but events.
//! The bus has to exist before the components do, or each one gets written
//! against direct calls and rewired later.
//!
//! Two guarantees are worth watching for below, because they pull in opposite
//! directions and a bus that gets them backwards is either lossy or deadlocked:
//!
//! - A **fact** is never dropped. If a subscriber falls behind, the emitter
//!   waits. Losing "revision published" means a collection silently never
//!   appears.
//! - A **sample** is never waited on. Transfer progress is coalesced per
//!   collection to the newest reading, because an older reading is not
//!   information once a newer one exists.
//!
//! And the lifecycle guarantee: a panicking component degrades rather than
//! taking the process with it, and nothing is still running once shutdown
//! returns.
//!
//! Run with `cargo run -p portalis-nexus-demo --bin 05-lifecycle`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use backend::core::events::{
    Connectivity, Event, EventBus, Handle, Path, PeerTrust, Progress, Security, Subject,
    VerifyFailure,
};
use backend::core::supervisor::{Outcome, Supervisor};

const COLLECTION: Handle = Handle(1);
const OTHER_COLLECTION: Handle = Handle(2);
const CONTACT: Handle = Handle(7);

#[tokio::main]
async fn main() {
    section("A core starts");
    let mut supervisor = Supervisor::new(EventBus::new(), Duration::from_millis(300));
    let mut watcher = supervisor.bus().subscribe().await;

    // A component that does real work and stops when asked.
    supervisor
        .start("connection", |mut shutdown| async move {
            shutdown.requested().await;
        })
        .await;
    // One that fails the moment it runs.
    supervisor
        .start("torrent", |_shutdown| async {
            panic!("the torrent engine could not open its port");
        })
        .await;
    // One that never looks at the shutdown signal.
    supervisor
        .start("stubborn", |_shutdown| async {
            std::future::pending::<()>().await;
        })
        .await;

    println!("  components, in the order they start:");
    for component in supervisor.components() {
        println!("    {component}");
    }
    println!("  Order is a dependency statement — the honest place for one,");
    println!("  rather than a sleep in whichever component loses the race.");

    facts_and_samples(supervisor.bus(), &mut watcher).await;

    section("What a subscriber actually received");
    let received = Arc::new(AtomicU64::new(0));
    let tally = Arc::clone(&received);
    let draining = tokio::spawn(async move {
        let mut kinds = Vec::new();
        while let Some(event) = watcher.next().await {
            tally.fetch_add(1, Ordering::Relaxed);
            kinds.push(name_of(&event));
        }
        kinds
    });

    section("A panicking component degrades, it does not kill the process");
    let outcomes = supervisor.shutdown().await;
    for (component, outcome) in &outcomes {
        let described = match outcome {
            Outcome::Stopped => "stopped when asked",
            Outcome::Panicked => "panicked — reported, and survived",
            Outcome::Abandoned => "ignored the deadline and was abandoned",
        };
        println!("  {component}: {described}");
    }
    println!("\n  The process is still here. One subsystem failing is a fact to");
    println!("  report, so the interface can say what stopped working, rather");
    println!("  than grounds for taking everything else down with it.");
    println!("  And shutdown is bounded: the stubborn component was abandoned");
    println!("  at the deadline rather than waited on forever.");

    let kinds = draining.await.expect("the drain task");
    println!(
        "\n  the subscriber received {} events, none dropped:",
        received.load(Ordering::Relaxed)
    );
    for kind in &kinds {
        println!("    {kind}");
    }

    section("Nothing is still running");
    println!("  `shutdown` returned, which means every task it owned has");
    println!("  finished or been abandoned. There is no detached task to leak,");
    println!("  because there is no way to spawn one outside the supervisor —");
    println!("  which is what keeps an app from taking seconds to quit.");

    assert!(
        outcomes.contains(&("torrent", Outcome::Panicked)),
        "the panic must be reported"
    );
    assert!(
        outcomes.contains(&("stubborn", Outcome::Abandoned)),
        "the deadline must be enforced"
    );
    println!("\nA core started, carried facts losslessly, coalesced samples,");
    println!("survived a panic, and stopped with nothing left running.");
}

/// The two guarantees, side by side: nothing durable is dropped, and nothing
/// is waited on for a sample.
async fn facts_and_samples(bus: &EventBus, watcher: &mut backend::core::events::Subscription) {
    section("Facts are never dropped");
    let facts = [
        Event::Connectivity(Connectivity::Online(Security {
            path: Path::Direct,
            peer: PeerTrust::Known,
        })),
        Event::PeerConnected {
            contact: CONTACT,
            security: Security {
                path: Path::Relayed,
                peer: PeerTrust::Unverified,
            },
        },
        Event::RevisionPublished {
            collection: COLLECTION,
            number: 3,
        },
        Event::ForkDetected {
            collection: COLLECTION,
            kept: [0xaa; 32],
            refused: [0xbb; 32],
        },
        Event::VerificationFailed {
            subject: Subject::Revision {
                collection: COLLECTION,
                number: 4,
            },
            reason: VerifyFailure::Rollback,
        },
    ];
    for fact in facts {
        bus.emit(fact).await;
    }
    println!("  {} facts emitted, all durable", facts.len());
    println!("  If a subscriber falls behind, the emitter waits. A fact that");
    println!("  can be dropped is a feature that silently does not happen.");

    section("Samples are never waited on");
    // Two collections downloading at once, many readings each.
    for done in 0..1_000_u64 {
        bus.emit(Event::TransferProgress {
            collection: COLLECTION,
            progress: sample(done),
        })
        .await;
        bus.emit(Event::TransferProgress {
            collection: OTHER_COLLECTION,
            progress: sample(done * 2),
        })
        .await;
    }

    let samples = watcher.samples();
    println!("  2 000 readings emitted, {} kept", samples.len());
    for (collection, progress) in [
        (COLLECTION, samples.get(&COLLECTION)),
        (OTHER_COLLECTION, samples.get(&OTHER_COLLECTION)),
    ] {
        if let Some(progress) = progress {
            println!(
                "    collection {} → {}/{} bytes",
                collection.0, progress.done, progress.total
            );
        }
    }
    println!("  Coalesced per collection, not globally: two transfers do not");
    println!("  overwrite one another. None of this went near the fact queue.");
}

fn sample(done: u64) -> Progress {
    Progress {
        done,
        total: 2_000,
        down_bytes_per_second: 1_024,
        up_bytes_per_second: 256,
        peers: 4,
    }
}

/// Names an event for display without printing what it carries.
fn name_of(event: &Event) -> &'static str {
    match event {
        Event::Connectivity(_) => "Connectivity",
        Event::PeerConnected { .. } => "PeerConnected",
        Event::PeerDisconnected { .. } => "PeerDisconnected",
        Event::RevisionPublished { .. } => "RevisionPublished",
        Event::RevisionReceived { .. } => "RevisionReceived",
        Event::EntryAvailable { .. } => "EntryAvailable",
        Event::MemberChanged { .. } => "MemberChanged",
        Event::TransferProgress { .. } => "TransferProgress",
        Event::TransferSettled { .. } => "TransferSettled",
        Event::VerificationFailed { .. } => "VerificationFailed",
        Event::ForkDetected { .. } => "ForkDetected",
        Event::DeviceRevoked { .. } => "DeviceRevoked",
        Event::ComponentStarted { .. } => "ComponentStarted",
        Event::ComponentStopped { .. } => "ComponentStopped",
        Event::ComponentFailed { .. } => "ComponentFailed",
        Event::CommandSettled { .. } => "CommandSettled",
    }
}

fn section(title: &str) {
    println!("\n{title}\n{}", "─".repeat(title.chars().count()));
}
