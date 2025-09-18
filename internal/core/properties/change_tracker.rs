// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{
    single_linked_list_pin::SingleLinkedListPinHead, BindingHolder, BindingResult, BindingVTable,
    DependencyListHead, DependencyNode,
};
use alloc::boxed::Box;
use core::cell::Cell;
use core::marker::PhantomPinned;
use core::pin::Pin;
use core::ptr::addr_of;

// TODO a pinned thread local key?
crate::thread_local! {static CHANGED_NODES : Pin<Box<DependencyListHead>> = Box::pin(DependencyListHead::default()) }

struct ChangeTrackerInner<T, EvalFn, NotifyFn, Data> {
    eval_fn: EvalFn,
    notify_fn: NotifyFn,
    value: T,
    data: Data,
    /// When true, we are currently running eval_fn or notify_fn and we shouldn't be dropped
    evaluating: bool,
}

/// A change tracker is used to run a callback when a property value changes.
///
/// The Change Tracker must be initialized with the [`Self::init`] method.
///
/// When the property changes, the ChangeTracker is added to a thread local list, and the notify
/// callback is called when the [`Self::run_change_handlers()`] method is called
#[derive(Debug)]
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
        self.init_impl(data, eval_fn, notify_fn, false);
    }

    /// Initialize the change tracker with the given data and callbacks.
    ///
    /// Same as [`Self::init`], but the first eval function is called in a future evaluation of the event loop.
    /// This means that the change tracker will consider the value as default initialized, and the eval function will
    /// be called the firs ttime if the initial value is not equal to the default constructed value.
    pub fn init_delayed<Data, T: Default + PartialEq, EF: Fn(&Data) -> T, NF: Fn(&Data, &T)>(
        &self,
        data: Data,
        eval_fn: EF,
        notify_fn: NF,
    ) {
        self.init_impl(data, eval_fn, notify_fn, true);
    }

    fn init_impl<Data, T: Default + PartialEq, EF: Fn(&Data) -> T, NF: Fn(&Data, &T)>(
        &self,
        data: Data,
        eval_fn: EF,
        notify_fn: NF,
        delayed: bool,
    ) {
        self.clear();
        let inner =
            ChangeTrackerInner { eval_fn, notify_fn, value: T::default(), data, evaluating: false };

        unsafe fn evaluate<T: PartialEq, EF: Fn(&Data) -> T, NF: Fn(&Data, &T), Data>(
            _self: *mut BindingHolder,
            _value: *mut (),
        ) -> BindingResult {
            let pinned_holder = Pin::new_unchecked(&*_self);
            let _self = _self as *mut BindingHolder<ChangeTrackerInner<T, EF, NF, Data>>;
            let inner = core::ptr::addr_of_mut!((*_self).binding).as_mut().unwrap();
            (*core::ptr::addr_of_mut!((*_self).dep_nodes)).take();
            assert!(!inner.evaluating);
            inner.evaluating = true;
            let new_value =
                super::CURRENT_BINDING.set(Some(pinned_holder), || (inner.eval_fn)(&inner.data));
            if new_value != inner.value {
                inner.value = new_value;
                (inner.notify_fn)(&inner.data, &inner.value);
            }
            if !core::mem::replace(&mut inner.evaluating, false) {
                // `drop` from the vtable was called while evaluating. Do it now.
                core::mem::drop(Box::from_raw(_self));
            }
            BindingResult::KeepBinding
        }

        unsafe fn drop<T, EF, NF, Data>(_self: *mut BindingHolder) {
            let _self = _self as *mut BindingHolder<ChangeTrackerInner<T, EF, NF, Data>>;
            let evaluating = core::mem::replace(
                &mut core::ptr::addr_of_mut!((*_self).binding).as_mut().unwrap().evaluating,
                false,
            );
            if !evaluating {
                core::mem::drop(Box::from_raw(_self));
            }
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
        unsafe { self.set_internal(raw as *mut BindingHolder) };
        if delayed {
            let mut dep_nodes = SingleLinkedListPinHead::default();
            let node = dep_nodes.push_front(DependencyNode::new(raw as *const BindingHolder));
            CHANGED_NODES.with(|changed_nodes| {
                changed_nodes.append(node);
            });
            unsafe { (*core::ptr::addr_of_mut!((*raw).dep_nodes)).set(dep_nodes) };
            return;
        }
        let value = unsafe {
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
                while let Some(node) = old_list.take_head() {
                    unsafe {
                        ((*addr_of!((*node).vtable)).evaluate)(
                            node as *mut BindingHolder,
                            core::ptr::null_mut(),
                        );
                    }
                }
            }
        });
    }

    pub(super) unsafe fn mark_dirty(_self: *const BindingHolder, _was_dirty: bool) {
        let _self = _self.as_ref().unwrap();
        let node_head = _self.dep_nodes.take();
        if let Some(node) = node_head.iter().next() {
            node.remove();
            CHANGED_NODES.with(|changed_nodes| {
                changed_nodes.append(node);
            });
        }
        let other = _self.dep_nodes.replace(node_head);
        debug_assert!(other.iter().next().is_none());
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

    let state = Rc::new(core::cell::RefCell::new(std::string::String::new()));

    change1.init(
        (state.clone(), prop1.clone()),
        |(_, prop1)| prop1.as_ref().get(),
        |(state, _), val| {
            *state.borrow_mut() += &std::format!(":1({val})");
        },
    );
    change2.init(
        (state.clone(), prop2.clone()),
        |(_, prop2)| prop2.as_ref().get(),
        |(state, _), val| {
            *state.borrow_mut() += &std::format!(":2({val})");
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

/// test for issue #8741
#[test]
fn delete_from_eval_fn() {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::string::String;

    let change = Rc::<RefCell<Option<ChangeTracker>>>::new(Some(ChangeTracker::default()).into());
    let xyz = RefCell::new(String::from("*"));
    let result = Rc::new(RefCell::new(String::new()));
    let result2 = result.clone();
    // The change event are run in reverse order as they are created, so this one shouldn't be ever called as it is being detroyed from `change`
    let another = Rc::<RefCell<Option<ChangeTracker>>>::new(Some(ChangeTracker::default()).into());
    another.borrow().as_ref().unwrap().init_delayed(
        (),
        |()| unreachable!(),
        move |(), &()| unreachable!(),
    );
    change.borrow().as_ref().unwrap().init_delayed(
        change.clone(),
        |x| {
            x.borrow_mut().take().unwrap();
            String::from("hi")
        },
        move |x, val| {
            assert!(x.borrow().is_none());
            assert_eq!(val, "hi");
            xyz.borrow_mut().push_str("+");
            assert!(xyz.borrow().as_str().starts_with("*+"));
            result2.replace(xyz.borrow().clone());
            another.borrow_mut().take().unwrap();
        },
    );

    assert_eq!(result.borrow().as_str(), "");
    ChangeTracker::run_change_handlers();
    assert_eq!(result.borrow().as_str(), "*+");
    ChangeTracker::run_change_handlers();
    assert_eq!(result.borrow().as_str(), "*+");
}

#[test]
fn change_mutliple_dependencies() {
    use super::Property;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::string::String;
    let prop1 = Rc::pin(Property::new(1));
    let prop2 = Rc::pin(Property::new(2));
    let prop3 = Rc::pin(Property::new(3));
    let prop4 = Rc::pin(Property::new(4));
    let prop_with_deps = Rc::pin(Property::new(5));
    let result = Rc::new(RefCell::new(String::new()));

    let change_tracker = ChangeTracker::default();
    change_tracker.init(
        result.clone(),
        {
            let prop1 = prop1.clone();
            let prop2 = prop2.clone();
            let prop3 = prop3.clone();
            let prop4 = prop4.clone();
            let prop_with_deps = prop_with_deps.clone();
            move |_| {
                prop1.as_ref().get()
                    + prop2.as_ref().get()
                    + prop3.as_ref().get()
                    + prop4.as_ref().get()
                    + prop_with_deps.as_ref().get()
            }
        },
        move |result, val| {
            *result.borrow_mut() += &std::format!("[{val}]");
        },
    );

    assert_eq!(result.borrow().as_str(), "");
    ChangeTracker::run_change_handlers();
    assert_eq!(result.borrow().as_str(), "");

    prop_with_deps.as_ref().set_binding({
        let prop1 = prop1.clone();
        let prop2 = prop2.clone();
        move || prop1.as_ref().get() + prop2.as_ref().get()
    });

    assert_eq!(result.borrow().as_str(), "");
    ChangeTracker::run_change_handlers();
    assert_eq!(prop_with_deps.as_ref().get(), 3);
    assert_eq!(result.borrow().as_str(), "[13]"); // 1 + 2 + 3 + 4 + 3

    ChangeTracker::run_change_handlers();
    assert_eq!(result.borrow().as_str(), "[13]");

    prop1.as_ref().set(10);
    assert_eq!(result.borrow().as_str(), "[13]");
    ChangeTracker::run_change_handlers();
    assert_eq!(result.borrow().as_str(), "[13][31]"); // 10 + 2 + 3 + 4 + 12

    prop2.as_ref().set(20);
    prop3.as_ref().set(30);
    assert_eq!(result.borrow().as_str(), "[13][31]");
    ChangeTracker::run_change_handlers();
    assert_eq!(result.borrow().as_str(), "[13][31][94]"); // 10 + 20 + 30 + 4 + 30

    ChangeTracker::run_change_handlers();
    assert_eq!(result.borrow().as_str(), "[13][31][94]");

    // just swap prop1 and prop2, doesn't change the outcome
    prop1.as_ref().set(20);
    prop2.as_ref().set(10);
    ChangeTracker::run_change_handlers();
    assert_eq!(result.borrow().as_str(), "[13][31][94]");
}
