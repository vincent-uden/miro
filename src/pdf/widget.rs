use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, Weak,
    },
    time::Duration,
};

use anyhow::Result;
use colorgrad::{Gradient as _, GradientBuilder, LinearGradient};
use iced::{
    Renderer, Size, Theme,
    advanced::{graphics::geometry, image},
    widget::{
        self,
        canvas::{self, Cache, Stroke},
    },
};
use iced::advanced::image::Renderer as _;
use bytes::Bytes;

use mupdf::{
    Colorspace, Device, Matrix, Pixmap, TextPageFlags,
    pdf::{PdfAnnotationType, PdfObject, PdfPage},
};
use serde::{Deserialize, Serialize};
use tracing::{error};

use crate::{
    CONFIG, DARK_THEME,
    config::{BindingMode, MOVE_STEP, MoveDirection, MouseAction},
    geometry::{Rect, Vector},
    pdf::{
        PdfMessage, SearchMatch, SearchMethod, find_search_matches,
        page_layout::{PageLayout, PageLayoutKind, PageRotation},
    },
};

#[derive(Debug, Clone)]
struct PageLink {
    bounds: mupdf::Rect,
    uri: String,
    dest: Option<mupdf::link::LinkDestination>,
}

#[derive(Debug, Clone)]
struct Comment {
    id: usize,
    page_idx: usize,
    bounds: Option<mupdf::Rect>,
    content: Option<String>,
    author: Option<String>,
    replies: Vec<Comment>,
}

#[derive(Debug, Clone)]
struct AnnotationCommentData {
    page_idx: usize,
    bounds: Option<mupdf::Rect>,
    content: Option<String>,
    author: Option<String>,
    annotation_type: PdfAnnotationType,
    object_id: Option<i32>,
    in_reply_to: Option<i32>,
}

fn build_comment_subtree(
    index: usize,
    annotations: &[AnnotationCommentData],
    children_by_parent: &[Vec<usize>],
    visited: &mut HashSet<usize>,
) -> Option<Comment> {
    if !visited.insert(index) {
        return None;
    }

    let replies: Vec<Comment> = children_by_parent[index]
        .iter()
        .filter_map(|child| build_comment_subtree(*child, annotations, children_by_parent, visited))
        .collect();
    let annotation = &annotations[index];
    if annotation.content.is_none() && replies.is_empty() {
        return None;
    }

    Some(Comment {
        id: index,
        page_idx: annotation.page_idx,
        bounds: annotation.bounds,
        content: annotation.content.clone(),
        author: annotation.author.clone(),
        replies,
    })
}

fn popup_position(
    anchor: iced::Point,
    popup_size: iced::Size,
    viewport: iced::Rectangle,
) -> iced::Point {
    const POPUP_MARGIN: f32 = 8.0;
    iced::Point::new(
        anchor
            .x
            .min(viewport.x + viewport.width - popup_size.width - POPUP_MARGIN)
            .max(viewport.x + POPUP_MARGIN),
        anchor
            .y
            .min(viewport.y + viewport.height - popup_size.height - POPUP_MARGIN)
            .max(viewport.y + POPUP_MARGIN),
    )
}

/// Annotation types whose content makes them standalone comment roots. This covers the
/// quadpoint-based markup annotations (highlights, strike-outs, ...) which carry notes the
/// same way text annotations do. Other types are deliberately excluded: FreeText is
/// already visible on the page and Caret, Redact, Stamp etc. have odd semantics.
fn is_commentable(annotation_type: PdfAnnotationType) -> bool {
    matches!(
        annotation_type,
        PdfAnnotationType::Text
            | PdfAnnotationType::Highlight
            | PdfAnnotationType::Underline
            | PdfAnnotationType::StrikeOut
            | PdfAnnotationType::Squiggly
            | PdfAnnotationType::Ink
    )
}

/// Builds reply trees in PDF annotation order; xref IDs are used only to resolve `/IRT` links.
fn build_comments(annotations: Vec<AnnotationCommentData>) -> Vec<Comment> {
    let annotations_by_id: HashMap<i32, usize> = annotations
        .iter()
        .enumerate()
        .filter_map(|(index, annotation)| annotation.object_id.map(|id| (id, index)))
        .collect();

    let mut children_by_parent = vec![Vec::new(); annotations.len()];
    for (index, annotation) in annotations.iter().enumerate() {
        let Some(parent_id) = annotation.in_reply_to else {
            continue;
        };
        let Some(parent_index) = annotations_by_id.get(&parent_id).copied() else {
            continue;
        };
        if parent_index != index {
            children_by_parent[parent_index].push(index);
        }
    }

    let mut comments = Vec::new();
    let mut visited = HashSet::new();
    for (index, annotation) in annotations.iter().enumerate() {
        if annotation.in_reply_to.is_some() || annotation.bounds.is_none() {
            continue;
        }
        let Some(comment) =
            build_comment_subtree(index, &annotations, &children_by_parent, &mut visited)
        else {
            continue;
        };
        let is_comment_root =
            is_commentable(annotation.annotation_type) && annotation.content.is_some();
        if is_comment_root || !comment.replies.is_empty() {
            comments.push(comment);
        }
    }

    // Preserve standalone comments and orphaned replies if their parent is missing or cyclic.
    for (index, annotation) in annotations.iter().enumerate() {
        if visited.contains(&index)
            || !is_commentable(annotation.annotation_type)
            || annotation.content.is_none()
            || annotation.bounds.is_none()
        {
            continue;
        }
        if let Some(comment) =
            build_comment_subtree(index, &annotations, &children_by_parent, &mut visited)
        {
            comments.push(comment);
        }
    }

    comments
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
const OVERVIEW_SCROLL_MARGIN: f32 = 64.0;
const OVERVIEW_SCROLL_STEP_PIXELS: f32 = 80.0;

/// Scroll the overview grid just enough to keep the selected thumbnail inside a vertical safe
/// zone. `scroll_y` is measured in screen pixels from the top-padded start of the content.
fn apply_overview_scroll(
    rects: &mut [Rect<f32>],
    selected_page_idx: usize,
    viewport: Size<f32>,
    scroll_y: &mut f32,
) {
    let Some(first) = rects.first().copied() else {
        *scroll_y = 0.0;
        return;
    };

    let content = rects
        .iter()
        .skip(1)
        .fold(first, |bounds, rect| bounds.union(rect));
    let margin = OVERVIEW_SCROLL_MARGIN.min(viewport.height.max(0.0) * 0.25);
    let safe_top = margin;
    let safe_bottom = (viewport.height - margin).max(safe_top);
    let safe_height = safe_bottom - safe_top;

    // A short grid is already centered by PageLayout; leave it untouched.
    if content.height() <= safe_height {
        *scroll_y = 0.0;
        return;
    }

    let max_scroll = content.height() - safe_height;
    *scroll_y = scroll_y.clamp(0.0, max_scroll);

    let mut selected = rects[selected_page_idx.min(rects.len() - 1)];
    let initial_offset = safe_top - content.x0.y - *scroll_y;
    selected.translate(Vector::new(0.0, initial_offset));
    let correction = if selected.height() > safe_height {
        selected.center().y - viewport.height * 0.5
    } else if selected.x0.y < safe_top {
        selected.x0.y - safe_top
    } else if selected.x1.y > safe_bottom {
        selected.x1.y - safe_bottom
    } else {
        0.0
    };
    *scroll_y = (*scroll_y + correction).clamp(0.0, max_scroll);

    let offset = safe_top - content.x0.y - *scroll_y;
    for rect in rects {
        rect.translate(Vector::new(0.0, offset));
    }
}

/// How long a needle update waits before actually spawning a search scan. Typing spawns one
/// task per keystroke; with this, intermediate ones are cancelled before doing any work.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(50);

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
///   includes the visible pixel dimensions and source offset so that panning
///   invalidates the cached crop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RenderKey {
    Full(usize, u32, PageRotation),
    Partial(usize, u32, i32, i32, u32, u32, PageRotation),
}

/// How a single page should be rasterized for the current frame: which cache key
/// to use, where to draw the resulting image, the pixmap dimensions and the
/// MuPDF matrix/scissor used to render into it.
#[derive(Debug, Clone)]
struct TilePlan {
    key: RenderKey,
    draw_rect: Rect<f32>,
    width: i32,
    height: i32,
    matrix: Matrix,
    scissor: mupdf::Rect,
}

