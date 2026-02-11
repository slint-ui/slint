// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use crate::graphics::{Brush, Color};
use crate::items::PropertyAnimation;

#[allow(non_camel_case_types)]
type c_void = ();
#[repr(C)]
/// Has the same layout as PropertyHandle
pub struct PropertyHandleOpaque(PropertyHandle);

/// Initialize the first pointer of the Property. Does not initialize the content.
/// `out` is assumed to be uninitialized
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_init(out: *mut PropertyHandleOpaque) {
    unsafe { core::ptr::write(out, PropertyHandleOpaque(PropertyHandle::default())) };
}

/// To be called before accessing the value
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_update(handle: &PropertyHandleOpaque, val: *mut c_void) {
    unsafe {
        let handle = Pin::new_unchecked(&handle.0);
        handle.update(val);
        handle.register_as_dependency_to_current_binding();
    }
}

/// Mark the fact that the property was changed and that its binding need to be removed, and
/// the dependencies marked dirty.
/// To be called after the `value` has been changed
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_set_changed(
    handle: &PropertyHandleOpaque,
    value: *const c_void,
) {
    unsafe {
        if !handle.0.access(|b| {
            b.is_some_and(|b| (b.vtable.intercept_set)(&*b as *const BindingHolder, value))
        }) {
            handle.0.remove_binding();
        }
        handle.0.mark_dirty();
    }
}

fn make_c_function_binding(
    binding: extern "C" fn(*mut c_void, *mut c_void),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    intercept_set: Option<
        extern "C" fn(user_data: *mut c_void, pointer_to_value: *const c_void) -> bool,
    >,
    intercept_set_binding: Option<
        extern "C" fn(user_data: *mut c_void, new_binding: *mut c_void) -> bool,
    >,
) -> impl BindingCallable<c_void> {
    struct CFunctionBinding<T> {
        binding_function: extern "C" fn(*mut c_void, *mut T),
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
        intercept_set:
            Option<extern "C" fn(user_data: *mut c_void, pointer_to_value: *const T) -> bool>,
        intercept_set_binding:
            Option<extern "C" fn(user_data: *mut c_void, new_binding: *mut c_void) -> bool>,
    }

    impl<T> Drop for CFunctionBinding<T> {
        fn drop(&mut self) {
            if let Some(x) = self.drop_user_data {
                x(self.user_data)
            }
        }
    }

    unsafe impl<T> BindingCallable<T> for CFunctionBinding<T> {
        fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
            (self.binding_function)(self.user_data, value as *mut T);
            BindingResult::KeepBinding
        }
        fn intercept_set(self: Pin<&Self>, value: &T) -> bool {
            match self.intercept_set {
                None => false,
                Some(intercept_set) => intercept_set(self.user_data, value as *const T),
            }
        }
        unsafe fn intercept_set_binding(self: Pin<&Self>, new_binding: *mut BindingHolder) -> bool {
            match self.intercept_set_binding {
                None => false,
                Some(intercept_set_b) => intercept_set_b(self.user_data, new_binding.cast()),
            }
        }
    }

    CFunctionBinding {
        binding_function: binding,
        user_data,
        drop_user_data,
        intercept_set,
        intercept_set_binding,
    }
}

/// Set a binding
///
/// The current implementation will do usually two memory allocation:
///  1. the allocation from the calling code to allocate user_data
///  2. the box allocation within this binding
/// It might be possible to reduce that by passing something with a
/// vtable, so there is the need for less memory allocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_set_binding(
    handle: &PropertyHandleOpaque,
    binding: extern "C" fn(user_data: *mut c_void, pointer_to_value: *mut c_void),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    intercept_set: Option<
        extern "C" fn(user_data: *mut c_void, pointer_to_value: *const c_void) -> bool,
    >,
    intercept_set_binding: Option<
        extern "C" fn(user_data: *mut c_void, new_binding: *mut c_void) -> bool,
    >,
) {
    let binding = make_c_function_binding(
        binding,
        user_data,
        drop_user_data,
        intercept_set,
        intercept_set_binding,
    );
    unsafe { handle.0.set_binding(binding) };
}

