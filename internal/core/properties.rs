// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
    Property binding engine.

    The current implementation uses lots of heap allocation but that can be optimized later using
    thin dst container, and intrusive linked list
*/

// cSpell: ignore rustflags

#![allow(unsafe_code)]
#![warn(missing_docs)]

/// A singled linked list whose nodes are pinned
mod single_linked_list_pin {
    #![allow(unsafe_code)]
    use alloc::boxed::Box;
    use core::pin::Pin;

    type NodePtr<T> = Option<Pin<Box<SingleLinkedListPinNode<T>>>>;
    struct SingleLinkedListPinNode<T> {
        next: NodePtr<T>,
        value: T,
    }

    pub struct SingleLinkedListPinHead<T>(NodePtr<T>);
    impl<T> Default for SingleLinkedListPinHead<T> {
        fn default() -> Self {
            Self(None)
        }
    }

    impl<T> Drop for SingleLinkedListPinHead<T> {
        fn drop(&mut self) {
            // Use a loop instead of relying on the Drop of NodePtr to avoid recursion
            while let Some(mut x) = core::mem::take(&mut self.0) {
                // Safety: we don't touch the `x.value` which is the one protected by the Pin
                self.0 = core::mem::take(unsafe { &mut Pin::get_unchecked_mut(x.as_mut()).next });
            }
        }
    }

    impl<T> SingleLinkedListPinHead<T> {
        pub fn push_front(&mut self, value: T) -> Pin<&T> {
            self.0 = Some(Box::pin(SingleLinkedListPinNode { next: self.0.take(), value }));
            // Safety: we can project from SingleLinkedListPinNode
            unsafe { Pin::new_unchecked(&self.0.as_ref().unwrap().value) }
        }

        #[allow(unused)]
        pub fn iter(&self) -> impl Iterator<Item = Pin<&T>> {
            struct I<'a, T>(&'a NodePtr<T>);

            impl<'a, T> Iterator for I<'a, T> {
                type Item = Pin<&'a T>;
                fn next(&mut self) -> Option<Self::Item> {
                    if let Some(x) = &self.0 {
                        let r = unsafe { Pin::new_unchecked(&x.value) };
                        self.0 = &x.next;
                        Some(r)
                    } else {
                        None
                    }
                }
            }
            I(&self.0)
        }
    }

    #[test]
    fn test_list() {
        let mut head = SingleLinkedListPinHead::default();
        head.push_front(1);
        head.push_front(2);
        head.push_front(3);
        assert_eq!(
            head.iter().map(|x: Pin<&i32>| *x.get_ref()).collect::<std::vec::Vec<i32>>(),
            std::vec![3, 2, 1]
        );
    }
    #[test]
    fn big_list() {
        // should not stack overflow
        let mut head = SingleLinkedListPinHead::default();
        for x in 0..100000 {
            head.push_front(x);
        }
    }
}

pub(crate) mod dependency_tracker {
    //! This module contains an implementation of a double linked list that can be used
    //! to track dependency, such that when a node is dropped, the nodes are automatically
    //! removed from the list.
    //! This is unsafe to use for various reason, so it is kept internal.

    use core::cell::Cell;
    use core::pin::Pin;

    #[repr(transparent)]
    pub struct DependencyListHead<T>(Cell<*const DependencyNode<T>>);

    impl<T> Default for DependencyListHead<T> {
        fn default() -> Self {
            Self(Cell::new(core::ptr::null()))
        }
    }
    impl<T> Drop for DependencyListHead<T> {
        fn drop(&mut self) {
            unsafe { DependencyListHead::drop(self as *mut Self) };
        }
    }

    impl<T> DependencyListHead<T> {
        pub unsafe fn mem_move(from: *mut Self, to: *mut Self) {
            (*to).0.set((*from).0.get());
            if let Some(next) = (*from).0.get().as_ref() {
                debug_assert_eq!(from as *const _, next.prev.get() as *const _);
                next.debug_assert_valid();
                next.prev.set(to as *const _);
                next.debug_assert_valid();
            }
        }

        /// Swap two list head
        pub fn swap(from: Pin<&Self>, to: Pin<&Self>) {
            Cell::swap(&from.0, &to.0);
            unsafe {
                if let Some(n) = from.0.get().as_ref() {
                    debug_assert_eq!(n.prev.get() as *const _, &to.0 as *const _);
                    n.prev.set(&from.0 as *const _);
                    n.debug_assert_valid();
                }

                if let Some(n) = to.0.get().as_ref() {
                    debug_assert_eq!(n.prev.get() as *const _, &from.0 as *const _);
                    n.prev.set(&to.0 as *const _);
                    n.debug_assert_valid();
                }
            }
        }

        /// Return true is the list is empty
        pub fn is_empty(&self) -> bool {
            self.0.get().is_null()
        }

        pub unsafe fn drop(_self: *mut Self) {
            if let Some(next) = (*_self).0.get().as_ref() {
                debug_assert_eq!(_self as *const _, next.prev.get() as *const _);
                next.debug_assert_valid();
                next.prev.set(core::ptr::null());
                next.debug_assert_valid();
            }
        }
        pub fn append(&self, node: Pin<&DependencyNode<T>>) {
            unsafe {
                node.remove();
                node.debug_assert_valid();
                let old = self.0.get();
                if let Some(x) = old.as_ref() {
                    x.debug_assert_valid();
                }
                self.0.set(node.get_ref() as *const DependencyNode<_>);
                node.next.set(old);
                node.prev.set(&self.0 as *const _);
                if let Some(old) = old.as_ref() {
                    old.prev.set((&node.next) as *const _);
                    old.debug_assert_valid();
                }
                node.debug_assert_valid();
            }
        }

        pub fn for_each(&self, mut f: impl FnMut(&T)) {
            unsafe {
                let mut next = self.0.get();
                while let Some(node) = next.as_ref() {
                    node.debug_assert_valid();
                    next = node.next.get();
                    f(&node.binding);
                }
            }
        }

        /// Returns the first node of the list, if any
        pub fn take_head(&self) -> Option<T>
        where
            T: Copy,
        {
            unsafe {
                if let Some(node) = self.0.get().as_ref() {
                    node.debug_assert_valid();
                    node.remove();
                    Some(node.binding)
                } else {
                    None
                }
            }
        }
    }

    /// The node is owned by the binding; so the binding is always valid
    /// The next and pref
    pub struct DependencyNode<T> {
        next: Cell<*const DependencyNode<T>>,
        /// This is either null, or a pointer to a pointer to ourself
        prev: Cell<*const Cell<*const DependencyNode<T>>>,
        binding: T,
    }

    impl<T> DependencyNode<T> {
        pub fn new(binding: T) -> Self {
            Self { next: Cell::new(core::ptr::null()), prev: Cell::new(core::ptr::null()), binding }
        }

        /// Assert that the invariant of `next` and `prev` are met.
        pub fn debug_assert_valid(&self) {
            unsafe {
                debug_assert!(
                    self.prev.get().is_null()
                        || (*self.prev.get()).get() == self as *const DependencyNode<T>
                );
                debug_assert!(
                    self.next.get().is_null()
                        || (*self.next.get()).prev.get()
                            == (&self.next) as *const Cell<*const DependencyNode<T>>
                );
                // infinite loop?
                debug_assert_ne!(self.next.get(), self as *const DependencyNode<T>);
                debug_assert_ne!(
                    self.prev.get(),
                    (&self.next) as *const Cell<*const DependencyNode<T>>
                );
            }
        }

