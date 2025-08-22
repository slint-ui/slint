// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains the GraphicsWindow that used to be within corelib.

// cspell:ignore accesskit borderless corelib nesw webgl winit winsys xlib

use core::cell::{Cell, RefCell};
use core::pin::Pin;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;

use euclid::approxeq::ApproxEq;

#[cfg(muda)]
use i_slint_core::api::LogicalPosition;
use i_slint_core::lengths::{PhysicalPx, ScaleFactor};
use winit::event_loop::ActiveEventLoop;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;
#[cfg(target_family = "windows")]
use winit::platform::windows::WindowExtWindows;

#[cfg(muda)]
use crate::muda::MudaType;
use crate::renderer::WinitCompatibleRenderer;

use corelib::item_tree::ItemTreeRc;
#[cfg(enable_accesskit)]
use corelib::item_tree::ItemTreeRef;
use corelib::items::{ColorScheme, MouseCursor};
#[cfg(enable_accesskit)]
use corelib::items::{ItemRc, ItemRef};

#[cfg(any(enable_accesskit, muda))]
use crate::SlintEvent;
use crate::{EventResult, SharedBackendData};
use corelib::api::PhysicalSize;
use corelib::layout::Orientation;
use corelib::lengths::LogicalLength;
use corelib::platform::{PlatformError, WindowEvent};
use corelib::window::{WindowAdapter, WindowAdapterInternal, WindowInner};
use corelib::Property;
use corelib::{graphics::*, Coord};
use i_slint_core::{self as corelib};
use std::cell::OnceCell;
#[cfg(any(enable_accesskit, muda))]
use winit::event_loop::EventLoopProxy;
use winit::window::{WindowAttributes, WindowButtons};

pub(crate) fn position_to_winit(pos: &corelib::api::WindowPosition) -> winit::dpi::Position {
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
            winit::dpi::Size::new(logical_size_to_winit(*size))
        }
        corelib::api::WindowSize::Physical(size) => {
            winit::dpi::Size::new(physical_size_to_winit(*size))
        }
    }
}

pub fn physical_size_to_slint(size: &winit::dpi::PhysicalSize<u32>) -> corelib::api::PhysicalSize {
    corelib::api::PhysicalSize::new(size.width, size.height)
}

fn logical_size_to_winit(s: i_slint_core::api::LogicalSize) -> winit::dpi::LogicalSize<f64> {
    winit::dpi::LogicalSize::new(s.width as f64, s.height as f64)
}

fn physical_size_to_winit(size: PhysicalSize) -> winit::dpi::PhysicalSize<u32> {
    winit::dpi::PhysicalSize::new(size.width, size.height)
}

fn filter_out_zero_width_or_height(
    size: winit::dpi::LogicalSize<f64>,
) -> winit::dpi::LogicalSize<f64> {
    fn filter(v: f64) -> f64 {
        if v.approx_eq(&0.) {
            // Some width or height is better than zero
            10.
        } else {
            v
        }
    }
    winit::dpi::LogicalSize { width: filter(size.width), height: filter(size.height) }
}

fn apply_scale_factor_to_logical_sizes_in_attributes(
    attributes: &mut WindowAttributes,
    scale_factor: f64,
) {
    let fixup = |maybe_size: &mut Option<winit::dpi::Size>| {
        if let Some(size) = maybe_size.as_mut() {
            *size = winit::dpi::Size::Physical(size.to_physical::<u32>(scale_factor))
        }
    };

    fixup(&mut attributes.inner_size);
    fixup(&mut attributes.min_inner_size);
    fixup(&mut attributes.max_inner_size);
    fixup(&mut attributes.resize_increments);
}

