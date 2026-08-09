//! Process configuration read at startup.

use std::fmt;
use std::net::{AddrParseError, SocketAddr};

pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8080";
/// The authority used when none is configured, matching local development.
pub const DEFAULT_SERVER_AUTHORITY: &str = "127.0.0.1:8080";

/// The database used when none is configured.
pub const DEFAULT_DATABASE: &str = "portalis_nexus";

/// The production server cannot run without durable identity storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingMongoUri;

impl fmt::Display for MissingMongoUri {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PORTALIS_NEXUS_MONGODB_URI must be set")
    }
}

impl std::error::Error for MissingMongoUri {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerConfig {
    pub listen_addr: SocketAddr,
    /// Where durable state lives. The server process refuses to start without
    /// this value; retaining the optional representation keeps configuration
    /// parsing independently testable.
    pub mongodb_uri: Option<String>,
    pub database: String,
}

impl ServerConfig {
    /// Parses the configured address or applies the local development default.
    ///
    /// # Errors
    ///
    /// Returns [`AddrParseError`] when the supplied address is malformed.
    pub fn from_listen_value(value: Option<&str>) -> Result<Self, AddrParseError> {
        let listen_addr = value.unwrap_or(DEFAULT_LISTEN_ADDR).parse()?;
        Ok(Self {
            listen_addr,
            mongodb_uri: None,
            database: DEFAULT_DATABASE.to_owned(),
        })
    }

    /// Reads the whole configuration from the environment.
    ///
    /// # Errors
    ///
    /// Returns [`AddrParseError`] when the listen address is malformed.
    pub fn from_environment() -> Result<Self, AddrParseError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Builds a configuration from an arbitrary variable lookup, so the
    /// precedence and defaulting rules can be tested without touching the
    /// real process environment, which every test would otherwise share.
    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, AddrParseError> {
        let listen = lookup("PORTALIS_NEXUS_LISTEN_ADDR");
        Ok(Self {
            mongodb_uri: lookup("PORTALIS_NEXUS_MONGODB_URI"),
            database: lookup("PORTALIS_NEXUS_DATABASE")
                .unwrap_or_else(|| DEFAULT_DATABASE.to_owned()),
            ..Self::from_listen_value(listen.as_deref())?
        })
    }

    /// Returns the durable store URI required by the server process.
    ///
    /// # Errors
    ///
    /// Returns [`MissingMongoUri`] when durable storage was not configured.
    pub fn require_mongodb_uri(&self) -> Result<&str, MissingMongoUri> {
        self.mongodb_uri.as_deref().ok_or(MissingMongoUri)
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

    #[test]
    fn leaves_the_database_uri_unset_when_not_configured() {
        let config = ServerConfig::from_listen_value(None).expect("valid default address");

        assert_eq!(config.mongodb_uri, None);
        assert_eq!(config.database, DEFAULT_DATABASE);
        assert_eq!(config.require_mongodb_uri(), Err(MissingMongoUri));
    }

    #[test]
    fn defaults_every_variable_that_is_unset() {
        let config = ServerConfig::from_lookup(|_name| None).expect("defaults are valid");

        assert_eq!(config.listen_addr.to_string(), DEFAULT_LISTEN_ADDR);
        assert_eq!(config.mongodb_uri, None);
        assert_eq!(config.database, DEFAULT_DATABASE);
        assert_eq!(config.require_mongodb_uri(), Err(MissingMongoUri));
    }

    #[test]
    fn reads_every_variable_that_is_set() {
        let set = [
            ("PORTALIS_NEXUS_LISTEN_ADDR", "0.0.0.0:9000"),
            ("PORTALIS_NEXUS_MONGODB_URI", "mongodb://example/"),
            ("PORTALIS_NEXUS_DATABASE", "custom_db"),
        ];
        let config = ServerConfig::from_lookup(|name| {
            set.iter()
                .find(|(variable, _)| *variable == name)
                .map(|(_, value)| (*value).to_owned())
        })
        .expect("every value is valid");

        assert_eq!(config.listen_addr.to_string(), "0.0.0.0:9000");
        assert_eq!(config.mongodb_uri.as_deref(), Some("mongodb://example/"));
        assert_eq!(config.database, "custom_db");
        assert_eq!(config.require_mongodb_uri(), Ok("mongodb://example/"));
    }

    /// The only path that touches the real process environment. Asserting it
    /// agrees with an equivalent explicit lookup keeps the test deterministic
    /// whatever the ambient environment happens to hold.
    #[test]
    fn reads_the_process_environment() {
        let expected = ServerConfig::from_lookup(|name| std::env::var(name).ok());

        assert_eq!(ServerConfig::from_environment(), expected);
    }

    #[test]
    fn rejects_an_invalid_address_from_the_environment() {
        let result = ServerConfig::from_lookup(|name| {
            (name == "PORTALIS_NEXUS_LISTEN_ADDR").then(|| "not-an-address".to_owned())
        });

        assert!(result.is_err());
    }
}
