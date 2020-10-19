/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#![warn(missing_docs)]
/*!
    Graphics Abstractions.

    This module contains the abstractions and convenience types to allow the runtime
    library to instruct different graphics backends to render the tree of items.

    The entry trait is [GraphicsBackend].

    The run-time library also makes use of [RenderingCache] to store the rendering primitives
    created by the backend in a type-erased manner.
*/
extern crate alloc;
use crate::input::{KeyEvent, KeyboardModifiers, MouseEvent, MouseEventType};
use crate::items::ItemRef;
use crate::properties::{InterpolatedPropertyValue, Property};
#[cfg(feature = "rtti")]
use crate::rtti::{BuiltinItem, FieldInfo, PropertyInfo, ValueType};
use crate::SharedArray;
#[cfg(feature = "rtti")]
use crate::Signal;

use auto_enums::auto_enum;
use cgmath::Matrix4;
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use sixtyfps_corelib_macros::*;
use std::cell::RefCell;
use std::rc::Rc;

/// 2D Rectangle
pub type Rect = euclid::default::Rect<f32>;
/// 2D Point
pub type Point = euclid::default::Point2D<f32>;
/// 2D Size
pub type Size = euclid::default::Size2D<f32>;

/// ARGBColor stores the red, green, blue and alpha components of a color
/// with the precision of the generic parameter T. For example if T is f32,
/// the values are normalized between 0 and 1. If T is u8, they values range
/// is 0 to 255.
/// This is merely a helper class for use with [`Color`].
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct ARGBColor<T> {
    /// The alpha component.
    pub alpha: T,
    /// The red channel.
    pub red: T,
    /// The green channel.
    pub green: T,
    /// The blue channel.
    pub blue: T,
}

impl<T: std::ops::Mul<Output = T> + Copy> ARGBColor<T> {
    /// Consume the color and multiply the alpha to the red, green and blue
    /// components.
    pub fn premultiply_alpha(self) -> Self {
        Self {
            alpha: self.alpha,
            red: self.red * self.alpha,
            green: self.green * self.alpha,
            blue: self.blue * self.alpha,
        }
    }
}

/// Color represents a color in the SixtyFPS run-time, represented using 8-bit channels for
/// red, green, blue and the alpha (opacity).
/// It can be conveniently constructed and destructured using the to_ and from_ (a)rgb helper functions:
/// ```
/// # fn do_something_with_red_and_green(_:f32, _:f32) {}
/// # fn do_something_with_red(_:u8) {}
/// # use sixtyfps_corelib::graphics::{Color, ARGBColor};
/// # let some_color = Color::from_rgb_u8(0, 0, 0);
/// let col = some_color.to_argb_f32();
/// do_something_with_red_and_green(col.red, col.green);
///
/// let ARGBColor { red, blue, green, .. } = some_color.to_argb_u8();
/// do_something_with_red(red);
///
/// let new_col = Color::from(ARGBColor{ red: 0.5, green: 0.65, blue: 0.32, alpha: 1.});
/// ```
#[derive(Copy, Clone, PartialEq, Debug, Default)]
#[repr(C)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl From<ARGBColor<u8>> for Color {
    fn from(col: ARGBColor<u8>) -> Self {
        Self { red: col.red, green: col.green, blue: col.blue, alpha: col.alpha }
    }
}

impl From<Color> for ARGBColor<u8> {
    fn from(col: Color) -> Self {
        ARGBColor { red: col.red, green: col.green, blue: col.blue, alpha: col.alpha }
    }
}

impl From<ARGBColor<u8>> for ARGBColor<f32> {
    fn from(col: ARGBColor<u8>) -> Self {
        Self {
            red: (col.red as f32) / 255.0,
            green: (col.green as f32) / 255.0,
            blue: (col.blue as f32) / 255.0,
            alpha: (col.alpha as f32) / 255.0,
        }
    }
}

impl From<Color> for ARGBColor<f32> {
    fn from(col: Color) -> Self {
        let u8col: ARGBColor<u8> = col.into();
        u8col.into()
    }
}

impl From<ARGBColor<f32>> for Color {
    fn from(col: ARGBColor<f32>) -> Self {
        Self {
            red: (col.red * 255.) as u8,
            green: (col.green * 255.) as u8,
            blue: (col.blue * 255.) as u8,
            alpha: (col.alpha * 255.) as u8,
        }
    }
}

impl Color {
    /// Construct a color from an integer encoded as `0xAARRGGBB`
    pub const fn from_argb_encoded(encoded: u32) -> Color {
        Self {
            red: (encoded >> 16) as u8,
            green: (encoded >> 8) as u8,
            blue: encoded as u8,
            alpha: (encoded >> 24) as u8,
        }
    }

    /// Returns `(alpha, red, green, blue)` encoded as u32
    pub fn as_argb_encoded(&self) -> u32 {
        ((self.red as u32) << 16)
            | ((self.green as u32) << 8)
            | (self.blue as u32)
            | ((self.alpha as u32) << 24)
    }

