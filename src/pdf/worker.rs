use std::path::PathBuf;
use tokio::sync::mpsc;

use anyhow::{Result, anyhow};
use iced::advanced::image;
use mupdf::{Colorspace, Device, Document, Matrix, Pixmap, Rect as MupdfRect};
use tracing::{error, info};

use crate::{DARK_THEME, LIGHT_THEME, geometry::Vector, pdf::inner::cpu_pdf_dark_mode_shader};
use super::text_extraction::{TextExtractor, TextSelection};
use super::link_extraction::{LinkExtractor, LinkInfo};
use super::outline_extraction::{OutlineExtractor, OutlineItem};

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
    /// Reloads the DocumentInfo for the active pdf
    RefreshFile,
    /// Extract text from a rectangular selection on the current page
    ExtractText(MupdfRect),
    /// Extract all links from the current page
    ExtractLinks,
    /// Extract document outline/table of contents
    ExtractOutline,
}

/// Requests sent from the ui thread to the worker thread
#[derive(Debug)]
pub struct RenderRequest {
    pub id: RequestId,
    pub page_number: i32,
    /// The bounds of a tile to be renderer. It's size and position is in screen pixels with respect to the pdf at scale 1.0
    pub bounds: mupdf::IRect,
    pub invert_colors: bool,
    /// The same pdf bounds can of course be up-/down-sample to many different resolutions
    /// depending on the viewport
    pub scale: f32,
    pub x: i32,
    pub y: i32,
    pub generation: usize,
}

#[derive(Debug, Clone)]
pub enum WorkerResponse {
    RenderedTile(CachedTile),
    Loaded(DocumentInfo),
    SetPage(PageInfo),
    Refreshed(PathBuf, DocumentInfo),
    ExtractedText(TextSelection),
    ExtractedLinks(Vec<LinkInfo>),
    ExtractedOutline(Vec<OutlineItem>),
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
    pub x: i32,
    pub y: i32,
    pub generation: usize,
}

/// Manages worker state
#[derive(Debug, Clone)]
pub struct PdfWorker {
    path: Option<PathBuf>,
    document: Option<mupdf::Document>,
    current_page: Option<mupdf::Page>,
    current_page_idx: i32,
}

impl PdfWorker {
    pub fn new() -> Self {
        Self {
            path: None,
            document: None,
            current_page: None,
            current_page_idx: -1,
        }
    }

    pub fn load_document(&mut self, path: PathBuf) -> Result<DocumentInfo> {
        let doc = Document::open(path.to_str().unwrap())?;
        let out = DocumentInfo {
            page_count: doc.page_count()?,
        };
        self.document = Some(doc);
        self.current_page = None;
        self.current_page_idx = -1;
        self.path = Some(path);
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

    pub fn extract_text(&self, selection_rect: MupdfRect) -> Result<TextSelection> {
        if let Some(ref page) = self.current_page {
            let extractor = TextExtractor::new(page);
            extractor.extract_text_in_rect(selection_rect)
        } else {
            Err(anyhow!("No page set"))
        }
    }

    pub fn extract_links(&self) -> Result<Vec<LinkInfo>> {
        if let Some(ref page) = self.current_page {
            let extractor = LinkExtractor::new(page);
            extractor.extract_all_links()
        } else {
            Err(anyhow!("No page set"))
        }
    }

    pub fn extract_outline(&self) -> Result<Vec<OutlineItem>> {
        if let Some(ref document) = self.document {
            let extractor = OutlineExtractor::new(document);
            extractor.extract_outline()
        } else {
            Err(anyhow!("No document loaded"))
        }
    }

    pub fn render_tile(&mut self, req: &RenderRequest) -> Result<CachedTile> {
        if let Some(ref page) = self.current_page {
            if self.current_page_idx != req.page_number {
                return Err(anyhow!(
                    "Page mismatch: worker has page {}, request {}",
                    self.current_page_idx,
                    req.page_number
                ));
            }
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
                // bounds: req.bounds,
                bounds: mupdf::IRect {
                    x0: (pixmap.width() as i32) * req.x,
                    y0: (pixmap.height() as i32) * req.y,
                    x1: (pixmap.width() as i32) * req.x + pixmap.width() as i32,
                    y1: (pixmap.height() as i32) * req.y + pixmap.height() as i32,
                },
                x: req.x,
                y: req.y,
                generation: req.generation,
            })
        } else {
            Err(anyhow!("No page set"))
        }
    }

    fn refresh_document(&mut self) -> Result<DocumentInfo> {
        if let Some(path) = &self.path {
            let doc = Document::open(path.to_str().unwrap())?;
            let out = DocumentInfo {
                page_count: doc.page_count()?,
            };
            self.document = Some(doc);
            Ok(out)
        } else {
            Err(anyhow!("No document set"))
        }
    }
}

pub async fn worker_main(
    mut command_rx: mpsc::UnboundedReceiver<WorkerCommand>,
    result_tx: mpsc::UnboundedSender<WorkerResponse>,
) {
    info!("Worker thread started");

    let mut worker = PdfWorker::new();
    let mut current_generation = 0usize;

    while let Some(cmd) = command_rx.recv().await {
        match cmd {
            WorkerCommand::RenderTile(req) => {
                if req.generation > current_generation {
                    current_generation = req.generation;
                }
                if req.generation == current_generation {
                    match worker.render_tile(&req) {
                        Ok(tile) => result_tx.send(WorkerResponse::RenderedTile(tile)).unwrap(),
                        Err(e) => {
                            error!("{}", e);
                        }
                    }
                }
            }
            WorkerCommand::LoadDocument(path_buf) => match worker.load_document(path_buf) {
                Ok(doc) => result_tx.send(WorkerResponse::Loaded(doc)).unwrap(),
                Err(e) => {
                    error!("{}", e);
                }
            },
            WorkerCommand::Shutdown => break,
            WorkerCommand::SetPage(idx) => match worker.set_page(idx) {
                Ok(page) => result_tx.send(WorkerResponse::SetPage(page)).unwrap(),
                Err(e) => {
                    error!("{}", e);
                }
            },
            WorkerCommand::RefreshFile => match (worker.path.clone(), worker.refresh_document()) {
                (Some(path), Ok(doc)) => result_tx
                    .send(WorkerResponse::Refreshed(path, doc))
                    .unwrap(),
                (_, Err(e)) => {
                    error!("{}", e)
                }
                _ => {
                    error!("Worker has no path")
                }
            },
            WorkerCommand::ExtractText(selection_rect) => match worker.extract_text(selection_rect) {
                Ok(text_selection) => result_tx
                    .send(WorkerResponse::ExtractedText(text_selection))
                    .unwrap(),
                Err(e) => {
                    error!("Text extraction failed: {}", e);
                }
            },
            WorkerCommand::ExtractLinks => match worker.extract_links() {
                Ok(links) => result_tx
                    .send(WorkerResponse::ExtractedLinks(links))
                    .unwrap(),
                Err(e) => {
                    error!("Link extraction failed: {}", e);
                }
            },
            WorkerCommand::ExtractOutline => match worker.extract_outline() {
                Ok(outline) => result_tx
                    .send(WorkerResponse::ExtractedOutline(outline))
                    .unwrap(),
                Err(e) => {
                    error!("Outline extraction failed: {}", e);
                }
            },
        }
    }

    info!("Worker thread shut down");
}
