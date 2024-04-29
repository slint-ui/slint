// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use super::{BindingHolder, BindingResult, BindingVTable, DependencyListHead, DependencyNode};
use core::cell::Cell;
use core::marker::PhantomPinned;
use core::pin::Pin;
use core::ptr::addr_of;

thread_local! {static CHANGED_NODES : DependencyListHead = DependencyListHead::default() }

struct ChangeTrackerInner<T, EvalFn, NotifyFn, Data> {
    eval_fn: EvalFn,
    notify_fn: NotifyFn,
    value: T,
    data: Data,
}

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
    pub fn init<Data, T: Default + PartialEq, EF: Fn(&Data) -> T, NF: Fn(&Data, &T)>(
        &self,
        data: Data,
        eval_fn: EF,
        notify_fn: NF,
    ) {
        self.clear();
        let inner = ChangeTrackerInner { eval_fn, notify_fn, value: T::default(), data };

        /// Safety: _self must be a pointer to a `BindingHolder<DirtyHandler>`
        unsafe fn mark_dirty(_self: *const BindingHolder, _was_dirty: bool) {
            debug_assert!(!_was_dirty);
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

        /// and value must be a pointer to T
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
                mark_dirty: mark_dirty,
                intercept_set: |_, _| false,
                intercept_set_binding: |_, _| false,
            };
        }
        let holder = BindingHolder {
            dependencies: Cell::new(0),
            dep_nodes: Default::default(),
            vtable: <ChangeTrackerInner<T, EF, NF, Data> as HasBindingVTable>::VT,
            dirty: Cell::new(true), // starts dirty so it evaluates the property when used
            is_two_way_binding: false,
            pinned: PhantomPinned,
            binding: inner,
            #[cfg(slint_debug_property)]
            debug_name: "<ChangeTracker>".into(),
        };

        self.inner.set(Box::into_raw(Box::new(holder)) as *mut BindingHolder);
    }

    fn clear(&self) {
        let inner = self.inner.get();
        if !inner.is_null() {
            unsafe {
                let drop = (*core::ptr::addr_of!((*inner).vtable)).drop;
                drop(inner as *mut BindingHolder);
            }
            self.inner.set(core::ptr::null_mut());
        }
    }

    fn run_change_handlers() {
        CHANGED_NODES.with(|list| {
            todo!("Swap the list before iterate.  Also clear the dependency node");
            list.for_each(|node| {
                let node = *node;
                unsafe {
                    ((*addr_of!((*node).vtable)).evaluate)(node as *mut BindingHolder, core::ptr::null_mut());
                }
            });
        });
    }
}
