use std::collections::HashMap;

use colorgrad::{Gradient, GradientBuilder, LinearGradient};
use iced::{
    Border, ContentFit, Element, Length, Shadow, Size,
    advanced::{Layout, Widget, image, layout, renderer::Quad, widget::Tree},
    border::Radius,
    widget::image::FilterMethod,
};
use mupdf::{DisplayList, Pixmap};
use tracing::debug;

use crate::{
    geometry::{Rect, Vector},
};

use super::{
    PdfMessage,
    link_extraction::{LinkInfo, LinkType},
};

/// Contains the state required to rasterize the currently shown page of a pdf.
#[derive(Debug)]
pub struct State {
    /// The viewport bounds
    pub bounds: Rect<f32>,
    pub list: DisplayList,
    /// The pixmap can only be allocated once we know the bounds of the widget
    pub pix: Option<Pixmap>,
    pub img: Option<image::Handle>,
}

#[derive(Debug)]
pub struct PageViewer<'a> {
    state: &'a State,
    width: Length,
    height: Length,
    content_fit: ContentFit,
    filter_method: FilterMethod,
    translation: Vector<f32>,
    scale: f32,
    invert_colors: bool,
    text_selection_rect: Option<Rect<f32>>,
    link_hitboxes: Option<&'a Vec<LinkInfo>>,
    is_over_link: bool,
}

impl<'a> PageViewer<'a> {
    pub fn new(state: &'a State) -> Self {
        Self {
            state,
            width: Length::Fill,
            height: Length::Fill,
            content_fit: ContentFit::None,
            filter_method: FilterMethod::Nearest,
            translation: Vector::zero(),
            scale: 1.0,
            invert_colors: false,
            text_selection_rect: None,
            link_hitboxes: None,
            is_over_link: false,
        }
    }
    /// Sets the width of the [`Image`] boundaries.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Image`] boundaries.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the [`ContentFit`] of the [`Image`] depicting the page.
    ///
    /// Defaults to [`ContentFit::Contain`]
    pub fn content_fit(mut self, content_fit: ContentFit) -> Self {
        self.content_fit = content_fit;
        self
    }

    /// Sets the [`FilterMethod`] of the [`Image`] depicting the page.
    pub fn filter_method(mut self, filter_method: FilterMethod) -> Self {
        self.filter_method = filter_method;
        self
    }

    /// Sets the translation of the viewport on page.
    pub fn translation(mut self, translation: Vector<f32>) -> Self {
        self.translation = translation;
        self
    }

    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    pub fn invert_colors(mut self, invert: bool) -> Self {
        self.invert_colors = invert;
        self
    }

    pub fn text_selection(mut self, rect: Option<Rect<f32>>) -> Self {
        self.text_selection_rect = rect;
        self
    }

    pub fn link_hitboxes(mut self, links: Option<&'a Vec<LinkInfo>>) -> Self {
        self.link_hitboxes = links;
        self
    }

    pub fn over_link(mut self, is_over_link: bool) -> Self {
        self.is_over_link = is_over_link;
        self
    }
}

