// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! This module contains the GraphicsWindow that used to be within corelib.

// cspell:ignore borderless corelib nesw webgl winit winsys xlib

use core::cell::Cell;
#[cfg(target_arch = "wasm32")]
use core::cell::RefCell;
use core::pin::Pin;
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use std::rc::Weak;

#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;

use crate::renderer::WinitCompatibleRenderer;
use const_field_offset::FieldOffsets;

use corelib::component::ComponentRc;
#[cfg(enable_accesskit)]
use corelib::component::ComponentRef;
use corelib::items::MouseCursor;
#[cfg(enable_accesskit)]
use corelib::items::{ItemRc, ItemRef};

use corelib::api::PhysicalSize;
use corelib::layout::Orientation;
use corelib::lengths::LogicalLength;
use corelib::platform::{PlatformError, WindowEvent};
use corelib::window::{WindowAdapter, WindowAdapterInternal, WindowInner};
use corelib::Property;
use corelib::{graphics::*, Coord};
use i_slint_core as corelib;
use once_cell::unsync::OnceCell;

fn position_to_winit(pos: &corelib::api::WindowPosition) -> winit::dpi::Position {
    match pos {
        corelib::api::WindowPosition::Logical(pos) => {
            winit::dpi::Position::new(winit::dpi::LogicalPosition::new(pos.x, pos.y))
        }
        corelib::api::WindowPosition::Physical(pos) => {
            winit::dpi::Position::new(winit::dpi::PhysicalPosition::new(pos.x, pos.y))
        }
    }
}

fn window_size_to_slint(size: &corelib::api::WindowSize) -> winit::dpi::Size {
    match size {
        corelib::api::WindowSize::Logical(size) => {
            winit::dpi::Size::new(winit::dpi::LogicalSize::new(size.width, size.height))
        }
        corelib::api::WindowSize::Physical(size) => {
            winit::dpi::Size::new(winit::dpi::PhysicalSize::new(size.width, size.height))
        }
    }
}

fn physical_size_to_slint(size: &winit::dpi::PhysicalSize<u32>) -> corelib::api::PhysicalSize {
    corelib::api::PhysicalSize::new(size.width, size.height)
}

fn icon_to_winit(icon: corelib::graphics::Image) -> Option<winit::window::Icon> {
    let image_inner: &ImageInner = (&icon).into();

    let pixel_buffer = match image_inner {
        ImageInner::EmbeddedImage { buffer, .. } => buffer.clone(),
        _ => return None,
    };

    // This could become a method in SharedPixelBuffer...
    let rgba_pixels: Vec<u8> = match &pixel_buffer {
        SharedImageBuffer::RGB8(pixels) => pixels
            .as_bytes()
            .chunks(3)
            .flat_map(|rgb| IntoIterator::into_iter([rgb[0], rgb[1], rgb[2], 255]))
            .collect(),
        SharedImageBuffer::RGBA8(pixels) => pixels.as_bytes().to_vec(),
        SharedImageBuffer::RGBA8Premultiplied(pixels) => pixels
            .as_bytes()
            .chunks(4)
            .flat_map(|rgba| {
                let alpha = rgba[3] as u32;
                IntoIterator::into_iter(rgba)
                    .take(3)
                    .map(move |component| (*component as u32 * alpha / 255) as u8)
                    .chain(std::iter::once(alpha as u8))
            })
            .collect(),
    };

    winit::window::Icon::from_rgba(rgba_pixels, pixel_buffer.width(), pixel_buffer.height()).ok()
}

fn window_is_resizable(
    min_size: Option<corelib::api::LogicalSize>,
    max_size: Option<corelib::api::LogicalSize>,
) -> bool {
    if let Some((
        corelib::api::LogicalSize { width: min_width, height: min_height, .. },
        corelib::api::LogicalSize { width: max_width, height: max_height, .. },
    )) = min_size.zip(max_size)
    {
        min_width < max_width || min_height < max_height
    } else {
        true
    }
}

/// GraphicsWindow is an implementation of the [WindowAdapter][`crate::eventloop::WindowAdapter`] trait. This is
/// typically instantiated by entry factory functions of the different graphics back ends.
pub struct WinitWindowAdapter {
    window: OnceCell<corelib::api::Window>,
    #[cfg(target_arch = "wasm32")]
    self_weak: Weak<Self>,
    currently_pressed_key_code: std::cell::Cell<Option<winit::event::VirtualKeyCode>>,
    pending_redraw: Cell<bool>,
    dark_color_scheme: OnceCell<Pin<Box<Property<bool>>>>,
    constraints: Cell<corelib::window::LayoutConstraints>,
    shown: Cell<bool>,
    window_level: Cell<winit::window::WindowLevel>,

