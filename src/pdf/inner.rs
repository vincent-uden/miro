use iced::{
    Border, Color, ContentFit, Element, Length, Size,
    advanced::{Layout, Widget, image, layout, renderer::Quad, widget::Tree},
    widget::image::FilterMethod,
};
use mupdf::Page;
use tracing::debug;

use crate::geometry::{Rect, Vector};

use super::PdfMessage;

#[derive(Debug, Default)]
pub struct State {
    pub bounds: Rect<f32>,
}

#[derive(Debug)]
pub struct PageViewer<'a> {
    page: &'a Page,
    state: &'a State,
    // TODO: Maybe remove these?
    width: Length,
    height: Length,
    content_fit: ContentFit,
    // ---
    filter_method: FilterMethod,
    translation: Vector<f32>,
    scale: f32,
}

impl<'a> PageViewer<'a> {
    pub fn new(page: &'a Page, state: &'a State) -> Self {
        Self {
            page,
            state,
            width: Length::Fill,
            height: Length::Fill,
            content_fit: ContentFit::None,
            filter_method: FilterMethod::Nearest,
            translation: Vector::zero(),
            scale: 1.0,
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

    fn visible_bbox(&self) -> mupdf::IRect {
        let page_bounds = self.page.bounds().unwrap();
        let mut out_box = Rect::<f32>::from(self.state.bounds);
        out_box.translate(self.translation.scaled(self.scale));
        out_box.translate(Vector::new(
            -(self.state.bounds.width() - page_bounds.width() * self.scale) / 2.0,
            -(self.state.bounds.height() - page_bounds.height() * self.scale) / 2.0,
        ));
        out_box.into()
    }
}

impl<'a, Renderer> Widget<PdfMessage, iced::Theme, Renderer> for PageViewer<'a>
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
        theme: &iced::Theme,
        _style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        _viewport: &iced::Rectangle,
    ) {
        // TODO: This might be leaking memory. Could be related to wl_registry still attached
        let mut img_bounds = layout.bounds();
        let image = {
            use mupdf::{Colorspace, Device, Matrix, Pixmap};
            // Generate image of pdf
            let mut matrix = Matrix::default();
            matrix.scale(self.scale, self.scale);
            let mut pixmap =
                Pixmap::new_with_rect(&Colorspace::device_rgb(), self.visible_bbox(), true)
                    .unwrap();
            let bg_color = theme.extended_palette().background.base.color.into_rgba8();
            for (i, p) in pixmap.samples_mut().iter_mut().enumerate() {
                if i % 4 == 0 {
                    *p = bg_color[0];
                } else if i % 4 == 1 {
                    *p = bg_color[1];
                } else if i % 4 == 2 {
                    *p = bg_color[2];
                } else if i % 4 == 3 {
                    *p = 255;
                }
            }
            let device = Device::from_pixmap(&pixmap).unwrap();
            self.page.run(&device, &matrix).unwrap();
            img_bounds.width = pixmap.width() as f32;
            img_bounds.height = pixmap.height() as f32;
            image::Handle::from_rgba(pixmap.width(), pixmap.height(), pixmap.samples().to_vec())
        };
        let bounds = layout.bounds();

        // Render said image onto the screen
        let render = |renderer: &mut Renderer| {
            renderer.fill_quad(
                Quad {
                    bounds,
                    ..Default::default()
                },
                Color::BLACK,
            );
            renderer.draw_image(
                image::Image {
                    handle: image,
                    filter_method: self.filter_method,
                    rotation: iced::Radians::from(0.0),
                    opacity: 1.0,
                    snap: true,
                },
                img_bounds,
            );
        };
        renderer.with_layer(img_bounds, render);
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
        }
        iced::event::Status::Ignored
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

#[cfg(test)]
mod tests {
    use mupdf::{Colorspace, Device, Document, Matrix, Pixmap};

    use super::*;

    const LANDSCAPE_PDF: &[u8] = include_bytes!("../../assets/tribo_storlek.pdf");

    #[test]
    pub fn non_transformed_bbox() {
        let doc = Document::from_bytes(LANDSCAPE_PDF, "pdf").unwrap();
        let page = doc.load_page(0).unwrap();
        let state = State::default();
        let viewer = PageViewer::new(&page, &state).scale(1.0);
        let bbox: Rect<i32> = viewer.visible_bbox().into();
        let expected = Rect::from_points(Vector::new(0, 0), Vector::new(1296, 432));
        assert_eq!(bbox, expected)
    }

    #[test]
    pub fn zoomed_in_bbox() {
        let scale = 2.0;
        let doc = Document::from_bytes(LANDSCAPE_PDF, "pdf").unwrap();
        let page = doc.load_page(0).unwrap();
        let state = State::default();
        let viewer = PageViewer::new(&page, &state).scale(scale);
        let bbox: Rect<i32> = viewer.visible_bbox().into();
        let expected = Rect::from_points(Vector::new(324, 108), Vector::new(972, 324));
        assert_eq!(bbox, expected);
    }
}
