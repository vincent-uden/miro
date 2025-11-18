use std::cell::Ref;
use iced::{
    Border, ContentFit, Element, Length, Shadow, Size,
    advanced::{Layout, Widget, image, layout, renderer::Quad, widget::Tree},
    border::Radius,
    widget::image::FilterMethod,
};
use mupdf::{DisplayList, Pixmap};

use crate::{
    geometry::{Rect, Vector},
};

use super::{
    PdfMessage,
    link_extraction::{LinkInfo, LinkType},
};

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

/// Contains the state required to rasterize the currently shown page of a pdf.
#[derive(Debug)]
pub struct State {
    /// The viewport bounds
    pub bounds: Rect<f32>,
    pub page_size: Vector<f32>,
    pub list: DisplayList,
    /// The pixmap can only be allocated once we know the bounds of the widget
    pub pix: Option<Pixmap>,
}

#[derive(Debug)]
pub struct PageViewer<'a> {
    state: Ref<'a, State>,
    width: Length,
    height: Length,
    content_fit: ContentFit,
    filter_method: FilterMethod,
    translation: Vector<f32>,
    scale: f32,
    invert_colors: bool,
    draw_page_borders: bool,
    text_selection_rect: Option<Rect<f32>>,
    link_hitboxes: Option<&'a Vec<LinkInfo>>,
    link_keys: Option<Vec<String>>,
    pending_link_key: String,
    is_over_link: bool,
}

impl<'a> PageViewer<'a> {
    pub fn new(state: Ref<'a, State>) -> Self {
        Self {
            state,
            width: Length::Fill,
            height: Length::Fill,
            content_fit: ContentFit::None,
            filter_method: FilterMethod::Nearest,
            translation: Vector::zero(),
            scale: 1.0,
            invert_colors: false,
            draw_page_borders: false,
            text_selection_rect: None,
            link_hitboxes: None,
            link_keys: None,
            pending_link_key: String::new(),
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

    pub fn draw_page_borders(mut self, draw: bool) -> Self {
        self.draw_page_borders = draw;
        self
    }

    pub fn text_selection(mut self, rect: Option<Rect<f32>>) -> Self {
        self.text_selection_rect = rect;
        self
    }

    pub fn link_hitboxes(mut self, links: Option<&'a Vec<LinkInfo>>) -> Self {
        self.link_hitboxes = links;
        self.link_keys = links.map(|l| generate_key_combinations(l.len()));
        self
    }

    pub fn over_link(mut self, is_over_link: bool) -> Self {
        self.is_over_link = is_over_link;
        self
    }
}

impl<Renderer> Widget<PdfMessage, iced::Theme, Renderer> for PageViewer<'_>
where
    Renderer:
        image::Renderer<Handle = image::Handle> + iced::advanced::text::Renderer<Font = iced::Font>,
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
        // It is probably possible to modify the mupdf-rs library to store the pixels in a Bytes
        // struct. This would allow for zero-copy sharing of the bytes in the image handle, rather
        // than the expensive clone we are doing now.
        let scaled_page_size = self.state.page_size.scaled(self.scale);
        let pdf_center = Vector::new(
            (viewport_bounds.width - scaled_page_size.x) / 2.0,
            (viewport_bounds.height - scaled_page_size.y) / 2.0,
        );
        let top_left =
            pdf_center - self.translation.scaled(self.scale) + viewport_bounds.position().into();
        let page_bounds = iced::Rectangle::new(top_left.into(), scaled_page_size.into());
        let pdf_layer_bounds = if self.draw_page_borders {
            page_bounds
        } else {
            viewport_bounds
        };

        let draw_background = |renderer: &mut Renderer| {
            if self.draw_page_borders {
                renderer.fill_quad(
                    Quad {
                        bounds: viewport_bounds.into(),
                        border: Border::default(),
                        shadow: Shadow::default(),
                    },
                    get_background_color(self.invert_colors),
                );
            }
        };

        let draw_pdf = |renderer: &mut Renderer| {
            if let Some(pix) = &self.state.pix {
                if pix.width() > 0 && pix.height() > 0 {
                    renderer.draw_image(
                        image::Image {
                            handle: image::Handle::from_rgba(
                                pix.width(),
                                pix.height(),
                                pix.samples().to_vec(),
                            ),
                            filter_method: FilterMethod::Nearest,
                            rotation: iced::Radians::from(0.0),
                            opacity: 1.0,
                            snap: true,
                        },
                        viewport_bounds,
                    );
                }
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
                    let doc_rect = link.bounds;

                    let link_bounds = Rect::from_points(
                        (doc_rect.x0 - self.translation).scaled(self.scale)
                            + pdf_center
                            + viewport_bounds.position().into(),
                        (doc_rect.x1 - self.translation).scaled(self.scale)
                            + pdf_center
                            + viewport_bounds.position().into(),
                    );

                    let (border_color, fill_color) = get_link_colors(&link.link_type);

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
                }
            }
        };

