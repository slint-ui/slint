// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! System tray integration.
//!
//! This module hosts the `SystemTray` native item (the element exposed to `.slint`) and
//! wraps the platform-specific tray icon backends: [`ksni`](ksni) on Linux/BSD and
//! [`tray-icon`](tray_icon) (muda-based) on macOS and Windows.

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

#[cfg(all(feature = "system-tray", not(any(target_os = "macos", target_os = "windows"))))]
mod ksni;
#[cfg(all(feature = "system-tray", any(target_os = "macos", target_os = "windows")))]
mod tray_icon;

#[cfg(all(feature = "system-tray", not(any(target_os = "macos", target_os = "windows"))))]
use self::ksni::PlatformTray;
#[cfg(all(feature = "system-tray", any(target_os = "macos", target_os = "windows")))]
use self::tray_icon::PlatformTray;

/// Parameters passed to the platform-specific tray backend when building a tray icon.
pub struct Params<'a> {
    pub icon: &'a Image,
    pub title: &'a str,
}

/// Errors raised while constructing a platform tray icon.
#[derive(Debug, derive_more::Error, derive_more::Display)]
pub enum Error {
    #[display("Failed to create a rgba8 buffer from an icon image")]
    Rgba8,
    #[cfg(all(feature = "system-tray", any(target_os = "macos", target_os = "windows")))]
    #[display("Bad icon: {}", 0)]
    BadIcon(::tray_icon::BadIcon),
    #[cfg(all(feature = "system-tray", any(target_os = "macos", target_os = "windows")))]
    #[display("Build error: {}", 0)]
    BuildError(::tray_icon::Error),
    #[display("{}", 0)]
    PlatformError(crate::platform::PlatformError),
    #[display("{}", 0)]
    EventLoopError(crate::api::EventLoopError),
}

/// Owning handle to a live platform tray icon. Dropping it removes the icon.
#[cfg(feature = "system-tray")]
pub struct SystemTrayHandle(#[allow(dead_code)] PlatformTray);

#[cfg(feature = "system-tray")]
impl SystemTrayHandle {
    pub fn new(params: Params) -> Result<Self, Error> {
        PlatformTray::new(params).map(Self)
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
    #[cfg(feature = "system-tray")]
    inner: std::cell::OnceCell<SystemTrayHandle>,
    #[cfg_attr(not(feature = "system-tray"), allow(unused))]
    change_tracker: crate::properties::ChangeTracker,
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

impl Item for SystemTray {
    #[cfg_attr(not(feature = "system-tray"), allow(unused))]
    fn init(self: Pin<&Self>, self_rc: &ItemRc) {
        #[cfg(feature = "system-tray")]
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