/// Set a binding using an already allocated building holder
///
//// (take ownership of the binding)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_set_binding_internal(
    handle: &PropertyHandleOpaque,
    binding: *mut c_void,
) {
    handle.0.set_binding_impl(binding.cast());
}

/// Delete a binding. The pointer must be a pointer to a binding (so a BindingHolder)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_delete_binding(binding: *mut c_void) {
    let b = binding as *mut BindingHolder;
    unsafe { ((*b).vtable.drop)(b) };
}

/// Evaluate a raw binding
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_evaluate_binding(binding: *mut c_void, value: *mut c_void) {
    let b = binding as *mut BindingHolder;
    unsafe { ((*b).vtable.evaluate)(b, value) };
}

/// Returns whether the property behind this handle is marked as dirty
#[unsafe(no_mangle)]
pub extern "C" fn slint_property_is_dirty(handle: &PropertyHandleOpaque) -> bool {
    handle.0.access(|binding| binding.is_some_and(|b| b.dirty.get()))
}

/// Marks the property as dirty and notifies dependencies.
#[unsafe(no_mangle)]
pub extern "C" fn slint_property_mark_dirty(handle: &PropertyHandleOpaque) {
    handle.0.mark_dirty()
}

/// Marks the property as dirty and notifies dependencies.
#[unsafe(no_mangle)]
pub extern "C" fn slint_property_set_constant(handle: &PropertyHandleOpaque) {
    handle.0.set_constant()
}

/// Destroy handle
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_drop(handle: *mut PropertyHandleOpaque) {
    unsafe {
        core::ptr::drop_in_place(handle);
    }
}

fn c_set_animated_value<T: InterpolatedPropertyValue + Clone>(
    handle: &PropertyHandleOpaque,
    from: T,
    to: T,
    animation_data: &PropertyAnimation,
) {
    let d = RefCell::new(properties_animations::PropertyValueAnimationData::new(
        from,
        to,
        animation_data.clone(),
    ));
    // Safety: The BindingCallable is for type T
    unsafe {
        handle.0.set_binding(move |val: &mut T| {
            let (value, finished) = d.borrow_mut().compute_interpolated_value();
            *val = value;
            if finished {
                BindingResult::RemoveBinding
            } else {
                crate::animations::CURRENT_ANIMATION_DRIVER
                    .with(|driver| driver.set_has_active_animations());
                BindingResult::KeepBinding
            }
        })
    };
    handle.0.mark_dirty();
}

/// Internal function to set up a property animation to the specified target value for an integer property.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_set_animated_value_int(
    handle: &PropertyHandleOpaque,
    from: i32,
    to: i32,
    animation_data: &PropertyAnimation,
) {
    c_set_animated_value(handle, from, to, animation_data)
}

/// Internal function to set up a property animation to the specified target value for a float property.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_set_animated_value_float(
    handle: &PropertyHandleOpaque,
    from: f32,
    to: f32,
    animation_data: &PropertyAnimation,
) {
    c_set_animated_value(handle, from, to, animation_data)
}

/// Internal function to set up a property animation to the specified target value for a color property.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_set_animated_value_color(
    handle: &PropertyHandleOpaque,
    from: Color,
    to: Color,
    animation_data: &PropertyAnimation,
) {
    c_set_animated_value(handle, from, to, animation_data);
}

/// Internal function to set up a property animation to the specified target value for a brush property.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_set_animated_value_brush(
    handle: &PropertyHandleOpaque,
    from: &Brush,
    to: &Brush,
    animation_data: &PropertyAnimation,
) {
    c_set_animated_value(handle, from.clone(), to.clone(), animation_data);
}

