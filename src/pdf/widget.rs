use std::{
    cell::RefCell,
    collections::{HashMap},
    path::PathBuf,
    sync::{Arc, Mutex, Weak},
};

use anyhow::Result;
use colorgrad::{Gradient as _, GradientBuilder, LinearGradient};
use iced::{
    Renderer, Size,
    advanced::{graphics::geometry, image},
    widget::{
        self,
        canvas::{self, Cache, Stroke},
    },
};
use iced_aw::iced_fonts::REQUIRED_FONT;
use iced_fonts::required::{RequiredIcons, icon_to_string};

use mupdf::{
    Colorspace, Device, Matrix, Pixmap, TextPageFlags,
    pdf::{PdfAnnotationType, PdfPage},
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    CONFIG, DARK_THEME,
    config::{MOVE_STEP, MouseAction},
    geometry::{Rect, Vector},
    pdf::{PdfMessage, SearchMatch, SearchMethod, find_search_matches, page_layout::PageLayout},
};

#[derive(Debug, Clone)]
struct PageLink {
    bounds: mupdf::Rect,
    uri: String,
    dest: Option<mupdf::link::LinkDestination>,
}

#[derive(Debug, Clone)]
struct Comment {
    page_idx: usize,
    bounds: mupdf::Rect,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineItem {
    pub title: String,
    pub page: Option<u32>,
    pub level: u32,
    pub children: Vec<OutlineItem>,
}

const MIN_SELECTION: f32 = 5.0;
const MIN_CLICK_DISTANCE: f32 = 5.0;

/// A pixel buffer that returns itself to a shared pool when dropped.
///
/// Allocation pressure is the motivating concern: a single 4K page at 2× scale
/// is ~64 MiB of RGBA data. Doing that per frame during zoom or pan causes
/// severe allocator churn, so the pool turns allocation into zero-cost reuse
/// after warmup.
#[derive(Debug)]
struct PooledBuffer {
    buf: Option<Vec<u8>>,
    pool: Weak<Mutex<HashMap<usize, Vec<Vec<u8>>>>>,
    page_idx: usize,
}

impl AsRef<[u8]> for PooledBuffer {
    fn as_ref(&self) -> &[u8] {
        self.buf.as_ref().expect("Buffer should not be None")
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        // Returning the buffer on Drop lets us recycle the allocation without
        // forcing callers to manage a manual release path.
        if let Some(buf) = self.buf.take()
            && let Some(pool) = self.pool.upgrade()
            && let Ok(mut pool) = pool.lock()
        {
            pool.entry(self.page_idx).or_default().push(buf);
        }
    }
}

type BufferPool = Arc<Mutex<HashMap<usize, Vec<Vec<u8>>>>>;

/// Cache key for rendered page images.
///
/// - `Full` is used when the entire page fits inside the viewport. The cached
///   image is independent of translation so panning does not trigger re-renders.
/// - `Partial` is used when only a sub-rect of the page is visible. The key
///   includes the visible rectangle (in viewport pixels) so that any pan or
///   zoom invalidates the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RenderKey {
    Full(usize, u32),
    Partial(usize, u32, i32, i32, i32, i32),
}

struct Document {
    cache: Cache,
    pages: Vec<(image::Handle, Rect<f32>)>,
    draw_page_borders: bool,
    pdf_dark_mode: bool,
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("cache", &self.cache)
            .field("page_count", &self.pages.len())
            .finish()
    }
}

impl Document {
    pub fn new(
        pages: Vec<(image::Handle, Rect<f32>)>,
        draw_page_borders: bool,
        pdf_dark_mode: bool,
    ) -> Self {
        Self {
            cache: Cache::default(),
            pages,
            draw_page_borders,
            pdf_dark_mode,
        }
    }
}

impl widget::canvas::Program<PdfMessage> for Document {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let _span = tracy_client::span!("Pdf draw");
        let bg = self.cache.draw(renderer, bounds.size(), |frame| {
            let bg_color = get_pdf_background_color(self.pdf_dark_mode, self.draw_page_borders);
            frame.fill_rectangle(iced::Point::new(0.0, 0.0), bounds.size(), bg_color);

            for (handle, rect) in &self.pages {
                let bounds: iced::Rectangle = (*rect).into();
                frame.draw_image(bounds, handle);
            }
        });
        vec![bg]
    }
}

#[derive(Debug)]
struct SelectionOverlay<'a> {
    viewer: &'a PdfViewer,
}

impl<'a> SelectionOverlay<'a> {
    fn new(viewer: &'a PdfViewer) -> Self {
        Self { viewer }
    }
}

impl<'a> widget::canvas::Program<PdfMessage> for SelectionOverlay<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let Some(selection) = self.viewer.selection_rect() else {
            return Vec::new();
        };

        let viewport = bounds.size();

        let mut frame = canvas::Frame::new(renderer, viewport);

        let mut color = iced::Color::from_rgb(0.0, 0.4, 0.8);
        color.a = 0.25;
        frame.fill_rectangle(selection.x0.into(), selection.size().into(), color);

        vec![frame.into_geometry()]
    }
}

#[derive(Debug, Default)]
struct InteractiveOverlayState {
    /// Accumulator for keyboard driven-link activation
    pending_key: String,
    /// Keeps track for toggle link hitboxes events
    was_active: bool,
}

#[derive(Debug)]
struct InteractiveOverlay<'a> {
    viewer: &'a PdfViewer,
}

impl<'a> InteractiveOverlay<'a> {
    fn new(viewer: &'a PdfViewer) -> Self {
        Self { viewer }
    }
}