    /// Construct a color from the alpha, red, green and blue color channel parameters.
    pub fn from_argb_u8(alpha: u8, red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue, alpha }
    }

    /// Construct a color from the red, green and blue color channel parameters. The alpha
    /// channel will have the value 255.
    pub fn from_rgb_u8(red: u8, green: u8, blue: u8) -> Self {
        Self::from_argb_u8(255, red, green, blue)
    }

    /// Construct a color from the alpha, red, green and blue color channel parameters.
    pub fn from_argb_f32(alpha: f32, red: f32, green: f32, blue: f32) -> Self {
        ARGBColor { alpha, red, green, blue }.into()
    }

    /// Construct a color from the red, green and blue color channel parameters. The alpha
    /// channel will have the value 255.
    pub fn from_rgb_f32(red: f32, green: f32, blue: f32) -> Self {
        Self::from_argb_f32(1.0, red, green, blue)
    }

    /// Converts this color to an ARGBColor struct for easy destructuring.
    pub fn to_argb_u8(&self) -> ARGBColor<u8> {
        ARGBColor::from(*self)
    }

    /// Converts this color to an ARGBColor struct for easy destructuring.
    pub fn to_argb_f32(&self) -> ARGBColor<f32> {
        ARGBColor::from(*self)
    }

    /// Returns the red channel of the color as u8 in the range 0..255.
    pub fn red(self) -> u8 {
        self.red
    }

    /// Returns the green channel of the color as u8 in the range 0..255.
    pub fn green(self) -> u8 {
        self.green
    }

    /// Returns the blue channel of the color as u8 in the range 0..255.
    pub fn blue(self) -> u8 {
        self.blue
    }

    /// Returns the alpha channel of the color as u8 in the range 0..255.
    pub fn alpha(self) -> u8 {
        self.alpha
    }
}

impl InterpolatedPropertyValue for Color {
    fn interpolate(self, target_value: Self, t: f32) -> Self {
        Self {
            red: self.red.interpolate(target_value.red, t),
            green: self.green.interpolate(target_value.green, t),
            blue: self.blue.interpolate(target_value.blue, t),
            alpha: self.alpha.interpolate(target_value.alpha, t),
        }
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "argb({}, {}, {}, {})", self.alpha, self.red, self.green, self.blue)
    }
}

