// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Module containing the private api that is used by the generated code.
//!
//! This is internal API that shouldn't be used because compatibility is not
//! guaranteed
#![doc(hidden)]

use alloc::rc::Rc;
use core::pin::Pin;
use re_exports::*;

// Helper functions called from generated code to reduce code bloat from
// extra copies of the original functions for each call site due to
// the impl Fn() they are taking.

pub trait StrongComponentRef: Sized {
    type Weak: Clone + 'static;
    fn to_weak(&self) -> Self::Weak;
    fn from_weak(weak: &Self::Weak) -> Option<Self>;
}

impl<C: 'static> StrongComponentRef for VRc<ComponentVTable, C> {
    type Weak = VWeak<ComponentVTable, C>;
    fn to_weak(&self) -> Self::Weak {
        VRc::downgrade(self)
    }
    fn from_weak(weak: &Self::Weak) -> Option<Self> {
        weak.upgrade()
    }
}

impl<C: 'static> StrongComponentRef for VRcMapped<ComponentVTable, C> {
    type Weak = VWeakMapped<ComponentVTable, C>;
    fn to_weak(&self) -> Self::Weak {
        VRcMapped::downgrade(self)
    }
    fn from_weak(weak: &Self::Weak) -> Option<Self> {
        weak.upgrade()
    }
}

impl<C: 'static> StrongComponentRef for Pin<Rc<C>> {
    type Weak = PinWeak<C>;
    fn to_weak(&self) -> Self::Weak {
        PinWeak::downgrade(self.clone())
    }
    fn from_weak(weak: &Self::Weak) -> Option<Self> {
        weak.upgrade()
    }
}

pub fn set_property_binding<T: Clone + 'static, StrongRef: StrongComponentRef + 'static>(
    property: Pin<&Property<T>>,
    component_strong: &StrongRef,
    binding: fn(StrongRef) -> T,
) {
    let weak = component_strong.to_weak();
    property
        .set_binding(move || binding(<StrongRef as StrongComponentRef>::from_weak(&weak).unwrap()))
}

pub fn set_animated_property_binding<
    T: Clone + i_slint_core::properties::InterpolatedPropertyValue + 'static,
    StrongRef: StrongComponentRef + 'static,
>(
    property: Pin<&Property<T>>,
    component_strong: &StrongRef,
    binding: fn(StrongRef) -> T,
    animation_data: PropertyAnimation,
) {
    let weak = component_strong.to_weak();
    property.set_animated_binding(
        move || binding(<StrongRef as StrongComponentRef>::from_weak(&weak).unwrap()),
        animation_data,
    )
}

pub fn set_animated_property_binding_for_transition<
    T: Clone + i_slint_core::properties::InterpolatedPropertyValue + 'static,
    StrongRef: StrongComponentRef + 'static,
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
        move || binding(<StrongRef as StrongComponentRef>::from_weak(&weak_1).unwrap()),
        move || {
            compute_animation_details(
                <StrongRef as StrongComponentRef>::from_weak(&weak_2).unwrap(),
            )
        },
    )
}

pub fn set_property_state_binding<StrongRef: StrongComponentRef + 'static>(
    property: Pin<&Property<StateInfo>>,
    component_strong: &StrongRef,
    binding: fn(StrongRef) -> i32,
) {
    let weak = component_strong.to_weak();
    re_exports::set_state_binding(property, move || {
        binding(<StrongRef as StrongComponentRef>::from_weak(&weak).unwrap())
    })
}

pub fn set_callback_handler<
    Arg: ?Sized + 'static,
    Ret: Default + 'static,
    StrongRef: StrongComponentRef + 'static,
>(
    callback: Pin<&Callback<Arg, Ret>>,
    component_strong: &StrongRef,
    handler: fn(StrongRef, &Arg) -> Ret,
) {
    let weak = component_strong.to_weak();
    callback.set_handler(move |arg| {
        handler(<StrongRef as StrongComponentRef>::from_weak(&weak).unwrap(), arg)
    })
}

pub fn debug(s: SharedString) {
    #[cfg(feature = "log")]
    log::debug!("{s}");
    #[cfg(not(feature = "log"))]
    {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        println!("{s}");
        #[cfg(any(not(feature = "std"), target_arch = "wasm32"))]
        i_slint_core::debug_log!("{s}");
    }
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
) -> SharedString {
    i_slint_core::translations::translate(&origin, &context, &domain, args.as_slice())
}

#[cfg(feature = "gettext")]
pub fn init_translations(domain: &str, dirname: impl Into<std::path::PathBuf>) {
    i_slint_core::translations::gettext_bindtextdomain(domain, dirname.into()).unwrap()
}

/// internal re_exports used by the macro generated
pub mod re_exports {
    pub use alloc::boxed::Box;
    pub use alloc::format;
    pub use alloc::rc::{Rc, Weak};
    pub use alloc::string::String;
    pub use alloc::{vec, vec::Vec};
    pub use const_field_offset::{self, FieldOffsets, PinnedDrop};
    pub use core::iter::FromIterator;
    pub use i_slint_backend_selector::native_widgets::*;
    pub use i_slint_core::accessibility::AccessibleStringProperty;
    pub use i_slint_core::animations::{animation_tick, EasingCurve};
    pub use i_slint_core::callbacks::Callback;
    pub use i_slint_core::component::{
        register_component, unregister_component, Component, ComponentRefPin, ComponentVTable,
        ComponentWeak, IndexRange,
    };
    pub use i_slint_core::graphics::*;
    pub use i_slint_core::input::{
        key_codes::Key, FocusEvent, InputEventResult, KeyEvent, KeyEventResult, KeyboardModifiers,
        MouseEvent,
    };
    pub use i_slint_core::item_tree::{
        visit_item_tree, ItemTreeNode, ItemVisitorRefMut, ItemVisitorVTable, ItemWeak,
        TraversalOrder, VisitChildrenResult,
    };
    pub use i_slint_core::items::*;
    pub use i_slint_core::layout::*;
    pub use i_slint_core::lengths::LogicalLength;
    pub use i_slint_core::model::*;
    pub use i_slint_core::properties::{set_state_binding, Property, PropertyTracker, StateInfo};
    pub use i_slint_core::slice::Slice;
    pub use i_slint_core::window::{InputMethodRequest, WindowAdapter, WindowInner};
    pub use i_slint_core::Color;
    pub use i_slint_core::ComponentVTable_static;
    pub use i_slint_core::Coord;
    pub use i_slint_core::SharedString;
    pub use i_slint_core::SharedVector;
    pub use num_traits::float::Float;
    pub use once_cell::race::OnceBox;
    pub use once_cell::unsync::OnceCell;
    pub use pin_weak::rc::PinWeak;
    pub use vtable::{self, *};
}
