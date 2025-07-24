use anyhow::Result;
use iced::advanced::image;
use mupdf::{Colorspace, Device, DisplayList, Document, IRect, Matrix, Page, Pixmap};
use num::Integer;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error};

use crate::{
    config::MouseAction,
    geometry::{self, Rect, Vector},
    pdf::link_extraction::LinkType,
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
        page.run(&list_dev, &ctm)?;

        Ok(Self {
            scale: 1.0,
            name: name,
            path: path,
            label: String::new(),
            cur_page_idx: 0,
            translation: Vector { x: 0.0, y: 0.0 },
            invert_colors: false,
            inner_state: inner::State {
                bounds: Rect::default(),
                list,
                pix: None,
                img: None,
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
        self.draw_pdf_to_pixmap().unwrap();
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
        self.cur_page_idx = idx.clamp(0, self.doc.page_count()? - 1);
        self.page = self.doc.load_page(self.cur_page_idx)?;
        let bounds = self.page.bounds()?;

        // Regenerate DisplayList for the new page
        self.inner_state.list = DisplayList::new(bounds)?;
        let list_dev = Device::from_display_list(&self.inner_state.list)?;
        let ctm = Matrix::IDENTITY;
        self.page.run(&list_dev, &ctm)?;

        Ok(())
    }

    pub fn refresh_file(&mut self) -> Result<()> {
        todo!("Re-render");
        Ok(())
    }

    fn page_size(&self) -> Vector<f32> {
        let page_bounds: geometry::Rect<f32> = self.page.bounds().unwrap().into();
        page_bounds.size()
    }

    fn zoom_fit_ratio(&mut self) -> Result<f32> {
        let vertical_scale = self.inner_state.bounds.height() / self.page_size().y;
        let horizontal_scale = self.inner_state.bounds.width() / self.page_size().x;
        Ok(vertical_scale.min(horizontal_scale))
    }

    fn screen_to_document_coords(&self, mut screen_pos: Vector<f32>) -> Vector<f32> {
        screen_pos += self.inner_state.bounds.x0;
        screen_pos -= self.inner_state.bounds.center();
        screen_pos.scale(1.0 / self.scale);
        screen_pos += self.translation;
        screen_pos += self.page_size().scaled(0.5);
        screen_pos
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

    fn draw_pdf_to_pixmap(&mut self) -> Result<()> {
        let mut ctm = Matrix::IDENTITY;
        ctm.scale(self.scale, self.scale);
        ctm.pre_translate(-self.translation.x, -self.translation.y);

        if self.inner_state.pix.is_none() {
            self.inner_state.pix = Some(
                Pixmap::new_with_w_h(
                    &Colorspace::device_rgb(),
                    self.inner_state.bounds.width().round() as i32,
                    self.inner_state.bounds.height().round() as i32,
                    true,
                )
                .unwrap(),
            );
        }
        let pix = self.inner_state.pix.as_mut().unwrap();
        pix.clear_with(255)?;
        let device = Device::from_pixmap(pix)?;
        // Why are all the pixels just white?
        self.inner_state
            .list
            .run(&device, &ctm, self.inner_state.bounds.into())?;
        self.inner_state.img = Some(image::Handle::from_rgba(
            pix.width(),
            pix.height(),
            pix.samples().to_vec(), // TODO: Avoid this copy if possible
        ));

        Ok(())
    }
}
