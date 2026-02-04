//! WebSocket route handler.
//!
//! Handles WebSocket upgrade, message loop, and cleanup.

use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

use super::{
    connection::ConnectionManager,
    messages::{msg_types, system, ClientMessage, ServerMessage},
    playback_messages::*,
    playback_session::{PlaybackError, PlaybackSessionManager},
};
use crate::server::session::Session;
use crate::server::state::{GuardedConnectionManager, GuardedPlaybackSessionManager};

/// State needed for WebSocket handling (internal).
struct WsState {
    connection_manager: Arc<ConnectionManager>,
    playback_session_manager: Arc<PlaybackSessionManager>,
}

/// WebSocket upgrade handler.
///
/// This is the route handler for `GET /v1/ws`. It validates the session
/// and upgrades the connection to WebSocket.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    session: Session,
    State(connection_manager): State<GuardedConnectionManager>,
    State(playback_session_manager): State<GuardedPlaybackSessionManager>,
) -> Response {
    let state = Arc::new(WsState {
        connection_manager,
        playback_session_manager,
    });
    // Require device_id for WebSocket connections
    let device_id = match session.device_id {
        Some(id) => id,
        None => {
            warn!(
                "WebSocket connection attempt without device_id for user {}",
                session.user_id
            );
            // Return a 400 Bad Request - can't use WS without device tracking
            return Response::builder()
                .status(400)
                .body("Device ID required for WebSocket connection".into())
                .unwrap();
        }
    };

    let device_type = session
        .device_type
        .map(|dt| dt.as_str().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    debug!(
        "WebSocket upgrade for user {} device {} ({})",
        session.user_id, device_id, device_type
    );

    ws.on_upgrade(move |socket| {
        handle_socket(socket, session.user_id, device_id, device_type, state)
    })
}

/// Handle an established WebSocket connection.
async fn handle_socket(
    socket: WebSocket,
    user_id: usize,
    device_id: usize,
    device_type_str: String,
    state: Arc<WsState>,
) {
    debug!(
        "WebSocket connected: user {} device {} ({})",
        user_id, device_id, device_type_str
    );

    // Register connection and get receiver for outgoing messages
    let outgoing_rx = state
        .connection_manager
        .register(user_id, device_id, device_type_str.clone())
        .await;

    let (ws_sink, ws_stream) = socket.split();

    // Send connected message
    let connected_msg = ServerMessage::new(
        msg_types::CONNECTED,
        system::Connected {
            device_id,
            server_version: format!("{}-{}", env!("APP_VERSION"), env!("GIT_HASH")),
        },
    );

    // Spawn task to forward outgoing messages to WebSocket
    let outgoing_handle = tokio::spawn(forward_outgoing(ws_sink, outgoing_rx, connected_msg));

    // Process incoming messages
    process_incoming(ws_stream, user_id, device_id, &state).await;

    // Cleanup
    debug!(
        "WebSocket disconnected: user {} device {}",
        user_id, device_id
    );
    outgoing_handle.abort();

    // Parse device type for playback session manager
    let device_type = match device_type_str.as_str() {
        "web" => DeviceType::Web,
        "android" => DeviceType::Android,
        "ios" => DeviceType::Ios,
        _ => DeviceType::Web, // Default to web for unknown types
    };

    // Look up device name from playback session manager before disconnecting
    let device_name = state
        .playback_session_manager
        .get_device_name(user_id, device_id)
        .await
        .unwrap_or_else(|| "Unknown".to_string());

    // Notify playback session manager of disconnect
    state
        .playback_session_manager
        .handle_device_disconnect(user_id, device_id, &device_name, device_type)
        .await;

    state
        .connection_manager
        .unregister(user_id, device_id)
        .await;
}

/// Forward messages from the outgoing channel to the WebSocket.
async fn forward_outgoing(
    mut ws_sink: futures::stream::SplitSink<WebSocket, Message>,
    mut outgoing_rx: mpsc::Receiver<ServerMessage>,
    initial_msg: ServerMessage,
) {
    // Send initial connected message
    if let Ok(json) = serde_json::to_string(&initial_msg) {
        if ws_sink.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }

    // Forward all subsequent messages
    while let Some(msg) = outgoing_rx.recv().await {
        match serde_json::to_string(&msg) {
            Ok(json) => {
                if ws_sink.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            Err(e) => {
                error!("Failed to serialize WebSocket message: {}", e);
            }
        }
    }
}

/// Process incoming messages from the WebSocket.
async fn process_incoming(
    mut ws_stream: futures::stream::SplitStream<WebSocket>,
    user_id: usize,
    device_id: usize,
    state: &WsState,
) {
    while let Some(result) = ws_stream.next().await {
        match result {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(msg) => {
                        handle_client_message(user_id, device_id, msg, state).await;
                    }
                    Err(e) => {
                        debug!("Failed to parse client message: {}", e);
                        // Send error response
                        let error_msg = ServerMessage::new(
                            msg_types::ERROR,
                            system::Error::new(
                                "parse_error",
                                format!("Invalid message format: {}", e),
                            ),
                        );
                        let _ = state
                            .connection_manager
                            .send_to_device(user_id, device_id, error_msg)
                            .await;
                    }
                }
            }
            Ok(Message::Binary(_)) => {
                debug!("Received binary message, ignoring");
            }
            Ok(Message::Ping(_)) => {
                // Axum/tungstenite handles pong automatically
                debug!("Received ping");
            }
            Ok(Message::Pong(_)) => {
                debug!("Received pong");
            }
            Ok(Message::Close(_)) => {
                debug!("Received close frame");
                break;
            }
            Err(e) => {
                debug!("WebSocket error: {}", e);
                break;
            }
        }
    }
}

