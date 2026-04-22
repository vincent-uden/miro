use anyhow::Result;
use colorgrad::{Gradient as _, GradientBuilder, LinearGradient};
use mupdf::{Colorspace, Device, DisplayList, Document, Matrix, Page, Pixmap};
use std::{cell::RefCell, path::PathBuf};
use tracing::{error, info};
use open;

use crate::{
    CONFIG, DARK_THEME,
    app::AppMessage,
    config::MouseAction,
    geometry::{self, Rect, Vector},
    pdf::{
        link_extraction::LinkType,
        outline_extraction::OutlineExtractor,
        text_extraction::{TextExtractor, TextSelection},
    },
};

use super::{PdfMessage, link_extraction::LinkInfo, outline_extraction::OutlineItem};

#[derive(Debug, Clone, Copy)]
pub enum PageLayout {
    /// One page per row, many rows
    SinglePage,
    /// Two pages per row, many rows
    TwoPage,
    /// Two pages per row, many rows, except for the first page which is on its own
    TwoPageTitlePage,
    /// Only one page on the screen at a time
    Presentation,
}

impl PageLayout {
    /// Returns visible pages and their bounding boxes relative to the widgets origin.
    fn pages_rects(
        &self,
        doc: &Document,
        translation: Vector<f32>,
        scale: f32,
        fractional_scale: f64,
        viewport: Rect<f32>,
    ) -> Vec<(Page, Rect<f32>)> {
        todo!("");
    }

    /// Returns the translation that would leave the page at [page_idx] visible on the screen. If
    /// `page_idx > doc.page_count()` this will move to the last page.
    fn translation_for_page(
        &self,
        doc: &Document,
        scale: f32,
        fractional_scale: f64,
        page_idx: usize,
        viewport: Rect<f32>,
    ) -> Vector<f32> {
        todo!("")
    }

    /// Returns the height of the row of pages occupying the middle of the creen
    fn page_set_height(
        &self,
        doc: &Document,
        translation: Vector<f32>,
        scale: f32,
        fractional_scale: f64,
        viewport: Rect<f32>,
    ) -> f32 {
        todo!("")
    }
}
const MIN_SELECTION: f32 = 5.0;
const MIN_CLICK_DISTANCE: f32 = 5.0;

#[derive(Debug)]
pub struct State {
    bounds: Rect<f32>,
}

/// Renders a pdf document. Owns all information related to the document.
#[derive(Debug)]
pub struct PdfViewer {
    pub name: String,
    pub path: PathBuf,
    pub label: String,
    pub page_progress: String,
    pub translation: Vector<f32>, // In document space
    pub invert_colors: bool,
    pub draw_page_borders: bool,
    layout: PageLayout,
    inner_state: RefCell<State>,
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

        let link_hitboxes = extract_all_links(&doc)?;

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

