// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! System tray integration.
//!
//! This module hosts the `SystemTray` native item (the element exposed to `.slint`) and
//! wraps the platform-specific tray icon backends: `ksni` on Linux/BSD and
//! `tray-icon` (muda-based) on macOS and Windows.

#![allow(unsafe_code)]

use crate::graphics::Image;
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, InternalKeyEvent,
    KeyEventResult, MouseEvent,
};
use crate::item_rendering::CachedRenderingData;
use crate::items::{Item, ItemConsts, ItemRc, MouseCursor, Orientation, RenderingResult};
use crate::layout::LayoutInfo;
use crate::lengths::{LogicalRect, LogicalSize};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use crate::{Coord, Property, SharedString};
use alloc::boxed::Box;
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use i_slint_core_macros::*;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod ksni;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod tray_icon;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use self::ksni::PlatformTray;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use self::tray_icon::PlatformTray;

std::thread_local! {
    /// Map from `tray_id` to the `SystemTray` item it refers to. Populated by
    /// [`register_tray`] when a menu is installed on a tray; entries are removed when
    /// the owning [`MenuState`] is dropped.
    static TRAYS: core::cell::RefCell<std::collections::HashMap<u64, crate::item_tree::ItemWeak>> =
        core::cell::RefCell::new(std::collections::HashMap::new());
}

/// Register a new platform tray and return its freshly-allocated id. Ids are not
/// reused across trays, so stale ids from platform menus never dispatch to the wrong
/// tray after a tray is dropped and another created.
fn register_tray(self_weak: crate::item_tree::ItemWeak) -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    TRAYS.with(|t| {
        t.borrow_mut().insert(id, self_weak);
    });
    id
}

fn unregister_tray(id: u64) {
    TRAYS.with(|t| {
        t.borrow_mut().remove(&id);
    });
}

fn activate_tray_menu_entry(tray_id: u64, entry_index: usize) {
    let Some(item_weak) = TRAYS.with(|t| t.borrow().get(&tray_id).cloned()) else { return };
    let Some(item_rc) = item_weak.upgrade() else { return };
    let Some(tray) = item_rc.downcast::<SystemTray>() else { return };
    let tray = tray.as_pin_ref();
    let menu_borrow = tray.data.menu.borrow();
    let Some(state) = menu_borrow.as_ref() else { return };
    if let Some(entry) = state.entries.get(entry_index) {
        vtable::VRc::borrow(&state.menu_vrc).activate(entry);
    }
}

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
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[display("Bad icon: {}", 0)]
    BadIcon(::tray_icon::BadIcon),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[display("Build error: {}", 0)]
    BuildError(::tray_icon::Error),
    #[display("{}", 0)]
    PlatformError(crate::platform::PlatformError),
    #[display("{}", 0)]
    EventLoopError(crate::api::EventLoopError),
}

/// Owning handle to a live platform tray icon. Dropping it removes the icon.
pub struct SystemTrayHandle(PlatformTray);

impl SystemTrayHandle {
    pub fn new(params: Params) -> Result<Self, Error> {
        PlatformTray::new(params).map(Self)
    }

    pub fn rebuild_menu(
        &self,
        menu: vtable::VRef<'_, crate::menus::MenuVTable>,
        tray_id: u64,
        entries_out: &mut std::vec::Vec<crate::items::MenuEntry>,
    ) {
        self.0.rebuild_menu(menu, tray_id, entries_out);
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
    menu: core::cell::RefCell<Option<MenuState>>,
}

struct MenuState {
    tray_id: u64,
    menu_vrc: vtable::VRc<crate::menus::MenuVTable>,
    entries: std::vec::Vec<crate::items::MenuEntry>,
    tracker: Pin<Box<crate::properties::PropertyTracker<false, MenuDirtyHandler>>>,
}

impl Drop for MenuState {
    fn drop(&mut self) {
        unregister_tray(self.tray_id);
    }
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
    pub cached_rendering_data: CachedRenderingData,
    data: SystemTrayDataBox,
}

impl SystemTray {
    /// Called from generated code (via the `SetupSystemTray` builtin) to hand off the
    /// lowered menu's `VRc<MenuVTable>` to the native item. The item walks the menu via
    /// this vtable inside its own `PropertyTracker`, so property changes inside the menu
    /// tree automatically trigger a rebuild of the platform tray menu. The menu is also
    /// registered in a thread-local dispatch table so platform click events can route
    /// back through `MenuVTable::activate`. Subsequent calls replace any previously
    /// installed menu.
    pub fn set_menu(
        self: Pin<&Self>,
        self_rc: &ItemRc,
        menu_vrc: vtable::VRc<crate::menus::MenuVTable>,
    ) {
        let tray_id = register_tray(self_rc.downgrade());
        let tracker = Box::pin(crate::properties::PropertyTracker::new_with_dirty_handler(
            MenuDirtyHandler { self_weak: self_rc.downgrade() },
        ));
        // Replacing a previous MenuState drops it, which unregisters the old tray_id
        // via MenuState::drop — safe because registration is keyed by the unique id.
        *self.data.menu.borrow_mut() =
            Some(MenuState { tray_id, menu_vrc, entries: std::vec::Vec::new(), tracker });
        // If the platform tray is already up (icon was set before the menu), populate
        // the menu now; otherwise the icon tracker's notify will call rebuild_menu
        // once the handle exists.
        self.rebuild_menu();
    }

    fn rebuild_menu(self: Pin<&Self>) {
        let Some(handle) = self.data.inner.get() else { return };
        let mut menu_borrow = self.data.menu.borrow_mut();
        let Some(MenuState { tray_id, menu_vrc, entries, tracker }) = menu_borrow.as_mut() else {
            return;
        };
        tracker.as_ref().evaluate(|| {
            handle.rebuild_menu(vtable::VRc::borrow(menu_vrc), *tray_id, entries);
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
                let tray = tray.as_pin_ref();
                let handle = match SystemTrayHandle::new(Params {
                    icon: &tray.icon(),
                    title: &tray.title(),
                }) {
                    Ok(handle) => handle,
                    Err(err) => panic!("{}", err),
                };

                let _ = tray.data.inner.set(handle);
                // If a menu was already installed before the icon was set, build it now
                // that we have a platform handle.
                tray.rebuild_menu();
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
