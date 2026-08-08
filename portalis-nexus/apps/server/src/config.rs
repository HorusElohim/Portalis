//! Process configuration read at startup.

use std::net::{AddrParseError, SocketAddr};

pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8080";
/// The authority used when none is configured, matching local development.
pub const DEFAULT_SERVER_AUTHORITY: &str = "127.0.0.1:8080";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerConfig {
    pub listen_addr: SocketAddr,
}

impl ServerConfig {
    /// Parses the configured address or applies the local development default.
    ///
    /// # Errors
    ///
    /// Returns [`AddrParseError`] when the supplied address is malformed.
    pub fn from_listen_value(value: Option<&str>) -> Result<Self, AddrParseError> {
        let listen_addr = value.unwrap_or(DEFAULT_LISTEN_ADDR).parse()?;
        Ok(Self { listen_addr })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_default_address() {
        let config = ServerConfig::from_listen_value(None).expect("valid default address");

        assert_eq!(config.listen_addr.to_string(), DEFAULT_LISTEN_ADDR);
    }

    #[test]
    fn accepts_custom_address() {
        let config =
            ServerConfig::from_listen_value(Some("0.0.0.0:9000")).expect("valid custom address");

        assert_eq!(config.listen_addr.to_string(), "0.0.0.0:9000");
    }

    #[test]
    fn rejects_invalid_address() {
        assert!(ServerConfig::from_listen_value(Some("not-an-address")).is_err());
    }
}