impl<'a> widget::canvas::Program<PdfMessage> for InteractiveOverlay<'a> {
    type State = InteractiveOverlayState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        _bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> (iced::event::Status, Option<PdfMessage>) {
        if let canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key, modifiers, ..
        }) = event.clone()
        {
            if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
                && !modifiers.control()
                && !modifiers.alt()
                && !modifiers.logo()
            {
                if self.viewer.active_comment.is_some() {
                    return (
                        iced::event::Status::Captured,
                        Some(PdfMessage::CloseComment),
                    );
                }
            }
        }

        if !self.viewer.show_link_hitboxes {
            state.was_active = false;
            return (iced::event::Status::Ignored, None);
        }

        if !state.was_active {
            state.pending_key.clear();
        }
        state.was_active = true;

        if let canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key, modifiers, ..
        }) = event
        {
            if modifiers.control() || modifiers.alt() || modifiers.logo() {
                return (iced::event::Status::Ignored, None);
            }

            if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                return (
                    iced::event::Status::Captured,
                    Some(PdfMessage::CloseLinkHitboxes),
                );
            }

            if let iced::keyboard::Key::Character(c) = key {
                let ch = c.to_lowercase().to_string();
                state.pending_key.push_str(&ch);

                let viewport = *self.viewer.viewport.borrow();
                let link_visible = self.viewer.visible_links(viewport);
                let keys = generate_key_combinations(link_visible.len());

                if let Some(idx) = keys.iter().position(|k| k == &state.pending_key) {
                    state.pending_key.clear();
                    return (
                        iced::event::Status::Captured,
                        Some(PdfMessage::ActivateLink(idx)),
                    );
                }

                let is_prefix = keys.iter().any(|k| k.starts_with(&state.pending_key));
                if is_prefix {
                    return (iced::event::Status::Captured, None);
                }

                state.pending_key.clear();
                return (iced::event::Status::Captured, None);
            }
        }

        (iced::event::Status::Ignored, None)
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        *self.viewer.widget_position.borrow_mut() = bounds.position();
        let viewport = bounds.size();
        let link_visible = self.viewer.visible_links(viewport);
        let search_visible = self.viewer.visible_search_results(viewport);
        let comment_visible = self.viewer.visible_comments(viewport);
        if link_visible.is_empty()
            && search_visible.is_empty()
            && comment_visible.is_empty()
            && self.viewer.hovered_link.is_none()
            && self.viewer.hovered_search_result.is_none()
            && self.viewer.hovered_comment.is_none()
        {
            return Vec::new();
        }

        let mut frame = canvas::Frame::new(renderer, viewport);

        // Draw search results first (behind links).
        for (match_idx, rect) in &search_visible {
            let is_hovered = self
                .viewer
                .hovered_search_result
                .as_ref()
                .is_some_and(|h| h == match_idx);
            let mut color = if is_hovered {
                iced::Color::from_rgb(1.0, 0.5, 0.0)
            } else {
                iced::Color::from_rgb(1.0, 0.8, 0.2)
            };
            color.a = if is_hovered { 0.35 } else { 0.2 };
            frame.fill_rectangle(rect.x0.into(), rect.size().into(), color);
        }

        // Draw hovered link fill.
        if let Some((page_idx, link_idx)) = self.viewer.hovered_link {
            if let Some((_, rect)) = link_visible
                .iter()
                .find(|((p, l), _)| *p == page_idx && *l == link_idx)
            {
                let mut color = iced::Color::from_rgb(0.0, 0.4, 0.8);
                color.a = 0.15;
                frame.fill_rectangle(rect.x0.into(), rect.size().into(), color);
            }
        }

        // Draw link hitbox mode.
        if self.viewer.show_link_hitboxes {
            let keys = generate_key_combinations(link_visible.len());
            for (((_page_idx, _link_idx), rect), key) in link_visible.iter().zip(keys.iter()) {
                let mut fill_color = iced::Color::from_rgb(0.9, 0.3, 0.1);
                fill_color.a = 0.2;
                frame.fill_rectangle(rect.x0.into(), rect.size().into(), fill_color);

                let stroke_color = iced::Color::from_rgb(0.9, 0.3, 0.1);
                frame.stroke_rectangle(
                    rect.x0.into(),
                    rect.size().into(),
                    Stroke::default().with_color(stroke_color).with_width(1.5),
                );

                let text_size = 16.0;
                let padding = 3.0;
                let approx_char_w = text_size * 0.6;
                let bg_w = approx_char_w * key.len() as f32 + padding * 2.0;
                let bg_h = text_size + padding;
                let bg_x = rect.x1.x + 2.0;
                let bg_y = rect.center().y - bg_h / 2.0;
                frame.fill_rectangle(
                    iced::Point::new(bg_x, bg_y),
                    iced::Size::new(bg_w, bg_h),
                    iced::Color::from_rgb(0.1, 0.1, 0.1),
                );

                frame.fill_text(geometry::Text {
                    content: key.clone(),
                    position: iced::Point::new(bg_x + bg_w / 2.0, bg_y + bg_h / 2.0),
                    color: iced::Color::WHITE,
                    size: text_size.into(),
                    line_height: widget::text::LineHeight::Relative(1.0),
                    font: iced::Font::default(),
                    horizontal_alignment: iced::alignment::Horizontal::Center,
                    vertical_alignment: iced::alignment::Vertical::Center,
                    shaping: widget::text::Shaping::Basic,
                });
            }
        }

        // Draw hovered comment indicator.
        if let Some(comment_idx) = self.viewer.hovered_comment {
            if let Some((_, rect)) = comment_visible.iter().find(|(idx, _)| *idx == comment_idx) {
                let mut color = iced::Color::from_rgb(1.0, 0.9, 0.0);
                color.a = 0.25;
                frame.fill_rectangle(rect.x0.into(), rect.size().into(), color);
                let stroke_color = iced::Color::from_rgb(1.0, 0.9, 0.0);
                frame.stroke_rectangle(
                    rect.x0.into(),
                    rect.size().into(),
                    Stroke::default().with_color(stroke_color).with_width(1.5),
                );
            }
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        _bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> iced::advanced::mouse::Interaction {
        if self.viewer.hovered_link.is_some()
            || self.viewer.hovered_search_result.is_some()
            || self.viewer.hovered_comment.is_some()
        {
            iced::advanced::mouse::Interaction::Pointer
        } else {
            iced::advanced::mouse::Interaction::default()
        }
    }
}

#[derive(Debug)]
pub enum MouseInteraction {
    None,
    Panning,
    Selecting,
}

/// A pixmap is cached by its page number and the zoom level at which it was generated.
/// Renders a pdf document. Owns all information related to the document.
#[derive(Debug)]
pub struct PdfViewer {
    pub name: String,
    pub path: PathBuf,

    pdf_dark_mode: bool,
    interface_dark_mode: bool,
    pub draw_page_borders: bool,

    doc: mupdf::Document,
    display_lists: Vec<mupdf::DisplayList>,
    /// Final iced image handles cached by render key. Kept separately so iced can reuse the
    /// GPU texture without re-uploading when the widget redraws for non-visual reasons.
    render_cache: RefCell<HashMap<RenderKey, image::Handle>>,
    /// Reusable MuPDF pixmaps keyed by page. These are expensive to allocate and are not Send,
    /// so we pool them separately from the plain CPU buffers.
    pixmap_pool: RefCell<HashMap<usize, Pixmap>>,
    /// Plain CPU buffers returned by dropped images and shared across threads. MuPDF data is not
    /// thread-safe, but iced may render on any thread, so we must copy into a Vec<u8> and pool
    /// it to avoid allocating multi-megabyte buffers on every frame during zoom or pan.
    buffer_pool: BufferPool,

    pub translation: Vector<f32>,
    pub scale: f32,
    fractional_scaling: f32,

    viewport: RefCell<Size<f32>>,

    mouse_pos: Vector<f32>,
    mouse_pressed_at: Vector<f32>,
    mouse_interaction: MouseInteraction,

    selection_start: Option<Vector<f32>>,
    selection_end: Option<Vector<f32>>,
    selected_text: String,

    layout: PageLayout,

    gradient_cache: [[u8; 4]; 256],

    show_link_hitboxes: bool,
    links: Vec<Vec<PageLink>>,
    hovered_link: Option<(usize, usize)>,

    show_search_results: bool,
    hovered_search_result: Option<usize>,
    current_search_result: Option<usize>,

    outline: Vec<OutlineItem>,

    /// The entire textual contents of the document. Used to search through text
    text_contents: String,
    /// Bounding boxes of every character in the document. Used to highlight searched text
    /// Each entry is (page_index, byte_offset_in_text_contents, bounding_box)
    char_bboxes: Vec<(usize, usize, Rect<f32>)>,
    /// The search matches found in the document
    search_matches: Vec<SearchMatch>,
    pub(crate) search_method: SearchMethod,
    /// The thing to search for
    pub(crate) needle: String,
    /// Monotonically incremented to cancel stale async search tasks.
    search_generation: u64,

    /// All text annotations (sticky notes / comments) extracted from the document.
    comments: Vec<Comment>,
    hovered_comment: Option<usize>,
    active_comment: Option<usize>,

    /// The widget's position in window coordinates, updated each frame by the overlay draw.
    widget_position: RefCell<iced::Point>,
}

impl PdfViewer {
    fn build_document_data(
        doc: &mupdf::Document,
    ) -> Result<(
        Vec<mupdf::DisplayList>,
        Vec<Vec<PageLink>>,
        Vec<OutlineItem>,
        Vec<Comment>,
    )> {
        let mut display_lists = vec![];
        let mut links = vec![];
        let mut comments = vec![];
        for (page_idx, page) in doc.pages()?.flatten().enumerate() {
            let dl = mupdf::DisplayList::new(page.bounds()?)?;
            let dummy_device = Device::from_display_list(&dl)?;
            let ctm = Matrix::IDENTITY;
            page.run(&dummy_device, &ctm)?;
            display_lists.push(dl);

            let page_links: Vec<PageLink> = page
                .links()?
                .map(|link| PageLink {
                    bounds: link.bounds,
                    uri: link.uri,
                    dest: link.dest,
                })
                .collect();
            links.push(page_links);

            if let Ok(pdf_page) = PdfPage::try_from(page) {
                for ann in pdf_page.annotations() {
                    let Ok(PdfAnnotationType::Text) = ann.r#type() else { continue };
                    let Ok(Some(content)) = ann.contents() else { continue };
                    let Ok(bounds) = ann.rect() else { continue };
                    comments.push(Comment {
                        page_idx,
                        bounds,
                        content: content.to_string(),
                    });
                }
            }
        }
        let outline = Self::extract_outline(doc).unwrap_or_default();
        Ok((display_lists, links, outline, comments))
    }