fn overview_render_scale(
    page_bounds: Rect<f32>,
    rotation: PageRotation,
    rect_ss: Rect<f32>,
) -> f32 {
    rect_ss.width() / rotation.rotated_size(page_bounds.size()).x
}

/// Decide whether a page can be rendered once in full or must be scissored to
/// the visible intersection, and compute the corresponding render parameters.
///
/// `rect_ss` is the page bounding box in screen coordinates (relative to the
/// widget origin) and `page_bounds` is the page size in PDF coordinates.
fn plan_tile(
    page_idx: usize,
    page_bounds: Rect<f32>,
    rotation: PageRotation,
    rect_ss: Rect<f32>,
    effective_scale: f32,
    viewport_rect: Rect<f32>,
) -> TilePlan {
    let fully_visible = rect_ss.x0.x >= 0.0
        && rect_ss.x1.x <= viewport_rect.x1.x
        && rect_ss.x0.y >= 0.0
        && rect_ss.x1.y <= viewport_rect.x1.y;

    if fully_visible {
        let key = RenderKey::Full(page_idx, effective_scale.to_bits(), rotation);
        let width = rect_ss.width().ceil().max(1.0) as i32;
        let height = rect_ss.height().ceil().max(1.0) as i32;
        let matrix = page_matrix(page_bounds, rotation, effective_scale, 0.0, 0.0);
        // Scissor must be in device coordinates, i.e. the pixmap size. The
        // unscaled page bounds would cull the bottom/right once the page is
        // scaled up to fit the viewport (e.g. after ZoomFit).
        let scissor = mupdf::Rect::new(0.0, 0.0, width as f32, height as f32);
        TilePlan {
            key,
            draw_rect: rect_ss,
            width,
            height,
            matrix,
            scissor,
        }
    } else {
        let vis = rect_ss.intersect(&viewport_rect);
        let width = vis.width().ceil().max(1.0) as i32;
        let height = vis.height().ceil().max(1.0) as i32;

        let render_offset_x = rect_ss.x0.x - vis.x0.x;
        let render_offset_y = rect_ss.x0.y - vis.x0.y;

        let key = RenderKey::Partial(
            page_idx,
            effective_scale.to_bits(),
            width,
            height,
            render_offset_x.to_bits(),
            render_offset_y.to_bits(),
            rotation,
        );

        let matrix = page_matrix(
            page_bounds,
            rotation,
            effective_scale,
            render_offset_x,
            render_offset_y,
        );

        // Scissor is in pixmap coordinates and covers the whole pixmap. It only
        // culls objects entirely outside the visible region.
        let scissor = mupdf::Rect::new(0.0, 0.0, width as f32, height as f32);

        TilePlan {
            key,
            draw_rect: vis,
            width,
            height,
            matrix,
            scissor,
        }
    }
}

/// Create the MuPDF transform from PDF coordinates to the normalized, rotated page raster.
fn page_matrix(
    bounds: Rect<f32>,
    rotation: PageRotation,
    scale: f32,
    offset_x: f32,
    offset_y: f32,
) -> Matrix {
    let (a, b, c, d, e, f) = match rotation {
        PageRotation::Deg0 => (1.0, 0.0, 0.0, 1.0, -bounds.x0.x, -bounds.x0.y),
        PageRotation::Deg90 => (0.0, 1.0, -1.0, 0.0, bounds.x1.y, -bounds.x0.x),
        PageRotation::Deg180 => (-1.0, 0.0, 0.0, -1.0, bounds.x1.x, bounds.x1.y),
        PageRotation::Deg270 => (0.0, -1.0, 1.0, 0.0, -bounds.x0.y, bounds.x1.x),
    };
    Matrix::new(
        a * scale,
        b * scale,
        c * scale,
        d * scale,
        (offset_x + e * scale).round(),
        (offset_y + f * scale).round(),
    )
}

fn transform_rect(
    rect: Rect<f32>,
    mut transform_point: impl FnMut(Vector<f32>) -> Vector<f32>,
) -> Rect<f32> {
    let mut min = transform_point(rect.x0);
    let mut max = min;
    for point in [
        Vector::new(rect.x0.x, rect.x1.y),
        Vector::new(rect.x1.x, rect.x0.y),
        rect.x1,
    ] {
        let point = transform_point(point);
        min.x = min.x.min(point.x);
        min.y = min.y.min(point.y);
        max.x = max.x.max(point.x);
        max.y = max.y.max(point.y);
    }
    Rect::from_points(min, max)
}

fn pdf_rect_to_screen(
    pdf_rect: Rect<f32>,
    page_bounds: Rect<f32>,
    page_rect: Rect<f32>,
    rotation: PageRotation,
) -> Rect<f32> {
    let rotated_size = rotation.rotated_size(page_bounds.size());
    let scale_x = page_rect.width() / rotated_size.x;
    let scale_y = page_rect.height() / rotated_size.y;
    transform_rect(pdf_rect, |point| {
        let rotated = rotation.to_rotated(point, page_bounds);
        page_rect.x0 + Vector::new(rotated.x * scale_x, rotated.y * scale_y)
    })
}

struct Document<'a> {
    cache: Cache,
    pages: Vec<(usize, image::Handle, Rect<f32>)>,
    allocation_cache: &'a RefCell<HashMap<image::Id, image::Allocation>>,
    draw_page_borders: bool,
    pdf_dark_mode: bool,
    highlight_page_idx: Option<usize>,
}

impl<'a> std::fmt::Debug for Document<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("cache", &self.cache)
            .field("page_count", &self.pages.len())
            .finish()
    }
}

impl<'a> Document<'a> {
    pub fn new(
        allocation_cache: &'a RefCell<HashMap<image::Id, image::Allocation>>,
        pages: Vec<(usize, image::Handle, Rect<f32>)>,
        draw_page_borders: bool,
        pdf_dark_mode: bool,
        highlight_page_idx: Option<usize>,
    ) -> Self {
        Self {
            cache: Cache::default(),
            pages,
            allocation_cache,
            draw_page_borders,
            pdf_dark_mode,
            highlight_page_idx,
        }
    }
}

