// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! System tray integration.
//!
//! This module hosts the `SystemTrayIcon` native item (the element exposed to `.slint`) and
//! wraps the platform-specific tray icon backends: `ksni` on Linux/BSD, AppKit
//! (`NSStatusBar` / `NSStatusItem`) on macOS, and `Shell_NotifyIconW` on Windows.

#![allow(unsafe_code)]

use crate::graphics::Image;
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, InternalKeyEvent,
    KeyEventResult, MouseEvent,
};
use crate::item_rendering::CachedRenderingData;
use crate::items::{Item, ItemConsts, ItemRc, MouseCursor, Orientation, RenderingResult, VoidArg};
use crate::layout::LayoutInfo;
use crate::lengths::{LogicalRect, LogicalSize};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use crate::{Callback, Coord, Property, SharedString};
use alloc::boxed::Box;
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use i_slint_core_macros::*;

#[cfg(target_os = "macos")]
mod appkit;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod ksni;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use self::appkit::PlatformTray;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use self::ksni::PlatformTray;
#[cfg(target_os = "windows")]
use self::windows::PlatformTray;

/// Parameters passed to the platform-specific tray backend when building a tray icon.
pub struct Params<'a> {
    pub icon: &'a Image,
    pub tooltip: &'a str,
    pub title: &'a str,
}

/// Errors raised while constructing a platform tray icon.
#[allow(dead_code)]
#[derive(Debug, derive_more::Error, derive_more::Display)]
pub enum Error {
    #[display("Failed to create a rgba8 buffer from an icon image")]
    Rgba8,
    #[display("{}", 0)]
    PlatformError(crate::platform::PlatformError),
    #[display("{}", 0)]
    EventLoopError(crate::api::EventLoopError),
}

/// Owning handle to a live platform tray icon. Dropping it removes the icon.
pub struct SystemTrayIconHandle(PlatformTray);

impl SystemTrayIconHandle {
    pub fn new(
        params: Params,
        self_weak: crate::item_tree::ItemWeak,
        context: &crate::SlintContext,
    ) -> Result<Self, Error> {
        PlatformTray::new(params, self_weak, context).map(Self)
    }

    pub fn rebuild_menu(
        &self,
        menu: vtable::VRef<'_, crate::menus::MenuVTable>,
        entries_out: &mut std::vec::Vec<crate::items::MenuEntry>,
    ) {
        self.0.rebuild_menu(menu, entries_out);
    }

    pub fn set_visible(&self, visible: bool) {
        self.0.set_visible(visible);
    }

    pub fn set_icon(&self, icon: &Image) {
        self.0.set_icon(icon);
    }

    pub fn set_tooltip(&self, tooltip: &str) {
        self.0.set_tooltip(tooltip);
    }

    pub fn set_title(&self, title: &str) {
        self.0.set_title(title);
    }
}

// ---------------------------------------------------------------------------
// Native `SystemTrayIcon` item, exposed to `.slint`.
// ---------------------------------------------------------------------------

#[repr(C)]
/// Wraps the internal data structure for the SystemTrayIcon
pub struct SystemTrayIconDataBox(core::ptr::NonNull<SystemTrayIconData>);

impl Default for SystemTrayIconDataBox {
    fn default() -> Self {
        SystemTrayIconDataBox(Box::leak(Box::<SystemTrayIconData>::default()).into())
    }
}
impl Drop for SystemTrayIconDataBox {
    fn drop(&mut self) {
        // Safety: the self.0 was constructed from a Box::leak in SystemTrayIconDataBox::default
        drop(unsafe { Box::from_raw(self.0.as_ptr()) });
    }
}

impl core::ops::Deref for SystemTrayIconDataBox {
    type Target = SystemTrayIconData;
    fn deref(&self) -> &Self::Target {
        // Safety: initialized in SystemTrayIconDataBox::default
        unsafe { self.0.as_ref() }
    }
}

#[derive(Default)]
pub struct SystemTrayIconData {
    inner: std::cell::OnceCell<SystemTrayIconHandle>,
    change_tracker: crate::properties::ChangeTracker,
    visible_tracker: crate::properties::ChangeTracker,
    icon_tracker: crate::properties::ChangeTracker,
    tooltip_tracker: crate::properties::ChangeTracker,
    title_tracker: crate::properties::ChangeTracker,
    /// Whether this tray currently contributes to the SlintContext keepalive
    /// counter. Flipped in lockstep with `acquire_keepalive`/`release_keepalive`
    /// so that a re-fired tracker can't double-increment.
    keepalive_live: core::cell::Cell<bool>,
    menu: core::cell::RefCell<Option<MenuState>>,
}

impl Drop for SystemTrayIconData {
    fn drop(&mut self) {
        if self.keepalive_live.get()
            && let Some(ctx) = crate::context::GLOBAL_CONTEXT.with(|p| p.get().cloned())
        {
            ctx.release_keepalive();
        }
    }
}

