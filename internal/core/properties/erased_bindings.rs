// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Install property bindings and callback handlers on a component without
//! monomorphizing the machinery per component type.
//!
//! The generated code of every component would otherwise instantiate its own
//! copy of the binding holder machinery for each property type it uses. The
//! functions in this module erase the component type at the boundary: the
//! component is kept behind a [`VWeakMappedErased`] and the given function
//! pointer is transmuted to take the erased pointer. Everything past the
//! `#[inline]` shims is monomorphized per property type only.
//!
//! # Soundness
//!
//! The public functions are safe for any well-typed arguments: their
//! signatures tie the component type `X` between the strong reference and the
//! function pointer, so a mismatched pairing is a compile error. The erasure
//! is sound because:
//! - `Pin<&X>` is `repr(transparent)` over `&X`, and `&X` (for sized `X`) and
//!   `*const ()` are ABI-compatible function-pointer parameter types, so
//!   calling the transmuted function pointer is defined behavior.
//! - The pointer passed to it at call time is the pointer
//!   [`VRcMapped::downgrade_erased`] erased from the `*const X` of the strong
//!   reference, only cast through raw pointers on the way (no intermediate
//!   reference is formed), so its provenance covers all of `X`.
//! - [`VWeakMappedErased::with_upgraded`] holds the upgraded strong reference
//!   for the duration of the call, so the object is alive and pinned while
//!   the function runs.

use super::Property;
use crate::callbacks::Callback;
use core::pin::Pin;
use vtable::{VRcMapped, VTableMetaDropInPlace, VWeakMappedErased};

/// Erase the component argument of a function pointer.
///
/// Private to this module: the result must only be called with a pointer
/// erased from a valid, pinned `X`, which the public functions of this
/// module guarantee. See the module documentation for why the transmute and
/// the later call are defined behavior.
fn erase<X, R>(f: fn(Pin<&X>) -> R) -> fn(*const ()) -> R {
    // SAFETY: see the module documentation.
    unsafe { core::mem::transmute(f) }
}

/// Like [`erase`], for a function pointer with an extra argument.
fn erase2<X, A: ?Sized, R>(f: fn(Pin<&X>, &A) -> R) -> fn(*const (), &A) -> R {
    // SAFETY: see the module documentation.
    unsafe { core::mem::transmute(f) }
}

/// Sets the binding of `property` to evaluate `binding` on the component
/// `self_rc`, or to produce the default value once the component is gone.
#[inline]
pub fn set_property_binding_erased<
    T: Clone + Default + 'static,
    VT: VTableMetaDropInPlace + 'static,
    X,
>(
    property: Pin<&Property<T>>,
    self_rc: &VRcMapped<VT, X>,
    binding: fn(Pin<&X>) -> T,
) {
    let weak = VRcMapped::downgrade_erased(self_rc);
    set_property_binding_impl(property, weak, erase(binding))
}

fn set_property_binding_impl<T: Clone + Default + 'static, VT: VTableMetaDropInPlace + 'static>(
    property: Pin<&Property<T>>,
    weak: VWeakMappedErased<VT>,
    binding: fn(*const ()) -> T,
) {
    property.set_binding(move || weak.with_upgraded(binding).unwrap_or_default())
}

/// Like [`set_property_binding_erased`], for an animated binding.
/// The component must outlive the binding.
#[inline]
pub fn set_animated_property_binding_erased<
    T: Clone + super::InterpolatedPropertyValue + 'static,
    VT: VTableMetaDropInPlace + 'static,
    X,
>(
    property: Pin<&Property<T>>,
    self_rc: &VRcMapped<VT, X>,
    binding: fn(Pin<&X>) -> T,
    compute_animation_details: fn(
        Pin<&X>,
    ) -> (
        crate::items::PropertyAnimation,
        Option<crate::animations::Instant>,
    ),
) {
    let weak = VRcMapped::downgrade_erased(self_rc);
    set_animated_property_binding_impl(
        property,
        weak,
        erase(binding),
        erase(compute_animation_details),
    )
}

