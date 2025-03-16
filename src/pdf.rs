use std::path::{Path, PathBuf};

use crate::custom_serde_functions::*;
use iced::{
    Color, ContentFit, Element, Length, Point, Rectangle, Rotation, Size, Vector,
    advanced::{
        Layout, Widget,
        image::{self, FilterMethod},
        layout, mouse, renderer,
        widget::Tree,
    },
    widget::vertical_space,
};
use mupdf::{Colorspace, Document, Matrix};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
pub struct State {
    pub bounds: Rectangle,
}

#[derive(Debug)]
pub struct InnerPdfViewer<'a, Handle = image::Handle> {
    handle: Handle,
    width: Length,
    height: Length,
    content_fit: ContentFit,
    filter_method: FilterMethod,
    rotation: Rotation,
    opacity: f32,
    translation: Vector,
    state: &'a State,
}

impl<'a, Handle> InnerPdfViewer<'a, Handle> {
    /// Creates a new [`Image`] with the given path.
    pub fn new(handle: impl Into<Handle>, state: &'a State) -> Self {
        InnerPdfViewer {
            handle: handle.into(),
            width: Length::Shrink,
            height: Length::Shrink,
            content_fit: ContentFit::None,
            filter_method: FilterMethod::default(),
            rotation: Rotation::default(),
            opacity: 1.0,
            translation: Vector::ZERO,
            state,
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

    /// Sets the [`ContentFit`] of the [`Image`].
    ///
    /// Defaults to [`ContentFit::Contain`]
    pub fn content_fit(mut self, content_fit: ContentFit) -> Self {
        self.content_fit = content_fit;
        self
    }

    /// Sets the [`FilterMethod`] of the [`Image`].
    pub fn filter_method(mut self, filter_method: FilterMethod) -> Self {
        self.filter_method = filter_method;
        self
    }

    /// Applies the given [`Rotation`] to the [`Image`].
    pub fn rotation(mut self, rotation: impl Into<Rotation>) -> Self {
        self.rotation = rotation.into();
        self
    }

    /// Sets the opacity of the [`Image`].
    ///
    /// It should be in the [0.0, 1.0] range—`0.0` meaning completely transparent,
    /// and `1.0` meaning completely opaque.
    pub fn opacity(mut self, opacity: impl Into<f32>) -> Self {
        self.opacity = opacity.into();
        self
    }

    pub fn translation(mut self, translation: Vector) -> Self {
        self.translation = translation;
        self
    }
}

/// Computes the layout of an [`Image`].
pub fn layout<Renderer, Handle>(
    renderer: &Renderer,
    limits: &layout::Limits,
    handle: &Handle,
    width: Length,
    height: Length,
    content_fit: ContentFit,
    rotation: Rotation,
) -> layout::Node
where
    Renderer: image::Renderer<Handle = Handle>,
{
    // The raw w/h of the underlying image
    let image_size = renderer.measure_image(handle);
    let image_size = Size::new(image_size.width as f32, image_size.height as f32);

    // The rotated size of the image
    let rotated_size = rotation.apply(image_size);

    // The size to be available to the widget prior to `Shrink`ing
    let raw_size = limits.resolve(width, height, rotated_size);

    // The uncropped size of the image when fit to the bounds above
    let full_size = content_fit.fit(rotated_size, raw_size);

    // Shrink the widget to fit the resized image, if requested
    let final_size = Size {
        width: match width {
            Length::Shrink => f32::min(raw_size.width, full_size.width),
            _ => raw_size.width,
        },
        height: match height {
            Length::Shrink => f32::min(raw_size.height, full_size.height),
            _ => raw_size.height,
        },
    };

    layout::Node::new(final_size)
}

/// Draws an [`Image`]
pub fn draw<Renderer, Handle>(
    renderer: &mut Renderer,
    layout: Layout<'_>,
    handle: &Handle,
    filter_method: FilterMethod,
    opacity: f32,
    translation: Vector,
) where
    Renderer: image::Renderer<Handle = Handle>,
    Handle: Clone,
{
    let Size { width, height } = renderer.measure_image(handle);
    let image_size = Size::new(width as f32, height as f32);

    let bounds = layout.bounds();
    let final_size = image_size;

    let mut position = Point::new(
        bounds.x + (bounds.width - image_size.width) / 2.0,
        bounds.y + (bounds.height - image_size.height) / 2.0,
    );
    position.x -= translation.x;
    position.y -= translation.y;

    let drawing_bounds = Rectangle::new(position, final_size);

    let render = |renderer: &mut Renderer| {
        renderer.draw_image(
            image::Image {
                handle: handle.clone(),
                filter_method,
                rotation: iced::Radians::from(0.0),
                opacity,
                snap: true,
            },
            drawing_bounds,
        );
    };

    renderer.with_layer(bounds, render);
}

impl<'a, Theme, Renderer, Handle> Widget<PdfMessage, Theme, Renderer> for InnerPdfViewer<'a, Handle>
where
    Renderer: image::Renderer<Handle = Handle>,
    Handle: Clone,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(
        &self,
        _tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout(
            renderer,
            limits,
            &self.handle,
            self.width,
            self.height,
            self.content_fit,
            self.rotation,
        )
    }

    fn draw(
        &self,
        _state: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        draw(
            renderer,
            layout,
            &self.handle,
            self.filter_method,
            self.opacity,
            self.translation,
        )
    }

    fn on_event(
        &mut self,
        _state: &mut Tree,
        event: iced::Event,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, PdfMessage>,
        _viewport: &Rectangle,
    ) -> iced::advanced::graphics::core::event::Status {
        let bounds = layout.bounds();
        let out = match event {
            iced::Event::Window(event) => match event {
                iced::window::Event::Opened {
                    position: _,
                    size: _,
                } => Some(PdfMessage::UpdateBounds(bounds)),
                iced::window::Event::Moved(_) => Some(PdfMessage::UpdateBounds(bounds)),
                iced::window::Event::Resized(_) => Some(PdfMessage::UpdateBounds(bounds)),
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

impl<'a, Theme, Renderer, Handle> From<InnerPdfViewer<'a, Handle>>
    for Element<'a, PdfMessage, Theme, Renderer>
where
    Renderer: image::Renderer<Handle = Handle>,
    Handle: Clone + 'a,
{
    fn from(image: InnerPdfViewer<'a, Handle>) -> Element<'a, PdfMessage, Theme, Renderer> {
        Element::new(image)
    }
}

#[derive(Debug, Default)]
pub struct PdfViewer {
    pub name: String,
    doc: Option<Document>,
    page: i32,
    img_handle: Option<image::Handle>,
    scale: f32,
    translation: Vector,
    inner_state: State,
    initial_page_size: Size,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PdfMessage {
    OpenFile(PathBuf),
    NextPage,
    PreviousPage,
    ZoomIn,
    ZoomOut,
    ZoomHome,
    ZoomFit,
    MoveHorizontal(f32),
    MoveVertical(f32),
    #[serde(
        serialize_with = "serialize_rectangle",
        deserialize_with = "deserialize_rectangle"
    )]
    UpdateBounds(Rectangle),
}

impl PdfViewer {
    pub fn new() -> Self {
        Self {
            scale: 1.0,
            ..Default::default()
        }
    }

    pub fn update(&mut self, message: PdfMessage) -> iced::Task<PdfMessage> {
        match message {
            PdfMessage::NextPage => {
                if self.page
                    < self
                        .doc
                        .as_ref()
                        .map(|d| d.page_count().unwrap_or(0))
                        .unwrap_or(0)
                        - 1
                {
                    self.page += 1;
                    self.show_current_page();
                }
            }
            PdfMessage::PreviousPage => {
                if self.page > 0 {
                    self.page -= 1;
                    self.show_current_page();
                }
            }
            PdfMessage::ZoomIn => {
                self.scale *= 1.1;
                self.show_current_page();
            }
            PdfMessage::ZoomOut => {
                self.scale /= 1.1;
                self.show_current_page();
            }
            PdfMessage::ZoomHome => {
                self.scale = 1.0;
                self.show_current_page();
            }
            PdfMessage::ZoomFit => {
                let x_scale = self.inner_state.bounds.width / (self.initial_page_size.width);
                let y_scale = self.inner_state.bounds.height / (self.initial_page_size.height);
                self.scale = x_scale.min(y_scale);
                self.show_current_page();
            }
            PdfMessage::MoveHorizontal(delta) => {
                self.translation.x += delta;
            }
            PdfMessage::MoveVertical(delta) => {
                self.translation.y += delta;
            }
            PdfMessage::OpenFile(path_buf) => {
                self.load_file(&path_buf);
            }
            PdfMessage::UpdateBounds(rectangle) => {
                self.inner_state.bounds = rectangle;
            }
        }
        iced::Task::none()
    }

    pub fn view(&self) -> iced::Element<'_, PdfMessage> {
        let image: Element<'_, PdfMessage> = if let Some(h) = &self.img_handle {
            InnerPdfViewer::<image::Handle>::new(h, &self.inner_state)
                .width(Length::Fill)
                .height(Length::Fill)
                .translation(self.translation)
                .into()
        } else {
            vertical_space().into()
        };
        image
    }

    fn load_file(&mut self, path: &Path) {
        let doc = Document::open(path.to_str().unwrap()).unwrap();
        self.doc = Some(doc);
        self.page = 0;
        self.show_current_page();
        if let Some(image::Handle::Rgba {
            id: _,
            width,
            height,
            pixels: _,
        }) = self.img_handle
        {
            self.initial_page_size = Size::new(width as f32, height as f32);
        }
        self.name = path.to_string_lossy().to_string();
    }

    fn show_current_page(&mut self) {
        if let Some(doc) = &self.doc {
            let page = doc.load_page(self.page).unwrap();
            let mut matrix = Matrix::default();
            matrix.scale(self.scale, self.scale);
            let pixmap = page
                .to_pixmap(&matrix, &Colorspace::device_rgb(), 1.0, false)
                .unwrap();
            let mut image_data = pixmap.samples().to_vec();
            image_data.clone_from_slice(pixmap.samples());
            self.img_handle = Some(image::Handle::from_rgba(
                pixmap.width(),
                pixmap.height(),
                image_data,
            ));
        }
    }
}
