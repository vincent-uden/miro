use anyhow::Result;
use colorgrad::{Gradient as _, GradientBuilder, LinearGradient};
use mupdf::{Colorspace, Device, DisplayList, Document, Matrix, Page, Pixmap};
use std::{cell::RefCell, path::PathBuf};
use tracing::{error, info};
use open;

use crate::{
    config::MouseAction,
    geometry::{self, Rect, Vector},
    pdf::{
        link_extraction::{LinkExtractor, LinkType},
        outline_extraction::OutlineExtractor,
        text_extraction::TextExtractor,
    },
    CONFIG, DARK_THEME,
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
    pub page_progress: String,
    pub cur_page_idx: i32,
    pub translation: Vector<f32>, // In document space
    pub invert_colors: bool,
    pub draw_page_borders: bool,
    inner_state: RefCell<inner::State>,
    /// Mouse position in screen space. Thus if the PdfViewer isn't positioned at the top left
    /// corner of the screen, it must account for that offset.
    last_mouse_pos: Option<Vector<f32>>,
    /// Position where the mouse was pressed down, used to detect clicks vs pans
    mouse_down_pos: Option<Vector<f32>>,
    panning: bool,
    scale: f32,
    /// Factor used to scale the pixmap up/down to compensate for fractional scaling in at the
    /// WM/DE level.
    scale_factor: f64,
    text_selection_start: Option<Vector<f32>>,
    link_hitboxes: Vec<LinkInfo>,
    show_link_hitboxes: bool,
    is_over_link: bool,
    document_outline: Vec<OutlineItem>,

    doc: Document,
    page: Page,
    // Two-page state
    pub two_page_mode: bool,
    pub cover_page: bool,
    right_page: Option<Page>,
    right_list: Option<DisplayList>,

    old_bounds: RefCell<Rect<f32>>,

    gradient_cache: [[u8; 4]; 256],
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

        let extractor = LinkExtractor::new(&page);
        let link_hitboxes = extractor.extract_all_links()?;

        let extractor = OutlineExtractor::new(&doc);
        let document_outline = extractor.extract_outline()?;

        let bg_color = DARK_THEME
            .extended_palette()
            .background
            .base
            .color
            .into_rgba8();
        let mut gradient_cache = [[0; 4]; 256];
        generate_gradient_cache(&mut gradient_cache, &bg_color);

        let mut viewer = Self {
            scale: 1.0,
            scale_factor: 1.0,
            name,
            path,
            label: String::new(),
            page_progress: String::new(),
            cur_page_idx: 0,
            translation: Vector { x: 0.0, y: 0.0 },
            invert_colors: CONFIG.read().unwrap().invert_pdf,
            draw_page_borders: CONFIG.read().unwrap().page_borders,
            inner_state: RefCell::new(inner::State {
                bounds: Rect::default(),
                page_size: page.bounds()?.size().into(),
                list,
                pix: None,
            }),
            last_mouse_pos: None,
            mouse_down_pos: None,
            panning: false,
            text_selection_start: None,
            link_hitboxes,
            show_link_hitboxes: false,
            is_over_link: false,
            document_outline,
            doc,
            page,
            two_page_mode: false,
            cover_page: false,
            right_page: None,
            right_list: None,
            gradient_cache,
            old_bounds: RefCell::new(Rect::default()),
        };
        // Ensure consistent initialization of layout-dependent fields
        viewer.set_page(0)?;
        Ok(viewer)
    }

    pub fn update(&mut self, message: PdfMessage) -> iced::Task<PdfMessage> {
        let task: iced::Task<PdfMessage> = match message {
            PdfMessage::NextPage => {
                if self.two_page_mode {
                    if self.cover_page && self.cur_page_idx == 0 {
                        self.set_page(1).unwrap();
                    } else {
                        self.set_page(self.cur_page_idx + 2).unwrap();
                    }
                } else {
                    self.set_page(self.cur_page_idx + 1).unwrap();
                }
                iced::Task::none()
            }
            PdfMessage::PreviousPage => {
                if self.two_page_mode {
                    if self.cover_page && self.cur_page_idx == 1 {
                        self.set_page(0).unwrap();
                    } else {
                        self.set_page(self.cur_page_idx - 2).unwrap();
                    }
                } else {
                    self.set_page(self.cur_page_idx - 1).unwrap();
                }
                iced::Task::none()
            }
            PdfMessage::SetPage(page) => {
                self.set_page(page).unwrap();
                iced::Task::none()
            }
            PdfMessage::SetTranslation(translation) => {
                self.translation = translation;
                iced::Task::none()
            }
            PdfMessage::ZoomIn => {
                self.scale *= 1.2;
                iced::Task::none()
            }
            PdfMessage::ZoomOut => {
                self.scale /= 1.2;
                iced::Task::none()
            }
            PdfMessage::ZoomHome => {
                self.scale = 1.0;
                self.translation.x = 0.0;
                self.translation.y = 0.0;
                iced::Task::none()
            }
            PdfMessage::ZoomFit => {
                self.scale = self.zoom_fit_ratio().unwrap_or(1.0);
                self.translation.x = 0.0;
                self.translation.y = 0.0;
                iced::Task::none()
            }
            PdfMessage::Move(vec) => {
                self.translation.x += vec.x / self.scale;
                self.translation.y += vec.y / self.scale;
                iced::Task::none()
            }
            PdfMessage::UpdateBounds(rectangle) => {
                self.inner_state.borrow_mut().bounds = rectangle;
                iced::Task::done(PdfMessage::ReallocPixmap)
            }
            PdfMessage::ToggleTwoPage => {
                self.two_page_mode = !self.two_page_mode;
                // Re-anchor page index and recompute lists and sizes
                let page = self.cur_page_idx;
                self.set_page(page).unwrap();
                iced::Task::none()
            }
            PdfMessage::ToggleCoverPage => {
                self.cover_page = !self.cover_page;
                if self.two_page_mode {
                    let page = self.cur_page_idx;
                    self.set_page(page).unwrap();
                }
                iced::Task::none()
            }
            PdfMessage::None => iced::Task::none(),
            PdfMessage::MouseMoved(vector) => {
                if self.inner_state.borrow().bounds.contains(vector) {
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
                iced::Task::none()
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
                        let mut padded_bounds = self.inner_state.borrow().bounds;
                        padded_bounds.x0 += Vector { x: 11.0, y: 11.0 };
                        padded_bounds.x1 -= Vector { x: 11.0, y: 11.0 };
                        if padded_bounds.contains(mp) {
                            self.panning = true;
                        }
                    }
                }
                iced::Task::none()
            }
            PdfMessage::MouseLeftUp(shift_pressed) => {
                if !shift_pressed {
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

                    if is_click
                        && let Some(pos) = self
                            .last_mouse_pos
                            .map(|p| self.screen_to_document_coords(p))
                        && let Some(link) = self
                            .link_hitboxes
                            .iter()
                            .find(|link| link.bounds.contains(pos))
                    {
                        match link.link_type {
                            LinkType::InternalPage(page) => {
                                if self.set_page(page as i32).is_err() {
                                    error!("Couldn't jump to page {page}");
                                }
                            }
                            LinkType::ExternalUrl => {
                                if let Err(e) = open::that(&link.uri) {
                                    error!("Failed to open external link: {}", e);
                                    // Fallback to clipboard copy if opening fails
                                    if let Ok(mut clipboard) = arboard::Clipboard::new()
                                        && let Err(e) = clipboard.set_text(&link.uri)
                                    {
                                        error!("Failed to copy link to clipboard: {}", e);
                                    }
                                }
                            }
                            LinkType::Email | LinkType::Other => {
                                if let Ok(mut clipboard) = arboard::Clipboard::new()
                                    && let Err(e) = clipboard.set_text(&link.uri)
                                {
                                    error!("Failed to copy link to clipboard: {}", e);
                                }
                            }
                        }
                        // Hide links after activation
                        self.show_link_hitboxes = false;
                    }
                    self.panning = false;
                    self.mouse_down_pos = None;
                }
                iced::Task::none()
            }
            PdfMessage::MouseAction(action, pressed) => {
                match (action, pressed) {
                    (MouseAction::Panning, true) => {
                        if let Some(mp) = self.last_mouse_pos {
                            let mut padded_bounds = self.inner_state.borrow().bounds;
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
                            let mut doc_start = self.screen_to_document_coords(start_pos);
                            let mut doc_end = self.screen_to_document_coords(end_pos);

                            // Limit selection to a single page. If both points are on the right page,
                            // adjust coordinates and use right_page for extraction.
                            let mut use_right = false;
                            let left_w = self.page_size().x;
                            if self.two_page_mode
                                && doc_start.x > left_w
                                && doc_end.x > left_w
                                && self.right_page.is_some()
                            {
                                doc_start.x -= left_w;
                                doc_end.x -= left_w;
                                use_right = true;
                            }

                            let selection_rect = Rect::from_points(
                                Vector::new(doc_start.x.min(doc_end.x), doc_start.y.min(doc_end.y)),
                                Vector::new(doc_start.x.max(doc_end.x), doc_start.y.max(doc_end.y)),
                            );

                            if selection_rect.width() > MIN_SELECTION
                                && selection_rect.height() > MIN_SELECTION
                            {
                                let selection = if use_right {
                                    let extractor =
                                        TextExtractor::new(self.right_page.as_ref().unwrap());
                                    extractor
                                        .extract_text_in_rect(selection_rect.into())
                                        .unwrap()
                                } else {
                                    let extractor = TextExtractor::new(&self.page);
                                    extractor
                                        .extract_text_in_rect(selection_rect.into())
                                        .unwrap()
                                };
                                info!("Copied: \"{}\" at {:?}", selection.text, selection.bounds);
                                arboard::Clipboard::new().map_or_else(
                                    |e| error!("{e}"),
                                    |mut clipboard| {
                                        clipboard
                                            .set_text(selection.text)
                                            .inspect_err(|e| error!("{e}"))
                                            .unwrap();
                                    },
                                )
                            }
                        }
                        self.text_selection_start = None;
                    }
                    (MouseAction::NextPage, true) => {
                        if self.two_page_mode {
                            if self.cover_page && self.cur_page_idx == 0 {
                                let _ = self.set_page(1);
                            } else {
                                let _ = self.set_page(self.cur_page_idx + 2);
                            }
                        } else {
                            let _ = self.set_page(self.cur_page_idx + 1);
                        }
                    }
                    (MouseAction::NextPage, false) => {}
                    (MouseAction::PreviousPage, true) => {
                        if self.two_page_mode {
                            if self.cover_page && self.cur_page_idx == 1 {
                                let _ = self.set_page(0);
                            } else {
                                let _ = self.set_page(self.cur_page_idx - 2);
                            }
                        } else {
                            let _ = self.set_page(self.cur_page_idx - 1);
                        }
                    }
                    (MouseAction::PreviousPage, false) => {}
                    (MouseAction::ZoomIn, true) => {
                        self.scale *= 1.2;
                    }
                    (MouseAction::ZoomIn, false) => {}
                    (MouseAction::ZoomOut, true) => {
                        self.scale /= 1.2;
                    }
                    (MouseAction::ZoomOut, false) => {}
                    (MouseAction::MoveUp, true) => {
                        self.translation.y -= crate::config::MOVE_STEP / self.scale;
                    }
                    (MouseAction::MoveUp, false) => {}
                    (MouseAction::MoveDown, true) => {
                        self.translation.y += crate::config::MOVE_STEP / self.scale;
                    }
                    (MouseAction::MoveDown, false) => {}
                    (MouseAction::MoveLeft, true) => {
                        self.translation.x -= crate::config::MOVE_STEP / self.scale;
                    }
                    (MouseAction::MoveLeft, false) => {}
                    (MouseAction::MoveRight, true) => {
                        self.translation.x += crate::config::MOVE_STEP / self.scale;
                    }
                    (MouseAction::MoveRight, false) => {}
                }
                iced::Task::none()
            }
            PdfMessage::ToggleLinkHitboxes => {
                self.show_link_hitboxes = !self.show_link_hitboxes;
                iced::Task::none()
            }
            PdfMessage::ActivateLink(index) => {
                if let Some(link) = self.link_hitboxes.get(index) {
                    match link.link_type {
                        LinkType::InternalPage(page) => {
                            if self.set_page(page as i32).is_err() {
                                error!("Couldn't jump to page {page}");
                            }
                        }
                        LinkType::ExternalUrl => {
                            if let Err(e) = open::that(&link.uri) {
                                error!("Failed to open external link: {}", e);
                                if let Ok(mut clipboard) = arboard::Clipboard::new()
                                    && let Err(e) = clipboard.set_text(&link.uri)
                                {
                                    error!("Failed to copy link to clipboard: {}", e);
                                }
                            }
                        }
                        LinkType::Email | LinkType::Other => {
                            if let Ok(mut clipboard) = arboard::Clipboard::new()
                                && let Err(e) = clipboard.set_text(&link.uri)
                            {
                                error!("Failed to copy link to clipboard: {}", e);
                            }
                        }
                    }
                    // Hide links after activation
                    self.show_link_hitboxes = false;
                }
                iced::Task::none()
            }
            PdfMessage::CloseLinkHitboxes => {
                self.show_link_hitboxes = false;
                iced::Task::none()
            }
            PdfMessage::FileChanged => {
                self.refresh_file().unwrap();
                iced::Task::none()
            }
            PdfMessage::ReallocPixmap => {
                self.inner_state.borrow_mut().pix = None;
                iced::Task::none()
            }
            PdfMessage::PrintPdf => {
                let path = self.path.clone();
                iced::Task::perform(
                    async move {
                        let file_url = format!("file://{}", path.to_string_lossy());
                        if let Err(e) = webbrowser::open(&file_url) {
                            error!("Failed to open PDF in default browser: {}", e);
                        }
                    },
                    |_| PdfMessage::None,
                )
            }
        };
        self.label = format!(
            "{} {}/{}",
            self.name,
            self.cur_page_idx + 1,
            self.doc.page_count().unwrap_or(0),
        );
        self.page_progress = format!(
            " {}/{}",
            self.cur_page_idx + 1,
            self.doc.page_count().unwrap_or(0),
        );
        task
    }

    pub fn is_jumpable_action(&self, message: &PdfMessage) -> bool {
        match message {
            PdfMessage::ActivateLink(index) => {
                if let Some(link) = self.link_hitboxes.get(*index) {
                    matches!(link.link_type, LinkType::InternalPage(_))
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn view(&self) -> iced::Element<'_, PdfMessage> {
        self.draw_pdf_to_pixmap().unwrap();
        PageViewer::new(self.inner_state.borrow())
            .translation(self.translation)
            .scale(self.scale)
            .invert_colors(self.invert_colors)
            .draw_page_borders(self.draw_page_borders)
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
        let page_count = self.doc.page_count()?;
        let idx = idx.clamp(0, page_count - 1);
        // Anchor index depending on two-page & cover
        let anchor = if self.two_page_mode {
            if self.cover_page {
                if idx == 0 { 0 } else if idx % 2 == 1 { idx } else { idx - 1 }
            } else {
                if idx % 2 == 0 { idx } else { idx - 1 }
            }
        } else {
            idx
        };
        self.cur_page_idx = anchor;

        // Load left page
        self.page = self.doc.load_page(self.cur_page_idx)?;
        let left_bounds = self.page.bounds()?;

        // Left page display list
        {
            let mut state = self.inner_state.borrow_mut();
            state.page_size = left_bounds.size().into();
            state.list = DisplayList::new(left_bounds)?;
            let list_dev = Device::from_display_list(&state.list)?;
            let ctm = Matrix::IDENTITY;
            self.page.run(&list_dev, &ctm)?;
        }

        // Right page (if in range and two-page mode)
        self.right_page = None;
        self.right_list = None;
        let mut combined_links: Vec<LinkInfo> = Vec::new();
        // Extract left links first
        {
            let extractor = LinkExtractor::new(&self.page);
            let left_links = extractor.extract_all_links()?;
            combined_links.extend(left_links);
        }

        let left_size: Vector<f32> = left_bounds.size().into();

        if self.two_page_mode {
            let right_idx = self.cur_page_idx + 1;
            if right_idx < page_count {
                let rp = self.doc.load_page(right_idx)?;
                let rb = rp.bounds()?;
                let list = DisplayList::new(rb)?;
                let list_dev = Device::from_display_list(&list)?;
                let ctm = Matrix::IDENTITY;
                rp.run(&list_dev, &ctm)?;
                self.right_page = Some(rp);
                self.right_list = Some(list);

                // Extract right links and offset by left width (doc units)
                let extractor = LinkExtractor::new(self.right_page.as_ref().unwrap());
                let mut right_links = extractor.extract_all_links()?;
                for l in &mut right_links {
                    l.bounds.x0.x += left_size.x;
                    l.bounds.x1.x += left_size.x;
                }
                combined_links.extend(right_links);

                // Update combined page size in state
                let mut state = self.inner_state.borrow_mut();
                let right_size: Vector<f32> = rb.size().into();
                state.page_size = Vector::new(left_size.x + right_size.x, left_size.y.max(right_size.y));
            } else {
                // Only left page exists; treat as single
                let mut state = self.inner_state.borrow_mut();
                state.page_size = left_size;
            }
        } else {
            let mut state = self.inner_state.borrow_mut();
            state.page_size = left_size;
        }

        self.link_hitboxes = combined_links;
        Ok(())
    }

    pub fn refresh_file(&mut self) -> Result<()> {
        self.doc = Document::open(&self.path.to_str().unwrap())?;
        let extractor = OutlineExtractor::new(&self.doc);
        self.document_outline = extractor.extract_outline()?;
        self.set_page(self.cur_page_idx)?;
        Ok(())
    }

    fn page_size(&self) -> Vector<f32> {
        // Single-page size of the left/anchor page
        let page_bounds: geometry::Rect<f32> = self.page.bounds().unwrap().into();
        page_bounds.size()
    }

    fn spread_size(&self) -> Vector<f32> {
        if self.two_page_mode {
            let left_size = self.page_size();
            if let Some(ref rp) = self.right_page {
                let r_bounds: geometry::Rect<f32> = rp.bounds().unwrap().into();
                let r_size = r_bounds.size();
                Vector::new(left_size.x + r_size.x, left_size.y.max(r_size.y))
            } else {
                left_size
            }
        } else {
            self.page_size()
        }
    }

    fn zoom_fit_ratio(&mut self) -> Result<f32> {
        let size = self.spread_size();
        let vertical_scale = self.inner_state.borrow().bounds.height() / size.y;
        let horizontal_scale = self.inner_state.borrow().bounds.width() / size.x;
        Ok(vertical_scale.min(horizontal_scale))
    }

    fn screen_to_document_coords(&self, mut screen_pos: Vector<f32>) -> Vector<f32> {
        let centering_vector = (self.inner_state.borrow().bounds.size()
            - self.spread_size().scaled(self.scale))
        .scaled(0.5);
        screen_pos -= self.inner_state.borrow().bounds.x0; // screen scale
        screen_pos -= centering_vector; // screen scale
        screen_pos.scale(1.0 / self.scale);
        screen_pos += self.translation;
        screen_pos
    }

    pub fn get_outline(&self) -> &[OutlineItem] {
        self.document_outline.as_slice()
    }

    pub fn get_page_count(&self) -> i32 {
        self.doc.page_count().unwrap_or(0)
    }

    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        if (self.scale_factor - scale_factor).abs() > 0.01 {
            self.scale_factor = scale_factor;
            // Invalidate pixmap to force re-render at new scale factor
            self.inner_state.borrow_mut().pix = None;
        }
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

    fn draw_pdf_to_pixmap(&self) -> Result<()> {
        let mut state = self.inner_state.borrow_mut();
        // Build transforms per page

        let effective_scale = self.scale * self.scale_factor as f32;
        let centering_vector = (state.bounds.size().scaled(self.scale_factor as f32)
            - self.spread_size().scaled(effective_scale))
        .scaled(0.5);
        // Left page transform
        let mut ctm_left = Matrix::IDENTITY;
        ctm_left.pre_translate(centering_vector.x, centering_vector.y);
        ctm_left.scale(effective_scale, effective_scale);
        ctm_left.pre_translate(-self.translation.x, -self.translation.y);

        let mut old_bounds = self.old_bounds.borrow_mut();
        // The bounds check here saves one frame of jitter on resizing the window for some reason
        if *old_bounds != state.bounds || state.pix.is_none() {
            let render_width = (state.bounds.width() * self.scale_factor as f32).round() as i32;
            let render_height = (state.bounds.height() * self.scale_factor as f32).round() as i32;

            state.pix = Some(
                Pixmap::new_with_w_h(&Colorspace::device_rgb(), render_width, render_height, true)
                    .unwrap(),
            );

            *old_bounds = state.bounds;
        }
        let bounds = state.bounds;
        let device = {
            let pix = state.pix.as_mut().unwrap();
            let samples = pix.samples_mut();
            samples.fill(255);
            Device::from_pixmap(pix)?
        };
        // Render left page
        state.list.run(
            &device,
            &ctm_left,
            mupdf::Rect {
                x0: 0.0,
                y0: 0.0,
                x1: bounds.width() * self.scale_factor as f32,
                y1: bounds.height() * self.scale_factor as f32,
            },
        )?;
        // Render right page if present
        if let (true, Some(list), Some(_rp)) = (self.two_page_mode, &self.right_list, &self.right_page) {
            let left_w = self.page_size().x;
            let mut ctm_right = Matrix::IDENTITY;
            ctm_right.pre_translate(centering_vector.x, centering_vector.y);
            ctm_right.scale(effective_scale, effective_scale);
            // Shift by left page width (doc units)
            ctm_right.pre_translate(left_w, 0.0);
            ctm_right.pre_translate(-self.translation.x, -self.translation.y);
            list.run(
                &device,
                &ctm_right,
                mupdf::Rect {
                    x0: 0.0,
                    y0: 0.0,
                    x1: bounds.width() * self.scale_factor as f32,
                    y1: bounds.height() * self.scale_factor as f32,
                },
            )?;
        }
        let pix = state.pix.as_mut().unwrap();
        if self.invert_colors {
            cpu_pdf_dark_mode_shader(pix, &self.gradient_cache);
        }

        Ok(())
    }
}

fn generate_gradient_cache(cache: &mut [[u8; 4]; 256], bg_color: &[u8; 4]) {
    let gradient = GradientBuilder::new()
        .colors(&[
            colorgrad::Color::from_rgba8(255, 255, 255, 255),
            colorgrad::Color::from_rgba8(bg_color[0], bg_color[1], bg_color[2], bg_color[3]),
        ])
        .build::<LinearGradient>()
        .unwrap();
    for (i, item) in cache.iter_mut().enumerate().take(256) {
        *item = gradient.at((i as f32) / 255.0).to_rgba8();
    }
}

fn cpu_pdf_dark_mode_shader(pixmap: &mut Pixmap, gradient_cache: &[[u8; 4]; 256]) {
    let samples = pixmap.samples_mut();
    for pixel in samples.chunks_exact_mut(4) {
        let r: u16 = pixel[0] as u16;
        let g: u16 = pixel[1] as u16;
        let b: u16 = pixel[2] as u16;
        let brightness = ((r + g + b) / 3) as usize;
        let pixel_array: &mut [u8; 4] = pixel.try_into().unwrap();
        *pixel_array = gradient_cache[brightness];
    }
}
