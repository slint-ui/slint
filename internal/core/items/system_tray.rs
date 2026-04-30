// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! System tray integration.
//!
//! This module hosts the `SystemTray` native item (the element exposed to `.slint`) and
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
pub struct SystemTrayHandle(PlatformTray);

impl SystemTrayHandle {
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

    pub fn set_title(&self, title: &str) {
        self.0.set_title(title);
    }
}

// ---------------------------------------------------------------------------
// Native `SystemTray` item, exposed to `.slint`.
// ---------------------------------------------------------------------------

#[repr(C)]
/// Wraps the internal data structure for the SystemTray
pub struct SystemTrayDataBox(core::ptr::NonNull<SystemTrayData>);

impl Default for SystemTrayDataBox {
    fn default() -> Self {
        SystemTrayDataBox(Box::leak(Box::<SystemTrayData>::default()).into())
    }
}
impl Drop for SystemTrayDataBox {
    fn drop(&mut self) {
        // Safety: the self.0 was constructed from a Box::leak in SystemTrayDataBox::default
        drop(unsafe { Box::from_raw(self.0.as_ptr()) });
    }
}

impl core::ops::Deref for SystemTrayDataBox {
    type Target = SystemTrayData;
    fn deref(&self) -> &Self::Target {
        // Safety: initialized in SystemTrayDataBox::default
        unsafe { self.0.as_ref() }
    }
}

#[derive(Default)]
pub struct SystemTrayData {
    inner: std::cell::OnceCell<SystemTrayHandle>,
    change_tracker: crate::properties::ChangeTracker,
    visible_tracker: crate::properties::ChangeTracker,
    icon_tracker: crate::properties::ChangeTracker,
    title_tracker: crate::properties::ChangeTracker,
    menu: core::cell::RefCell<Option<MenuState>>,
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
            let Some(tray) = item_rc.downcast::<SystemTray>() else { return };
            tray.as_pin_ref().rebuild_menu();
        });
    }
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct SystemTray {
    pub icon: Property<Image>,
    pub title: Property<SharedString>,
    pub visible: Property<bool>,
    pub activated: Callback<VoidArg>,
    pub cached_rendering_data: CachedRenderingData,
    data: SystemTrayDataBox,
}

impl SystemTray {
    /// Called from generated code (via the `SetupSystemTray` builtin) to hand off the
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
}

impl Item for SystemTray {
    fn init(self: Pin<&Self>, self_rc: &ItemRc) {
        self.data.change_tracker.init_delayed(
            self_rc.downgrade(),
            |_| true,
            |self_weak, has_icon| {
                let Some(tray_rc) = self_weak.upgrade() else {
                    return;
                };
                let Some(tray) = tray_rc.downcast::<SystemTray>() else {
                    return;
                };
                if !*has_icon {
                    return;
                }
                // The change tracker fires from the Slint event loop once the item tree
                // is mapped to a window, so this usually resolves. If it somehow doesn't,
                // the tracker will re-fire on the next property change and try again.
                let Some(adapter) = tray_rc.window_adapter() else { return };
                let ctx = crate::window::WindowInner::from_pub(adapter.window()).context().clone();
                let tray = tray.as_pin_ref();
                let handle = match SystemTrayHandle::new(
                    Params { icon: &tray.icon(), title: &tray.title() },
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
            },
        );

        self.data.visible_tracker.init_delayed(
            self_rc.downgrade(),
            |self_weak| {
                let Some(tray_rc) = self_weak.upgrade() else { return false };
                let Some(tray) = tray_rc.downcast::<SystemTray>() else { return false };
                tray.as_pin_ref().visible()
            },
            |self_weak, visible| {
                let Some(tray_rc) = self_weak.upgrade() else { return };
                let Some(tray) = tray_rc.downcast::<SystemTray>() else { return };
                if let Some(handle) = tray.as_pin_ref().data.inner.get() {
                    handle.set_visible(*visible);
                }
                // If the platform handle isn't up yet, the icon-driven init path
                // will create it later; the next change-tracker fire (or the
                // initial visibility burned in at creation, once that's wired
                // up properly) takes care of the visible state.
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
                let Some(tray) = tray_rc.downcast::<SystemTray>() else { return Image::default() };
                tray.as_pin_ref().icon()
            },
            |self_weak, icon| {
                let Some(tray_rc) = self_weak.upgrade() else { return };
                let Some(tray) = tray_rc.downcast::<SystemTray>() else { return };
                if let Some(handle) = tray.as_pin_ref().data.inner.get() {
                    handle.set_icon(icon);
                }
            },
        );

        self.data.title_tracker.init_delayed(
            self_rc.downgrade(),
            |self_weak| {
                let Some(tray_rc) = self_weak.upgrade() else { return SharedString::default() };
                let Some(tray) = tray_rc.downcast::<SystemTray>() else {
                    return SharedString::default();
                };
                tray.as_pin_ref().title()
            },
            |self_weak, title| {
                let Some(tray_rc) = self_weak.upgrade() else { return };
                let Some(tray) = tray_rc.downcast::<SystemTray>() else { return };
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

impl ItemConsts for SystemTray {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data().as_unpinned_projection();
}

/// # Safety
/// This must be called using a non-null pointer pointing to a chunk of memory big enough to
/// hold a SystemTrayDataBox
#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_system_tray_data_init(data: *mut SystemTrayDataBox) {
    unsafe { core::ptr::write(data, SystemTrayDataBox::default()) };
}

/// # Safety
/// This must be called using a non-null pointer pointing to an initialized SystemTrayDataBox
#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_system_tray_data_free(data: *mut SystemTrayDataBox) {
    unsafe { core::ptr::drop_in_place(data) };
}

#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_system_tray_set_menu(
    system_tray: &SystemTray,
    item_rc: &ItemRc,
    menu_vrc: &vtable::VRc<crate::menus::MenuVTable>,
) {
    unsafe { Pin::new_unchecked(system_tray) }.set_menu(item_rc, menu_vrc.clone());
}
