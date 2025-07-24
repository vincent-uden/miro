use anyhow::Result;
use mupdf::{Device, DisplayList, Document, IRect, Matrix, Page, Pixmap};
use num::Integer;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error};

use crate::{
    geometry::{Rect, Vector},
    pdf::link_extraction::LinkType,
    config::MouseAction,
};

use super::{
    PdfMessage,
    inner::{self, PageViewer},
    link_extraction::LinkInfo,
    outline_extraction::OutlineItem,
};

const MIN_SELECTION: f32 = 5.0;
const MIN_CLICK_DISTANCE: f32 = 5.0;

/// Renders a pdf document. Owns all information related to the document.
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
    /// Position where the mouse was pressed down, used to detect clicks vs pans
    mouse_down_pos: Option<Vector<f32>>,
    panning: bool,
    scale: f32,
    text_selection_start: Option<Vector<f32>>,
    selected_text: Option<String>,
    link_hitboxes: Vec<LinkInfo>,
    show_link_hitboxes: bool,
    is_over_link: bool,
    document_outline: Option<Vec<OutlineItem>>,

    doc: Document,
    page: Page,
}

impl PdfViewer {
    pub fn from_path(path: PathBuf) -> Result<Self> {
        let name = path
            .file_name()
            .expect("The pdf must have a file name")
            .to_string_lossy()
            .to_string();
        let doc = Document::open(&path.to_str().unwrap())?;
        let page = doc.load_page(0)?;
        let bounds = page.bounds()?;
        // All of these can be immutable since the mutability is actually hidden across the ffi
        // boundary in the C structs.
        let list = DisplayList::new(bounds)?;
        let list_dev = Device::from_display_list(&list)?;
        let ctm = Matrix::IDENTITY;
        list.run(&list_dev, &ctm, bounds)?;

        Ok(Self {
            scale: 1.0,
            name: name,
            path: path,
            label: String::new(),
            cur_page_idx: 0,
            translation: Vector { x: 0.0, y: 0.0 },
            invert_colors: false,
            inner_state: inner::State {
                bounds: bounds.into(),
                list,
                pix: None,
            },
            last_mouse_pos: None,
            mouse_down_pos: None,
            panning: false,
            text_selection_start: None,
            selected_text: None,
            link_hitboxes: Vec::new(),
            show_link_hitboxes: false,
            is_over_link: false,
            document_outline: None,
            doc,
            page,
        })
    }

