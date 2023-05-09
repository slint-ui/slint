// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains the GraphicsWindow that used to be within corelib.

// cspell:ignore borderless corelib nesw webgl winit winsys xlib

use core::cell::Cell;
#[cfg(target_arch = "wasm32")]
use core::cell::RefCell;
use core::pin::Pin;
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use std::rc::Weak;

use crate::event_loop::WinitWindow;
use crate::renderer::WinitCompatibleRenderer;
use const_field_offset::FieldOffsets;

use corelib::component::ComponentRc;
use corelib::items::MouseCursor;

use corelib::layout::Orientation;
use corelib::lengths::{LogicalLength, LogicalPoint, LogicalSize};
use corelib::platform::PlatformError;
use corelib::window::{WindowAdapter, WindowAdapterSealed, WindowInner};
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

fn window_is_resizable(min_size: Option<LogicalSize>, max_size: Option<LogicalSize>) -> bool {
    if let Some((
        LogicalSize { width: min_width, height: min_height, .. },
        LogicalSize { width: max_width, height: max_height, .. },
    )) = min_size.zip(max_size)
    {
        min_width < max_width || min_height < max_height
    } else {
        true
    }
}

/// GraphicsWindow is an implementation of the [WindowAdapter][`crate::eventloop::WindowAdapter`] trait. This is
/// typically instantiated by entry factory functions of the different graphics back ends.
pub(crate) struct WinitWindowAdapter<Renderer: WinitCompatibleRenderer + 'static> {
    window: OnceCell<corelib::api::Window>,
    #[cfg(target_arch = "wasm32")]
    self_weak: Weak<Self>,
    currently_pressed_key_code: std::cell::Cell<Option<winit::event::VirtualKeyCode>>,
    pending_redraw: Cell<bool>,
    in_resize_event: Cell<bool>,
    dark_color_scheme: OnceCell<Pin<Box<Property<bool>>>>,
    constraints: Cell<(corelib::layout::LayoutInfo, corelib::layout::LayoutInfo)>,
    shown: Cell<bool>,

    renderer: OnceCell<Renderer>,

    #[cfg(target_arch = "wasm32")]
    virtual_keyboard_helper: RefCell<Option<super::wasm_input_helper::WasmInputHelper>>,
}

impl<Renderer: WinitCompatibleRenderer + 'static> Default for WinitWindowAdapter<Renderer> {
    fn default() -> Self {
        Self {
            window: Default::default(),
            #[cfg(target_arch = "wasm32")]
            self_weak: Default::default(),
            currently_pressed_key_code: Default::default(),
            pending_redraw: Default::default(),
            in_resize_event: Default::default(),
            dark_color_scheme: Default::default(),
            constraints: Default::default(),
            shown: Default::default(),
            renderer: Default::default(),
            #[cfg(target_arch = "wasm32")]
            virtual_keyboard_helper: Default::default(),
        }
    }
}

impl<Renderer: WinitCompatibleRenderer + 'static> WinitWindowAdapter<Renderer> {
    /// Creates a new reference-counted instance.
    pub(crate) fn new(
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        // Error that occured during construction. This is only used temporarily during new.
        let mut platform_error: Option<PlatformError> = None;

        let self_rc = Rc::new_cyclic(|self_weak| {
            let mut result = Self::default();

            match Self::window_builder(
                #[cfg(target_arch = "wasm32")]
                canvas_id,
            )
            .and_then(|builder| {
                Renderer::new(
                    &(self_weak.clone() as _),
                    builder,
                    #[cfg(target_arch = "wasm32")]
                    canvas_id,
                )
            }) {
                Ok(new_renderer) => result.renderer = OnceCell::with_value(new_renderer),
                Err(err) => {
                    platform_error = Some(err);
                    return Self::default();
                }
            };

            result.window = OnceCell::with_value(corelib::api::Window::new(self_weak.clone() as _));
            #[cfg(target_arch = "wasm32")]
            {
                result.self_weak = self_weak.clone();
            }
            result
        });
        if let Some(err) = platform_error.take() {
            Err(err)
        } else {
            let id = self_rc.renderer().window().id();
            crate::event_loop::register_window(id, (self_rc.clone()) as _);
            Ok(self_rc as _)
        }
    }

    fn unmap(&self) -> Result<(), PlatformError> {
        self.renderer().hide()
    }

    fn call_with_event_loop(
        &self,
        callback: fn(&Self) -> Result<(), PlatformError>,
    ) -> Result<(), PlatformError> {
        // With wasm, winit's `run()` consumes the event loop and access to it from outside the event handler yields
        // loop and thus ends up trying to create a new event loop instance, which panics in winit. Instead, forward
        // the call to be invoked from within the event loop
        #[cfg(target_arch = "wasm32")]
        return corelib::api::invoke_from_event_loop({
            let self_weak = send_wrapper::SendWrapper::new(self.self_weak.clone());

            move || {
                if let Some(this) = self_weak.take().upgrade() {
                    // Can't propagate the returned error because we're in an async callback, so throw.
                    callback(&this).unwrap()
                }
            }
        })
        .map_err(|_| {
            format!("internal error in winit backend: invoke_from_event_loop failed").into()
        });
        #[cfg(not(target_arch = "wasm32"))]
        return callback(self);
    }

    fn renderer(&self) -> &Renderer {
        self.renderer.get().unwrap()
    }

    fn window_builder(
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) -> Result<winit::window::WindowBuilder, PlatformError> {
        let mut window_builder =
            winit::window::WindowBuilder::new().with_transparent(true).with_visible(false);

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
}

