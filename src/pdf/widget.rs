use anyhow::Result;
use colorgrad::{Gradient as _, GradientBuilder, LinearGradient};
use iced::{
    Element,
    advanced::{Widget, image},
};
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
        link_extraction::{LinkExtractor, LinkType},
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
pub struct State {}

/// Renders a pdf document. Owns all information related to the document.
#[derive(Debug, Clone, Copy)]
pub struct PdfViewer {}

impl PdfViewer {}

impl<Renderer> Widget<AppMessage, iced::Theme, Renderer> for PdfViewer
where
    Renderer:
        image::Renderer<Handle = image::Handle> + iced::advanced::text::Renderer<Font = iced::Font>,
{
    fn size(&self) -> iced::Size<iced::Length> {
        todo!()
    }

    fn layout(
        &self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        todo!()
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut Renderer,
        theme: &iced::Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        todo!()
    }
}

impl<'a, Renderer> From<PdfViewer> for Element<'a, AppMessage, iced::Theme, Renderer>
where
    Renderer:
        image::Renderer<Handle = image::Handle> + iced::advanced::text::Renderer<Font = iced::Font>,
{
    fn from(value: PdfViewer) -> Self {
        Self::new(value)
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