fn icon_to_winit(
    icon: corelib::graphics::Image,
    size: euclid::Size2D<Coord, PhysicalPx>,
) -> Option<winit::window::Icon> {
    let image_inner: &ImageInner = (&icon).into();

    let Some(pixel_buffer) = image_inner.render_to_buffer(Some(size.cast())) else {
        return None;
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

enum WinitWindowOrNone {
    HasWindow {
        window: Arc<winit::window::Window>,
        #[cfg(enable_accesskit)]
        accesskit_adapter: RefCell<crate::accesskit::AccessKitAdapter>,
        #[cfg(muda)]
        muda_adapter: RefCell<Option<crate::muda::MudaAdapter>>,
        #[cfg(muda)]
        context_menu_muda_adapter: RefCell<Option<crate::muda::MudaAdapter>>,
    },
    None(RefCell<WindowAttributes>),
}

impl WinitWindowOrNone {
    fn as_window(&self) -> Option<Arc<winit::window::Window>> {
        match self {
            Self::HasWindow { window, .. } => Some(window.clone()),
            Self::None { .. } => None,
        }
    }

    fn set_window_icon(&self, icon: Option<winit::window::Icon>) {
        match self {
            Self::HasWindow { window, .. } => {
                #[cfg(target_family = "windows")]
                window.set_taskbar_icon(icon.as_ref().cloned());
                window.set_window_icon(icon);
            }
            Self::None(attributes) => attributes.borrow_mut().window_icon = icon,
        }
    }

    fn set_title(&self, title: &str) {
        match self {
            Self::HasWindow { window, .. } => window.set_title(title),
            Self::None(attributes) => attributes.borrow_mut().title = title.into(),
        }
    }

    fn set_decorations(&self, decorations: bool) {
        match self {
            Self::HasWindow { window, .. } => window.set_decorations(decorations),
            Self::None(attributes) => attributes.borrow_mut().decorations = decorations,
        }
    }

    fn fullscreen(&self) -> Option<winit::window::Fullscreen> {
        match self {
            Self::HasWindow { window, .. } => window.fullscreen(),
            Self::None(attributes) => attributes.borrow().fullscreen.clone(),
        }
    }

    fn set_fullscreen(&self, fullscreen: Option<winit::window::Fullscreen>) {
        match self {
            Self::HasWindow { window, .. } => window.set_fullscreen(fullscreen),
            Self::None(attributes) => attributes.borrow_mut().fullscreen = fullscreen,
        }
    }

    fn set_window_level(&self, level: winit::window::WindowLevel) {
        match self {
            Self::HasWindow { window, .. } => window.set_window_level(level),
            Self::None(attributes) => attributes.borrow_mut().window_level = level,
        }
    }

    fn set_visible(&self, visible: bool) {
        match self {
            Self::HasWindow { window, .. } => window.set_visible(visible),
            Self::None(attributes) => attributes.borrow_mut().visible = visible,
        }
    }

    fn set_maximized(&self, maximized: bool) {
        match self {
            Self::HasWindow { window, .. } => window.set_maximized(maximized),
            Self::None(attributes) => attributes.borrow_mut().maximized = maximized,
        }
    }

    fn set_minimized(&self, minimized: bool) {
        match self {
            Self::HasWindow { window, .. } => window.set_minimized(minimized),
            Self::None(..) => { /* TODO: winit is missing attributes.borrow_mut().minimized = minimized*/
            }
        }
    }

    fn set_resizable(&self, resizable: bool) {
        match self {
            Self::HasWindow { window, .. } => {
                window.set_resizable(resizable);
            }
            Self::None(attributes) => attributes.borrow_mut().resizable = resizable,
        }
    }

    fn set_min_inner_size(
        &self,
        min_inner_size: Option<winit::dpi::LogicalSize<f64>>,
        scale_factor: f64,
    ) {
        match self {
            Self::HasWindow { window, .. } => {
                // Store as physical size to make sure that our potentially overriding scale factor is applied.
                window
                    .set_min_inner_size(min_inner_size.map(|s| s.to_physical::<u32>(scale_factor)))
            }
            Self::None(attributes) => {
                // Store as logical size, so that we can apply the real window scale factor later when it's known.
                attributes.borrow_mut().min_inner_size = min_inner_size.map(|s| s.into());
            }
        }
    }

    fn set_max_inner_size(
        &self,
        max_inner_size: Option<winit::dpi::LogicalSize<f64>>,
        scale_factor: f64,
    ) {
        match self {
            Self::HasWindow { window, .. } => {
                // Store as physical size to make sure that our potentially overriding scale factor is applied.
                window
                    .set_max_inner_size(max_inner_size.map(|s| s.to_physical::<u32>(scale_factor)))
            }
            Self::None(attributes) => {
                // Store as logical size, so that we can apply the real window scale factor later when it's known.
                attributes.borrow_mut().max_inner_size = max_inner_size.map(|s| s.into())
            }
        }
    }
}

#[derive(Default, PartialEq, Clone, Copy)]
pub(crate) enum WindowVisibility {
    #[default]
    Hidden,
    /// This implies that we might resize the window the first time it's shown.
    ShownFirstTime,
    Shown,
}

/// GraphicsWindow is an implementation of the [WindowAdapter][`crate::eventloop::WindowAdapter`] trait. This is
/// typically instantiated by entry factory functions of the different graphics back ends.
pub struct WinitWindowAdapter {
    pub shared_backend_data: Rc<SharedBackendData>,
    window: OnceCell<corelib::api::Window>,
    pub(crate) self_weak: Weak<Self>,
    pending_redraw: Cell<bool>,
    color_scheme: OnceCell<Pin<Box<Property<ColorScheme>>>>,
    constraints: Cell<corelib::window::LayoutConstraints>,
    /// Indicates if the window is shown, from the perspective of the API user.
    shown: Cell<WindowVisibility>,
    window_level: Cell<winit::window::WindowLevel>,
    maximized: Cell<bool>,
    minimized: Cell<bool>,
    fullscreen: Cell<bool>,

    pub(crate) renderer: Box<dyn WinitCompatibleRenderer>,
    /// We cache the size because winit_window.inner_size() can return different value between calls (eg, on X11)
    /// And we wan see the newer value before the Resized event was received, leading to inconsistencies
    size: Cell<PhysicalSize>,
    /// We requested a size to be set, but we didn't get the resize event from winit yet
    pending_requested_size: Cell<Option<winit::dpi::Size>>,

    /// Whether the size has been set explicitly via `set_size`.
    /// If that's the case, we should't resize to the preferred size in set_visible
    has_explicit_size: Cell<bool>,

    /// Indicate whether we've ever received a resize event from winit after showing the window.
    pending_resize_event_after_show: Cell<bool>,

    #[cfg(target_arch = "wasm32")]
    virtual_keyboard_helper: RefCell<Option<super::wasm_input_helper::WasmInputHelper>>,

    #[cfg(any(enable_accesskit, muda))]
    event_loop_proxy: EventLoopProxy<SlintEvent>,

    pub(crate) window_event_filter: Cell<
        Option<Box<dyn FnMut(&corelib::api::Window, &winit::event::WindowEvent) -> EventResult>>,
    >,

    winit_window_or_none: RefCell<WinitWindowOrNone>,
    window_existence_wakers: RefCell<Vec<core::task::Waker>>,

    #[cfg(not(use_winit_theme))]
    xdg_settings_watcher: RefCell<Option<i_slint_core::future::JoinHandle<()>>>,

    #[cfg(muda)]
    menubar: RefCell<Option<vtable::VRc<i_slint_core::menus::MenuVTable>>>,

    #[cfg(muda)]
    context_menu: RefCell<Option<vtable::VRc<i_slint_core::menus::MenuVTable>>>,

    #[cfg(all(muda, target_os = "macos"))]
    muda_enable_default_menu_bar: bool,

    /// Winit's window_icon API has no way of checking if the window icon is
    /// the same as a previously set one, so keep track of that here.
    window_icon_cache_key: RefCell<Option<ImageCacheKey>>,

    frame_throttle: Box<dyn crate::frame_throttle::FrameThrottle>,
}

impl WinitWindowAdapter {
    /// Creates a new reference-counted instance.
    pub(crate) fn new(
        shared_backend_data: Rc<SharedBackendData>,
        renderer: Box<dyn WinitCompatibleRenderer>,
        window_attributes: winit::window::WindowAttributes,
        #[cfg(any(enable_accesskit, muda))] proxy: EventLoopProxy<SlintEvent>,
        #[cfg(all(muda, target_os = "macos"))] muda_enable_default_menu_bar: bool,
    ) -> Rc<Self> {
        let self_rc = Rc::new_cyclic(|self_weak| Self {
            shared_backend_data: shared_backend_data.clone(),
            window: OnceCell::from(corelib::api::Window::new(self_weak.clone() as _)),
            self_weak: self_weak.clone(),
            pending_redraw: Default::default(),
            color_scheme: Default::default(),
            constraints: Default::default(),
            shown: Default::default(),
            window_level: Default::default(),
            maximized: Cell::default(),
            minimized: Cell::default(),
            fullscreen: Cell::default(),
            winit_window_or_none: RefCell::new(WinitWindowOrNone::None(window_attributes.into())),
            window_existence_wakers: RefCell::new(Vec::default()),
            size: Cell::default(),
            pending_requested_size: Cell::new(None),
            has_explicit_size: Default::default(),
            pending_resize_event_after_show: Default::default(),
            renderer,
            #[cfg(target_arch = "wasm32")]
            virtual_keyboard_helper: Default::default(),
            #[cfg(any(enable_accesskit, muda))]
            event_loop_proxy: proxy,
            window_event_filter: Cell::new(None),
            #[cfg(not(use_winit_theme))]
            xdg_settings_watcher: Default::default(),
            #[cfg(muda)]
            menubar: Default::default(),
            #[cfg(muda)]
            context_menu: Default::default(),
            #[cfg(all(muda, target_os = "macos"))]
            muda_enable_default_menu_bar,
            window_icon_cache_key: Default::default(),
            frame_throttle: crate::frame_throttle::create_frame_throttle(
                self_weak.clone(),
                shared_backend_data.is_wayland,
            ),
        });

        self_rc.shared_backend_data.register_inactive_window((self_rc.clone()) as _);

        self_rc
    }

    fn renderer(&self) -> &dyn WinitCompatibleRenderer {
        self.renderer.as_ref()
    }

    pub fn ensure_window(
        &self,
        active_event_loop: &ActiveEventLoop,
    ) -> Result<Arc<winit::window::Window>, PlatformError> {
        #[allow(unused_mut)]
        let mut window_attributes = match &*self.winit_window_or_none.borrow() {
            WinitWindowOrNone::HasWindow { window, .. } => return Ok(window.clone()),
            WinitWindowOrNone::None(attributes) => attributes.borrow().clone(),
        };

        #[cfg(all(unix, not(target_vendor = "apple")))]
        {
            if let Some(xdg_app_id) = WindowInner::from_pub(self.window()).xdg_app_id() {
                #[cfg(feature = "wayland")]
                {
                    use winit::platform::wayland::WindowAttributesExtWayland;
                    window_attributes = window_attributes.with_name(xdg_app_id.clone(), "");
                }
                #[cfg(feature = "x11")]
                {
                    use winit::platform::x11::WindowAttributesExtX11;
                    window_attributes = window_attributes.with_name(xdg_app_id.clone(), "");
                }
            }
        }

        let mut winit_window_or_none = self.winit_window_or_none.borrow_mut();

        // Never show the window right away, as we
        //  a) need to compute the correct size based on the scale factor before it's shown on the screen (handled by set_visible)
        //  b) need to create the accesskit adapter before it's shown on the screen, as required by accesskit.
        let show_after_creation = std::mem::replace(&mut window_attributes.visible, false);
        let resizable = window_attributes.resizable;

        let overriding_scale_factor = std::env::var("SLINT_SCALE_FACTOR")
            .ok()
            .and_then(|x| x.parse::<f32>().ok())
            .filter(|f| *f > 0.);

        if let Some(sf) = overriding_scale_factor {
            apply_scale_factor_to_logical_sizes_in_attributes(&mut window_attributes, sf as f64)
        }

        // Work around issue with menu bar appearing translucent in fullscreen (#8793)
        #[cfg(all(muda, target_os = "windows"))]
        if self.menubar.borrow().is_some() {
            window_attributes = window_attributes.with_transparent(false);
        }

        let winit_window = self.renderer.resume(active_event_loop, window_attributes)?;

        let scale_factor =
            overriding_scale_factor.unwrap_or_else(|| winit_window.scale_factor() as f32);
        self.window().try_dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor })?;

        *winit_window_or_none = WinitWindowOrNone::HasWindow {
            window: winit_window.clone(),
            #[cfg(enable_accesskit)]
            accesskit_adapter: crate::accesskit::AccessKitAdapter::new(
                self.self_weak.clone(),
                active_event_loop,
                &winit_window,
                self.event_loop_proxy.clone(),
            )
            .into(),
            #[cfg(muda)]
            muda_adapter: self
                .menubar
                .borrow()
                .as_ref()
                .map(|menubar| {
                    crate::muda::MudaAdapter::setup(
                        menubar,
                        &winit_window,
                        self.event_loop_proxy.clone(),
                        self.self_weak.clone(),
                    )
                })
                .into(),
            #[cfg(muda)]
            context_menu_muda_adapter: None.into(),
        };

        drop(winit_window_or_none);

        if show_after_creation {
            self.shown.set(WindowVisibility::Hidden);
            self.set_visibility(WindowVisibility::ShownFirstTime)?;
        }

        {
            // Workaround for winit bug #2990
            // Non-resizable windows can still contain a maximize button,
            // so we'd have to additionally remove the button.
            let mut buttons = winit_window.enabled_buttons();
            buttons.set(WindowButtons::MAXIMIZE, resizable);
            winit_window.set_enabled_buttons(buttons);
        }

        self.shared_backend_data
            .register_window(winit_window.id(), (self.self_weak.upgrade().unwrap()) as _);

        for waker in self.window_existence_wakers.take().into_iter() {
            waker.wake();
        }

        Ok(winit_window)
    }

    pub(crate) fn suspend(&self) -> Result<(), PlatformError> {
        let mut winit_window_or_none = self.winit_window_or_none.borrow_mut();
        match *winit_window_or_none {
            WinitWindowOrNone::HasWindow { ref window, .. } => {
                self.renderer().suspend()?;

                let last_window_rc = window.clone();

                let mut attributes = Self::window_attributes().unwrap_or_default();
                attributes.inner_size = Some(physical_size_to_winit(self.size.get()).into());
                attributes.position = last_window_rc.outer_position().ok().map(|pos| pos.into());
                *winit_window_or_none = WinitWindowOrNone::None(attributes.into());

                if let Some(last_instance) = Arc::into_inner(last_window_rc) {
                    // Note: Don't register the window in inactive_windows for re-creation later, as creating the window
                    // on wayland implies making it visible. Unfortunately, winit won't allow creating a window on wayland
                    // that's not visible.
                    self.shared_backend_data.unregister_window(Some(last_instance.id()));
                    drop(last_instance);
                } else {
                    i_slint_core::debug_log!(
                        "Slint winit backend: request to hide window failed because references to the window still exist. This could be an application issue, make sure that there are no slint::WindowHandle instances left"
                    );
                }
            }
            WinitWindowOrNone::None(ref attributes) => {
                attributes.borrow_mut().visible = false;
            }
        }

        Ok(())
    }

    pub(crate) fn window_attributes() -> Result<WindowAttributes, PlatformError> {
        let mut attrs = WindowAttributes::default().with_transparent(true).with_visible(false);

        attrs = attrs.with_title("Slint Window".to_string());

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;

            use wasm_bindgen::JsCast;

            if let Some(html_canvas) = web_sys::window()
                .ok_or_else(|| "winit backend: Could not retrieve DOM window".to_string())?
                .document()
                .ok_or_else(|| "winit backend: Could not retrieve DOM document".to_string())?
                .get_element_by_id("canvas")
                .and_then(|canvas_elem| canvas_elem.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            {
                attrs = attrs
                    .with_canvas(Some(html_canvas))
                    // Don't activate the window by default, as that will cause the page to scroll,
                    // ignoring any existing anchors.
                    .with_active(false);
            }
        };

        Ok(attrs)
    }

    /// Draw the items of the specified `component` in the given window.
    pub fn draw(&self) -> Result<(), PlatformError> {
        if matches!(self.shown.get(), WindowVisibility::Hidden) {
            return Ok(()); // caller bug, doesn't make sense to call draw() when not shown
        }

        self.pending_redraw.set(false);

        if let Some(winit_window) = self.winit_window_or_none.borrow().as_window() {
            // on macOS we sometimes don't get a resize event after calling
            // request_inner_size(), it returning None (promising a resize event), and then delivering RedrawRequested. To work around this,
            // catch up here to ensure the renderer can resize the surface correctly.
            // Note: On displays with a scale factor != 1, we get a scale factor change
            // event and a resize event, so all is good.
            if self.pending_resize_event_after_show.take() {
                self.resize_event(winit_window.inner_size())?;
            }
        }

        let renderer = self.renderer();
        renderer.render(self.window())?;

        Ok(())
    }

    pub fn winit_window(&self) -> Option<Arc<winit::window::Window>> {
        self.winit_window_or_none.borrow().as_window()
    }

    #[cfg(muda)]
    pub fn rebuild_menubar(&self) {
        let WinitWindowOrNone::HasWindow {
            window: winit_window,
            muda_adapter: maybe_muda_adapter,
            ..
        } = &*self.winit_window_or_none.borrow()
        else {
            return;
        };
        let mut maybe_muda_adapter = maybe_muda_adapter.borrow_mut();
        let Some(muda_adapter) = maybe_muda_adapter.as_mut() else { return };
        muda_adapter.rebuild_menu(&winit_window, self.menubar.borrow().as_ref(), MudaType::Menubar);
    }

    #[cfg(muda)]
    pub fn muda_event(&self, entry_id: usize, muda_type: MudaType) {
        let Ok(maybe_muda_adapter) = std::cell::Ref::filter_map(
            self.winit_window_or_none.borrow(),
            |winit_window_or_none| match (winit_window_or_none, muda_type) {
                (WinitWindowOrNone::HasWindow { muda_adapter, .. }, MudaType::Menubar) => {
                    Some(muda_adapter)
                }
                (
                    WinitWindowOrNone::HasWindow { context_menu_muda_adapter, .. },
                    MudaType::Context,
                ) => Some(context_menu_muda_adapter),
                (WinitWindowOrNone::None(..), _) => None,
            },
        ) else {
            return;
        };
        let maybe_muda_adapter = maybe_muda_adapter.borrow();
        let Some(muda_adapter) = maybe_muda_adapter.as_ref() else { return };
        let menu = match muda_type {
            MudaType::Menubar => &self.menubar,
            MudaType::Context => &self.context_menu,
        };
        let menu = menu.borrow();
        let Some(menu) = menu.as_ref() else { return };
        muda_adapter.invoke(menu, entry_id);
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
        match &*self.winit_window_or_none.borrow() {
            WinitWindowOrNone::HasWindow { window, .. } => {
                if let Some(size) = window.request_inner_size(size) {
                    // On wayland we might not get a WindowEvent::Resized, so resize the EGL surface right away.
                    self.resize_event(size)?;
                    Ok(true)
                } else {
                    self.pending_requested_size.set(size.into());
                    // None means that we'll get a `WindowEvent::Resized` later
                    Ok(false)
                }
            }
            WinitWindowOrNone::None(attributes) => {
                let scale_factor = self.window().scale_factor() as _;
                // Avoid storing the physical size in the attributes. When creating a new window, we don't know the scale
                // factor, so we've computed the desired size based on a factor of 1 and provided the physical size
                // will be wrong when the window is created. So stick to a logical size.
                attributes.borrow_mut().inner_size =
                    Some(size.to_logical::<f64>(scale_factor).into());
                self.resize_event(size.to_physical(scale_factor))?;
                Ok(true)
            }
        }
    }

    pub fn resize_event(&self, size: winit::dpi::PhysicalSize<u32>) -> Result<(), PlatformError> {
        self.pending_resize_event_after_show.set(false);
        // When a window is minimized on Windows, we get a move event to an off-screen position
        // and a resize even with a zero size. Don't forward that, especially not to the renderer,
        // which might panic when trying to create a zero-sized surface.
        if size.width > 0 && size.height > 0 {
            let physical_size = physical_size_to_slint(&size);
            self.size.set(physical_size);
            self.pending_requested_size.set(None);
            let scale_factor = WindowInner::from_pub(self.window()).scale_factor();
            self.window().try_dispatch_event(WindowEvent::Resized {
                size: physical_size.to_logical(scale_factor),
            })?;

            // Workaround fox winit not sync'ing CSS size of the canvas (the size shown on the browser)
            // with the width/height attribute (the size of the viewport/GL surface)
            // If they're not in sync, the UI would be shown as scaled
            #[cfg(target_arch = "wasm32")]
            if let Some(html_canvas) = self
                .winit_window_or_none
                .borrow()
                .as_window()
                .and_then(|winit_window| winit_window.canvas())
            {
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
            .set(scheme);
        // Inform winit about the selected color theme, so that the window decoration is drawn correctly.
        #[cfg(not(use_winit_theme))]
        if let Some(winit_window) = self.winit_window() {
            winit_window.set_theme(match scheme {
                ColorScheme::Unknown => None,
                ColorScheme::Dark => Some(winit::window::Theme::Dark),
                ColorScheme::Light => Some(winit::window::Theme::Light),
            });
        }
    }

    pub fn window_state_event(&self) {
        let Some(winit_window) = self.winit_window_or_none.borrow().as_window() else { return };

        if let Some(minimized) = winit_window.is_minimized() {
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
        let maximized = winit_window.is_maximized();
        if !self.window().is_minimized() {
            self.maximized.set(maximized);
            if maximized != self.window().is_maximized() {
                self.window().set_maximized(maximized);
            }
        }

        // NOTE: Fullscreen overrides maximized so if both are true then the
        // window will remain in fullscreen. Fullscreen must be false to switch
        // to maximized.
        let fullscreen = winit_window.fullscreen().is_some();
        if fullscreen != self.window().is_fullscreen() {
            self.window().set_fullscreen(fullscreen);
        }
    }

    #[cfg(enable_accesskit)]
    pub(crate) fn accesskit_adapter(
        &self,
    ) -> Option<std::cell::Ref<'_, RefCell<crate::accesskit::AccessKitAdapter>>> {
        std::cell::Ref::filter_map(
            self.winit_window_or_none.try_borrow().ok()?,
            |wor: &WinitWindowOrNone| match wor {
                WinitWindowOrNone::HasWindow { accesskit_adapter, .. } => Some(accesskit_adapter),
                WinitWindowOrNone::None(..) => None,
            },
        )
        .ok()
    }

    #[cfg(enable_accesskit)]
    pub(crate) fn with_access_kit_adapter_from_weak_window_adapter(
        self_weak: Weak<Self>,
        callback: impl FnOnce(&RefCell<crate::accesskit::AccessKitAdapter>),
    ) {
        let Some(self_) = self_weak.upgrade() else { return };
        let winit_window_or_none = self_.winit_window_or_none.borrow();
        match &*winit_window_or_none {
            WinitWindowOrNone::HasWindow { accesskit_adapter, .. } => callback(accesskit_adapter),
            WinitWindowOrNone::None(..) => {}
        }
    }

    #[cfg(not(use_winit_theme))]
    fn spawn_xdg_settings_watcher(&self) -> Option<i_slint_core::future::JoinHandle<()>> {
        let window_inner = WindowInner::from_pub(self.window());
        let self_weak = self.self_weak.clone();
        window_inner
            .context()
            .spawn_local(async move {
                if let Err(err) = crate::xdg_color_scheme::watch(self_weak).await {
                    i_slint_core::debug_log!("Error watching for xdg color schemes: {}", err);
                }
            })
            .ok()
    }

    pub fn activation_changed(&self, is_active: bool) -> Result<(), PlatformError> {
        let have_focus = is_active || self.input_method_focused();
        let slint_window = self.window();
        let runtime_window = WindowInner::from_pub(slint_window);
        // We don't render popups as separate windows yet, so treat
        // focus to be the same as being active.
        if have_focus != runtime_window.active() {
            slint_window.try_dispatch_event(
                corelib::platform::WindowEvent::WindowActiveChanged(have_focus),
            )?;
        }

        #[cfg(all(muda, target_os = "macos"))]
        {
            if let WinitWindowOrNone::HasWindow { muda_adapter, .. } =
                &*self.winit_window_or_none.borrow()
            {
                if muda_adapter.borrow().is_none()
                    && self.muda_enable_default_menu_bar
                    && self.menubar.borrow().is_none()
                {
                    *muda_adapter.borrow_mut() =
                        Some(crate::muda::MudaAdapter::setup_default_menu_bar()?);
                }

                if let Some(muda_adapter) = muda_adapter.borrow().as_ref() {
                    muda_adapter.window_activation_changed(is_active);
                }
            }
        }

        Ok(())
    }

    fn set_visibility(&self, visibility: WindowVisibility) -> Result<(), PlatformError> {
        if visibility == self.shown.get() {
            return Ok(());
        }

        self.shown.set(visibility);
        self.pending_resize_event_after_show.set(!matches!(visibility, WindowVisibility::Hidden));
        self.pending_redraw.set(false);
        if matches!(visibility, WindowVisibility::ShownFirstTime | WindowVisibility::Shown) {
            let recreating_window = matches!(visibility, WindowVisibility::Shown);

            let Some(winit_window) = self.winit_window() else {
                // Can't really show it on the screen, safe it in the attributes and try again later
                // by registering it for activation when we can.
                self.winit_window_or_none.borrow().set_visible(true);
                self.shared_backend_data
                    .register_inactive_window((self.self_weak.upgrade().unwrap()) as _);
                return Ok(());
            };

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
                // Don't set the preferred size as the user may have resized the window
                && !recreating_window
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
            // Wayland doesn't support hiding a window, only destroying it entirely.
            if self.winit_window_or_none.borrow().as_window().is_some_and(|winit_window| {
                use raw_window_handle::HasWindowHandle;
                winit_window.window_handle().is_ok_and(|h| {
                    matches!(h.as_raw(), raw_window_handle::RawWindowHandle::Wayland(..))
                }) || std::env::var_os("SLINT_DESTROY_WINDOW_ON_HIDE").is_some()
            }) {
                self.suspend()?;
                // Note: Don't register the window in inactive_windows for re-creation later, as creating the window
                // on wayland implies making it visible. Unfortunately, winit won't allow creating a window on wayland
                // that's not visible.
            } else {
                self.winit_window_or_none.borrow().set_visible(false);
            }

            /* FIXME:
            if let Some(existing_blinker) = self.cursor_blinker.borrow().upgrade() {
                existing_blinker.stop();
            }*/
            Ok(())
        }
    }

    pub(crate) fn visibility(&self) -> WindowVisibility {
        self.shown.get()
    }

    pub(crate) fn pending_redraw(&self) -> bool {
        self.pending_redraw.get()
    }

    pub async fn async_winit_window(
        self_weak: Weak<Self>,
    ) -> Result<Arc<winit::window::Window>, PlatformError> {
        std::future::poll_fn(move |context| {
            let Some(self_) = self_weak.upgrade() else {
                return std::task::Poll::Ready(Err(format!(
                    "Unable to obtain winit window from destroyed window"
                )
                .into()));
            };
            match self_.winit_window() {
                Some(window) => std::task::Poll::Ready(Ok(window)),
                None => {
                    let waker = context.waker();
                    if !self_.window_existence_wakers.borrow().iter().any(|w| w.will_wake(waker)) {
                        self_.window_existence_wakers.borrow_mut().push(waker.clone());
                    }
                    std::task::Poll::Pending
                }
            }
        })
        .await
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
        self.set_visibility(if visible {
            WindowVisibility::Shown
        } else {
            WindowVisibility::Hidden
        })
    }

    fn position(&self) -> Option<corelib::api::PhysicalPosition> {
        match &*self.winit_window_or_none.borrow() {
            WinitWindowOrNone::HasWindow { window, .. } => match window.outer_position() {
                Ok(outer_position) => {
                    Some(corelib::api::PhysicalPosition::new(outer_position.x, outer_position.y))
                }
                Err(_) => None,
            },
            WinitWindowOrNone::None(attributes) => {
                attributes.borrow().position.map(|pos| {
                    match pos {
                        winit::dpi::Position::Physical(phys_pos) => {
                            corelib::api::PhysicalPosition::new(phys_pos.x, phys_pos.y)
                        }
                        winit::dpi::Position::Logical(logical_pos) => {
                            // Best effort: Use the last known scale factor
                            corelib::api::LogicalPosition::new(
                                logical_pos.x as _,
                                logical_pos.y as _,
                            )
                            .to_physical(self.window().scale_factor())
                        }
                    }
                })
            }
        }
    }

    fn set_position(&self, position: corelib::api::WindowPosition) {
        let winit_pos = position_to_winit(&position);
        match &*self.winit_window_or_none.borrow() {
            WinitWindowOrNone::HasWindow { window, .. } => window.set_outer_position(winit_pos),
            WinitWindowOrNone::None(attributes) => {
                attributes.borrow_mut().position = Some(winit_pos);
            }
        }
    }

    fn set_size(&self, size: corelib::api::WindowSize) {
        self.has_explicit_size.set(true);
        // TODO: don't ignore error, propagate to caller
        self.resize_window(window_size_to_winit(&size)).ok();
    }

    fn size(&self) -> corelib::api::PhysicalSize {
        self.size.get()
    }

    fn request_redraw(&self) {
        if !self.pending_redraw.replace(true) {
            self.frame_throttle.request_throttled_redraw();
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

        let winit_window_or_none = self.winit_window_or_none.borrow();

        // Use our scale factor instead of winit's logical size to take a scale factor override into account.
        let sf = self.window().scale_factor();

        // Update the icon only if it changes, to avoid flashing.
        let icon_image = window_item.icon();
        let icon_image_cache_key = ImageCacheKey::new((&icon_image).into());
        if *self.window_icon_cache_key.borrow() != icon_image_cache_key {
            *self.window_icon_cache_key.borrow_mut() = icon_image_cache_key;
            winit_window_or_none.set_window_icon(icon_to_winit(
                icon_image,
                i_slint_core::lengths::LogicalSize::new(64., 64.) * ScaleFactor::new(sf),
            ));
        }
        winit_window_or_none.set_title(&properties.title());
        winit_window_or_none.set_decorations(
            !window_item.no_frame() || winit_window_or_none.fullscreen().is_some(),
        );

        let new_window_level = if window_item.always_on_top() {
            winit::window::WindowLevel::AlwaysOnTop
        } else {
            winit::window::WindowLevel::Normal
        };
        // Only change the window level if it changes, to avoid https://github.com/slint-ui/slint/issues/3280
        // (Ubuntu 20.04's window manager always bringing the window to the front on x11)
        if self.window_level.replace(new_window_level) != new_window_level {
            winit_window_or_none.set_window_level(new_window_level);
        }

        let mut width = window_item.width().get() as f32;
        let mut height = window_item.height().get() as f32;
        let mut must_resize = false;
        let existing_size = self.size.get().to_logical(sf);

        if width <= 0. || height <= 0. {
            must_resize = true;
            if width <= 0. {
                width = existing_size.width;
            }
            if height <= 0. {
                height = existing_size.height;
            }
        }

        // Adjust the size of the window to the value of the width and height property (if these property are changed from .slint).
        // But not if there is a pending resize in flight as that resize will reset these properties back
        if ((existing_size.width - width).abs() > 1. || (existing_size.height - height).abs() > 1.)
            && self.pending_requested_size.get().is_none()
        {
            // If we're in fullscreen state, don't try to resize the window but maintain the surface
            // size we've been assigned to from the windowing system. Weston/Wayland don't like it
            // when we create a surface that's bigger than the screen due to constraints (#532).
            if winit_window_or_none.fullscreen().is_none() {
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
            self.window()
                .try_dispatch_event(WindowEvent::Resized {
                    size: i_slint_core::api::LogicalSize::new(width, height),
                })
                .unwrap();
        }

        let m = properties.is_fullscreen();
        if m != self.fullscreen.get() {
            if m {
                if winit_window_or_none.fullscreen().is_none() {
                    winit_window_or_none
                        .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                }
            } else {
                winit_window_or_none.set_fullscreen(None);
            }
            self.fullscreen.set(m);
        }

        let m = properties.is_maximized();
        if m != self.maximized.get() {
            self.maximized.set(m);
            winit_window_or_none.set_maximized(m);
        }

        let m = properties.is_minimized();
        if m != self.minimized.get() {
            self.minimized.set(m);
            winit_window_or_none.set_minimized(m);
        }

        // If we're in fullscreen, don't try to resize the window but
        // maintain the surface size we've been assigned to from the
        // windowing system. Weston/Wayland don't like it when we create a
        // surface that's bigger than the screen due to constraints (#532).
        if winit_window_or_none.fullscreen().is_some() {
            return;
        }

        let new_constraints = properties.layout_constraints();
        if new_constraints == self.constraints.get() {
            return;
        }

        self.constraints.set(new_constraints);

        let resizable = window_is_resizable(new_constraints.min, new_constraints.max);
        // we must call set_resizable before setting the min and max size otherwise setting the min and max size don't work on X11
        winit_window_or_none.set_resizable(resizable);
        // Important: Filter out (temporary?) zero width/heights, to avoid attempting to create a zero surface. For example, with wayland
        // the client-side rendering ends up passing a zero width/height to the renderer.
        let winit_min_inner =
            new_constraints.min.map(logical_size_to_winit).map(filter_out_zero_width_or_height);
        winit_window_or_none.set_min_inner_size(winit_min_inner, sf as f64);
        let winit_max_inner =
            new_constraints.max.map(logical_size_to_winit).map(filter_out_zero_width_or_height);
        winit_window_or_none.set_max_inner_size(winit_max_inner, sf as f64);

        // On ios, etc. apps are fullscreen and need to be responsive.
        #[cfg(not(ios_and_friends))]
        adjust_window_size_to_satisfy_constraints(self, winit_min_inner, winit_max_inner);

        // Auto-resize to the preferred size if users (SlintPad) requests it
        #[cfg(target_arch = "wasm32")]
        if let Some(canvas) =
            winit_window_or_none.as_window().and_then(|winit_window| winit_window.canvas())
        {
            if canvas
                .dataset()
                .get("slintAutoResizeToPreferred")
                .and_then(|val_str| val_str.parse().ok())
                .unwrap_or_default()
            {
                let pref = new_constraints.preferred;
                if pref.width > 0 as Coord || pref.height > 0 as Coord {
                    // TODO: don't ignore error, propgate to caller
                    self.resize_window(logical_size_to_winit(pref).into()).ok();
                };
            }
        }
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
        if let Some(winit_window) = self.winit_window_or_none.borrow().as_window() {
            winit_window.set_cursor_visible(cursor != MouseCursor::None);
            winit_window.set_cursor(winit_cursor);
        }
    }

    fn input_method_request(&self, request: corelib::window::InputMethodRequest) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(winit_window) = self.winit_window_or_none.borrow().as_window() {
            let props = match &request {
                corelib::window::InputMethodRequest::Enable(props) => {
                    winit_window.set_ime_allowed(true);
                    props
                }
                corelib::window::InputMethodRequest::Disable => {
                    return winit_window.set_ime_allowed(false);
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
        }

        #[cfg(target_arch = "wasm32")]
        match request {
            corelib::window::InputMethodRequest::Enable(..) => {
                let mut vkh = self.virtual_keyboard_helper.borrow_mut();
                let Some(canvas) =
                    self.winit_window().and_then(|winit_window| winit_window.canvas())
                else {
                    return;
                };
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
                    cfg_if::cfg_if! {
                        if #[cfg(use_winit_theme)] {
                            self.winit_window_or_none
                                .borrow()
                                .as_window()
                                .and_then(|window| window.theme())
                                .map_or(ColorScheme::Unknown, |theme| match theme {
                                    winit::window::Theme::Dark => ColorScheme::Dark,
                                    winit::window::Theme::Light => ColorScheme::Light,
                                })
                        } else {
                            if let Some(old_watch) = self.xdg_settings_watcher.replace(self.spawn_xdg_settings_watcher()) {
                                old_watch.abort()
                            }
                            ColorScheme::Unknown
                        }
                    }
                }))
            })
            .as_ref()
            .get()
    }

    #[cfg(muda)]
    fn supports_native_menu_bar(&self) -> bool {
        true
    }

    #[cfg(muda)]
    fn setup_menubar(&self, menubar: vtable::VRc<i_slint_core::menus::MenuVTable>) {
        self.menubar.replace(Some(menubar));

        if let WinitWindowOrNone::HasWindow { muda_adapter, .. } =
            &*self.winit_window_or_none.borrow()
        {
            // On Windows, we must destroy the muda menu before re-creating a new one
            drop(muda_adapter.borrow_mut().take());
            muda_adapter.replace(Some(crate::muda::MudaAdapter::setup(
                self.menubar.borrow().as_ref().unwrap(),
                &self.winit_window().unwrap(),
                self.event_loop_proxy.clone(),
                self.self_weak.clone(),
            )));
        }
    }

    #[cfg(muda)]
    fn show_native_popup_menu(
        &self,
        context_menu_item: vtable::VRc<i_slint_core::menus::MenuVTable>,
        position: LogicalPosition,
    ) -> bool {
        self.context_menu.replace(Some(context_menu_item));

        if let WinitWindowOrNone::HasWindow { context_menu_muda_adapter, .. } =
            &*self.winit_window_or_none.borrow()
        {
            // On Windows, we must destroy the muda menu before re-creating a new one
            drop(context_menu_muda_adapter.borrow_mut().take());
            if let Some(new_adapter) = crate::muda::MudaAdapter::show_context_menu(
                self.context_menu.borrow().as_ref().unwrap(),
                &self.winit_window().unwrap(),
                position,
                self.event_loop_proxy.clone(),
            ) {
                context_menu_muda_adapter.replace(Some(new_adapter));
                return true;
            }
        }
        false
    }

    #[cfg(enable_accesskit)]
    fn handle_focus_change(&self, _old: Option<ItemRc>, _new: Option<ItemRc>) {
        let Some(accesskit_adapter_cell) = self.accesskit_adapter() else { return };
        accesskit_adapter_cell.borrow_mut().handle_focus_item_change();
    }

    #[cfg(enable_accesskit)]
    fn register_item_tree(&self) {
        let Some(accesskit_adapter_cell) = self.accesskit_adapter() else { return };
        // If the accesskit_adapter is already borrowed, this means the new items were created when the tree was built and there is no need to re-visit them
        if let Ok(mut a) = accesskit_adapter_cell.try_borrow_mut() {
            a.reload_tree();
        };
    }

    #[cfg(enable_accesskit)]
    fn unregister_item_tree(
        &self,
        component: ItemTreeRef,
        _: &mut dyn Iterator<Item = Pin<ItemRef<'_>>>,
    ) {
        let Some(accesskit_adapter_cell) = self.accesskit_adapter() else { return };
        if let Ok(mut a) = accesskit_adapter_cell.try_borrow_mut() {
            a.unregister_item_tree(component);
        };
    }

    #[cfg(feature = "raw-window-handle-06")]
    fn window_handle_06_rc(
        &self,
    ) -> Result<Arc<dyn raw_window_handle::HasWindowHandle>, raw_window_handle::HandleError> {
        self.winit_window_or_none
            .borrow()
            .as_window()
            .map_or(Err(raw_window_handle::HandleError::Unavailable), |window| Ok(window))
    }

    #[cfg(feature = "raw-window-handle-06")]
    fn display_handle_06_rc(
        &self,
    ) -> Result<Arc<dyn raw_window_handle::HasDisplayHandle>, raw_window_handle::HandleError> {
        self.winit_window_or_none
            .borrow()
            .as_window()
            .map_or(Err(raw_window_handle::HandleError::Unavailable), |window| Ok(window))
    }

    fn bring_to_front(&self) -> Result<(), PlatformError> {
        if let Some(winit_window) = self.winit_window_or_none.borrow().as_window() {
            winit_window.set_minimized(false);
            winit_window.focus_window();
        }
        Ok(())
    }
}

impl Drop for WinitWindowAdapter {
    fn drop(&mut self) {
        self.shared_backend_data.unregister_window(
            self.winit_window_or_none.borrow().as_window().map(|winit_window| winit_window.id()),
        );

        #[cfg(not(use_winit_theme))]
        if let Some(xdg_watch_future) = self.xdg_settings_watcher.take() {
            xdg_watch_future.abort();
        }
    }
}

// Winit doesn't automatically resize the window to satisfy constraints. Qt does it though, and so do we here.
#[cfg(not(ios_and_friends))]
fn adjust_window_size_to_satisfy_constraints(
    adapter: &WinitWindowAdapter,
    min_size: Option<winit::dpi::LogicalSize<f64>>,
    max_size: Option<winit::dpi::LogicalSize<f64>>,
) {
    let sf = adapter.window().scale_factor() as f64;
    let current_size = adapter
        .pending_requested_size
        .get()
        .map(|s| s.to_logical::<f64>(sf))
        .unwrap_or_else(|| physical_size_to_winit(adapter.size.get()).to_logical(sf));

    let mut window_size = current_size;
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

    if window_size != current_size {
        // TODO: don't ignore error, propgate to caller
        adapter.resize_window(window_size.into()).ok();
    }
}
