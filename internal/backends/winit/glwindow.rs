// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains the GraphicsWindow that used to be within corelib.

// cspell:ignore borderless corelib nesw webgl winit winsys xlib

use core::cell::{Cell, RefCell};
use core::pin::Pin;
use std::rc::{Rc, Weak};

use crate::event_loop::WinitWindow;
use crate::renderer::{WinitCompatibleCanvas, WinitCompatibleRenderer};
use const_field_offset::FieldOffsets;
use corelib::component::ComponentRc;
use corelib::graphics::euclid::num::Zero;
use corelib::input::KeyboardModifiers;
use corelib::items::{ItemRef, MouseCursor};
use corelib::layout::Orientation;
use corelib::lengths::{LogicalLength, LogicalPoint, LogicalSize};
use corelib::window::{WindowAdapter, WindowAdapterSealed, WindowInner};
use corelib::Property;
use corelib::{graphics::*, Coord};
use i_slint_core as corelib;

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

fn size_to_winit(pos: &corelib::api::WindowSize) -> winit::dpi::Size {
    match pos {
        corelib::api::WindowSize::Logical(size) => {
            winit::dpi::Size::new(winit::dpi::LogicalSize::new(size.width, size.height))
        }
        corelib::api::WindowSize::Physical(size) => {
            winit::dpi::Size::new(winit::dpi::PhysicalSize::new(size.width, size.height))
        }
    }
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

/// GraphicsWindow is an implementation of the [WindowAdapter][`crate::eventloop::WindowAdapter`] trait. This is
/// typically instantiated by entry factory functions of the different graphics back ends.
pub(crate) struct GLWindow<Renderer: WinitCompatibleRenderer + 'static> {
    window: corelib::api::Window,
    self_weak: Weak<Self>,
    map_state: RefCell<GraphicsWindowBackendState<Renderer>>,
    keyboard_modifiers: std::cell::Cell<KeyboardModifiers>,
    currently_pressed_key_code: std::cell::Cell<Option<winit::event::VirtualKeyCode>>,
    pending_redraw: Cell<bool>,

    renderer: Renderer,

    #[cfg(target_arch = "wasm32")]
    virtual_keyboard_helper: RefCell<Option<super::wasm_input_helper::WasmInputHelper>>,
}

impl<Renderer: WinitCompatibleRenderer + 'static> GLWindow<Renderer> {
    /// Creates a new reference-counted instance.
    ///
    /// Arguments:
    /// * `graphics_backend_factory`: The factor function stored in the GraphicsWindow that's called when the state
    ///   of the window changes to mapped. The event loop and window builder parameters can be used to create a
    ///   backing window.
    pub(crate) fn new(#[cfg(target_arch = "wasm32")] canvas_id: String) -> Rc<dyn WindowAdapter> {
        let self_rc = Rc::new_cyclic(|self_weak| Self {
            window: corelib::api::Window::new(self_weak.clone() as _),
            self_weak: self_weak.clone(),
            map_state: RefCell::new(GraphicsWindowBackendState::Unmapped {
                requested_position: None,
                requested_size: None,
            }),
            keyboard_modifiers: Default::default(),
            currently_pressed_key_code: Default::default(),
            pending_redraw: Cell::new(false),
            renderer: Renderer::new(
                &(self_weak.clone() as _),
                #[cfg(target_arch = "wasm32")]
                canvas_id,
            ),
            #[cfg(target_arch = "wasm32")]
            virtual_keyboard_helper: Default::default(),
        });
        self_rc as _
    }

    fn is_mapped(&self) -> bool {
        matches!(&*self.map_state.borrow(), GraphicsWindowBackendState::Mapped { .. })
    }

    fn borrow_mapped_window(&self) -> Option<std::cell::Ref<MappedWindow<Renderer>>> {
        if self.is_mapped() {
            std::cell::Ref::map(self.map_state.borrow(), |state| match state {
                GraphicsWindowBackendState::Unmapped{..} => {
                    panic!("borrow_mapped_window must be called after checking if the window is mapped")
                }
                GraphicsWindowBackendState::Mapped(window) => window,
            }).into()
        } else {
            None
        }
    }

    fn unmap(&self) {
        let old_mapped = match self.map_state.replace(GraphicsWindowBackendState::Unmapped {
            requested_position: None,
            requested_size: None,
        }) {
            GraphicsWindowBackendState::Unmapped { .. } => return,
            GraphicsWindowBackendState::Mapped(old_mapped) => old_mapped,
        };

        old_mapped.canvas.with_window_handle(|winit_window| {
            crate::event_loop::unregister_window(winit_window.id());
        });

        self.renderer.release_canvas(old_mapped.canvas);
    }

    fn call_with_event_loop(&self, callback: fn(&Self)) {
        // With wasm, winit's `run()` consumes the event loop and access to it from outside the event handler yields
        // loop and thus ends up trying to create a new event loop instance, which panics in winit. Instead, forward
        // the call to be invoked from within the event loop
        #[cfg(target_arch = "wasm32")]
        corelib::api::invoke_from_event_loop({
            let self_weak = send_wrapper::SendWrapper::new(self.self_weak.clone());

            move || {
                if let Some(this) = self_weak.take().upgrade() {
                    callback(&this)
                }
            }
        })
        .unwrap();
        #[cfg(not(target_arch = "wasm32"))]
        callback(self)
    }
}

