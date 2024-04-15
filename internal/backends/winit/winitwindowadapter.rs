// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

//! This module contains the GraphicsWindow that used to be within corelib.

// cspell:ignore accesskit borderless corelib nesw webgl winit winsys xlib

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

use corelib::item_tree::ItemTreeRc;
#[cfg(enable_accesskit)]
use corelib::item_tree::ItemTreeRef;
use corelib::items::{ColorScheme, MouseCursor};
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
use winit::window::WindowBuilder;

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

fn window_size_to_winit(size: &corelib::api::WindowSize) -> winit::dpi::Size {
    match size {
        corelib::api::WindowSize::Logical(size) => {
            winit::dpi::Size::new(winit::dpi::LogicalSize::new(size.width, size.height))
        }
        corelib::api::WindowSize::Physical(size) => {
            winit::dpi::Size::new(winit::dpi::PhysicalSize::new(size.width, size.height))
        }
    }
}

pub fn physical_size_to_slint(size: &winit::dpi::PhysicalSize<u32>) -> corelib::api::PhysicalSize {
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
    pending_redraw: Cell<bool>,
    color_scheme: OnceCell<Pin<Box<Property<ColorScheme>>>>,
    constraints: Cell<corelib::window::LayoutConstraints>,
    shown: Cell<bool>,
    window_level: Cell<winit::window::WindowLevel>,
    maximized: Cell<bool>,
    minimized: Cell<bool>,
    fullscreen: Cell<bool>,

    pub(crate) renderer: Box<dyn WinitCompatibleRenderer>,
    /// We cache the size because winit_window.inner_size() can return different value between calls (eg, on X11)
    /// And we wan see the newer value before the Resized event was received, leading to inconsistencies
    size: Cell<PhysicalSize>,

    /// Whether the size has been set explicitly via `set_size`
    has_explicit_size: Cell<bool>,

    #[cfg(target_arch = "wasm32")]
    virtual_keyboard_helper: RefCell<Option<super::wasm_input_helper::WasmInputHelper>>,

    #[cfg(enable_accesskit)]
    pub accesskit_adapter: crate::accesskit::AccessKitAdapter,

    winit_window: Rc<winit::window::Window>, // Last field so that any previously provided window handles are still valid in the drop impl of the renderers, etc.
}

