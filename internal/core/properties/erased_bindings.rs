// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Install property bindings and callback handlers on a component without
//! monomorphizing the machinery per component type.
//!
//! The generated code of every component would otherwise instantiate its own
//! copy of the binding holder machinery for each property type it uses. The
//! functions here keep the component behind an [`ErasedWeakFn`], which pairs a
//! type-erased weak reference with the binding function so that everything past
//! the `#[inline]` shims is monomorphized per property type only, not per
//! component. The erasure and its safety live in the `vtable` crate; this module
//! is entirely safe.
//!
//! The binding functions all take an extra `&Arg` (`&()` for those without a
//! meaningful argument), so that erasing them never changes the function's arity.
//! For property bindings, it's the [`PropertyContext`] they're evaluated in.

use super::{Property, PropertyContext, WithContext};
use crate::callbacks::Callback;
use core::pin::Pin;
use vtable::{ErasedWeakFn, VRcMapped, VTableMetaDropInPlace};

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
    binding: fn(Pin<&X>, &PropertyContext) -> T,
) {
    set_property_binding_impl(property, VRcMapped::downgrade_erased_fn(self_rc, binding))
}

fn set_property_binding_impl<T: Clone + Default + 'static, VT: VTableMetaDropInPlace + 'static>(
    property: Pin<&Property<T>>,
    binding: ErasedWeakFn<VT, T, PropertyContext>,
) {
    property.set_binding(WithContext(move |ctx: &PropertyContext| {
        binding.upgrade_and_call(ctx).unwrap_or_default()
    }))
}

/// Like [`set_property_binding_erased`], for an animated binding.
#[inline]
pub fn set_animated_property_binding_erased<
    T: Clone + super::InterpolatedPropertyValue + 'static,
    VT: VTableMetaDropInPlace + 'static,
    X,
>(
    property: Pin<&Property<T>>,
    driver: &crate::animations::AnimationDriver,
    self_rc: &VRcMapped<VT, X>,
    binding: fn(Pin<&X>, &PropertyContext) -> T,
    compute_animation_details: fn(
        Pin<&X>,
        &(),
    ) -> (
        crate::items::PropertyAnimation,
        Option<crate::animations::Instant>,
    ),
) {
    set_animated_property_binding_impl(
        property,
        driver,
        VRcMapped::downgrade_erased_fn(self_rc, binding),
        VRcMapped::downgrade_erased_fn(self_rc, compute_animation_details),
    )
}

fn set_animated_property_binding_impl<
    T: Clone + super::InterpolatedPropertyValue + 'static,
    VT: VTableMetaDropInPlace + 'static,
>(
    property: Pin<&Property<T>>,
    driver: &crate::animations::AnimationDriver,
    binding: ErasedWeakFn<VT, T, PropertyContext>,
    compute_animation_details: ErasedWeakFn<
        VT,
        (crate::items::PropertyAnimation, Option<crate::animations::Instant>),
    >,
) {
    property.set_animated_binding(
        driver,
        WithContext(move |ctx: &PropertyContext| binding.upgrade_and_call(ctx).unwrap_or_default()),
        move || compute_animation_details.upgrade_and_call(&()).unwrap_or_default(),
    )
}

/// Like [`set_property_binding_erased`], for a state binding.
/// The component must outlive the binding.
#[inline]
pub fn set_property_state_binding_erased<VT: VTableMetaDropInPlace + 'static, X>(
    property: Pin<&Property<super::StateInfo>>,
    driver: &crate::animations::AnimationDriver,
    self_rc: &VRcMapped<VT, X>,
    binding: fn(Pin<&X>, &PropertyContext) -> i32,
) {
    set_property_state_binding_impl(
        property,
        driver,
        VRcMapped::downgrade_erased_fn(self_rc, binding),
    )
}

fn set_property_state_binding_impl<VT: VTableMetaDropInPlace + 'static>(
    property: Pin<&Property<super::StateInfo>>,
    driver: &crate::animations::AnimationDriver,
    binding: ErasedWeakFn<VT, i32, PropertyContext>,
) {
    super::set_state_binding(
        property,
        driver,
        WithContext(move |ctx: &PropertyContext| {
            binding.upgrade_and_call(ctx).expect("binding evaluated on dropped component")
        }),
    )
}

/// Initialize `change_tracker` to evaluate `eval` on the component `self_rc`
/// and call `notify` from `queue` when the result changes.
/// The component must outlive the change tracker.
#[inline]
pub fn change_tracker_init_erased<
    T: Default + PartialEq + 'static,
    VT: VTableMetaDropInPlace + 'static,
    X,
>(
    change_tracker: &super::ChangeTracker,
    queue: &super::ChangeHandlerQueue,
    self_rc: &VRcMapped<VT, X>,
    eval: fn(Pin<&X>, &PropertyContext) -> T,
    notify: fn(Pin<&X>, &T),
) {
    change_tracker_init_impl(
        change_tracker,
        queue,
        VRcMapped::downgrade_erased_fn(self_rc, eval),
        VRcMapped::downgrade_erased_fn(self_rc, notify),
    )
}

fn change_tracker_init_impl<
    T: Default + PartialEq + 'static,
    VT: VTableMetaDropInPlace + 'static,
>(
    change_tracker: &super::ChangeTracker,
    queue: &super::ChangeHandlerQueue,
    eval: ErasedWeakFn<VT, T, PropertyContext>,
    notify: ErasedWeakFn<VT, (), T>,
) {
    change_tracker.init_with_context(
        queue,
        (eval, notify),
        |(eval, _), ctx| eval.upgrade_and_call(ctx).expect("change tracker on dropped component"),
        |(_, notify), value| {
            notify.upgrade_and_call(value).expect("change tracker on dropped component");
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
    set_callback_handler_impl(callback, VRcMapped::downgrade_erased_fn(self_rc, handler))
}

fn set_callback_handler_impl<
    Arg: ?Sized + 'static,
    Ret: Default + 'static,
    VT: VTableMetaDropInPlace + 'static,
>(
    callback: Pin<&Callback<Arg, Ret>>,
    handler: ErasedWeakFn<VT, Ret, Arg>,
) {
    callback.set_handler(move |arg| {
        handler.upgrade_and_call(arg).expect("callback invoked on a dropped component")
    })
}
