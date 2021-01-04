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
use crate::items::{ItemRc, ItemRef, ItemWeak};
use crate::properties::{InterpolatedPropertyValue, Property, PropertyTracker};
#[cfg(feature = "rtti")]
use crate::rtti::{BuiltinItem, FieldInfo, PropertyInfo, ValueType};
#[cfg(feature = "rtti")]
use crate::Callback;
use crate::{
    component::{ComponentRc, ComponentWeak},
    slice::Slice,
};

use auto_enums::auto_enum;
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use sixtyfps_corelib_macros::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// 2D Rectangle
pub type Rect = euclid::default::Rect<f32>;
/// 2D Rectangle with integer coordinates
pub type IntRect = euclid::default::Rect<i32>;
/// 2D Point
pub type Point = euclid::default::Point2D<f32>;
/// 2D Size
pub type Size = euclid::default::Size2D<f32>;

/// RgbaColor stores the red, green, blue and alpha components of a color
/// with the precision of the generic parameter T. For example if T is f32,
/// the values are normalized between 0 and 1. If T is u8, they values range
/// is 0 to 255.
/// This is merely a helper class for use with [`Color`].
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct RgbaColor<T> {
    /// The alpha component.
    pub alpha: T,
    /// The red channel.
    pub red: T,
    /// The green channel.
    pub green: T,
    /// The blue channel.
    pub blue: T,
}

/// Color represents a color in the SixtyFPS run-time, represented using 8-bit channels for
/// red, green, blue and the alpha (opacity).
/// It can be conveniently constructed and destructured using the to_ and from_ (a)rgb helper functions:
/// ```
/// # fn do_something_with_red_and_green(_:f32, _:f32) {}
/// # fn do_something_with_red(_:u8) {}
/// # use sixtyfps_corelib::graphics::{Color, RgbaColor};
/// # let some_color = Color::from_rgb_u8(0, 0, 0);
/// let col = some_color.to_argb_f32();
/// do_something_with_red_and_green(col.red, col.green);
///
/// let RgbaColor { red, blue, green, .. } = some_color.to_argb_u8();
/// do_something_with_red(red);
///
/// let new_col = Color::from(RgbaColor{ red: 0.5, green: 0.65, blue: 0.32, alpha: 1.});
/// ```
#[derive(Copy, Clone, PartialEq, Debug, Default)]
#[repr(C)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl From<RgbaColor<u8>> for Color {
    fn from(col: RgbaColor<u8>) -> Self {
        Self { red: col.red, green: col.green, blue: col.blue, alpha: col.alpha }
    }
}

impl From<Color> for RgbaColor<u8> {
    fn from(col: Color) -> Self {
        RgbaColor { red: col.red, green: col.green, blue: col.blue, alpha: col.alpha }
    }
}

impl From<RgbaColor<u8>> for RgbaColor<f32> {
    fn from(col: RgbaColor<u8>) -> Self {
        Self {
            red: (col.red as f32) / 255.0,
            green: (col.green as f32) / 255.0,
            blue: (col.blue as f32) / 255.0,
            alpha: (col.alpha as f32) / 255.0,
        }
    }
}

impl From<Color> for RgbaColor<f32> {
    fn from(col: Color) -> Self {
        let u8col: RgbaColor<u8> = col.into();
        u8col.into()
    }
}

impl From<RgbaColor<f32>> for Color {
    fn from(col: RgbaColor<f32>) -> Self {
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
        RgbaColor { alpha, red, green, blue }.into()
    }

    /// Construct a color from the red, green and blue color channel parameters. The alpha
    /// channel will have the value 255.
    pub fn from_rgb_f32(red: f32, green: f32, blue: f32) -> Self {
        Self::from_argb_f32(1.0, red, green, blue)
    }

    /// Converts this color to an RgbaColor struct for easy destructuring.
    pub fn to_argb_u8(&self) -> RgbaColor<u8> {
        RgbaColor::from(*self)
    }