impl<'a> widget::canvas::Program<PdfMessage> for Document<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let _span = tracy_client::span!("Pdf draw");
        let bg = self.cache.draw(renderer, bounds.size(), |frame| {
            let bg_color = get_pdf_background_color(self.pdf_dark_mode, self.draw_page_borders);
            frame.fill_rectangle(iced::Point::new(0.0, 0.0), bounds.size(), bg_color);

            for (page_idx, handle, rect) in &self.pages {
                let bounds: iced::Rectangle = (*rect).into();

                // NOTE: Ensure the image is explicitly allocated on the GPU so the next
                // frame is guaranteed to render it without asynchronous upload delay.
                let img = {
                    let mut cache = self.allocation_cache.borrow_mut();
                    if !cache.contains_key(&handle.id())
                        && let Ok(allocation) = renderer.load_image(handle)
                    {
                        let handle = allocation.handle().clone();
                        cache.insert(handle.id(), allocation);
                        image::Image::new(handle).filter_method(image::FilterMethod::Nearest)
                    } else {
                        // NOTE: This should in practice never happen but I still dont want to crash
                        // if we for some reason fail to upload an image
                        image::Image::new(handle).filter_method(image::FilterMethod::Nearest)
                    }
                };

                if Some(*page_idx) == self.highlight_page_idx {
                    let palette = theme.palette();
                    frame.stroke_rectangle(
                        bounds.position(),
                        bounds.size(),
                        Stroke::default()
                            .with_color(palette.primary)
                            .with_width(5.0),
                    );
                }
                frame.draw_image(bounds, img);
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
        event: &canvas::Event,
        _bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Option<canvas::Action<PdfMessage>> {
        let event = (*event).clone();
        if let canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            ref key, modifiers, ..
        }) = event
            && key == &iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
            && !modifiers.control()
            && !modifiers.alt()
            && !modifiers.logo()
            && self.viewer.active_comment.is_some()
        {
            return Some(canvas::Action::publish(PdfMessage::CloseComment).and_capture());
        }

        if !self.viewer.show_link_hitboxes {
            state.was_active = false;
            return None;
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
                return None;
            }

            if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                return Some(canvas::Action::publish(PdfMessage::CloseLinkHitboxes).and_capture());
            }

            if let iced::keyboard::Key::Character(c) = key {
                let ch = c.to_lowercase().to_string();
                state.pending_key.push_str(&ch);

                let viewport = *self.viewer.viewport.borrow();
                let link_visible = self.viewer.visible_links(viewport);
                let keys = generate_key_combinations(link_visible.len());

                if let Some(idx) = keys.iter().position(|k| k == &state.pending_key) {
                    state.pending_key.clear();
                    return Some(
                        canvas::Action::publish(PdfMessage::ActivateLink(idx)).and_capture(),
                    );
                }

                let is_prefix = keys.iter().any(|k| k.starts_with(&state.pending_key));
                if is_prefix {
                    return Some(canvas::Action::capture());
                }

                state.pending_key.clear();
                return Some(canvas::Action::capture());
            }
        }

        None
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
        if let Some((page_idx, link_idx)) = self.viewer.hovered_link
            && let Some((_, rect)) = link_visible
                .iter()
                .find(|((p, l), _)| *p == page_idx && *l == link_idx)
        {
            let mut color = iced::Color::from_rgb(0.0, 0.4, 0.8);
            color.a = 0.15;
            frame.fill_rectangle(rect.x0.into(), rect.size().into(), color);
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
                    max_width: bg_w,
                    color: iced::Color::WHITE,
                    size: text_size.into(),
                    line_height: widget::text::LineHeight::Relative(1.0),
                    font: iced::Font::default(),
                    align_x: iced::alignment::Horizontal::Center.into(),
                    align_y: iced::alignment::Vertical::Center,
                    shaping: widget::text::Shaping::Basic,
                });
            }
        }

        // Draw hovered comment indicator.
        if let Some(comment_idx) = self.viewer.hovered_comment
            && let Some((_, rect)) = comment_visible.iter().find(|(idx, _)| *idx == comment_idx)
        {
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
    /// PDF-space bounding box of every page, snapshotted once at open. Layout math must use
    /// this instead of iterating `doc.pages()`, which performs a full MuPDF page load per
    /// page per call — several full-document iterations per frame made large documents stall
    /// for seconds on every input event.
    page_bounds: Vec<Rect<f32>>,
    /// Final iced image handles cached by render key. Kept separately so iced can reuse the
    /// GPU texture without re-uploading when the widget redraws for non-visual reasons.
    render_cache: RefCell<HashMap<RenderKey, image::Handle>>,
    /// Explicit image allocations that guarantee GPU textures are uploaded and retained
    /// for visible pages.
    allocation_cache: RefCell<HashMap<image::Id, image::Allocation>>,
    /// Reusable MuPDF pixmaps keyed by page. These are expensive to allocate and are not Send,
    /// so we pool them separately from the plain CPU buffers.
    pixmap_pool: RefCell<HashMap<usize, Pixmap>>,
    /// Plain CPU buffers returned by dropped images and shared across threads. MuPDF data is not
    /// thread-safe, but iced may render on any thread, so we must copy into a Vec<u8> and pool
    /// it to avoid allocating multi-megabyte buffers on every frame during zoom or pan.
    buffer_pool: BufferPool,

    pub translation: Vector<f64>,
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

    text_contents: Arc<String>,
    char_bboxes: Arc<Vec<(usize, usize, Rect<f32>)>>,
    /// The search matches found in the document
    search_matches: Vec<SearchMatch>,
    pub(crate) search_method: SearchMethod,
    /// The thing to search for
    pub(crate) needle: String,
    /// Monotonically incremented to invalidate stale async search tasks. Shared with the
    /// spawned debounce futures so superseded tasks can drop out before spawning a scan.
    search_generation: Arc<AtomicU64>,
    /// Set to cancel all in-flight search scans. Replaced with a fresh flag per search;
    /// dropping the viewer also sets it so pending scans exit quickly instead of blocking
    /// iced's tokio runtime shutdown (which waits for blocking tasks).
    search_cancel: Arc<AtomicBool>,

    /// Comment-thread roots extracted from PDF annotations.
    comments: Vec<Comment>,
    /// IDs of comment nodes whose reply subtrees are collapsed.
    collapsed_comments: HashSet<usize>,
    hovered_comment: Option<usize>,
    active_comment: Option<usize>,
    comment_popup_hovered: bool,

    /// The widget's position in window coordinates, updated each frame by the overlay draw.
    widget_position: RefCell<iced::Point>,

    /// The layout kind that was active before the overview layout was entered,
    /// restored when overview mode is left.
    layout_before_overview: PageLayoutKind,

    /// The index of the "hovered" page in overview mode.
    overview_page_idx: usize,
    /// Screen-pixel scroll offset for keeping the overview selection in its safe zone.
    overview_scroll_y: Cell<f32>,
    /// Fractional page movement accumulated from trackpad scrolling.
    overview_scroll_remainder: f32,
}

impl Drop for PdfViewer {
    fn drop(&mut self) {
        // Cancel any in-flight search scans. Iced owns a tokio runtime and dropping it waits
        // for spawned blocking tasks, so without this, quitting mid-search would hang the
        // whole program until every stale full-document scan finished.
        self.search_cancel.store(true, Ordering::Relaxed);
    }
}

