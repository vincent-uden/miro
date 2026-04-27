use std::{path::PathBuf};

use anyhow::Result;
use colorgrad::{Gradient as _, GradientBuilder, LinearGradient};
use iced::{
    Renderer,
    widget::{
        self,
        canvas::{self, Cache, Stroke},
    },
};
use tracing::debug;

use crate::{
    DARK_THEME,
    config::{MOVE_STEP, MouseAction},
    geometry::{Rect, Vector},
    pdf::{PdfMessage, outline_extraction::OutlineItem, page_layout::PageLayout},
};

const MIN_SELECTION: f32 = 5.0;
const MIN_CLICK_DISTANCE: f32 = 5.0;

// NOTE: The primitive might not end up being a page here but rather the entire document. Regardless
// using a canvas allows us to sidestep creating a custom widget entirely. This should be the
// simpler approach.
#[derive(Debug)]
struct Document {
    cache: Cache,
    // TODO: This should be a texture rather than a color
    pages: Vec<(iced::Color, Rect<f32>)>,
}

impl Document {
    pub fn new(pages: Vec<(iced::Color, Rect<f32>)>) -> Self {
        Self {
            cache: Cache::default(),
            pages,
        }
    }
}

impl<'a> widget::canvas::Program<PdfMessage> for Document {
    type State = ();

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &iced::Theme,
        bounds: iced::Rectangle,
        cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let bg = self.cache.draw(renderer, bounds.size(), |frame| {
            frame.fill_text("Hello world!");
            for (color, rect) in &self.pages {
                let mut c = *color;
                c.a = 0.2;
                frame.fill_rectangle((rect.x0).into(), rect.size().into(), c);
                frame.stroke_rectangle(
                    (rect.x0).into(),
                    rect.size().into(),
                    Stroke::default().with_color(*color).with_width(1.0),
                );
            }
        });
        vec![bg]
    }
}

#[derive(Debug)]
pub enum MouseInteraction {
    None,
    Panning,
    Selecting,
}

/// Renders a pdf document. Owns all information related to the document.
#[derive(Debug)]
pub struct PdfViewer {
    pub name: String,
    pub path: PathBuf,

    pub invert_colors: bool,
    pub draw_page_borders: bool,

    doc: mupdf::Document,

    pub translation: Vector<f32>,
    pub scale: f32,
    fractional_scaling: f32,

    mouse_pos: Vector<f32>,
    mouse_pressed_at: Vector<f32>,
    mouse_interaction: MouseInteraction,

    layout: PageLayout,

    gradient_cache: [[u8; 4]; 256],
}

impl PdfViewer {
    pub fn from_path(path: PathBuf) -> Result<Self> {
        let name = path
            .file_name()
            .expect("The pdf must have a file name")
            .to_string_lossy()
            .to_string();
        let doc = mupdf::Document::open(&path.to_str().unwrap())?;

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
            invert_colors: false,
            draw_page_borders: true,
            doc,
            translation: Vector::zero(),
            scale: 1.0,
            fractional_scaling: 1.0,
            layout: PageLayout::TwoPage,
            gradient_cache,
            mouse_pos: Vector::zero(),
            mouse_pressed_at: Vector::zero(),
            mouse_interaction: MouseInteraction::None,
        })
    }

    pub fn update(&mut self, msg: PdfMessage) -> iced::Task<PdfMessage> {
        let mut out = iced::Task::none();
        //debug!("PdfViewer::update({msg:?})");
        match msg {
            PdfMessage::PageDown => {}
            PdfMessage::PageUp => {}
            PdfMessage::SetPage(_) => {}
            PdfMessage::SetTranslation(vector) => {}
            PdfMessage::SetLocation(vector, _) => {}
            PdfMessage::SetLayout(page_layout) => {
                self.layout = page_layout;
            }
            PdfMessage::ZoomIn => {
                self.scale *= 1.2;
            }
            PdfMessage::ZoomOut => {
                self.scale /= 1.2;
            }
            PdfMessage::ZoomHome => {}
            PdfMessage::ZoomFit => {}
            PdfMessage::Move(vector) => {
                self.translation += vector;
            }
            PdfMessage::MouseMoved(vector) => {
                match self.mouse_interaction {
                    MouseInteraction::None => {}
                    MouseInteraction::Panning => {
                        out = iced::Task::done(PdfMessage::Move(
                            (vector - self.mouse_pos).scaled(self.scale * self.fractional_scaling),
                        ))
                    }
                    MouseInteraction::Selecting => todo!(),
                }
                self.mouse_pos = vector;
            }
            PdfMessage::MouseLeftDown(shift_pressed) => {
                if shift_pressed {
                    // TODO: Selection
                } else {
                    self.mouse_interaction = MouseInteraction::Panning;
                    self.mouse_pressed_at = self.mouse_pos;
                }
            }
            PdfMessage::MouseAction(mouse_action, pressed) => {
                if pressed {
                    match mouse_action {
                        MouseAction::Panning => {
                            self.mouse_interaction = MouseInteraction::Panning;
                            self.mouse_pressed_at = self.mouse_pos;
                        }
                        MouseAction::Selection => {
                            self.mouse_interaction = MouseInteraction::Selecting;
                            self.mouse_pressed_at = self.mouse_pos;
                        }
                        MouseAction::NextPage => {
                            out = iced::Task::done(PdfMessage::PageDown);
                        }
                        MouseAction::PreviousPage => {
                            out = iced::Task::done(PdfMessage::PageDown);
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
                        MouseInteraction::None | MouseInteraction::Panning => {}
                        MouseInteraction::Selecting => {
                            // TODO: Copy text
                        }
                    }
                    self.mouse_interaction = MouseInteraction::None;
                }
            }
            PdfMessage::ToggleLinkHitboxes => {}
            PdfMessage::ActivateLink(_) => {}
            PdfMessage::CloseLinkHitboxes => {}
            PdfMessage::FileChanged => {}
            PdfMessage::PrintPdf => {}
            PdfMessage::None => {}
        }
        out
    }

    pub fn view(&self) -> iced::Element<'_, PdfMessage> {
        widget::responsive(|size| {
            let rects = self
                .layout
                .pages_rects(
                    &self.doc,
                    self.translation.scaled(-1.0),
                    self.scale,
                    self.fractional_scaling,
                    size,
                )
                .unwrap();
            let with_colors: Vec<_> = rects
                .into_iter()
                .map(|r| (iced::Color::from_rgba(1.0, 1.0, 1.0, 1.0), r))
                .collect();
            widget::canvas(Document::new(with_colors))
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .into()
        })
        .into()
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

    pub fn page_progress(&self) -> &str {
        // TODO: Implement
        "(? / ?)"
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
