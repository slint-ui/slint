// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{BindingHolder, BindingResult, BindingVTable, DependencyListHead};
#[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
use crate::thread_local;
use alloc::boxed::Box;
use core::cell::Cell;
use core::marker::PhantomPinned;
use core::pin::Pin;
use core::ptr::addr_of;

// TODO a pinned thread local key?
thread_local! {static CHANGED_NODES : Pin<Box<DependencyListHead>> = Box::pin(DependencyListHead::default()) }

struct ChangeTrackerInner<T, EvalFn, NotifyFn, Data> {
    eval_fn: EvalFn,
    notify_fn: NotifyFn,
    value: T,
    data: Data,
}

/// A change tracker is used to run a callback when a property value changes.
///
/// The Change Tracker must be initialized with the [`Self::init`] method.
///
/// When the property changes, the ChangeTracker is added to a thread local list, and the notify
/// callback is called when the [`Self::run_change_handlers()`] method is called
pub struct ChangeTracker {
    /// (Actually a `BindingHolder<ChangeTrackerInner>`)
    inner: Cell<*mut BindingHolder>,
}

impl Default for ChangeTracker {
    fn default() -> Self {
        Self { inner: Cell::new(core::ptr::null_mut()) }
    }
}

impl Drop for ChangeTracker {
    fn drop(&mut self) {
        self.clear();
    }
}

impl ChangeTracker {
    /// Initialize the change tracker with the given data and callbacks.
    ///
    /// The `data` is any struct that is going to be passed to the functor.
    /// The `eval_fn` is a function that queries and return the property.
    /// And the `notify_fn` is the callback run if the property is changed
    pub fn init<Data, T: Default + PartialEq, EF: Fn(&Data) -> T, NF: Fn(&Data, &T)>(
        &self,
        data: Data,
        eval_fn: EF,
        notify_fn: NF,
    ) {
        self.clear();
        let inner = ChangeTrackerInner { eval_fn, notify_fn, value: T::default(), data };

        unsafe fn evaluate<T: PartialEq, EF: Fn(&Data) -> T, NF: Fn(&Data, &T), Data>(
            _self: *mut BindingHolder,
            _value: *mut (),
        ) -> BindingResult {
            let pinned_holder = Pin::new_unchecked(&*_self);
            let _self = _self as *mut BindingHolder<ChangeTrackerInner<T, EF, NF, Data>>;
            let inner = core::ptr::addr_of_mut!((*_self).binding).as_mut().unwrap();
            let new_value =
                super::CURRENT_BINDING.set(Some(pinned_holder), || (inner.eval_fn)(&inner.data));
            if new_value != inner.value {
                inner.value = new_value;
                (inner.notify_fn)(&inner.data, &inner.value);
            }
            BindingResult::KeepBinding
        }

        unsafe fn drop<T, EF, NF, Data>(_self: *mut BindingHolder) {
            core::mem::drop(Box::from_raw(
                _self as *mut BindingHolder<ChangeTrackerInner<T, EF, NF, Data>>,
            ));
        }

        trait HasBindingVTable {
            const VT: &'static BindingVTable;
        }
        impl<T: PartialEq, EF: Fn(&Data) -> T, NF: Fn(&Data, &T), Data> HasBindingVTable
            for ChangeTrackerInner<T, EF, NF, Data>
        {
            const VT: &'static BindingVTable = &BindingVTable {
                drop: drop::<T, EF, NF, Data>,
                evaluate: evaluate::<T, EF, NF, Data>,
                mark_dirty: ChangeTracker::mark_dirty,
                intercept_set: |_, _| false,
                intercept_set_binding: |_, _| false,
            };
        }
        let holder = BindingHolder {
            dependencies: Cell::new(0),
            dep_nodes: Default::default(),
            vtable: <ChangeTrackerInner<T, EF, NF, Data> as HasBindingVTable>::VT,
            dirty: Cell::new(false),
            is_two_way_binding: false,
            pinned: PhantomPinned,
            binding: inner,
            #[cfg(slint_debug_property)]
            debug_name: "<ChangeTracker>".into(),
        };