unsafe fn c_set_animated_binding<T: InterpolatedPropertyValue + Clone>(
    handle: &PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void, *mut T),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    transition_data: extern "C" fn(
        user_data: *mut c_void,
        start_instant: &mut *mut u64,
    ) -> PropertyAnimation,
) {
    unsafe {
        let binding = core::mem::transmute::<
            extern "C" fn(*mut c_void, *mut T),
            extern "C" fn(*mut c_void, *mut ()),
        >(binding);
        let original_binding = PropertyHandle {
            handle: Cell::new(
                (alloc_binding_holder(make_c_function_binding(
                    binding,
                    user_data,
                    drop_user_data,
                    None,
                    None,
                )) as usize)
                    | 0b10,
            ),
        };
        let animation_data = RefCell::new(properties_animations::PropertyValueAnimationData::new(
            T::default(),
            T::default(),
            PropertyAnimation::default(),
        ));

        handle.0.set_binding(properties_animations::AnimatedBindingCallable::<T, _> {
            original_binding,
            state: Cell::new(properties_animations::AnimatedBindingState::NotAnimating),
            animation_data,
            compute_animation_details: move || -> properties_animations::AnimationDetail {
                // The transition_data function receives a *mut *mut u64 pointer for the
                // timestamp.
                // If the function sets the pointer to nullptr, it doesn't provide a start_time.
                // Otherwise, we assume it has written a value to the start_instant.
                // This basically models a `&mut Option<u64>`, which is then converted to an
                // `Option<Instant>`
                let mut start_instant = 0u64;
                let mut start_instant_ref = &mut start_instant as *mut u64;
                let anim = transition_data(user_data, &mut start_instant_ref);
                let start_instant = if start_instant_ref.is_null() {
                    None
                } else {
                    Some(crate::animations::Instant(start_instant))
                };
                (anim, start_instant)
            },
        });
        handle.0.mark_dirty();
    }
}

/// Internal function to set up a property animation between values produced by the specified binding for an integer property.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_set_animated_binding_int(
    handle: &PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void, *mut core::ffi::c_int),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    transition_data: extern "C" fn(
        user_data: *mut c_void,
        start_instant: &mut *mut u64,
    ) -> PropertyAnimation,
) {
    unsafe {
        c_set_animated_binding(handle, binding, user_data, drop_user_data, transition_data);
    }
}

/// Internal function to set up a property animation between values produced by the specified binding for a float property.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_set_animated_binding_float(
    handle: &PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void, *mut f32),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    transition_data: extern "C" fn(
        user_data: *mut c_void,
        start_instant: &mut *mut u64,
    ) -> PropertyAnimation,
) {
    unsafe {
        c_set_animated_binding(handle, binding, user_data, drop_user_data, transition_data);
    }
}

/// Internal function to set up a property animation between values produced by the specified binding for a color property.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_set_animated_binding_color(
    handle: &PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void, *mut Color),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    transition_data: extern "C" fn(
        user_data: *mut c_void,
        start_instant: &mut *mut u64,
    ) -> PropertyAnimation,
) {
    unsafe {
        c_set_animated_binding(handle, binding, user_data, drop_user_data, transition_data);
    }
}

/// Internal function to set up a property animation between values produced by the specified binding for a brush property.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_set_animated_binding_brush(
    handle: &PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void, *mut Brush),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
    transition_data: extern "C" fn(
        user_data: *mut c_void,
        start_instant: &mut *mut u64,
    ) -> PropertyAnimation,
) {
    unsafe {
        c_set_animated_binding(handle, binding, user_data, drop_user_data, transition_data);
    }
}

/// Internal function to set up a state binding on a Property<StateInfo>.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_set_state_binding(
    handle: &PropertyHandleOpaque,
    binding: extern "C" fn(*mut c_void) -> i32,
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
) {
    struct CStateBinding {
        binding: extern "C" fn(*mut c_void) -> i32,
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    }

    impl Drop for CStateBinding {
        fn drop(&mut self) {
            if let Some(x) = self.drop_user_data {
                x(self.user_data)
            }
        }
    }

    impl CStateBinding {
        fn call(&self) -> i32 {
            (self.binding)(self.user_data)
        }
    }

    let c_state_binding = CStateBinding { binding, user_data, drop_user_data };
    let bind_callable =
        StateInfoBinding { dirty_time: Cell::new(None), binding: move || c_state_binding.call() };
    unsafe { handle.0.set_binding(bind_callable) }
}

#[repr(C)]
/// Opaque type representing the PropertyTracker
pub struct PropertyTrackerOpaque {
    dependencies: usize,
    dep_nodes: usize,
    vtable: usize,
    dirty: bool,
}