    pub(crate) renderer: Box<dyn WinitCompatibleRenderer>,
    // We cache the size because winit_window.inner_size() can return different value between calls (eg, on X11)
    // And we wan see the newer value before the Resized event was received, leading to inconsistencies
    size: Cell<PhysicalSize>,

    #[cfg(target_arch = "wasm32")]
    virtual_keyboard_helper: RefCell<Option<super::wasm_input_helper::WasmInputHelper>>,

    #[cfg(enable_accesskit)]
    pub accesskit_adapter: crate::accesskit::AccessKitAdapter,

    winit_window: Rc<winit::window::Window>, // Last field so that any previously provided window handles are still valid in the drop impl of the renderers, etc.
}

impl WinitWindowAdapter {
    /// Creates a new reference-counted instance.
    #[allow(clippy::new_ret_no_self)]
    pub(crate) fn new<R: WinitCompatibleRenderer + 'static>(
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let (renderer, winit_window) = Self::window_builder(
            #[cfg(target_arch = "wasm32")]
            canvas_id,
        )
        .and_then(|builder| R::new(builder))?;

        let winit_window = Rc::new(winit_window);

        let self_rc = Rc::new_cyclic(|self_weak| Self {
            window: OnceCell::with_value(corelib::api::Window::new(self_weak.clone() as _)),
            #[cfg(target_arch = "wasm32")]
            self_weak: self_weak.clone(),
            currently_pressed_key_code: Default::default(),
            pending_redraw: Default::default(),
            dark_color_scheme: Default::default(),
            constraints: Default::default(),
            shown: Default::default(),
            window_level: Default::default(),
            winit_window: winit_window.clone(),
            size: Default::default(),
            renderer: Box::new(renderer),
            #[cfg(target_arch = "wasm32")]
            virtual_keyboard_helper: Default::default(),
            #[cfg(enable_accesskit)]
            accesskit_adapter: crate::accesskit::AccessKitAdapter::new(
                self_weak.clone(),
                &winit_window,
            ),
        });

        let id = self_rc.winit_window().id();
        crate::event_loop::register_window(id, (self_rc.clone()) as _);