        let draw_key_hints = |renderer: &mut Renderer| {
            if let (Some(links), Some(keys)) = (self.link_hitboxes, &self.link_keys) {
                for (link, key) in links.iter().zip(keys.iter()) {
                    let doc_rect = link.bounds;

                    let link_bounds = Rect::from_points(
                        (doc_rect.x0 - self.translation).scaled(self.scale)
                            + pdf_center
                            + viewport_bounds.position().into(),
                        (doc_rect.x1 - self.translation).scaled(self.scale)
                            + pdf_center
                            + viewport_bounds.position().into(),
                    );

                    let link_center_y = (link_bounds.x0.y + link_bounds.x1.y) / 2.0 + 2.0;
                    let text_height = 16.0;
                    let bounds_top = link_center_y - text_height / 2.0;
                    let hint_position = iced::Point::new(link_bounds.x1.x + 2.0, bounds_top);
                    let hint_bounds = iced::Rectangle::new(
                        hint_position,
                        iced::Size::new(12.0 * key.len() as f32, text_height),
                    );

                    let (border_color, _) = get_link_colors(&link.link_type);

                    renderer.fill_quad(
                        Quad {
                            bounds: hint_bounds,
                            border: Border {
                                color: border_color,
                                width: 1.0,
                                radius: Radius::from(3.0),
                            },
                            shadow: Shadow::default(),
                        },
                        border_color,
                    );

                    let text_color = if self.invert_colors {
                        iced::Color::WHITE
                    } else {
                        iced::Color::BLACK
                    };

                    renderer.fill_text(
                        iced::advanced::text::Text {
                            content: key.clone(),
                            bounds: iced::Size::new(24.0, text_height),
                            size: iced::Pixels(12.0),
                            line_height: iced::widget::text::LineHeight::default(),
                            font: iced::Font::default(),
                            horizontal_alignment: iced::alignment::Horizontal::Left,
                            vertical_alignment: iced::alignment::Vertical::Top,
                            shaping: iced::advanced::text::Shaping::default(),
                            wrapping: iced::advanced::text::Wrapping::default(),
                        },
                        hint_position + Vector::new(2.0, 2.0).into(),
                        text_color,
                        hint_bounds,
                    );
                }
            }
        };

        renderer.with_layer(viewport_bounds, draw_background);
        renderer.with_layer(pdf_layer_bounds, draw_pdf);
        renderer.with_layer(viewport_bounds, draw_selection);
        renderer.with_layer(viewport_bounds, draw_link_hitboxes);
        renderer.with_layer(viewport_bounds, draw_key_hints);
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
        let mut event_handled = false;
        let out = match event {
            iced::Event::Window(event) => {
                if let iced::window::Event::Opened {
                    position: _,
                    size: _,
                } = event
                {
                    Some(PdfMessage::UpdateBounds(bounds.into()))
                } else {
                    None
                }
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                match key {
                    // Handle Escape key to close links (hardcoded, not configurable)
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                        if modifiers.is_empty() && self.link_hitboxes.is_some() {
                            event_handled = true;
                            Some(PdfMessage::CloseLinkHitboxes)
                        } else {
                            None
                        }
                    }
                    iced::keyboard::Key::Character(ref key_char) => {
                        if let Some(keys) = &self.link_keys
                            && !keys.is_empty()
                        {
                            event_handled = true;
                            self.pending_link_key.push_str(key_char);
                            if self.pending_link_key.len() >= keys[0].len() {
                                keys.iter()
                                    .position(|k| **k == self.pending_link_key)
                                    .map(PdfMessage::ActivateLink)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(msg) = out {
            shell.publish(msg);
        } else if self.state.bounds.size() == Vector::zero() || self.state.bounds != bounds.into() {
            shell.publish(PdfMessage::UpdateBounds(bounds.into()));
        }
        if event_handled {
            iced::event::Status::Captured
        } else {
            iced::event::Status::Ignored
        }
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
    Renderer:
        image::Renderer<Handle = image::Handle> + iced::advanced::text::Renderer<Font = iced::Font>,
{
    fn from(value: PageViewer<'a>) -> Self {
        Element::new(value)
    }
}
