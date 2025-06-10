use anyhow::Result;
use iced::widget::vertical_space;
use std::{collections::HashMap, path::PathBuf};
use tracing::debug;

use mupdf::{Document, Page};

use crate::geometry::Vector;

use super::{
    PdfMessage,
    cache::{
        CachedTile, DocumentInfo, PageInfo, RenderRequest, RequestId, WorkerCommand, WorkerResponse,
    },
    inner::{self, PageViewer},
};

#[derive(Debug)]
pub struct PdfViewer {
    pub name: String,
    pub path: PathBuf,
    pub label: String,
    pub cur_page_idx: i32,
    scale: f32,
    translation: Vector<f32>, // In document space
    pub invert_colors: bool,
    inner_state: inner::State,
    last_mouse_pos: Option<Vector<f32>>,
    panning: bool,
    command_tx: tokio::sync::mpsc::UnboundedSender<WorkerCommand>,
    document_info: Option<DocumentInfo>,
    page_info: Option<PageInfo>,
    tile_cache: HashMap<RequestId, CachedTile>,
}

impl PdfViewer {
    pub fn new(command_tx: tokio::sync::mpsc::UnboundedSender<WorkerCommand>) -> Self {
        Self {
            scale: 1.0,
            name: String::new(),
            path: PathBuf::new(),
            label: String::new(),
            cur_page_idx: 0,
            translation: Vector { x: 0.0, y: 0.0 },
            invert_colors: false,
            inner_state: inner::State::default(),
            last_mouse_pos: None,
            panning: false,
            command_tx,
            document_info: None,
            page_info: None,
            tile_cache: HashMap::new(),
        }
    }

    pub fn update(&mut self, message: PdfMessage) -> iced::Task<PdfMessage> {
        self.label = format!(
            "{} {}/{}",
            self.name,
            self.cur_page_idx + 1,
            self.document_info.map(|x| x.page_count).unwrap_or_default(),
        );
        match message {
            PdfMessage::OpenFile(path_buf) => self.load_file(path_buf).unwrap(),
            PdfMessage::RefreshFile => self.refresh_file().unwrap(),
            PdfMessage::NextPage => self.set_page(self.cur_page_idx + 1).unwrap(),
            PdfMessage::PreviousPage => self.set_page(self.cur_page_idx - 1).unwrap(),
            PdfMessage::ZoomIn => {
                self.scale *= 1.2;
            }
            PdfMessage::ZoomOut => {
                self.scale /= 1.2;
            }
            PdfMessage::ZoomHome => {
                self.scale = 1.0;
            }
            PdfMessage::ZoomFit => {
                self.scale = self.zoom_fit_ratio().unwrap_or(1.0);
            }
            PdfMessage::MoveHorizontal(delta) => {
                self.translation.x += delta / self.scale;
            }
            PdfMessage::MoveVertical(delta) => {
                self.translation.y += delta / self.scale;
            }
            PdfMessage::UpdateBounds(rectangle) => {
                self.inner_state.bounds = rectangle;
                self.tile_cache.clear();
                self.command_tx
                    .send(WorkerCommand::RenderTile(RenderRequest {
                        id: 0,
                        page_number: self.cur_page_idx,
                        bounds: self.visible_page_rect().unwrap(),
                        invert_colors: self.invert_colors,
                        scale: self.scale,
                    }))
                    .unwrap();
            }
            PdfMessage::None => {}
            PdfMessage::MouseMoved(vector) => {
                if self.inner_state.bounds.contains(vector) {
                    if self.panning && self.last_mouse_pos.is_some() {
                        self.translation +=
                            (self.last_mouse_pos.unwrap() - vector).scaled(1.0 / self.scale);
                    }
                    self.last_mouse_pos = Some(vector);
                } else {
                    self.last_mouse_pos = None;
                }
            }
            PdfMessage::MouseLeftDown => {
                self.panning = true;
            }
            PdfMessage::MouseRightDown => {}
            PdfMessage::MouseLeftUp => {
                self.panning = false;
            }
            PdfMessage::MouseRightUp => {}
            PdfMessage::WorkerResponse(worker_response) => match worker_response {
                WorkerResponse::RenderedTile(cached_tile) => {
                    self.tile_cache.insert(cached_tile.id, cached_tile);
                }
                WorkerResponse::Loaded(document_info) => {
                    debug!("LOADED");
                    self.document_info = Some(document_info);
                    self.set_page(0).unwrap();
                }
                WorkerResponse::SetPage(page_info) => {
                    self.page_info = Some(page_info);
                    // TODO: Render tiles
                    self.command_tx
                        .send(WorkerCommand::RenderTile(RenderRequest {
                            id: 0,
                            page_number: self.cur_page_idx,
                            bounds: self.visible_page_rect().unwrap(),
                            invert_colors: self.invert_colors,
                            scale: self.scale,
                        }))
                        .unwrap();
                }
            },
        }
        iced::Task::none()
    }

    pub fn view(&self) -> iced::Element<'_, PdfMessage> {
        // TODO: Show rendered tiles
        if self.tile_cache.is_empty() {
            vertical_space().into()
        } else {
            PageViewer::new(&self.tile_cache, &self.inner_state)
                .translation(self.translation)
                .scale(self.scale)
                .invert_colors(self.invert_colors)
                .into()
        }
    }

    fn set_page(&mut self, idx: i32) -> Result<()> {
        if let Some(doc) = self.document_info {
            self.command_tx.send(WorkerCommand::SetPage(idx))?;
            self.cur_page_idx = idx.clamp(0, doc.page_count);
        }
        Ok(())
    }

    fn load_file(&mut self, path: PathBuf) -> Result<()> {
        // TODO: Clear cache when it exists
        self.name = path
            .file_name()
            .expect("The pdf must have a file name")
            .to_string_lossy()
            .to_string();
        self.path = path.to_path_buf();
        self.command_tx.send(WorkerCommand::LoadDocument(path))?;
        Ok(())
    }

    fn refresh_file(&mut self) -> Result<()> {
        // TODO: Implement, maybe through a special command
        Ok(())
    }

    fn zoom_fit_ratio(&mut self) -> Result<f32> {
        if let Some(page) = &self.page_info {
            let page_size = page.size;
            let vertical_scale = self.inner_state.bounds.height() / page_size.y;
            let horizontal_scale = self.inner_state.bounds.width() / page_size.x;
            Ok(vertical_scale.min(horizontal_scale))
        } else {
            Ok(1.0)
        }
    }

    fn visible_page_rect(&self) -> Option<mupdf::IRect> {
        // TODO: This is returning a rect with no size
        if let Some(page) = self.page_info {
            let mut out_box = self.inner_state.bounds;
            out_box.translate(self.translation.scaled(self.scale));
            out_box.translate(Vector::new(
                -(self.inner_state.bounds.width() - page.size.x * self.scale) / 2.0,
                -(self.inner_state.bounds.height() - page.size.y * self.scale) / 2.0,
            ));
            Some(out_box.into())
        } else {
            None
        }
    }
}