impl<Renderer: WinitCompatibleRenderer + 'static> WinitWindow for WinitWindowAdapter<Renderer> {
    fn take_pending_redraw(&self) -> bool {
        self.pending_redraw.take()
    }

    fn currently_pressed_key_code(&self) -> &Cell<Option<winit::event::VirtualKeyCode>> {
        &self.currently_pressed_key_code
    }

    /// Draw the items of the specified `component` in the given window.
    fn draw(&self) -> Result<bool, PlatformError> {
        if !self.shown.get() {
            return Ok(false); // caller bug, doesn't make sense to call draw() when not shown
        }

        self.pending_redraw.set(false);

        let renderer = self.renderer();
        renderer.render(physical_size_to_slint(&renderer.window().inner_size()))?;

        Ok(self.pending_redraw.get())
    }

    fn with_window_handle(&self, callback: &mut dyn FnMut(&winit::window::Window)) {
        callback(&self.renderer().window());
    }

    fn winit_window(&self) -> Rc<winit::window::Window> {
        self.renderer().window()
    }

    fn is_shown(&self) -> bool {
        self.shown.get()
    }

    #[cfg(target_arch = "wasm32")]
    fn input_method_focused(&self) -> bool {
        match self.virtual_keyboard_helper.try_borrow() {
            Ok(vkh) => vkh.as_ref().map_or(false, |h| h.has_focus()),
            // the only location in which the virtual_keyboard_helper is mutably borrowed is from
            // show_virtual_keyboard, which means we have the focus
            Err(_) => true,
        }
    }

    fn resize_event(&self, size: winit::dpi::PhysicalSize<u32>) -> Result<(), PlatformError> {
        // slint::Window::set_size will call set_size() on this type, which would call
        // set_inner_size on the winit Window. On Windows that triggers an new resize event
        // in the next event loop iteration for mysterious reasons, with slightly different sizes.
        // I suspect a bug in the way the frame decorations are added and subtracted from the size
        // we provide.
        // Work around it with this guard that prevents us from calling set_inner_size again.
        assert!(!self.in_resize_event.get());
        self.in_resize_event.set(true);
        scopeguard::defer! { self.in_resize_event.set(false); }

        // When a window is minimized on Windows, we get a move event to an off-screen position
        // and a resize even with a zero size. Don't forward that, especially not to the renderer,
        // which might panic when trying to create a zero-sized surface.
        if size.width > 0 && size.height > 0 {
            let physical_size = physical_size_to_slint(&size);
            self.window().set_size(physical_size);
            self.renderer().resize_event(physical_size)
        } else {
            Ok(())
        }
    }

    fn set_dark_color_scheme(&self, dark_mode: bool) {
        self.dark_color_scheme
            .get_or_init(|| Box::pin(Property::new(false)))
            .as_ref()
            .set(dark_mode)
    }
}

impl<Renderer: WinitCompatibleRenderer + 'static> WindowAdapter for WinitWindowAdapter<Renderer> {
    fn window(&self) -> &corelib::api::Window {
        self.window.get().unwrap()
    }
}