        pub fn remove(&self) {
            self.debug_assert_valid();
            unsafe {
                if let Some(prev) = self.prev.get().as_ref() {
                    prev.set(self.next.get());
                }
                if let Some(next) = self.next.get().as_ref() {
                    next.debug_assert_valid();
                    next.prev.set(self.prev.get());
                    next.debug_assert_valid();
                }
            }
            self.prev.set(core::ptr::null());
            self.next.set(core::ptr::null());
        }
    }

    impl<T> Drop for DependencyNode<T> {
        fn drop(&mut self) {
            self.remove();
        }
    }
}

type DependencyListHead = dependency_tracker::DependencyListHead<*const BindingHolder>;
type DependencyNode = dependency_tracker::DependencyNode<*const BindingHolder>;

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::{Cell, RefCell, UnsafeCell};
use core::marker::PhantomPinned;
use core::pin::Pin;

/// if a DependencyListHead points to that value, it is because the property is actually
/// constant and cannot have dependencies
static CONSTANT_PROPERTY_SENTINEL: u32 = 0;

/// The return value of a binding
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum BindingResult {
    /// The binding is a normal binding, and we keep it to re-evaluate it once it is dirty
    KeepBinding,
    /// The value of the property is now constant after the binding was evaluated, so
    /// the binding can be removed.
    RemoveBinding,
}

struct BindingVTable {
    drop: unsafe fn(_self: *mut BindingHolder),
    evaluate: unsafe fn(_self: *mut BindingHolder, value: *mut ()) -> BindingResult,
    mark_dirty: unsafe fn(_self: *const BindingHolder, was_dirty: bool),
    intercept_set: unsafe fn(_self: *const BindingHolder, value: *const ()) -> bool,
    intercept_set_binding:
        unsafe fn(_self: *const BindingHolder, new_binding: *mut BindingHolder) -> bool,
}

/// A binding trait object can be used to dynamically produces values for a property.
///
/// # Safety
///
/// IS_TWO_WAY_BINDING cannot be true if Self is not a TwoWayBinding
unsafe trait BindingCallable {
    /// This function is called by the property to evaluate the binding and produce a new value. The
    /// previous property value is provided in the value parameter.
    unsafe fn evaluate(self: Pin<&Self>, value: *mut ()) -> BindingResult;

    /// This function is used to notify the binding that one of the dependencies was changed
    /// and therefore this binding may evaluate to a different value, too.
    fn mark_dirty(self: Pin<&Self>) {}

    /// Allow the binding to intercept what happens when the value is set.
    /// The default implementation returns false, meaning the binding will simply be removed and
    /// the property will get the new value.
    /// When returning true, the call was intercepted and the binding will not be removed,
    /// but the property will still have that value
    unsafe fn intercept_set(self: Pin<&Self>, _value: *const ()) -> bool {
        false
    }

    /// Allow the binding to intercept what happens when the value is set.
    /// The default implementation returns false, meaning the binding will simply be removed.
    /// When returning true, the call was intercepted and the binding will not be removed.
    unsafe fn intercept_set_binding(self: Pin<&Self>, _new_binding: *mut BindingHolder) -> bool {
        false
    }

    /// Set to true if and only if Self is a TwoWayBinding<T>
    const IS_TWO_WAY_BINDING: bool = false;
}

unsafe impl<F: Fn(*mut ()) -> BindingResult> BindingCallable for F {
    unsafe fn evaluate(self: Pin<&Self>, value: *mut ()) -> BindingResult {
        self(value)
    }
}

#[cfg(feature = "std")]
use std::thread_local;
#[cfg(feature = "std")]
scoped_tls_hkt::scoped_thread_local!(static CURRENT_BINDING : for<'a> Option<Pin<&'a BindingHolder>>);

#[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
mod unsafe_single_threaded {
    use super::BindingHolder;
    use core::cell::Cell;
    use core::pin::Pin;
    use core::ptr::null;
    pub(super) struct FakeThreadStorage(Cell<*const BindingHolder>);
    impl FakeThreadStorage {
        pub const fn new() -> Self {
            Self(Cell::new(null()))
        }
        pub fn set<T>(&self, value: Option<Pin<&BindingHolder>>, f: impl FnOnce() -> T) -> T {
            let old = self.0.replace(value.map_or(null(), |v| v.get_ref() as *const BindingHolder));
            let res = f();
            let new = self.0.replace(old);
            assert_eq!(new, value.map_or(null(), |v| v.get_ref() as *const BindingHolder));
            res
        }
        pub fn is_set(&self) -> bool {
            !self.0.get().is_null()
        }
        pub fn with<T>(&self, f: impl FnOnce(Option<Pin<&BindingHolder>>) -> T) -> T {
            let local = unsafe { self.0.get().as_ref().map(|x| Pin::new_unchecked(x)) };
            let res = f(local);
            assert_eq!(self.0.get(), local.map_or(null(), |v| v.get_ref() as *const BindingHolder));
            res
        }
    }
    // Safety: the unsafe_single_threaded feature means we will only be called from a single thread
    unsafe impl Send for FakeThreadStorage {}
    unsafe impl Sync for FakeThreadStorage {}
}
#[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
static CURRENT_BINDING: unsafe_single_threaded::FakeThreadStorage =
    unsafe_single_threaded::FakeThreadStorage::new();

/// Evaluate a function, but do not register any property dependencies if that function
/// get the value of properties
pub fn evaluate_no_tracking<T>(f: impl FnOnce() -> T) -> T {
    CURRENT_BINDING.set(None, f)
}

/// Return true if there is currently a binding being evaluated so that access to
/// properties register dependencies to that binding.
pub fn is_currently_tracking() -> bool {
    CURRENT_BINDING.is_set() && CURRENT_BINDING.with(|x| x.is_some())
}

/// This structure erase the `B` type with a vtable.
#[repr(C)]
struct BindingHolder<B = ()> {
    /// Access to the list of binding which depends on this binding
    dependencies: Cell<usize>,
    /// The binding own the nodes used in the dependencies lists of the properties
    /// From which we depend.
    dep_nodes: Cell<single_linked_list_pin::SingleLinkedListPinHead<DependencyNode>>,
    vtable: &'static BindingVTable,
    /// The binding is dirty and need to be re_evaluated
    dirty: Cell<bool>,
    /// Specify that B is a `TwoWayBinding<T>`
    is_two_way_binding: bool,
    pinned: PhantomPinned,
    #[cfg(slint_debug_property)]
    pub debug_name: alloc::string::String,

    binding: B,
}

impl BindingHolder {
    fn register_self_as_dependency(
        self: Pin<&Self>,
        property_that_will_notify: *mut DependencyListHead,
        #[cfg(slint_debug_property)] _other_debug_name: &str,
    ) {
        let node = DependencyNode::new(self.get_ref() as *const _);
        let mut dep_nodes = self.dep_nodes.take();
        let node = dep_nodes.push_front(node);
        unsafe { DependencyListHead::append(&*property_that_will_notify, node) }
        self.dep_nodes.set(dep_nodes);
    }
}

