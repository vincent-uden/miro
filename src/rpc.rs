use std::{fs, net::SocketAddr, path::PathBuf, time::Duration};

use async_watcher::{AsyncDebouncer, notify::RecursiveMode};
use axum::{Router, extract::State, routing::get};
use iced::{
    futures::{SinkExt, Stream, channel::mpsc::Sender},
    stream,
};
use tokio::sync::mpsc;
use tracing::error;

use crate::app::AppMessage;

#[derive(Clone)]
struct AppState {
    tx: Sender<AppMessage>,
}

#[derive(Debug, Clone, Copy)]
pub enum RpcMessage {}

pub fn rpc_server() -> impl Stream<Item = AppMessage> {
    stream::channel(100, |output| async move {
        let app = Router::new()
            .route("/", get(root_handler))
            .with_state(AppState { tx: output });
        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
        axum::serve(listener, app).await.unwrap();
    })
}

async fn root_handler(State(mut state): State<AppState>) -> String {
    let message = AppMessage::Debug("GET on /".into());
    if let Err(e) = state.tx.send(message).await {
        error!("Failed to send message: {}", e);
    }
    "Hello from Axum!".to_string()
}