impl<Renderer: WinitCompatibleRenderer + 'static> WinitWindow for GLWindow<Renderer> {
    fn take_pending_redraw(&self) -> bool {
        self.pending_redraw.take()
    }

    fn currently_pressed_key_code(&self) -> &Cell<Option<winit::event::VirtualKeyCode>> {
        &self.currently_pressed_key_code
    }

    fn current_keyboard_modifiers(&self) -> &Cell<KeyboardModifiers> {
        &self.keyboard_modifiers
    }

    /// Draw the items of the specified `component` in the given window.
    fn draw(&self) -> bool {
        let window = match self.borrow_mapped_window() {
            Some(window) => window,
            None => return false, // caller bug, doesn't make sense to call draw() when not mapped
        };

        self.pending_redraw.set(false);
        self.renderer.render(&window.canvas, self);

        self.pending_redraw.get()
    }

    fn with_window_handle(&self, callback: &mut dyn FnMut(&winit::window::Window)) {
        if let Some(mapped_window) = self.borrow_mapped_window() {
            mapped_window.canvas.with_window_handle(callback);
        }
    }

    fn constraints(&self) -> (corelib::layout::LayoutInfo, corelib::layout::LayoutInfo) {
        self.borrow_mapped_window().map(|window| window.constraints.get()).unwrap_or_default()
    }

    fn set_constraints(
        &self,
        constraints: (corelib::layout::LayoutInfo, corelib::layout::LayoutInfo),
    ) {
        if let Some(window) = self.borrow_mapped_window() {
            window.constraints.set(constraints);
        }
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

    fn resize_event(&self, size: winit::dpi::PhysicalSize<u32>) {
        if let Some(mapped_window) = self.borrow_mapped_window() {
            self.window().set_size(corelib::api::PhysicalSize::new(size.width, size.height));
            mapped_window.canvas.resize_event()
        }
    }
}

impl<Renderer: WinitCompatibleRenderer + 'static> WindowAdapter for GLWindow<Renderer> {
    fn window(&self) -> &corelib::api::Window {
        &self.window
    }
}

