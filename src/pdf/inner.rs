use iced::{
    ContentFit, Length, Size,
    advanced::{Widget, image, layout},
    overlay::menu::default,
    widget::image::FilterMethod,
};
use mupdf::Page;

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
    width: Length,
    height: Length,
    content_fit: ContentFit,
    filter_method: FilterMethod,
    translation: Vector<f32>,
    scale: f32,
}

impl<'a> PageViewer<'a> {
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

    fn visible_bbox(&self) -> mupdf::IRect {
        let page_bounds = self.page.bounds().unwrap();
        let mut out_box = Rect::<f32>::from(page_bounds);
        out_box.translate(out_box.size().scaled(0.5));
        out_box.translate(self.translation);
        out_box.scale(self.scale);
        out_box.into()
    }
}

impl<'a, Theme, Renderer> Widget<PdfMessage, Theme, Renderer> for PageViewer<'a>
where
    Renderer: image::Renderer<Handle = image::Handle>,
{
    fn size(&self) -> iced::Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &self,
        _tree: &mut iced::advanced::widget::Tree,
        _renderer: &Renderer,
        _limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let page_bounds = self.page.bounds().unwrap();
        layout::Node::new(Size::new(page_bounds.width(), page_bounds.height()))
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        let image = {
            use mupdf::{Colorspace, Device, Matrix, Pixmap};
            // Generate image of pdf
            let mut matrix = Matrix::default();
            matrix.scale(self.scale.x, self.scale.y);
            let pixmap =
                Pixmap::new_with_rect(&Colorspace::device_rgb(), self.visible_bbox(), true)
                    .unwrap();
            let device = Device::from_pixmap(&pixmap).unwrap();
            self.page.run(&device, &matrix).unwrap();
            image::Handle::from_rgba(pixmap.width(), pixmap.height(), pixmap.samples().to_vec())
        };

        // Render said image onto the screen
        let bounds = layout.bounds();
        let render = |renderer: &mut Renderer| {
            renderer.draw_image(
                image::Image {
                    handle: image,
                    filter_method: self.filter_method,
                    rotation: iced::Radians::from(0.0),
                    opacity: 1.0,
                    snap: true,
                },
                bounds,
            );
        };
        renderer.with_layer(bounds, render);
    }
}
