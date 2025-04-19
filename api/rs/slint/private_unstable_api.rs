// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Module containing the private api that is used by the generated code.
//!
//! This is internal API that shouldn't be used because compatibility is not
//! guaranteed
#![doc(hidden)]

use core::pin::Pin;
use re_exports::*;

// Helper functions called from generated code to reduce code bloat from
// extra copies of the original functions for each call site due to
// the impl Fn() they are taking.

pub trait StrongItemTreeRef: Sized {
    type Weak: Clone + 'static;
    fn to_weak(&self) -> Self::Weak;
    fn from_weak(weak: &Self::Weak) -> Option<Self>;
}

impl<C: 'static> StrongItemTreeRef for VRc<ItemTreeVTable, C> {
    type Weak = VWeak<ItemTreeVTable, C>;
    fn to_weak(&self) -> Self::Weak {
        VRc::downgrade(self)
    }
    fn from_weak(weak: &Self::Weak) -> Option<Self> {
        weak.upgrade()
    }
}

impl<C: 'static> StrongItemTreeRef for VRcMapped<ItemTreeVTable, C> {
    type Weak = VWeakMapped<ItemTreeVTable, C>;
    fn to_weak(&self) -> Self::Weak {
        VRcMapped::downgrade(self)
    }
    fn from_weak(weak: &Self::Weak) -> Option<Self> {
        weak.upgrade()
    }
}

impl<C: 'static> StrongItemTreeRef for Pin<Rc<C>> {
    type Weak = PinWeak<C>;
    fn to_weak(&self) -> Self::Weak {
        PinWeak::downgrade(self.clone())
    }
    fn from_weak(weak: &Self::Weak) -> Option<Self> {
        weak.upgrade()
    }
}

pub fn set_property_binding<
    T: Clone + Default + 'static,
    StrongRef: StrongItemTreeRef + 'static,
>(
    property: Pin<&Property<T>>,
    component_strong: &StrongRef,
    binding: fn(StrongRef) -> T,
) {
    let weak = component_strong.to_weak();
    property.set_binding(move || {
        <StrongRef as StrongItemTreeRef>::from_weak(&weak).map(binding).unwrap_or_default()
    })
}

pub fn set_animated_property_binding<
    T: Clone + i_slint_core::properties::InterpolatedPropertyValue + 'static,
    StrongRef: StrongItemTreeRef + 'static,
>(
    property: Pin<&Property<T>>,
    component_strong: &StrongRef,
    binding: fn(StrongRef) -> T,
    animation_data: PropertyAnimation,
) {
    let weak = component_strong.to_weak();
    property.set_animated_binding(
        move || binding(<StrongRef as StrongItemTreeRef>::from_weak(&weak).unwrap()),
        animation_data,
    )
}

pub fn set_animated_property_binding_for_transition<
    T: Clone + i_slint_core::properties::InterpolatedPropertyValue + 'static,
    StrongRef: StrongItemTreeRef + 'static,
>(
    property: Pin<&Property<T>>,
    component_strong: &StrongRef,
    binding: fn(StrongRef) -> T,
    compute_animation_details: fn(
        StrongRef,
    ) -> (PropertyAnimation, i_slint_core::animations::Instant),
) {
    let weak_1 = component_strong.to_weak();
    let weak_2 = weak_1.clone();
    property.set_animated_binding_for_transition(
        move || binding(<StrongRef as StrongItemTreeRef>::from_weak(&weak_1).unwrap()),
        move || {
            compute_animation_details(<StrongRef as StrongItemTreeRef>::from_weak(&weak_2).unwrap())
        },
    )
}

pub fn set_property_state_binding<StrongRef: StrongItemTreeRef + 'static>(
    property: Pin<&Property<StateInfo>>,
    component_strong: &StrongRef,
    binding: fn(StrongRef) -> i32,
) {
    let weak = component_strong.to_weak();
    re_exports::set_state_binding(property, move || {
        binding(<StrongRef as StrongItemTreeRef>::from_weak(&weak).unwrap())
    })
}

pub fn set_callback_handler<
    Arg: ?Sized + 'static,
    Ret: Default + 'static,
    StrongRef: StrongItemTreeRef + 'static,
>(
    callback: Pin<&Callback<Arg, Ret>>,
    component_strong: &StrongRef,
    handler: fn(StrongRef, &Arg) -> Ret,
) {
    let weak = component_strong.to_weak();
    callback.set_handler(move |arg| {
        handler(<StrongRef as StrongItemTreeRef>::from_weak(&weak).unwrap(), arg)
    })
}