struct MenuState {
    menu_vrc: vtable::VRc<crate::menus::MenuVTable>,
    entries: std::vec::Vec<crate::items::MenuEntry>,
    tracker: Pin<Box<crate::properties::PropertyTracker<false, MenuDirtyHandler>>>,
}

struct MenuDirtyHandler {
    self_weak: crate::item_tree::ItemWeak,
}

impl crate::properties::PropertyDirtyHandler for MenuDirtyHandler {
    fn notify(self: Pin<&Self>) {
        let self_weak = self.self_weak.clone();
        crate::timers::Timer::single_shot(Default::default(), move || {
            let Some(item_rc) = self_weak.upgrade() else { return };
            let Some(tray) = item_rc.downcast::<SystemTrayIcon>() else { return };
            tray.as_pin_ref().rebuild_menu();
        });
    }
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct SystemTrayIcon {
    pub icon: Property<Image>,
    pub tooltip: Property<SharedString>,
    pub title: Property<SharedString>,
    pub visible: Property<bool>,
    pub activated: Callback<VoidArg>,
    pub cached_rendering_data: CachedRenderingData,
    data: SystemTrayIconDataBox,
}

impl SystemTrayIcon {
    /// Called from generated code (via the `SetupSystemTrayIcon` builtin) to hand off the
    /// lowered menu's `VRc<MenuVTable>` to the native item. The item walks the menu via
    /// this vtable inside its own `PropertyTracker`, so property changes inside the menu
    /// tree automatically trigger a rebuild of the platform tray menu. Subsequent calls
    /// replace any previously installed menu.
    pub fn set_menu(
        self: Pin<&Self>,
        self_rc: &ItemRc,
        menu_vrc: vtable::VRc<crate::menus::MenuVTable>,
    ) {
        let tracker = Box::pin(crate::properties::PropertyTracker::new_with_dirty_handler(
            MenuDirtyHandler { self_weak: self_rc.downgrade() },
        ));
        *self.data.menu.borrow_mut() =
            Some(MenuState { menu_vrc, entries: std::vec::Vec::new(), tracker });
        // If the platform tray is already up (icon was set before the menu), populate
        // the menu now; otherwise the icon tracker's notify will call rebuild_menu
        // once the handle exists.
        self.rebuild_menu();
    }

    fn rebuild_menu(self: Pin<&Self>) {
        let Some(handle) = self.data.inner.get() else { return };
        let mut menu_borrow = self.data.menu.borrow_mut();
        let Some(MenuState { menu_vrc, entries, tracker }) = menu_borrow.as_mut() else {
            return;
        };
        tracker.as_ref().evaluate(|| {
            handle.rebuild_menu(vtable::VRc::borrow(menu_vrc), entries);
        });
    }