impl WinitWindowAdapter {
    /// Creates a new reference-counted instance.
    pub(crate) fn new(
        renderer: Box<dyn WinitCompatibleRenderer>,
        winit_window: Rc<winit::window::Window>,
    ) -> Rc<Self> {
        let self_rc = Rc::new_cyclic(|self_weak| Self {
            window: OnceCell::with_value(corelib::api::Window::new(self_weak.clone() as _)),
            #[cfg(target_arch = "wasm32")]
            self_weak: self_weak.clone(),
            pending_redraw: Default::default(),
            color_scheme: Default::default(),
            constraints: Default::default(),
            shown: Default::default(),
            window_level: Default::default(),
            maximized: Cell::default(),
            minimized: Cell::default(),
            fullscreen: Cell::default(),
            winit_window: winit_window.clone(),
            size: Default::default(),
            has_explicit_size: Default::default(),
            renderer,
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

        self_rc
    }

    fn renderer(&self) -> &dyn WinitCompatibleRenderer {
        self.renderer.as_ref()
    }

    pub(crate) fn window_builder(
        #[cfg(target_arch = "wasm32")] canvas_id: &str,
    ) -> Result<WindowBuilder, PlatformError> {
        let mut window_builder = WindowBuilder::new().with_transparent(true).with_visible(false);

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
            window_builder = window_builder
                .with_canvas(Some(html_canvas))
                // Don't activate the window by default, as that will cause the page to scroll,
                // ignoring any existing anchors.
                .with_active(false)
        };

        Ok(window_builder)
    }

    /// Draw the items of the specified `component` in the given window.
    pub fn draw(&self) -> Result<(), PlatformError> {
        if !self.shown.get() {
            return Ok(()); // caller bug, doesn't make sense to call draw() when not shown
        }

        self.pending_redraw.set(false);

        let renderer = self.renderer();
        renderer.render(self.window())?;

        Ok(())
    }

    fn with_window_handle(&self, callback: &mut dyn FnMut(&winit::window::Window)) {
        callback(&self.winit_window());
    }

    pub fn winit_window(&self) -> Rc<winit::window::Window> {
        self.winit_window.clone()
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

    // Requests for the window to be resized. Returns true if the window was resized immediately,
    // or if it will be resized later (false).
    fn resize_window(&self, size: winit::dpi::Size) -> Result<bool, PlatformError> {
        if let Some(size) = self.winit_window().request_inner_size(size) {
            // On wayland we might not get a WindowEvent::Resized, so resize the EGL surface right away.
            self.resize_event(size)?;
            Ok(true)
        } else {
            // None means that we'll get a `WindowEvent::Resized` later
            Ok(false)
        }
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

            // Workaround fox winit not sync'ing CSS size of the canvas (the size shown on the browser)
            // with the width/height attribute (the size of the viewport/GL surface)
            // If they're not in sync, the UI would be shown as scaled
            #[cfg(target_arch = "wasm32")]
            if let Some(html_canvas) = self.winit_window.canvas() {
                html_canvas.set_width(physical_size.width);
                html_canvas.set_height(physical_size.height);
            }
        }
        Ok(())
    }

    pub fn set_color_scheme(&self, scheme: ColorScheme) {
        self.color_scheme
            .get_or_init(|| Box::pin(Property::new(ColorScheme::Unknown)))
            .as_ref()
            .set(scheme)
    }

    pub fn window_state_event(&self) {
        if let Some(minimized) = self.winit_window.is_minimized() {
            self.minimized.set(minimized);
            if minimized != self.window().is_minimized() {
                self.window().set_minimized(minimized);
            }
        }

        // The method winit::Window::is_maximized returns false when the window
        // is minimized, even if it was previously maximized. We have to ensure
        // that we only update the internal maximized state when the window is
        // not minimized. Otherwise, the window would be restored in a
        // non-maximized state even if it was maximized before being minimized.
        let maximized = self.winit_window.is_maximized();
        if !self.window().is_minimized() {
            self.maximized.set(maximized);
            if maximized != self.window().is_maximized() {
                self.window().set_maximized(maximized);
            }
        }

        // NOTE: Fullscreen overrides maximized so if both are true then the
        // window will remain in fullscreen. Fullscreen must be false to switch
        // to maximized.
        let fullscreen = self.winit_window.fullscreen().is_some();
        if fullscreen != self.window().is_fullscreen() {
            self.window().set_fullscreen(fullscreen);
        }
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
        if visible == self.shown.get() {
            return Ok(());
        }

        self.shown.set(visible);
        if visible {
            let winit_window = self.winit_window();

            let runtime_window = WindowInner::from_pub(self.window());

            let scale_factor = runtime_window.scale_factor() as f64;

            let component_rc = runtime_window.component();
            let component = ItemTreeRc::borrow_pin(&component_rc);

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
            if let Some(html_canvas) = winit_window.canvas() {
                let existing_canvas_size = winit::dpi::LogicalSize::new(
                    html_canvas.client_width() as f32,
                    html_canvas.client_height() as f32,
                );
                // Try to maintain the existing size of the canvas element, if any
                if existing_canvas_size.width > 0. {
                    preferred_size.width = existing_canvas_size.width;
                }
                if existing_canvas_size.height > 0. {
                    preferred_size.height = existing_canvas_size.height;
                }
            }

            if winit_window.fullscreen().is_none()
                && !self.has_explicit_size.get()
                && preferred_size.width > 0 as Coord
                && preferred_size.height > 0 as Coord
            {
                // use the Slint's window Scale factor to take in account the override
                let size = preferred_size.to_physical::<u32>(scale_factor);
                self.resize_window(size.into())?;
            };

            winit_window.set_visible(true);

            // Make sure the dark color scheme property is up-to-date, as it may have been queried earlier when
            // the window wasn't mapped yet.
            if let Some(color_scheme_prop) = self.color_scheme.get() {
                if let Some(theme) = winit_window.theme() {
                    color_scheme_prop.as_ref().set(match theme {
                        winit::window::Theme::Dark => ColorScheme::Dark,
                        winit::window::Theme::Light => ColorScheme::Light,
                    })
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
        self.has_explicit_size.set(true);
        // TODO: don't ignore error, propgate to caller
        self.resize_window(window_size_to_winit(&size)).ok();
    }

    fn size(&self) -> corelib::api::PhysicalSize {
        self.size.get()
    }

    fn request_redraw(&self) {
        if !self.pending_redraw.replace(true) {
            self.winit_window.request_redraw()
        }
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
                // TODO: don't ignore error, propgate to caller
                let immediately_resized = self
                    .resize_window(winit::dpi::LogicalSize::new(width, height).into())
                    .unwrap_or_default();
                if immediately_resized {
                    // The resize event was already dispatched
                    must_resize = false;
                }
            }
        }

        if must_resize {
            self.window().dispatch_event(WindowEvent::Resized {
                size: i_slint_core::api::LogicalSize::new(width, height),
            });
        }

        self.with_window_handle(&mut |winit_window| {
            let m = properties.is_fullscreen();
            if m != self.fullscreen.get() {
                if m {
                    if winit_window.fullscreen().is_none() {
                        winit_window
                            .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                    }
                } else {
                    winit_window.set_fullscreen(None);
                }
            }

            let m = properties.is_maximized();
            if m != self.maximized.get() {
                self.maximized.set(m);
                winit_window.set_maximized(m);
            }

            let m = properties.is_minimized();
            if m != self.minimized.get() {
                self.minimized.set(m);
                winit_window.set_minimized(m);
            }

            // If we're in fullscreen, don't try to resize the window but
            // maintain the surface size we've been assigned to from the
            // windowing system. Weston/Wayland don't like it when we create a
            // surface that's bigger than the screen due to constraints (#532).
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
            // we must call set_resizable before setting the min and max size otherwise setting the min and max size don't work on X11
            winit_window.set_resizable(resizable);
            let winit_min_inner = new_constraints.min.map(into_size);
            winit_window.set_min_inner_size(winit_min_inner);
            let winit_max_inner = new_constraints.max.map(into_size);
            winit_window.set_max_inner_size(winit_max_inner);

            adjust_window_size_to_satisfy_constraints(self, winit_min_inner, winit_max_inner);

            // Auto-resize to the preferred size if users (SlintPad) requests it
            #[cfg(target_arch = "wasm32")]
            if let Some(canvas) = winit_window.canvas() {
                if canvas
                    .dataset()
                    .get("slintAutoResizeToPreferred")
                    .and_then(|val_str| val_str.parse().ok())
                    .unwrap_or_default()
                {
                    let pref_width = new_constraints.preferred.width;
                    let pref_height = new_constraints.preferred.height;
                    if pref_width > 0 as Coord || pref_height > 0 as Coord {
                        // TODO: don't ignore error, propgate to caller
                        self.resize_window(
                            winit::dpi::LogicalSize::new(pref_width, pref_height).into(),
                        )
                        .ok();
                    };
                }
            }
        });
    }

    fn internal(&self, _: corelib::InternalToken) -> Option<&dyn WindowAdapterInternal> {
        Some(self)
    }

    #[cfg(feature = "raw-window-handle-06")]
    fn window_handle_06(
        &self,
    ) -> Result<raw_window_handle_06::WindowHandle<'_>, raw_window_handle_06::HandleError> {
        raw_window_handle_06::HasWindowHandle::window_handle(&self.winit_window)
    }

    #[cfg(feature = "raw-window-handle-06")]
    fn display_handle_06(
        &self,
    ) -> Result<raw_window_handle_06::DisplayHandle<'_>, raw_window_handle_06::HandleError> {
        raw_window_handle_06::HasDisplayHandle::display_handle(&self.winit_window)
    }
}