static_assertions::assert_eq_align!(PropertyTrackerOpaque, PropertyTracker);
static_assertions::assert_eq_size!(PropertyTrackerOpaque, PropertyTracker);

/// Initialize the first pointer of the PropertyTracker.
/// `out` is assumed to be uninitialized
/// slint_property_tracker_drop need to be called after that
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_tracker_init(out: *mut PropertyTrackerOpaque) {
    unsafe {
        core::ptr::write(out as *mut PropertyTracker, PropertyTracker::default());
    }
}

/// Call the callback with the user data. Any properties access within the callback will be registered.
/// Any currently evaluated bindings or property trackers will be notified if accessed properties are changed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_tracker_evaluate(
    handle: *const PropertyTrackerOpaque,
    callback: extern "C" fn(user_data: *mut c_void),
    user_data: *mut c_void,
) {
    unsafe { Pin::new_unchecked(&*(handle as *const PropertyTracker)) }
        .evaluate(|| callback(user_data))
}

/// Call the callback with the user data. Any properties access within the callback will be registered.
/// Any currently evaluated bindings or property trackers will be not notified if accessed properties are changed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_tracker_evaluate_as_dependency_root(
    handle: *const PropertyTrackerOpaque,
    callback: extern "C" fn(user_data: *mut c_void),
    user_data: *mut c_void,
) {
    unsafe { Pin::new_unchecked(&*(handle as *const PropertyTracker)) }
        .evaluate_as_dependency_root(|| callback(user_data))
}
/// Query if the property tracker is dirty
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_tracker_is_dirty(
    handle: *const PropertyTrackerOpaque,
) -> bool {
    unsafe { (*(handle as *const PropertyTracker)).is_dirty() }
}

/// Destroy handle
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_tracker_drop(handle: *mut PropertyTrackerOpaque) {
    unsafe { core::ptr::drop_in_place(handle as *mut PropertyTracker) };
}

/// Construct a ChangeTracker
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_change_tracker_construct(ct: *mut ChangeTracker) {
    unsafe { core::ptr::write(ct, ChangeTracker::default()) };
}

/// Drop a ChangeTracker
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_change_tracker_drop(ct: *mut ChangeTracker) {
    unsafe { core::ptr::drop_in_place(ct) };
}