/// A resource is a reference to binary data, for example images. They can be accessible on the file
/// system or embedded in the resulting binary. Or they might be URLs to a web server and a downloaded
/// is necessary before they can be used.
#[derive(Clone, PartialEq, Debug)]
#[repr(u8)]
pub enum Resource {
    /// A resource that does not represent any data.
    None,
    /// A resource that points to a file in the file system
    AbsoluteFilePath(crate::SharedString),
    /// A resource that is embedded in the program and accessible via pointer
    /// The format is the same as in a file
    EmbeddedData(super::slice::Slice<'static, u8>),
    /// Raw ARGB
    #[allow(missing_docs)]
    EmbeddedRgbaImage { width: u32, height: u32, data: super::sharedarray::SharedArray<u32> },
}

impl Default for Resource {
    fn default() -> Self {
        Resource::None
    }
}

/// The run-time library uses this enum to instruct the [GraphicsBackend] to render SixtyFPS
/// graphics items.
/// The different variants of this enum closely resemble the properties found in the `.60`
/// mark-up language for various items. More specifically this enum typically holds the
/// properties that usually require for the allocation and uploading of GPU side data, such
/// as vertex buffers or textures. Other properties such as colors not part of the enum but
/// are provided to the back-end using [RenderingVariable]. That means that certain variants
/// of this enum relate to a sequence of rendering variables.
///
/// Always absent here are the starting coordinates for the primitives. Those are provided
/// using a translation in the transform parameter of [Frame::render_primitive].
#[derive(PartialEq, Debug)]
#[repr(C)]
#[allow(missing_docs)]
pub enum HighLevelRenderingPrimitive {
    /// There is nothing to draw.
    ///
    /// Associated rendering variables: None.
    NoContents,
    /// Renders a rectangle with the specified `width` and `height`.
    ///
    /// Expected rendering variables:
    /// * [`RenderingVariable::Color`]: The fill color to use for the rectangle.
    Rectangle { width: f32, height: f32 },
    /// Renders a rectangle with the specified `width` and `height`, as well as a border
    /// around it. The `border_width` specifies the width to use for the border, and the
    /// `border_radius` can be used to render a rounded rectangle.
    ///
    /// Expected rendering variables:
    /// * [`RenderingVariable::Color`]: The color to fill the rectangle with.
    /// * [`RenderingVariable::Color`]: The color to use for stroking the border of the rectangle.
    BorderRectangle { width: f32, height: f32, border_width: f32, border_radius: f32 },
    /// Renders a image referenced by the specified `source`.
    ///
    /// Optional rendering variables:
    /// * [`RenderingVariable::ScaledWidth`]: The image will be scaled to the specified width.
    /// * [`RenderingVariable::ScaledHeight`]: The image will be scaled to the specified height.
    Image { source: crate::Resource },
    /// Renders the specified `text` with a font that matches the specified family (`font_family`) and the given
    /// pixel size (`font_size`).
    ///
    /// Expected rendering variables:
    /// * [`RenderingVariable::Color`]: The color to use for rendering the glyphs.
    /// * [`RenderingVariable::TextCursor`]: Draw a text cursor.
    Text { text: crate::SharedString, font_family: crate::SharedString, font_size: f32 },
    /// Renders a path specified by the `elements` parameter. The path will be scaled to fit into the given
    /// `width` and `height`. If the `stroke_width` is greater than zero, then path will also be outlined.
    ///
    /// Expected rendering variables:
    /// * [`RenderingVariable::Color`]: The color to use for filling the path.
    /// * [`RenderingVariable::Color`]: The color to use for the path outline, if a non-zero `stroke_width`
    ///   was specified.
    Path { width: f32, height: f32, elements: crate::PathData, stroke_width: f32 },
    /// Applies a clip rectangle for all subsequent rendering, with the given `width` and `height. When rendering
    /// the low-level rendering primitive created from this variant, [`Frame::render_primitive`] will return a
    /// vector with cleanup primitives that must be applied in order to unapply the clipping.
    ClipRect { width: f32, height: f32 },
}

impl Default for HighLevelRenderingPrimitive {
    fn default() -> Self {
        Self::NoContents
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
/// This enum is used to affect various aspects of the rendering of [GraphicsBackend::LowLevelRenderingPrimitive]
/// without the need to re-create them. See the documentation of [HighLevelRenderingPrimitive]
/// about which variables are supported in which order.
pub enum RenderingVariable {
    /// Translates the primitive by the given (x, y) vector.
    Translate(f32, f32),
    /// Apply the specified color. Depending on the order in the rendering variables array this may apply to different
    /// aspects of the primitive, such as the fill or stroke.
    Color(Color),
    /// Scale the primitive by the specified width.
    ScaledWidth(f32),
    /// Scale the primitive by the specified height.
    ScaledHeight(f32),
    /// Draw a text cursor. The parameters provide the x coordiante and the width/height as (x, width, height) tuple.
    TextCursor(f32, f32, f32),
    /// Draw a text selection. The parameters provide the starting x coordinate, the width and the height. This variable
    /// must be followed by two colors, foreground and background.
    TextSelection(f32, f32, f32),
}

impl RenderingVariable {
    /// Returns the color of this variable, or panics if the enum holds a different variant.
    pub fn as_color(&self) -> &Color {
        match self {
            RenderingVariable::Color(c) => c,
            _ => panic!("internal error: expected color but found something else"),
        }
    }
    /// Returns the scaled width of this variable, or panics if the enum holds a different variant.
    pub fn as_scaled_width(&self) -> f32 {
        match self {
            RenderingVariable::ScaledWidth(w) => *w,
            _ => panic!("internal error: expected scaled width but found something else"),
        }
    }
    /// Returns the scaled height of this variable, or panics if the enum holds a different variant.
    pub fn as_scaled_height(&self) -> f32 {
        match self {
            RenderingVariable::ScaledHeight(h) => *h,
            _ => panic!("internal error: expected scaled height but found something else"),
        }
    }
}

/// Frame is used to render previously created [GraphicsBackend::LowLevelRenderingPrimitive] instances
/// to the back-buffer of the window.
pub trait Frame {
    /// This associated type is usually provided through the [GraphicsBackend::LowLevelRenderingPrimitive] type.
    type LowLevelRenderingPrimitive;
    /// Renderings the provided primitive to the back-buffer, taking the provided transform and additional rendering
    /// variables into account.
    ///
    /// The returned primitives must be rendered after rendering any rendering primitives that are supposed to be
    /// in a visual tree after this primitive. This is for example used to clean up clipping regions.
    ///
    /// Arguments:
    /// * `primitive`: The primitive to render.
    /// * `transform`: The geometry of the primitive will be transformed by this 4x4 matrix. This can be used to apply
    ///                rotation, scaling, etc. without re-creating the low-level rendering primitive.
    /// * `variables`: An array of [RenderingVariable] instances that are applied when rendering the primitive. These
    ///                variables typically translate to OpenGL uniforms and allow for affecting various aspects of the
    ///                rendering of the primitive without expensive buffer uploads to the GPU.
    fn render_primitive(
        &mut self,
        primitive: &Self::LowLevelRenderingPrimitive,
        transform: &Matrix4<f32>,
        variables: SharedArray<RenderingVariable>,
    ) -> Vec<Self::LowLevelRenderingPrimitive>;
}

/// RenderingPrimitivesBuilder is used to convert instances of [HighLevelRenderingPrimitive] to
/// the back-end specific [GraphicsBackend::LowLevelRenderingPrimitive], giving the backend a way
/// to determin the optimal representation for rendering later. For example this may involve uploading
/// textures for images to GPU memory, pre-rendering glyphs or allocating vertex buffers.
pub trait RenderingPrimitivesBuilder {
    /// This associated type is usually provided through the [GraphicsBackend::LowLevelRenderingPrimitive] type.
    type LowLevelRenderingPrimitive;

    /// Lowers the high-level rendering primitive to a representation suitable for the graphics backend.
    ///
    /// Arguments:
    /// * `primitive`: The primitive to convert.
    fn create(
        &mut self,
        primitive: HighLevelRenderingPrimitive,
    ) -> Self::LowLevelRenderingPrimitive;
}

/// GraphicsBackend is the trait that the the SixtyFPS run-time uses to convert [HighLevelRenderingPrimitive]
/// to an internal representation that is optimal for the backend, in order to render it later. The internal
/// representation is opaque but must be provided via the [GraphicsBackend::LowLevelRenderingPrimitive] associated type.
///
/// The backend operates in two modes:
///   1. It can be used to create new rendering primitives, by calling [GraphicsBackend::new_rendering_primitives_builder]. This is
///      usually an expensive step, that involves uploading data to the GPU or performing other pre-calculations.
///
///   1. A series of low-level rendering primitives can be rendered into a frame, that's started using [GraphicsBackend::new_frame].
///      The low-level rendering primitives are intended to be fast and ready for rendering.
pub trait GraphicsBackend: Sized {
    /// This associated type is typically opaque and is produced by the [RenderingPrimitivesBuilder]. For example it may contain
    /// handles that refer to data that was uploaded to the GPU.
    type LowLevelRenderingPrimitive;
    /// This associated type ties the Frame trait together with this trait's LowLevelRenderingPrimitive.
    type Frame: Frame<LowLevelRenderingPrimitive = Self::LowLevelRenderingPrimitive>;
    /// This associated type ties the RenderingPrimitivesBuilder trait with this trait's LowLevelRenderingPrimitive.
    type RenderingPrimitivesBuilder: RenderingPrimitivesBuilder<
        LowLevelRenderingPrimitive = Self::LowLevelRenderingPrimitive,
    >;

    /// Creates a new RenderingPrimitivesBuilder for the allocation of any GPU side data of different primitives. Call
    /// [GraphicsBackend::finish_primitives] when done.
    fn new_rendering_primitives_builder(&mut self) -> Self::RenderingPrimitivesBuilder;
    /// When all low-level rendering primitives have been created needed to render your scene, then this method
    /// needs to be called to complete the process.
    ///
    /// Arguments:
    /// * `builder`: The [RenderingPrimitivesBuilder] created by calling [GraphicsBackend::new_rendering_primitives_builder].
    fn finish_primitives(&mut self, builder: Self::RenderingPrimitivesBuilder);

    /// Begins the process of rendering a new frame into what is typically the window back-buffer. Call [GraphicsBackend::present_frame]
    /// when all rendering primitives have been queued for rendering.
    ///
    /// Arguments:
    /// * `width`: The width of the window to render.
    /// * `height`: The height of the window to render.
    /// * `clear_color`: The color to clear the back-buffer with.
    fn new_frame(&mut self, width: u32, height: u32, clear_color: &Color) -> Self::Frame;
    /// When all rendering primitives have been queued for rendering with the [Frame] API, pass the frame instance to this function
    /// and thereby complete the rendering. The backend then will present the contents on the screen inside the window, for example by
    /// flushing the backing store or swapping OpenGL buffers.
    ///
    /// Arguments:
    /// * `frame`: The frame created by calling [GraphicsBackend::new_frame].
    fn present_frame(&mut self, frame: Self::Frame);

    /// Returns the window that the backend is associated with.
    fn window(&self) -> &winit::window::Window;
}

/// Holds a GraphicBackend's rendering primitive as well as a PropertyTracker that allows lazily re-creating
/// the primitive if the properties needed to create it have changed.
pub struct TrackingRenderingPrimitive<Backend: GraphicsBackend> {
    /// The rendering primitive that's being tracked.
    pub primitive: Backend::LowLevelRenderingPrimitive,
    /// The property tracker that should be used to evaluate whether the primitive needs to be re-created
    /// or not.
    pub dependency_tracker: core::pin::Pin<Box<crate::properties::PropertyTracker>>,
}

impl<Backend: GraphicsBackend> TrackingRenderingPrimitive<Backend> {
    /// Creates a new TrackingRenderingPrimitive by evaluating the provided update_fn once, storing the returned
    /// rendering primitive and initializing the dependency tracker.
    pub fn new(update_fn: impl FnOnce() -> Backend::LowLevelRenderingPrimitive) -> Self {
        let dependency_tracker = Box::pin(crate::properties::PropertyTracker::default());
        let primitive = dependency_tracker.as_ref().evaluate(update_fn);
        Self { primitive, dependency_tracker }
    }
}

/// The RenderingCache is used by the run-time library to avoid storing the
/// typed [GraphicsBackend::LowLevelRenderingPrimitive] instances created for
/// [Items][`crate::items`]. Instead it allows mapping them to a usize
/// handle, and it also allows tracking whenever any of the properties used to
/// create the primitive changed.
pub type RenderingCache<Backend> = vec_arena::Arena<TrackingRenderingPrimitive<Backend>>;

type WindowFactoryFn<Backend> =
    dyn Fn(&crate::eventloop::EventLoop, winit::window::WindowBuilder) -> Backend;

struct MappedWindow<Backend: GraphicsBackend + 'static> {
    backend: RefCell<Backend>,
    rendering_cache: RefCell<RenderingCache<Backend>>,
}

enum GraphicsWindowBackendState<Backend: GraphicsBackend + 'static> {
    Unmapped,
    Mapped(MappedWindow<Backend>),
}

impl<Backend: GraphicsBackend + 'static> GraphicsWindowBackendState<Backend> {
    fn as_mapped(&self) -> &MappedWindow<Backend> {
        match self {
            GraphicsWindowBackendState::Unmapped => panic!(
                "internal error: tried to access window functions that require a mapped window"
            ),
            GraphicsWindowBackendState::Mapped(mw) => &mw,
        }
    }
}

