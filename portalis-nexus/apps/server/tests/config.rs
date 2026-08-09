use portalis_nexus_server::{DEFAULT_DATABASE, DEFAULT_LISTEN_ADDR, ServerConfig};

#[test]
fn a_missing_mongo_uri_reports_a_clear_startup_error() {
    let config = ServerConfig::from_listen_value(None).expect("the default address is valid");
    let error = config
        .require_mongodb_uri()
        .expect_err("a server without durable storage must not start");

    assert_eq!(config.listen_addr.to_string(), DEFAULT_LISTEN_ADDR);
    assert_eq!(config.database, DEFAULT_DATABASE);
    assert_eq!(error.to_string(), "PORTALIS_NEXUS_MONGODB_URI must be set");
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn a_configured_mongo_uri_is_available_to_the_server_process() {
    let config = ServerConfig {
        listen_addr: DEFAULT_LISTEN_ADDR.parse().expect("the default is valid"),
        mongodb_uri: Some("mongodb://nexus.example/".to_owned()),
        database: "nexus".to_owned(),
    };

    assert_eq!(config.require_mongodb_uri(), Ok("mongodb://nexus.example/"));
}