/// Initialize the change tracker.
///
/// When called inside an initialization scope (see `slint_initialization_scope_begin`),
/// the first evaluation is automatically deferred until the scope ends.
/// This prevents recursion when initializing change trackers that depend on
/// properties computed during layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_change_tracker_init(
    ct: &ChangeTracker,
    user_data: *mut c_void,
    drop_user_data: extern "C" fn(user_data: *mut c_void),
    eval_fn: extern "C" fn(user_data: *mut c_void) -> bool,
    notify_fn: extern "C" fn(user_data: *mut c_void),
) {
    #[allow(non_camel_case_types)]
    struct C_ChangeTrackerInner {
        user_data: *mut c_void,
        drop_user_data: extern "C" fn(user_data: *mut c_void),
        eval_fn: extern "C" fn(user_data: *mut c_void) -> bool,
        notify_fn: extern "C" fn(user_data: *mut c_void),
        /// Skip notify on first evaluation
        skip_first_notify: Cell<bool>,
        /// When true, we are currently running eval_fn or notify_fn and we shouldn't be dropped
        evaluating: Cell<bool>,
    }
    impl Drop for C_ChangeTrackerInner {
        fn drop(&mut self) {
            (self.drop_user_data)(self.user_data);
        }
    }

    unsafe fn drop(_self: *mut BindingHolder) {
        unsafe {
            let _self = _self as *mut BindingHolder<C_ChangeTrackerInner>;
            // If we're currently evaluating, just mark that drop was requested.
            // The actual drop will happen when evaluate() finishes.
            let evaluating =
                core::ptr::addr_of!((*_self).binding).as_ref().unwrap().evaluating.replace(false);
            if !evaluating {
                core::mem::drop(Box::from_raw(_self));
            }
        }
    }

    unsafe fn evaluate(_self: *const BindingHolder, _value: *mut ()) -> BindingResult {
        unsafe {
            let pinned_holder = Pin::new_unchecked(&*_self);
            let _self = _self as *mut BindingHolder<C_ChangeTrackerInner>;
            let inner = core::ptr::addr_of_mut!((*_self).binding).as_mut().unwrap();
            // Clear dep_nodes before re-registering dependencies
            (*core::ptr::addr_of!((*_self).dep_nodes)).take();
            assert!(!inner.evaluating.get());
            inner.evaluating.set(true);
            // Clear skip_first_notify BEFORE evaluating, so subsequent evaluations
            // will notify even if the first evaluation had no value change
            let is_first_eval = inner.skip_first_notify.replace(false);
            let notify = super::CURRENT_BINDING
                .set(Some(pinned_holder), || (inner.eval_fn)(inner.user_data));
            if notify && !is_first_eval {
                (inner.notify_fn)(inner.user_data);
            }

            if !inner.evaluating.replace(false) {
                // `drop` from the vtable was called while evaluating. Do it now.
                core::mem::drop(Box::from_raw(_self));
            }
            BindingResult::KeepBinding
        }
    }

    const VT: &'static BindingVTable = &BindingVTable {
        drop,
        evaluate,
        mark_dirty: ChangeTracker::mark_dirty,
        intercept_set: |_, _| false,
        intercept_set_binding: |_, _| false,
    };

    ct.clear();

    let inner = C_ChangeTrackerInner {
        user_data,
        drop_user_data,
        eval_fn,
        notify_fn,
        skip_first_notify: Cell::new(true), // Skip first notify
        evaluating: Cell::new(false),
    };

    let holder = BindingHolder {
        dependencies: Cell::new(0),
        dep_nodes: Default::default(),
        vtable: VT,
        dirty: Cell::new(false),
        is_two_way_binding: false,
        pinned: PhantomPinned,
        binding: inner,
        #[cfg(slint_debug_property)]
        debug_name: "<ChangeTracker>".into(),
    };

    let raw = Box::into_raw(Box::new(holder));
    unsafe { ct.set_internal(raw as *mut BindingHolder) };

    // Match Rust ChangeTracker::init behavior:
    // - If in initialization scope: defer evaluation to end of scope
    // - If not in scope: evaluate immediately
    if crate::initialization_scope::is_in_initialization_scope() {
        let raw_ptr = raw as usize;
        crate::initialization_scope::defer_initialization(move || {
            let raw = raw_ptr as *mut BindingHolder;
            unsafe {
                ((*core::ptr::addr_of!((*raw).vtable)).evaluate)(raw, core::ptr::null_mut());
            }
        });
    } else {
        // Evaluate immediately (skip_first_notify ensures no notify callback)
        unsafe {
            ((*core::ptr::addr_of!((*raw).vtable)).evaluate)(
                raw as *const BindingHolder,
                core::ptr::null_mut(),
            );
        }
    }
}