#[derive(FieldOffsets)]
#[repr(C)]
#[pin]
struct WindowProperties {
    scale_factor: Property<f32>,
    width: Property<f32>,
    height: Property<f32>,
}

impl Default for WindowProperties {
    fn default() -> Self {
        Self {
            scale_factor: Property::new(1.0),
            width: Property::new(800.),
            height: Property::new(600.),
        }
    }
}

/// GraphicsWindow is an implementation of the [GenericWindow][`crate::eventloop::GenericWindow`] trait. This is
/// typically instantiated by entry factory functions of the different graphics backends.
pub struct GraphicsWindow<Backend: GraphicsBackend + 'static> {
    window_factory: Box<WindowFactoryFn<Backend>>,
    map_state: RefCell<GraphicsWindowBackendState<Backend>>,
    properties: Pin<Box<WindowProperties>>,
    cursor_blinker: std::cell::RefCell<pin_weak::rc::PinWeak<TextCursorBlinker>>,
    keyboard_modifiers: std::cell::Cell<KeyboardModifiers>,
}

impl<Backend: GraphicsBackend + 'static> GraphicsWindow<Backend> {
    /// Creates a new reference-counted instance.
    ///
    /// Arguments:
    /// * `graphics_backend_factory`: The factor function stored in the GraphicsWindow that's called when the state
    ///   of the window changes to mapped. The event loop and window builder parameters can be used to create a
    ///   backing window.
    pub fn new(
        graphics_backend_factory: impl Fn(&crate::eventloop::EventLoop, winit::window::WindowBuilder) -> Backend
            + 'static,
    ) -> Rc<Self> {
        Rc::new(Self {
            window_factory: Box::new(graphics_backend_factory),
            map_state: RefCell::new(GraphicsWindowBackendState::Unmapped),
            properties: Box::pin(WindowProperties::default()),
            cursor_blinker: Default::default(),
            keyboard_modifiers: Default::default(),
        })
    }

    /// Returns the window id of the window if it is mapped, None otherwise.
    pub fn id(&self) -> Option<winit::window::WindowId> {
        Some(self.map_state.borrow().as_mapped().backend.borrow().window().id())
    }
}