fn set_animated_property_binding_impl<
    T: Clone + super::InterpolatedPropertyValue + 'static,
    VT: VTableMetaDropInPlace + 'static,
>(
    property: Pin<&Property<T>>,
    weak: VWeakMappedErased<VT>,
    binding: fn(*const ()) -> T,
    compute_animation_details: fn(
        *const (),
    ) -> (
        crate::items::PropertyAnimation,
        Option<crate::animations::Instant>,
    ),
) {
    let weak2 = weak.clone();
    property.set_animated_binding(
        move || weak.with_upgraded(binding).expect("binding evaluated on dropped component"),
        move || {
            weak2
                .with_upgraded(compute_animation_details)
                .expect("binding evaluated on dropped component")
        },
    )
}

/// Like [`set_property_binding_erased`], for a state binding.
/// The component must outlive the binding.
#[inline]
pub fn set_property_state_binding_erased<VT: VTableMetaDropInPlace + 'static, X>(
    property: Pin<&Property<super::StateInfo>>,
    self_rc: &VRcMapped<VT, X>,
    binding: fn(Pin<&X>) -> i32,
) {
    let weak = VRcMapped::downgrade_erased(self_rc);
    set_property_state_binding_impl(property, weak, erase(binding))
}

fn set_property_state_binding_impl<VT: VTableMetaDropInPlace + 'static>(
    property: Pin<&Property<super::StateInfo>>,
    weak: VWeakMappedErased<VT>,
    binding: fn(*const ()) -> i32,
) {
    super::set_state_binding(property, move || {
        weak.with_upgraded(binding).expect("binding evaluated on dropped component")
    })
}

/// Initialize `change_tracker` to evaluate `eval` on the component `self_rc`
/// and call `notify` when the result changes. The component must outlive the
/// change tracker.
#[inline]
pub fn change_tracker_init_erased<
    T: Default + PartialEq + 'static,
    VT: VTableMetaDropInPlace + 'static,
    X,
>(
    change_tracker: &super::ChangeTracker,
    self_rc: &VRcMapped<VT, X>,
    eval: fn(Pin<&X>) -> T,
    notify: fn(Pin<&X>, &T),
) {
    let weak = VRcMapped::downgrade_erased(self_rc);
    change_tracker_init_impl(change_tracker, weak, erase(eval), erase2(notify))
}

fn change_tracker_init_impl<
    T: Default + PartialEq + 'static,
    VT: VTableMetaDropInPlace + 'static,
>(
    change_tracker: &super::ChangeTracker,
    weak: VWeakMappedErased<VT>,
    eval: fn(*const ()) -> T,
    notify: fn(*const (), &T),
) {
    change_tracker.init(
        weak,
        move |weak| weak.with_upgraded(eval).expect("change tracker on dropped component"),
        move |weak, value| {
            weak.with_upgraded(|object| notify(object, value))
                .expect("change tracker on dropped component");
        },
    )
}

/// Sets the handler of `callback` to evaluate `handler` on the component
/// `self_rc`. The component must outlive the callback handler.
#[inline]
pub fn set_callback_handler_erased<
    Arg: ?Sized + 'static,
    Ret: Default + 'static,
    VT: VTableMetaDropInPlace + 'static,
    X,
>(
    callback: Pin<&Callback<Arg, Ret>>,
    self_rc: &VRcMapped<VT, X>,
    handler: fn(Pin<&X>, &Arg) -> Ret,
) {
    let weak = VRcMapped::downgrade_erased(self_rc);
    set_callback_handler_impl(callback, weak, erase2(handler))
}

fn set_callback_handler_impl<
    Arg: ?Sized + 'static,
    Ret: Default + 'static,
    VT: VTableMetaDropInPlace + 'static,
>(
    callback: Pin<&Callback<Arg, Ret>>,
    weak: VWeakMappedErased<VT>,
    handler: fn(*const (), &Arg) -> Ret,
) {
    callback.set_handler(move |arg| {
        weak.with_upgraded(|object| handler(object, arg))
            .expect("callback invoked on a dropped component")
    })
}
