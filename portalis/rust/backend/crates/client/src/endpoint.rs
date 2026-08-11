//! Authenticated application connections without a Nexus-specific data layer.
//!
//! Iroh owns QUIC, TLS, direct paths, hole punching, and relay fallback. Nexus
//! supplies the existing device secret, accepted ALPNs, and remote address;
//! the caller owns every byte written to the returned QUIC connection.

use iroh::{
    Endpoint, NodeAddr, NodeId, RelayMode, SecretKey, Watcher as _,
    endpoint::{BindError, ConnectError, Connection, ConnectionType, Incoming},
};

type EndpointAddr = NodeAddr;
type EndpointId = NodeId;

/// The first Nexus application protocol carried by QUIC.
pub const NEXUS_ALPN: &[u8] = b"portalis/nexus/1";

/// The path Iroh currently has available to a remote device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionPath {
    /// No verified path is currently available.
    Unavailable,
    /// Traffic has a verified direct UDP path.
    Direct,
    /// Traffic currently uses an encrypted relay path.
    Relay,
    /// Direct and relay addresses exist, but the direct path is not confirmed.
    Mixed,
}

/// One device endpoint for both server and peer application connections.
#[derive(Debug, Clone)]
pub struct NexusEndpoint {
    inner: Endpoint,
}

impl NexusEndpoint {
    /// Binds an endpoint using the device's existing Ed25519 secret.
    ///
    /// `relay_mode` is explicit so production can use a Portalis relay while
    /// local tests and deployments can disable relays entirely.
    ///
    /// # Errors
    ///
    /// Returns an error when Iroh cannot bind the local sockets or configure
    /// the requested relay.
    pub async fn bind(
        device_secret: [u8; 32],
        alpns: Vec<Vec<u8>>,
        relay_mode: RelayMode,
    ) -> Result<Self, BindError> {
        let inner = Endpoint::builder()
            .clear_discovery()
            .relay_mode(relay_mode)
            .secret_key(SecretKey::from_bytes(&device_secret))
            .alpns(alpns)
            .bind()
            .await?;
        Ok(Self { inner })
    }

    /// The public connection identity derived from the supplied device secret.
    #[must_use]
    pub fn id(&self) -> EndpointId {
        self.inner.node_id()
    }

    /// Current direct and relay addressing information to publish for peers.
    #[must_use]
    pub fn addr(&self) -> EndpointAddr {
        self.inner
            .node_addr()
            .get()
            .unwrap_or_else(|| EndpointAddr::new(self.id()))
    }

    /// Connects to a known application peer and returns its raw QUIC connection.
    ///
    /// # Errors
    ///
    /// Returns an error when no supplied address reaches the authenticated
    /// remote device or when QUIC, TLS, or ALPN negotiation fails.
    pub async fn connect(
        &self,
        remote: impl Into<EndpointAddr>,
        alpn: &[u8],
    ) -> Result<Connection, ConnectError> {
        self.inner.connect(remote, alpn).await
    }

    /// Waits for the next authenticated incoming connection attempt.
    pub async fn accept(&self) -> Option<Incoming> {
        self.inner.accept().await
    }

    /// Reports transport truth without turning it into application state.
    #[must_use]
    pub fn path_to(&self, remote: EndpointId) -> ConnectionPath {
        match self
            .inner
            .conn_type(remote)
            .map(|mut watcher| watcher.get())
        {
            Some(ConnectionType::Direct(_)) => ConnectionPath::Direct,
            Some(ConnectionType::Relay(_)) => ConnectionPath::Relay,
            Some(ConnectionType::Mixed(_, _)) => ConnectionPath::Mixed,
            Some(ConnectionType::None) | None => ConnectionPath::Unavailable,
        }
    }

    /// Notifies the endpoint after an OS network transition.
    pub async fn network_change(&self) {
        self.inner.network_change().await;
    }

    /// Closes all connections and releases the endpoint sockets.
    pub async fn close(&self) {
        self.inner.close().await;
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use ed25519_dalek::SigningKey;

    use super::*;

    #[tokio::test]
    async fn reuses_the_existing_device_identity() {
        let secret = [7_u8; 32];
        let endpoint = NexusEndpoint::bind(secret, vec![NEXUS_ALPN.to_vec()], RelayMode::Disabled)
            .await
            .expect("bind endpoint");

        assert_eq!(
            endpoint.id().as_bytes(),
            &SigningKey::from_bytes(&secret).verifying_key().to_bytes()
        );
        endpoint.close().await;
    }

    #[tokio::test]
    async fn returns_raw_streams_over_a_direct_connection() {
        let server = NexusEndpoint::bind([1; 32], vec![NEXUS_ALPN.to_vec()], RelayMode::Disabled)
            .await
            .expect("bind server");
        let client = NexusEndpoint::bind([2; 32], Vec::new(), RelayMode::Disabled)
            .await
            .expect("bind client");

        let mut address = EndpointAddr::new(server.id());
        let server_socket = server
            .inner
            .bound_sockets()
            .into_iter()
            .find(std::net::SocketAddr::is_ipv4)
            .expect("server IPv4 socket");
        address = address.with_direct_addresses([std::net::SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            server_socket.port(),
        )]);

        let accepting = tokio::spawn({
            let server = server.clone();
            async move {
                let incoming = server.accept().await.expect("incoming connection");
                let connection = incoming
                    .accept()
                    .expect("accept connection")
                    .await
                    .expect("authenticate connection");
                let (mut send, mut receive) = connection.accept_bi().await.expect("accept stream");
                let request = receive.read_to_end(16).await.expect("read request");
                send.write_all(&request).await.expect("write response");
                send.finish().expect("finish response");
                connection
            }
        });

        let connection = client
            .connect(address, NEXUS_ALPN)
            .await
            .expect("connect directly");
        let (mut send, mut receive) = connection.open_bi().await.expect("open stream");
        send.write_all(b"raw bytes").await.expect("write request");
        send.finish().expect("finish request");

        assert_eq!(
            receive.read_to_end(16).await.expect("read response"),
            b"raw bytes"
        );
        assert_eq!(client.path_to(server.id()), ConnectionPath::Direct);

        let _server_connection = accepting.await.expect("server task");
        client.close().await;
        server.close().await;
    }
}
