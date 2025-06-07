use std::sync::mpsc;

use iced::advanced::image;
use tracing::info;

/// A unique identifier for a complete render request (e.g., for a specific view).
pub type RequestId = u64;

/// Tells the worker what to render. Sent from UI -> Worker.
#[derive(Debug)]
pub enum WorkerCommand {
    /// Request to render a new view.
    RenderTile(RenderRequest),
    /// Request to change the loaded document.
    LoadDocument(std::path::PathBuf),
    /// Refresh the current document (for file watching)
    RefreshDocument(std::path::PathBuf),
    /// Command to shut down the worker thread gracefully.
    Shutdown,
}

/// Requests sent from the ui thread to the worker thread
#[derive(Debug)]
pub struct RenderRequest {
    pub id: RequestId,
    pub page_number: i32,
    /// Bounds in pdf-coordinate space
    pub bounds: mupdf::IRect,
    pub invert_colors: bool,
    /// The same pdf bounds can of course be up-/down-sample to many different resolutions
    /// depending on the viewport
    pub scale: f32,
}

/// Responses from the worker thread to the ui thread
#[derive(Debug, Clone)]
pub struct CachedTile {
    pub id: RequestId,
    pub image_handle: image::Handle,
    pub bounds: mupdf::IRect,
}

pub fn worker_main(
    mut command_rx: mpsc::Receiver<WorkerCommand>,
    result_tx: mpsc::Sender<CachedTile>,
) {
    info!("Worker thread started");
}
