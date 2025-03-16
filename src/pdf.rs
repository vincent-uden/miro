use iced::{
    Color, ContentFit, Element, Length, Point, Rectangle, Rotation, Size, Vector,
    advanced::{
        Layout, Widget,
        image::{self, FilterMethod},
        layout, mouse, renderer,
        widget::Tree,
    },
};

#[derive(Debug)]
pub struct PdfViewer<Handle = image::Handle> {
    handle: Handle,
    width: Length,
    height: Length,
    content_fit: ContentFit,
    filter_method: FilterMethod,
    rotation: Rotation,
    opacity: f32,
    translation: Vector,
}

impl<Handle> PdfViewer<Handle> {
    /// Creates a new [`Image`] with the given path.
    pub fn new(handle: impl Into<Handle>) -> Self {
        PdfViewer {
            handle: handle.into(),
            width: Length::Shrink,
            height: Length::Shrink,
            content_fit: ContentFit::None,
            filter_method: FilterMethod::default(),
            rotation: Rotation::default(),
            opacity: 1.0,
            translation: Vector::ZERO,
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
        (bounds.width - image_size.width) / 2.0,
        (bounds.height - image_size.height) / 2.0,
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

impl<Message, Theme, Renderer, Handle> Widget<Message, Theme, Renderer> for PdfViewer<Handle>
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
}

impl<'a, Message, Theme, Renderer, Handle> From<PdfViewer<Handle>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: image::Renderer<Handle = Handle>,
    Handle: Clone + 'a,
{
    fn from(image: PdfViewer<Handle>) -> Element<'a, Message, Theme, Renderer> {
        Element::new(image)
    }
}