impl<Backend: GraphicsBackend> Drop for GraphicsWindow<Backend> {
    fn drop(&mut self) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(mw) => {
                crate::eventloop::unregister_window(mw.backend.borrow().window().id());
            }
        }
        if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
            existing_blinker.stop();
        }
    }
}

impl<Backend: GraphicsBackend> crate::eventloop::GenericWindow for GraphicsWindow<Backend> {
    fn draw(self: Rc<Self>, component: crate::component::ComponentRefPin) {
        {
            let map_state = self.map_state.borrow();
            let window = map_state.as_mapped();
            let mut backend = window.backend.borrow_mut();
            let mut rendering_primitives_builder = backend.new_rendering_primitives_builder();

            // Generate cached rendering data once
            crate::item_tree::visit_items(
                component,
                crate::item_tree::TraversalOrder::BackToFront,
                |_, item, _| {
                    crate::item_rendering::update_item_rendering_data(
                        item,
                        &window.rendering_cache,
                        &mut rendering_primitives_builder,
                        &self,
                    );
                    crate::item_tree::ItemVisitorResult::Continue(())
                },
                (),
            );

            backend.finish_primitives(rendering_primitives_builder);
        }

        let map_state = self.map_state.borrow();
        let window = map_state.as_mapped();
        let mut backend = window.backend.borrow_mut();
        let size = backend.window().inner_size();
        let mut frame = backend.new_frame(
            size.width,
            size.height,
            &ARGBColor { red: 255 as u8, green: 255, blue: 255, alpha: 255 }.into(),
        );
        crate::item_rendering::render_component_items(
            component,
            &mut frame,
            &window.rendering_cache,
            &self,
        );
        backend.present_frame(frame);
    }

    fn process_mouse_input(
        self: Rc<Self>,
        pos: winit::dpi::PhysicalPosition<f64>,
        what: MouseEventType,
        component: crate::component::ComponentRefPin,
    ) {
        component.as_ref().input_event(
            MouseEvent { pos: euclid::point2(pos.x as _, pos.y as _), what },
            &crate::eventloop::ComponentWindow::new(self.clone()),
            &component,
        );
    }

    fn process_key_input(
        self: Rc<Self>,
        event: &KeyEvent,
        component: core::pin::Pin<crate::component::ComponentRef>,
    ) {
        component.as_ref().key_event(event, &crate::eventloop::ComponentWindow::new(self.clone()));
    }

    fn with_platform_window(&self, callback: &dyn Fn(&winit::window::Window)) {
        let map_state = self.map_state.borrow();
        let window = map_state.as_mapped();
        let backend = window.backend.borrow();
        let handle = backend.window();
        callback(handle);
    }

    fn map_window(
        self: Rc<Self>,
        event_loop: &crate::eventloop::EventLoop,
        root_item: Pin<ItemRef>,
    ) {
        if matches!(&*self.map_state.borrow(), GraphicsWindowBackendState::Mapped(..)) {
            return;
        }

        let id = {
            let window_builder = winit::window::WindowBuilder::new();

            let backend = self.window_factory.as_ref()(&event_loop, window_builder);

            let platform_window = backend.window();

            if std::env::var("SIXTYFPS_FULLSCREEN").is_ok() {
                platform_window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            }

            let window_id = platform_window.id();

            // Ideally we should be passing the initial requested size to the window builder, but those properties
            // may be specified in logical pixels, relative to the scale factory, which we only know *after* mapping
            // the window to the screen. So we first map the window then, propagate the scale factory and *then* the
            // width/height properties should have the correct values calculated via their bindings that multiply with
            // the scale factor.
            // We could pass the logical requested size at window builder time, *if* we knew what the values are.
            {
                self.properties.as_ref().scale_factor.set(platform_window.scale_factor() as _);
                let existing_size = platform_window.inner_size();

                let mut new_size = existing_size;

                if let Some(window_item) = ItemRef::downcast_pin(root_item) {
                    let width =
                        crate::items::Window::FIELD_OFFSETS.width.apply_pin(window_item).get();
                    if width > 0. {
                        new_size.width = width as _;
                    }
                    let height =
                        crate::items::Window::FIELD_OFFSETS.height.apply_pin(window_item).get();
                    if height > 0. {
                        new_size.height = height as _;
                    }

                    {
                        let window = self.clone();
                        window_item.as_ref().width.set_binding(move || {
                            WindowProperties::FIELD_OFFSETS
                                .width
                                .apply_pin(window.properties.as_ref())
                                .get()
                        });
                    }
                    {
                        let window = self.clone();
                        window_item.as_ref().height.set_binding(move || {
                            WindowProperties::FIELD_OFFSETS
                                .height
                                .apply_pin(window.properties.as_ref())
                                .get()
                        });
                    }
                }

                if new_size != existing_size {
                    platform_window.set_inner_size(new_size)
                }

                self.properties.as_ref().width.set(new_size.width as _);
                self.properties.as_ref().height.set(new_size.height as _);
            }

            self.map_state.replace(GraphicsWindowBackendState::Mapped(MappedWindow {
                backend: RefCell::new(backend),
                rendering_cache: Default::default(),
            }));

            window_id
        };

        crate::eventloop::register_window(
            id,
            self.clone() as Rc<dyn crate::eventloop::GenericWindow>,
        );
    }

