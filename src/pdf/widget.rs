use anyhow::Result;
use num::Integer;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error};

use crate::{
    geometry::{Rect, Vector},
    pdf::link_extraction::LinkType,
};

use super::{
    PdfMessage,
    inner::{self, PageViewer},
    link_extraction::LinkInfo,
    worker::{
        CachedTile, DocumentInfo, PageInfo, RenderRequest, WorkerCommand, WorkerResponse,
        worker_main,
    },
};

const TILE_CACHE_GRID_SIZE: i32 = 5;

const MIN_SELECTION: f32 = 5.0;

#[derive(Debug)]
pub struct PdfViewer {
    pub name: String,
    pub path: PathBuf,
    pub label: String,
    pub cur_page_idx: i32,
    translation: Vector<f32>, // In document space
    pub invert_colors: bool,
    inner_state: inner::State,
    /// Mouse position in screen space. Thus if the PdfViewer isn't positioned at the top left
    /// corner of the screen, it must account for that offset.
    last_mouse_pos: Option<Vector<f32>>,
    panning: bool,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    pub result_rx: Arc<Mutex<mpsc::UnboundedReceiver<WorkerResponse>>>,
    worker_handle: Option<std::thread::JoinHandle<()>>,
    document_info: Option<DocumentInfo>,
    page_info: Option<PageInfo>,
    shown_scale: f32,
    pending_scale: f32,
    pending_tile_cache: HashMap<(i32, i32), CachedTile>,
    shown_tile_cache: HashMap<(i32, i32), CachedTile>,
    current_center_tile: Vector<i32>,
    generation: Arc<std::sync::Mutex<usize>>,
    text_selection_rect: Option<Rect<f32>>,
    selected_text: Option<String>,
    link_hitboxes: Vec<LinkInfo>,
    show_link_hitboxes: bool,
}