fn alloc_binding_holder<B: BindingCallable + 'static>(binding: B) -> *mut BindingHolder {
    /// Safety: _self must be a pointer that comes from a `Box<BindingHolder<B>>::into_raw()`
    unsafe fn binding_drop<B>(_self: *mut BindingHolder) {
        drop(Box::from_raw(_self as *mut BindingHolder<B>));
    }

    /// Safety: _self must be a pointer to a `BindingHolder<B>`
    /// and value must be a pointer to T
    unsafe fn evaluate<B: BindingCallable>(
        _self: *mut BindingHolder,
        value: *mut (),
    ) -> BindingResult {
        let pinned_holder = Pin::new_unchecked(&*_self);
        CURRENT_BINDING.set(Some(pinned_holder), || {
            Pin::new_unchecked(&((*(_self as *mut BindingHolder<B>)).binding)).evaluate(value)
        })
    }

    /// Safety: _self must be a pointer to a `BindingHolder<B>`
    unsafe fn mark_dirty<B: BindingCallable>(_self: *const BindingHolder, _: bool) {
        Pin::new_unchecked(&((*(_self as *const BindingHolder<B>)).binding)).mark_dirty()
    }

    /// Safety: _self must be a pointer to a `BindingHolder<B>`
    unsafe fn intercept_set<B: BindingCallable>(
        _self: *const BindingHolder,
        value: *const (),
    ) -> bool {
        Pin::new_unchecked(&((*(_self as *const BindingHolder<B>)).binding)).intercept_set(value)
    }

    unsafe fn intercept_set_binding<B: BindingCallable>(
        _self: *const BindingHolder,
        new_binding: *mut BindingHolder,
    ) -> bool {
        Pin::new_unchecked(&((*(_self as *const BindingHolder<B>)).binding))
            .intercept_set_binding(new_binding)
    }

    trait HasBindingVTable {
        const VT: &'static BindingVTable;
    }
    impl<B: BindingCallable> HasBindingVTable for B {
        const VT: &'static BindingVTable = &BindingVTable {
            drop: binding_drop::<B>,
            evaluate: evaluate::<B>,
            mark_dirty: mark_dirty::<B>,
            intercept_set: intercept_set::<B>,
            intercept_set_binding: intercept_set_binding::<B>,
        };
    }

    let holder: BindingHolder<B> = BindingHolder {
        dependencies: Cell::new(0),
        dep_nodes: Default::default(),
        vtable: <B as HasBindingVTable>::VT,
        dirty: Cell::new(true), // starts dirty so it evaluates the property when used
        is_two_way_binding: B::IS_TWO_WAY_BINDING,
        pinned: PhantomPinned,
        #[cfg(slint_debug_property)]
        debug_name: Default::default(),
        binding,
    };
    Box::into_raw(Box::new(holder)) as *mut BindingHolder
}

#[repr(transparent)]
#[derive(Default)]
struct PropertyHandle {
    /// The handle can either be a pointer to a binding, or a pointer to the list of dependent properties.
    /// The two least significant bit of the pointer are flags, as the pointer will be aligned.
    /// The least significant bit (`0b01`) tells that the binding is borrowed. So no two reference to the
    /// binding exist at the same time.
    /// The second to last bit (`0b10`) tells that the pointer points to a binding. Otherwise, it is the head
    /// node of the linked list of dependent binding
    handle: Cell<usize>,
}

impl core::fmt::Debug for PropertyHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let handle = self.handle.get();
        write!(
            f,
            "PropertyHandle {{ handle: 0x{:x}, locked: {}, binding: {} }}",
            handle & !0b11,
            (handle & 0b01) == 0b01,
            (handle & 0b10) == 0b10
        )
    }
}

impl PropertyHandle {
    /// The lock flag specify that we can get reference to the Cell or unsafe cell
    fn lock_flag(&self) -> bool {
        self.handle.get() & 0b1 == 1
    }
    /// Sets the lock_flag.
    /// Safety: the lock flag must not be unset if there exist reference to what's inside the cell
    unsafe fn set_lock_flag(&self, set: bool) {
        self.handle.set(if set { self.handle.get() | 0b1 } else { self.handle.get() & !0b1 })
    }

    /// Access the value.
    /// Panics if the function try to recursively access the value
    fn access<R>(&self, f: impl FnOnce(Option<Pin<&mut BindingHolder>>) -> R) -> R {
        #[cfg(slint_debug_property)]
        if self.lock_flag() {
            unsafe {
                let handle = self.handle.get();
                if handle & 0b10 == 0b10 {
                    let binding = &mut *((handle & !0b11) as *mut BindingHolder);
                    let debug_name = &binding.debug_name;
                    panic!("Recursion detected with property {debug_name}");
                }
            }
        }
        assert!(!self.lock_flag(), "Recursion detected");
        unsafe {
            self.set_lock_flag(true);
            scopeguard::defer! { self.set_lock_flag(false); }
            let handle = self.handle.get();
            let binding = if handle & 0b10 == 0b10 {
                Some(Pin::new_unchecked(&mut *((handle & !0b11) as *mut BindingHolder)))
            } else {
                None
            };
            f(binding)
        }
    }

    fn remove_binding(&self) {
        assert!(!self.lock_flag(), "Recursion detected");
        let val = self.handle.get();
        if val & 0b10 == 0b10 {
            unsafe {
                self.set_lock_flag(true);
                let binding = (val & !0b11) as *mut BindingHolder;
                let const_sentinel = (&CONSTANT_PROPERTY_SENTINEL) as *const u32 as usize;
                if (*binding).dependencies.get() == const_sentinel {
                    self.handle.set(const_sentinel);
                    (*binding).dependencies.set(0);
                } else {
                    DependencyListHead::mem_move(
                        (*binding).dependencies.as_ptr() as *mut DependencyListHead,
                        self.handle.as_ptr() as *mut DependencyListHead,
                    );
                }
                ((*binding).vtable.drop)(binding);
            }
            debug_assert!(self.handle.get() & 0b11 == 0);
        }
    }

    /// Safety: the BindingCallable must be valid for the type of this property
    unsafe fn set_binding<B: BindingCallable + 'static>(
        &self,
        binding: B,
        #[cfg(slint_debug_property)] debug_name: &str,
    ) {
        let binding = alloc_binding_holder::<B>(binding);
        #[cfg(slint_debug_property)]
        {
            (*binding).debug_name = debug_name.into();
        }
        self.set_binding_impl(binding);
    }

    /// Implementation of Self::set_binding.
    fn set_binding_impl(&self, binding: *mut BindingHolder) {
        let previous_binding_intercepted = self.access(|b| {
            b.is_some_and(|b| unsafe {
                // Safety: b is a BindingHolder<T>
                (b.vtable.intercept_set_binding)(&*b as *const BindingHolder, binding)
            })
        });

        if previous_binding_intercepted {
            return;
        }

        self.remove_binding();
        debug_assert!((binding as usize) & 0b11 == 0);
        debug_assert!(self.handle.get() & 0b11 == 0);
        let const_sentinel = (&CONSTANT_PROPERTY_SENTINEL) as *const u32 as usize;
        let is_constant = self.handle.get() == const_sentinel;
        unsafe {
            if is_constant {
                (*binding).dependencies.set(const_sentinel);
            } else {
                DependencyListHead::mem_move(
                    self.handle.as_ptr() as *mut DependencyListHead,
                    (*binding).dependencies.as_ptr() as *mut DependencyListHead,
                );
            }
        }
        self.handle.set((binding as usize) | 0b10);
        if !is_constant {
            self.mark_dirty(
                #[cfg(slint_debug_property)]
                "",
            );
        }
    }

    fn dependencies(&self) -> *mut DependencyListHead {
        assert!(!self.lock_flag(), "Recursion detected");
        if (self.handle.get() & 0b10) != 0 {
            self.access(|binding| binding.unwrap().dependencies.as_ptr() as *mut DependencyListHead)
        } else {
            self.handle.as_ptr() as *mut DependencyListHead
        }
    }

    // `value` is the content of the unsafe cell and will be only dereferenced if the
    // handle is not locked. (Upholding the requirements of UnsafeCell)
    unsafe fn update<T>(&self, value: *mut T) {
        let remove = self.access(|binding| {
            if let Some(mut binding) = binding {
                if binding.dirty.get() {
                    // clear all the nodes so that we can start from scratch
                    binding.dep_nodes.set(Default::default());
                    let r = (binding.vtable.evaluate)(
                        binding.as_mut().get_unchecked_mut() as *mut BindingHolder,
                        value as *mut (),
                    );
                    binding.dirty.set(false);
                    if r == BindingResult::RemoveBinding {
                        return true;
                    }
                }
            }
            false
        });
        if remove {
            self.remove_binding()
        }
    }

    /// Register this property as a dependency to the current binding being evaluated
    fn register_as_dependency_to_current_binding(
        self: Pin<&Self>,
        #[cfg(slint_debug_property)] debug_name: &str,
    ) {
        if CURRENT_BINDING.is_set() {
            CURRENT_BINDING.with(|cur_binding| {
                if let Some(cur_binding) = cur_binding {
                    let dependencies = self.dependencies();
                    if !core::ptr::eq(
                        unsafe { *(dependencies as *mut *const u32) },
                        (&CONSTANT_PROPERTY_SENTINEL) as *const u32,
                    ) {
                        cur_binding.register_self_as_dependency(
                            dependencies,
                            #[cfg(slint_debug_property)]
                            debug_name,
                        );
                    }
                }
            });
        }
    }

    fn mark_dirty(&self, #[cfg(slint_debug_property)] debug_name: &str) {
        #[cfg(not(slint_debug_property))]
        let debug_name = "";
        unsafe {
            let dependencies = self.dependencies();
            assert!(
                !core::ptr::eq(
                    *(dependencies as *mut *const u32),
                    (&CONSTANT_PROPERTY_SENTINEL) as *const u32,
                ),
                "Constant property being changed {debug_name}"
            );
            mark_dependencies_dirty(dependencies)
        };
    }

    fn set_constant(&self) {
        unsafe {
            let dependencies = self.dependencies();
            if !core::ptr::eq(
                *(dependencies as *mut *const u32),
                (&CONSTANT_PROPERTY_SENTINEL) as *const u32,
            ) {
                DependencyListHead::drop(dependencies);
                *(dependencies as *mut *const u32) = (&CONSTANT_PROPERTY_SENTINEL) as *const u32
            }
        }
    }
}