    fn request_redraw(&self) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                window.backend.borrow().window().request_redraw()
            }
        }
    }

    fn unmap_window(self: Rc<Self>) {
        self.map_state.replace(GraphicsWindowBackendState::Unmapped);
        if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
            existing_blinker.stop();
        }
    }

    fn scale_factor(&self) -> f32 {
        WindowProperties::FIELD_OFFSETS.scale_factor.apply_pin(self.properties.as_ref()).get()
    }

    fn set_scale_factor(&self, factor: f32) {
        self.properties.as_ref().scale_factor.set(factor);
    }

    fn set_width(&self, width: f32) {
        self.properties.as_ref().width.set(width);
    }

    fn set_height(&self, height: f32) {
        self.properties.as_ref().height.set(height);
    }

    fn free_graphics_resources(
        self: Rc<Self>,
        component: core::pin::Pin<crate::component::ComponentRef>,
    ) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                crate::item_rendering::free_item_rendering_data(component, &window.rendering_cache)
            }
        }
    }

    fn set_cursor_blink_binding(&self, prop: &crate::properties::Property<bool>) {
        let existing_blinker = self.cursor_blinker.borrow().clone();

        let blinker = existing_blinker.upgrade().unwrap_or_else(|| {
            let new_blinker = TextCursorBlinker::new();
            *self.cursor_blinker.borrow_mut() =
                pin_weak::rc::PinWeak::downgrade(new_blinker.clone());
            new_blinker
        });

        TextCursorBlinker::set_binding(blinker, prop);
    }

    /// Returns the currently active keyboard notifiers.
    fn current_keyboard_modifiers(&self) -> KeyboardModifiers {
        self.keyboard_modifiers.get()
    }
    /// Sets the currently active keyboard notifiers. This is used only for testing or directly
    /// from the event loop implementation.
    fn set_current_keyboard_modifiers(&self, state: KeyboardModifiers) {
        self.keyboard_modifiers.set(state)
    }

    fn set_focus_item(
        self: Rc<Self>,
        component: core::pin::Pin<crate::component::ComponentRef>,
        item_ptr: *const u8,
    ) {
        let window = crate::eventloop::ComponentWindow::new(self.clone());
        component.as_ref().focus_event(&crate::input::FocusEvent::FocusOut, &window);
        component.as_ref().focus_event(&crate::input::FocusEvent::FocusIn(item_ptr), &window);
    }

    fn set_focus(
        self: Rc<Self>,
        component: core::pin::Pin<crate::component::ComponentRef>,
        have_focus: bool,
    ) {
        let window = crate::eventloop::ComponentWindow::new(self.clone());
        let event = if have_focus {
            crate::input::FocusEvent::WindowReceivedFocus
        } else {
            crate::input::FocusEvent::WindowLostFocus
        };
        component.as_ref().focus_event(&event, &window);
    }
}

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem, Clone, Debug, PartialEq)]
#[pin]
/// PathLineTo describes the event of moving the cursor on the path to the specified location
/// along a straight line.
pub struct PathLineTo {
    #[rtti_field]
    /// The x coordinate where the line should go to.
    pub x: f32,
    #[rtti_field]
    /// The y coordinate where the line should go to.
    pub y: f32,
}

