use std::{path::PathBuf};

use anyhow::Result;
use colorgrad::{Gradient as _, GradientBuilder, LinearGradient};
use iced::{
    Renderer,
    widget::{self, canvas},
};
use mupdf::Document;

use crate::{
    geometry::Vector,
    pdf::{PdfMessage, outline_extraction::OutlineItem, page_layout::PageLayout},
};

const MIN_SELECTION: f32 = 5.0;
const MIN_CLICK_DISTANCE: f32 = 5.0;

// NOTE: The primitive might not end up being a page here but rather the entire document. Regardless
// using a canvas allows us to sidestep creating a custom widget entirely. This should be the
// simpler approach.
#[derive(Debug)]
struct Page {}

impl widget::canvas::Program<PdfMessage> for Page {
    type State = ();

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &iced::Theme,
        bounds: iced::Rectangle,
        cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        todo!()
    }
}

/// Renders a pdf document. Owns all information related to the document.
#[derive(Debug)]
pub struct PdfViewer {
    pub name: String,
    pub path: PathBuf,
    pub page_progress: String,

    pub invert_colors: bool,
    pub draw_page_borders: bool,

    doc: Document,

    pub translation: Vector<f32>,
    pub scale: f32,
    fractional_scaling: f32,

    layout: PageLayout,
}

impl PdfViewer {
    pub fn from_path(path: PathBuf) -> Result<Self> {
        todo!()
    }

    pub fn update(&mut self, msg: PdfMessage) -> iced::Task<PdfMessage> {
        todo!()
    }

    pub fn view(&self) -> iced::Element<'_, PdfMessage> {
        widget::responsive(|size| widget::canvas(Page {}).into()).into()
    }

    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        self.fractional_scaling = scale_factor as f32;
    }

    pub fn is_jumpable_action(&self, msg: &PdfMessage) -> bool {
        // TODO: Implement
        false
    }

    pub fn get_outline(&self) -> &[OutlineItem] {
        // TODO: Implement
        &[]
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

fn get_background_color(invert_colors: bool) -> iced::Color {
    if invert_colors {
        iced::Color::from_rgb8(21, 22, 32)
    } else {
        iced::Color::from_rgb8(220, 219, 218)
    }
}