pub fn debug(s: SharedString) {
    #[cfg(feature = "log")]
    log::debug!("{s}");
    #[cfg(not(feature = "log"))]
    i_slint_core::debug_log!("{s}");
}

pub fn ensure_backend() -> Result<(), crate::PlatformError> {
    i_slint_backend_selector::with_platform(|_b| {
        // Nothing to do, just make sure a backend was created
        Ok(())
    })
}

/// Creates a new window to render components in.
pub fn create_window_adapter(
) -> Result<alloc::rc::Rc<dyn i_slint_core::window::WindowAdapter>, crate::PlatformError> {
    i_slint_backend_selector::with_platform(|b| b.create_window_adapter())
}

/// Wrapper around i_slint_core::translations::translate for the generated code
pub fn translate(
    origin: SharedString,
    context: SharedString,
    domain: SharedString,
    args: Slice<SharedString>,
    n: i32,
    plural: SharedString,
) -> SharedString {
    i_slint_core::translations::translate(&origin, &context, &domain, args.as_slice(), n, &plural)
}

#[cfg(feature = "gettext")]
pub fn init_translations(domain: &str, dirname: impl Into<std::path::PathBuf>) {
    i_slint_core::translations::gettext_bindtextdomain(domain, dirname.into()).unwrap()
}

pub fn use_24_hour_format() -> bool {
    i_slint_core::date_time::use_24_hour_format()
}

/// internal re_exports used by the macro generated
pub mod re_exports {
    pub use alloc::boxed::Box;
    pub use alloc::rc::{Rc, Weak};
    pub use alloc::string::String;
    pub use alloc::{vec, vec::Vec};
    pub use const_field_offset::{self, FieldOffsets, PinnedDrop};
    pub use core::iter::FromIterator;
    pub use core::option::{Option, Option::*};
    pub use core::result::{Result, Result::*};
    pub use i_slint_core::format;
    // This one is empty when Qt is not available, which triggers a warning
    pub use euclid::approxeq::ApproxEq;
    #[allow(unused_imports)]
    pub use i_slint_backend_selector::native_widgets::*;
    pub use i_slint_core::accessibility::{
        AccessibilityAction, AccessibleStringProperty, SupportedAccessibilityAction,
    };
    pub use i_slint_core::animations::{animation_tick, EasingCurve};
    pub use i_slint_core::api::LogicalPosition;
    pub use i_slint_core::callbacks::Callback;
    pub use i_slint_core::date_time::*;
    pub use i_slint_core::detect_operating_system;
    pub use i_slint_core::graphics::*;
    pub use i_slint_core::input::{
        key_codes::Key, FocusEvent, InputEventResult, KeyEvent, KeyEventResult, KeyboardModifiers,
        MouseEvent,
    };
    pub use i_slint_core::item_tree::{
        register_item_tree, unregister_item_tree, IndexRange, ItemTree, ItemTreeRefPin,
        ItemTreeVTable, ItemTreeWeak,
    };
    pub use i_slint_core::item_tree::{
        visit_item_tree, ItemTreeNode, ItemVisitorRefMut, ItemVisitorVTable, ItemWeak,
        TraversalOrder, VisitChildrenResult,
    };
    pub use i_slint_core::items::*;
    pub use i_slint_core::layout::*;
    pub use i_slint_core::lengths::{
        logical_position_to_api, LogicalLength, LogicalPoint, LogicalRect,
    };
    pub use i_slint_core::menus::{Menu, MenuFromItemTree, MenuVTable};
    pub use i_slint_core::model::*;
    pub use i_slint_core::properties::{
        set_state_binding, ChangeTracker, Property, PropertyTracker, StateInfo,
    };
    pub use i_slint_core::slice::Slice;
    pub use i_slint_core::string::shared_string_from_number;
    pub use i_slint_core::string::shared_string_from_number_fixed;
    pub use i_slint_core::string::shared_string_from_number_precision;
    pub use i_slint_core::timers::{Timer, TimerMode};
    pub use i_slint_core::translations::{
        set_bundled_languages, translate_from_bundle, translate_from_bundle_with_plural,
    };
    pub use i_slint_core::window::{
        InputMethodRequest, WindowAdapter, WindowAdapterRc, WindowInner,
    };
    pub use i_slint_core::{Color, Coord, SharedString, SharedVector};
    pub use i_slint_core::{ItemTreeVTable_static, MenuVTable_static};
    pub use num_traits::float::Float;
    pub use num_traits::ops::euclid::Euclid;
    pub use once_cell::race::OnceBox;
    pub use once_cell::unsync::OnceCell;
    pub use pin_weak::rc::PinWeak;
    pub use unicode_segmentation::UnicodeSegmentation;
    pub use vtable::{self, *};
}