impl Drop for PropertyHandle {
    fn drop(&mut self) {
        self.remove_binding();
        debug_assert!(self.handle.get() & 0b11 == 0);
        if self.handle.get() as *const u32 != (&CONSTANT_PROPERTY_SENTINEL) as *const u32 {
            unsafe {
                DependencyListHead::drop(self.handle.as_ptr() as *mut _);
            }
        }
    }
}

/// Safety: the dependency list must be valid and consistent
unsafe fn mark_dependencies_dirty(dependencies: *mut DependencyListHead) {
    debug_assert!(!core::ptr::eq(
        *(dependencies as *mut *const u32),
        (&CONSTANT_PROPERTY_SENTINEL) as *const u32,
    ));
    DependencyListHead::for_each(&*dependencies, |binding| {
        let binding: &BindingHolder = &**binding;
        let was_dirty = binding.dirty.replace(true);
        (binding.vtable.mark_dirty)(binding as *const BindingHolder, was_dirty);

        assert!(
            !core::ptr::eq(
                *(binding.dependencies.as_ptr() as *mut *const u32),
                (&CONSTANT_PROPERTY_SENTINEL) as *const u32,
            ),
            "Const property marked as dirty"
        );

        if !was_dirty {
            mark_dependencies_dirty(binding.dependencies.as_ptr() as *mut DependencyListHead)
        }
    });
}

/// Types that can be set as bindings for a `Property<T>`
pub trait Binding<T> {
    /// Evaluate the binding and return the new value
    fn evaluate(&self, old_value: &T) -> T;
}

impl<T, F: Fn() -> T> Binding<T> for F {
    fn evaluate(&self, _value: &T) -> T {
        self()
    }
}

/// A Property that allow binding that track changes
///
/// Property can have an assigned value, or binding.
/// When a binding is assigned, it is lazily evaluated on demand
/// when calling `get()`.
/// When accessing another property from a binding evaluation,
/// a dependency will be registered, such that when the property
/// change, the binding will automatically be updated
#[repr(C)]
pub struct Property<T> {
    /// This is usually a pointer, but the least significant bit tells what it is
    handle: PropertyHandle,
    /// This is only safe to access when the lock flag is not set on the handle.
    value: UnsafeCell<T>,
    pinned: PhantomPinned,
    /// Enabled only if compiled with `RUSTFLAGS='--cfg slint_debug_property'`
    /// Note that adding this flag will also tell the rust compiler to set this
    /// and that this will not work with C++ because of binary incompatibility
    #[cfg(slint_debug_property)]
    pub debug_name: RefCell<alloc::string::String>,
}

impl<T: core::fmt::Debug + Clone> core::fmt::Debug for Property<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[cfg(slint_debug_property)]
        write!(f, "[{}]=", self.debug_name.borrow())?;
        write!(
            f,
            "Property({:?}{})",
            self.get_internal(),
            if self.is_dirty() { " (dirty)" } else { "" }
        )
    }
}

impl<T: Default> Default for Property<T> {
    fn default() -> Self {
        Self {
            handle: Default::default(),
            value: Default::default(),
            pinned: PhantomPinned,
            #[cfg(slint_debug_property)]
            debug_name: Default::default(),
        }
    }
}

impl<T: Clone> Property<T> {
    /// Create a new property with this value
    pub fn new(value: T) -> Self {
        Self {
            handle: Default::default(),
            value: UnsafeCell::new(value),
            pinned: PhantomPinned,
            #[cfg(slint_debug_property)]
            debug_name: Default::default(),
        }
    }

    /// Same as [`Self::new`] but with a 'static string use for debugging only
    pub fn new_named(value: T, _name: &'static str) -> Self {
        Self {
            handle: Default::default(),
            value: UnsafeCell::new(value),
            pinned: PhantomPinned,
            #[cfg(slint_debug_property)]
            debug_name: RefCell::new(_name.into()),
        }
    }

    /// Get the value of the property
    ///
    /// This may evaluate the binding if there is a binding and it is dirty
    ///
    /// If the function is called directly or indirectly from a binding evaluation
    /// of another Property, a dependency will be registered.
    ///
    /// Panics if this property is get while evaluating its own binding or
    /// cloning the value.
    pub fn get(self: Pin<&Self>) -> T {
        unsafe { self.handle.update(self.value.get()) };
        let handle = unsafe { Pin::new_unchecked(&self.handle) };
        handle.register_as_dependency_to_current_binding(
            #[cfg(slint_debug_property)]
            self.debug_name.borrow().as_str(),
        );
        self.get_internal()
    }

    /// Same as get() but without registering a dependency
    ///
    /// This allow to optimize bindings that know that they might not need to
    /// re_evaluate themselves when the property change or that have registered
    /// the dependency in another way.
    ///
    /// ## Example
    /// ```
    /// use std::rc::Rc;
    /// use i_slint_core::Property;
    /// let prop1 = Rc::pin(Property::new(100));
    /// let prop2 = Rc::pin(Property::<i32>::default());
    /// prop2.as_ref().set_binding({
    ///     let prop1 = prop1.clone(); // in order to move it into the closure.
    ///     move || { prop1.as_ref().get_untracked() + 30 }
    /// });
    /// assert_eq!(prop2.as_ref().get(), 130);
    /// prop1.set(200);
    /// // changing prop1 do not affect the prop2 binding because no dependency was registered
    /// assert_eq!(prop2.as_ref().get(), 130);
    /// ```
    pub fn get_untracked(self: Pin<&Self>) -> T {
        unsafe { self.handle.update(self.value.get()) };
        self.get_internal()
    }