/// Initialize the change tracker with delayed first evaluation.
///
/// Same as `slint_change_tracker_init`, but the first evaluation is deferred
/// to `slint_change_tracker_run_change_handlers()`. This means the change tracker
/// will consider the value as default initialized, and the notify function will
/// be called the first time if the initial value differs from the default.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_change_tracker_init_delayed(
    ct: &ChangeTracker,
    user_data: *mut c_void,
    drop_user_data: extern "C" fn(user_data: *mut c_void),
    eval_fn: extern "C" fn(user_data: *mut c_void) -> bool,
    notify_fn: extern "C" fn(user_data: *mut c_void),
) {
    #[allow(non_camel_case_types)]
    struct C_ChangeTrackerInner {
        user_data: *mut c_void,
        drop_user_data: extern "C" fn(user_data: *mut c_void),
        eval_fn: extern "C" fn(user_data: *mut c_void) -> bool,
        notify_fn: extern "C" fn(user_data: *mut c_void),
        /// Skip notify on first evaluation (set to true for init_delayed)
        skip_first_notify: Cell<bool>,
        /// When true, we are currently running eval_fn or notify_fn and we shouldn't be dropped
        evaluating: Cell<bool>,
    }
    impl Drop for C_ChangeTrackerInner {
        fn drop(&mut self) {
            (self.drop_user_data)(self.user_data);
        }
    }

    unsafe fn drop(_self: *mut BindingHolder) {
        unsafe {
            let _self = _self as *mut BindingHolder<C_ChangeTrackerInner>;
            // If we're currently evaluating, just mark that drop was requested.
            // The actual drop will happen when evaluate() finishes.
            let evaluating =
                core::ptr::addr_of!((*_self).binding).as_ref().unwrap().evaluating.replace(false);
            if !evaluating {
                core::mem::drop(Box::from_raw(_self));
            }
        }
    }

    unsafe fn evaluate(_self: *const BindingHolder, _value: *mut ()) -> BindingResult {
        unsafe {
            let pinned_holder = Pin::new_unchecked(&*_self);
            let _self = _self as *mut BindingHolder<C_ChangeTrackerInner>;
            let inner = core::ptr::addr_of_mut!((*_self).binding).as_mut().unwrap();
            // Clear dep_nodes before re-registering dependencies
            (*core::ptr::addr_of!((*_self).dep_nodes)).take();
            assert!(!inner.evaluating.get());
            inner.evaluating.set(true);
            // Clear skip_first_notify BEFORE evaluating, so subsequent evaluations
            // will notify even if the first evaluation had no value change
            let is_first_eval = inner.skip_first_notify.replace(false);
            let notify = super::CURRENT_BINDING
                .set(Some(pinned_holder), || (inner.eval_fn)(inner.user_data));
            if notify && !is_first_eval {
                (inner.notify_fn)(inner.user_data);
            }

            if !inner.evaluating.replace(false) {
                // `drop` from the vtable was called while evaluating. Do it now.
                core::mem::drop(Box::from_raw(_self));
            }
            BindingResult::KeepBinding
        }
    }

    const VT: &'static BindingVTable = &BindingVTable {
        drop,
        evaluate,
        mark_dirty: ChangeTracker::mark_dirty,
        intercept_set: |_, _| false,
        intercept_set_binding: |_, _| false,
    };

    ct.clear();

    let inner = C_ChangeTrackerInner {
        user_data,
        drop_user_data,
        eval_fn,
        notify_fn,
        skip_first_notify: Cell::new(true), // Skip first notify for init_delayed
        evaluating: Cell::new(false),
    };

    let holder = BindingHolder {
        dependencies: Cell::new(0),
        dep_nodes: Default::default(),
        vtable: VT,
        dirty: Cell::new(false),
        is_two_way_binding: false,
        pinned: PhantomPinned,
        binding: inner,
        #[cfg(slint_debug_property)]
        debug_name: "<ChangeTracker>".into(),
    };

    let raw = Box::into_raw(Box::new(holder));
    unsafe { ct.set_internal(raw as *mut BindingHolder) };

    // Queue for run_change_handlers() - this is the key difference from init()
    let mut dep_nodes = super::single_linked_list_pin::SingleLinkedListPinHead::default();
    let node = dep_nodes.push_front(super::DependencyNode::new(raw as *const BindingHolder));
    super::change_tracker::CHANGED_NODES.with(|changed_nodes| {
        changed_nodes.append(node);
    });
    unsafe { (*core::ptr::addr_of_mut!((*raw).dep_nodes)).set(dep_nodes) };
}

/// Run all pending change handlers.
///
/// This processes any change trackers that have been queued for evaluation
/// via mark_dirty. Note that with the new initialization scope mechanism,
/// initial evaluations are handled automatically when the scope closes.
#[unsafe(no_mangle)]
pub extern "C" fn slint_change_tracker_run_change_handlers() {
    ChangeTracker::run_change_handlers();
}

/// Begin an initialization scope.
///
/// Any change tracker initialization that occurs before `slint_initialization_scope_end`
/// is called will have its first evaluation deferred until the scope ends.
/// This prevents recursion when initializing change trackers that depend on
/// properties computed during layout.
///
/// Returns 1 if a new scope was created, 0 if we're already inside a scope.
/// If this returns 1, you MUST call `slint_initialization_scope_end(1)` to process
/// the deferred tasks.
#[unsafe(no_mangle)]
pub extern "C" fn slint_initialization_scope_begin() -> u8 {
    if crate::initialization_scope::begin_initialization_scope() {
        1 // New scope created
    } else {
        0 // Already in a scope
    }
}

