use std::path::PathBuf;

use axum::{Json, Router, extract::State, routing::post};
use iced::{
    futures::{SinkExt, Stream, channel::mpsc::Sender},
    stream,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::app::AppMessage;

#[derive(Clone)]
struct AppState {
    tx: Sender<AppMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
enum RpcMessage {
    OpenFile { path: PathBuf },
    CloseFile { path: PathBuf },
    ToggleDarkModeUi,
    ToggleDarkModePdf,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
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

    "OK".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn json_parsing_of_rpc() {
        let input = r#"{"message": {"type": "OpenFile", "data": { "path": "./"}}}"#;
        let output: RpcRequest = serde_json::from_str(input).unwrap();
        assert!(
            output
                == RpcRequest {
                    message: RpcMessage::OpenFile { path: "./".into() }
                }
        );
    }
}