    /// Converts this color to an RgbaColor struct for easy destructuring.
    pub fn to_argb_f32(&self) -> RgbaColor<f32> {
        RgbaColor::from(*self)
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

#[cfg(feature = "femtovg_backend")]
impl From<&Color> for femtovg::Color {
    fn from(col: &Color) -> Self {
        Self::rgba(col.red, col.green, col.blue, col.alpha)
    }
}

#[cfg(feature = "femtovg_backend")]
impl From<Color> for femtovg::Color {
    fn from(col: Color) -> Self {
        Self::rgba(col.red, col.green, col.blue, col.alpha)
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
    EmbeddedRgbaImage { width: u32, height: u32, data: super::sharedvector::SharedVector<u32> },
}

impl Default for Resource {
    fn default() -> Self {
        Resource::None
    }
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
    type ItemRenderer: crate::items::ItemRenderer;
    fn new_renderer(&mut self, width: u32, height: u32, clear_color: &Color) -> Self::ItemRenderer;
    fn flush_renderer(&mut self, renderer: Self::ItemRenderer);

    /// Returns the window that the backend is associated with.
    fn window(&self) -> &winit::window::Window;
}

type WindowFactoryFn<Backend> =
    dyn Fn(&crate::eventloop::EventLoop, winit::window::WindowBuilder) -> Backend;

struct MappedWindow<Backend: GraphicsBackend + 'static> {
    backend: RefCell<Backend>,
    constraints: Cell<crate::layout::LayoutInfo>,
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
    component: std::cell::RefCell<ComponentWeak>,
    /// Gets dirty when the layout restrictions, or some other property of the windows change
    meta_property_listener: Pin<Rc<PropertyTracker>>,
    focus_item: std::cell::RefCell<ItemWeak>,
    mouse_input_state: std::cell::Cell<crate::input::MouseInputState>,
    /// Current popup's component and position
    /// FIXME: the popup should actually be another window, not just some overlay
    active_popup: std::cell::RefCell<Option<(ComponentRc, Point)>>,
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
            component: Default::default(),
            meta_property_listener: Rc::pin(Default::default()),
            focus_item: Default::default(),
            mouse_input_state: Default::default(),
            active_popup: Default::default(),
        })
    }

    /// Returns the window id of the window if it is mapped, None otherwise.
    pub fn id(&self) -> Option<winit::window::WindowId> {
        Some(self.map_state.borrow().as_mapped().backend.borrow().window().id())
    }

    fn apply_geometry_constraint(&self, constraints: crate::layout::LayoutInfo) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                if constraints != window.constraints.get() {
                    let min_width = constraints.min_width.min(constraints.max_width);
                    let min_height = constraints.min_height.min(constraints.max_height);
                    let max_width = constraints.max_width.max(constraints.min_width);
                    let max_height = constraints.max_height.max(constraints.min_height);

                    window.backend.borrow().window().set_min_inner_size(
                        if min_width > 0. || min_height > 0. {
                            Some(winit::dpi::PhysicalSize::new(min_width, min_height))
                        } else {
                            None
                        },
                    );
                    window.backend.borrow().window().set_max_inner_size(
                        if max_width < f32::MAX || max_height < f32::MAX {
                            Some(winit::dpi::PhysicalSize::new(
                                max_width.min(65535.),
                                max_height.min(65535.),
                            ))
                        } else {
                            None
                        },
                    );
                    window.constraints.set(constraints);
                }
            }
        }
    }
    fn apply_window_properties(&self, window_item: Pin<&crate::items::Window>) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                let backend = window.backend.borrow();
                backend.window().set_title(
                    crate::items::Window::FIELD_OFFSETS.title.apply_pin(window_item).get().as_str(),
                );
            }
        }
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
    fn set_component(self: Rc<Self>, component: &ComponentRc) {
        *self.component.borrow_mut() = vtable::VRc::downgrade(&component)
    }

    fn draw(self: Rc<Self>) {
        let component_rc = self.component.borrow().upgrade().unwrap();
        let component = ComponentRc::borrow_pin(&component_rc);

        {
            if self.meta_property_listener.as_ref().is_dirty() {
                self.meta_property_listener.as_ref().evaluate(|| {
                    self.apply_geometry_constraint(component.as_ref().layout_info());
                    component.as_ref().apply_layout(self.get_geometry());

                    let root_item = component.as_ref().get_item_ref(0);
                    if let Some(window_item) = ItemRef::downcast_pin(root_item) {
                        self.apply_window_properties(window_item);
                    }

                    if let Some((popup, pos)) = &*self.active_popup.borrow() {
                        let popup = ComponentRc::borrow_pin(popup);
                        let popup_root = popup.as_ref().get_item_ref(0);
                        let size = if let Some(window_item) = ItemRef::downcast_pin(popup_root) {
                            let layout_info = popup.as_ref().layout_info();

                            let width =
                                crate::items::Window::FIELD_OFFSETS.width.apply_pin(window_item);
                            let mut w = width.get();
                            if w < layout_info.min_width {
                                w = layout_info.min_width;
                                width.set(w);
                            }

                            let height =
                                crate::items::Window::FIELD_OFFSETS.height.apply_pin(window_item);
                            let mut h = height.get();
                            if h < layout_info.min_height {
                                h = layout_info.min_height;
                                height.set(h);
                            }
                            Size::new(h, w)
                        } else {
                            Size::default()
                        };
                        popup.as_ref().apply_layout(Rect::new(pos.clone(), size));
                    }
                })
            }
        }

        let map_state = self.map_state.borrow();
        let window = map_state.as_mapped();
        let mut backend = window.backend.borrow_mut();
        let size = backend.window().inner_size();
        let root_item = component.as_ref().get_item_ref(0);
        let background_color = if let Some(window_item) = ItemRef::downcast_pin(root_item) {
            crate::items::Window::FIELD_OFFSETS.color.apply_pin(window_item).get()
        } else {
            RgbaColor { red: 255 as u8, green: 255, blue: 255, alpha: 255 }.into()
        };

        let mut renderer = backend.new_renderer(size.width, size.height, &background_color);
        crate::item_rendering::render_component_items(
            &component_rc,
            &mut renderer,
            &self,
            Point::default(),
        );
        if let Some(popup) = &*self.active_popup.borrow() {
            crate::item_rendering::render_component_items(&popup.0, &mut renderer, &self, popup.1);
        }
        backend.flush_renderer(renderer);
    }

    fn process_mouse_input(
        self: Rc<Self>,
        pos: winit::dpi::PhysicalPosition<f64>,
        what: MouseEventType,
    ) {
        let mut pos = euclid::point2(pos.x as _, pos.y as _);
        let active_popup = (*self.active_popup.borrow()).clone();
        let component = if let Some(popup) = &active_popup {
            pos -= popup.1.to_vector();
            if what == MouseEventType::MousePressed {
                // close the popup if one press outside the popup
                let geom =
                    ComponentRc::borrow_pin(&popup.0).as_ref().get_item_ref(0).as_ref().geometry();
                if !geom.contains(pos) {
                    self.close_popup();
                    return;
                }
            }
            popup.0.clone()
        } else {
            self.component.borrow().upgrade().unwrap()
        };

        self.mouse_input_state.set(crate::input::process_mouse_input(
            component,
            MouseEvent { pos, what },
            &crate::eventloop::ComponentWindow::new(self.clone()),
            self.mouse_input_state.take(),
        ));

        if active_popup.is_some() {
            //FIXME: currently the ComboBox is the only thing that uses the popup, and it should close automatically
            // on release.  But ideally, there would be API to close the popup rather than always closing it on release
            if what == MouseEventType::MouseReleased {
                self.close_popup();
            }
        }
    }

    fn process_key_input(self: Rc<Self>, event: &KeyEvent) {
        if let Some(focus_item) = self.as_ref().focus_item.borrow().upgrade() {
            let window = &crate::eventloop::ComponentWindow::new(self.clone());
            focus_item.borrow().as_ref().key_event(event, &window);
        }
    }

    fn with_platform_window(&self, callback: &dyn Fn(&winit::window::Window)) {
        let map_state = self.map_state.borrow();
        let window = map_state.as_mapped();
        let backend = window.backend.borrow();
        let handle = backend.window();
        callback(handle);
    }

    fn map_window(self: Rc<Self>, event_loop: &crate::eventloop::EventLoop) {
        if matches!(&*self.map_state.borrow(), GraphicsWindowBackendState::Mapped(..)) {
            return;
        }

        let component = self.component.borrow().upgrade().unwrap();
        let component = ComponentRc::borrow_pin(&component);
        let root_item = component.as_ref().get_item_ref(0);

        let window_title = if let Some(window_item) = ItemRef::downcast_pin(root_item) {
            crate::items::Window::FIELD_OFFSETS.title.apply_pin(window_item).get().to_string()
        } else {
            "SixtyFPS Window".to_string()
        };
        let window_builder = winit::window::WindowBuilder::new().with_title(window_title);

        let id = {
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
                constraints: Default::default(),
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

    fn get_geometry(&self) -> crate::graphics::Rect {
        euclid::rect(
            0.,
            0.,
            WindowProperties::FIELD_OFFSETS.width.apply_pin(self.properties.as_ref()).get(),
            WindowProperties::FIELD_OFFSETS.height.apply_pin(self.properties.as_ref()).get(),
        )
    }

    fn free_graphics_resources<'a>(self: Rc<Self>, items: &Slice<'a, Pin<ItemRef<'a>>>) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped => {}
            GraphicsWindowBackendState::Mapped(window) => {
                let mut backend = window.backend.borrow_mut();
                crate::item_rendering::free_item_rendering_data(items, &mut *backend)
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

    fn set_focus_item(self: Rc<Self>, focus_item: &ItemRc) {
        let window = crate::eventloop::ComponentWindow::new(self.clone());

        if let Some(old_focus_item) = self.as_ref().focus_item.borrow().upgrade() {
            old_focus_item
                .borrow()
                .as_ref()
                .focus_event(&crate::input::FocusEvent::FocusOut, &window);
        }

        *self.as_ref().focus_item.borrow_mut() = focus_item.downgrade();

        focus_item.borrow().as_ref().focus_event(&crate::input::FocusEvent::FocusIn, &window);
    }

    fn set_focus(self: Rc<Self>, have_focus: bool) {
        let window = crate::eventloop::ComponentWindow::new(self.clone());
        let event = if have_focus {
            crate::input::FocusEvent::WindowReceivedFocus
        } else {
            crate::input::FocusEvent::WindowLostFocus
        };

        if let Some(focus_item) = self.as_ref().focus_item.borrow().upgrade() {
            focus_item.borrow().as_ref().focus_event(&event, &window);
        }
    }

    fn show_popup(&self, popup: &ComponentRc, position: Point) {
        self.meta_property_listener.set_dirty();
        *self.active_popup.borrow_mut() = Some((popup.clone(), position));
    }

    fn close_popup(&self) {
        *self.active_popup.borrow_mut() = None;
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
    FromEvents(&'a crate::SharedVector<PathEvent>, &'a crate::SharedVector<Point>),
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
    Elements(crate::SharedVector<PathElement>),
    /// The Events variant describes the path as a series of low-level events and
    /// associated coordinates.
    Events(crate::SharedVector<PathEvent>, crate::SharedVector<Point>),
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

    /// Expand IntRect so that cbindgen can see it. ( is in fact euclid::default::Rect<i32>)
    #[cfg(cbindgen)]
    #[repr(C)]
    struct IntRect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
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
        let arr = crate::SharedVector::from(std::slice::from_raw_parts(first_element, count));
        core::ptr::write(out as *mut crate::SharedVector<PathElement>, arr.clone());
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
        let events =
            crate::SharedVector::from(std::slice::from_raw_parts(first_event, event_count));
        core::ptr::write(out_events as *mut crate::SharedVector<PathEvent>, events.clone());
        let coordinates = crate::SharedVector::from(std::slice::from_raw_parts(
            first_coordinate,
            coordinate_count,
        ));
        core::ptr::write(out_coordinates as *mut crate::SharedVector<Point>, coordinates.clone());
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
                toggle_cursor,
            );
        }
    }

    fn stop(&self) {
        self.cursor_blink_timer.stop()
    }
}