    /// Get the cached value without registering any dependencies or executing any binding
    pub fn get_internal(&self) -> T {
        self.handle.access(|_| {
            // Safety: PropertyHandle::access ensure that the value is locked
            unsafe { (*self.value.get()).clone() }
        })
    }

    /// Change the value of this property
    ///
    /// If other properties have binding depending of this property, these properties will
    /// be marked as dirty.
    // FIXME  pub fn set(self: Pin<&Self>, t: T) {
    pub fn set(&self, t: T)
    where
        T: PartialEq,
    {
        let previous_binding_intercepted = self.handle.access(|b| {
            b.is_some_and(|b| unsafe {
                // Safety: b is a BindingHolder<T>
                (b.vtable.intercept_set)(&*b as *const BindingHolder, &t as *const T as *const ())
            })
        });
        if !previous_binding_intercepted {
            self.handle.remove_binding();
        }

        // Safety: PropertyHandle::access ensure that the value is locked
        let has_value_changed = self.handle.access(|_| unsafe {
            *self.value.get() != t && {
                *self.value.get() = t;
                true
            }
        });
        if has_value_changed {
            self.handle.mark_dirty(
                #[cfg(slint_debug_property)]
                self.debug_name.borrow().as_str(),
            );
        }
    }

    /// Set a binding to this property.
    ///
    /// Bindings are evaluated lazily from calling get, and the return value of the binding
    /// is the new value.
    ///
    /// If other properties have bindings depending of this property, these properties will
    /// be marked as dirty.
    ///
    /// Closures of type `Fn()->T` implements `Binding<T>` and can be used as a binding
    ///
    /// ## Example
    /// ```
    /// use std::rc::Rc;
    /// use i_slint_core::Property;
    /// let prop1 = Rc::pin(Property::new(100));
    /// let prop2 = Rc::pin(Property::<i32>::default());
    /// prop2.as_ref().set_binding({
    ///     let prop1 = prop1.clone(); // in order to move it into the closure.
    ///     move || { prop1.as_ref().get() + 30 }
    /// });
    /// assert_eq!(prop2.as_ref().get(), 130);
    /// prop1.set(200);
    /// // A change in prop1 forced the binding on prop2 to re_evaluate
    /// assert_eq!(prop2.as_ref().get(), 230);
    /// ```
    //FIXME pub fn set_binding(self: Pin<&Self>, f: impl Binding<T> + 'static) {
    pub fn set_binding(&self, binding: impl Binding<T> + 'static) {
        // Safety: This will make a binding callable for the type T
        unsafe {
            self.handle.set_binding(
                move |val: *mut ()| {
                    let val = &mut *(val as *mut T);
                    *val = binding.evaluate(val);
                    BindingResult::KeepBinding
                },
                #[cfg(slint_debug_property)]
                self.debug_name.borrow().as_str(),
            )
        }
        self.handle.mark_dirty(
            #[cfg(slint_debug_property)]
            self.debug_name.borrow().as_str(),
        );
    }

    /// Any of the properties accessed during the last evaluation of the closure called
    /// from the last call to evaluate is potentially dirty.
    pub fn is_dirty(&self) -> bool {
        self.handle.access(|binding| binding.is_some_and(|b| b.dirty.get()))
    }

    /// Internal function to mark the property as dirty and notify dependencies, regardless of
    /// whether the property value has actually changed or not.
    pub fn mark_dirty(&self) {
        self.handle.mark_dirty(
            #[cfg(slint_debug_property)]
            self.debug_name.borrow().as_str(),
        )
    }

    /// Mark that this property will never be modified again and that no tracking should be done
    pub fn set_constant(&self) {
        self.handle.set_constant();
    }
}

#[test]
fn properties_simple_test() {
    use pin_weak::rc::PinWeak;
    use std::rc::Rc;
    fn g(prop: &Property<i32>) -> i32 {
        unsafe { Pin::new_unchecked(prop).get() }
    }

    #[derive(Default)]
    struct Component {
        width: Property<i32>,
        height: Property<i32>,
        area: Property<i32>,
    }

    let compo = Rc::pin(Component::default());
    let w = PinWeak::downgrade(compo.clone());
    compo.area.set_binding(move || {
        let compo = w.upgrade().unwrap();
        g(&compo.width) * g(&compo.height)
    });
    compo.width.set(4);
    compo.height.set(8);
    assert_eq!(g(&compo.width), 4);
    assert_eq!(g(&compo.height), 8);
    assert_eq!(g(&compo.area), 4 * 8);

    let w = PinWeak::downgrade(compo.clone());
    compo.width.set_binding(move || {
        let compo = w.upgrade().unwrap();
        g(&compo.height) * 2
    });
    assert_eq!(g(&compo.width), 8 * 2);
    assert_eq!(g(&compo.height), 8);
    assert_eq!(g(&compo.area), 8 * 8 * 2);
}

impl<T: PartialEq + Clone + 'static> Property<T> {
    /// Link two property such that any change to one property is affecting the other property as if they
    /// where, in fact, a single property.
    /// The value or binding of prop2 is kept.
    pub fn link_two_way(prop1: Pin<&Self>, prop2: Pin<&Self>) {
        struct TwoWayBinding<T> {
            common_property: Pin<Rc<Property<T>>>,
        }
        unsafe impl<T: PartialEq + Clone + 'static> BindingCallable for TwoWayBinding<T> {
            unsafe fn evaluate(self: Pin<&Self>, value: *mut ()) -> BindingResult {
                *(value as *mut T) = self.common_property.as_ref().get();
                BindingResult::KeepBinding
            }

            unsafe fn intercept_set(self: Pin<&Self>, value: *const ()) -> bool {
                self.common_property.as_ref().set((*(value as *const T)).clone());
                true
            }

            unsafe fn intercept_set_binding(
                self: Pin<&Self>,
                new_binding: *mut BindingHolder,
            ) -> bool {
                self.common_property.handle.set_binding_impl(new_binding);
                true
            }

            const IS_TWO_WAY_BINDING: bool = true;
        }

        #[cfg(slint_debug_property)]
        let debug_name =
            alloc::format!("<{}<=>{}>", prop1.debug_name.borrow(), prop2.debug_name.borrow());

        let value = prop2.get_internal();

        let prop1_handle_val = prop1.handle.handle.get();
        if prop1_handle_val & 0b10 == 0b10 {
            // Safety: the handle is a pointer to a binding
            let holder = unsafe { &*((prop1_handle_val & !0b11) as *const BindingHolder) };
            if holder.is_two_way_binding {
                unsafe {
                    // Safety: the handle is a pointer to a binding whose B is a TwoWayBinding<T>
                    let holder =
                        &*((prop1_handle_val & !0b11) as *const BindingHolder<TwoWayBinding<T>>);
                    // Safety: TwoWayBinding's T is the same as the type for both properties
                    prop2.handle.set_binding(
                        TwoWayBinding { common_property: holder.binding.common_property.clone() },
                        #[cfg(slint_debug_property)]
                        debug_name.as_str(),
                    );
                }
                prop2.set(value);
                return;
            }
        };

        let prop2_handle_val = prop2.handle.handle.get();
        let handle = if prop2_handle_val & 0b10 == 0b10 {
            // Safety: the handle is a pointer to a binding
            let holder = unsafe { &*((prop2_handle_val & !0b11) as *const BindingHolder) };
            if holder.is_two_way_binding {
                unsafe {
                    // Safety: the handle is a pointer to a binding whose B is a TwoWayBinding<T>
                    let holder =
                        &*((prop2_handle_val & !0b11) as *const BindingHolder<TwoWayBinding<T>>);
                    // Safety: TwoWayBinding's T is the same as the type for both properties
                    prop1.handle.set_binding(
                        TwoWayBinding { common_property: holder.binding.common_property.clone() },
                        #[cfg(slint_debug_property)]
                        debug_name.as_str(),
                    );
                }
                return;
            }
            // If prop2 is a binding, just "steal it"
            prop2.handle.handle.set(0);
            PropertyHandle { handle: Cell::new(prop2_handle_val) }
        } else {
            PropertyHandle::default()
        };

        let common_property = Rc::pin(Property {
            handle,
            value: UnsafeCell::new(value),
            pinned: PhantomPinned,
            #[cfg(slint_debug_property)]
            debug_name: debug_name.clone().into(),
        });
        // Safety: TwoWayBinding's T is the same as the type for both properties
        unsafe {
            prop1.handle.set_binding(
                TwoWayBinding { common_property: common_property.clone() },
                #[cfg(slint_debug_property)]
                debug_name.as_str(),
            );
            prop2.handle.set_binding(
                TwoWayBinding { common_property },
                #[cfg(slint_debug_property)]
                debug_name.as_str(),
            );
        }
    }
}

