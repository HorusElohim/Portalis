//! The production clock and randomness behind the domain's ports.

use portalis_nexus_server_core::{Clock, RandomSource};

/// Wall-clock time in milliseconds since the Unix epoch.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_ms(&self) -> u64 {
        now_unix_ms()
    }
}

/// The operating system's cryptographically secure random source.
#[derive(Clone, Copy, Debug, Default)]
pub struct OsRandom;

impl RandomSource for OsRandom {
    fn fill(&self, buffer: &mut [u8]) {
        getrandom::fill(buffer).expect("the operating system random source is available");
    }
}

/// Reads the wall clock, saturating rather than panicking on a broken clock.
#[must_use]
pub fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(since_epoch.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_reads_a_plausible_time() {
        // Later than 2020 and before 2100, which is enough to prove it is
        // reading the real clock without pinning a moment.
        let now = SystemClock.now_unix_ms();

        assert!(now > 1_577_836_800_000, "{now} should be after 2020");
        assert!(now < 4_102_444_800_000, "{now} should be before 2100");
        assert!(now_unix_ms() >= now, "the clock must not run backwards");
    }

    #[test]
    fn randomness_fills_the_whole_buffer_and_varies() {
        let mut first = [0_u8; 32];
        let mut second = [0_u8; 32];

        OsRandom.fill(&mut first);
        OsRandom.fill(&mut second);

        assert_ne!(first, [0_u8; 32]);
        assert_ne!(first, second);
        // A zero-length request must not panic.
        OsRandom.fill(&mut []);
    }
}