        Ok(Self {
            scale: 1.0,
            scale_factor: 1.0,
            name,
            path,
            label: String::new(),
            page_progress: String::new(),
            translation: Vector { x: 0.0, y: 0.0 },
            invert_colors: CONFIG.read().unwrap().invert_pdf,
            draw_page_borders: CONFIG.read().unwrap().page_borders,
            layout: PageLayout::SinglePage,
            inner_state: RefCell::new(State {
                bounds: Rect::default(),
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
            gradient_cache,
        })
    }

    pub fn update(&mut self, message: PdfMessage) -> iced::Task<PdfMessage> {
        let task: iced::Task<PdfMessage> = match message {
            PdfMessage::PageDown => {
                let bounds = { self.inner_state.borrow().bounds };
                self.set_position(
                    self.translation
                        + Vector::new(
                            0.0,
                            self.layout.page_set_height(
                                &self.doc,
                                self.translation,
                                self.scale,
                                self.scale_factor,
                                bounds,
                            ),
                        ),
                    self.scale,
                );
                iced::Task::none()
            }
            PdfMessage::PageUp => {
                let bounds = { self.inner_state.borrow().bounds };
                self.set_position(
                    self.translation
                        - Vector::new(
                            0.0,
                            // Maybe this should be based on the page set above rather than the
                            // current one?
                            self.layout.page_set_height(
                                &self.doc,
                                self.translation,
                                self.scale,
                                self.scale_factor,
                                bounds,
                            ),
                        ),
                    self.scale,
                );
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
            PdfMessage::None => iced::Task::none(),
            PdfMessage::MouseMoved(vector) => {
                if self.inner_state.borrow().bounds.contains(vector) {
                    if self.panning && self.last_mouse_pos.is_some() {
                        self.translation +=
                            (self.last_mouse_pos.unwrap() - vector).scaled(1.0 / self.scale);
                    }
                    let (page_idx, doc_pos) = self.screen_to_document_coords(vector);
                    self.is_over_link = self
                        .link_hitboxes
                        .iter()
                        .any(|link| link.bounds.contains(doc_pos) && link.page_idx == page_idx);

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
                        && let Some((page_idx, pos)) = self
                            .last_mouse_pos
                            .map(|p| self.screen_to_document_coords(p))
                        && let Some(link) = self
                            .link_hitboxes
                            .iter()
                            .find(|link| link.bounds.contains(pos) && link.page_idx == page_idx)
                    {
                        match link.link_type {
                            LinkType::InternalPage(page) => {
                                if self.set_page(page as usize).is_err() {
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
                            let (start_page, doc_start) = self.screen_to_document_coords(start_pos);
                            let (end_page, doc_end) = self.screen_to_document_coords(end_pos);

                            // FIX: Handle which page the selection is on
                            // Selection might span multiple pages
                            let selection_rect = Rect::from_points(
                                Vector::new(doc_start.x.min(doc_end.x), doc_start.y.min(doc_end.y)),
                                Vector::new(doc_start.x.max(doc_end.x), doc_start.y.max(doc_end.y)),
                            );

                            if selection_rect.width() > MIN_SELECTION
                                && selection_rect.height() > MIN_SELECTION
                            {
                                // FIX: Possibly multiple text extractors here
                                // let extractor = TextExtractor::new(&self.page);
                                // let selection = extractor
                                //     .extract_text_in_rect(selection_rect.into())
                                //     .unwrap();
                                // info!("Copied: \"{}\" at {:?}", selection.text, selection.bounds);
                                // arboard::Clipboard::new().map_or_else(
                                //     |e| error!("{e}"),
                                //     |mut clipboard| {
                                //         clipboard
                                //             .set_text(selection.text)
                                //             .inspect_err(|e| error!("{e}"))
                                //             .unwrap();
                                //     },
                                // )
                            }
                        }
                        self.text_selection_start = None;
                    }
                    (MouseAction::NextPage, true) => {
                        self.update(PdfMessage::PageDown);
                    }
                    (MouseAction::NextPage, false) => {}
                    (MouseAction::PreviousPage, true) => {
                        self.update(PdfMessage::PageUp);
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
                            if self.set_page(page as usize).is_err() {
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
        // TODO : What page number should be shown here? A range? It is reasonable to assume layouts
        // to return contiguous ranges of pages. In the absence of a better solution that might have
        // to do.
        self.label = format!(
            "{} {}/{}",
            self.name,
            0,
            // self.cur_page_idx + 1,
            self.doc.page_count().unwrap_or(0),
        );
        self.page_progress = format!(
            " {}/{}",
            0,
            // self.cur_page_idx + 1,
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

    fn set_page(&mut self, idx: usize) -> Result<()> {
        {
            let inner = self.inner_state.borrow();
            self.translation = self.layout.translation_for_page(
                &self.doc,
                self.scale,
                self.scale_factor,
                idx,
                inner.bounds,
            );
        }

        self.set_position(self.translation, self.scale)
    }

    fn set_position(&mut self, translation: Vector<f32>, scale: f32) -> Result<()> {
        let bounds = {
            let inner = self.inner_state.borrow();
            inner.bounds
        };
        self.translation = translation;
        self.scale = scale;
        let visible_pages = self.layout.pages_rects(
            &self.doc,
            self.translation,
            self.scale,
            self.scale_factor,
            bounds,
        );
        // TODO:
        // Create DisplayLists for pages
        // Old code
        // state.list = DisplayList::new(bounds)?;
        // let list_dev = Device::from_display_list(&state.list)?;
        // let ctm = Matrix::IDENTITY;
        // self.page.run(&list_dev, &ctm)?;
        //
        // TODO:
        // Extract links for visible pages
        // let extractor = LinkExtractor::new(&self.page);
        // self.link_hitboxes = extractor.extract_all_links()?;
        todo!()
    }

    pub fn refresh_file(&mut self) -> Result<()> {
        self.doc = Document::open(&self.path.to_str().unwrap())?;
        let extractor = OutlineExtractor::new(&self.doc);
        self.document_outline = extractor.extract_outline()?;
        self.set_position(self.translation, self.scale)
    }

    fn zoom_fit_ratio(&mut self) -> Result<f32> {
        // TODO: Some implementation based on the current layout.
        //       Maybe a method on PageLayout?
        todo!("")
    }

    /// Returns the coordinates in document space and which page the position is in
    fn screen_to_document_coords(&self, mut screen_pos: Vector<f32>) -> (usize, Vector<f32>) {
        todo!("")
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
            // TODO: Invalidate pixmap to force re-render at new scale factor
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
        // TODO:
        // - Draw each pdf to a different pixmap
        // - Probably store this in inner::State
        todo!()
    }
}

pub fn extract_all_links(doc: &Document) -> Result<Vec<LinkInfo>> {
    todo!()
    // let mut links = Vec::new();

    // let link_iter = self.page.links()?;

    // for link in link_iter {
    //     let bounds = Rect::from_points(
    //         Vector::new(link.bounds.x0, link.bounds.y0),
    //         Vector::new(link.bounds.x1, link.bounds.y1),
    //     );
    //     let link_type = categorize_link(&link);
    //     links.push(LinkInfo {
    //         page_idx: self.idx,
    //         bounds,
    //         uri: link.uri,
    //         link_type,
    //     });
    // }

    // Ok(links)
}

pub fn extract_text_in_rect(page: &Page, selection_rect: mupdf::Rect) -> Result<TextSelection> {
    // let text_page = self.page.to_text_page(TextPageFlags::empty())?;

    // let mut selected_text = String::new();
    // let mut bounds = Vec::new();

    // for block in text_page.blocks() {
    //     for line in block.lines() {
    //         let line_bounds = line.bounds();

    //         if rectangles_intersect(selection_rect, line_bounds) {
    //             for ch in line.chars() {
    //                 let char_quad = ch.quad();
    //                 let char_rect = MupdfRect {
    //                     x0: char_quad.ul.x,
    //                     y0: char_quad.ul.y,
    //                     x1: char_quad.lr.x,
    //                     y1: char_quad.lr.y,
    //                 };

    //                 if rectangles_intersect(selection_rect, char_rect)
    //                     && let Some(c) = ch.char()
    //                 {
    //                     selected_text.push(c);
    //                 }
    //             }
    //             selected_text.push('\n');
    //             bounds.push(line_bounds);
    //         }
    //     }
    // }

    // let total_bounds = bounds
    //     .iter()
    //     .fold(Rect::default(), |acc: Rect<f32>, r| Rect {
    //         x0: Vector {
    //             x: acc.x0.x.min(r.x0),
    //             y: acc.x0.y.min(r.y0),
    //         },
    //         x1: Vector {
    //             x: acc.x1.x.max(r.x1),
    //             y: acc.x1.y.max(r.y1),
    //         },
    //     });

    // Ok(TextSelection {
    //     text: selected_text.trim().to_string(),
    //     bounds: total_bounds,
    // })
    todo!()
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

fn generate_key_combinations(count: usize) -> Vec<String> {
    // Use easily distinguishable characters (excluding confusing ones like 'I', 'l', 'O', '0')
    const CHARS: &[char] = &[
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'j', 'k', 'm', 'n', 'p', 'q', 'r', 's', 't', 'u',
        'v', 'w', 'x', 'y', 'z',
    ];

    let mut keys = Vec::new();

    for &c in CHARS.iter().take(count.min(CHARS.len())) {
        keys.push(c.to_string());
    }

    if count > CHARS.len() {
        let remaining = count - CHARS.len();
        let mut added = 0;
        'outer: for &c1 in CHARS {
            for &c2 in CHARS {
                if added >= remaining {
                    break 'outer;
                }
                keys.push(format!("{}{}", c1, c2));
                added += 1;
            }
        }
    }

    keys
}

fn get_link_colors(link_type: &LinkType) -> (iced::Color, iced::Color) {
    match link_type {
        LinkType::ExternalUrl => (
            iced::Color::from_rgb(0.0, 0.4, 1.0),       // Blue border
            iced::Color::from_rgba(0.0, 0.4, 1.0, 0.1), // Semi-transparent blue fill
        ),
        LinkType::InternalPage(_) => (
            iced::Color::from_rgb(0.0, 0.8, 0.0),       // Green border
            iced::Color::from_rgba(0.0, 0.8, 0.0, 0.1), // Semi-transparent green fill
        ),
        LinkType::Email => (
            iced::Color::from_rgb(1.0, 0.6, 0.0),       // Orange border
            iced::Color::from_rgba(1.0, 0.6, 0.0, 0.1), // Semi-transparent orange fill
        ),
        LinkType::Other => (
            iced::Color::from_rgb(0.5, 0.5, 0.5),       // Gray border
            iced::Color::from_rgba(0.5, 0.5, 0.5, 0.1), // Semi-transparent gray fill
        ),
    }
}

fn get_background_color(invert_colors: bool) -> iced::Color {
    if invert_colors {
        iced::Color::from_rgb8(21, 22, 32)
    } else {
        iced::Color::from_rgb8(220, 219, 218)
    }
}