#[test]
fn property_two_ways_test() {
    let p1 = Rc::pin(Property::new(42));
    let p2 = Rc::pin(Property::new(88));

    let depends = Box::pin(Property::new(0));
    depends.as_ref().set_binding({
        let p1 = p1.clone();
        move || p1.as_ref().get() + 8
    });
    assert_eq!(depends.as_ref().get(), 42 + 8);
    Property::link_two_way(p1.as_ref(), p2.as_ref());
    assert_eq!(p1.as_ref().get(), 88);
    assert_eq!(p2.as_ref().get(), 88);
    assert_eq!(depends.as_ref().get(), 88 + 8);
    p2.as_ref().set(5);
    assert_eq!(p1.as_ref().get(), 5);
    assert_eq!(p2.as_ref().get(), 5);
    assert_eq!(depends.as_ref().get(), 5 + 8);
    p1.as_ref().set(22);
    assert_eq!(p1.as_ref().get(), 22);
    assert_eq!(p2.as_ref().get(), 22);
    assert_eq!(depends.as_ref().get(), 22 + 8);
}

#[test]
fn property_two_ways_test_binding() {
    let p1 = Rc::pin(Property::new(42));
    let p2 = Rc::pin(Property::new(88));
    let global = Rc::pin(Property::new(23));
    p2.as_ref().set_binding({
        let global = global.clone();
        move || global.as_ref().get() + 9
    });

    let depends = Box::pin(Property::new(0));
    depends.as_ref().set_binding({
        let p1 = p1.clone();
        move || p1.as_ref().get() + 8
    });

    Property::link_two_way(p1.as_ref(), p2.as_ref());
    assert_eq!(p1.as_ref().get(), 23 + 9);
    assert_eq!(p2.as_ref().get(), 23 + 9);
    assert_eq!(depends.as_ref().get(), 23 + 9 + 8);
    global.as_ref().set(55);
    assert_eq!(p1.as_ref().get(), 55 + 9);
    assert_eq!(p2.as_ref().get(), 55 + 9);
    assert_eq!(depends.as_ref().get(), 55 + 9 + 8);
}

#[test]
fn property_two_ways_recurse_from_binding() {
    let xx = Rc::pin(Property::new(0));

    let p1 = Rc::pin(Property::new(42));
    let p2 = Rc::pin(Property::new(88));
    let global = Rc::pin(Property::new(23));

    let done = Rc::new(Cell::new(false));
    xx.set_binding({
        let p1 = p1.clone();
        let p2 = p2.clone();
        let global = global.clone();
        let xx_weak = pin_weak::rc::PinWeak::downgrade(xx.clone());
        move || {
            if !done.get() {
                done.set(true);
                Property::link_two_way(p1.as_ref(), p2.as_ref());
                let xx_weak = xx_weak.clone();
                p1.as_ref().set_binding(move || xx_weak.upgrade().unwrap().as_ref().get() + 9);
            }
            global.as_ref().get() + 2
        }
    });
    assert_eq!(xx.as_ref().get(), 23 + 2);
    assert_eq!(p1.as_ref().get(), 23 + 2 + 9);
    assert_eq!(p2.as_ref().get(), 23 + 2 + 9);

    global.as_ref().set(55);
    assert_eq!(p1.as_ref().get(), 55 + 2 + 9);
    assert_eq!(p2.as_ref().get(), 55 + 2 + 9);
    assert_eq!(xx.as_ref().get(), 55 + 2);
}

#[test]
fn property_two_ways_binding_of_two_way_binding_first() {
    let p1_1 = Rc::pin(Property::new(2));
    let p1_2 = Rc::pin(Property::new(4));
    Property::link_two_way(p1_1.as_ref(), p1_2.as_ref());

    assert_eq!(p1_1.as_ref().get(), 4);
    assert_eq!(p1_2.as_ref().get(), 4);

    let p2 = Rc::pin(Property::new(3));
    Property::link_two_way(p1_1.as_ref(), p2.as_ref());

    assert_eq!(p1_1.as_ref().get(), 3);
    assert_eq!(p1_2.as_ref().get(), 3);
    assert_eq!(p2.as_ref().get(), 3);

    p1_1.set(6);

    assert_eq!(p1_1.as_ref().get(), 6);
    assert_eq!(p1_2.as_ref().get(), 6);
    assert_eq!(p2.as_ref().get(), 6);

    p1_2.set(8);

    assert_eq!(p1_1.as_ref().get(), 8);
    assert_eq!(p1_2.as_ref().get(), 8);
    assert_eq!(p2.as_ref().get(), 8);

    p2.set(7);

    assert_eq!(p1_1.as_ref().get(), 7);
    assert_eq!(p1_2.as_ref().get(), 7);
    assert_eq!(p2.as_ref().get(), 7);
}

#[test]
fn property_two_ways_binding_of_two_way_binding_second() {
    let p1 = Rc::pin(Property::new(2));
    let p2_1 = Rc::pin(Property::new(3));
    let p2_2 = Rc::pin(Property::new(5));
    Property::link_two_way(p2_1.as_ref(), p2_2.as_ref());

    assert_eq!(p2_1.as_ref().get(), 5);
    assert_eq!(p2_2.as_ref().get(), 5);

    Property::link_two_way(p1.as_ref(), p2_2.as_ref());

    assert_eq!(p1.as_ref().get(), 5);
    assert_eq!(p2_1.as_ref().get(), 5);
    assert_eq!(p2_2.as_ref().get(), 5);

    p1.set(6);

    assert_eq!(p1.as_ref().get(), 6);
    assert_eq!(p2_1.as_ref().get(), 6);
    assert_eq!(p2_2.as_ref().get(), 6);

    p2_1.set(7);

    assert_eq!(p1.as_ref().get(), 7);
    assert_eq!(p2_1.as_ref().get(), 7);
    assert_eq!(p2_2.as_ref().get(), 7);

    p2_2.set(9);

    assert_eq!(p1.as_ref().get(), 9);
    assert_eq!(p2_1.as_ref().get(), 9);
    assert_eq!(p2_2.as_ref().get(), 9);
}