/// Handle a parsed client message.
async fn handle_client_message(
    user_id: usize,
    device_id: usize,
    msg: ClientMessage,
    state: &WsState,
) {
    match msg.msg_type.as_str() {
        msg_types::PING => {
            // Respond with pong
            let pong = ServerMessage::new(msg_types::PONG, system::Pong);
            let _ = state
                .connection_manager
                .send_to_device(user_id, device_id, pong)
                .await;
        }
        other => {
            // Check for feature-specific handlers based on prefix
            if other.starts_with("sync.") {
                // Future: dispatch to sync handler
                debug!("Received sync message: {}", other);
            } else if other.starts_with("playback.") {
                handle_playback_message(user_id, device_id, other, msg.payload, state).await;
            } else {
                debug!("Unknown message type: {}", other);
                let error_msg = ServerMessage::new(
                    msg_types::ERROR,
                    system::Error::new("unknown_type", format!("Unknown message type: {}", other)),
                );
                let _ = state
                    .connection_manager
                    .send_to_device(user_id, device_id, error_msg)
                    .await;
            }
        }
    }
}

/// Handle playback-related messages.
async fn handle_playback_message(
    user_id: usize,
    device_id: usize,
    msg_type: &str,
    payload: serde_json::Value,
    state: &WsState,
) {
    let manager = &state.playback_session_manager;

    let result: Result<(), PlaybackError> = match msg_type {
        msg_types::PLAYBACK_HELLO => match serde_json::from_value::<HelloPayload>(payload) {
            Ok(p) => {
                let welcome = manager
                    .handle_hello(user_id, device_id, p.device_name, p.device_type)
                    .await;
                let _ = state
                    .connection_manager
                    .send_to_device(
                        user_id,
                        device_id,
                        ServerMessage::new(msg_types::PLAYBACK_WELCOME, welcome),
                    )
                    .await;
                Ok(())
            }
            Err(e) => Err(PlaybackError::InvalidMessage(format!(
                "Invalid hello payload: {}",
                e
            ))),
        },

        msg_types::PLAYBACK_STATE => match serde_json::from_value::<PlaybackState>(payload) {
            Ok(state_payload) => {
                manager
                    .handle_state_update(user_id, device_id, state_payload)
                    .await
            }
            Err(e) => Err(PlaybackError::InvalidMessage(format!(
                "Invalid state payload: {}",
                e
            ))),
        },

        msg_types::PLAYBACK_QUEUE_UPDATE => {
            match serde_json::from_value::<QueueUpdatePayload>(payload) {
                Ok(p) => {
                    manager
                        .handle_queue_update(user_id, device_id, p.queue, p.queue_version)
                        .await
                }
                Err(e) => Err(PlaybackError::InvalidMessage(format!(
                    "Invalid queue_update payload: {}",
                    e
                ))),
            }
        }

        msg_types::PLAYBACK_COMMAND => {
            match serde_json::from_value::<PlaybackCommandPayload>(payload) {
                Ok(cmd) => {
                    manager
                        .handle_command(user_id, device_id, &cmd.command, cmd.payload)
                        .await
                }
                Err(e) => Err(PlaybackError::InvalidMessage(format!(
                    "Invalid command payload: {}",
                    e
                ))),
            }
        }

        msg_types::PLAYBACK_REQUEST_QUEUE => {
            match manager.handle_request_queue(user_id, device_id).await {
                Ok(sync) => {
                    let _ = state
                        .connection_manager
                        .send_to_device(
                            user_id,
                            device_id,
                            ServerMessage::new(msg_types::PLAYBACK_QUEUE_SYNC, sync),
                        )
                        .await;
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }

        msg_types::PLAYBACK_REGISTER_AUDIO_DEVICE => {
            let result = manager.register_audio_device(user_id, device_id).await;
            // Send register_ack regardless of success/failure
            let ack = match &result {
                Ok(()) => RegisterAckPayload {
                    success: true,
                    error: None,
                },
                Err(e) => RegisterAckPayload {
                    success: false,
                    error: Some(e.clone().into()),
                },
            };
            let _ = state
                .connection_manager
                .send_to_device(
                    user_id,
                    device_id,
                    ServerMessage::new(msg_types::PLAYBACK_REGISTER_ACK, ack),
                )
                .await;
            result
        }

        msg_types::PLAYBACK_UNREGISTER_AUDIO_DEVICE => {
            manager.unregister_audio_device(user_id, device_id).await
        }

        msg_types::PLAYBACK_TRANSFER_READY => {
            match serde_json::from_value::<TransferReadyPayload>(payload) {
                Ok(p) => {
                    manager
                        .handle_transfer_ready(user_id, device_id, p.transfer_id, p.state, p.queue)
                        .await
                }
                Err(e) => Err(PlaybackError::InvalidMessage(format!(
                    "Invalid transfer_ready payload: {}",
                    e
                ))),
            }
        }

        msg_types::PLAYBACK_TRANSFER_COMPLETE => {
            match serde_json::from_value::<TransferCompletePayload>(payload) {
                Ok(p) => {
                    manager
                        .handle_transfer_complete(user_id, device_id, p.transfer_id)
                        .await
                }
                Err(e) => Err(PlaybackError::InvalidMessage(format!(
                    "Invalid transfer_complete payload: {}",
                    e
                ))),
            }
        }

        msg_types::PLAYBACK_RECLAIM_AUDIO_DEVICE => {
            match serde_json::from_value::<PlaybackState>(payload) {
                Ok(state_payload) => {
                    manager
                        .handle_reclaim(user_id, device_id, state_payload)
                        .await
                }
                Err(e) => Err(PlaybackError::InvalidMessage(format!(
                    "Invalid reclaim payload: {}",
                    e
                ))),
            }
        }

        _ => {
            debug!("Unknown playback message type: {}", msg_type);
            Err(PlaybackError::InvalidMessage(format!(
                "Unknown playback message type: {}",
                msg_type
            )))
        }
    };

    if let Err(e) = result {
        let payload: PlaybackErrorPayload = e.into();
        let error_msg = ServerMessage::new(msg_types::PLAYBACK_ERROR, payload);
        let _ = state
            .connection_manager
            .send_to_device(user_id, device_id, error_msg)
            .await;
    }
}