        let raw = Box::into_raw(Box::new(holder));
        let value = unsafe {
            self.set_internal(raw as *mut BindingHolder);
            let pinned_holder = Pin::new_unchecked((raw as *mut BindingHolder).as_ref().unwrap());
            let inner = core::ptr::addr_of!((*raw).binding).as_ref().unwrap();
            super::CURRENT_BINDING.set(Some(pinned_holder), || (inner.eval_fn)(&inner.data))
        };
        unsafe { core::ptr::addr_of_mut!((*raw).binding).as_mut().unwrap().value = value };
    }

    /// Clear the change tracker.
    /// No notify function will be called after this.
    pub fn clear(&self) {
        let inner = self.inner.get();
        if !inner.is_null() {
            unsafe {
                let drop = (*core::ptr::addr_of!((*inner).vtable)).drop;
                drop(inner);
            }
            self.inner.set(core::ptr::null_mut());
        }
    }

    /// Run all the change handler that were queued.
    pub fn run_change_handlers() {
        CHANGED_NODES.with(|list| {
            let old_list = DependencyListHead::default();
            let old_list = core::pin::pin!(old_list);
            let mut count = 0;
            while !list.is_empty() {
                count += 1;
                if count > 9 {
                    crate::debug_log!("Slint: long changed callback chain detected");
                    return;
                }
                DependencyListHead::swap(list.as_ref(), old_list.as_ref());
                old_list.for_each(|node| {
                    let node = *node;
                    unsafe {
                        ((*addr_of!((*node).vtable)).evaluate)(
                            node as *mut BindingHolder,
                            core::ptr::null_mut(),
                        );
                    }
                });
                old_list.as_ref().clear();
            }
        });
    }

    pub(super) unsafe fn mark_dirty(_self: *const BindingHolder, _was_dirty: bool) {
        // Move the dependency list node from the dependency list to the CHANGED_NODE
        let _self = _self.as_ref().unwrap();
        let node_head = _self.dep_nodes.take();
        if let Some(node) = node_head.iter().next() {
            node.remove();
            CHANGED_NODES.with(|changed_nodes| {
                changed_nodes.append(node);
            });
        }
        _self.dep_nodes.set(node_head);
    }

    pub(super) unsafe fn set_internal(&self, raw: *mut BindingHolder) {
        self.inner.set(raw);
    }
}

#[test]
fn change_tracker() {
    use super::Property;
    use std::rc::Rc;
    let prop1 = Rc::pin(Property::new(42));
    let prop2 = Rc::pin(Property::<i32>::default());
    prop2.as_ref().set_binding({
        let prop1 = prop1.clone();
        move || prop1.as_ref().get() * 2
    });

    let change1 = ChangeTracker::default();
    let change2 = ChangeTracker::default();

    let state = Rc::new(core::cell::RefCell::new(String::new()));

    change1.init(
        (state.clone(), prop1.clone()),
        |(_, prop1)| prop1.as_ref().get(),
        |(state, _), val| {
            *state.borrow_mut() += &format!(":1({val})");
        },
    );
    change2.init(
        (state.clone(), prop2.clone()),
        |(_, prop2)| prop2.as_ref().get(),
        |(state, _), val| {
            *state.borrow_mut() += &format!(":2({val})");
        },
    );

    assert_eq!(state.borrow().as_str(), "");
    prop1.as_ref().set(10);
    assert_eq!(state.borrow().as_str(), "");
    prop1.as_ref().set(30);
    assert_eq!(state.borrow().as_str(), "");

    ChangeTracker::run_change_handlers();
    assert_eq!(state.borrow().as_str(), ":1(30):2(60)");
    ChangeTracker::run_change_handlers();
    assert_eq!(state.borrow().as_str(), ":1(30):2(60)");
    prop1.as_ref().set(1);
    assert_eq!(state.borrow().as_str(), ":1(30):2(60)");
    ChangeTracker::run_change_handlers();
    assert_eq!(state.borrow().as_str(), ":1(30):2(60):1(1):2(2)");
}