impl WindowAdapterInternal for WinitWindowAdapter {
    fn set_mouse_cursor(&self, cursor: MouseCursor) {
        let winit_cursor = match cursor {
            MouseCursor::Default => winit::window::CursorIcon::Default,
            MouseCursor::None => winit::window::CursorIcon::Default,
            MouseCursor::Help => winit::window::CursorIcon::Help,
            MouseCursor::Pointer => winit::window::CursorIcon::Pointer,
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
            let props = match &request {
                corelib::window::InputMethodRequest::Enable(props) => {
                    winit_window.set_ime_allowed(true);
                    props
                }
                corelib::window::InputMethodRequest::Disable => {
                    return winit_window.set_ime_allowed(false)
                }
                corelib::window::InputMethodRequest::Update(props) => props,
                _ => return,
            };
            winit_window.set_ime_purpose(match props.input_type {
                corelib::items::InputType::Password => winit::window::ImePurpose::Password,
                _ => winit::window::ImePurpose::Normal,
            });
            winit_window.set_ime_cursor_area(
                position_to_winit(&props.cursor_rect_origin.into()),
                window_size_to_winit(&props.cursor_rect_size.into()),
            );
        });

        #[cfg(target_arch = "wasm32")]
        match request {
            corelib::window::InputMethodRequest::Enable(..) => {
                let mut vkh = self.virtual_keyboard_helper.borrow_mut();
                let Some(canvas) = self.winit_window().canvas() else { return };
                let h = vkh.get_or_insert_with(|| {
                    super::wasm_input_helper::WasmInputHelper::new(self.self_weak.clone(), canvas)
                });
                h.show();
            }
            corelib::window::InputMethodRequest::Disable => {
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

    fn color_scheme(&self) -> ColorScheme {
        self.color_scheme
            .get_or_init(|| {
                Box::pin(Property::new({
                    self.winit_window().theme().map_or(ColorScheme::Unknown, |theme| match theme {
                        winit::window::Theme::Dark => ColorScheme::Dark,
                        winit::window::Theme::Light => ColorScheme::Light,
                    })
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
    fn register_item_tree(&self) {
        self.accesskit_adapter.register_item_tree();
    }

    #[cfg(enable_accesskit)]
    fn unregister_item_tree(
        &self,
        _component: ItemTreeRef,
        _: &mut dyn Iterator<Item = Pin<ItemRef<'_>>>,
    ) {
        self.accesskit_adapter.unregister_item_tree(_component);
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
    winit_window: &WinitWindowAdapter,
    min_size: Option<winit::dpi::PhysicalSize<f32>>,
    max_size: Option<winit::dpi::PhysicalSize<f32>>,
) {
    let mut window_size = winit_window.size();

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

    if window_size != winit_window.size() {
        // TODO: don't ignore error, propgate to caller
        winit_window
            .resize_window(
                winit::dpi::PhysicalSize::new(window_size.width, window_size.height).into(),
            )
            .ok();
    }
}
