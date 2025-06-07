use std::{path::Path, sync::mpsc};

use anyhow::Result;
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

/// Manages worker state
#[derive(Debug, Clone)]
pub struct PdfWorker {
    document: Option<mupdf::Document>,
    current_page: Option<mupdf::Page>,
    current_page_idx: i32,
}

impl PdfWorker {
    pub fn new() -> Self {
        Self {
            document: None,
            current_page: None,
            current_page_idx: -1,
        }
    }

    pub fn load_document(&mut self, path: &Path) -> Result<()> {
        todo!()
    }

    pub fn refresh_document(&mut self, path: &Path) -> Result<()> {
        todo!()
    }

    pub fn render_tile(&mut self, req: RenderRequest) -> Result<CachedTile> {
        todo!()
    }
}

pub fn worker_main(
    mut command_rx: mpsc::Receiver<WorkerCommand>,
    result_tx: mpsc::Sender<CachedTile>,
) {
    info!("Worker thread started");

    let mut worker = PdfWorker::new();

    while let Ok(cmd) = command_rx.recv() {
        match cmd {
            WorkerCommand::RenderTile(req) => match worker.render_tile(req) {
                Ok(_) => {}
                Err(_) => todo!(),
            },
            WorkerCommand::LoadDocument(path_buf) => match worker.load_document(&path_buf) {
                Ok(_) => {}
                Err(_) => todo!(),
            },
            WorkerCommand::RefreshDocument(path_buf) => match worker.refresh_document(&path_buf) {
                Ok(_) => {}
                Err(_) => todo!(),
            },
            WorkerCommand::Shutdown => break,
        }
    }

    info!("Worker thread shut down");
}
