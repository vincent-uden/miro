use std::{fs, net::SocketAddr, path::PathBuf, time::Duration};

use async_watcher::{AsyncDebouncer, notify::RecursiveMode};
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use iced::{
    futures::{SinkExt, Stream, channel::mpsc::Sender},
    stream,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::error;

use crate::app::AppMessage;

#[derive(Clone)]
struct AppState {
    tx: Sender<AppMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum RpcMessage {
    OpenFile { path: PathBuf },
    CloseFile { path: PathBuf },
    ToggleDarkModeUi,
    ToggleDarkModePdf,
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    pub message: RpcMessage,
}

pub fn rpc_server() -> impl Stream<Item = AppMessage> {
    stream::channel(100, |output| async move {
        let app = Router::new()
            .route("/", post(root_handler))
            .with_state(AppState { tx: output });
        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
        axum::serve(listener, app).await.unwrap();
    })
}

async fn root_handler(
    State(mut state): State<AppState>,
    Json(payload): Json<RpcRequest>,
) -> String {
    let message = match payload.message {
        RpcMessage::OpenFile { path } => AppMessage::OpenFile(path),
        RpcMessage::CloseFile { path } => AppMessage::CloseFile(path),
        RpcMessage::ToggleDarkModeUi => AppMessage::ToggleDarkModeUi,
        RpcMessage::ToggleDarkModePdf => AppMessage::ToggleDarkModePdf,
    };

    if let Err(e) = state.tx.send(message).await {
        error!("Failed to send message: {}", e);
    }

    "Hello from Axum!".to_string()
}