impl<Renderer: WinitCompatibleRenderer + 'static> WindowAdapterSealed for GLWindow<Renderer> {
    fn request_redraw(&self) {
        self.pending_redraw.set(true);
        self.with_window_handle(&mut |window| window.request_redraw())
    }

    fn unregister_component<'a>(
        &self,
        component: corelib::component::ComponentRef,
        _items: &mut dyn Iterator<Item = Pin<ItemRef<'a>>>,
    ) {
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped { .. } => {}
            GraphicsWindowBackendState::Mapped(mapped_window) => {
                mapped_window.canvas.component_destroyed(component)
            }
        }
    }

    fn request_window_properties_update(&self) {
        self.call_with_event_loop(|self_| {
            self_.with_window_handle(&mut |window| {
                let window_id = window.id();
                crate::event_loop::with_window_target(|event_loop| {
                    event_loop.event_loop_proxy().send_event(
                        crate::event_loop::CustomEvent::UpdateWindowProperties(window_id),
                    )
                })
                .ok();
            })
        });
    }

    fn apply_window_properties(&self, window_item: Pin<&i_slint_core::items::WindowItem>) {
        // Make the unwrap() calls on self.borrow_mapped_window*() safe
        if !self.is_mapped() {
            return;
        }

        let mut width = window_item.width().get() as f32;
        let mut height = window_item.height().get() as f32;

        let mut must_resize = false;

        self.with_window_handle(&mut |winit_window| {
            winit_window.set_window_icon(icon_to_winit(window_item.icon()));
            winit_window.set_title(&window_item.title());
            winit_window
                .set_decorations(!window_item.no_frame() || winit_window.fullscreen().is_some());

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

            if (existing_size.width - width).abs() > 1.
                || (existing_size.height - height).abs() > 1.
            {
                // If we're in fullscreen state, don't try to resize the window but maintain the surface
                // size we've been assigned to from the windowing system. Weston/Wayland don't like it
                // when we create a surface that's bigger than the screen due to constraints (#532).
                if winit_window.fullscreen().is_none() {
                    winit_window.set_inner_size(winit::dpi::LogicalSize::new(width, height));
                }
            }
        });

        if must_resize {
            let win = self.window();

            win.set_size(i_slint_core::api::LogicalSize::new(width, height));
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

            if (constraints_horizontal, constraints_vertical) != self.constraints() {
                let min_width = constraints_horizontal.min.min(constraints_horizontal.max) as f32;
                let min_height = constraints_vertical.min.min(constraints_vertical.max) as f32;
                let max_width = constraints_horizontal.max.max(constraints_horizontal.min) as f32;
                let max_height = constraints_vertical.max.max(constraints_vertical.min) as f32;

                let sf = self.window().scale_factor();

                winit_window.set_resizable(true);
                winit_window.set_min_inner_size(if min_width > 0. || min_height > 0. {
                    Some(winit::dpi::PhysicalSize::new(min_width * sf, min_height * sf))
                } else {
                    None
                });
                winit_window.set_max_inner_size(
                    if max_width < i32::MAX as f32 || max_height < i32::MAX as f32 {
                        Some(winit::dpi::PhysicalSize::new(
                            (max_width * sf).min(65535.),
                            (max_height * sf).min(65535.),
                        ))
                    } else {
                        None
                    },
                );
                self.set_constraints((constraints_horizontal, constraints_vertical));
                winit_window.set_resizable(min_width < max_width || min_height < max_height);

                #[cfg(target_arch = "wasm32")]
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
            }
        });
    }

    fn show(&self) {
        self.call_with_event_loop(|self_| {
            let (requested_position, requested_size) = match &*self_.map_state.borrow() {
                GraphicsWindowBackendState::Unmapped { requested_position, requested_size } => {
                    (requested_position.clone(), requested_size.clone())
                }
                GraphicsWindowBackendState::Mapped(_) => return,
            };

            let mut window_builder = winit::window::WindowBuilder::new();

            let runtime_window = WindowInner::from_pub(self_.window());
            let component_rc = runtime_window.component();
            let component = ComponentRc::borrow_pin(&component_rc);

            window_builder = if let Some(window_item) =
                runtime_window.window_item().as_ref().map(|i| i.as_pin_ref())
            {
                window_builder
                    .with_title(window_item.title().to_string())
                    .with_resizable(
                        window_item.height() <= LogicalLength::zero()
                            || window_item.width() <= LogicalLength::zero(),
                    )
                    .with_decorations(!window_item.no_frame())
                    .with_window_icon(icon_to_winit(window_item.icon()))
            } else {
                window_builder.with_title("Slint Window".to_string())
            };

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

            let into_size = |s: winit::dpi::LogicalSize<f32>| -> winit::dpi::Size {
                if let Some(f) = scale_factor_override {
                    s.to_physical::<f32>(f).into()
                } else {
                    s.into()
                }
            };

            let layout_info_h = component.as_ref().layout_info(Orientation::Horizontal);
            if let Some(window_item) = runtime_window.window_item() {
                // Setting the width to its preferred size before querying the vertical layout info
                // is important in case the height depends on the width
                window_item.width.set(layout_info_h.preferred_bounded());
            }
            let layout_info_v = component.as_ref().layout_info(Orientation::Vertical);
            let s = winit::dpi::LogicalSize::new(
                layout_info_h.preferred_bounded(),
                layout_info_v.preferred_bounded(),
            );

            let window_builder = if std::env::var("SLINT_FULLSCREEN").is_ok() {
                window_builder.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
            } else {
                if layout_info_h.min >= 1. || layout_info_v.min >= 1. {
                    window_builder = window_builder.with_min_inner_size(into_size(
                        winit::dpi::LogicalSize::new(layout_info_h.min, layout_info_v.min),
                    ))
                }
                if layout_info_h.max < f32::MAX || layout_info_v.max < f32::MAX {
                    window_builder = window_builder.with_max_inner_size(into_size(
                        winit::dpi::LogicalSize::new(layout_info_h.max, layout_info_v.max),
                    ))
                }

                if let Some(requested_size) = &requested_size {
                    // It would be nice to bound this with our constraints, but those are in logical coordinates
                    // and we don't know the scale factor yet...
                    if let Some(sf) = scale_factor_override {
                        let physical_size = requested_size.to_physical(sf as f32);
                        window_builder.with_inner_size(winit::dpi::Size::new(
                            winit::dpi::PhysicalSize::new(
                                physical_size.width,
                                physical_size.height,
                            ),
                        ))
                    } else {
                        window_builder.with_inner_size(size_to_winit(requested_size))
                    }
                } else if s.width > 0 as Coord && s.height > 0 as Coord {
                    // Make sure that the window's inner size is in sync with the root window item's
                    // width/height.
                    runtime_window.set_window_item_geometry(LogicalSize::new(s.width, s.height));
                    window_builder.with_inner_size(into_size(s))
                } else {
                    window_builder
                }
            };

            let window_builder = if let Some(pos) = &requested_position {
                window_builder.with_position(position_to_winit(pos))
            } else {
                window_builder
            };

            let canvas = self_.renderer.create_canvas(window_builder);

            let id = canvas.with_window_handle(|winit_window| {
                WindowInner::from_pub(&self_.window).set_scale_factor(
                    scale_factor_override.unwrap_or_else(|| winit_window.scale_factor()) as _,
                );
                // On wasm, with_inner_size on the WindowBuilder don't have effect, so apply manually
                #[cfg(target_arch = "wasm32")]
                if s.width > 0 as Coord && s.height > 0 as Coord {
                    winit_window.set_inner_size(s);
                }
                winit_window.id()
            });

            self_.map_state.replace(GraphicsWindowBackendState::Mapped(MappedWindow {
                canvas,
                constraints: Default::default(),
            }));

            crate::event_loop::register_window(id, self_.self_weak.upgrade().unwrap());
        });
    }

    fn hide(&self) {
        self.call_with_event_loop(|self_| {
            self_.unmap();

            /* FIXME:
            if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
                existing_blinker.stop();
            }*/
            crate::event_loop::with_window_target(|event_loop| {
                event_loop
                    .event_loop_proxy()
                    .send_event(crate::event_loop::CustomEvent::WindowHidden)
            })
            .unwrap();
        });
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
        &self.renderer
    }

    fn enable_input_method(&self, _it: corelib::items::InputType) {
        #[cfg(target_arch = "wasm32")]
        {
            let mut vkh = self.virtual_keyboard_helper.borrow_mut();
            let h = vkh.get_or_insert_with(|| {
                let canvas =
                    self.borrow_mapped_window().unwrap().canvas.html_canvas_element().clone();
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
        match &*self.map_state.borrow() {
            GraphicsWindowBackendState::Unmapped { requested_position, .. } => requested_position
                .as_ref()
                .map(|p| p.to_physical(self.window().scale_factor()))
                .unwrap_or_default(),
            GraphicsWindowBackendState::Mapped(mapped_window) => mapped_window
                .canvas
                .with_window_handle(|winit_window| match winit_window.outer_position() {
                    Ok(outer_position) => {
                        corelib::api::PhysicalPosition::new(outer_position.x, outer_position.y)
                    }
                    Err(_) => Default::default(),
                }),
        }
    }

    fn set_position(&self, position: corelib::api::WindowPosition) {
        match &mut *self.map_state.borrow_mut() {
            GraphicsWindowBackendState::Unmapped { requested_position, .. } => {
                *requested_position = Some(position)
            }
            GraphicsWindowBackendState::Mapped(mapped_window) => {
                mapped_window.canvas.with_window_handle(|winit_window| {
                    winit_window.set_outer_position(position_to_winit(&position))
                })
            }
        }
    }

    fn set_size(&self, size: corelib::api::WindowSize) {
        if let Ok(mut map_state) = self.map_state.try_borrow_mut() {
            // otherwise we are called from the resize event
            match &mut *map_state {
                GraphicsWindowBackendState::Unmapped { requested_size, .. } => {
                    *requested_size = Some(size)
                }
                GraphicsWindowBackendState::Mapped(mapped_window) => {
                    mapped_window.canvas.with_window_handle(|winit_window| {
                        winit_window.set_inner_size(size_to_winit(&size));
                    });
                }
            }
        }
    }

    fn dark_style(&self) -> bool {
        dark_light::detect() == dark_light::Mode::Dark
    }
}

impl<Renderer: WinitCompatibleRenderer + 'static> Drop for GLWindow<Renderer> {
    fn drop(&mut self) {
        self.unmap();
    }
}

struct MappedWindow<Renderer: WinitCompatibleRenderer> {
    canvas: Renderer::Canvas,
    constraints: Cell<(corelib::layout::LayoutInfo, corelib::layout::LayoutInfo)>,
}

enum GraphicsWindowBackendState<Renderer: WinitCompatibleRenderer> {
    Unmapped {
        requested_position: Option<corelib::api::WindowPosition>,
        requested_size: Option<corelib::api::WindowSize>,
    },
    Mapped(MappedWindow<Renderer>),
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
