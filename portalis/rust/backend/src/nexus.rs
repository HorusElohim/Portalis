//! Backend-owned entry point for the portable Nexus client.
//!
//! Keep this module below the Flutter bridge for now: protobuf envelopes and
//! transport handles are implementation details, while the eventual Dart API
//! should speak in collection/share operations.

use portalis_nexus_client::{NexusClient, TransportError};

/// Opens the supervised client used by the online collection workflow.
#[allow(dead_code)]
pub(crate) async fn connect(endpoint: &str) -> Result<NexusClient, TransportError> {
    NexusClient::connect(endpoint).await
}
