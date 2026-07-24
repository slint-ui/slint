// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use crate::graphics::{Brush, Color};
use crate::items::PropertyAnimation;
use core::ffi::c_void;

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

/// Register this property as a dependency of the current tracking scope
/// without evaluating any binding.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_register_as_dependency(handle: &PropertyHandleOpaque) {
    unsafe {
        let handle = Pin::new_unchecked(&handle.0);
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
///
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
/// (take ownership of the binding)
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

/// Call `intercept_set` on a raw binding, returning whether the binding accepted the write
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_property_intercept_set_binding(
    binding: *mut c_void,
    value: *const c_void,
) -> bool {
    let b = binding as *mut BindingHolder;
    unsafe { ((*b).vtable.intercept_set)(b, value) }
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
            extern "C" fn(*mut c_void, *mut c_void),
        >(binding);
        let original_binding = PropertyHandle {
            handle: Cell::new(
                (alloc_binding_holder(make_c_function_binding(
                    binding,
                    user_data,
                    drop_user_data,
                    None,
                    None,
                )) as *mut ())
                    .map_addr(|a| a | 0b10),
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
            carried_velocity: Cell::new(0.0),
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

#[repr(C)]
/// Opaque type representing the ChangeTracker
pub struct ChangeTrackerOpaque {
    _inner: *const c_void,
}

static_assertions::assert_eq_align!(ChangeTrackerOpaque, ChangeTracker);
static_assertions::assert_eq_size!(ChangeTrackerOpaque, ChangeTracker);

/// Construct a ChangeTracker
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_change_tracker_construct(ct: *mut ChangeTrackerOpaque) {
    unsafe { core::ptr::write(ct as *mut ChangeTracker, ChangeTracker::default()) };
}

/// Drop a ChangeTracker
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_change_tracker_drop(ct: *mut ChangeTrackerOpaque) {
    unsafe { core::ptr::drop_in_place(ct as *mut ChangeTracker) };
}

/// initialize the change tracker
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_change_tracker_init(
    ct: *const ChangeTrackerOpaque,
    user_data: *mut c_void,
    drop_user_data: extern "C" fn(user_data: *mut c_void),
    eval_fn: extern "C" fn(user_data: *mut c_void) -> bool,
    notify_fn: extern "C" fn(user_data: *mut c_void),
) {
    let ct = unsafe { &*ct.cast::<ChangeTracker>() };
    #[allow(non_camel_case_types)]
    struct C_ChangeTrackerInner {
        user_data: *mut c_void,
        drop_user_data: extern "C" fn(user_data: *mut c_void),
        eval_fn: extern "C" fn(user_data: *mut c_void) -> bool,
        notify_fn: extern "C" fn(user_data: *mut c_void),
    }
    impl Drop for C_ChangeTrackerInner {
        fn drop(&mut self) {
            (self.drop_user_data)(self.user_data);
        }
    }

    unsafe fn drop(_self: *mut BindingHolder) {
        core::mem::drop(unsafe {
            Box::from_raw(_self as *mut BindingHolder<C_ChangeTrackerInner>)
        });
    }

    unsafe fn evaluate(_self: *const BindingHolder, _value: *mut c_void) -> BindingResult {
        let _self_raw = _self;
        let _self = _self as *mut BindingHolder<C_ChangeTrackerInner>;
        let inner = unsafe { core::ptr::addr_of_mut!((*_self).binding).as_mut().unwrap() };
        unsafe { *(*core::ptr::addr_of!((*_self).dep_nodes)).get() = Default::default() };
        let notify = super::current_binding_storage::set(Some(_self_raw), || {
            (inner.eval_fn)(inner.user_data)
        });
        if notify {
            (inner.notify_fn)(inner.user_data);
        }
        BindingResult::KeepBinding
    }

    const VT: &BindingVTable = &BindingVTable {
        drop,
        evaluate,
        mark_dirty: ChangeTracker::mark_dirty,
        intercept_set: |_, _| false,
        intercept_set_binding: |_, _| false,
        velocity: |_| None,
    };

    ct.clear();

    let inner = C_ChangeTrackerInner { user_data, drop_user_data, eval_fn, notify_fn };

    let holder = BindingHolder {
        dependencies: Cell::new(core::ptr::null_mut()),
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

    let inner = unsafe { core::ptr::addr_of_mut!((*raw).binding).as_mut().unwrap() };
    super::current_binding_storage::set(Some(raw as *const BindingHolder), || {
        (inner.eval_fn)(inner.user_data)
    });
}

/// return the current animation tick for the `animation-tick` function
#[unsafe(no_mangle)]
pub extern "C" fn slint_animation_tick() -> u64 {
    crate::animations::animation_tick()
}

#[cfg(test)]
mod ffi_change_tracker_leak_test {
    use super::*;
    use crate::properties::ChangeTracker;
    use alloc::boxed::Box;
    use core::cell::Cell;
    use core::pin::Pin;

    // What the generated C++ stores for a `changed` handler: the watched
    // property and the last seen value.
    struct EvalState {
        prop: *const Property<i32>,
        last: Cell<i32>,
    }

    extern "C" fn eval_fn(user_data: *mut c_void) -> bool {
        let st = unsafe { &*(user_data as *const EvalState) };
        let v = unsafe { Pin::new_unchecked(&*st.prop) }.get();
        let changed = v != st.last.get();
        st.last.set(v);
        changed
    }
    extern "C" fn notify_fn(_user_data: *mut c_void) {}
    extern "C" fn drop_fn(_user_data: *mut c_void) {}

    // The dependency nodes must not accumulate across re-evaluations.
    #[test]
    fn ffi_change_tracker_does_not_leak_dep_nodes() {
        let prop = Box::pin(Property::new(0));
        let state = EvalState { prop: &*prop as *const _, last: Cell::new(0) };
        let ct = ChangeTracker::default();
        unsafe {
            slint_change_tracker_init(
                &ct as *const ChangeTracker as *const ChangeTrackerOpaque,
                &state as *const EvalState as *mut c_void,
                drop_fn,
                eval_fn,
                notify_fn,
            );
        }
        assert_eq!(ct.test_dep_node_count(), 1);

        for i in 1..=200 {
            prop.as_ref().set(i);
            ChangeTracker::run_change_handlers();
            assert_eq!(ct.test_dep_node_count(), 1, "leaked a DependencyNode at iteration {i}");
        }
    }
}
