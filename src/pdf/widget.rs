use anyhow::Result;
use num::Integer;
use std::{collections::HashMap, path::PathBuf};
use tracing::debug;

use crate::{RENDER_GENERATION, geometry::Vector};

use super::{
    PdfMessage,
    inner::{self, PageViewer},
    worker::{CachedTile, DocumentInfo, PageInfo, RenderRequest, WorkerCommand, WorkerResponse},
};

const TILE_CACHE_GRID_SIZE: i32 = 5;

#[derive(Debug)]
pub struct PdfViewer {
    pub name: String,
    pub path: PathBuf,
    pub label: String,
    pub cur_page_idx: i32,
    translation: Vector<f32>, // In document space
    pub invert_colors: bool,
    inner_state: inner::State,
    last_mouse_pos: Option<Vector<f32>>,
    panning: bool,
    command_tx: tokio::sync::mpsc::UnboundedSender<WorkerCommand>,
    document_info: Option<DocumentInfo>,
    page_info: Option<PageInfo>,
    shown_scale: f32,
    pending_scale: f32,
    pending_tile_cache: HashMap<(i32, i32), CachedTile>,
    shown_tile_cache: HashMap<(i32, i32), CachedTile>,
    current_center_tile: Vector<i32>,
    generation: usize,
}

impl PdfViewer {
    pub fn new(command_tx: tokio::sync::mpsc::UnboundedSender<WorkerCommand>) -> Self {
        assert!(
            TILE_CACHE_GRID_SIZE.is_odd(),
            "The tile cache grid must be of an odd size, is currently {TILE_CACHE_GRID_SIZE}"
        );
        Self {
            shown_scale: 1.0,
            pending_scale: 1.0,
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
            pending_tile_cache: HashMap::new(),
            shown_tile_cache: HashMap::new(),
            current_center_tile: Vector::zero(),
            generation: 0,
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
            PdfMessage::NextPage => self.set_page(self.cur_page_idx + 1).unwrap(),
            PdfMessage::PreviousPage => self.set_page(self.cur_page_idx - 1).unwrap(),
            PdfMessage::SetPage(page) => self.set_page(page).unwrap(),
            PdfMessage::ZoomIn => {
                self.pending_scale *= 1.2;
                self.invalidate_cache();
            }
            PdfMessage::ZoomOut => {
                self.pending_scale /= 1.2;
                self.invalidate_cache();
            }
            PdfMessage::ZoomHome => {
                self.pending_scale = 1.0;
                self.invalidate_cache();
            }
            PdfMessage::ZoomFit => {
                self.pending_scale = self.zoom_fit_ratio().unwrap_or(1.0);
                self.invalidate_cache();
            }
            PdfMessage::MoveHorizontal(delta) => {
                self.translation.x += delta / self.shown_scale;
            }
            PdfMessage::MoveVertical(delta) => {
                self.translation.y += delta / self.shown_scale;
            }
            PdfMessage::UpdateBounds(rectangle) => {
                // TODO: The amount of wl_registrys that appear scale with the amount of resizing
                // of the window that is done
                self.inner_state.bounds = rectangle;
            }
            PdfMessage::None => {}
            PdfMessage::MouseMoved(vector) => {
                if self.inner_state.bounds.contains(vector) {
                    if self.panning && self.last_mouse_pos.is_some() {
                        self.translation +=
                            (self.last_mouse_pos.unwrap() - vector).scaled(1.0 / self.shown_scale);
                        self.invalidate_cache();
                    }
                    self.last_mouse_pos = Some(vector);
                } else {
                    self.last_mouse_pos = None;
                }
            }
            PdfMessage::MouseLeftDown => {
                // Dont start panning if we're close enough to the edge that a pane resizing might
                // happen
                if let Some(mp) = self.last_mouse_pos {
                    let mut padded_bounds = self.inner_state.bounds;
                    padded_bounds.x0 += Vector { x: 11.0, y: 11.0 };
                    padded_bounds.x1 -= Vector { x: 11.0, y: 11.0 };
                    if padded_bounds.contains(mp) {
                        self.panning = true;
                    }
                }
            }
            PdfMessage::MouseRightDown => {}
            PdfMessage::MouseLeftUp => {
                self.panning = false;
            }
            PdfMessage::MouseRightUp => {}
            PdfMessage::WorkerResponse(worker_response) => match worker_response {
                WorkerResponse::RenderedTile(cached_tile) => {
                    self.pending_tile_cache
                        .insert((cached_tile.x, cached_tile.y), cached_tile);
                    if self.pending_tile_cache.len() == TILE_CACHE_GRID_SIZE.pow(2) as usize {
                        std::mem::swap(&mut self.pending_tile_cache, &mut self.shown_tile_cache);
                        self.pending_tile_cache.clear();
                        self.shown_scale = self.pending_scale;
                    }
                }
                WorkerResponse::Loaded(document_info) => {
                    self.pending_tile_cache.clear();
                    self.shown_tile_cache.clear();
                    self.document_info = Some(document_info);
                    self.set_page(0).unwrap();
                }
                WorkerResponse::SetPage(page_info) => {
                    self.page_info = Some(page_info);
                    if self.inner_state.bounds.size() != Vector::zero() {
                        self.force_invalidate_cache();
                    }
                }
                WorkerResponse::Refreshed(_, document_info) => {
                    self.document_info = Some(document_info);
                    self.refresh_file().unwrap();
                }
            },
        }
        iced::Task::none()
    }

