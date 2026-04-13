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

/// A singly linked list whose nodes are pinned in raw allocations.
/// Nodes are also referenced through external raw pointers in the
/// dependency tracking system.
mod single_linked_list_pin {
    #![allow(unsafe_code)]
    use core::pin::Pin;
    use core::ptr::NonNull;

    type NodePtr<T> = Option<NonNull<SingleLinkedListPinNode<T>>>;
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
            // Iterative drop to avoid stack overflow on long lists.
            let mut cur = self.0.take();
            while let Some(node) = cur {
                // Safety: we own this node.
                // drop_in_place keeps the value at its pinned address.
                unsafe {
                    cur = (*node.as_ptr()).next;
                    core::ptr::drop_in_place(&raw mut (*node.as_ptr()).value);
                    alloc::alloc::dealloc(
                        node.as_ptr().cast(),
                        core::alloc::Layout::new::<SingleLinkedListPinNode<T>>(),
                    );
                }
            }
        }
    }

    impl<T> SingleLinkedListPinHead<T> {
        pub fn push_front(&mut self, value: T) -> Pin<&T> {
            let node = SingleLinkedListPinNode { next: self.0.take(), value };
            // Safety: raw allocation, written once and never moved
            let ptr = unsafe {
                let layout = core::alloc::Layout::new::<SingleLinkedListPinNode<T>>();
                let mem = alloc::alloc::alloc(layout) as *mut SingleLinkedListPinNode<T>;
                assert!(!mem.is_null(), "allocation failed");
                core::ptr::write(mem, node);
                NonNull::new_unchecked(mem)
            };
            self.0 = Some(ptr);
            // Safety: the value is pinned because we never move it out of the allocation
            unsafe { Pin::new_unchecked(&(*ptr.as_ptr()).value) }
        }

        #[allow(unused)]
        pub fn iter(&self) -> impl Iterator<Item = Pin<&T>> {
            struct I<'a, T>(&'a NodePtr<T>);

            impl<'a, T> Iterator for I<'a, T> {
                type Item = Pin<&'a T>;
                fn next(&mut self) -> Option<Self::Item> {
                    if let Some(node) = self.0 {
                        // Safety: node is a valid allocation we own
                        let r = unsafe { Pin::new_unchecked(&(*node.as_ptr()).value) };
                        self.0 = unsafe { &(*node.as_ptr()).next };
                        Some(r)
                    } else {
                        None
                    }
                }
            }
            I(&self.0)
        }

        /// Returns true if the list is empty
        pub fn is_empty(&self) -> bool {
            self.0.is_none()
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
            unsafe {
                (*to).0.set((*from).0.get());
                if let Some(next) = (*from).0.get().as_ref() {
                    debug_assert_eq!(from as *const _, next.prev.get() as *const _);
                    next.debug_assert_valid();
                    next.prev.set(to as *const _);
                    next.debug_assert_valid();
                }
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
            unsafe {
                if let Some(next) = (*_self).0.get().as_ref() {
                    #[cfg(not(miri))]
                    debug_assert_eq!(_self as *const _, next.prev.get() as *const _);
                    next.debug_assert_valid();
                    next.prev.set(core::ptr::null());
                    next.debug_assert_valid();
                }
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
            // Under Miri with Tree Borrows, reading through prev/next creates
            // foreign accesses that conflict with active protectors.
            #[cfg(not(miri))]
            unsafe {
                debug_assert!(
                    self.prev.get().is_null() || core::ptr::eq((*self.prev.get()).get(), self)
                );
                debug_assert!(
                    self.next.get().is_null()
                        || core::ptr::eq((*self.next.get()).prev.get(), &self.next)
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
use core::cell::{Cell, RefCell, UnsafeCell};
use core::ffi::c_void;
use core::marker::PhantomPinned;
use core::pin::Pin;

/// if a DependencyListHead points to that value, it is because the property is actually
/// constant and cannot have dependencies
static CONSTANT_PROPERTY_SENTINEL: u32 = 0;

#[inline(always)]
fn const_sentinel() -> *mut () {
    (&CONSTANT_PROPERTY_SENTINEL) as *const u32 as *mut ()
}

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
    evaluate: unsafe fn(_self: *const BindingHolder, value: *mut c_void) -> BindingResult,
    mark_dirty: unsafe fn(_self: *const BindingHolder, was_dirty: bool),
    intercept_set: unsafe fn(_self: *const BindingHolder, value: *const c_void) -> bool,
    intercept_set_binding:
        unsafe fn(_self: *const BindingHolder, new_binding: *mut BindingHolder) -> bool,
}

/// A binding trait object can be used to dynamically produces values for a property.
///
/// # Safety
///
/// IS_TWO_WAY_BINDING cannot be true if Self is not a TwoWayBinding
unsafe trait BindingCallable<T> {
    /// This function is called by the property to evaluate the binding and produce a new value. The
    /// previous property value is provided in the value parameter.
    fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult;

    /// This function is used to notify the binding that one of the dependencies was changed
    /// and therefore this binding may evaluate to a different value, too.
    fn mark_dirty(self: Pin<&Self>) {}

    /// Allow the binding to intercept what happens when the value is set.
    /// The default implementation returns false, meaning the binding will simply be removed and
    /// the property will get the new value.
    /// When returning true, the call was intercepted and the binding will not be removed,
    /// but the property will still have that value
    fn intercept_set(self: Pin<&Self>, _value: &T) -> bool {
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

unsafe impl<T, F: Fn(&mut T) -> BindingResult> BindingCallable<T> for F {
    fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
        self(value)
    }
}

/// Stores a raw pointer to the binding currently being evaluated.
mod current_binding_storage {
    use super::BindingHolder;
    use core::cell::Cell;

    #[cfg(feature = "std")]
    std::thread_local! {
        static CURRENT_BINDING: Cell<*const BindingHolder> = const { Cell::new(core::ptr::null()) };
    }

    #[cfg(feature = "std")]
    pub(super) fn set<T>(value: Option<*const BindingHolder>, f: impl FnOnce() -> T) -> T {
        CURRENT_BINDING.with(|cell| {
            let old = cell.replace(value.unwrap_or(core::ptr::null()));
            let res = f();
            cell.set(old);
            res
        })
    }

    #[cfg(feature = "std")]
    pub(super) fn with<T>(f: impl FnOnce(Option<*const BindingHolder>) -> T) -> T {
        CURRENT_BINDING.with(|cell| {
            let ptr = cell.get();
            f(if ptr.is_null() { None } else { Some(ptr) })
        })
    }

    #[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
    static CURRENT_BINDING: ScopedRawPtr = ScopedRawPtr(Cell::new(core::ptr::null()));

    #[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
    struct ScopedRawPtr(Cell<*const BindingHolder>);
    // Safety: the unsafe_single_threaded feature means only one thread accesses this
    #[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
    unsafe impl Send for ScopedRawPtr {}
    #[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
    unsafe impl Sync for ScopedRawPtr {}

    #[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
    pub(super) fn set<T>(value: Option<*const BindingHolder>, f: impl FnOnce() -> T) -> T {
        let old = CURRENT_BINDING.0.replace(value.unwrap_or(core::ptr::null()));
        let res = f();
        CURRENT_BINDING.0.set(old);
        res
    }

    #[cfg(all(not(feature = "std"), feature = "unsafe-single-threaded"))]
    pub(super) fn with<T>(f: impl FnOnce(Option<*const BindingHolder>) -> T) -> T {
        let ptr = CURRENT_BINDING.0.get();
        f(if ptr.is_null() { None } else { Some(ptr) })
    }
}

/// Evaluate a function without registering any property dependencies.
pub fn evaluate_no_tracking<T>(f: impl FnOnce() -> T) -> T {
    current_binding_storage::set(None, f)
}

/// Returns true if a binding is currently being evaluated
/// so that property accesses register dependencies.
pub fn is_currently_tracking() -> bool {
    current_binding_storage::with(|x| x.is_some())
}

/// This structure erase the `B` type with a vtable.
#[repr(C)]
struct BindingHolder<B = ()> {
    /// Head of the list of bindings that depend on this binding.
    dependencies: Cell<*mut ()>,
    /// Nodes that link this binding into the dependency lists of
    /// the properties it reads.
    /// UnsafeCell allows in-place mutation without moving the allocation.
    dep_nodes: UnsafeCell<single_linked_list_pin::SingleLinkedListPinHead<DependencyNode>>,
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
    /// Registers this binding as a dependency of the given property.
    fn register_self_as_dependency(
        self_ptr: *const BindingHolder,
        property_that_will_notify: *mut DependencyListHead,
        #[cfg(slint_debug_property)] _other_debug_name: &str,
    ) {
        let node = DependencyNode::new(self_ptr);
        // Safety: self_ptr is valid and pinned
        unsafe {
            let dep_nodes = &mut *(*self_ptr).dep_nodes.get();
            let node = dep_nodes.push_front(node);
            DependencyListHead::append(&*property_that_will_notify, node);
        }
    }
}

fn alloc_binding_holder<T, B: BindingCallable<T> + 'static>(binding: B) -> *mut BindingHolder {
    /// Safety: _self must be a pointer that comes from a `Box<BindingHolder<B>>::into_raw()`
    unsafe fn binding_drop<B>(_self: *mut BindingHolder) {
        unsafe {
            drop(Box::from_raw(_self as *mut BindingHolder<B>));
        }
    }

    /// Safety: _self must be a pointer to a `BindingHolder<B>`
    /// and value must be a pointer to T
    unsafe fn evaluate<T, B: BindingCallable<T>>(
        _self: *const BindingHolder,
        value: *mut c_void,
    ) -> BindingResult {
        unsafe {
            Pin::new_unchecked(&((*(_self as *const BindingHolder<B>)).binding))
                .evaluate(&mut *(value as *mut T))
        }
    }

    /// Safety: _self must be a pointer to a `BindingHolder<B>`
    unsafe fn mark_dirty<T, B: BindingCallable<T>>(_self: *const BindingHolder, _: bool) {
        unsafe { Pin::new_unchecked(&((*(_self as *const BindingHolder<B>)).binding)).mark_dirty() }
    }

    /// Safety: _self must be a pointer to a `BindingHolder<B>`
    unsafe fn intercept_set<T, B: BindingCallable<T>>(
        _self: *const BindingHolder,
        value: *const c_void,
    ) -> bool {
        unsafe {
            Pin::new_unchecked(&((*(_self as *const BindingHolder<B>)).binding))
                .intercept_set(&*(value as *const T))
        }
    }

    unsafe fn intercept_set_binding<T, B: BindingCallable<T>>(
        _self: *const BindingHolder,
        new_binding: *mut BindingHolder,
    ) -> bool {
        unsafe {
            Pin::new_unchecked(&((*(_self as *const BindingHolder<B>)).binding))
                .intercept_set_binding(new_binding)
        }
    }

    trait HasBindingVTable<T> {
        const VT: &'static BindingVTable;
    }
    impl<T, B: BindingCallable<T>> HasBindingVTable<T> for B {
        const VT: &'static BindingVTable = &BindingVTable {
            drop: binding_drop::<B>,
            evaluate: evaluate::<T, B>,
            mark_dirty: mark_dirty::<T, B>,
            intercept_set: intercept_set::<T, B>,
            intercept_set_binding: intercept_set_binding::<T, B>,
        };
    }

    let holder: BindingHolder<B> = BindingHolder {
        dependencies: Cell::new(core::ptr::null_mut()),
        dep_nodes: Default::default(),
        vtable: <B as HasBindingVTable<T>>::VT,
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
    /// Either a pointer to a binding or the head of the dependent-properties list.
    /// The two least significant bits are flags (the pointer is always aligned).
    /// Bit 0 (`0b01`): the binding is borrowed.
    /// Bit 1 (`0b10`): the value is a pointer to a binding.
    handle: Cell<*mut ()>,
}

const BINDING_BORROWED: usize = 0b01;
const BINDING_POINTER_TO_BINDING: usize = 0b10;
const BINDING_POINTER_MASK: usize = !(BINDING_POINTER_TO_BINDING | BINDING_BORROWED);

impl core::fmt::Debug for PropertyHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let handle = self.handle.get();
        write!(
            f,
            "PropertyHandle {{ handle: 0x{:x}, locked: {}, binding: {} }}",
            handle.addr() & !0b11,
            self.lock_flag(),
            PropertyHandle::is_pointer_to_binding(handle)
        )
    }
}

impl PropertyHandle {
    /// The lock flag specifies that we can get a reference to the Cell or unsafe cell
    #[inline]
    fn lock_flag(&self) -> bool {
        self.handle.get().addr() & BINDING_BORROWED != 0
    }
    /// Sets the lock_flag.
    /// Safety: the lock flag must not be unset if there exist references to what's inside the cell
    unsafe fn set_lock_flag(&self, set: bool) {
        self.handle.set(if set {
            self.handle.get().map_addr(|a| a | BINDING_BORROWED)
        } else {
            self.handle.get().map_addr(|a| a & !BINDING_BORROWED)
        })
    }

    #[inline]
    fn is_pointer_to_binding(handle: *mut ()) -> bool {
        handle.addr() & BINDING_POINTER_TO_BINDING != 0
    }

    /// Get the pointer **without locking** if the handle points to a pointer otherwise None
    #[inline]
    fn pointer_to_binding(handle: *mut ()) -> Option<*mut BindingHolder> {
        if Self::is_pointer_to_binding(handle) {
            Some(handle.map_addr(|a| a & BINDING_POINTER_MASK) as *mut BindingHolder)
        } else {
            None
        }
    }

    /// The handle is not borrowed to any other binding
    /// and the handle does not point to another binding
    #[inline]
    fn has_no_binding_or_lock(handle: *mut ()) -> bool {
        handle.addr() & (BINDING_BORROWED | BINDING_POINTER_TO_BINDING) == 0
    }

    /// Access the value.
    /// Panics if the function try to recursively access the value
    fn access<R>(&self, f: impl FnOnce(Option<Pin<&mut BindingHolder>>) -> R) -> R {
        #[cfg(slint_debug_property)]
        if self.lock_flag() {
            unsafe {
                let handle = self.handle.get();
                if let Some(binding_pointer) = Self::pointer_to_binding(handle) {
                    let binding = &mut *(binding_pointer);
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
            let binding =
                Self::pointer_to_binding(handle).map(|pointer| Pin::new_unchecked(&mut *(pointer)));
            f(binding)
        }
    }

    /// Transfer the dependency list from the current binding back to the
    /// handle and return the now-detached binding pointer. The binding is
    /// **not** dropped; the caller is responsible for its lifetime.
    ///
    /// Returns `None` when the handle does not point to a binding.
    fn detach_binding(&self) -> Option<*mut BindingHolder> {
        let binding = Self::pointer_to_binding(self.handle.get())?;
        unsafe {
            let const_sentinel = const_sentinel();
            if (*binding).dependencies.get() == const_sentinel {
                self.handle.set(const_sentinel);
            } else {
                DependencyListHead::mem_move(
                    (*binding).dependencies.as_ptr() as *mut DependencyListHead,
                    self.handle.as_ptr() as *mut DependencyListHead,
                );
            }
            (*binding).dependencies.set(core::ptr::null_mut());
        }
        Some(binding)
    }

    fn remove_binding(&self) {
        assert!(!self.lock_flag(), "Recursion detected");

        if let Some(binding) = self.detach_binding() {
            unsafe {
                ((*binding).vtable.drop)(binding);
            }
        }
        debug_assert!(Self::has_no_binding_or_lock(self.handle.get()));
    }

    /// Safety: the BindingCallable must be valid for the type of this property
    unsafe fn set_binding<T, B: BindingCallable<T> + 'static>(
        &self,
        binding: B,
        #[cfg(slint_debug_property)] debug_name: &str,
    ) {
        let binding = alloc_binding_holder::<T, B>(binding);
        #[cfg(slint_debug_property)]
        unsafe {
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
        debug_assert!(Self::has_no_binding_or_lock(binding as *mut ()));
        debug_assert!(Self::has_no_binding_or_lock(self.handle.get()));
        let const_sentinel = const_sentinel();
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
        self.handle.set((binding as *mut ()).map_addr(|a| a | BINDING_POINTER_TO_BINDING));
        if !is_constant {
            self.mark_dirty(
                #[cfg(slint_debug_property)]
                "",
            );
        }
    }

    fn dependencies(&self) -> *mut DependencyListHead {
        assert!(!self.lock_flag(), "Recursion detected");
        if Self::is_pointer_to_binding(self.handle.get()) {
            self.access(|binding| binding.unwrap().dependencies.as_ptr() as *mut DependencyListHead)
        } else {
            self.handle.as_ptr() as *mut DependencyListHead
        }
    }

    // `value` is the content of the unsafe cell and will be only dereferenced if the
    // handle is not locked. (Upholding the requirements of UnsafeCell)
    unsafe fn update<T>(&self, value: *mut T) {
        let binding_ptr = Self::pointer_to_binding(self.handle.get());

        let remove = self.access(|binding| {
            if let Some(binding) = binding
                && binding.dirty.get()
            {
                // Safety: binding is Some so binding_ptr is too
                let binding_ptr = unsafe { binding_ptr.unwrap_unchecked() };

                // clear all the nodes so that we can start from scratch
                unsafe { *(*binding_ptr).dep_nodes.get() = Default::default() };
                let r = unsafe {
                    current_binding_storage::set(Some(binding_ptr), || {
                        ((*binding_ptr).vtable.evaluate)(binding_ptr, value as *mut c_void)
                    })
                };
                unsafe { (*binding_ptr).dirty.set(false) };
                if r == BindingResult::RemoveBinding {
                    return true;
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
        current_binding_storage::with(|cur_binding| {
            if let Some(cur_binding) = cur_binding {
                let dependencies = self.dependencies();
                if unsafe { *(dependencies as *mut *mut ()) } != const_sentinel() {
                    BindingHolder::register_self_as_dependency(
                        cur_binding,
                        dependencies,
                        #[cfg(slint_debug_property)]
                        debug_name,
                    );
                }
            }
        });
    }

    fn mark_dirty(&self, #[cfg(slint_debug_property)] debug_name: &str) {
        #[cfg(not(slint_debug_property))]
        let debug_name = "";
        unsafe {
            let dependencies = self.dependencies();
            assert!(
                *(dependencies as *mut *mut ()) != const_sentinel(),
                "Constant property being changed {debug_name}"
            );
            mark_dependencies_dirty(dependencies)
        };
    }

    fn set_constant(&self) {
        unsafe {
            let dependencies = self.dependencies();
            let const_sentinel = const_sentinel();
            if *(dependencies as *mut *mut ()) != const_sentinel {
                DependencyListHead::drop(dependencies);
                *(dependencies as *mut *mut ()) = const_sentinel;
            }
        }
    }

    fn is_constant(&self) -> bool {
        let dependencies = self.dependencies();
        // Safety: dependencies is a valid pointer to a DependencyListHead (Cell<*mut ()> internally)
        unsafe { *(dependencies as *mut *mut ()) == const_sentinel() }
    }
}

impl Drop for PropertyHandle {
    fn drop(&mut self) {
        self.remove_binding();
        debug_assert!(Self::has_no_binding_or_lock(self.handle.get()));
        if self.handle.get() != const_sentinel() {
            unsafe {
                DependencyListHead::drop(self.handle.as_ptr() as *mut _);
            }
        }
    }
}

/// Safety: the dependency list must be valid and consistent
unsafe fn mark_dependencies_dirty(dependencies: *mut DependencyListHead) {
    unsafe {
        debug_assert!(*(dependencies as *mut *mut ()) != const_sentinel());
        DependencyListHead::for_each(&*dependencies, |binding| {
            let binding: &BindingHolder = &**binding;
            let was_dirty = binding.dirty.replace(true);
            (binding.vtable.mark_dirty)(binding as *const BindingHolder, was_dirty);

            assert!(
                binding.dependencies.get() != const_sentinel(),
                "Const property marked as dirty"
            );

            if !was_dirty {
                mark_dependencies_dirty(binding.dependencies.as_ptr() as *mut DependencyListHead)
            }
        });
    }
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

/// A Property that allows a binding that tracks changes
///
/// Property can have an assigned value, or a binding.
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

    /// Register this property as a dependency of the current tracking scope
    /// without evaluating any binding.
    /// Use this when you only need the tracking scope to be notified on
    /// future changes, not the current value.
    ///
    /// Unlike [`Self::get`], this doesn't evaluate a dirty binding,
    /// so the caller won't be notified about a pending evaluation that
    /// hasn't run yet.
    /// Only use this when the property has no binding or when its binding
    /// is known to be already evaluated.
    pub fn register_as_dependency(self: Pin<&Self>) {
        let handle = unsafe { Pin::new_unchecked(&self.handle) };
        handle.register_as_dependency_to_current_binding(
            #[cfg(slint_debug_property)]
            self.debug_name.borrow().as_str(),
        );
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
                (b.vtable.intercept_set)(
                    &*b as *const BindingHolder,
                    (&t as *const T).cast::<c_void>(),
                )
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
                move |val: &mut T| {
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

    /// Returns true if the property has currently a binding (like an animation, ...), otherwise false
    pub fn has_binding(&self) -> bool {
        PropertyHandle::pointer_to_binding(self.handle.handle.get()).is_some()
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

    /// Returns true if set_constant was called on this property
    pub fn is_constant(&self) -> bool {
        self.handle.is_constant()
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

mod change_tracker;
mod two_way_binding;
pub use change_tracker::*;
mod properties_animations;
pub use properties_animations::*;

/// Value of the state property
/// A state is just the current state, but also has information about the previous state and the moment it changed
#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[repr(C)]
pub struct StateInfo {
    /// The current state value
    pub current_state: i32,
    /// The previous state
    pub previous_state: i32,
    /// The instant in which the state changed last
    pub change_time: crate::animations::Instant,
}

struct StateInfoBinding<F, T> {
    dirty_time: Cell<Option<crate::animations::Instant>>,
    binding: F,
    _phantom: core::marker::PhantomData<fn() -> T>,
}

unsafe impl<F: Fn() -> i32, T> crate::properties::BindingCallable<T> for StateInfoBinding<F, T>
where
    T: Default + From<StateInfo> + 'static,
    StateInfo: TryFrom<T>,
{
    fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
        let new_state = (self.binding)();
        let timestamp = self.dirty_time.take();
        // The conversion only fails on the property's initial value
        // (`Value::Void` in the interpreter); start from the default then.
        let mut state_info: StateInfo = core::mem::take(value).try_into().unwrap_or_default();
        if new_state != state_info.current_state {
            state_info.previous_state = state_info.current_state;
            state_info.change_time = timestamp.unwrap_or_else(crate::animations::current_tick);
            state_info.current_state = new_state;
        }
        *value = T::from(state_info);
        BindingResult::KeepBinding
    }

    fn mark_dirty(self: Pin<&Self>) {
        if self.dirty_time.get().is_none() {
            self.dirty_time.set(Some(crate::animations::current_tick()))
        }
    }
}

/// Sets a binding that returns a state index to a property that stores
/// state-tracking information. The property type `T` must be convertible
/// to/from [`StateInfo`] — the generated code uses `Property<StateInfo>`
/// directly, while the interpreter uses `Property<Value>` with the
/// conversion going through `Value::Struct`.
pub fn set_state_binding<T>(property: Pin<&Property<T>>, binding: impl Fn() -> i32 + 'static)
where
    T: Default + From<StateInfo> + 'static,
    StateInfo: TryFrom<T>,
{
    let bind_callable = StateInfoBinding {
        dirty_time: Cell::new(None),
        binding,
        _phantom: core::marker::PhantomData,
    };
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

/// A PropertyTracker tracks which properties are accessed during evaluation,
/// and can notify when those properties change.
///
/// The `NEEDS_SET_DIRTY` const parameter controls whether this tracker
/// supports being dirtied externally via [`PropertyTracker::set_dirty`].
/// When `false` (the default), the tracker can be more efficient: it will
/// skip registering itself as a dependency of outer bindings if it has no
/// tracked dependencies of its own, since there is no external way to dirty it.
pub struct PropertyTracker<const NEEDS_SET_DIRTY: bool = false, DirtyHandler = ()> {
    holder: BindingHolder<DirtyHandler>,
}

impl<const NEEDS_SET_DIRTY: bool> Default for PropertyTracker<NEEDS_SET_DIRTY, ()> {
    fn default() -> Self {
        static VT: &BindingVTable = &BindingVTable {
            drop: |_| (),
            evaluate: |_, _| BindingResult::KeepBinding,
            mark_dirty: |_, _| (),
            intercept_set: |_, _| false,
            intercept_set_binding: |_, _| false,
        };

        let holder = BindingHolder {
            dependencies: Cell::new(core::ptr::null_mut()),
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

impl<const NEEDS_SET_DIRTY: bool, DirtyHandler> Drop
    for PropertyTracker<NEEDS_SET_DIRTY, DirtyHandler>
{
    fn drop(&mut self) {
        unsafe {
            DependencyListHead::drop(self.holder.dependencies.as_ptr() as *mut DependencyListHead);
        }
    }
}

impl<const NEEDS_SET_DIRTY: bool, DirtyHandler: PropertyDirtyHandler>
    PropertyTracker<NEEDS_SET_DIRTY, DirtyHandler>
{
    #[cfg(slint_debug_property)]
    /// set the debug name when `cfg(slint_debug_property`
    pub fn set_debug_name(&mut self, debug_name: alloc::string::String) {
        self.holder.debug_name = debug_name;
    }

    /// Registers this property tracker as a dependency to the current binding being evaluated.
    pub fn register_as_dependency_to_current_binding(self: Pin<&Self>) {
        // Safety: only reading dep_nodes, not moving it
        if !NEEDS_SET_DIRTY && unsafe { (*self.holder.dep_nodes.get()).is_empty() } {
            return;
        }
        current_binding_storage::with(|cur_binding| {
            if let Some(cur_binding) = cur_binding {
                debug_assert!(self.holder.dependencies.get() != const_sentinel());
                BindingHolder::register_self_as_dependency(
                    cur_binding,
                    self.holder.dependencies.as_ptr() as *mut DependencyListHead,
                    #[cfg(slint_debug_property)]
                    &self.holder.debug_name,
                );
            }
        });
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
        let r = self.evaluate_as_dependency_root(f);
        self.register_as_dependency_to_current_binding();
        r
    }

    /// Evaluate the function, and record dependencies of properties accessed within this function.
    /// If this is called during the evaluation of another property binding or property tracker, then
    /// any changes to accessed properties will not propagate to the other tracker.
    pub fn evaluate_as_dependency_root<R>(self: Pin<&Self>, f: impl FnOnce() -> R) -> R {
        // clear all the nodes so that we can start from scratch
        unsafe { *self.holder.dep_nodes.get() = Default::default() };

        let holder_ptr = &raw const self.holder as *const BindingHolder;
        let r = current_binding_storage::set(Some(holder_ptr), f);
        self.holder.dirty.set(false);
        r
    }

    /// Call [`Self::evaluate`] if and only if it is dirty.
    /// But register a dependency in any case.
    pub fn evaluate_if_dirty<R>(self: Pin<&Self>, f: impl FnOnce() -> R) -> Option<R> {
        let r = self.is_dirty().then(|| self.evaluate_as_dependency_root(f));
        self.register_as_dependency_to_current_binding();
        r
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
                unsafe {
                    Pin::new_unchecked(&(*(_self as *const BindingHolder<B>)).binding).notify()
                };
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
            dependencies: Cell::new(core::ptr::null_mut()),
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

impl<DirtyHandler> PropertyTracker<true, DirtyHandler> {
    /// Mark this PropertyTracker as dirty
    pub fn set_dirty(&self) {
        self.holder.dirty.set(true);
        unsafe { mark_dependencies_dirty(self.holder.dependencies.as_ptr() as *mut _) };
    }
}

#[test]
fn test_property_handler_binding() {
    use core::ptr::without_provenance_mut;
    assert_eq!(
        PropertyHandle::has_no_binding_or_lock(without_provenance_mut(BINDING_BORROWED)),
        false
    );
    assert_eq!(
        PropertyHandle::has_no_binding_or_lock(without_provenance_mut(BINDING_POINTER_TO_BINDING)),
        false
    );
    assert_eq!(
        PropertyHandle::has_no_binding_or_lock(without_provenance_mut(
            BINDING_BORROWED | BINDING_POINTER_TO_BINDING
        )),
        false
    );
    assert_eq!(PropertyHandle::has_no_binding_or_lock(core::ptr::null_mut()), true);
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
    let tracker1 = Box::pin(<PropertyTracker>::default());
    let tracker2 = Box::pin(<PropertyTracker>::default());
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
    let call_flag = std::rc::Rc::new(Cell::new(false));
    let tracker = Box::pin(PropertyTracker::<false, _>::new_with_dirty_handler({
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
    let outer_tracker = Box::pin(<PropertyTracker>::default());
    let inner_tracker = Box::pin(<PropertyTracker>::default());
    let prop = Box::pin(Property::new(42));

    let r =
        outer_tracker.as_ref().evaluate(|| inner_tracker.as_ref().evaluate(|| prop.as_ref().get()));
    assert_eq!(r, 42);

    drop(inner_tracker);
    prop.as_ref().set(200); // don't crash
}

#[test]
fn test_nested_property_tracker_dirty() {
    let outer_tracker = Box::pin(PropertyTracker::<true, ()>::default());
    let inner_tracker = Box::pin(PropertyTracker::<true, ()>::default());
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
    let outer_tracker = Box::pin(<PropertyTracker>::default());
    let inner_tracker = Box::pin(<PropertyTracker>::default());
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
