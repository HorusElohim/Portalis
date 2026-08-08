use futures_util::{SinkExt, StreamExt};
use portalis_nexus_protocol::v1::{Envelope, ServerHello};
use portalis_nexus_protocol::{MAX_FRAME_BYTES, WEBSOCKET_SUBPROTOCOL, decode_frame, encode_frame};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderValue, SEC_WEBSOCKET_PROTOCOL};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async_with_config};

use crate::{ClientError, ClientProtocol, validate_hello, validate_pong};

#[derive(Debug, Error)]
pub enum TransportError {
    #[error(transparent)]
    WebSocket(#[from] WebSocketError),
    #[error(transparent)]
    Frame(#[from] portalis_nexus_protocol::FrameError),
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error("server did not negotiate the {WEBSOCKET_SUBPROTOCOL} subprotocol")]
    MissingSubprotocol,
    #[error("connection closed before a response arrived")]
    ConnectionClosed,
    #[error("expected a binary protobuf response")]
    UnexpectedWebSocketMessage,
}

pub struct NexusClient {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    hello: ServerHello,
    protocol: ClientProtocol,
}

impl NexusClient {
    /// Connects to a Portalis Nexus WebSocket endpoint and validates its hello.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the WebSocket handshake, subprotocol, or
    /// protobuf hello is invalid.
    pub async fn connect(endpoint: &str) -> Result<Self, TransportError> {
        let mut request = endpoint.into_client_request()?;
        request.headers_mut().insert(
            SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static(WEBSOCKET_SUBPROTOCOL),
        );
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_FRAME_BYTES))
            .max_frame_size(Some(MAX_FRAME_BYTES));
        let (mut socket, response) =
            connect_async_with_config(request, Some(config), false).await?;
        let uses_expected_subprotocol = response
            .headers()
            .get(SEC_WEBSOCKET_PROTOCOL)
            .is_some_and(|value| value.as_bytes() == WEBSOCKET_SUBPROTOCOL.as_bytes());
        if !uses_expected_subprotocol {
            return Err(TransportError::MissingSubprotocol);
        }
        let hello = validate_hello(receive_envelope(&mut socket).await?)?;

        Ok(Self {
            socket,
            hello,
            protocol: ClientProtocol::default(),
        })
    }

    #[must_use]
    pub fn hello(&self) -> &ServerHello {
        &self.hello
    }

    /// Sends a ping and verifies the correlated pong response.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when sending or validating the response fails.
    pub async fn ping(
        &mut self,
        nonce: u64,
        sent_at_unix_ms: u64,
    ) -> Result<Envelope, TransportError> {
        let request = self.protocol.ping(nonce, sent_at_unix_ms);
        let frame = encode_frame(&request)?;
        self.socket.send(Message::Binary(frame.into())).await?;
        let response = receive_envelope(&mut self.socket).await?;
        validate_pong(&request, &response)?;
        Ok(response)
    }
}

async fn receive_envelope(
    socket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> Result<Envelope, TransportError> {
    let Some(message) = socket.next().await else {
        return Err(TransportError::ConnectionClosed);
    };
    match message? {
        Message::Binary(frame) => Ok(decode_frame(&frame)?),
        Message::Close(_) => Err(TransportError::ConnectionClosed),
        _ => Err(TransportError::UnexpectedWebSocketMessage),
    }
}