    pub fn view(&self) -> iced::Element<'_, PdfMessage> {
        PageViewer::new(&self.shown_tile_cache, &self.inner_state)
            .translation(self.translation)
            .scale(self.shown_scale)
            .invert_colors(self.invert_colors)
            .into()
    }

    fn set_page(&mut self, idx: i32) -> Result<()> {
        if let Some(doc) = self.document_info {
            self.cur_page_idx = idx.clamp(0, doc.page_count - 1);
            self.command_tx
                .send(WorkerCommand::SetPage(self.cur_page_idx))?;
        }
        Ok(())
    }

    fn load_file(&mut self, path: PathBuf) -> Result<()> {
        self.name = path
            .file_name()
            .expect("The pdf must have a file name")
            .to_string_lossy()
            .to_string();
        self.path = path.to_path_buf();
        self.command_tx.send(WorkerCommand::LoadDocument(path))?;
        Ok(())
    }

    pub fn force_invalidate_cache(&mut self) {
        let middle_of_screen = self.translation.scaled(self.pending_scale);
        let tile_size = self.tile_bounds(Vector::zero()).unwrap();

        let viewport_tile_coord = Vector {
            x: (middle_of_screen.x / tile_size.width() as f32).round() as i32,
            y: (middle_of_screen.y / tile_size.height() as f32).round() as i32,
        };

        self.increment_generation();
        self.pending_tile_cache.clear();
        self.current_center_tile = viewport_tile_coord;
        self.populate_cache();
    }

    fn increment_generation(&mut self) {
        let gen_mtx = RENDER_GENERATION.get().unwrap();
        let mut generation = gen_mtx.blocking_lock();
        *generation += 1;
        self.generation = *generation;
    }

    fn invalidate_cache(&mut self) {
        let middle_of_screen = self.translation.scaled(self.pending_scale);
        let tile_size = self.tile_bounds(Vector::zero()).unwrap();

        let viewport_tile_coord = Vector {
            x: (middle_of_screen.x / tile_size.width() as f32).round() as i32,
            y: (middle_of_screen.y / tile_size.height() as f32).round() as i32,
        };

        if viewport_tile_coord != self.current_center_tile || self.pending_scale != self.shown_scale
        {
            if self.pending_scale != self.shown_scale {
                self.increment_generation();
                self.pending_tile_cache.clear();
            } else {
                self.pending_tile_cache = self.shown_tile_cache.clone();
                self.pending_tile_cache.retain(|_, v| {
                    (v.x - viewport_tile_coord.x).abs() <= 1
                        && (v.y - viewport_tile_coord.y).abs() <= 1
                });
            }
            self.current_center_tile = viewport_tile_coord;
            self.populate_cache();
        }
    }

    fn populate_cache(&mut self) {
        let half_grid_size = TILE_CACHE_GRID_SIZE / 2;
        for x in -half_grid_size..=half_grid_size {
            for y in -half_grid_size..=half_grid_size {
                if !self.pending_tile_cache.contains_key(&(
                    self.current_center_tile.x + x,
                    self.current_center_tile.y + y,
                )) {
                    self.command_tx
                        .send(WorkerCommand::RenderTile(RenderRequest {
                            id: 0,
                            page_number: self.cur_page_idx,
                            bounds: self
                                .tile_bounds(self.current_center_tile + Vector { x, y })
                                .unwrap(),
                            invert_colors: self.invert_colors,
                            scale: self.pending_scale,
                            x: self.current_center_tile.x + x,
                            y: self.current_center_tile.y + y,
                            generation: self.generation,
                        }))
                        .unwrap();
                }
            }
        }
    }

    fn refresh_file(&mut self) -> Result<()> {
        self.force_invalidate_cache();
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

    fn tile_bounds(&self, coord: Vector<i32>) -> Option<mupdf::IRect> {
        if let Some(page) = self.page_info {
            let mut out_box = self.inner_state.bounds;
            out_box.translate(Vector::new(
                -(self.inner_state.bounds.width() - page.size.x * self.pending_scale) / 2.0,
                -(self.inner_state.bounds.height() - page.size.y * self.pending_scale) / 2.0,
            ));
            out_box.scale(0.6);
            for _ in 0..coord.x.abs() {
                out_box.translate(Vector {
                    x: out_box.width() * (coord.x.signum() as f32),
                    y: 0.0,
                });
            }
            for _ in 0..coord.y.abs() {
                out_box.translate(Vector {
                    x: 0.0,
                    y: out_box.height() * (coord.y.signum() as f32),
                });
            }
            Some(out_box.into())
        } else {
            None
        }
    }
}
