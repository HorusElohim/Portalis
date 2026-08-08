//! Domain command handling, one module per subsystem.
//!
//! The socket knows how to move bytes and the session knows who a connection
//! is; neither knows what a command means. Dispatch routes a decoded envelope
//! to the module that owns it, so adding a subsystem adds a module here rather
//! than changing the transport.

use portalis_nexus_protocol::v1::Envelope;
use portalis_nexus_protocol::v1::envelope::Payload;
use portalis_nexus_server_core::IdentityRepository;

use crate::identity::NexusIdentities;
use crate::messages::response_for;
use crate::session::Session;

pub(crate) mod identity;

/// Answers one decoded request on behalf of its connection.
pub async fn dispatch<S: IdentityRepository>(
    session: &mut Session,
    identities: &NexusIdentities<S>,
    server_authority: &str,
    request: &Envelope,
    now_unix_ms: u64,
) -> Envelope {
    match &request.payload {
        Some(Payload::RegisterUser(register)) => {
            identity::claim(
                session,
                identities,
                server_authority,
                request,
                register,
                now_unix_ms,
            )
            .await
        }
        Some(Payload::AuthenticateDevice(authenticate)) => {
            identity::prove(
                session,
                identities,
                server_authority,
                request,
                authenticate,
                now_unix_ms,
            )
            .await
        }
        // Ping and anything this version does not accept yet.
        _ => response_for(request, now_unix_ms),
    }
}