    /// Reconcile the SlintContext keepalive counter with this tray's state.
    /// A tray contributes to the counter only while it has a live platform
    /// handle and its `visible` property is `true`; everything else is a
    /// no-op so a re-fired tracker can't double-increment.
    fn update_keepalive(self: Pin<&Self>) {
        let want_live = self.data.inner.get().is_some() && self.visible();
        let was_live = self.data.keepalive_live.get();
        if want_live == was_live {
            return;
        }
        let Some(ctx) = crate::context::GLOBAL_CONTEXT.with(|p| p.get().cloned()) else {
            return;
        };
        if want_live {
            ctx.acquire_keepalive();
            self.data.keepalive_live.set(true);
        } else {
            self.data.keepalive_live.set(false);
            ctx.release_keepalive();
        }
    }
}

impl Item for SystemTrayIcon {
    fn init(self: Pin<&Self>, self_rc: &ItemRc) {
        self.data.change_tracker.init_delayed(
            self_rc.downgrade(),
            |_| true,
            |self_weak, has_icon| {
                let Some(tray_rc) = self_weak.upgrade() else {
                    return;
                };
                let Some(tray) = tray_rc.downcast::<SystemTrayIcon>() else {
                    return;
                };
                if !*has_icon {
                    return;
                }
                // The platform is set before any item's `init` runs (the public
                // component's `new` calls `ensure_backend()` first), so the
                // global context is populated by the time this tracker fires
                // from the event loop. SystemTrayIcon has no `WindowAdapter` of
                // its own, so we read the context directly rather than going
                // through `tray_rc.window_adapter()`.
                let Some(ctx) = crate::context::GLOBAL_CONTEXT.with(|p| p.get().cloned()) else {
                    return;
                };
                let tray = tray.as_pin_ref();
                let handle = match SystemTrayIconHandle::new(
                    Params { icon: &tray.icon(), tooltip: &tray.tooltip(), title: &tray.title() },
                    self_weak.clone(),
                    &ctx,
                ) {
                    Ok(handle) => handle,
                    Err(err) => panic!("{}", err),
                };

                let _ = tray.data.inner.set(handle);
                // If a menu was already installed before the icon was set, build it now
                // that we have a platform handle.
                tray.rebuild_menu();
                tray.update_keepalive();
            },
        );

        self.data.visible_tracker.init_delayed(
            self_rc.downgrade(),
            |self_weak| {
                let Some(tray_rc) = self_weak.upgrade() else { return false };
                let Some(tray) = tray_rc.downcast::<SystemTrayIcon>() else { return false };
                tray.as_pin_ref().visible()
            },
            |self_weak, visible| {
                let Some(tray_rc) = self_weak.upgrade() else { return };
                let Some(tray) = tray_rc.downcast::<SystemTrayIcon>() else { return };
                let tray = tray.as_pin_ref();
                if let Some(handle) = tray.data.inner.get() {
                    handle.set_visible(*visible);
                }
                tray.update_keepalive();
                // If the platform handle isn't up yet, the icon-driven init path
                // will create it later and call update_keepalive itself.
            },
        );

        // Push live icon / title changes through to the platform handle. The
        // initial spawn always uses the latest values (the icon-driven init
        // path reads them at fire time), so these trackers are no-ops until
        // the user mutates the property after the tray is up.
        self.data.icon_tracker.init_delayed(
            self_rc.downgrade(),
            |self_weak| {
                let Some(tray_rc) = self_weak.upgrade() else { return Image::default() };
                let Some(tray) = tray_rc.downcast::<SystemTrayIcon>() else {
                    return Image::default();
                };
                tray.as_pin_ref().icon()
            },
            |self_weak, icon| {
                let Some(tray_rc) = self_weak.upgrade() else { return };
                let Some(tray) = tray_rc.downcast::<SystemTrayIcon>() else { return };
                if let Some(handle) = tray.as_pin_ref().data.inner.get() {
                    handle.set_icon(icon);
                }
            },
        );

        self.data.tooltip_tracker.init_delayed(
            self_rc.downgrade(),
            |self_weak| {
                let Some(tray_rc) = self_weak.upgrade() else { return SharedString::default() };
                let Some(tray) = tray_rc.downcast::<SystemTrayIcon>() else {
                    return SharedString::default();
                };
                tray.as_pin_ref().tooltip()
            },
            |self_weak, tooltip| {
                let Some(tray_rc) = self_weak.upgrade() else { return };
                let Some(tray) = tray_rc.downcast::<SystemTrayIcon>() else { return };
                if let Some(handle) = tray.as_pin_ref().data.inner.get() {
                    handle.set_tooltip(tooltip.as_str());
                }
            },
        );

        self.data.title_tracker.init_delayed(
            self_rc.downgrade(),
            |self_weak| {
                let Some(tray_rc) = self_weak.upgrade() else { return SharedString::default() };
                let Some(tray) = tray_rc.downcast::<SystemTrayIcon>() else {
                    return SharedString::default();
                };
                tray.as_pin_ref().title()
            },
            |self_weak, title| {
                let Some(tray_rc) = self_weak.upgrade() else { return };
                let Some(tray) = tray_rc.downcast::<SystemTrayIcon>() else { return };
                if let Some(handle) = tray.as_pin_ref().data.inner.get() {
                    handle.set_title(title.as_str());
                }
            },
        );
    }

    fn deinit(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _cross_axis_constraint: Coord,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        LayoutInfo::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut MouseCursor,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut MouseCursor,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn capture_key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn render(
        self: Pin<&Self>,
        _backend: &mut &mut dyn crate::item_rendering::ItemRenderer,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren
    }

    fn bounding_rect(
        self: core::pin::Pin<&Self>,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        geometry: LogicalRect,
    ) -> LogicalRect {
        geometry
    }

    fn clips_children(self: core::pin::Pin<&Self>) -> bool {
        false
    }
}

impl ItemConsts for SystemTrayIcon {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data().as_unpinned_projection();
}

/// # Safety
/// This must be called using a non-null pointer pointing to a chunk of memory big enough to
/// hold a SystemTrayIconDataBox
#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_system_tray_icon_data_init(data: *mut SystemTrayIconDataBox) {
    unsafe { core::ptr::write(data, SystemTrayIconDataBox::default()) };
}

/// # Safety
/// This must be called using a non-null pointer pointing to an initialized SystemTrayIconDataBox
#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_system_tray_icon_data_free(data: *mut SystemTrayIconDataBox) {
    unsafe { core::ptr::drop_in_place(data) };
}

#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_system_tray_icon_set_menu(
    system_tray: &SystemTrayIcon,
    item_rc: &ItemRc,
    menu_vrc: &vtable::VRc<crate::menus::MenuVTable>,
) {
    unsafe { Pin::new_unchecked(system_tray) }.set_menu(item_rc, menu_vrc.clone());
}