/// End an initialization scope.
///
/// This processes all deferred initialization tasks that were queued since
/// the corresponding `slint_initialization_scope_begin` call.
///
/// The `handle` parameter must be the value returned by the matching begin call.
/// If handle is 0, this is a no-op (we're in a nested scope).
#[unsafe(no_mangle)]
pub extern "C" fn slint_initialization_scope_end(handle: u8) {
    if handle != 0 {
        crate::initialization_scope::end_initialization_scope();
    }
}

/// return the current animation tick for the `animation-tick` function
#[unsafe(no_mangle)]
pub extern "C" fn slint_animation_tick() -> u64 {
    crate::animations::animation_tick()
}

/// Test that dropping a change tracker during its notify callback doesn't cause use-after-free.
/// This is the FFI equivalent of the `delete_from_eval_fn` test in change_tracker.rs.
///
/// The scenario: change tracker is dropped during notify_fn execution (not first eval).
/// Without the `evaluating` guard, this would cause use-after-free because the evaluate
/// function would continue to use freed memory after notify_fn returns.
#[test]
fn ffi_delete_from_notify_fn_delayed() {
    use super::Property;
    use std::cell::RefCell;
    use std::pin::Pin;
    use std::rc::Rc;

    // A property that the change tracker will depend on
    let prop = Rc::pin(Property::new(1i32));

    // Shared state: the ChangeTracker wrapped in Option so we can take() it during notify
    struct TestData {
        ct: RefCell<Option<ChangeTracker>>,
        prop: Pin<Rc<Property<i32>>>,
        eval_count: RefCell<i32>,
        notify_count: RefCell<i32>,
    }

    let data = Rc::new(TestData {
        ct: RefCell::new(Some(ChangeTracker::default())),
        prop: prop.clone(),
        eval_count: RefCell::new(0),
        notify_count: RefCell::new(0),
    });

    extern "C" fn drop_data(user_data: *mut c_void) {
        unsafe {
            drop(Rc::from_raw(user_data as *const TestData));
        }
    }

    extern "C" fn eval_fn(user_data: *mut c_void) -> bool {
        let data = unsafe { &*(user_data as *const TestData) };
        let count = *data.eval_count.borrow();
        *data.eval_count.borrow_mut() = count + 1;
        // Access the property to register dependency
        let old_val = if count == 0 { 0 } else { data.prop.as_ref().get() - 1 };
        let new_val = data.prop.as_ref().get();
        new_val != old_val // Return true if value changed
    }

    extern "C" fn notify_fn(user_data: *mut c_void) {
        let data = unsafe { &*(user_data as *const TestData) };
        *data.notify_count.borrow_mut() += 1;
        // Drop the change tracker during notify - this is the critical test
        // Without the evaluating guard, the memory would be freed while
        // evaluate() is still running
        data.ct.borrow_mut().take();
    }

    // Initialize the change tracker using FFI
    // With the new API, init() automatically defers when we're not in a scope,
    // wrapping the evaluation in its own scope.
    {
        let ct_ref = data.ct.borrow();
        let ct = ct_ref.as_ref().unwrap();
        let data_ptr = Rc::into_raw(data.clone()) as *mut c_void;

        unsafe {
            slint_change_tracker_init(ct, data_ptr, drop_data, eval_fn, notify_fn);
        }
    }

    // First evaluation happened during init (deferred then immediately processed)
    // eval_fn is called, but notify is skipped (first eval skips notify)
    assert_eq!(*data.eval_count.borrow(), 1);
    assert_eq!(*data.notify_count.borrow(), 0); // First eval skips notify
    assert!(data.ct.borrow().is_some()); // Tracker still exists

    // Change the property to trigger a second evaluation
    prop.as_ref().set(2);

    // Second run - now notify_fn will be called, which drops the tracker
    // This should not crash even though notify_fn drops the tracker mid-evaluation
    ChangeTracker::run_change_handlers();
    assert_eq!(*data.eval_count.borrow(), 2);
    assert_eq!(*data.notify_count.borrow(), 1);
    assert!(data.ct.borrow().is_none()); // Tracker was dropped in notify_fn
}
