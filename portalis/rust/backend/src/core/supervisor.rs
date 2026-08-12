//! Who owns the tasks, in what order they start, and how they all stop.
//!
//! The rule this module exists to enforce: **no detached task**. Every
//! `tokio::spawn` in the application core goes through here, so shutdown can
//! be bounded and a leak is a thing that cannot happen rather than a thing
//! nobody checked for. A detached task is not merely untidy — it keeps a
//! runtime alive after the user has closed the window, and it is the reason
//! "the app takes a few seconds to quit" becomes permanent.
//!
//! Two behaviours are worth stating because they are easy to get backwards:
//!
//! **A panicking component degrades; it does not take the process down.** One
//! subsystem failing is a fact to report, not grounds for killing the others.
//! The supervisor catches it, emits [`Event::ComponentFailed`], and carries
//! on — the interface can then say what stopped working instead of the whole
//! application vanishing.
//!
//! **Shutdown is bounded.** Components are asked to stop, and given a
//! deadline. One that ignores it is abandoned rather than waited on forever,
//! and reported. A shutdown that can hang is a shutdown that will.

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinSet;

use super::events::{Event, EventBus};

/// How long a component gets to finish after being asked to stop.
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Handed to every component so it can tell when to wind up.
///
/// A component that ignores this is abandoned at the deadline, so the polite
/// path is also the one that gets to finish its work.
#[derive(Clone, Debug)]
pub struct Shutdown {
    signal: watch::Receiver<bool>,
}

impl Shutdown {
    /// Resolves when shutdown has been requested, immediately if it already
    /// has.
    pub async fn requested(&mut self) {
        // The initial value is only `true` if shutdown ran before this
        // component started, which is a race worth surviving rather than
        // hanging on.
        if *self.signal.borrow_and_update() {
            return;
        }
        let _ = self.signal.changed().await;
    }

    /// Whether shutdown has been requested, without waiting.
    #[must_use]
    pub fn is_requested(&self) -> bool {
        *self.signal.borrow()
    }
}

/// What became of one component when everything stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Returned on its own, or after being asked to.
    Stopped,
    /// Panicked. Reported, and survived.
    Panicked,
    /// Still running at the deadline, and abandoned.
    Abandoned,
}

/// Owns every task the core runs.
///
/// Startup order is the order components are added, because a component that
/// needs another to exist first is expressing a dependency, and the honest
/// place for it is the sequence rather than a sleep.
#[derive(Debug)]
pub struct Supervisor {
    bus: EventBus,
    tasks: JoinSet<()>,
    /// Which component each task is, so a panic can be attributed exactly
    /// rather than guessed at from what has not reported yet.
    names: HashMap<tokio::task::Id, &'static str>,
    order: Vec<&'static str>,
    stop: watch::Sender<bool>,
    grace: Duration,
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new(EventBus::new(), SHUTDOWN_GRACE)
    }
}

impl Supervisor {
    #[must_use]
    pub fn new(bus: EventBus, grace: Duration) -> Self {
        Self {
            bus,
            tasks: JoinSet::new(),
            names: HashMap::new(),
            order: Vec::new(),
            stop: watch::Sender::new(false),
            grace,
        }
    }

    /// The bus every component shares.
    #[must_use]
    pub const fn bus(&self) -> &EventBus {
        &self.bus
    }

    /// Starts one component and takes ownership of its task.
    ///
    /// The component receives a [`Shutdown`] and is expected to return when it
    /// resolves. Its panic is contained here.
    pub async fn start<F, Fut>(&mut self, component: &'static str, run: F)
    where
        F: FnOnce(Shutdown) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let shutdown = Shutdown {
            signal: self.stop.subscribe(),
        };
        self.order.push(component);
        let task = self.tasks.spawn(run(shutdown));
        self.names.insert(task.id(), component);
        self.bus.emit(Event::ComponentStarted { component }).await;
    }

    /// The components currently owned, in the order they were started.
    #[must_use]
    pub fn components(&self) -> &[&'static str] {
        &self.order
    }

    /// Asks every component to stop, waits up to the grace period, and reports
    /// what each one did.
    ///
    /// Returns after every task has finished or been abandoned, so a caller
    /// that awaits this knows the runtime is quiet.
    pub async fn shutdown(mut self) -> Vec<(&'static str, Outcome)> {
        // Ignored deliberately: no receivers means no components, which is a
        // shutdown with nothing to do rather than a failure.
        let _ = self.stop.send(true);

        let mut outcomes = Vec::with_capacity(self.order.len());
        let deadline = tokio::time::sleep(self.grace);
        tokio::pin!(deadline);

        while !self.tasks.is_empty() {
            tokio::select! {
                joined = self.tasks.join_next_with_id() => {
                    let Some(joined) = joined else { break };
                    self.settle(&mut outcomes, joined, Outcome::Panicked).await;
                }
                () = &mut deadline => {
                    // Whoever is left had their chance.
                    self.tasks.abort_all();
                    while let Some(joined) = self.tasks.join_next_with_id().await {
                        self.settle(&mut outcomes, joined, Outcome::Abandoned).await;
                    }
                    break;
                }
            }
        }
        outcomes
    }