    pub fn update(&mut self, message: PdfMessage) -> iced::Task<PdfMessage> {
        self.label = format!("{} {}/{}", self.name, self.cur_page_idx + 1, "?",);
        match message {
            PdfMessage::OpenFile(path_buf) => self.load_file(path_buf).unwrap(),
            PdfMessage::NextPage => self.set_page(self.cur_page_idx + 1).unwrap(),
            PdfMessage::PreviousPage => self.set_page(self.cur_page_idx - 1).unwrap(),
            PdfMessage::SetPage(page) => self.set_page(page).unwrap(),
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
                // TODO: The amount of wl_registrys that appear scale with the amount of resizing
                // of the window that is done
                self.inner_state.bounds = rectangle;
            }
            PdfMessage::None => {}
            PdfMessage::MouseMoved(vector) => {
                if self.inner_state.bounds.contains(vector) {
                    if self.panning && self.last_mouse_pos.is_some() {
                        self.translation +=
                            (self.last_mouse_pos.unwrap() - vector).scaled(1.0 / self.scale);
                    }
                    let doc_pos = self.screen_to_document_coords(vector);
                    self.is_over_link = self
                        .link_hitboxes
                        .iter()
                        .any(|link| link.bounds.contains(doc_pos));

                    self.last_mouse_pos = Some(vector);
                } else {
                    self.last_mouse_pos = None;
                    self.is_over_link = false;
                }
            }
            PdfMessage::MouseLeftDown(shift_pressed) => {
                // Store the initial mouse position for click vs pan detection
                self.mouse_down_pos = self.last_mouse_pos;

                if shift_pressed {
                    // Start text selection at mouse position
                    if let Some(pos) = self.last_mouse_pos {
                        self.text_selection_start = Some(pos);
                    }
                } else {
                    // Don't start panning if we're close enough to the edge that a pane resizing might happen
                    if let Some(mp) = self.last_mouse_pos {
                        let mut padded_bounds = self.inner_state.bounds;
                        padded_bounds.x0 += Vector { x: 11.0, y: 11.0 };
                        padded_bounds.x1 -= Vector { x: 11.0, y: 11.0 };
                        if padded_bounds.contains(mp) {
                            self.panning = true;
                        }
                    }
                }
            }

            PdfMessage::MouseLeftUp(shift_pressed) => {
                if shift_pressed {
                    if let (Some(start_pos), Some(end_pos)) =
                        (self.text_selection_start, self.last_mouse_pos)
                    {
                        let doc_start = self.screen_to_document_coords(start_pos);
                        let doc_end = self.screen_to_document_coords(end_pos);

                        let selection_rect = Rect::from_points(
                            Vector::new(doc_start.x.min(doc_end.x), doc_start.y.min(doc_end.y)),
                            Vector::new(doc_start.x.max(doc_end.x), doc_start.y.max(doc_end.y)),
                        );

                        if selection_rect.width() > MIN_SELECTION
                            && selection_rect.height() > MIN_SELECTION
                        {
                            // TODO: Remove this entire section. Should be handled in mouse action
                        }
                    }
                    self.text_selection_start = None;
                } else {
                    // Handle link clicks only if mouse didn't move significantly (click vs pan)
                    let is_click = if let (Some(down_pos), Some(up_pos)) =
                        (self.mouse_down_pos, self.last_mouse_pos)
                    {
                        let delta = up_pos - down_pos;
                        let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();
                        distance < MIN_CLICK_DISTANCE
                    } else {
                        false
                    };

                    if is_click {
                        if let Some(pos) = self
                            .last_mouse_pos
                            .map(|p| self.screen_to_document_coords(p))
                            && let Some(link) = self
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
                    self.panning = false;
                    self.mouse_down_pos = None;
                }
            }

            PdfMessage::MouseAction(action, pressed) => match (action, pressed) {
                (MouseAction::Panning, true) => {
                    if let Some(mp) = self.last_mouse_pos {
                        let mut padded_bounds = self.inner_state.bounds;
                        padded_bounds.x0 += Vector { x: 11.0, y: 11.0 };
                        padded_bounds.x1 -= Vector { x: 11.0, y: 11.0 };
                        if padded_bounds.contains(mp) {
                            self.panning = true;
                        }
                    }
                }
                (MouseAction::Panning, false) => {
                    self.panning = false;
                }
                (MouseAction::Selection, true) => {
                    if let Some(pos) = self.last_mouse_pos {
                        self.text_selection_start = Some(pos);
                    }
                }
                (MouseAction::Selection, false) => {
                    if let (Some(start_pos), Some(end_pos)) =
                        (self.text_selection_start, self.last_mouse_pos)
                    {
                        let doc_start = self.screen_to_document_coords(start_pos);
                        let doc_end = self.screen_to_document_coords(end_pos);

                        let selection_rect = Rect::from_points(
                            Vector::new(doc_start.x.min(doc_end.x), doc_start.y.min(doc_end.y)),
                            Vector::new(doc_start.x.max(doc_end.x), doc_start.y.max(doc_end.y)),
                        );

                        if selection_rect.width() > MIN_SELECTION
                            && selection_rect.height() > MIN_SELECTION
                        {
                            todo!("Extract text");
                        }
                    }
                    self.text_selection_start = None;
                }
                (MouseAction::NextPage, true) => {
                    let _ = self.set_page(self.cur_page_idx + 1);
                }
                (MouseAction::NextPage, false) => {}
                (MouseAction::PreviousPage, true) => {
                    let _ = self.set_page(self.cur_page_idx - 1);
                }
                (MouseAction::PreviousPage, false) => {}
            },
            PdfMessage::ToggleLinkHitboxes => {
                self.show_link_hitboxes = !self.show_link_hitboxes;
            }
        }
        iced::Task::none()
    }

    pub fn view(&self) -> iced::Element<'_, PdfMessage> {
        PageViewer::new(&self.inner_state)
            .translation(self.translation)
            .scale(self.scale)
            .invert_colors(self.invert_colors)
            .text_selection(self.current_selection_rect())
            .link_hitboxes(if self.show_link_hitboxes {
                Some(&self.link_hitboxes)
            } else {
                None
            })
            .over_link(self.is_over_link)
            .into()
    }

    fn set_page(&mut self, idx: i32) -> Result<()> {
        todo!("Enable once we have access to the document");
        /* if let Some(doc) = self.document_info {
            self.cur_page_idx = idx.clamp(0, doc.page_count - 1);
        } */
        Ok(())
    }

    fn load_file(&mut self, path: PathBuf) -> Result<()> {
        Ok(())
    }

    fn refresh_file(&mut self) -> Result<()> {
        todo!("Re-render");
        Ok(())
    }

    fn zoom_fit_ratio(&mut self) -> Result<f32> {
        todo!()
        // if let Some(page) = &self.page_info {
        //     let page_size = page.size;
        //     let vertical_scale = self.inner_state.bounds.height() / page_size.y;
        //     let horizontal_scale = self.inner_state.bounds.width() / page_size.x;
        //     Ok(vertical_scale.min(horizontal_scale))
        // } else {
        //     Ok(1.0)
        // }
    }

    fn screen_to_document_coords(&self, mut screen_pos: Vector<f32>) -> Vector<f32> {
        todo!()
        /* if let Some(page) = self.page_info {
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
        } */
    }

    pub fn get_outline(&self) -> Option<&Vec<OutlineItem>> {
        self.document_outline.as_ref()
    }

    fn current_selection_rect(&self) -> Option<Rect<f32>> {
        if let (Some(start_pos), Some(current_pos)) =
            (self.text_selection_start, self.last_mouse_pos)
        {
            let top_left = Vector::new(
                start_pos.x.min(current_pos.x),
                start_pos.y.min(current_pos.y),
            );
            let bottom_right = Vector::new(
                start_pos.x.max(current_pos.x),
                start_pos.y.max(current_pos.y),
            );
            Some(Rect::from_points(top_left, bottom_right))
        } else {
            None
        }
    }
}
