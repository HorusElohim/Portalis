//! Tuning for one supervised client connection.

use std::time::Duration;

use crate::reconnect::ReconnectPolicy;

/// How long a command waits for its correlated response before failing.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientConfig {
    /// Governs the first connection attempt and every later reconnect.
    pub reconnect: ReconnectPolicy,
    /// How long one command waits for its correlated response.
    pub request_timeout: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            reconnect: ReconnectPolicy::default(),
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_the_protocol_request_timeout() {
        let config = ClientConfig::default();

        assert_eq!(config.request_timeout, DEFAULT_REQUEST_TIMEOUT);
        assert_eq!(config.reconnect, ReconnectPolicy::default());
    }
}