#[repr(C)]
#[derive(FieldOffsets, Default, BuiltinItem, Clone, Debug, PartialEq)]
#[pin]
/// PathArcTo describes the event of moving the cursor on the path across an arc to the specified
/// x/y coordinates, with the specified x/y radius and additional properties.
pub struct PathArcTo {
    #[rtti_field]
    /// The x coordinate where the arc should end up.
    pub x: f32,
    #[rtti_field]
    /// The y coordinate where the arc should end up.
    pub y: f32,
    #[rtti_field]
    /// The radius on the x-axis of the arc.
    pub radius_x: f32,
    #[rtti_field]
    /// The radius on the y-axis of the arc.
    pub radius_y: f32,
    #[rtti_field]
    /// The rotation along the x-axis of the arc in degress.
    pub x_rotation: f32,
    #[rtti_field]
    /// large_arc indicates whether to take the long or the shorter path to complete the arc.
    pub large_arc: bool,
    #[rtti_field]
    /// sweep indicates the direction of the arc. If true, a clockwise direction is chosen,
    /// otherwise counter-clockwise.
    pub sweep: bool,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
/// PathElement describes a single element on a path, such as move-to, line-to, etc.
pub enum PathElement {
    /// The LineTo variant describes a line.
    LineTo(PathLineTo),
    /// The PathArcTo variant describes an arc.
    ArcTo(PathArcTo),
    /// Indicates that the path should be closed now by connecting to the starting point.
    Close,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
/// PathEvent is a low-level data structure describing the composition of a path. Typically it is
/// generated at compile time from a higher-level description, such as SVG commands.
pub enum PathEvent {
    /// The beginning of the path.
    Begin,
    /// A straight line on the path.
    Line,
    /// A quadratic bezier curve on the path.
    Quadratic,
    /// A cubic bezier curve on the path.
    Cubic,
    /// The end of the path that remains open.
    EndOpen,
    /// The end of a path that is closed.
    EndClosed,
}

struct ToLyonPathEventIterator<'a> {
    events_it: std::slice::Iter<'a, PathEvent>,
    coordinates_it: std::slice::Iter<'a, Point>,
    first: Option<&'a Point>,
    last: Option<&'a Point>,
}

impl<'a> Iterator for ToLyonPathEventIterator<'a> {
    type Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>;
    fn next(&mut self) -> Option<Self::Item> {
        use lyon::path::Event;

        self.events_it.next().map(|event| match event {
            PathEvent::Begin => Event::Begin { at: self.coordinates_it.next().unwrap().clone() },
            PathEvent::Line => Event::Line {
                from: self.coordinates_it.next().unwrap().clone(),
                to: self.coordinates_it.next().unwrap().clone(),
            },
            PathEvent::Quadratic => Event::Quadratic {
                from: self.coordinates_it.next().unwrap().clone(),
                ctrl: self.coordinates_it.next().unwrap().clone(),
                to: self.coordinates_it.next().unwrap().clone(),
            },
            PathEvent::Cubic => Event::Cubic {
                from: self.coordinates_it.next().unwrap().clone(),
                ctrl1: self.coordinates_it.next().unwrap().clone(),
                ctrl2: self.coordinates_it.next().unwrap().clone(),
                to: self.coordinates_it.next().unwrap().clone(),
            },
            PathEvent::EndOpen => Event::End {
                first: self.first.unwrap().clone(),
                last: self.last.unwrap().clone(),
                close: false,
            },
            PathEvent::EndClosed => Event::End {
                first: self.first.unwrap().clone(),
                last: self.last.unwrap().clone(),
                close: true,
            },
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.events_it.size_hint()
    }
}

impl<'a> ExactSizeIterator for ToLyonPathEventIterator<'a> {}

struct TransformedLyonPathIterator<EventIt> {
    it: EventIt,
    transform: lyon::math::Transform,
}

impl<EventIt: Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>>> Iterator
    for TransformedLyonPathIterator<EventIt>
{
    type Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>;
    fn next(&mut self) -> Option<Self::Item> {
        self.it.next().map(|ev| ev.transformed(&self.transform))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}

impl<EventIt: Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>>>
    ExactSizeIterator for TransformedLyonPathIterator<EventIt>
{
}

/// PathDataIterator is a data structure that acts as starting point for iterating
/// through the low-level events of a path. If the path was constructed from said
/// events, then it is a very thin abstraction. If the path was created from higher-level
/// elements, then an intermediate lyon path is required/built.
pub struct PathDataIterator<'a> {
    it: LyonPathIteratorVariant<'a>,
    transform: Option<lyon::math::Transform>,
}

enum LyonPathIteratorVariant<'a> {
    FromPath(lyon::path::Path),
    FromEvents(&'a crate::SharedArray<PathEvent>, &'a crate::SharedArray<Point>),
}

impl<'a> PathDataIterator<'a> {
    /// Create a new iterator for path traversal.
    #[auto_enum(Iterator)]
    pub fn iter(
        &'a self,
    ) -> impl Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>> + 'a {
        match &self.it {
            LyonPathIteratorVariant::FromPath(path) => self.apply_transform(path.iter()),
            LyonPathIteratorVariant::FromEvents(events, coordinates) => {
                self.apply_transform(ToLyonPathEventIterator {
                    events_it: events.iter(),
                    coordinates_it: coordinates.iter(),
                    first: coordinates.first(),
                    last: coordinates.last(),
                })
            }
        }
    }