#[test]
fn property_two_ways_binding_of_two_two_way_bindings() {
    let p1_1 = Rc::pin(Property::new(2));
    let p1_2 = Rc::pin(Property::new(4));
    Property::link_two_way(p1_1.as_ref(), p1_2.as_ref());
    assert_eq!(p1_1.as_ref().get(), 4);
    assert_eq!(p1_2.as_ref().get(), 4);

    let p2_1 = Rc::pin(Property::new(3));
    let p2_2 = Rc::pin(Property::new(5));
    Property::link_two_way(p2_1.as_ref(), p2_2.as_ref());

    assert_eq!(p2_1.as_ref().get(), 5);
    assert_eq!(p2_2.as_ref().get(), 5);

    Property::link_two_way(p1_1.as_ref(), p2_2.as_ref());

    assert_eq!(p1_1.as_ref().get(), 5);
    assert_eq!(p1_2.as_ref().get(), 5);
    assert_eq!(p2_1.as_ref().get(), 5);
    assert_eq!(p2_2.as_ref().get(), 5);

    p1_1.set(6);
    assert_eq!(p1_1.as_ref().get(), 6);
    assert_eq!(p1_2.as_ref().get(), 6);
    assert_eq!(p2_1.as_ref().get(), 6);
    assert_eq!(p2_2.as_ref().get(), 6);

    p1_2.set(8);
    assert_eq!(p1_1.as_ref().get(), 8);
    assert_eq!(p1_2.as_ref().get(), 8);
    assert_eq!(p2_1.as_ref().get(), 8);
    assert_eq!(p2_2.as_ref().get(), 8);

    p2_1.set(7);
    assert_eq!(p1_1.as_ref().get(), 7);
    assert_eq!(p1_2.as_ref().get(), 7);
    assert_eq!(p2_1.as_ref().get(), 7);
    assert_eq!(p2_2.as_ref().get(), 7);

    p2_2.set(9);
    assert_eq!(p1_1.as_ref().get(), 9);
    assert_eq!(p1_2.as_ref().get(), 9);
    assert_eq!(p2_1.as_ref().get(), 9);
    assert_eq!(p2_2.as_ref().get(), 9);
}

mod change_tracker;
pub use change_tracker::*;
mod properties_animations;
pub use crate::items::StateInfo;
pub use properties_animations::*;

struct StateInfoBinding<F> {
    dirty_time: Cell<Option<crate::animations::Instant>>,
    binding: F,
}

unsafe impl<F: Fn() -> i32> crate::properties::BindingCallable for StateInfoBinding<F> {
    unsafe fn evaluate(self: Pin<&Self>, value: *mut ()) -> BindingResult {
        // Safety: We should only set this binding on a property of type StateInfo
        let value = &mut *(value as *mut StateInfo);
        let new_state = (self.binding)();
        let timestamp = self.dirty_time.take();
        if new_state != value.current_state {
            value.previous_state = value.current_state;
            value.change_time = timestamp.unwrap_or_else(crate::animations::current_tick);
            value.current_state = new_state;
        }
        BindingResult::KeepBinding
    }

    fn mark_dirty(self: Pin<&Self>) {
        if self.dirty_time.get().is_none() {
            self.dirty_time.set(Some(crate::animations::current_tick()))
        }
    }
}

/// Sets a binding that returns a state to a StateInfo property
pub fn set_state_binding(property: Pin<&Property<StateInfo>>, binding: impl Fn() -> i32 + 'static) {
    let bind_callable = StateInfoBinding { dirty_time: Cell::new(None), binding };
    // Safety: The StateInfoBinding is a BindingCallable for type StateInfo
    unsafe {
        property.handle.set_binding(
            bind_callable,
            #[cfg(slint_debug_property)]
            property.debug_name.borrow().as_str(),
        )
    }
}

#[doc(hidden)]
pub trait PropertyDirtyHandler {
    fn notify(self: Pin<&Self>);
}

impl PropertyDirtyHandler for () {
    fn notify(self: Pin<&Self>) {}
}

impl<F: Fn()> PropertyDirtyHandler for F {
    fn notify(self: Pin<&Self>) {
        (self.get_ref())()
    }
}

/// This structure allow to run a closure that queries properties, and can report
/// if any property we accessed have become dirty
pub struct PropertyTracker<DirtyHandler = ()> {
    holder: BindingHolder<DirtyHandler>,
}

impl Default for PropertyTracker<()> {
    fn default() -> Self {
        static VT: &BindingVTable = &BindingVTable {
            drop: |_| (),
            evaluate: |_, _| BindingResult::KeepBinding,
            mark_dirty: |_, _| (),
            intercept_set: |_, _| false,
            intercept_set_binding: |_, _| false,
        };

        let holder = BindingHolder {
            dependencies: Cell::new(0),
            dep_nodes: Default::default(),
            vtable: VT,
            dirty: Cell::new(true), // starts dirty so it evaluates the property when used
            is_two_way_binding: false,
            pinned: PhantomPinned,
            binding: (),
            #[cfg(slint_debug_property)]
            debug_name: "<PropertyTracker<()>>".into(),
        };
        Self { holder }
    }
}

impl<DirtyHandler> Drop for PropertyTracker<DirtyHandler> {
    fn drop(&mut self) {
        unsafe {
            DependencyListHead::drop(self.holder.dependencies.as_ptr() as *mut DependencyListHead);
        }
    }
}

impl<DirtyHandler: PropertyDirtyHandler> PropertyTracker<DirtyHandler> {
    #[cfg(slint_debug_property)]
    /// set the debug name when `cfg(slint_debug_property`
    pub fn set_debug_name(&mut self, debug_name: alloc::string::String) {
        self.holder.debug_name = debug_name;
    }

    /// Register this property tracker as a dependency to the current binding/property tracker being evaluated
    pub fn register_as_dependency_to_current_binding(self: Pin<&Self>) {
        if CURRENT_BINDING.is_set() {
            CURRENT_BINDING.with(|cur_binding| {
                if let Some(cur_binding) = cur_binding {
                    debug_assert!(!core::ptr::eq(
                        self.holder.dependencies.get() as *const u32,
                        (&CONSTANT_PROPERTY_SENTINEL) as *const u32,
                    ));
                    cur_binding.register_self_as_dependency(
                        self.holder.dependencies.as_ptr() as *mut DependencyListHead,
                        #[cfg(slint_debug_property)]
                        &self.holder.debug_name,
                    );
                }
            });
        }
    }

    /// Any of the properties accessed during the last evaluation of the closure called
    /// from the last call to evaluate is potentially dirty.
    pub fn is_dirty(&self) -> bool {
        self.holder.dirty.get()
    }

    /// Evaluate the function, and record dependencies of properties accessed within this function.
    /// If this is called during the evaluation of another property binding or property tracker, then
    /// any changes to accessed properties will also mark the other binding/tracker dirty.
    pub fn evaluate<R>(self: Pin<&Self>, f: impl FnOnce() -> R) -> R {
        self.register_as_dependency_to_current_binding();
        self.evaluate_as_dependency_root(f)
    }

    /// Evaluate the function, and record dependencies of properties accessed within this function.
    /// If this is called during the evaluation of another property binding or property tracker, then
    /// any changes to accessed properties will not propagate to the other tracker.
    pub fn evaluate_as_dependency_root<R>(self: Pin<&Self>, f: impl FnOnce() -> R) -> R {
        // clear all the nodes so that we can start from scratch
        self.holder.dep_nodes.set(Default::default());

        // Safety: it is safe to project the holder as we don't implement drop or unpin
        let pinned_holder = unsafe {
            self.map_unchecked(|s| {
                core::mem::transmute::<&BindingHolder<DirtyHandler>, &BindingHolder<()>>(&s.holder)
            })
        };
        let r = CURRENT_BINDING.set(Some(pinned_holder), f);
        self.holder.dirty.set(false);
        r
    }