impl<Renderer> Widget<PdfMessage, iced::Theme, Renderer> for PageViewer<'_>
where
    Renderer: image::Renderer<Handle = image::Handle>,
{
    fn size(&self) -> iced::Size<Length> {
        Size::new(
            match self.width {
                Length::Fill => Length::Fill,
                _ => self.width,
            },
            match self.height {
                Length::Fill => Length::Fill,
                _ => self.height,
            },
        )
    }

    fn layout(
        &self,
        _tree: &mut iced::advanced::widget::Tree,
        _renderer: &Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let size = limits.resolve(self.width, self.height, Size::default());
        layout::Node::new(size)
    }

    fn draw(
        &self,
        _tree: &iced::advanced::widget::Tree,
        renderer: &mut Renderer,
        _theme: &iced::Theme,
        _style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        _viewport: &iced::Rectangle,
    ) {
        let viewport_bounds = layout.bounds();
        let draw_pdf = |renderer: &mut Renderer| {
            if let Some(img) = &self.state.img {
                renderer.draw_image(
                    image::Image {
                        handle: img.clone(), // TODO: Can we avoid this clone?
                        filter_method: FilterMethod::Nearest,
                        rotation: iced::Radians::from(0.0),
                        opacity: 1.0,
                        snap: true,
                    },
                    viewport_bounds,
                );
            }
        };

        let draw_selection = |renderer: &mut Renderer| {
            if let Some(rect) = self.text_selection_rect {
                // Draw selection rectangle with semi-transparent blue fill and blue border
                renderer.fill_quad(
                    Quad {
                        bounds: rect.into(),
                        border: Border {
                            color: iced::Color::from_rgb(0.0, 0.5, 1.0), // Blue border
                            width: 2.0,
                            radius: Radius::from(0.0),
                        },
                        shadow: Shadow::default(),
                    },
                    iced::Color::from_rgba(0.0, 0.5, 1.0, 0.2), // Semi-transparent blue fill
                );
            }
        };

        let draw_link_hitboxes = |renderer: &mut Renderer| {
            if let Some(links) = self.link_hitboxes {
                for link in links {
                    // TODO: Enable once we have access to page bounds
                    let doc_rect = link.bounds;
                    /* if let Some(page) = self.page_info {
                        let scaled_page_size = page.size.scaled(self.scale);
                        let pdf_center = Vector::new(
                            (img_bounds.width - scaled_page_size.x) / 2.0,
                            (img_bounds.height - scaled_page_size.y) / 2.0,
                        );

                        let offset = pdf_center - self.translation.scaled(self.scale);
                        let mut link_bounds = Rect::from_points(
                            doc_rect.x0.scaled(self.scale),
                            doc_rect.x1.scaled(self.scale),
                        );
                        link_bounds.translate(offset);

                        let (border_color, fill_color) = match link.link_type {
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
                        };

                        renderer.fill_quad(
                            Quad {
                                bounds: link_bounds.into(),
                                border: Border {
                                    color: border_color,
                                    width: 1.5,
                                    radius: Radius::from(2.0),
                                },
                                shadow: Shadow::default(),
                            },
                            fill_color,
                        );
                    } */
                }
            }
        };

        renderer.with_layer(viewport_bounds, draw_pdf);
        renderer.with_layer(viewport_bounds, draw_selection);
        renderer.with_layer(viewport_bounds, draw_link_hitboxes);
    }

    fn on_event(
        &mut self,
        _state: &mut Tree,
        event: iced::Event,
        layout: Layout<'_>,
        _cursor: iced::mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, PdfMessage>,
        _viewport: &iced::Rectangle,
    ) -> iced::advanced::graphics::core::event::Status {
        let bounds = layout.bounds();
        let out = match event {
            iced::Event::Window(event) => match event {
                iced::window::Event::Opened {
                    position: _,
                    size: _,
                } => Some(PdfMessage::UpdateBounds(bounds.into())),
                iced::window::Event::Moved(_) => Some(PdfMessage::UpdateBounds(bounds.into())),
                iced::window::Event::Resized(_) => Some(PdfMessage::UpdateBounds(bounds.into())),
                _ => None,
            },
            _ => None,
        };
        if let Some(msg) = out {
            shell.publish(msg);
        } else if self.state.bounds.size() == Vector::zero() || self.state.bounds != bounds.into() {
            shell.publish(PdfMessage::UpdateBounds(bounds.into()));
        }
        iced::event::Status::Ignored
    }

    fn mouse_interaction(
        &self,
        _state: &Tree,
        layout: Layout<'_>,
        cursor: iced::mouse::Cursor,
        _viewport: &iced::Rectangle,
        _renderer: &Renderer,
    ) -> iced::mouse::Interaction {
        if cursor.is_over(layout.bounds()) && self.is_over_link {
            iced::mouse::Interaction::Pointer
        } else {
            iced::mouse::Interaction::default()
        }
    }
}

impl<'a, Renderer> From<PageViewer<'a>> for Element<'a, PdfMessage, iced::Theme, Renderer>
where
    Renderer: image::Renderer<Handle = image::Handle>,
{
    fn from(value: PageViewer<'a>) -> Self {
        Element::new(value)
    }
}

pub fn cpu_pdf_dark_mode_shader(pixmap: &mut Pixmap, bg_color: &[u8; 4]) {
    let samples = pixmap.samples_mut();
    for i in 0..(samples.len() / 4) {
        let a = samples[i * 4 + 3];
        // If the background is transparent, assume it's suppused to be white
        if a < 255 {
            samples[i * 4 + 3] = 255;
        }
    }
    let gradient = GradientBuilder::new()
        .colors(&[
            colorgrad::Color::from_rgba8(255, 255, 255, 255),
            colorgrad::Color::from_rgba8(bg_color[0], bg_color[1], bg_color[2], 255),
        ])
        .build::<LinearGradient>()
        .unwrap();
    for i in 0..(samples.len() / 4) {
        let r: u16 = samples[i * 4] as u16;
        let g: u16 = samples[i * 4 + 1] as u16;
        let b: u16 = samples[i * 4 + 2] as u16;
        if samples[i * 4..i * 4 + 3] == bg_color[0..3] {
            continue;
        }
        let brightness = ((r + g + b) as f32) / (255.0 * 3.0);
        let [r_out, g_out, b_out, _] = gradient.at(brightness).to_rgba8();
        samples[i * 4] = r_out;
        samples[i * 4 + 1] = g_out;
        samples[i * 4 + 2] = b_out;
    }
}