    fn fit(&mut self, width: f32, height: f32) {
        if width > 0. || height > 0. {
            let br = lyon::algorithms::aabb::bounding_rect(self.iter());
            self.transform = Some(lyon::algorithms::fit::fit_rectangle(
                &br,
                &Rect::from_size(Size::new(width, height)),
                lyon::algorithms::fit::FitStyle::Min,
            ));
        }
    }
    #[auto_enum(Iterator)]
    fn apply_transform(
        &'a self,
        event_it: impl Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>> + 'a,
    ) -> impl Iterator<Item = lyon::path::Event<lyon::math::Point, lyon::math::Point>> + 'a {
        match self.transform {
            Some(transform) => TransformedLyonPathIterator { it: event_it, transform },
            None => event_it,
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
/// PathData represents a path described by either high-level elements or low-level
/// events and coordinates.
pub enum PathData {
    /// None is the variant when the path is empty.
    None,
    /// The Elements variant is used to make a Path from shared arrays of elements.
    Elements(crate::SharedArray<PathElement>),
    /// The Events variant describes the path as a series of low-level events and
    /// associated coordinates.
    Events(crate::SharedArray<PathEvent>, crate::SharedArray<Point>),
}

impl Default for PathData {
    fn default() -> Self {
        Self::None
    }
}

impl PathData {
    /// This function returns an iterator that allows traversing the path by means of lyon events.
    pub fn iter(&self) -> PathDataIterator {
        PathDataIterator {
            it: match self {
                PathData::None => LyonPathIteratorVariant::FromPath(lyon::path::Path::new()),
                PathData::Elements(elements) => LyonPathIteratorVariant::FromPath(
                    PathData::build_path(elements.as_slice().iter()),
                ),
                PathData::Events(events, coordinates) => {
                    LyonPathIteratorVariant::FromEvents(events, coordinates)
                }
            },
            transform: None,
        }
    }

    /// This function returns an iterator that allows traversing the path by means of lyon events.
    pub fn iter_fitted(&self, width: f32, height: f32) -> PathDataIterator {
        let mut it = self.iter();
        it.fit(width, height);
        it
    }

    fn build_path(element_it: std::slice::Iter<PathElement>) -> lyon::path::Path {
        use lyon::geom::SvgArc;
        use lyon::math::{Angle, Point, Vector};
        use lyon::path::{
            builder::{Build, FlatPathBuilder, SvgBuilder},
            ArcFlags,
        };

        let mut path_builder = lyon::path::Path::builder().with_svg();
        for element in element_it {
            match element {
                PathElement::LineTo(PathLineTo { x, y }) => {
                    path_builder.line_to(Point::new(*x, *y))
                }
                PathElement::ArcTo(PathArcTo {
                    x,
                    y,
                    radius_x,
                    radius_y,
                    x_rotation,
                    large_arc,
                    sweep,
                }) => {
                    let radii = Vector::new(*radius_x, *radius_y);
                    let x_rotation = Angle::degrees(*x_rotation);
                    let flags = ArcFlags { large_arc: *large_arc, sweep: *sweep };
                    let to = Point::new(*x, *y);

                    let svg_arc = SvgArc {
                        from: path_builder.current_position(),
                        radii,
                        x_rotation,
                        flags,
                        to,
                    };

                    if svg_arc.is_straight_line() {
                        path_builder.line_to(to);
                    } else {
                        path_builder.arc_to(radii, x_rotation, flags, to)
                    }
                }
                PathElement::Close => path_builder.close(),
            }
        }

        path_builder.build()
    }
}

pub(crate) mod ffi {
    #![allow(unsafe_code)]

    use super::*;

    #[allow(non_camel_case_types)]
    type c_void = ();

    /// Expand Rect so that cbindgen can see it. ( is in fact euclid::default::Rect<f32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    }

    /// Expand Point so that cbindgen can see it. ( is in fact euclid::default::PointD2<f32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct Point {
        x: f32,
        y: f32,
    }

    #[no_mangle]
    /// This function is used for the low-level C++ interface to allocate the backing vector for a shared path element array.
    pub unsafe extern "C" fn sixtyfps_new_path_elements(
        out: *mut c_void,
        first_element: *const PathElement,
        count: usize,
    ) {
        let arr = crate::SharedArray::from(std::slice::from_raw_parts(first_element, count));
        core::ptr::write(out as *mut crate::SharedArray<PathElement>, arr.clone());
    }

    #[no_mangle]
    /// This function is used for the low-level C++ interface to allocate the backing vector for a shared path event array.
    pub unsafe extern "C" fn sixtyfps_new_path_events(
        out_events: *mut c_void,
        out_coordinates: *mut c_void,
        first_event: *const PathEvent,
        event_count: usize,
        first_coordinate: *const Point,
        coordinate_count: usize,
    ) {
        let events = crate::SharedArray::from(std::slice::from_raw_parts(first_event, event_count));
        core::ptr::write(out_events as *mut crate::SharedArray<PathEvent>, events.clone());
        let coordinates = crate::SharedArray::from(std::slice::from_raw_parts(
            first_coordinate,
            coordinate_count,
        ));
        core::ptr::write(out_coordinates as *mut crate::SharedArray<Point>, coordinates.clone());
    }
}

/// The TextCursorBlinker takes care of providing a toggled boolean property
/// that can be used to animate a blinking cursor. It's typically stored in the
/// Window using a Weak and set_binding() can be used to set up a binding on a given
/// property that'll keep it up-to-date. That binding keeps a strong reference to the
/// blinker. If the underlying item that uses it goes away, the binding goes away and
/// so does the blinker.
#[derive(FieldOffsets)]
#[repr(C)]
#[pin]
struct TextCursorBlinker {
    cursor_visible: Property<bool>,
    cursor_blink_timer: crate::timers::Timer,
}

impl TextCursorBlinker {
    fn new() -> Pin<Rc<Self>> {
        Rc::pin(Self {
            cursor_visible: Property::new(true),
            cursor_blink_timer: Default::default(),
        })
    }

    fn set_binding(instance: Pin<Rc<TextCursorBlinker>>, prop: &crate::properties::Property<bool>) {
        instance.as_ref().cursor_visible.set(true);
        // Re-start timer, in case.
        Self::start(&instance);
        prop.set_binding(move || {
            TextCursorBlinker::FIELD_OFFSETS.cursor_visible.apply_pin(instance.as_ref()).get()
        });
    }

    fn start(self: &Pin<Rc<Self>>) {
        if self.cursor_blink_timer.running() {
            self.cursor_blink_timer.restart();
        } else {
            let toggle_cursor = {
                let weak_blinker = pin_weak::rc::PinWeak::downgrade(self.clone());
                move || {
                    if let Some(blinker) = weak_blinker.upgrade() {
                        let visible = TextCursorBlinker::FIELD_OFFSETS
                            .cursor_visible
                            .apply_pin(blinker.as_ref())
                            .get();
                        blinker.cursor_visible.set(!visible);
                    }
                }
            };
            self.cursor_blink_timer.start(
                crate::timers::TimerMode::Repeated,
                std::time::Duration::from_millis(500),
                Box::new(toggle_cursor),
            );
        }
    }

    fn stop(&self) {
        self.cursor_blink_timer.stop()
    }
}