impl Default for PdfViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfViewer {
    pub fn new() -> Self {
        assert!(
            TILE_CACHE_GRID_SIZE.is_odd(),
            "The tile cache grid must be of an odd size, is currently {TILE_CACHE_GRID_SIZE}"
        );

        let (command_tx, command_rx) = mpsc::unbounded_channel::<WorkerCommand>();
        let (result_tx, result_rx) = mpsc::unbounded_channel::<WorkerResponse>();

        let worker_handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(worker_main(command_rx, result_tx));
        });

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
            result_rx: Arc::new(Mutex::new(result_rx)),
            worker_handle: Some(worker_handle),
            document_info: None,
            page_info: None,
            pending_tile_cache: HashMap::new(),
            shown_tile_cache: HashMap::new(),
            current_center_tile: Vector::zero(),
            generation: Arc::new(std::sync::Mutex::new(0)),
            text_selection_rect: None,
            selected_text: None,
            link_hitboxes: Vec::new(),
            show_link_hitboxes: false,
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
                    } else if let Some(rect) = self.text_selection_rect {
                        // Update text selection - extend rectangle to current mouse position
                        let top_left =
                            Vector::new(rect.x0.x.min(vector.x), rect.x0.y.min(vector.y));
                        let bottom_right =
                            Vector::new(rect.x0.x.max(vector.x), rect.x0.y.max(vector.y));
                        self.text_selection_rect = Some(Rect::from_points(top_left, bottom_right));
                    }
                    self.last_mouse_pos = Some(vector);
                } else {
                    self.last_mouse_pos = None;
                }
            }
            PdfMessage::MouseLeftDown(ctrl_pressed) => {
                if ctrl_pressed {
                    // Don't start panning if we're close enough to the edge that a pane resizing might happen
                    if let Some(mp) = self.last_mouse_pos {
                        let mut padded_bounds = self.inner_state.bounds;
                        padded_bounds.x0 += Vector { x: 11.0, y: 11.0 };
                        padded_bounds.x1 -= Vector { x: 11.0, y: 11.0 };
                        if padded_bounds.contains(mp) {
                            self.panning = true;
                        }
                    }
                } else if let Some(pos) = self.last_mouse_pos {
                    // Start text selection with a zero-size rectangle at mouse position
                    self.text_selection_rect = Some(Rect::from_points(pos, pos));
                }
            }
            PdfMessage::MouseRightDown => {}
            PdfMessage::MouseLeftUp(ctrl_pressed) => {
                if ctrl_pressed {
                    self.panning = false;
                } else {
                    if let Some(screen_rect) = self.text_selection_rect {
                        let doc_start = self.screen_to_document_coords(screen_rect.x0);
                        let doc_end = self.screen_to_document_coords(screen_rect.x1);

                        let selection_rect = Rect::from_points(
                            Vector::new(doc_start.x.min(doc_end.x), doc_start.y.min(doc_end.y)),
                            Vector::new(doc_start.x.max(doc_end.x), doc_start.y.max(doc_end.y)),
                        );

                        if selection_rect.width() > MIN_SELECTION
                            && selection_rect.height() > MIN_SELECTION
                        {
                            if let Err(e) = self
                                .command_tx
                                .send(WorkerCommand::ExtractText(selection_rect.into()))
                            {
                                error!("Failed to send text extraction command: {}", e);
                            }
                        } else if let Err(e) = self.command_tx.send(WorkerCommand::ExtractLinks) {
                            error!("Failed to send link extraction command: {}", e);
                        }
                    }
                    self.text_selection_rect = None;
                }
            }
            PdfMessage::MouseRightUp => {}
            PdfMessage::ToggleLinkHitboxes => {
                self.show_link_hitboxes = !self.show_link_hitboxes;
            }
            PdfMessage::WorkerResponse(worker_response) => match worker_response {
                WorkerResponse::RenderedTile(cached_tile) => {
                    let current_generation = *self.generation.lock().unwrap();
                    if cached_tile.generation == current_generation {
                        self.pending_tile_cache
                            .insert((cached_tile.x, cached_tile.y), cached_tile);
                        if self.pending_tile_cache.len() == TILE_CACHE_GRID_SIZE.pow(2) as usize {
                            std::mem::swap(
                                &mut self.pending_tile_cache,
                                &mut self.shown_tile_cache,
                            );
                            self.pending_tile_cache.clear();
                            self.shown_scale = self.pending_scale;
                        }
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
                    // Extract links for the new page
                    if let Err(e) = self.command_tx.send(WorkerCommand::ExtractLinks) {
                        error!("Failed to send link extraction command: {}", e);
                    }
                }
                WorkerResponse::Refreshed(_, document_info) => {
                    self.document_info = Some(document_info);
                    self.refresh_file().unwrap();
                }
                WorkerResponse::ExtractedText(text_selection) => {
                    self.selected_text = Some(text_selection.text.clone());
                    if let Ok(mut clipboard) = arboard::Clipboard::new()
                        && let Err(e) = clipboard.set_text(&text_selection.text)
                    {
                        error!("Failed to copy text to clipboard: {}", e);
                    }
                }
                WorkerResponse::ExtractedLinks(links) => {
                    self.link_hitboxes = links;
                    if let Some(pos) = self
                        .last_mouse_pos
                        .map(|p| self.screen_to_document_coords(p))
                    {
                        if let Some(link) = self
                            .link_hitboxes
                            .iter()
                            .find(|link| link.bounds.contains(pos))
                        {
                            debug!("{link:?}");
                            match link.link_type {
                                LinkType::InternalPage(page) => {
                                    if self.set_page(page as i32).is_err() {
                                        error!("Couldn't jump to page {page}");
                                    }
                                }
                                _ => {
                                    if let Ok(mut clipboard) = arboard::Clipboard::new()
                                        && let Err(e) = clipboard.set_text(&link.uri)
                                    {
                                        error!("Failed to copy link to clipboard: {}", e);
                                    }
                                }
                            }
                        }
                    }
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
            .text_selection(self.text_selection_rect)
            .link_hitboxes(if self.show_link_hitboxes {
                Some(&self.link_hitboxes)
            } else {
                None
            })
            .page_info(self.page_info)
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
        let mut generation = self.generation.lock().unwrap();
        *generation += 1;
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
                            generation: *self.generation.lock().unwrap(),
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

    pub async fn try_recv_worker_response(&self) -> Option<WorkerResponse> {
        if let Ok(mut rx) = self.result_rx.try_lock() {
            rx.try_recv().ok()
        } else {
            None
        }
    }

    pub fn refresh_file_worker(&mut self) -> Result<()> {
        self.command_tx.send(WorkerCommand::RefreshFile)?;
        Ok(())
    }

    fn screen_to_document_coords(&self, mut screen_pos: Vector<f32>) -> Vector<f32> {
        if let Some(page) = self.page_info {
            screen_pos += self.inner_state.bounds.x0;
            screen_pos -= self.inner_state.bounds.center();
            screen_pos.scale(1.0 / self.shown_scale);
            screen_pos += self.translation;
            screen_pos += page.size.scaled(0.5);
            screen_pos
        } else {
            // Fallback to old method if no page info
            let viewport_center = self.inner_state.bounds.center();
            let relative_pos = screen_pos - viewport_center;
            relative_pos.scaled(1.0 / self.shown_scale) + self.translation
        }
    }
}

impl Drop for PdfViewer {
    fn drop(&mut self) {
        let _ = self.command_tx.send(WorkerCommand::Shutdown);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}
