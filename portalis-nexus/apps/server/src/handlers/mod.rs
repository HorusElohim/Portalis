//! Domain command handling, one module per subsystem.
//!
//! The socket knows how to move bytes and the session knows who a connection
//! is; neither knows what a command means. Dispatch routes a decoded envelope
//! to the module that owns it, so adding a subsystem adds a module here rather
//! than changing the transport.

use portalis_nexus_protocol::v1::Envelope;
use portalis_nexus_protocol::v1::envelope::Payload;

use crate::messages::response_for;
use crate::session::Session;
use crate::state::AppState;

pub(crate) mod friends;
pub(crate) mod identity;

/// Answers one decoded request on behalf of its connection.
pub async fn dispatch(
    session: &mut Session,
    state: &AppState,
    request: &Envelope,
    now_unix_ms: u64,
) -> Envelope {
    let authority = state.server_authority();
    match &request.payload {
        Some(Payload::RegisterUser(register)) => {
            identity::claim(
                session,
                state.identities(),
                authority,
                request,
                register,
                now_unix_ms,
            )
            .await
        }
        Some(Payload::AuthenticateDevice(authenticate)) => {
            identity::prove(
                session,
                state.identities(),
                authority,
                request,
                authenticate,
                now_unix_ms,
            )
            .await
        }
        Some(Payload::ResolveHandleRequest(lookup)) => {
            friends::resolve(session, state.friends(), request, lookup, now_unix_ms).await
        }
        Some(Payload::FriendCommand(command)) => {
            friends::command(session, state.friends(), request, command, now_unix_ms).await
        }
        Some(Payload::ListFriendsRequest(_)) => {
            friends::list(session, state.friends(), request, now_unix_ms).await
        }
        // Ping and anything this version does not accept yet.
        _ => response_for(request, now_unix_ms),
    }
}