impl<Renderer: WinitCompatibleRenderer + 'static> WindowAdapterSealed
    for WinitWindowAdapter<Renderer>
{
    fn request_redraw(&self) {
        self.pending_redraw.set(true);
        self.with_window_handle(&mut |window| window.request_redraw())
    }

    fn request_window_properties_update(&self) {
        self.call_with_event_loop(|self_| {
            self_.with_window_handle(&mut |window| {
                let window_id = window.id();
                crate::event_loop::with_window_target(|event_loop| {
                    event_loop.event_loop_proxy().send_event(crate::SlintUserEvent::CustomEvent {
                        event: crate::event_loop::CustomEvent::UpdateWindowProperties(window_id),
                    })
                })
                .ok();
            });
            Ok(()) // Doesn't matter if the eventloop is already closed, nothing to update then.
        })
        .ok();
    }

    fn apply_window_properties(&self, window_item: Pin<&i_slint_core::items::WindowItem>) {
        let winit_window = self.winit_window();

        let mut width = window_item.width().get() as f32;
        let mut height = window_item.height().get() as f32;

        let mut must_resize = false;

        winit_window.set_window_icon(icon_to_winit(window_item.icon()));
        winit_window.set_title(&window_item.title());
        winit_window
            .set_decorations(!window_item.no_frame() || winit_window.fullscreen().is_some());
        winit_window.set_window_level(if window_item.always_on_top() {
            winit::window::WindowLevel::AlwaysOnTop
        } else {
            winit::window::WindowLevel::Normal
        });

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

        let existing_size: winit::dpi::LogicalSize<f32> =
            winit_window.inner_size().to_logical(self.window().scale_factor().into());

        if (existing_size.width - width).abs() > 1. || (existing_size.height - height).abs() > 1. {
            // If we're in fullscreen state, don't try to resize the window but maintain the surface
            // size we've been assigned to from the windowing system. Weston/Wayland don't like it
            // when we create a surface that's bigger than the screen due to constraints (#532).
            if winit_window.fullscreen().is_none() {
                winit_window.set_inner_size(winit::dpi::LogicalSize::new(width, height));
            }
        }

        if must_resize {
            self.window().set_size(i_slint_core::api::LogicalSize::new(width, height));
        }
    }

    fn apply_geometry_constraint(
        &self,
        constraints_horizontal: corelib::layout::LayoutInfo,
        constraints_vertical: corelib::layout::LayoutInfo,
    ) {
        self.with_window_handle(&mut |winit_window| {
            // If we're in fullscreen state, don't try to resize the window but maintain the surface
            // size we've been assigned to from the windowing system. Weston/Wayland don't like it
            // when we create a surface that's bigger than the screen due to constraints (#532).
            if winit_window.fullscreen().is_some() {
                return;
            }

            let new_constraints = (constraints_horizontal, constraints_vertical);

            if new_constraints == self.constraints.get() {
                return;
            }

            self.constraints.set(new_constraints);

            // Use our scale factor instead of winit's logical size to take a scale factor override into account.
            let sf = self.window().scale_factor();

            let into_size = |s: LogicalSize| -> winit::dpi::PhysicalSize<f32> {
                winit::dpi::LogicalSize::new(s.width, s.height).to_physical(sf as f64)
            };

            let (min_size, max_size) = i_slint_core::layout::min_max_size_for_layout_constraints(
                constraints_horizontal,
                constraints_vertical,
            );

            let resizable = window_is_resizable(min_size, max_size);

            let winit_min_inner = min_size.map(into_size);
            winit_window.set_min_inner_size(winit_min_inner);
            let winit_max_inner = max_size.map(into_size);
            winit_window.set_max_inner_size(winit_max_inner);
            winit_window.set_resizable(resizable);

            adjust_window_size_to_satisfy_constraints(
                winit_window,
                winit_min_inner,
                winit_max_inner,
            );

            #[cfg(target_arch = "wasm32")]
            if let Some((
                LogicalSize { width: min_width, height: min_height, .. },
                LogicalSize { width: max_width, height: max_height, .. },
            )) = min_size.zip(max_size)
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
                let canvas = self.renderer().html_canvas_element();

                if canvas
                    .dataset()
                    .get("slintAutoResizeToPreferred")
                    .and_then(|val_str| val_str.parse().ok())
                    .unwrap_or_default()
                {
                    let pref_width = constraints_horizontal.preferred_bounded();
                    let pref_height = constraints_vertical.preferred_bounded();
                    if pref_width > 0 as Coord || pref_height > 0 as Coord {
                        winit_window
                            .set_inner_size(winit::dpi::LogicalSize::new(pref_width, pref_height));
                    };
                }
            }
        });
    }

    fn show(&self) -> Result<(), PlatformError> {
        self.call_with_event_loop(|self_| {
            self_.shown.set(true);

            let winit_window = self_.renderer().window();

            let runtime_window = WindowInner::from_pub(&self_.window());

            let scale_factor_override = runtime_window.scale_factor();
            // If the scale factor was already set programmatically, use that
            // else, use the SLINT_SCALE_FACTOR if set, otherwise use the one from winit
            let scale_factor_override = if scale_factor_override > 1. {
                Some(scale_factor_override as f64)
            } else {
                std::env::var("SLINT_SCALE_FACTOR")
                    .ok()
                    .and_then(|x| x.parse::<f64>().ok())
                    .filter(|f| *f > 0.)
            };
            let scale_factor = scale_factor_override.unwrap_or_else(|| winit_window.scale_factor());
            WindowInner::from_pub(&self_.window()).set_scale_factor(scale_factor as _);

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
                let html_canvas = self_.renderer().html_canvas_element();
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

            if std::env::var("SLINT_FULLSCREEN").is_err() {
                if preferred_size.width > 0 as Coord && preferred_size.height > 0 as Coord {
                    let into_size = |s: winit::dpi::LogicalSize<Coord>| -> winit::dpi::Size {
                        if let Some(f) = scale_factor_override {
                            s.to_physical::<f32>(f).into()
                        } else {
                            s.into()
                        }
                    };
                    winit_window.set_inner_size(into_size(preferred_size));
                }
            };

            self_.renderer().show()?;
            winit_window.set_visible(true);

            let s = winit_window.inner_size().to_logical(scale_factor);
            // Make sure that the window's inner size is in sync with the root window item's
            // width/height.
            runtime_window.set_window_item_geometry(LogicalSize::new(s.width, s.height));

            // Make sure the dark color scheme property is up-to-date, as it may have been queried earlier when
            // the window wasn't mapped yet.
            if let Some(dark_color_scheme_prop) = self_.dark_color_scheme.get() {
                if let Some(theme) = winit_window.theme() {
                    dark_color_scheme_prop.as_ref().set(theme == winit::window::Theme::Dark)
                }
            }

            Ok(())
        })
    }

    fn hide(&self) -> Result<(), PlatformError> {
        self.call_with_event_loop(|self_| {
            self_.shown.set(false);

            self_.unmap()?;

            /* FIXME:
            if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
                existing_blinker.stop();
            }*/
            crate::event_loop::with_window_target(|event_loop| {
                event_loop.event_loop_proxy().send_event(crate::SlintUserEvent::CustomEvent {
                    event: crate::event_loop::CustomEvent::WindowHidden,
                })
            })
            .ok(); // It's okay to call hide() even after the event loop is closed. We don't need the logic for quitting the event loop anymore at this point.
            Ok(())
        })
    }

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

    fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        self.renderer().as_core_renderer()
    }

    fn enable_input_method(&self, _it: corelib::items::InputType) {
        #[cfg(target_arch = "wasm32")]
        {
            let mut vkh = self.virtual_keyboard_helper.borrow_mut();
            let h = vkh.get_or_insert_with(|| {
                let canvas = self.renderer().html_canvas_element();
                super::wasm_input_helper::WasmInputHelper::new(self.self_weak.clone(), canvas)
            });
            h.show();
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.with_window_handle(&mut |winit_window| {
            winit_window.set_ime_allowed(matches!(_it, corelib::items::InputType::Text))
        });
    }

    fn disable_input_method(&self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(h) = &*self.virtual_keyboard_helper.borrow() {
            h.hide()
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.with_window_handle(&mut |winit_window| winit_window.set_ime_allowed(false));
    }

    fn set_ime_position(&self, ime_pos: LogicalPoint) {
        self.with_window_handle(&mut |winit_window| {
            winit_window.set_ime_position(winit::dpi::LogicalPosition::new(ime_pos.x, ime_pos.y))
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn position(&self) -> corelib::api::PhysicalPosition {
        match self.renderer().window().outer_position() {
            Ok(outer_position) => {
                corelib::api::PhysicalPosition::new(outer_position.x, outer_position.y)
            }
            Err(_) => Default::default(),
        }
    }

    fn set_position(&self, position: corelib::api::WindowPosition) {
        self.renderer().window().set_outer_position(position_to_winit(&position))
    }

    fn set_size(&self, size: corelib::api::WindowSize) {
        if self.in_resize_event.get() {
            return;
        }
        self.renderer().window().set_inner_size(window_size_to_slint(&size))
    }

    fn dark_color_scheme(&self) -> bool {
        self.dark_color_scheme
            .get_or_init(|| {
                Box::pin(Property::new({
                    self.renderer()
                        .window()
                        .theme()
                        .map_or(false, |theme| theme == winit::window::Theme::Dark)
                }))
            })
            .as_ref()
            .get()
    }

    fn is_visible(&self) -> bool {
        self.renderer().window().is_visible().unwrap_or(true)
    }
}

impl<Renderer: WinitCompatibleRenderer + 'static> Drop for WinitWindowAdapter<Renderer> {
    fn drop(&mut self) {
        if let Some(renderer) = self.renderer.get() {
            renderer.hide().ok(); // ignore errors here, we're going away anyway
            crate::event_loop::unregister_window(renderer.window().id());
        }
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