    /// Call [`Self::evaluate`] if and only if it is dirty.
    /// But register a dependency in any case.
    pub fn evaluate_if_dirty<R>(self: Pin<&Self>, f: impl FnOnce() -> R) -> Option<R> {
        self.register_as_dependency_to_current_binding();
        self.is_dirty().then(|| self.evaluate_as_dependency_root(f))
    }

    /// Mark this PropertyTracker as dirty
    pub fn set_dirty(&self) {
        self.holder.dirty.set(true);
        unsafe { mark_dependencies_dirty(self.holder.dependencies.as_ptr() as *mut _) };
    }

    /// Sets the specified callback handler function, which will be called if any
    /// properties that this tracker depends on becomes dirty.
    ///
    /// The `handler` `PropertyDirtyHandler` is a trait which is implemented for
    /// any `Fn()` closure
    ///
    /// Note that the handler will be invoked immediately when a property is modified or
    /// marked as dirty. In particular, the involved property are still in a locked
    /// state and should not be accessed while the handler is run. This function can be
    /// useful to mark some work to be done later.
    pub fn new_with_dirty_handler(handler: DirtyHandler) -> Self {
        /// Safety: _self must be a pointer to a `BindingHolder<DirtyHandler>`
        unsafe fn mark_dirty<B: PropertyDirtyHandler>(
            _self: *const BindingHolder,
            was_dirty: bool,
        ) {
            if !was_dirty {
                Pin::new_unchecked(&(*(_self as *const BindingHolder<B>)).binding).notify();
            }
        }

        trait HasBindingVTable {
            const VT: &'static BindingVTable;
        }
        impl<B: PropertyDirtyHandler> HasBindingVTable for B {
            const VT: &'static BindingVTable = &BindingVTable {
                drop: |_| (),
                evaluate: |_, _| BindingResult::KeepBinding,
                mark_dirty: mark_dirty::<B>,
                intercept_set: |_, _| false,
                intercept_set_binding: |_, _| false,
            };
        }

        let holder = BindingHolder {
            dependencies: Cell::new(0),
            dep_nodes: Default::default(),
            vtable: <DirtyHandler as HasBindingVTable>::VT,
            dirty: Cell::new(true), // starts dirty so it evaluates the property when used
            is_two_way_binding: false,
            pinned: PhantomPinned,
            binding: handler,
            #[cfg(slint_debug_property)]
            debug_name: "<PropertyTracker>".into(),
        };
        Self { holder }
    }
}

#[test]
fn test_property_listener_scope() {
    let scope = Box::pin(PropertyTracker::default());
    let prop1 = Box::pin(Property::new(42));
    assert!(scope.is_dirty()); // It is dirty at the beginning

    let r = scope.as_ref().evaluate(|| prop1.as_ref().get());
    assert_eq!(r, 42);
    assert!(!scope.is_dirty()); // It is no longer dirty
    prop1.as_ref().set(88);
    assert!(scope.is_dirty()); // now dirty for prop1 changed.
    let r = scope.as_ref().evaluate(|| prop1.as_ref().get() + 1);
    assert_eq!(r, 89);
    assert!(!scope.is_dirty());
    let r = scope.as_ref().evaluate(|| 12);
    assert_eq!(r, 12);
    assert!(!scope.is_dirty());
    prop1.as_ref().set(1);
    assert!(!scope.is_dirty());
    scope.as_ref().evaluate_if_dirty(|| panic!("should not be dirty"));
    scope.set_dirty();
    let mut ok = false;
    scope.as_ref().evaluate_if_dirty(|| ok = true);
    assert!(ok);
}

#[test]
fn test_nested_property_trackers() {
    let tracker1 = Box::pin(PropertyTracker::default());
    let tracker2 = Box::pin(PropertyTracker::default());
    let prop = Box::pin(Property::new(42));

    let r = tracker1.as_ref().evaluate(|| tracker2.as_ref().evaluate(|| prop.as_ref().get()));
    assert_eq!(r, 42);

    prop.as_ref().set(1);
    assert!(tracker2.as_ref().is_dirty());
    assert!(tracker1.as_ref().is_dirty());

    let r = tracker1
        .as_ref()
        .evaluate(|| tracker2.as_ref().evaluate_as_dependency_root(|| prop.as_ref().get()));
    assert_eq!(r, 1);
    prop.as_ref().set(100);
    assert!(tracker2.as_ref().is_dirty());
    assert!(!tracker1.as_ref().is_dirty());
}

#[test]
fn test_property_dirty_handler() {
    let call_flag = Rc::new(Cell::new(false));
    let tracker = Box::pin(PropertyTracker::new_with_dirty_handler({
        let call_flag = call_flag.clone();
        move || {
            (*call_flag).set(true);
        }
    }));
    let prop = Box::pin(Property::new(42));

    let r = tracker.as_ref().evaluate(|| prop.as_ref().get());

    assert_eq!(r, 42);
    assert!(!tracker.as_ref().is_dirty());
    assert!(!call_flag.get());

    prop.as_ref().set(100);
    assert!(tracker.as_ref().is_dirty());
    assert!(call_flag.get());

    // Repeated changes before evaluation should not trigger further
    // change handler calls, otherwise it would be a notification storm.
    call_flag.set(false);
    prop.as_ref().set(101);
    assert!(tracker.as_ref().is_dirty());
    assert!(!call_flag.get());
}

#[test]
fn test_property_tracker_drop() {
    let outer_tracker = Box::pin(PropertyTracker::default());
    let inner_tracker = Box::pin(PropertyTracker::default());
    let prop = Box::pin(Property::new(42));

    let r =
        outer_tracker.as_ref().evaluate(|| inner_tracker.as_ref().evaluate(|| prop.as_ref().get()));
    assert_eq!(r, 42);

    drop(inner_tracker);
    prop.as_ref().set(200); // don't crash
}

#[test]
fn test_nested_property_tracker_dirty() {
    let outer_tracker = Box::pin(PropertyTracker::default());
    let inner_tracker = Box::pin(PropertyTracker::default());
    let prop = Box::pin(Property::new(42));

    let r =
        outer_tracker.as_ref().evaluate(|| inner_tracker.as_ref().evaluate(|| prop.as_ref().get()));
    assert_eq!(r, 42);

    assert!(!outer_tracker.is_dirty());
    assert!(!inner_tracker.is_dirty());

    // Let's pretend that there was another dependency unaccounted first, mark the inner tracker as dirty
    // by hand.
    inner_tracker.as_ref().set_dirty();
    assert!(outer_tracker.is_dirty());
}

#[test]
#[allow(clippy::redundant_closure)]
fn test_nested_property_tracker_evaluate_if_dirty() {
    let outer_tracker = Box::pin(PropertyTracker::default());
    let inner_tracker = Box::pin(PropertyTracker::default());
    let prop = Box::pin(Property::new(42));

    let mut cache = 0;
    let mut cache_or_evaluate = || {
        if let Some(x) = inner_tracker.as_ref().evaluate_if_dirty(|| prop.as_ref().get() + 1) {
            cache = x;
        }
        cache
    };
    let r = outer_tracker.as_ref().evaluate(|| cache_or_evaluate());
    assert_eq!(r, 43);
    assert!(!outer_tracker.is_dirty());
    assert!(!inner_tracker.is_dirty());
    prop.as_ref().set(11);
    assert!(outer_tracker.is_dirty());
    assert!(inner_tracker.is_dirty());
    let r = outer_tracker.as_ref().evaluate(|| cache_or_evaluate());
    assert_eq!(r, 12);
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi;
