//! Bounded exponential reconnect scheduling.

use std::time::Duration;

use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReconnectPolicy {
    initial_delay: Duration,
    maximum_delay: Duration,
    maximum_attempts: u32,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ReconnectPolicyError {
    #[error("initial reconnect delay must be at least one millisecond")]
    ZeroInitialDelay,
    #[error("maximum reconnect delay must be at least the initial delay")]
    MaximumBeforeInitial,
    #[error("maximum connection attempts must be at least one")]
    ZeroMaximumAttempts,
}

impl ReconnectPolicy {
    /// Creates a bounded exponential reconnect policy.
    ///
    /// # Errors
    ///
    /// Returns [`ReconnectPolicyError`] when the delay bounds or attempt limit
    /// are invalid.
    pub fn new(
        initial_delay: Duration,
        maximum_delay: Duration,
        maximum_attempts: u32,
    ) -> Result<Self, ReconnectPolicyError> {
        if initial_delay.as_millis() == 0 {
            return Err(ReconnectPolicyError::ZeroInitialDelay);
        }
        if maximum_delay < initial_delay {
            return Err(ReconnectPolicyError::MaximumBeforeInitial);
        }
        if maximum_attempts == 0 {
            return Err(ReconnectPolicyError::ZeroMaximumAttempts);
        }
        Ok(Self {
            initial_delay,
            maximum_delay,
            maximum_attempts,
        })
    }

    #[must_use]
    pub fn can_retry_after(&self, attempts: u32) -> bool {
        attempts < self.maximum_attempts
    }

    /// Returns the delay to wait after `failures` consecutive failures.
    ///
    /// The initial delay doubles per failure, `entropy` spreads the result
    /// across 80%-120% of that value, and the maximum delay caps the jittered
    /// result so a policy never sleeps longer than its configured bound.
    #[must_use]
    pub fn delay_after_failure(&self, failures: u32, entropy: u64) -> Duration {
        let exponent = failures.saturating_sub(1).min(63);
        let base_millis = self
            .initial_delay
            .as_millis()
            .saturating_mul(1_u128 << exponent);
        let jitter_percent = 80_u128 + u128::from(entropy % 41);
        let jittered_millis = base_millis.saturating_mul(jitter_percent) / 100;
        let capped_millis = jittered_millis.min(self.maximum_delay.as_millis());
        Duration::from_millis(u64::try_from(capped_millis).unwrap_or(u64::MAX))
    }
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self::new(Duration::from_millis(100), Duration::from_secs(5), 10)
            .expect("the default reconnect policy is valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_its_bounds() {
        assert_eq!(
            ReconnectPolicy::new(Duration::ZERO, Duration::from_millis(1), 1),
            Err(ReconnectPolicyError::ZeroInitialDelay)
        );
        assert_eq!(
            ReconnectPolicy::new(Duration::from_millis(2), Duration::from_millis(1), 1),
            Err(ReconnectPolicyError::MaximumBeforeInitial)
        );
        assert_eq!(
            ReconnectPolicy::new(Duration::from_millis(1), Duration::from_millis(1), 0),
            Err(ReconnectPolicyError::ZeroMaximumAttempts)
        );
    }

    #[test]
    fn caps_exponential_jitter_and_attempts() {
        let policy =
            ReconnectPolicy::new(Duration::from_millis(100), Duration::from_millis(500), 3)
                .expect("valid reconnect policy");

        assert_eq!(policy.delay_after_failure(1, 0), Duration::from_millis(80));
        assert_eq!(
            policy.delay_after_failure(2, 40),
            Duration::from_millis(240)
        );
        assert_eq!(policy.delay_after_failure(4, 0), Duration::from_millis(500));
        assert!(policy.can_retry_after(2));
        assert!(!policy.can_retry_after(3));
        assert!(ReconnectPolicy::default().can_retry_after(9));
    }
}
