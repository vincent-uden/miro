use std::{
    fs::{self, File},
    io::{BufWriter, Write},
};

use anyhow::{Result, anyhow};
use colorgrad::{Gradient, GradientBuilder, LinearGradient};
use iced::{
    Color, ContentFit, Element, Length, Size,
    advanced::{Layout, Widget, image, layout, renderer::Quad, widget::Tree},
    widget::image::FilterMethod,
};
use mupdf::{Page, Pixmap};
use tracing::error;

use crate::{
    DARK_THEME, LIGHT_THEME,
    geometry::{Rect, Vector},
};

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
    invert_colors: bool,
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
            invert_colors: false,
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

    fn visible_bbox(&self) -> mupdf::IRect {
        let page_bounds = self.page.bounds().unwrap();
        let mut out_box = self.state.bounds;
        out_box.translate(self.translation.scaled(self.scale));
        out_box.translate(Vector::new(
            -(self.state.bounds.width() - page_bounds.width() * self.scale) / 2.0,
            -(self.state.bounds.height() - page_bounds.height() * self.scale) / 2.0,
        ));
        out_box.into()
    }

    fn render_page(&self) -> Result<Pixmap> {
        use mupdf::{Colorspace, Device, Matrix, Pixmap};
        // Generate image of pdf
        let mut matrix = Matrix::default();
        matrix.scale(self.scale, self.scale);
        let mut pixmap =
            Pixmap::new_with_rect(&Colorspace::device_rgb(), self.visible_bbox(), true).unwrap();
        let bg_color = if self.invert_colors {
            DARK_THEME
                .extended_palette()
                .background
                .base
                .color
                .into_rgba8()
        } else {
            LIGHT_THEME
                .extended_palette()
                .background
                .base
                .color
                .into_rgba8()
        };
        // TODO: Fix this rendering error related to dark mode
        // In the typst report, the aliasing problem occurs on every single page
        // In the latex report, it only happens on pages with images that has backgrounds? (page 23
        // at least)
        // page.run doesnt seem to respect the pre-filling of the pixmap, so we must add the
        // background in some other manner. Probably into the actual pdf-structure itself
        for samp in pixmap.samples_mut() {
            *samp = 255;
        }
        let device = Device::from_pixmap(&pixmap).unwrap();
        self.page.run(&device, &matrix).unwrap();
        if self.invert_colors {
            cpu_pdf_dark_mode_shader(&mut pixmap, &bg_color);
        }
        Ok(pixmap)
    }

    pub fn debug_write(&self, path: &str) -> Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        let pixmap = self.render_page()?;
        pixmap.write_to(&mut writer, mupdf::ImageFormat::PNG)?;
        writer.flush()?;

        Ok(())
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
        // TODO: This might be leaking memory. Could be related to wl_registry still attached
        let mut img_bounds = layout.bounds();
        let image = {
            let pixmap = self.render_page().unwrap();
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

pub fn cpu_pdf_dark_mode_shader(pixmap: &mut Pixmap, bg_color: &[u8; 4]) {
    let samples = pixmap.samples_mut();
    for i in 0..(samples.len() / 4) {
        let a = samples[i * 4 + 3];
        // If the background is transparent, assume it's suppused to be white
        if a < 255 {
            error!("This shouldnt happen");
            samples[i * 4] = 255;
            samples[i * 4 + 1] = 255;
            samples[i * 4 + 2] = 255;
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