#[allow(clippy::type_complexity)]
impl PdfViewer {
    fn build_document_data(
        doc: &mupdf::Document,
    ) -> Result<(
        Vec<mupdf::DisplayList>,
        Vec<Vec<PageLink>>,
        Vec<OutlineItem>,
        Vec<Comment>,
        Vec<Rect<f32>>,
    )> {
        let mut display_lists = vec![];
        let mut links = vec![];
        let mut page_bounds = vec![];
        let mut annotation_comments = vec![];
        for (page_idx, page) in doc.pages()?.flatten().enumerate() {
            let page_bound: mupdf::Rect = page.bounds()?;
            page_bounds.push(page_bound.into());
            let mut dl = mupdf::DisplayList::new(page_bound)?;
            let dummy_device = Device::from_display_list(&mut dl)?;
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
                    let Ok(annotation_type) = ann.r#type() else {
                        continue;
                    };
                    if annotation_type == PdfAnnotationType::Popup {
                        continue;
                    }

                    let pdf_obj = PdfObject::try_from(&ann).ok();
                    // Replacement annotation groups are not discussion replies.
                    let is_group = pdf_obj
                        .as_ref()
                        .and_then(|object| object.get_dict("RT").ok().flatten())
                        .and_then(|reply_type| reply_type.as_name().ok())
                        .is_some_and(|reply_type| reply_type.as_slice() == b"Group");
                    if is_group {
                        continue;
                    }

                    let object_id = pdf_obj
                        .as_ref()
                        .and_then(|object| object.as_indirect().ok());
                    let in_reply_to = pdf_obj
                        .as_ref()
                        .and_then(|object| object.get_dict("IRT").ok().flatten())
                        .and_then(|reply_to| reply_to.as_indirect().ok());
                    // Replacement annotation groups carry placeholder strike-outs with empty
                    // notes; treat empty content as no content so they can't become comment roots.
                    let content = ann
                        .contents()
                        .ok()
                        .flatten()
                        .map(|value| value.to_string())
                        .filter(|value| !value.trim().is_empty());
                    let bounds = ann.rect().ok();
                    let author = ann.author().ok().flatten().map(|value| value.to_string());

                    annotation_comments.push(AnnotationCommentData {
                        page_idx,
                        bounds,
                        content,
                        author,
                        annotation_type,
                        object_id,
                        in_reply_to,
                    });
                }
            }
        }
        let comments = build_comments(annotation_comments);
        let outline = Self::extract_outline(doc).unwrap_or_default();
        Ok((display_lists, links, outline, comments, page_bounds))
    }

    pub fn from_path(path: PathBuf) -> Result<Self> {
        let name = path
            .file_name()
            .expect("The pdf must have a file name")
            .to_string_lossy()
            .to_string();
        let doc = mupdf::Document::open(&path.to_str().unwrap())?;
        let (display_lists, links, outline, comments, page_bounds) =
            Self::build_document_data(&doc)?;
        let (all_text, bboxes) = Self::extract_search_data(&display_lists)?;

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
            page_bounds,
            render_cache: RefCell::default(),
            allocation_cache: RefCell::default(),
            pixmap_pool: RefCell::default(),
            buffer_pool: Arc::new(Mutex::new(HashMap::new())),
            translation: Vector::zero(),
            scale: 1.0,
            fractional_scaling: 1.0,
            viewport: RefCell::default(),
            layout: PageLayout::new(PageLayoutKind::SinglePage),
            layout_before_overview: PageLayoutKind::SinglePage,
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
            text_contents: Arc::new(all_text),
            char_bboxes: Arc::new(bboxes),
            search_matches: vec![],
            search_method: CONFIG.read().unwrap().default_search_method,
            needle: String::new(),
            search_generation: Arc::new(AtomicU64::new(0)),
            search_cancel: Arc::new(AtomicBool::new(false)),
            comments,
            collapsed_comments: HashSet::new(),
            hovered_comment: None,
            active_comment: None,
            overview_page_idx: 0,
            overview_scroll_y: Cell::new(0.0),
            overview_scroll_remainder: 0.0,
            comment_popup_hovered: false,
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
                    .center_of_page(&self.page_bounds, self.translation, *self.viewport.borrow())
                    .unwrap();
                let next = self
                    .layout
                    .center_of_page_below(
                        &self.page_bounds,
                        self.translation,
                        *self.viewport.borrow(),
                    )
                    .unwrap();

                self.translation.y += (next.center().y - current.center().y) as f64;
            }
            PdfMessage::PreviousPage => {
                let current = self
                    .layout
                    .center_of_page(&self.page_bounds, self.translation, *self.viewport.borrow())
                    .unwrap();
                let prev = self
                    .layout
                    .center_of_page_above(
                        &self.page_bounds,
                        self.translation,
                        *self.viewport.borrow(),
                    )
                    .unwrap();

                self.translation.y += (prev.center().y - current.center().y) as f64;
            }
            PdfMessage::SetPage(idx) => {
                if idx < page_count
                    && let Ok(translation) = self.layout.translation_for_page(
                        &self.page_bounds,
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
                if page_layout == PageLayoutKind::Overview
                    && self.layout.layout != PageLayoutKind::Overview
                {
                    self.layout_before_overview = self.layout.layout;
                    // Start the overview selection on the page that is
                    // currently on screen.
                    self.overview_page_idx = self.current_page();
                    self.overview_scroll_y.set(0.0);
                    self.overview_scroll_remainder = 0.0;
                }
                self.layout.layout = page_layout;
            }
            PdfMessage::ExitOverview(navigate) => {
                if self.layout.layout != PageLayoutKind::Overview {
                    return iced::Task::none();
                }
                self.layout.layout = self.layout_before_overview;
                self.overview_scroll_y.set(0.0);
                self.overview_scroll_remainder = 0.0;
                if navigate {
                    let idx = self.overview_page_idx.min(page_count.saturating_sub(1));
                    if let Ok(translation) = self.layout.translation_for_page(
                        &self.page_bounds,
                        self.scale,
                        self.fractional_scaling,
                        idx,
                        *self.viewport.borrow(),
                    ) {
                        self.translation = translation;
                    }
                }
            }
            PdfMessage::RotatePageClockwise => {
                if page_count > 0 {
                    let page_idx = self.current_page();
                    self.rotate_preserving_page_center(page_idx, |layout| {
                        layout.rotate_page_clockwise(page_idx);
                    });
                }
            }
            PdfMessage::RotatePageCounterClockwise => {
                if page_count > 0 {
                    let page_idx = self.current_page();
                    self.rotate_preserving_page_center(page_idx, |layout| {
                        layout.rotate_page_counter_clockwise(page_idx);
                    });
                }
            }
            PdfMessage::RotateAllPagesClockwise => {
                if page_count > 0 {
                    let page_idx = self.current_page();
                    self.rotate_preserving_page_center(page_idx, |layout| {
                        layout.rotate_all_pages_clockwise(page_count);
                    });
                }
            }
            PdfMessage::RotateAllPagesCounterClockwise => {
                if page_count > 0 {
                    let page_idx = self.current_page();
                    self.rotate_preserving_page_center(page_idx, |layout| {
                        layout.rotate_all_pages_counter_clockwise(page_count);
                    });
                }
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
                let viewport = *self.viewport.borrow();
                if let Ok((scale, translation)) = self.layout.zoom_fit(
                    &self.page_bounds,
                    page_idx,
                    self.fractional_scaling,
                    viewport,
                ) {
                    self.scale = scale;
                    self.translation = translation;
                }
            }
            PdfMessage::Move(vector) => {
                self.translation += vector;
            }
            PdfMessage::MouseMoved(vector) => {
                let old_local = self.local_mouse_pos();
                let pointer_moved = vector != self.mouse_pos;
                self.mouse_pos = vector;
                let new_local = self.local_mouse_pos();
                match self.mouse_interaction {
                    MouseInteraction::None => {}
                    MouseInteraction::Panning => {
                        out = iced::Task::done(PdfMessage::Move(
                            (old_local - new_local)
                                .scaled(1.0 / (self.scale * self.fractional_scaling))
                                .into(),
                        ))
                    }
                    MouseInteraction::Selecting => {
                        self.selection_end = Some(new_local);
                    }
                }
                // Ignore duplicate cursor events so keyboard selection isn't reset under a
                // stationary pointer.
                if pointer_moved && self.layout.layout == PageLayoutKind::Overview {
                    let viewport = *self.viewport.borrow();
                    if let Ok(rects) = self.page_rects(viewport)
                        && let Some(page_idx) =
                            rects.iter().position(|rect| rect.contains(new_local))
                    {
                        self.overview_page_idx = page_idx;
                    }
                }
                self.update_hover_state();
            }
            PdfMessage::MouseAction(mouse_action, pressed) => {
                if self.comment_popup_hovered {
                    self.mouse_interaction = MouseInteraction::None;
                    self.selection_start = None;
                    self.selection_end = None;
                } else if pressed {
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
                            out = iced::Task::done(PdfMessage::Move(Vector::new(
                                0.0,
                                -(MOVE_STEP as f64),
                            )));
                        }
                        MouseAction::MoveDown => {
                            out = iced::Task::done(PdfMessage::Move(Vector::new(
                                0.0,
                                MOVE_STEP as f64,
                            )));
                        }
                        MouseAction::MoveLeft => {
                            out = iced::Task::done(PdfMessage::Move(Vector::new(
                                -(MOVE_STEP as f64),
                                0.0,
                            )));
                        }
                        MouseAction::MoveRight => {
                            out = iced::Task::done(PdfMessage::Move(Vector::new(
                                MOVE_STEP as f64,
                                0.0,
                            )));
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
                                                    && let Err(e) = clipboard.set_text(text)
                                                {
                                                    error!(
                                                        "Failed to copy search result to clipboard: {}",
                                                        e
                                                    );
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
                self.comment_popup_hovered = false;
            }
            PdfMessage::CommentPopupHovered(hovered) => {
                self.comment_popup_hovered = hovered;
            }
            PdfMessage::ToggleCommentCollapse(comment_id) => {
                if !self.collapsed_comments.remove(&comment_id) {
                    self.collapsed_comments.insert(comment_id);
                }
            }
            PdfMessage::FileChanged => {
                self.render_cache.borrow_mut().clear();
                self.allocation_cache.borrow_mut().clear();
                self.pixmap_pool.borrow_mut().clear();

                if let Some(path_str) = self.path.to_str()
                    && let Ok(new_doc) = mupdf::Document::open(path_str)
                    && let Ok((display_lists, links, outline, comments, page_bounds)) =
                        Self::build_document_data(&new_doc)
                {
                    self.doc = new_doc;
                    self.display_lists = display_lists;
                    self.page_bounds = page_bounds;
                    self.links = links;
                    self.outline = outline;
                    self.comments = comments;
                    self.collapsed_comments.clear();
                    self.active_comment = None;
                    self.hovered_comment = None;
                    self.comment_popup_hovered = false;
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
                    (-(vp.height / (self.scale * self.fractional_scaling))) as f64,
                )));
            }
            PdfMessage::PageDown => {
                let vp = self.viewport.borrow();
                out = iced::Task::done(PdfMessage::Move(Vector::new(
                    0.0,
                    (vp.height / (self.scale * self.fractional_scaling)) as f64,
                )));
            }
            PdfMessage::HalfPageUp => {
                let vp = self.viewport.borrow();
                out = iced::Task::done(PdfMessage::Move(Vector::new(
                    0.0,
                    (-(vp.height / (self.scale * self.fractional_scaling * 2.0))) as f64,
                )));
            }
            PdfMessage::HalfPageDown => {
                let vp = self.viewport.borrow();
                out = iced::Task::done(PdfMessage::Move(Vector::new(
                    0.0,
                    (vp.height / (self.scale * self.fractional_scaling * 2.0)) as f64,
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
                        &self.page_bounds,
                        self.scale,
                        self.fractional_scaling,
                        page_idx,
                        *self.viewport.borrow(),
                    ) {
                        let page_bounds: Rect<f32> = self.display_lists[page_idx].bounds().into();
                        let rotation = self.layout.rotation(page_idx);
                        let page_center = rotation.rotated_size(page_bounds.size()).scaled(0.5);
                        let match_rect = m.rects[0].1;
                        let rotated_match = transform_rect(match_rect, |point| {
                            rotation.to_rotated(point, page_bounds)
                        });
                        let match_center = rotated_match.center();
                        // Center vertically.
                        self.translation.y =
                            base_translation.y + (match_center.y - page_center.y) as f64;
                        // Horizontal: adjust minimally from current pan to keep match visible.
                        let viewport = *self.viewport.borrow();
                        let effective_scale = self.scale * self.fractional_scaling;
                        let half_viewport = viewport.width / (2.0 * effective_scale);
                        let lower_bound = rotated_match.x1.x - page_center.x - half_viewport;
                        let upper_bound = rotated_match.x0.x - page_center.x + half_viewport;
                        if lower_bound > upper_bound {
                            // Wider than viewport: center horizontally.
                            self.translation.x = (match_center.x - page_center.x) as f64;
                        } else if self.translation.x < lower_bound as f64 {
                            self.translation.x = lower_bound as f64;
                        } else if self.translation.x > upper_bound as f64 {
                            self.translation.x = upper_bound as f64;
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
                self.invalidate_search();
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
                self.invalidate_search();
                out = self.spawn_search_task();
            }
            PdfMessage::SearchResultsReady(matches, generation) => {
                if generation == self.search_generation.load(Ordering::Relaxed) {
                    self.search_matches = matches;
                    self.current_search_result = None;
                }
            }
            PdfMessage::None => {}
        }
        out
    }

    pub fn view(&self, mode: BindingMode) -> iced::Element<'_, PdfMessage> {
        widget::responsive(move |size| {
            {
                let mut viewport = self.viewport.borrow_mut();
                *viewport = size;
            }
            let rects = self.page_rects(size).unwrap();
            let viewport_rect =
                Rect::from_pos_size(Vector::zero(), Vector::new(size.width, size.height));

            // Drop pixmap allocations for pages that are no longer visible.
            let visible_indices: Vec<usize> = match mode {
                BindingMode::Normal | BindingMode::Overview => rects
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| viewport_rect.intersects(r))
                    .map(|(i, _)| i)
                    .collect(),
                BindingMode::Presentation => {
                    let current_page = self.current_page();
                    (0..rects.len()).filter(|i| *i == current_page).collect()
                }
            };

            self.pixmap_pool
                .borrow_mut()
                .retain(|idx, _| visible_indices.contains(idx));
            self.buffer_pool
                .lock()
                .unwrap()
                .retain(|idx, _| visible_indices.contains(idx));

            let mut used_keys = vec![];
            let with_handles: Vec<_> = visible_indices
                .into_iter()
                .map(|i| (i, rects[i]))
                .filter(|(_, r)| viewport_rect.intersects(r))
                .map(|(i, rect_ss)| {
                    // rect_ss = A pages bounding box in screen coordinates (relative to the widgets origin)
                    let page_bounds = self.page_bounds[i];

                    let effective_scale = match self.layout.layout {
                        PageLayoutKind::Overview => {
                            overview_render_scale(page_bounds, self.layout.rotation(i), rect_ss)
                        }
                        _ => self.scale * self.fractional_scaling,
                    };

                    let TilePlan {
                        key,
                        draw_rect,
                        width: w,
                        height: h,
                        matrix,
                        scissor,
                    } = plan_tile(
                        i,
                        page_bounds,
                        self.layout.rotation(i),
                        rect_ss,
                        effective_scale,
                        viewport_rect,
                    );

                    // Try to reuse a pixmap allocation for this page.
                    let mut pix = {
                        let mut pool = self.pixmap_pool.borrow_mut();
                        pool.remove(&i)
                    };
                    match pix {
                        Some(_) => {}
                        None => {
                            let mut new_pix =
                                Pixmap::new_with_w_h(&Colorspace::device_rgb(), w, h, true)
                                    .unwrap();
                            self.run(&mut new_pix, i, &matrix, scissor, key);
                            pix = Some(new_pix);
                        }
                    }
                    let mut pix = pix.unwrap();

                    // If the pooled pixmap has the wrong size, allocate a new one.
                    if pix.width() as i32 != w || pix.height() as i32 != h {
                        let _span = tracy_client::span!("Pixmap bounds mismatch");
                        pix = Pixmap::new_with_w_h(&Colorspace::device_rgb(), w, h, true).unwrap();

                        if matches!(key, RenderKey::Full(_, _, _)) {
                            self.run(&mut pix, i, &matrix, scissor, key);
                        }
                    }
                    if matches!(key, RenderKey::Partial(_, _, _, _, _, _, _)) {
                        self.run(&mut pix, i, &matrix, scissor, key);
                    }
                    // NOTE: I am not 100% sure how the key can be missing at this point, but it can
                    // happen. This is a fallback filling of the cache in case that happens.
                    let is_cache_missing = { !self.render_cache.borrow_mut().contains_key(&key) };
                    if is_cache_missing {
                        self.run(&mut pix, i, &matrix, scissor, key);
                    }

                    {
                        let mut pool = self.pixmap_pool.borrow_mut();
                        pool.insert(i, pix);
                    }

                    used_keys.push(key);
                    let cache = self.render_cache.borrow_mut();
                    (i, cache[&key].clone(), draw_rect)
                })
                .collect();

            {
                let mut cache = self.render_cache.borrow_mut();
                cache.retain(|key, _| used_keys.contains(key));
            }

            {
                let render_cache = self.render_cache.borrow();
                let active_ids: HashSet<_> = render_cache.values().map(|h| h.id()).collect();
                self.allocation_cache
                    .borrow_mut()
                    .retain(|id, _| active_ids.contains(id));
            }

            let pages_canvas = widget::canvas(Document::new(
                &self.allocation_cache,
                with_handles,
                self.draw_page_borders,
                self.pdf_dark_mode,
                if self.layout.layout == PageLayoutKind::Overview {
                    Some(self.overview_page_idx)
                } else {
                    None
                },
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

            if let Some(popup) = self.view_comment_popup(size) {
                stack_children.push(popup);
            }

            widget::Stack::with_children(stack_children)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .into()
        })
        .into()
    }

    fn run(
        &self,
        pix: &mut Pixmap,
        i: usize,
        matrix: &Matrix,
        scissor: mupdf::Rect,
        key: RenderKey,
    ) -> image::Handle {
        let _span = tracy_client::span!("run");
        pix.samples_mut().fill(255);
        let device = Device::from_pixmap(pix).unwrap();
        self.display_lists[i].run(&device, matrix, scissor).unwrap();
        if self.pdf_dark_mode {
            cpu_pdf_dark_mode_shader(pix, &self.gradient_cache);
        }
        let mut cache = self.render_cache.borrow_mut();
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

        let handle = image::Handle::from_rgba(
            pix.width(),
            pix.height(),
            Bytes::from_owner(PooledBuffer {
                buf: Some(buf),
                pool: Arc::downgrade(&self.buffer_pool),
                page_idx: i,
            }),
        );
        cache.insert(key, handle.clone());

        handle
    }

    fn view_comment_node<'a>(&'a self, comment: &'a Comment) -> iced::Element<'a, PdfMessage> {
        let has_replies = !comment.replies.is_empty();
        let is_collapsed = self.collapsed_comments.contains(&comment.id);
        let author = comment.author.as_deref().unwrap_or("Unknown author");
        let reply_count: iced::Element<'_, PdfMessage> = if has_replies {
            widget::button(
                widget::text(format!(
                    "{} {} replies",
                    if is_collapsed { "[+]" } else { "[-]" },
                    comment.replies.len(),
                ))
                .style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    widget::text::Style {
                        color: Some(palette.primary.base.color),
                        ..Default::default()
                    }
                })
                .size(14.0),
            )
            .style(widget::button::text)
            .padding(2.0)
            .on_press(PdfMessage::ToggleCommentCollapse(comment.id))
            .into()
        } else {
            widget::space::horizontal().into()
        };
        let header = widget::row![
            reply_count,
            widget::space::horizontal().width(iced::Length::Fill),
            widget::text(author)
                .font(iced::Font {
                    style: iced::font::Style::Italic,
                    ..Default::default()
                })
                .style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    widget::text::Style {
                        color: Some(palette.primary.base.color),
                        ..Default::default()
                    }
                }),
        ]
        .align_y(iced::alignment::Vertical::Center)
        .spacing(8);

        let mut body = widget::column![header].spacing(6.0);
        if let Some(content) = comment.content.as_deref() {
            body = body.push(
                widget::text(content)
                    .size(14.0)
                    .wrapping(widget::text::Wrapping::Word),
            );
        }
        if !is_collapsed {
            for reply in &comment.replies {
                body = body.push(self.view_comment_node(reply));
            }
        }

        let line = widget::rule::vertical(2.0).style(|theme: &iced::Theme| {
            let mut color = theme.extended_palette().primary.base.color;
            color.a = 0.65;
            widget::rule::Style {
                color,
                radius: iced::border::Radius::from(1.0),
                fill_mode: widget::rule::FillMode::Full,
                snap: true,
            }
        });
        widget::row![
            line,
            widget::container(body).padding(iced::Padding::new(0.0).left(6.0)),
        ]
        .spacing(8.0)
        .height(iced::Length::Shrink)
        .into()
    }

    fn view_comment_popup(
        &self,
        viewport_size: iced::Size,
    ) -> Option<iced::Element<'_, PdfMessage>> {
        let active_idx = self.active_comment?;
        let comment_visible = self.visible_comments(viewport_size);
        let (_, comment_rect) = comment_visible.iter().find(|(idx, _)| *idx == active_idx)?;

        const POPUP_MARGIN: f32 = 8.0;
        const POPUP_HEIGHT_WITHOUT_THREAD: f32 = 60.0;
        let popup_width = 280.0_f32.min(viewport_size.width - 16.0).max(120.0);
        let popup_x = comment_rect.x1.x + 8.0;
        let popup_y = comment_rect.x0.y;

        // Keep the popup's full height available regardless of its anchor position. It is moved
        // upward to fit first; scrolling is only needed when the thread exceeds the viewport.
        let max_thread_height =
            (viewport_size.height - 2.0 * POPUP_MARGIN - POPUP_HEIGHT_WITHOUT_THREAD).max(0.0);
        let thread = widget::container(
            widget::scrollable(self.view_comment_node(&self.comments[active_idx]))
                .width(iced::Length::Fill)
                .height(iced::Length::Shrink),
        )
        .width(iced::Length::Fill)
        .max_height(max_thread_height);

        let popup = widget::container(thread)
            .width(popup_width)
            .padding(16.0)
            .style(|theme: &iced::Theme| widget::container::Style {
                background: Some(theme.extended_palette().background.weak.color.into()),
                border: iced::Border {
                    color: theme.extended_palette().primary.base.color,
                    width: 2.0,
                    radius: iced::border::Radius::from(8.0),
                },
                ..Default::default()
            });

        let positioned = widget::float(
            widget::mouse_area(popup)
                .on_press(PdfMessage::None)
                .on_enter(PdfMessage::CommentPopupHovered(true))
                .on_exit(PdfMessage::CommentPopupHovered(false)),
        )
        .translate(move |bounds, viewport| {
            let anchor = iced::Point::new(bounds.x + popup_x, bounds.y + popup_y);
            let position = popup_position(anchor, bounds.size(), viewport);
            iced::Vector::new(position.x - bounds.x, position.y - bounds.y)
        });

        Some(positioned.into())
    }

    #[allow(clippy::type_complexity)]
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

    /// Cancel any in-flight search scans and start a fresh generation for the next one.
    fn invalidate_search(&mut self) {
        self.search_cancel.store(true, Ordering::Relaxed);
        self.search_cancel = Arc::new(AtomicBool::new(false));
        self.search_generation.fetch_add(1, Ordering::Relaxed);
    }

    fn spawn_search_task(&self) -> iced::Task<PdfMessage> {
        let text_contents = self.text_contents.clone();
        let needle = self.needle.clone();
        let method = self.search_method;
        let char_bboxes = self.char_bboxes.clone();
        let cancel = self.search_cancel.clone();
        let generation_counter = self.search_generation.clone();
        let generation = generation_counter.load(Ordering::Relaxed);

        iced::Task::perform(
            async move {
                // Debounce: typing spawns one task per keystroke, but only the newest
                // generation gets past this sleep and starts an actual scan.
                tokio::time::sleep(SEARCH_DEBOUNCE).await;
                if generation_counter.load(Ordering::Relaxed) != generation {
                    return Ok(Vec::new());
                }
                tokio::task::spawn_blocking(move || {
                    find_search_matches(&text_contents, &needle, method, &char_bboxes, &cancel)
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

        let viewport = *self.viewport.borrow();

        let Ok(rects) = self.page_rects(viewport) else {
            return String::new();
        };

        let mut result = String::new();

        for (i, page_rect) in rects.iter().enumerate() {
            let intersect = screen_rect.intersect(page_rect);
            if intersect.width() <= 0.0 || intersect.height() <= 0.0 {
                continue;
            }

            let page_bounds: Rect<f32> = self.display_lists[i].bounds().into();
            let rotation = self.layout.rotation(i);
            let rotated_size = rotation.rotated_size(page_bounds.size());
            let scale_x = page_rect.width() / rotated_size.x;
            let scale_y = page_rect.height() / rotated_size.y;
            let screen_to_pdf = |point: Vector<f32>| {
                rotation.from_rotated(
                    Vector::new(
                        (point.x - page_rect.x0.x) / scale_x,
                        (point.y - page_rect.x0.y) / scale_y,
                    ),
                    page_bounds,
                )
            };
            let pdf_start = screen_to_pdf(intersect.x0);
            let pdf_end = screen_to_pdf(intersect.x1);
            let pdf_rect = mupdf::Rect::new(
                pdf_start.x.min(pdf_end.x),
                pdf_start.y.min(pdf_end.y),
                pdf_start.x.max(pdf_end.x),
                pdf_start.y.max(pdf_end.y),
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
        let Ok(page_rects) = self.page_rects(viewport) else {
            return result;
        };

        let viewport_rect = Rect::from_pos_size(Vector::zero(), viewport.into());

        for (page_idx, page_rect) in page_rects.iter().enumerate() {
            if !viewport_rect.intersects(page_rect) {
                continue;
            }
            let page_bounds: Rect<f32> = self.display_lists[page_idx].bounds().into();

            for (link_idx, link) in self.links[page_idx].iter().enumerate() {
                let link_bounds: Rect<f32> = link.bounds.into();
                let screen_rect = pdf_rect_to_screen(
                    link_bounds,
                    page_bounds,
                    *page_rect,
                    self.layout.rotation(page_idx),
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
        let Ok(page_rects) = self.page_rects(viewport) else {
            return result;
        };

        let viewport_rect = Rect::from_pos_size(Vector::zero(), viewport.into());

        for (page_idx, page_rect) in page_rects.iter().enumerate() {
            if !viewport_rect.intersects(page_rect) {
                continue;
            }
            let page_bounds: Rect<f32> = self.display_lists[page_idx].bounds().into();

            for (match_idx, m) in self.search_matches.iter().enumerate() {
                for &(rect_page_idx, rect) in &m.rects {
                    if rect_page_idx != page_idx {
                        continue;
                    }
                    let screen_rect = pdf_rect_to_screen(
                        rect,
                        page_bounds,
                        *page_rect,
                        self.layout.rotation(page_idx),
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
        let Ok(page_rects) = self.page_rects(viewport) else {
            return result;
        };

        let viewport_rect = Rect::from_pos_size(Vector::zero(), viewport.into());

        for (page_idx, page_rect) in page_rects.iter().enumerate() {
            if !viewport_rect.intersects(page_rect) {
                continue;
            }
            let page_bounds: Rect<f32> = self.display_lists[page_idx].bounds().into();

            for (comment_idx, comment) in self.comments.iter().enumerate() {
                if comment.page_idx != page_idx {
                    continue;
                }
                let Some(comment_bounds) = comment.bounds else {
                    continue;
                };
                let comment_bounds: Rect<f32> = comment_bounds.into();
                let screen_rect = pdf_rect_to_screen(
                    comment_bounds,
                    page_bounds,
                    *page_rect,
                    self.layout.rotation(page_idx),
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
            self.allocation_cache.borrow_mut().clear();
            self.buffer_pool.lock().unwrap().clear();
            self.pixmap_pool.borrow_mut().clear();
        }
    }

    pub fn set_interface_dark_mode(&mut self, dark_mode_enabled: bool) {
        if self.interface_dark_mode != dark_mode_enabled {
            self.interface_dark_mode = dark_mode_enabled;
            self.render_cache.borrow_mut().clear();
            self.allocation_cache.borrow_mut().clear();
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

    pub fn is_pointer_over_viewport(&self) -> bool {
        let local_mouse = self.local_mouse_pos();
        let viewport = *self.viewport.borrow();
        local_mouse.x >= 0.0
            && local_mouse.y >= 0.0
            && local_mouse.x < viewport.width
            && local_mouse.y < viewport.height
    }

    pub fn current_page(&self) -> usize {
        if self.layout.layout == PageLayoutKind::Overview {
            return self
                .overview_page_idx
                .min(self.page_bounds.len().saturating_sub(1));
        }
        self.layout
            .current_page_index(&self.page_bounds, self.translation, *self.viewport.borrow())
            .unwrap()
    }

    fn page_rects(&self, viewport: Size<f32>) -> Result<Vec<Rect<f32>>> {
        let mut rects = self.layout.pages_rects(
            &self.page_bounds,
            -self.translation,
            self.scale,
            self.fractional_scaling,
            viewport,
        )?;
        if self.layout.layout == PageLayoutKind::Overview {
            let mut scroll_y = self.overview_scroll_y.get();
            apply_overview_scroll(&mut rects, self.overview_page_idx, viewport, &mut scroll_y);
            self.overview_scroll_y.set(scroll_y);
        }
        Ok(rects)
    }

    fn page_screen_center(&self, page_idx: usize, viewport: Size<f32>) -> Option<Vector<f32>> {
        let rects = self.page_rects(viewport).ok()?;
        rects.get(page_idx).map(Rect::center)
    }

    pub fn scroll_overview_lines(&mut self, lines: f32) {
        self.scroll_overview_steps(-lines);
    }

    pub fn scroll_overview_pixels(&mut self, pixels: f32) {
        self.scroll_overview_steps(pixels / OVERVIEW_SCROLL_STEP_PIXELS);
    }

    fn scroll_overview_steps(&mut self, steps: f32) {
        if self.layout.layout != PageLayoutKind::Overview || !steps.is_finite() {
            return;
        }

        self.overview_scroll_remainder =
            (self.overview_scroll_remainder + steps.clamp(-10.0, 10.0)).clamp(-10.0, 10.0);
        while self.overview_scroll_remainder.abs() >= 1.0 {
            let step = if self.overview_scroll_remainder > 0.0 {
                1.0
            } else {
                -1.0
            };
            let previous_page = self.overview_page_idx;
            let last_page = self.page_bounds.len().saturating_sub(1);
            self.overview_page_idx = if step > 0.0 {
                previous_page.saturating_add(1).min(last_page)
            } else {
                previous_page.saturating_sub(1)
            };
            if self.overview_page_idx == previous_page {
                self.overview_scroll_remainder = 0.0;
                break;
            }
            self.overview_scroll_remainder -= step;
        }
    }

    /// Moves the highlighted overview thumbnail one step in `direction`.
    ///
    /// Left/Right stay within the current visual row; Up/Down pick the closest
    /// thumbnail in the target direction, which lands on the column nearest the
    /// current one when rows are uneven. Does nothing when not in the overview
    /// layout or when already at an edge.
    pub fn move_overview_selection(&mut self, direction: MoveDirection) {
        if self.layout.layout != PageLayoutKind::Overview {
            return;
        }
        let viewport = *self.viewport.borrow();
        let Ok(rects) = self.page_rects(viewport) else {
            return;
        };
        self.overview_page_idx = self.overview_page_idx.min(rects.len().saturating_sub(1));
        let Some(current_rect) = rects.get(self.overview_page_idx) else {
            return;
        };
        let center = current_rect.center();

        let mut best: Option<(usize, f32)> = None;
        for (i, rect) in rects.iter().enumerate() {
            if i == self.overview_page_idx {
                continue;
            }
            let candidate_center = rect.center();
            let in_direction = match direction {
                MoveDirection::Left => {
                    // Same visual row: the vertical ranges of the thumbnails overlap.
                    rect.x0.y < current_rect.x1.y
                        && rect.x1.y > current_rect.x0.y
                        && candidate_center.x < center.x
                }
                MoveDirection::Right => {
                    rect.x0.y < current_rect.x1.y
                        && rect.x1.y > current_rect.x0.y
                        && candidate_center.x > center.x
                }
                MoveDirection::Up => candidate_center.y < center.y,
                MoveDirection::Down => candidate_center.y > center.y,
            };
            if !in_direction {
                continue;
            }
            let distance = (candidate_center - center).norm_squared();
            if best.is_none_or(|(_, best_distance)| distance < best_distance) {
                best = Some((i, distance));
            }
        }

        if let Some((idx, _)) = best {
            self.overview_page_idx = idx;
        }
    }

    fn rotate_preserving_page_center(
        &mut self,
        page_idx: usize,
        rotate: impl FnOnce(&mut PageLayout),
    ) {
        let viewport = *self.viewport.borrow();
        let center_before = self.page_screen_center(page_idx, viewport);
        rotate(&mut self.layout);
        let center_after = self.page_screen_center(page_idx, viewport);
        let effective_scale = self.scale * self.fractional_scaling;

        if let (Some(before), Some(after)) = (center_before, center_after)
            && effective_scale.is_finite()
            && effective_scale.abs() > f32::EPSILON
        {
            self.translation += (after - before).scaled(1.0 / effective_scale).into();
        }
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
    for pixel in samples.as_chunks_mut::<4>().0 {
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
    fn test_plan_tile_full_page_scissor_is_in_device_space() {
        // A page scaled up to exactly fit the viewport (as happens after
        // ZoomFit). The scissor must cover the whole pixmap, otherwise the
        // bottom/right of the page is culled away.
        let page_bounds = Rect::from_pos_size(Vector::new(0.0, 0.0), Vector::new(300.0, 200.0));
        let effective_scale = 2.0;
        let rect_ss = Rect::from_pos_size(Vector::new(100.0, 100.0), Vector::new(600.0, 400.0));
        let viewport_rect = Rect::from_pos_size(Vector::new(0.0, 0.0), Vector::new(800.0, 600.0));

        let plan = plan_tile(
            0,
            page_bounds,
            PageRotation::Deg0,
            rect_ss,
            effective_scale,
            viewport_rect,
        );

        assert_eq!(
            plan.key,
            RenderKey::Full(0, effective_scale.to_bits(), PageRotation::Deg0)
        );
        assert_eq!(plan.draw_rect, rect_ss);
        assert_eq!(plan.width, 600);
        assert_eq!(plan.height, 400);
        assert_eq!(
            plan.scissor,
            mupdf::Rect::new(0.0, 0.0, 600.0, 400.0),
            "full-page scissor must be in device/pixmap coordinates, not page coordinates"
        );
    }

    #[test]
    fn test_plan_tile_partial_page_scissor_covers_visible_region() {
        let page_bounds = Rect::from_pos_size(Vector::new(0.0, 0.0), Vector::new(300.0, 200.0));
        let effective_scale = 2.0;
        let rect_ss = Rect::from_pos_size(Vector::new(-100.0, -50.0), Vector::new(600.0, 400.0));
        let viewport_rect = Rect::from_pos_size(Vector::new(0.0, 0.0), Vector::new(800.0, 600.0));

        let plan = plan_tile(
            0,
            page_bounds,
            PageRotation::Deg0,
            rect_ss,
            effective_scale,
            viewport_rect,
        );

        assert_eq!(
            plan.key,
            RenderKey::Partial(
                0,
                effective_scale.to_bits(),
                500,
                350,
                (-100.0_f32).to_bits(),
                (-50.0_f32).to_bits(),
                PageRotation::Deg0,
            )
        );
        assert_eq!(
            plan.draw_rect,
            Rect::from_pos_size(Vector::new(0.0, 0.0), Vector::new(500.0, 350.0))
        );
        assert_eq!(plan.scissor, mupdf::Rect::new(0.0, 0.0, 500.0, 350.0));
    }

    #[test]
    fn rotating_all_pages_keeps_the_current_page_center_fixed() -> Result<()> {
        let mut viewer = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
        let viewport = iced::Size::new(800.0, 600.0);
        viewer.set_viewport_for_test(viewport);
        viewer.layout = PageLayout::new(PageLayoutKind::DoublePage);
        let page_idx = viewer.current_page();

        let center = |viewer: &PdfViewer| -> Result<Vector<f32>> {
            let rects = viewer.layout.pages_rects(
                &viewer.page_bounds,
                -viewer.translation,
                viewer.scale,
                viewer.fractional_scaling,
                viewport,
            )?;
            Ok(rects[page_idx].center())
        };
        let before = center(&viewer)?;

        let _ = viewer.update(PdfMessage::RotateAllPagesClockwise);

        let after = center(&viewer)?;
        assert!(
            (after - before).norm_squared() < 1e-3,
            "rotation should keep page {page_idx} centered at {before:?}, got {after:?}"
        );
        Ok(())
    }

    #[test]
    fn test_zoom_fit_scales_current_page_to_viewport() -> Result<()> {
        let mut viewer = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
        let viewport = iced::Size::new(800.0, 600.0);
        viewer.set_viewport_for_test(viewport);
        viewer.layout = PageLayout::new(PageLayoutKind::SinglePage);

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
            &viewer.page_bounds,
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
    fn test_zoom_fit_centers_page_spread() -> Result<()> {
        let viewport = iced::Size::new(800.0, 600.0);

        // Reference scale when fitting a single page.
        let mut single = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
        single.set_viewport_for_test(viewport);
        single.layout = PageLayout::new(PageLayoutKind::SinglePage);
        let _ = single.update(PdfMessage::ZoomFit);

        // (layout, current page, first/last page of the spread that should be fit).
        // The non-zero current pages cover spreads that sit far below the viewport at
        // zero translation, where an incorrect translation sign loses the document.
        let cases = [
            (
                PageLayout::new(PageLayoutKind::DoublePage),
                0usize,
                (0usize, 1usize),
            ),
            (PageLayout::new(PageLayoutKind::DoublePage), 2, (2, 2)),
            (
                PageLayout::new(PageLayoutKind::DoublePageTitlePage),
                2,
                (1, 2),
            ),
        ];

        for (layout, page, (first, last)) in cases {
            let mut viewer = PdfViewer::from_path(PathBuf::from("assets/links.pdf"))?;
            viewer.set_viewport_for_test(viewport);
            viewer.layout = layout;
            let _ = viewer.update(PdfMessage::SetPage(page));
            assert_eq!(viewer.current_page(), page);

            let _ = viewer.update(PdfMessage::ZoomFit);
            let current = viewer.current_page();
            assert!(
                (first..=last).contains(&current),
                "ZoomFit should stay on the fitted spread {first}..={last}, got page {current}"
            );

            // The whole spread must fit within the viewport and be centered, rather than
            // centering only the page nearest the middle of the screen.
            let rects = viewer.layout.pages_rects(
                &viewer.page_bounds,
                -viewer.translation,
                viewer.scale,
                viewer.fractional_scaling,
                viewport,
            )?;
            let spread = (first..=last).fold(rects[first], |acc, i| acc.union(&rects[i]));
            assert!(
                spread.x0.x >= -1e-3 && spread.x1.x <= viewport.width + 1e-3,
                "spread x-range {:?}-{:?} should fit in viewport width {}",
                spread.x0.x,
                spread.x1.x,
                viewport.width
            );
            assert!(
                spread.x0.y >= -1e-3 && spread.x1.y <= viewport.height + 1e-3,
                "spread y-range {:?}-{:?} should fit in viewport height {}",
                spread.x0.y,
                spread.x1.y,
                viewport.height
            );
            let center = spread.center();
            assert!(
                (center.x - viewport.width / 2.0).abs() < 1e-3
                    && (center.y - viewport.height / 2.0).abs() < 1e-3,
                "spread center {:?} should be at the viewport center",
                center
            );

            // Fitting a two-page spread must zoom out compared to fitting one page.
            if first != last {
                assert!(
                    viewer.scale < single.scale,
                    "double page fit scale {} should be smaller than single page scale {}",
                    viewer.scale,
                    single.scale
                );
            }
        }

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
            &AtomicBool::new(false),
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
            &AtomicBool::new(false),
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
            &AtomicBool::new(false),
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
            &AtomicBool::new(false),
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
            &AtomicBool::new(false),
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
            &AtomicBool::new(false),
        );
        let regex = find_search_matches(
            &viewer.text_contents,
            "Link Extraction",
            SearchMethod::Regex,
            &viewer.char_bboxes,
            &AtomicBool::new(false),
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
            &AtomicBool::new(false),
        );
        let regex = find_search_matches(
            &viewer.text_contents,
            "Code Blocks",
            SearchMethod::Regex,
            &viewer.char_bboxes,
            &AtomicBool::new(false),
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
            &AtomicBool::new(false),
        );
        assert!(result.is_empty(), "should not find nonexistent text");
        Ok(())
    }

    #[test]
    fn test_clicking_comment_popup_does_not_dismiss_it() -> Result<()> {
        let mut viewer = PdfViewer::from_path(PathBuf::from("assets/links_commented.pdf"))?;
        viewer.active_comment = Some(0);
        let _ = viewer.update(PdfMessage::CommentPopupHovered(true));
        let _ = viewer.update(PdfMessage::MouseAction(MouseAction::Selection, false));

        assert_eq!(viewer.active_comment, Some(0));
        assert!(matches!(viewer.mouse_interaction, MouseInteraction::None));
        Ok(())
    }

    #[test]
    fn test_replies_preserve_nested_parent_relationships() {
        let annotation =
            |object_id, in_reply_to, annotation_type, content: &str| AnnotationCommentData {
                page_idx: 0,
                bounds: Some(mupdf::Rect::new(0.0, 0.0, 10.0, 10.0)),
                content: Some(content.to_string()),
                author: None,
                annotation_type,
                object_id: Some(object_id),
                in_reply_to,
            };
        let comments = build_comments(vec![
            annotation(20, None, PdfAnnotationType::Ink, "original ink note"),
            annotation(21, Some(20), PdfAnnotationType::Text, "first reply"),
            annotation(22, Some(21), PdfAnnotationType::Text, "nested reply"),
            annotation(23, Some(20), PdfAnnotationType::Text, "sibling reply"),
        ]);

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].content.as_deref(), Some("original ink note"));
        assert_eq!(comments[0].replies.len(), 2);
        assert_eq!(
            comments[0].replies[0].content.as_deref(),
            Some("first reply")
        );
        assert_eq!(comments[0].replies[0].replies.len(), 1);
        assert_eq!(
            comments[0].replies[0].replies[0].content.as_deref(),
            Some("nested reply")
        );
        assert_eq!(
            comments[0].replies[1].content.as_deref(),
            Some("sibling reply")
        );
    }

    #[test]
    fn test_orphaned_and_cyclic_replies_are_preserved() {
        let annotation = |object_id, in_reply_to, content: &str| AnnotationCommentData {
            page_idx: 0,
            bounds: Some(mupdf::Rect::new(0.0, 0.0, 10.0, 10.0)),
            content: Some(content.to_string()),
            author: None,
            annotation_type: PdfAnnotationType::Text,
            object_id: Some(object_id),
            in_reply_to,
        };
        let comments = build_comments(vec![
            annotation(30, None, "standalone"),
            annotation(31, Some(999), "orphan"),
            annotation(32, Some(33), "cycle root"),
            annotation(33, Some(32), "cycle reply"),
        ]);

        assert_eq!(comments.len(), 3);
        assert_eq!(comments[0].content.as_deref(), Some("standalone"));
        assert_eq!(comments[1].content.as_deref(), Some("orphan"));
        assert_eq!(comments[2].content.as_deref(), Some("cycle root"));
        assert_eq!(comments[2].replies.len(), 1);
        assert_eq!(
            comments[2].replies[0].content.as_deref(),
            Some("cycle reply")
        );
    }

    #[test]
    fn test_comment_extraction_from_commented_pdf() -> Result<()> {
        let viewer = PdfViewer::from_path(PathBuf::from("assets/links_commented.pdf"))?;
        assert!(
            !viewer.comments.is_empty(),
            "should extract at least one comment from links_commented.pdf"
        );
        for comment in &viewer.comments {
            assert!(
                comment
                    .content
                    .as_deref()
                    .is_some_and(|content| !content.is_empty()),
                "comment content should not be empty"
            );
            assert!(
                comment
                    .bounds
                    .is_some_and(|bounds| { bounds.x1 > bounds.x0 && bounds.y1 > bounds.y0 }),
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
