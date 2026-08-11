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
pub(crate) mod presence;
pub(crate) mod shares;
pub(crate) mod snapshots;
pub(crate) mod swarm;

/// Answers one decoded request on behalf of its connection.
pub async fn dispatch(
    session: &mut Session,
    state: &AppState,
    request: &Envelope,
    now_unix_ns: u64,
) -> Envelope {
    let authority = state.server_authority();
    match &request.payload {
        Some(Payload::RegisterUser(register)) => {
            let reply = identity::claim(
                session,
                state.identities(),
                authority,
                request,
                register,
                now_unix_ns,
            )
            .await;
            arrived(session, state, now_unix_ns).await;
            reply
        }
        Some(Payload::AuthenticateDevice(authenticate)) => {
            let reply = identity::prove(
                session,
                state.identities(),
                authority,
                request,
                authenticate,
                now_unix_ns,
            )
            .await;
            arrived(session, state, now_unix_ns).await;
            reply
        }
        Some(Payload::LinkDevice(link_device)) => {
            identity::link(
                session,
                state.identities(),
                authority,
                request,
                link_device,
                now_unix_ns,
            )
            .await
        }
        Some(Payload::ResolveHandleRequest(lookup)) => {
            friends::resolve(session, state.friends(), request, lookup, now_unix_ns).await
        }
        Some(Payload::FriendCommand(command)) => {
            friends::command(session, state.friends(), request, command, now_unix_ns).await
        }
        Some(Payload::ListFriendsRequest(_)) => {
            friends::list(session, state.friends(), request, now_unix_ns).await
        }
        Some(Payload::PutKeyEnvelope(put)) => {
            shares::put(session, state.envelopes(), request, put, now_unix_ns).await
        }
        Some(Payload::ListKeyEnvelopesRequest(list)) => {
            shares::list(session, state.envelopes(), request, list, now_unix_ns).await
        }
        Some(Payload::PublishShare(command)) => {
            snapshots::publish(session, state, request, command, now_unix_ns).await
        }
        Some(Payload::ListSharesRequest(_)) => {
            snapshots::list(session, state, request, now_unix_ns).await
        }
        Some(Payload::FetchShareRequest(fetch)) => {
            snapshots::fetch(session, state, request, fetch, now_unix_ns).await
        }
        Some(Payload::RevokeShareAccess(revoke)) => {
            snapshots::revoke(session, state, request, revoke, now_unix_ns).await
        }
        Some(Payload::GrantShareAccess(grant)) => {
            snapshots::grant(session, state, request, grant, now_unix_ns).await
        }
        Some(Payload::ShareHandoff(handoff)) => {
            snapshots::handoff(session, state, request, handoff, now_unix_ns).await
        }
        Some(Payload::AnnouncePeer(announce)) => {
            swarm::announce(session, state, request, announce, now_unix_ns)
        }
        Some(Payload::LookupPeersRequest(lookup)) => {
            swarm::lookup(session, state, request, lookup, now_unix_ns)
        }
        Some(Payload::WithdrawPeer(withdraw)) => {
            swarm::withdraw(session, state, request, withdraw, now_unix_ns)
        }
        // Ping and anything this version does not accept yet.
        _ => response_for(request, now_unix_ns),
    }
}

/// Counts a newly authenticated connection and shares the news.
///
/// Called after every identity command; a connection that did not become
/// authenticated, or was already counted, changes nothing.
async fn arrived(session: &Session, state: &AppState, now_unix_ns: u64) {
    let Some(identity) = session.identity() else {
        return;
    };
    let user = identity.user.user_id;
    let connection = session.connection_id();

    if state.presence().arrive(user, connection).is_some() {
        presence::announce(state, user, true, now_unix_ns).await;
    }
    // Told on every authentication, not only the first device, so each new
    // connection learns where its friends stand.
    presence::greet(state, user, connection, now_unix_ns).await;
}

/// Forgets a connection that has ended, telling friends if it was the last.
pub async fn departed(session: &Session, state: &AppState, now_unix_ns: u64) {
    let connection = session.connection_id();
    state.connections().forget(connection);
    state.swarm().remove_connection(connection);
    let Some(identity) = session.identity() else {
        return;
    };
    let user = identity.user.user_id;
    if state
        .presence()
        .depart(user, connection, now_unix_ns)
        .is_some()
    {
        presence::announce(state, user, false, now_unix_ns).await;
    }
}