    pub fn from_path(path: PathBuf) -> Result<Self> {
        let name = path
            .file_name()
            .expect("The pdf must have a file name")
            .to_string_lossy()
            .to_string();
        let doc = mupdf::Document::open(&path.to_str().unwrap())?;
        let (display_lists, links, outline, comments) = Self::build_document_data(&doc)?;
        let (all_text, bboxes) = Self::extract_search_data(&display_lists)?;
        info!(
            "Document contains {} chars",
            Self::count_chars(&display_lists).unwrap()
        );
        println!(
            "Document contains {} chars",
            Self::count_chars(&display_lists).unwrap()
        );

        let bg_color = DARK_THEME
            .extended_palette()
            .background
            .base
            .color
            .into_rgba8();
        let mut gradient_cache = [[0; 4]; 256];
        generate_gradient_cache(&mut gradient_cache, &bg_color);

        Ok(PdfViewer {
            name,
            path,
            pdf_dark_mode: false,
            interface_dark_mode: false,
            draw_page_borders: true,
            doc,
            display_lists,
            render_cache: RefCell::default(),
            pixmap_pool: RefCell::default(),
            buffer_pool: Arc::new(Mutex::new(HashMap::new())),
            translation: Vector::zero(),
            scale: 1.0,
            fractional_scaling: 1.0,
            viewport: RefCell::default(),
            layout: PageLayout::SinglePage,
            gradient_cache,
            mouse_pos: Vector::zero(),
            mouse_pressed_at: Vector::zero(),
            mouse_interaction: MouseInteraction::None,
            selection_start: None,
            selection_end: None,
            selected_text: String::new(),
            show_link_hitboxes: false,
            links,
            hovered_link: None,
            show_search_results: false,
            hovered_search_result: None,
            current_search_result: None,
            outline,
            widget_position: RefCell::new(iced::Point::new(0.0, 0.0)),
            text_contents: all_text,
            char_bboxes: bboxes,
            search_matches: vec![],
            search_method: CONFIG.read().unwrap().default_search_method,
            needle: String::new(),
            search_generation: 0,
            comments,
            hovered_comment: None,
            active_comment: None,
        })
    }
}

