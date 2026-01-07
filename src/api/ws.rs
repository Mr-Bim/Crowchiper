//! WebSocket API with JWT authentication.
//!
//! Provides a WebSocket endpoint that requires JWT authentication.
//! The auth token is read from the HTTP-only auth_token cookie.

use axum::{
    Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket, rejection::WebSocketUpgradeRejection},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use futures::{SinkExt, StreamExt};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

use crate::auth::{AUTH_COOKIE_NAME, get_cookie};
use crate::db::Database;
use crate::jwt::JwtConfig;

/// State for WebSocket endpoints.
#[derive(Clone)]
pub struct WsState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
}

/// Authenticated WebSocket user info.
#[derive(Debug, Clone, Serialize)]
pub struct WsUser {
    pub uuid: String,
    pub username: String,
}

/// Messages sent from server to client.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Connection established with user info
    Connected { user: WsUser },
    /// Ping to keep connection alive
    Ping,
    /// Error message
    Error { message: String },
}

pub fn router(state: WsState) -> Router {
    Router::new().route("/", get(ws_handler)).with_state(state)
}

async fn ws_handler(
    State(state): State<WsState>,
    headers: axum::http::HeaderMap,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    // Get auth token from HTTP-only cookie
    let token = match get_cookie(&headers, AUTH_COOKIE_NAME) {
        Some(token) => token,
        None => {
            return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response();
        }
    };

    // Validate JWT token
    let claims = match state.jwt.validate_token(token) {
        Ok(claims) => claims,
        Err(_) => {
            return (StatusCode::UNAUTHORIZED, "Invalid or expired token").into_response();
        }
    };

    // Look up user in database
    let user = match state.db.users().get_by_uuid(&claims.sub).await {
        Ok(Some(user)) if user.activated => user,
        Ok(Some(_)) => {
            return (StatusCode::FORBIDDEN, "Account not activated").into_response();
        }
        Ok(None) => {
            return (StatusCode::UNAUTHORIZED, "User not found").into_response();
        }
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // Check for WebSocket upgrade
    let ws = match ws {
        Ok(ws) => ws,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, "Expected WebSocket upgrade").into_response();
        }
    };

    let ws_user = WsUser {
        uuid: user.uuid,
        username: user.username,
    };

    // Upgrade to WebSocket
    ws.on_upgrade(move |socket| handle_socket(socket, ws_user))
}

async fn handle_socket(socket: WebSocket, user: WsUser) {
    let (mut sender, mut receiver) = socket.split();

    // Send connected message with user info
    let connected_msg = ServerMessage::Connected { user: user.clone() };
    if let Ok(json) = serde_json::to_string(&connected_msg) {
        if sender.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }

    // Create channel for sending messages
    let (tx, mut rx) = mpsc::channel::<ServerMessage>(32);

    // Spawn task to forward messages to WebSocket
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Spawn ping task to keep connection alive
    let tx_ping = tx.clone();
    let mut ping_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            if tx_ping.send(ServerMessage::Ping).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(_) => {
                    // Client messages can be handled here if needed
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for any task to complete (connection closed)
    tokio::select! {
        _ = &mut send_task => {},
        _ = &mut recv_task => {},
        _ = &mut ping_task => {},
    }

    // Clean up
    send_task.abort();
    recv_task.abort();
    ping_task.abort();

    info!("WebSocket disconnected for user: {}", user.username);
}