        let scale_factor = std::env::var("SLINT_SCALE_FACTOR")
            .ok()
            .and_then(|x| x.parse::<f32>().ok())
            .filter(|f| *f > 0.)
            .unwrap_or_else(|| self_rc.winit_window().scale_factor() as f32);
        self_rc.window().dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor });

        Ok(self_rc as _)
    }

    fn renderer(&self) -> &dyn WinitCompatibleRenderer {
        self.renderer.as_ref()
    }

    fn window_builder(
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) -> Result<winit::window::WindowBuilder, PlatformError> {
        let mut window_builder =
            winit::window::WindowBuilder::new().with_transparent(true).with_visible(false);

        if std::env::var("SLINT_FULLSCREEN").is_ok() {
            window_builder =
                window_builder.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }

        window_builder = window_builder.with_title("Slint Window".to_string());

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowBuilderExtWebSys;

            use wasm_bindgen::JsCast;

            let html_canvas = web_sys::window()
                .ok_or_else(|| "winit backend: Could not retrieve DOM window".to_string())?
                .document()
                .ok_or_else(|| "winit backend: Could not retrieve DOM document".to_string())?
                .get_element_by_id(canvas_id)
                .ok_or_else(|| {
                    format!(
                        "winit backend: Could not retrieve existing HTML Canvas element '{}'",
                        canvas_id
                    )
                })?
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .map_err(|_| {
                    format!(
                        "winit backend: Specified DOM element '{}' is not a HTML Canvas",
                        canvas_id
                    )
                })?;
            window_builder = window_builder.with_canvas(Some(html_canvas))
        };

        Ok(window_builder)
    }

    pub fn take_pending_redraw(&self) -> bool {
        self.pending_redraw.take()
    }

    pub fn currently_pressed_key_code(&self) -> &Cell<Option<winit::event::VirtualKeyCode>> {
        &self.currently_pressed_key_code
    }

    /// Draw the items of the specified `component` in the given window.
    pub fn draw(&self) -> Result<bool, PlatformError> {
        if !self.shown.get() {
            return Ok(false); // caller bug, doesn't make sense to call draw() when not shown
        }

        self.pending_redraw.set(false);

        let renderer = self.renderer();
        renderer.render(self.window())?;

        Ok(self.pending_redraw.get())
    }

    fn with_window_handle(&self, callback: &mut dyn FnMut(&winit::window::Window)) {
        callback(&self.winit_window());
    }

    pub fn winit_window(&self) -> Rc<winit::window::Window> {
        self.winit_window.clone()
    }

    pub fn is_shown(&self) -> bool {
        self.shown.get()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn input_method_focused(&self) -> bool {
        match self.virtual_keyboard_helper.try_borrow() {
            Ok(vkh) => vkh.as_ref().map_or(false, |h| h.has_focus()),
            // the only location in which the virtual_keyboard_helper is mutably borrowed is from
            // show_virtual_keyboard, which means we have the focus
            Err(_) => true,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn input_method_focused(&self) -> bool {
        false
    }

    pub fn resize_event(&self, size: winit::dpi::PhysicalSize<u32>) -> Result<(), PlatformError> {
        // When a window is minimized on Windows, we get a move event to an off-screen position
        // and a resize even with a zero size. Don't forward that, especially not to the renderer,
        // which might panic when trying to create a zero-sized surface.
        if size.width > 0 && size.height > 0 {
            let physical_size = physical_size_to_slint(&size);
            self.size.set(physical_size);
            let scale_factor = WindowInner::from_pub(self.window()).scale_factor();
            self.window().dispatch_event(WindowEvent::Resized {
                size: physical_size.to_logical(scale_factor),
            });
        }
        Ok(())
    }

    pub fn set_dark_color_scheme(&self, dark_mode: bool) {
        self.dark_color_scheme
            .get_or_init(|| Box::pin(Property::new(false)))
            .as_ref()
            .set(dark_mode)
    }
}

impl WindowAdapter for WinitWindowAdapter {
    fn window(&self) -> &corelib::api::Window {
        self.window.get().unwrap()
    }

    fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        self.renderer().as_core_renderer()
    }

    fn set_visible(&self, visible: bool) -> Result<(), PlatformError> {
        self.shown.set(visible);
        if visible {
            let winit_window = self.winit_window();

            let runtime_window = WindowInner::from_pub(self.window());

            let scale_factor = runtime_window.scale_factor() as f64;

            let component_rc = runtime_window.component();
            let component = ComponentRc::borrow_pin(&component_rc);

            let layout_info_h = component.as_ref().layout_info(Orientation::Horizontal);
            if let Some(window_item) = runtime_window.window_item() {
                // Setting the width to its preferred size before querying the vertical layout info
                // is important in case the height depends on the width
                window_item.width.set(LogicalLength::new(layout_info_h.preferred_bounded()));
            }
            let layout_info_v = component.as_ref().layout_info(Orientation::Vertical);
            #[allow(unused_mut)]
            let mut preferred_size = winit::dpi::LogicalSize::new(
                layout_info_h.preferred_bounded(),
                layout_info_v.preferred_bounded(),
            );

            #[cfg(target_arch = "wasm32")]
            {
                let html_canvas = winit_window.canvas();
                let existing_canvas_size = winit::dpi::LogicalSize::new(
                    html_canvas.client_width() as f32,
                    html_canvas.client_height() as f32,
                );

                // Try to maintain the existing size of the canvas element. A window created with winit
                // on the web will always have 1024x768 as size otherwise.
                if preferred_size.width <= 0. {
                    preferred_size.width = existing_canvas_size.width;
                }
                if preferred_size.height <= 0. {
                    preferred_size.height = existing_canvas_size.height;
                }
            }

            if winit_window.fullscreen().is_none()
                && preferred_size.width > 0 as Coord
                && preferred_size.height > 0 as Coord
            {
                // use the Slint's window Scale factor to take in account the override
                let size = preferred_size.to_physical::<u32>(scale_factor);
                winit_window.set_inner_size(size);
                self.size.set(physical_size_to_slint(&size));
            };

            winit_window.set_visible(true);

            // Make sure the dark color scheme property is up-to-date, as it may have been queried earlier when
            // the window wasn't mapped yet.
            if let Some(dark_color_scheme_prop) = self.dark_color_scheme.get() {
                if let Some(theme) = winit_window.theme() {
                    dark_color_scheme_prop.as_ref().set(theme == winit::window::Theme::Dark)
                }
            }

            // In wasm a request_redraw() issued before show() results in a draw() even when the window
            // isn't visible, as opposed to regular windowing systems. The compensate for the lost draw,
            // explicitly render the first frame on show().
            #[cfg(target_arch = "wasm32")]
            if self.pending_redraw.get() {
                self.draw()?;
            };

            Ok(())
        } else {
            self.winit_window().set_visible(false);

            /* FIXME:
            if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
                existing_blinker.stop();
            }*/
            crate::send_event_via_global_event_loop_proxy(crate::SlintUserEvent::CustomEvent {
                event: crate::event_loop::CustomEvent::WindowHidden,
            })
            .ok(); // It's okay to call hide() even after the event loop is closed. We don't need the logic for quitting the event loop anymore at this point.
            Ok(())
        }
    }

    fn position(&self) -> Option<corelib::api::PhysicalPosition> {
        match self.winit_window().outer_position() {
            Ok(outer_position) => {
                Some(corelib::api::PhysicalPosition::new(outer_position.x, outer_position.y))
            }
            Err(_) => None,
        }
    }

    fn set_position(&self, position: corelib::api::WindowPosition) {
        self.winit_window().set_outer_position(position_to_winit(&position))
    }

    fn set_size(&self, size: corelib::api::WindowSize) {
        self.winit_window().set_inner_size(window_size_to_slint(&size))
    }

    fn size(&self) -> corelib::api::PhysicalSize {
        self.size.get()
    }

    fn request_redraw(&self) {
        self.pending_redraw.set(true);
        self.with_window_handle(&mut |window| window.request_redraw())
    }

    #[allow(clippy::unnecessary_cast)] // Coord is used!
    fn update_window_properties(&self, properties: corelib::window::WindowProperties<'_>) {
        let Some(window_item) =
            self.window.get().and_then(|w| WindowInner::from_pub(w).window_item())
        else {
            return;
        };
        let window_item = window_item.as_pin_ref();

        let winit_window = self.winit_window();

        let mut width = window_item.width().get() as f32;
        let mut height = window_item.height().get() as f32;

        let mut must_resize = false;

        winit_window.set_window_icon(icon_to_winit(window_item.icon()));
        winit_window.set_title(&properties.title());
        winit_window
            .set_decorations(!window_item.no_frame() || winit_window.fullscreen().is_some());
        let new_window_level = if window_item.always_on_top() {
            winit::window::WindowLevel::AlwaysOnTop
        } else {
            winit::window::WindowLevel::Normal
        };
        // Only change the window level if it changes, to avoid https://github.com/slint-ui/slint/issues/3280
        // (Ubuntu 20.04's window manager always bringing the window to the front on x11)
        if self.window_level.replace(new_window_level) != new_window_level {
            winit_window.set_window_level(new_window_level);
        }

        if width <= 0. || height <= 0. {
            must_resize = true;

            let winit_size =
                winit_window.inner_size().to_logical(self.window().scale_factor() as f64);

            if width <= 0. {
                width = winit_size.width;
            }
            if height <= 0. {
                height = winit_size.height;
            }
        }

        let existing_size = self.size().to_logical(self.window().scale_factor());

        if (existing_size.width - width).abs() > 1. || (existing_size.height - height).abs() > 1. {
            // If we're in fullscreen state, don't try to resize the window but maintain the surface
            // size we've been assigned to from the windowing system. Weston/Wayland don't like it
            // when we create a surface that's bigger than the screen due to constraints (#532).
            if winit_window.fullscreen().is_none() {
                winit_window.set_inner_size(winit::dpi::LogicalSize::new(width, height));
            }
        }

        if must_resize {
            self.window().dispatch_event(WindowEvent::Resized {
                size: i_slint_core::api::LogicalSize::new(width, height),
            });
        }

        self.with_window_handle(&mut |winit_window| {
            // If we're in fullscreen state, don't try to resize the window but maintain the surface
            // size we've been assigned to from the windowing system. Weston/Wayland don't like it
            // when we create a surface that's bigger than the screen due to constraints (#532).
            if winit_window.fullscreen().is_some() {
                return;
            }

            let new_constraints = properties.layout_constraints();
            if new_constraints == self.constraints.get() {
                return;
            }

            self.constraints.set(new_constraints);

            // Use our scale factor instead of winit's logical size to take a scale factor override into account.
            let sf = self.window().scale_factor();

            let into_size = |s: corelib::api::LogicalSize| -> winit::dpi::PhysicalSize<f32> {
                winit::dpi::LogicalSize::new(s.width, s.height).to_physical(sf as f64)
            };

            let resizable = window_is_resizable(new_constraints.min, new_constraints.max);

            let winit_min_inner = new_constraints.min.map(into_size);
            winit_window.set_min_inner_size(winit_min_inner);
            let winit_max_inner = new_constraints.max.map(into_size);
            winit_window.set_max_inner_size(winit_max_inner);
            winit_window.set_resizable(resizable);

            adjust_window_size_to_satisfy_constraints(
                winit_window,
                winit_min_inner,
                winit_max_inner,
            );

            #[cfg(target_arch = "wasm32")]
            if let Some((
                corelib::api::LogicalSize { width: min_width, height: min_height, .. },
                corelib::api::LogicalSize { width: max_width, height: max_height, .. },
            )) = new_constraints.min.zip(new_constraints.max)
            {
                // set_max_inner_size / set_min_inner_size don't work on wasm, so apply the size manually
                let existing_size: winit::dpi::LogicalSize<f32> =
                    winit_window.inner_size().to_logical(sf as f64);
                if !(min_width..=max_width).contains(&(existing_size.width))
                    || !(min_height..=max_height).contains(&(existing_size.height))
                {
                    let new_size = winit::dpi::LogicalSize::new(
                        existing_size.width.min(max_width).max(min_width),
                        existing_size.height.min(max_height).max(min_height),
                    );
                    winit_window.set_inner_size(new_size);
                }
            }

            // Auto-resize to the preferred size if users (SlintPad) requests it
            #[cfg(target_arch = "wasm32")]
            {
                let canvas = winit_window.canvas();

                if canvas
                    .dataset()
                    .get("slintAutoResizeToPreferred")
                    .and_then(|val_str| val_str.parse().ok())
                    .unwrap_or_default()
                {
                    let pref_width = new_constraints.preferred.width;
                    let pref_height = new_constraints.preferred.height;
                    if pref_width > 0 as Coord || pref_height > 0 as Coord {
                        winit_window
                            .set_inner_size(winit::dpi::LogicalSize::new(pref_width, pref_height));
                    };
                }
            }
        });
    }

    fn internal(&self, _: corelib::InternalToken) -> Option<&dyn WindowAdapterInternal> {
        Some(self)
    }
}

impl WindowAdapterInternal for WinitWindowAdapter {
    fn set_mouse_cursor(&self, cursor: MouseCursor) {
        let winit_cursor = match cursor {
            MouseCursor::Default => winit::window::CursorIcon::Default,
            MouseCursor::None => winit::window::CursorIcon::Default,
            MouseCursor::Help => winit::window::CursorIcon::Help,
            MouseCursor::Pointer => winit::window::CursorIcon::Hand,
            MouseCursor::Progress => winit::window::CursorIcon::Progress,
            MouseCursor::Wait => winit::window::CursorIcon::Wait,
            MouseCursor::Crosshair => winit::window::CursorIcon::Crosshair,
            MouseCursor::Text => winit::window::CursorIcon::Text,
            MouseCursor::Alias => winit::window::CursorIcon::Alias,
            MouseCursor::Copy => winit::window::CursorIcon::Copy,
            MouseCursor::Move => winit::window::CursorIcon::Move,
            MouseCursor::NoDrop => winit::window::CursorIcon::NoDrop,
            MouseCursor::NotAllowed => winit::window::CursorIcon::NotAllowed,
            MouseCursor::Grab => winit::window::CursorIcon::Grab,
            MouseCursor::Grabbing => winit::window::CursorIcon::Grabbing,
            MouseCursor::ColResize => winit::window::CursorIcon::ColResize,
            MouseCursor::RowResize => winit::window::CursorIcon::RowResize,
            MouseCursor::NResize => winit::window::CursorIcon::NResize,
            MouseCursor::EResize => winit::window::CursorIcon::EResize,
            MouseCursor::SResize => winit::window::CursorIcon::SResize,
            MouseCursor::WResize => winit::window::CursorIcon::WResize,
            MouseCursor::NeResize => winit::window::CursorIcon::NeResize,
            MouseCursor::NwResize => winit::window::CursorIcon::NwResize,
            MouseCursor::SeResize => winit::window::CursorIcon::SeResize,
            MouseCursor::SwResize => winit::window::CursorIcon::SwResize,
            MouseCursor::EwResize => winit::window::CursorIcon::EwResize,
            MouseCursor::NsResize => winit::window::CursorIcon::NsResize,
            MouseCursor::NeswResize => winit::window::CursorIcon::NeswResize,
            MouseCursor::NwseResize => winit::window::CursorIcon::NwseResize,
        };
        self.with_window_handle(&mut |winit_window| {
            winit_window.set_cursor_visible(cursor != MouseCursor::None);
            winit_window.set_cursor_icon(winit_cursor);
        });
    }

    fn input_method_request(&self, request: corelib::window::InputMethodRequest) {
        #[cfg(not(target_arch = "wasm32"))]
        self.with_window_handle(&mut |winit_window| {
            match request {
                corelib::window::InputMethodRequest::Enable { input_type, .. } => winit_window
                    .set_ime_allowed(matches!(input_type, corelib::items::InputType::Text)),
                corelib::window::InputMethodRequest::Disable { .. } => {
                    winit_window.set_ime_allowed(false)
                }
                corelib::window::InputMethodRequest::SetPosition { position, .. } => {
                    winit_window.set_ime_position(position_to_winit(&position.into()))
                }
                _ => {}
            };
        });

        #[cfg(target_arch = "wasm32")]
        match request {
            corelib::window::InputMethodRequest::Enable { .. } => {
                let mut vkh = self.virtual_keyboard_helper.borrow_mut();
                let h = vkh.get_or_insert_with(|| {
                    let canvas = self.winit_window().canvas();
                    super::wasm_input_helper::WasmInputHelper::new(self.self_weak.clone(), canvas)
                });
                h.show();
            }
            corelib::window::InputMethodRequest::Disable { .. } => {
                if let Some(h) = &*self.virtual_keyboard_helper.borrow() {
                    h.hide()
                }
            }
            _ => {}
        };
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn dark_color_scheme(&self) -> bool {
        self.dark_color_scheme
            .get_or_init(|| {
                Box::pin(Property::new({
                    self.winit_window()
                        .theme()
                        .map_or(false, |theme| theme == winit::window::Theme::Dark)
                }))
            })
            .as_ref()
            .get()
    }

    #[cfg(enable_accesskit)]
    fn handle_focus_change(&self, _old: Option<ItemRc>, _new: Option<ItemRc>) {
        self.accesskit_adapter.handle_focus_item_change();
    }

    #[cfg(enable_accesskit)]
    fn register_component(&self) {
        self.accesskit_adapter.register_component();
    }

    #[cfg(enable_accesskit)]
    fn unregister_component(
        &self,
        _component: ComponentRef,
        _: &mut dyn Iterator<Item = Pin<ItemRef<'_>>>,
    ) {
        self.accesskit_adapter.unregister_component(_component);
    }
}

impl Drop for WinitWindowAdapter {
    fn drop(&mut self) {
        crate::event_loop::unregister_window(self.winit_window().id());
    }
}

#[derive(FieldOffsets)]
#[repr(C)]
#[pin]
struct WindowProperties {
    scale_factor: Property<f32>,
}

impl Default for WindowProperties {
    fn default() -> Self {
        Self { scale_factor: Property::new(1.0) }
    }
}

// Winit doesn't automatically resize the window to satisfy constraints. Qt does it though, and so do we here.
fn adjust_window_size_to_satisfy_constraints(
    winit_window: &winit::window::Window,
    min_size: Option<winit::dpi::PhysicalSize<f32>>,
    max_size: Option<winit::dpi::PhysicalSize<f32>>,
) {
    let mut window_size = winit_window.inner_size();

    if let Some(min_size) = min_size {
        let min_size = min_size.cast();
        window_size.width = window_size.width.max(min_size.width);
        window_size.height = window_size.height.max(min_size.height);
    }

    if let Some(max_size) = max_size {
        let max_size = max_size.cast();
        window_size.width = window_size.width.min(max_size.width);
        window_size.height = window_size.height.min(max_size.height);
    }

    if window_size != winit_window.inner_size() {
        winit_window.set_inner_size(window_size);
    }
}