impl PdfViewer {
    pub fn update(&mut self, msg: PdfMessage) -> iced::Task<PdfMessage> {
        let mut out = iced::Task::none();
        let page_count = self.doc.page_count().unwrap() as usize;
        match msg {
            PdfMessage::NextPage => {
                let current = self
                    .layout
                    .center_of_page(&self.doc, self.translation, *self.viewport.borrow())
                    .unwrap();
                let next = self
                    .layout
                    .center_of_page_below(&self.doc, self.translation, *self.viewport.borrow())
                    .unwrap();

                self.translation.y += next.center().y - current.center().y;
            }
            PdfMessage::PreviousPage => {
                let current = self
                    .layout
                    .center_of_page(&self.doc, self.translation, *self.viewport.borrow())
                    .unwrap();
                let prev = self
                    .layout
                    .center_of_page_above(&self.doc, self.translation, *self.viewport.borrow())
                    .unwrap();

                self.translation.y += prev.center().y - current.center().y;
            }
            PdfMessage::SetPage(idx) => {
                if idx < page_count
                    && let Ok(translation) = self.layout.translation_for_page(
                        &self.doc,
                        self.scale,
                        self.fractional_scaling,
                        idx,
                        *self.viewport.borrow(),
                    )
                {
                    self.translation = translation;
                }
            }
            PdfMessage::SetTranslation(vector) => {
                self.translation = vector;
            }
            PdfMessage::SetLocation(vector, scale) => {
                self.translation = vector;
                self.scale = scale;
            }
            PdfMessage::SetLayout(page_layout) => {
                self.layout = page_layout;
            }
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
                let page_idx = self.current_page();
                if let Some(display_list) = self.display_lists.get(page_idx) {
                    let page_bounds = display_list.bounds();
                    let page_width = page_bounds.x1 - page_bounds.x0;
                    let page_height = page_bounds.y1 - page_bounds.y0;
                    if page_width > 0.0 && page_height > 0.0 {
                        let viewport = *self.viewport.borrow();
                        if viewport.width > 0.0 && viewport.height > 0.0 {
                            let scale_x = viewport.width / page_width;
                            let scale_y = viewport.height / page_height;
                            self.scale = scale_x.min(scale_y) / self.fractional_scaling;
                            if let Ok(translation) = self.layout.translation_for_page(
                                &self.doc,
                                self.scale,
                                self.fractional_scaling,
                                page_idx,
                                viewport,
                            ) {
                                self.translation = translation;
                            }
                        }
                    }
                }
            }
            PdfMessage::Move(vector) => {
                self.translation += vector;
            }
            PdfMessage::MouseMoved(vector) => {
                let old_local = self.local_mouse_pos();
                self.mouse_pos = vector;
                let new_local = self.local_mouse_pos();
                match self.mouse_interaction {
                    MouseInteraction::None => {}
                    MouseInteraction::Panning => {
                        out = iced::Task::done(PdfMessage::Move(
                            (old_local - new_local)
                                .scaled(1.0 / (self.scale * self.fractional_scaling)),
                        ))
                    }
                    MouseInteraction::Selecting => {
                        self.selection_end = Some(new_local);
                    }
                }
                self.update_hover_state();
            }
            PdfMessage::MouseAction(mouse_action, pressed) => {
                if pressed {
                    match mouse_action {
                        MouseAction::Panning => {
                            self.mouse_interaction = MouseInteraction::Panning;
                            self.mouse_pressed_at = self.mouse_pos;
                            self.selection_start = None;
                            self.selection_end = None;
                        }
                        MouseAction::Selection => {
                            self.mouse_interaction = MouseInteraction::Selecting;
                            self.mouse_pressed_at = self.mouse_pos;
                            let local = self.local_mouse_pos();
                            self.selection_start = Some(local);
                            self.selection_end = Some(local);
                            self.selected_text.clear();
                        }
                        MouseAction::NextPage => {
                            out = iced::Task::done(PdfMessage::NextPage);
                        }
                        MouseAction::PreviousPage => {
                            out = iced::Task::done(PdfMessage::PreviousPage);
                        }
                        MouseAction::ZoomIn => {
                            out = iced::Task::done(PdfMessage::ZoomIn);
                        }
                        MouseAction::ZoomOut => {
                            out = iced::Task::done(PdfMessage::ZoomOut);
                        }
                        MouseAction::MoveUp => {
                            out = iced::Task::done(PdfMessage::Move(Vector::new(0.0, -MOVE_STEP)));
                        }
                        MouseAction::MoveDown => {
                            out = iced::Task::done(PdfMessage::Move(Vector::new(0.0, MOVE_STEP)));
                        }
                        MouseAction::MoveLeft => {
                            out = iced::Task::done(PdfMessage::Move(Vector::new(-MOVE_STEP, 0.0)));
                        }
                        MouseAction::MoveRight => {
                            out = iced::Task::done(PdfMessage::Move(Vector::new(MOVE_STEP, 0.0)));
                        }
                    }
                } else {
                    match self.mouse_interaction {
                        MouseInteraction::None | MouseInteraction::Panning => {
                            let dist_sq = (self.mouse_pos - self.mouse_pressed_at).norm_squared();
                            if dist_sq < MIN_CLICK_DISTANCE * MIN_CLICK_DISTANCE {
                                if let Some((page_idx, link_idx)) = self.hovered_link {
                                    out = self.activate_link(page_idx, link_idx);
                                } else if let Some(match_idx) = self.hovered_search_result {
                                    if let Some(m) = self.search_matches.get(match_idx) {
                                        let text = self.text_contents[m.start_byte..m.end_byte]
                                            .to_string();
                                        out = iced::Task::perform(
                                            async move {
                                                if let Ok(mut clipboard) = arboard::Clipboard::new()
                                                {
                                                    if let Err(e) = clipboard.set_text(text) {
                                                        error!(
                                                            "Failed to copy search result to clipboard: {}",
                                                            e
                                                        );
                                                    }
                                                }
                                            },
                                            |_| PdfMessage::None,
                                        );
                                    }
                                } else if let Some(comment_idx) = self.hovered_comment {
                                    if self.active_comment == Some(comment_idx) {
                                        self.active_comment = None;
                                    } else {
                                        self.active_comment = Some(comment_idx);
                                    }
                                } else {
                                    self.active_comment = None;
                                }
                            }
                        }
                        MouseInteraction::Selecting => {
                            if let (Some(start), Some(end)) =
                                (self.selection_start, self.selection_end)
                            {
                                let min = Vector::new(start.x.min(end.x), start.y.min(end.y));
                                let max = Vector::new(start.x.max(end.x), start.y.max(end.y));
                                if (max - min).norm_squared() >= MIN_SELECTION * MIN_SELECTION {
                                    let selection_rect = Rect::from_points(min, max);
                                    self.selected_text =
                                        self.extract_text_from_rect(selection_rect);
                                }
                            }
                        }
                    }
                    self.selection_start = None;
                    self.selection_end = None;
                    self.mouse_interaction = MouseInteraction::None;
                }
            }
            PdfMessage::ToggleLinkHitboxes => {
                self.show_link_hitboxes = !self.show_link_hitboxes;
            }
            PdfMessage::ActivateLink(idx) => {
                let viewport = *self.viewport.borrow();
                let visible = self.visible_links(viewport);
                if let Some(((page_idx, link_idx), _)) = visible.get(idx) {
                    out = self.activate_link(*page_idx, *link_idx);
                }
            }
            PdfMessage::CloseLinkHitboxes => {
                self.show_link_hitboxes = false;
            }
            PdfMessage::CloseComment => {
                self.active_comment = None;
            }
            PdfMessage::FileChanged => {
                self.render_cache.borrow_mut().clear();
                self.pixmap_pool.borrow_mut().clear();

                if let Some(path_str) = self.path.to_str() {
                    if let Ok(new_doc) = mupdf::Document::open(path_str) {
                        if let Ok((display_lists, links, outline, comments)) =
                            Self::build_document_data(&new_doc)
                        {
                            self.doc = new_doc;
                            self.display_lists = display_lists;
                            self.links = links;
                            self.outline = outline;
                            self.comments = comments;
                            self.active_comment = None;
                            self.hovered_comment = None;
                        }
                    }
                }
            }
            PdfMessage::PrintPdf => {
                let path = self.path.clone();
                out = iced::Task::perform(
                    async move {
                        let file_url = format!("file://{}", path.to_string_lossy());
                        if let Err(e) = webbrowser::open(&file_url) {
                            error!("Failed to open PDF in default browser: {}", e);
                        }
                    },
                    |_| PdfMessage::None,
                );
            }
            PdfMessage::PageUp => {
                let vp = self.viewport.borrow();
                out = iced::Task::done(PdfMessage::Move(Vector::new(
                    0.0,
                    -vp.height / (self.scale * self.fractional_scaling),
                )));
            }
            PdfMessage::PageDown => {
                let vp = self.viewport.borrow();
                out = iced::Task::done(PdfMessage::Move(Vector::new(
                    0.0,
                    vp.height / (self.scale * self.fractional_scaling),
                )));
            }
            PdfMessage::HalfPageUp => {
                let vp = self.viewport.borrow();
                out = iced::Task::done(PdfMessage::Move(Vector::new(
                    0.0,
                    -vp.height / (self.scale * self.fractional_scaling * 2.0),
                )));
            }
            PdfMessage::HalfPageDown => {
                let vp = self.viewport.borrow();
                out = iced::Task::done(PdfMessage::Move(Vector::new(
                    0.0,
                    vp.height / (self.scale * self.fractional_scaling * 2.0),
                )));
            }
            PdfMessage::HighlightSearchResults => {
                self.show_search_results = true;
            }
            PdfMessage::HideSearchResults => {
                self.show_search_results = false;
            }
            PdfMessage::JumpToSearchResult(idx) => {
                if let Some(m) = self.search_matches.get(idx) {
                    self.current_search_result = Some(idx);
                    let page_idx = m.pages.start;
                    if let Ok(base_translation) = self.layout.translation_for_page(
                        &self.doc,
                        self.scale,
                        self.fractional_scaling,
                        page_idx,
                        *self.viewport.borrow(),
                    ) {
                        let page_bounds: Rect<f32> = self.display_lists[page_idx].bounds().into();
                        let page_center = page_bounds.center();
                        let match_rect = m.rects[0].1;
                        let match_center = match_rect.center();
                        // Center vertically.
                        self.translation.y = base_translation.y + (match_center.y - page_center.y);
                        // Horizontal: adjust minimally from current pan to keep match visible.
                        let viewport = *self.viewport.borrow();
                        let effective_scale = self.scale * self.fractional_scaling;
                        let half_viewport = viewport.width / (2.0 * effective_scale);
                        let lower_bound = match_rect.x1.x - page_center.x - half_viewport;
                        let upper_bound = match_rect.x0.x - page_center.x + half_viewport;
                        if lower_bound > upper_bound {
                            // Wider than viewport: center horizontally.
                            self.translation.x = match_center.x - page_center.x;
                        } else if self.translation.x < lower_bound {
                            self.translation.x = lower_bound;
                        } else if self.translation.x > upper_bound {
                            self.translation.x = upper_bound;
                        }
                    }
                }
            }
            PdfMessage::NextSearchResult => {
                if !self.search_matches.is_empty() {
                    let idx = match self.current_search_result {
                        Some(current) => (current + 1) % self.search_matches.len(),
                        None => 0,
                    };
                    out = iced::Task::done(PdfMessage::JumpToSearchResult(idx));
                }
            }
            PdfMessage::PreviousSearchResult => {
                if !self.search_matches.is_empty() {
                    let len = self.search_matches.len();
                    let idx = match self.current_search_result {
                        Some(current) => (current + len - 1) % len,
                        None => len - 1,
                    };
                    out = iced::Task::done(PdfMessage::JumpToSearchResult(idx));
                }
            }
            PdfMessage::UpdateSearchNeedle(needle) => {
                self.needle = needle;
                self.search_generation = self.search_generation.wrapping_add(1);
                out = self.spawn_search_task();
            }
            PdfMessage::SetSearchMethod(search_method) => {
                self.search_method = search_method;
                out = iced::Task::done(PdfMessage::UpdateSearchNeedle(self.needle.clone()))
            }
            PdfMessage::ToggleSearchMethod => {
                self.search_method = match self.search_method {
                    SearchMethod::PlainText => SearchMethod::Regex,
                    SearchMethod::Regex => SearchMethod::PlainText,
                };
                self.search_generation = self.search_generation.wrapping_add(1);
                out = self.spawn_search_task();
            }
            PdfMessage::SearchResultsReady(matches, generation) => {
                if generation == self.search_generation {
                    self.search_matches = matches;
                    self.current_search_result = None;
                }
            }
            PdfMessage::None => {}
        }
        out
    }

    pub fn view(&self) -> iced::Element<'_, PdfMessage> {
        widget::responsive(|size| {
            {
                let mut viewport = self.viewport.borrow_mut();
                *viewport = size;
            }
            let rects = self
                .layout
                .pages_rects(
                    self.doc.pages().unwrap(),
                    self.translation.scaled(-1.0),
                    self.scale,
                    self.fractional_scaling,
                    size,
                )
                .unwrap();
            let viewport_rect =
                Rect::from_pos_size(Vector::zero(), Vector::new(size.width, size.height));

            let effective_scale = self.scale * self.fractional_scaling;

            // Drop pixmap allocations for pages that are no longer visible.
            let visible_indices: Vec<usize> = rects
                .iter()
                .enumerate()
                .filter(|(_, r)| viewport_rect.intersects(r))
                .map(|(i, _)| i)
                .collect();
            self.pixmap_pool
                .borrow_mut()
                .retain(|idx, _| visible_indices.contains(idx));

            let with_handles: Vec<_> = rects
                .into_iter()
                .zip(self.doc.pages().unwrap())
                .enumerate()
                .filter(|(_, (r, _page))| viewport_rect.intersects(r))
                .map(|(i, (rect_ss, page))| {
                    // rect_ss = A pages bounding box in screen coordinates (relative to the widgets origin)
                    let page = page.unwrap();
                    let page_bounds: Rect<f32> = page.bounds().unwrap().into();

                    let fully_visible = rect_ss.x0.x >= 0.0
                        && rect_ss.x1.x <= viewport_rect.x1.x
                        && rect_ss.x0.y >= 0.0
                        && rect_ss.x1.y <= viewport_rect.x1.y;

                    let (key, draw_rect, w, h, matrix, scissor) = if fully_visible {
                        let key = RenderKey::Full(i, effective_scale.to_bits());
                        let w = rect_ss.width().ceil().max(1.0) as i32;
                        let h = rect_ss.height().ceil().max(1.0) as i32;
                        let tx = -page_bounds.x0.x * effective_scale;
                        let ty = -page_bounds.x0.y * effective_scale;
                        let matrix =
                            Matrix::new(effective_scale, 0.0, 0.0, effective_scale, tx, ty);
                        let scissor = mupdf::Rect::new(0.0, 0.0, w as f32, h as f32);
                        (key, rect_ss, w, h, matrix, scissor)
                    } else {
                        let vis = rect_ss.intersect(&viewport_rect);
                        let vw = vis.width().ceil().max(1.0) as i32;
                        let vh = vis.height().ceil().max(1.0) as i32;

                        let render_offset_x = rect_ss.x0.x - vis.x0.x;
                        let render_offset_y = rect_ss.x0.y - vis.x0.y;

                        // During a smooth pan the translation changes by sub-pixel amounts every
                        // frame. Using raw floats as a cache key would force a full re-render on
                        // every mouse event because the key would differ each time. We snap the
                        // offset to whole pixels so the cached image survives small pans, and
                        // compensate the draw rectangle by the rounding error so the visual
                        // position stays accurate without paying the render cost.
                        let snapped_offset_x = render_offset_x.round();
                        let snapped_offset_y = render_offset_y.round();

                        let key = RenderKey::Partial(
                            i,
                            effective_scale.to_bits(),
                            snapped_offset_x as i32,
                            snapped_offset_y as i32,
                            vw,
                            vh,
                        );

                        let raster_tx = snapped_offset_x - page_bounds.x0.x * effective_scale;
                        let raster_ty = snapped_offset_y - page_bounds.x0.y * effective_scale;
                        let matrix = Matrix::new(
                            effective_scale,
                            0.0,
                            0.0,
                            effective_scale,
                            raster_tx,
                            raster_ty,
                        );
                        let scissor = mupdf::Rect::new(0.0, 0.0, vw as f32, vh as f32);

                        // Compensate for snapping so the image is drawn at the
                        // correct sub-pixel position. draw_x = r.x0.x - snapped_offset_x
                        // which is the rounding error in [-0.5, 0.5].
                        let draw_rect = Rect::from_pos_size(
                            Vector::new(
                                rect_ss.x0.x - snapped_offset_x,
                                rect_ss.x0.y - snapped_offset_y,
                            ),
                            Vector::new(vw as f32, vh as f32),
                        );

                        (key, draw_rect, vw, vh, matrix, scissor)
                    };

                    let mut cache = self.render_cache.borrow_mut();
                    let handle = cache
                        .entry(key)
                        .or_insert_with(|| {
                            let _span = tracy_client::span!("Pdf cache miss");

                            // Try to reuse a pixmap allocation for this page.
                            let mut pool = self.pixmap_pool.borrow_mut();
                            let mut pix = pool.remove(&i).unwrap_or_else(|| {
                                Pixmap::new_with_w_h(&Colorspace::device_rgb(), w, h, true).unwrap()
                            });

                            // If the pooled pixmap has the wrong size, allocate a new one.
                            if pix.width() as i32 != w || pix.height() as i32 != h {
                                let _span = tracy_client::span!("Pixmap bounds mismatch");
                                pix = Pixmap::new_with_w_h(&Colorspace::device_rgb(), w, h, true)
                                    .unwrap();
                            }

                            // MuPDF only overwrites pixels actually touched by page content.
                            // Margins or transparent regions would keep stale data from the
                            // previous pool user (or be uninitialized). Filling with white
                            // guarantees the paper background that PDFs assume.
                            pix.samples_mut().fill(255);
                            let device = Device::from_pixmap(&pix).unwrap();
                            self.display_lists[i]
                                .run(&device, &matrix, scissor)
                                .unwrap();

                            if self.pdf_dark_mode {
                                cpu_pdf_dark_mode_shader(&mut pix, &self.gradient_cache);
                            }

                            let samples = pix.samples();

                            // NOTE: We have to copy the data at least once since the mupdf structures
                            // NOTE: and their associated data aren't thread safe. Iced could render
                            // NOTE: them on any thread without my control

                            // Try to reuse a CPU buffer from the shared pool.
                            let mut buf = self
                                .buffer_pool
                                .lock()
                                .unwrap()
                                .remove(&i)
                                .and_then(|mut v| v.pop())
                                .unwrap_or_else(|| Vec::with_capacity(samples.len()));
                            buf.clear();
                            buf.extend_from_slice(samples);
                            // Return the mupdf pixmap to the pool for reuse.
                            pool.insert(i, pix);

                            image::Handle::from_rgba(
                                w as u32,
                                h as u32,
                                image::Bytes::from_owner(PooledBuffer {
                                    buf: Some(buf),
                                    pool: Arc::downgrade(&self.buffer_pool),
                                    page_idx: i,
                                }),
                            )
                        })
                        .clone();
                    (handle, draw_rect)
                })
                .collect();

            let pages_canvas = widget::canvas(Document::new(
                with_handles,
                self.draw_page_borders,
                self.pdf_dark_mode,
            ))
            .width(iced::Length::Fill)
            .height(iced::Length::Fill);

            let selection_overlay = widget::canvas(SelectionOverlay::new(self))
                .width(iced::Length::Fill)
                .height(iced::Length::Fill);

            let interactive_overlay = widget::canvas(InteractiveOverlay::new(self))
                .width(iced::Length::Fill)
                .height(iced::Length::Fill);

            let mut stack_children: Vec<iced::Element<'_, PdfMessage>> = vec![
                pages_canvas.into(),
                selection_overlay.into(),
                interactive_overlay.into(),
            ];

            if let Some(popup) = self.build_comment_popup(size) {
                stack_children.push(popup);
            }

            widget::Stack::with_children(stack_children)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .into()
        })
        .into()
    }

    fn build_comment_popup(
        &self,
        viewport_size: iced::Size,
    ) -> Option<iced::Element<'_, PdfMessage>> {
        let active_idx = self.active_comment?;
        let comment_visible = self.visible_comments(viewport_size);
        let (_, comment_rect) = comment_visible.iter().find(|(idx, _)| *idx == active_idx)?;

        let popup_width = 280.0_f32.min(viewport_size.width - 16.0).max(120.0);
        let popup_x = comment_rect.x1.x + 8.0;
        let popup_y = comment_rect.x0.y;
        let clamped_x = popup_x
            .min(viewport_size.width - popup_width - 8.0)
            .max(8.0);
        let clamped_y = popup_y.min(viewport_size.height - 100.0).max(8.0);

        let popup = widget::container(
            widget::column![
                widget::row![
                    widget::text("Comment")
                        .size(14.0)
                        .font(iced::Font {
                            weight: iced::font::Weight::Bold,
                            ..iced::Font::default()
                        }),
                    widget::horizontal_space().width(iced::Length::Fill),
                    widget::button(
                        widget::text(icon_to_string(RequiredIcons::X))
                            .align_y(iced::alignment::Vertical::Bottom)
                            .size(24.0)
                            .font(REQUIRED_FONT),
                    )
                    .padding(0.0)
                    .style(|theme: &iced::Theme, status: widget::button::Status| {
                        let palette = theme.extended_palette();
                        let base = widget::button::Style {
                            background: Some(iced::Background::Color(palette.background.strong.color)),
                            text_color: palette.background.strong.text,
                            border: iced::border::rounded(2),
                            ..widget::button::Style::default()
                        };
                        match status {
                            widget::button::Status::Active
                            | widget::button::Status::Pressed
                            | widget::button::Status::Hovered => widget::button::Style {
                                background: None,
                                text_color: palette.background.base.text,
                                ..base
                            },
                            widget::button::Status::Disabled => widget::button::Style {
                                background: base.background.map(|bg| bg.scale_alpha(0.5)),
                                text_color: base.text_color.scale_alpha(0.5),
                                ..base
                            },
                        }
                    })
                    .on_press(PdfMessage::CloseComment),
                ]
                .align_y(iced::alignment::Vertical::Center),
                widget::text(&self.comments[active_idx].content)
                    .size(14.0)
                    .wrapping(widget::text::Wrapping::Word),
            ]
            .spacing(8.0),
        )
        .width(popup_width)
        .padding(12.0)
        .style(|theme: &iced::Theme| widget::container::Style {
            background: Some(theme.extended_palette().background.weak.color.into()),
            border: iced::Border {
                color: theme.extended_palette().primary.base.color,
                width: 2.0,
                radius: iced::border::Radius::from(8.0),
            },
            shadow: iced::Shadow {
                color: theme.extended_palette().primary.base.color,
                offset: iced::Vector { x: 0.0, y: 2.0 },
                blur_radius: 4.0,
            },
            ..Default::default()
        });

        let positioned = widget::container(
            widget::mouse_area(popup).on_press(PdfMessage::None),
        )
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .padding(
            iced::Padding::new(0.0)
                .top(clamped_y)
                .left(clamped_x),
        )
        .align_x(iced::alignment::Horizontal::Left)
        .align_y(iced::alignment::Vertical::Top);

        Some(positioned.into())
    }

    fn count_chars(display_lists: &[mupdf::DisplayList]) -> Result<usize> {
        let _span = tracy_client::span!("Counting chars");
        let mut count = 0;
        for dl in display_lists {
            let tp = dl.to_text_page(TextPageFlags::empty())?;
            for block in tp.blocks() {
                for line in block.lines() {
                    for char in line.chars() {
                        count += 1;
                    }
                }
            }
        }
        Ok(count)
    }

    /// Returns (search haystack, Vec<(page number, byte offset, bounding box)>)
    fn extract_search_data(
        display_lists: &[mupdf::DisplayList],
    ) -> Result<(String, Vec<(usize, usize, Rect<f32>)>)> {
        let _span = tracy_client::span!("Preparing search data");
        let mut all_text = String::new();
        let mut bounding_boxes = vec![];
        for (page_idx, dl) in display_lists.iter().enumerate() {
            let tp = dl.to_text_page(TextPageFlags::empty())?;
            for block in tp.blocks() {
                for line in block.lines() {
                    for char in line.chars() {
                        if let Some(c) = char.char() {
                            let byte_offset = all_text.len();
                            all_text.push(c);
                            let quad = char.quad();
                            bounding_boxes.push((
                                page_idx,
                                byte_offset,
                                Rect {
                                    x0: Vector::new(quad.ul.x, quad.ul.y),
                                    x1: Vector::new(quad.lr.x, quad.lr.y),
                                },
                            ));
                        }
                    }
                }
            }
        }
        Ok((all_text, bounding_boxes))
    }

    fn spawn_search_task(&self) -> iced::Task<PdfMessage> {
        let text_contents = self.text_contents.clone();
        let needle = self.needle.clone();
        let method = self.search_method;
        let char_bboxes = self.char_bboxes.clone();
        let generation = self.search_generation;

        iced::Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    find_search_matches(&text_contents, &needle, method, &char_bboxes)
                })
                .await
            },
            move |result| match result {
                Ok(matches) => PdfMessage::SearchResultsReady(matches, generation),
                Err(_) => PdfMessage::None,
            },
        )
    }

    pub fn extract_text_from_rect(&self, screen_rect: Rect<f32>) -> String {
        use mupdf::TextPageFlags;

        let effective_scale = self.scale * self.fractional_scaling;
        let viewport = *self.viewport.borrow();

        let Ok(pages) = self.doc.pages() else {
            return String::new();
        };
        let Ok(rects) = self.layout.pages_rects(
            pages,
            self.translation.scaled(-1.0),
            self.scale,
            self.fractional_scaling,
            viewport,
        ) else {
            return String::new();
        };

        let mut result = String::new();

        for (i, page_rect) in rects.iter().enumerate() {
            let intersect = screen_rect.intersect(page_rect);
            if intersect.width() <= 0.0 || intersect.height() <= 0.0 {
                continue;
            }

            let page_bounds = self.display_lists[i].bounds();

            let pdf_rect = mupdf::Rect::new(
                (intersect.x0.x - page_rect.x0.x) / effective_scale + page_bounds.x0,
                (intersect.x0.y - page_rect.x0.y) / effective_scale + page_bounds.y0,
                (intersect.x1.x - page_rect.x0.x) / effective_scale + page_bounds.x0,
                (intersect.x1.y - page_rect.x0.y) / effective_scale + page_bounds.y0,
            );

            let Ok(text_page) = self.display_lists[i].to_text_page(TextPageFlags::empty()) else {
                continue;
            };

            for block in text_page.blocks() {
                for line in block.lines() {
                    let line_bounds = line.bounds();
                    if !rectangles_intersect(pdf_rect, line_bounds) {
                        continue;
                    }
                    for ch in line.chars() {
                        let quad = ch.quad();
                        let char_rect =
                            mupdf::Rect::new(quad.ul.x, quad.ul.y, quad.lr.x, quad.lr.y);
                        if rectangles_intersect(pdf_rect, char_rect)
                            && let Some(c) = ch.char()
                        {
                            result.push(c);
                        }
                    }
                    result.push('\n');
                }
            }
        }

        result.trim().to_string()
    }

    pub fn selected_text(&self) -> &str {
        &self.selected_text
    }

    fn selection_rect(&self) -> Option<Rect<f32>> {
        let (start, end) = (self.selection_start?, self.selection_end?);
        Some(Rect::from_points(
            Vector::new(start.x.min(end.x), start.y.min(end.y)),
            Vector::new(start.x.max(end.x), start.y.max(end.y)),
        ))
    }

    fn visible_links(&self, viewport: iced::Size<f32>) -> Vec<((usize, usize), Rect<f32>)> {
        let mut result = Vec::new();
        let Ok(pages) = self.doc.pages() else {
            return result;
        };
        let Ok(page_rects) = self.layout.pages_rects(
            pages,
            self.translation.scaled(-1.0),
            self.scale,
            self.fractional_scaling,
            viewport,
        ) else {
            return result;
        };

        let viewport_rect = Rect::from_pos_size(Vector::zero(), viewport.into());

        for (page_idx, page_rect) in page_rects.iter().enumerate() {
            if !viewport_rect.intersects(page_rect) {
                continue;
            }
            let page_bounds = self.display_lists[page_idx].bounds();
            let page_width = page_bounds.x1 - page_bounds.x0;
            let page_height = page_bounds.y1 - page_bounds.y0;
            if page_width <= 0.0 || page_height <= 0.0 {
                continue;
            }
            let scale_x = page_rect.width() / page_width;
            let scale_y = page_rect.height() / page_height;

            for (link_idx, link) in self.links[page_idx].iter().enumerate() {
                let screen_rect = Rect::from_points(
                    Vector::new(
                        page_rect.x0.x + (link.bounds.x0 - page_bounds.x0) * scale_x,
                        page_rect.x0.y + (link.bounds.y0 - page_bounds.y0) * scale_y,
                    ),
                    Vector::new(
                        page_rect.x0.x + (link.bounds.x1 - page_bounds.x0) * scale_x,
                        page_rect.x0.y + (link.bounds.y1 - page_bounds.y0) * scale_y,
                    ),
                );
                if viewport_rect.intersects(&screen_rect) {
                    result.push(((page_idx, link_idx), screen_rect));
                }
            }
        }
        result
    }

    fn visible_search_results(&self, viewport: iced::Size<f32>) -> Vec<(usize, Rect<f32>)> {
        let mut result = Vec::new();
        if !self.show_search_results {
            return result;
        }
        let Ok(pages) = self.doc.pages() else {
            return result;
        };
        let Ok(page_rects) = self.layout.pages_rects(
            pages,
            self.translation.scaled(-1.0),
            self.scale,
            self.fractional_scaling,
            viewport,
        ) else {
            return result;
        };

        let viewport_rect = Rect::from_pos_size(Vector::zero(), viewport.into());

        for (page_idx, page_rect) in page_rects.iter().enumerate() {
            if !viewport_rect.intersects(page_rect) {
                continue;
            }
            let page_bounds = self.display_lists[page_idx].bounds();
            let page_width = page_bounds.x1 - page_bounds.x0;
            let page_height = page_bounds.y1 - page_bounds.y0;
            if page_width <= 0.0 || page_height <= 0.0 {
                continue;
            }
            let scale_x = page_rect.width() / page_width;
            let scale_y = page_rect.height() / page_height;

            for (match_idx, m) in self.search_matches.iter().enumerate() {
                for &(rect_page_idx, rect) in &m.rects {
                    if rect_page_idx != page_idx {
                        continue;
                    }
                    let screen_rect = Rect::from_points(
                        Vector::new(
                            page_rect.x0.x + (rect.x0.x - page_bounds.x0) * scale_x,
                            page_rect.x0.y + (rect.x0.y - page_bounds.y0) * scale_y,
                        ),
                        Vector::new(
                            page_rect.x0.x + (rect.x1.x - page_bounds.x0) * scale_x,
                            page_rect.x0.y + (rect.x1.y - page_bounds.y0) * scale_y,
                        ),
                    );
                    if viewport_rect.intersects(&screen_rect) {
                        result.push((match_idx, screen_rect));
                    }
                }
            }
        }
        result
    }

    fn visible_comments(&self, viewport: iced::Size<f32>) -> Vec<(usize, Rect<f32>)> {
        let mut result = Vec::new();
        let Ok(pages) = self.doc.pages() else {
            return result;
        };
        let Ok(page_rects) = self.layout.pages_rects(
            pages,
            self.translation.scaled(-1.0),
            self.scale,
            self.fractional_scaling,
            viewport,
        ) else {
            return result;
        };

        let viewport_rect = Rect::from_pos_size(Vector::zero(), viewport.into());

        for (page_idx, page_rect) in page_rects.iter().enumerate() {
            if !viewport_rect.intersects(page_rect) {
                continue;
            }
            let page_bounds = self.display_lists[page_idx].bounds();
            let page_width = page_bounds.x1 - page_bounds.x0;
            let page_height = page_bounds.y1 - page_bounds.y0;
            if page_width <= 0.0 || page_height <= 0.0 {
                continue;
            }
            let scale_x = page_rect.width() / page_width;
            let scale_y = page_rect.height() / page_height;

            for (comment_idx, comment) in self.comments.iter().enumerate() {
                if comment.page_idx != page_idx {
                    continue;
                }
                let screen_rect = Rect::from_points(
                    Vector::new(
                        page_rect.x0.x + (comment.bounds.x0 - page_bounds.x0) * scale_x,
                        page_rect.x0.y + (comment.bounds.y0 - page_bounds.y0) * scale_y,
                    ),
                    Vector::new(
                        page_rect.x0.x + (comment.bounds.x1 - page_bounds.x0) * scale_x,
                        page_rect.x0.y + (comment.bounds.y1 - page_bounds.y0) * scale_y,
                    ),
                );
                if viewport_rect.intersects(&screen_rect) {
                    result.push((comment_idx, screen_rect));
                }
            }
        }
        result
    }

    fn local_mouse_pos(&self) -> Vector<f32> {
        let offset: Vector<f32> = (*self.widget_position.borrow()).into();
        self.mouse_pos - offset
    }

    fn update_hover_state(&mut self) {
        let local_mouse = self.local_mouse_pos();
        let viewport = *self.viewport.borrow();

        let visible_links = self.visible_links(viewport);
        self.hovered_link = visible_links
            .iter()
            .find(|(_, rect)| rect.contains(local_mouse))
            .map(|((page_idx, link_idx), _)| (*page_idx, *link_idx));
        if self.hovered_link.is_some() {
            self.hovered_search_result = None;
            self.hovered_comment = None;
            return;
        }

        let visible_search = self.visible_search_results(viewport);
        self.hovered_search_result = visible_search
            .iter()
            .find(|(_, rect)| rect.contains(local_mouse))
            .map(|(match_idx, _)| *match_idx);
        if self.hovered_search_result.is_some() {
            self.hovered_comment = None;
            return;
        }

        let visible_comments = self.visible_comments(viewport);
        self.hovered_comment = visible_comments
            .iter()
            .find(|(_, rect)| rect.contains(local_mouse))
            .map(|(comment_idx, _)| *comment_idx);
    }

    fn activate_link(&mut self, page_idx: usize, link_idx: usize) -> iced::Task<PdfMessage> {
        let Some(link) = self.links.get(page_idx).and_then(|p| p.get(link_idx)) else {
            return iced::Task::none();
        };

        self.show_link_hitboxes = false;

        if link.uri.starts_with("http://")
            || link.uri.starts_with("https://")
            || link.uri.starts_with("mailto:")
        {
            let _ = open::that(&link.uri);
        } else if let Some(dest) = link.dest {
            let page_num = dest.loc.page_number as usize;
            if page_num < self.doc.page_count().unwrap() as usize {
                return iced::Task::done(PdfMessage::SetPage(page_num));
            }
        } else if link.uri.starts_with("#page=")
            && let Some(page_str) = link.uri.strip_prefix("#page=")
            && let Ok(page_num) = page_str.parse::<usize>()
        {
            if page_num > 0 {
                return iced::Task::done(PdfMessage::SetPage(page_num - 1));
            }
        } else if link.uri.chars().all(|c| c.is_ascii_digit())
            && let Ok(page_num) = link.uri.parse::<usize>()
            && page_num > 0
        {
            return iced::Task::done(PdfMessage::SetPage(page_num - 1));
        }

        iced::Task::none()
    }

    pub fn page_count(&self) -> Result<i32> {
        Ok(self.doc.page_count()?)
    }

    #[cfg(test)]
    pub fn set_viewport_for_test(&mut self, size: iced::Size) {
        *self.viewport.borrow_mut() = size;
    }

    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        self.fractional_scaling = scale_factor as f32;
    }

    pub fn set_pdf_dark_mode(&mut self, dark_mode_enabled: bool) {
        if self.pdf_dark_mode != dark_mode_enabled {
            self.pdf_dark_mode = dark_mode_enabled;
            self.render_cache.borrow_mut().clear();
        }
    }

    pub fn set_interface_dark_mode(&mut self, dark_mode_enabled: bool) {
        if self.interface_dark_mode != dark_mode_enabled {
            self.interface_dark_mode = dark_mode_enabled;
            self.render_cache.borrow_mut().clear();
        }
    }

    pub fn is_jumpable_action(&self, msg: &PdfMessage) -> bool {
        match msg {
            PdfMessage::ActivateLink(index) => {
                if let Some(link) = self
                    .links
                    .get(self.current_page())
                    .and_then(|p| p.get(*index))
                {
                    link.uri.starts_with("#page=")
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn get_outline(&self) -> &[OutlineItem] {
        &self.outline
    }

    fn extract_outline(doc: &mupdf::Document) -> Result<Vec<OutlineItem>> {
        let outlines = doc.outlines()?;
        let mut items = Vec::new();
        for outline in &outlines {
            items.push(Self::convert_outline(outline, 0)?);
        }
        Ok(items)
    }

    fn convert_outline(outline: &mupdf::Outline, level: u32) -> Result<OutlineItem> {
        let mut children = Vec::new();
        for child in &outline.down {
            children.push(Self::convert_outline(child, level + 1)?);
        }
        Ok(OutlineItem {
            title: outline.title.clone(),
            page: outline.dest.map(|d| d.loc.page_number),
            level,
            children,
        })
    }

    pub fn page_progress(&self) -> String {
        let current = self.current_page() + 1;
        let total = self.page_count().unwrap_or(0);
        format!("({} / {})", current, total)
    }

    pub fn current_page(&self) -> usize {
        self.layout
            .current_page_index(&self.doc, self.translation, *self.viewport.borrow())
            .unwrap()
    }

    pub fn search_progress(&self) -> String {
        if self.needle.is_empty() {
            String::new()
        } else {
            let current = self.current_search_result.map(|i| i + 1).unwrap_or(0);
            let total = self.search_matches.len();
            format!("({} / {})", current, total)
        }
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

fn cpu_pdf_dark_mode_shader(pixmap: &mut mupdf::Pixmap, gradient_cache: &[[u8; 4]; 256]) {
    // PERF: Slow in debug builds but more than fast enough in release builds.
    let _span = tracy_client::span!("Cpu dark mode shader");
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

fn rectangles_intersect(a: mupdf::Rect, b: mupdf::Rect) -> bool {
    a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0
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

/// Returns the pdf background color
fn get_pdf_background_color(pdf_dark_mode: bool, show_borders: bool) -> iced::Color {
    if show_borders {
        if pdf_dark_mode {
            iced::Color::from_rgb8(21, 22, 32)
        } else {
            iced::Color::from_rgb8(220, 219, 218)
        }
    } else {
        if pdf_dark_mode {
            DARK_THEME.palette().background
        } else {
            iced::Color::WHITE
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use crate::pdf::find_search_matches;
    use super::*;

    #[test]
    fn test_zoom_fit_scales_current_page_to_viewport() -> Result<()> {
        let mut viewer = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
        let viewport = iced::Size::new(800.0, 600.0);
        viewer.set_viewport_for_test(viewport);
        viewer.layout = PageLayout::SinglePage;

        // Start on page 0
        let start_page = viewer.current_page();
        assert_eq!(start_page, 0);

        let _ = viewer.update(PdfMessage::ZoomFit);

        let page_idx = viewer.current_page();
        assert_eq!(
            page_idx, start_page,
            "ZoomFit should keep the same current page"
        );

        let page_bounds = viewer.display_lists[page_idx].bounds();
        let page_width = page_bounds.x1 - page_bounds.x0;
        let page_height = page_bounds.y1 - page_bounds.y0;

        let effective_scale = viewer.scale * viewer.fractional_scaling;
        let scaled_width = page_width * effective_scale;
        let scaled_height = page_height * effective_scale;

        assert!(
            scaled_width <= viewport.width + 1e-3,
            "Scaled width {} should fit in viewport width {}",
            scaled_width,
            viewport.width
        );
        assert!(
            scaled_height <= viewport.height + 1e-3,
            "Scaled height {} should fit in viewport height {}",
            scaled_height,
            viewport.height
        );

        // The scale should be the largest scale that still fits the page.
        let scale_x = viewport.width / page_width;
        let scale_y = viewport.height / page_height;
        let expected_scale = scale_x.min(scale_y) / viewer.fractional_scaling;
        assert!(
            (viewer.scale - expected_scale).abs() < 1e-3,
            "Expected scale ~{}, got {}",
            expected_scale,
            viewer.scale
        );

        // Verify the page is fully visible by checking its rect.
        let rects = viewer.layout.pages_rects(
            viewer.doc.pages()?,
            -viewer.translation,
            viewer.scale,
            viewer.fractional_scaling,
            viewport,
        )?;
        let page_rect = rects[page_idx];
        assert!(
            page_rect.x0.x >= -1e-3,
            "Page left edge {} should be inside viewport",
            page_rect.x0.x
        );
        assert!(
            page_rect.x1.x <= viewport.width + 1e-3,
            "Page right edge {} should be inside viewport",
            page_rect.x1.x
        );
        assert!(
            page_rect.x0.y >= -1e-3,
            "Page top edge {} should be inside viewport",
            page_rect.x0.y
        );
        assert!(
            page_rect.x1.y <= viewport.height + 1e-3,
            "Page bottom edge {} should be inside viewport",
            page_rect.x1.y
        );

        Ok(())
    }

    #[test]
    fn test_plaintext_search_link_extraction_on_page_0() -> Result<()> {
        let viewer = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
        let result = find_search_matches(
            &viewer.text_contents,
            "Link Extraction",
            SearchMethod::PlainText,
            &viewer.char_bboxes,
        );
        assert_eq!(result.len(), 1, "should find exactly one 'Link Extraction'");
        assert_eq!(
            result[0].pages,
            0..1,
            "'Link Extraction' should be on page 0"
        );
        assert_eq!(result[0].rects[0].0, 0, "rect should be on page 0");
        assert!(viewer.text_contents.is_char_boundary(result[0].start_byte));
        assert!(viewer.text_contents.is_char_boundary(result[0].end_byte));
        assert!(
            &viewer.text_contents[result[0].start_byte..result[0].end_byte]
                .starts_with("Link Extraction")
        );
        Ok(())
    }

    #[test]
    fn test_plaintext_search_code_blocks_on_page_1() -> Result<()> {
        let viewer = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
        let result = find_search_matches(
            &viewer.text_contents,
            "Code Blocks",
            SearchMethod::PlainText,
            &viewer.char_bboxes,
        );
        assert_eq!(result.len(), 1, "should find exactly one 'Code Blocks'");
        assert_eq!(result[0].pages, 1..2, "'Code Blocks' should be on page 1");
        assert_eq!(result[0].rects[0].0, 1, "rect should be on page 1");
        assert!(
            &viewer.text_contents[result[0].start_byte..result[0].end_byte]
                .starts_with("Code Blocks")
        );
        Ok(())
    }

    #[test]
    fn test_plaintext_search_bullet_multibyte() -> Result<()> {
        let viewer = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
        let result = find_search_matches(
            &viewer.text_contents,
            "•",
            SearchMethod::PlainText,
            &viewer.char_bboxes,
        );
        assert!(!result.is_empty(), "should find bullet characters");
        // Every bullet match should be a valid char boundary
        for m in &result {
            assert!(viewer.text_contents.is_char_boundary(m.start_byte));
            assert!(viewer.text_contents.is_char_boundary(m.end_byte));
            assert_eq!(&viewer.text_contents[m.start_byte..m.end_byte], "•");
        }
        // At least one bullet should be on page 0 (the first one at byte 194)
        assert!(result.iter().any(|m| m.pages == (0..1)));
        Ok(())
    }

    #[test]
    fn test_regex_search_link_extraction_on_page_0() -> Result<()> {
        let viewer = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
        let result = find_search_matches(
            &viewer.text_contents,
            "Link Extraction",
            SearchMethod::Regex,
            &viewer.char_bboxes,
        );
        assert_eq!(
            result.len(),
            1,
            "should find exactly one 'Link Extraction' via regex"
        );
        assert_eq!(
            result[0].pages,
            0..1,
            "regex 'Link Extraction' should be on page 0"
        );
        assert!(viewer.text_contents.is_char_boundary(result[0].start_byte));
        assert!(viewer.text_contents.is_char_boundary(result[0].end_byte));
        Ok(())
    }

    #[test]
    fn test_regex_search_code_blocks_on_page_1() -> Result<()> {
        let viewer = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
        let result = find_search_matches(
            &viewer.text_contents,
            "Code Blocks",
            SearchMethod::Regex,
            &viewer.char_bboxes,
        );
        assert_eq!(
            result.len(),
            1,
            "should find exactly one 'Code Blocks' via regex"
        );
        assert_eq!(result[0].pages, 1..2);
        assert_eq!(result[0].rects[0].0, 1);
        Ok(())
    }

    #[test]
    fn test_plaintext_regex_parity_link_extraction() -> Result<()> {
        let viewer = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
        let plain = find_search_matches(
            &viewer.text_contents,
            "Link Extraction",
            SearchMethod::PlainText,
            &viewer.char_bboxes,
        );
        let regex = find_search_matches(
            &viewer.text_contents,
            "Link Extraction",
            SearchMethod::Regex,
            &viewer.char_bboxes,
        );
        assert_eq!(
            plain.len(),
            regex.len(),
            "plaintext and regex should find same number of 'Link Extraction' matches"
        );
        for (p, r) in plain.iter().zip(regex.iter()) {
            assert_eq!(p.start_byte, r.start_byte, "start bytes should match");
            assert_eq!(p.end_byte, r.end_byte, "end bytes should match");
            assert_eq!(p.pages, r.pages, "pages should match");
            assert_eq!(p.rects.len(), r.rects.len(), "rect counts should match");
        }
        Ok(())
    }

    #[test]
    fn test_plaintext_regex_parity_code_blocks() -> Result<()> {
        let viewer = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
        let plain = find_search_matches(
            &viewer.text_contents,
            "Code Blocks",
            SearchMethod::PlainText,
            &viewer.char_bboxes,
        );
        let regex = find_search_matches(
            &viewer.text_contents,
            "Code Blocks",
            SearchMethod::Regex,
            &viewer.char_bboxes,
        );
        assert_eq!(
            plain, regex,
            "plaintext and regex should produce identical results for 'Code Blocks'"
        );
        Ok(())
    }

    #[test]
    fn test_no_match_on_real_pdf() -> Result<()> {
        let viewer = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
        let result = find_search_matches(
            &viewer.text_contents,
            "XYZ_NONEXISTENT",
            SearchMethod::PlainText,
            &viewer.char_bboxes,
        );
        assert!(result.is_empty(), "should not find nonexistent text");
        Ok(())
    }

    #[test]
    fn test_comment_extraction_from_commented_pdf() -> Result<()> {
        let viewer = PdfViewer::from_path(PathBuf::from("assets/links_commented.pdf"))?;
        assert!(
            !viewer.comments.is_empty(),
            "should extract at least one comment from links_commented.pdf"
        );
        for comment in &viewer.comments {
            assert!(!comment.content.is_empty(), "comment content should not be empty");
            assert!(
                comment.bounds.x1 > comment.bounds.x0 && comment.bounds.y1 > comment.bounds.y0,
                "comment bounds should be valid"
            );
        }
        Ok(())
    }

    #[test]
    fn test_no_comments_on_plain_pdf() -> Result<()> {
        let viewer = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
        assert!(
            viewer.comments.is_empty(),
            "links.pdf should have no comments"
        );
        Ok(())
    }
}