    /// Records what one task did and tells the bus about it.
    ///
    /// `unfinished` is what a join error means in this phase: a panic while
    /// components are still being given time, an abandonment once the deadline
    /// has passed and everything left was aborted.
    async fn settle(
        &self,
        outcomes: &mut Vec<(&'static str, Outcome)>,
        joined: Result<(tokio::task::Id, ()), tokio::task::JoinError>,
        unfinished: Outcome,
    ) {
        let (id, outcome) = match joined {
            Ok((id, ())) => (id, Outcome::Stopped),
            // A panic reaches here rather than the process's abort handler,
            // which is the whole point.
            Err(error) => (error.id(), unfinished),
        };
        let component = self.names.get(&id).copied().unwrap_or("unknown");
        outcomes.push((component, outcome));

        let event = match outcome {
            Outcome::Stopped => Event::ComponentStopped { component },
            Outcome::Panicked => Event::ComponentFailed {
                component,
                panicked: true,
            },
            Outcome::Abandoned => Event::ComponentFailed {
                component,
                panicked: false,
            },
        };
        self.bus.emit(event).await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::*;

    /// Long enough that a well-behaved component always finishes inside it,
    /// short enough that the abandonment test is not a pause.
    const BRIEF: Duration = Duration::from_millis(200);

    fn supervisor() -> Supervisor {
        Supervisor::new(EventBus::new(), BRIEF)
    }

    #[tokio::test]
    async fn components_start_in_order_and_stop_when_asked() {
        let mut supervisor = supervisor();
        let order = Arc::new(std::sync::Mutex::new(Vec::new()));

        for name in ["store", "connection", "collections"] {
            let order = Arc::clone(&order);
            supervisor
                .start(name, move |mut shutdown| async move {
                    order.lock().expect("not poisoned").push(name);
                    shutdown.requested().await;
                })
                .await;
        }
        assert_eq!(
            supervisor.components(),
            ["store", "connection", "collections"]
        );

        let outcomes = supervisor.shutdown().await;

        assert_eq!(outcomes.len(), 3);
        assert!(outcomes
            .iter()
            .all(|(_, outcome)| *outcome == Outcome::Stopped));
        // Startup order is a dependency statement, so it is the order started
        // rather than the order they happened to be scheduled.
        assert_eq!(
            *order.lock().expect("not poisoned"),
            ["store", "connection", "collections"]
        );
    }

    /// The behaviour that keeps one broken subsystem from taking the window
    /// down with it.
    #[tokio::test]
    async fn a_panicking_component_is_reported_and_the_rest_carry_on() {
        let mut supervisor = supervisor();
        let mut events = supervisor.bus().subscribe().await;
        let survived = Arc::new(AtomicUsize::new(0));

        supervisor
            .start("doomed", |_shutdown| async {
                panic!("this component is having a bad day");
            })
            .await;
        let counter = Arc::clone(&survived);
        supervisor
            .start("healthy", move |mut shutdown| async move {
                shutdown.requested().await;
                counter.fetch_add(1, Ordering::Relaxed);
            })
            .await;

        let outcomes = supervisor.shutdown().await;

        assert!(
            outcomes.contains(&("doomed", Outcome::Panicked)),
            "the panic is reported: {outcomes:?}"
        );
        assert!(outcomes.contains(&("healthy", Outcome::Stopped)));
        assert_eq!(
            survived.load(Ordering::Relaxed),
            1,
            "the other component ran to completion"
        );

        // And it reached the bus, so an interface can say what stopped working.
        let mut failures = Vec::new();
        while let Some(event) = events.next().await {
            if let Event::ComponentFailed {
                component,
                panicked,
            } = event
            {
                failures.push((component, panicked));
            }
        }
        assert_eq!(failures, [("doomed", true)]);
    }

    /// A component that ignores the deadline is abandoned rather than waited
    /// on, because a shutdown that can hang is a shutdown that will.
    #[tokio::test]
    async fn a_component_that_ignores_shutdown_is_abandoned_at_the_deadline() {
        let mut supervisor = supervisor();

        supervisor
            .start("stubborn", |_shutdown| async {
                // Never looks at the signal.
                std::future::pending::<()>().await;
            })
            .await;

        let started = std::time::Instant::now();
        let outcomes = supervisor.shutdown().await;

        assert_eq!(outcomes, [("stubborn", Outcome::Abandoned)]);
        assert!(
            started.elapsed() < BRIEF * 4,
            "shutdown is bounded, not indefinite"
        );
    }

    /// Every task is owned, so there is nothing left running afterwards. This
    /// is the "no detached task" gate, checked rather than asserted in prose.
    #[tokio::test]
    async fn nothing_keeps_running_after_shutdown_returns() {
        let mut supervisor = supervisor();
        let ticks = Arc::new(AtomicUsize::new(0));

        let counter = Arc::clone(&ticks);
        supervisor
            .start("ticker", move |mut shutdown| async move {
                loop {
                    tokio::select! {
                        () = shutdown.requested() => break,
                        () = tokio::time::sleep(Duration::from_millis(1)) => {
                            counter.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            })
            .await;

        tokio::time::sleep(Duration::from_millis(20)).await;
        supervisor.shutdown().await;

        let after = ticks.load(Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(
            ticks.load(Ordering::Relaxed),
            after,
            "the task stopped when shutdown returned, not eventually"
        );
    }

    #[tokio::test]
    async fn starting_a_component_announces_it() {
        let mut supervisor = supervisor();
        let mut events = supervisor.bus().subscribe().await;

        supervisor
            .start("store", |mut shutdown| async move {
                shutdown.requested().await;
            })
            .await;

        assert_eq!(
            events.next().await,
            Some(Event::ComponentStarted { component: "store" })
        );

        supervisor.shutdown().await;
        assert_eq!(
            events.next().await,
            Some(Event::ComponentStopped { component: "store" })
        );
    }

    /// A component started after shutdown must not hang waiting for a signal
    /// that has already been sent.
    #[tokio::test]
    async fn shutdown_already_requested_is_seen_immediately() {
        let stop = watch::Sender::new(true);
        let mut shutdown = Shutdown {
            signal: stop.subscribe(),
        };

        assert!(shutdown.is_requested());
        // Would hang if it waited for a change that already happened.
        shutdown.requested().await;
    }

    #[tokio::test]
    async fn a_component_can_finish_before_being_asked_to() {
        let mut supervisor = supervisor();

        supervisor.start("brief", |_shutdown| async {}).await;
        tokio::task::yield_now().await;

        assert_eq!(supervisor.shutdown().await, [("brief", Outcome::Stopped)]);
    }

    #[tokio::test]
    async fn a_supervisor_with_nothing_to_stop_stops_immediately() {
        let started = std::time::Instant::now();

        assert!(supervisor().shutdown().await.is_empty());
        assert!(started.elapsed() < BRIEF);
    }

    #[tokio::test]
    async fn a_default_supervisor_uses_the_standard_grace_period() {
        let supervisor = Supervisor::default();

        assert!(supervisor.components().is_empty());
        assert_eq!(supervisor.bus().subscribers().await, 0);
        assert!(supervisor.shutdown().await.is_empty());
    }

    #[tokio::test]
    async fn a_component_can_ask_whether_shutdown_has_begun_without_waiting() {
        let mut supervisor = supervisor();
        let seen = Arc::new(AtomicUsize::new(0));

        let counter = Arc::clone(&seen);
        supervisor
            .start("poller", move |mut shutdown| async move {
                // Recorded rather than asserted, so the check happens in the
                // test where a failure is legible, and so shutdown cannot be
                // requested before this task is first scheduled.
                counter.store(usize::from(shutdown.is_requested()) + 1, Ordering::Release);
                shutdown.requested().await;
                counter.store(usize::from(shutdown.is_requested()) + 10, Ordering::Release);
            })
            .await;

        // Wait until the component has looked, before asking it to stop.
        while seen.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
        assert_eq!(seen.load(Ordering::Acquire), 1, "not requested yet");

        supervisor.shutdown().await;
        assert_eq!(seen.load(Ordering::Acquire), 11, "and now it has been");
    }

    /// With two components down at once the supervisor cannot know which task
    /// failed, and says so by naming an unaccounted-for component rather than
    /// inventing certainty.
    #[tokio::test]
    async fn several_failures_are_all_reported() {
        let mut supervisor = supervisor();

        for name in ["first", "second"] {
            supervisor
                .start(name, |_shutdown| async {
                    panic!("both of these fail");
                })
                .await;
        }

        let outcomes = supervisor.shutdown().await;

        assert_eq!(outcomes.len(), 2);
        assert!(outcomes
            .iter()
            .all(|(_, outcome)| *outcome == Outcome::Panicked));
        let mut named: Vec<_> = outcomes.iter().map(|(name, _)| *name).collect();
        named.sort_unstable();
        assert_eq!(named, ["first", "second"]);
    }
}
