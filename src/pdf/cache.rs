use std::path::Path;
use tokio::sync::mpsc;

use anyhow::{Result, anyhow};
use iced::advanced::image;
use mupdf::{Colorspace, Device, Document, Matrix, Pixmap};
use tracing::{debug, error, info};

use crate::{DARK_THEME, LIGHT_THEME, geometry::Vector, pdf::inner::cpu_pdf_dark_mode_shader};

/// A unique identifier for a complete render request (e.g., for a specific view).
pub type RequestId = u64;

/// Tells the worker what to render. Sent from UI -> Worker.
#[derive(Debug)]
pub enum WorkerCommand {
    /// Request to render a new view.
    RenderTile(RenderRequest),
    /// Request to change the loaded document.
    LoadDocument(std::path::PathBuf),
    /// Sets the current page from which tiles could be rendered
    SetPage(i32),
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

#[derive(Debug, Clone)]
pub enum WorkerResponse {
    RenderedTile(CachedTile),
    Loaded(DocumentInfo),
    SetPage(PageInfo),
}

#[derive(Debug, Clone, Copy)]
pub struct DocumentInfo {
    pub page_count: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct PageInfo {
    pub idx: i32,
    pub size: Vector<f32>,
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

    pub fn load_document(&mut self, path: &Path) -> Result<DocumentInfo> {
        let doc = Document::open(path.to_str().unwrap())?;
        let out = DocumentInfo {
            page_count: doc.page_count()?,
        };
        self.document = Some(doc);
        self.current_page = None;
        self.current_page_idx = -1;
        Ok(out)
    }

    pub fn set_page(&mut self, idx: i32) -> Result<PageInfo> {
        if let Some(ref doc) = self.document {
            let page = doc.load_page(idx)?;
            let page_bounds = page.bounds()?;
            self.current_page = Some(page);
            self.current_page_idx = idx;
            Ok(PageInfo {
                idx,
                size: Vector {
                    x: page_bounds.width(),
                    y: page_bounds.height(),
                },
            })
        } else {
            Err(anyhow!("No document loaded"))
        }
    }

    pub fn render_tile(&mut self, req: RenderRequest) -> Result<CachedTile> {
        if let Some(ref page) = self.current_page {
            if self.current_page_idx != req.page_number {
                return Err(anyhow!(
                    "Page mismatch: worker has page {}, request {}",
                    self.current_page_idx,
                    req.page_number
                ));
            }
            // TODO: Look at current render_page to figure somethign out
            let mut matrix = Matrix::default();
            matrix.scale(req.scale, req.scale);
            let mut pixmap = Pixmap::new_with_rect(&Colorspace::device_rgb(), req.bounds, true)?;
            for samp in pixmap.samples_mut() {
                *samp = 255;
            }
            let device = Device::from_pixmap(&pixmap).unwrap();
            page.run(&device, &matrix).unwrap();
            let bg_color = if req.invert_colors {
                DARK_THEME
                    .extended_palette()
                    .background
                    .base
                    .color
                    .into_rgba8()
            } else {
                LIGHT_THEME
                    .extended_palette()
                    .background
                    .base
                    .color
                    .into_rgba8()
            };
            if req.invert_colors {
                cpu_pdf_dark_mode_shader(&mut pixmap, &bg_color);
            }
            let handle = image::Handle::from_rgba(
                pixmap.width(),
                pixmap.height(),
                pixmap.samples().to_vec(),
            );
            Ok(CachedTile {
                id: req.id,
                image_handle: handle,
                bounds: req.bounds,
            })
        } else {
            Err(anyhow!("No page set"))
        }
    }
}

pub async fn worker_main(
    mut command_rx: mpsc::UnboundedReceiver<WorkerCommand>,
    result_tx: mpsc::UnboundedSender<WorkerResponse>,
) {
    info!("Worker thread started");

    let mut worker = PdfWorker::new();

    while let Some(cmd) = command_rx.recv().await {
        debug!("{:?}", cmd);
        match cmd {
            WorkerCommand::RenderTile(req) => match worker.render_tile(req) {
                Ok(tile) => result_tx.send(WorkerResponse::RenderedTile(tile)).unwrap(),
                Err(e) => {
                    error!("{}", e);
                }
            },
            WorkerCommand::LoadDocument(path_buf) => match worker.load_document(&path_buf) {
                Ok(doc) => result_tx.send(WorkerResponse::Loaded(doc)).unwrap(),
                Err(_) => todo!(),
            },
            WorkerCommand::Shutdown => break,
            WorkerCommand::SetPage(idx) => match worker.set_page(idx) {
                Ok(page) => result_tx.send(WorkerResponse::SetPage(page)).unwrap(),
                Err(_) => todo!(),
            },
        }
    }

    info!("Worker thread shut down");
}
