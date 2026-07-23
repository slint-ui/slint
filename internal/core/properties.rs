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

pub(crate) mod dependency_tracker {
    //! Intrusive dependency-tracking lists backed by a per-thread slab.
    //!
    //! Each tracked relationship is a *node* living in a thread-local slab (a
    //! chunked arena). Nodes are referenced by 32-bit [`Key`]s rather than raw
    //! pointers, which keeps the per-node links small. The slab never moves or
    //! frees a slot's backing storage until thread exit, so a slot's address is
    //! stable for its whole life; only the keys are used for linkage.
    //!
    //! A node participates in two lists at once:
    //!  * the *dependents ring* of the property/tracker it depends on
    //!    ([`DependencyListHead`]) — a circular doubly-linked list anchored by a
    //!    lazily-allocated sentinel slot; and
    //!  * its *owner chain* ([`OwnerChain`]) — the list of all the nodes a single
    //!    binding registered — so re-evaluating or dropping the binding frees them
    //!    all at once.
    //!
    //! Recycling slots through the slab's free list avoids the malloc/free churn
    //! of rebuilding these lists on every property re-evaluation.
    //!
    //! This is unsafe to use for various reasons, so it is kept internal.

    use alloc::boxed::Box;
    use alloc::vec::Vec;
    use core::cell::Cell;
    use core::marker::PhantomData;
    use core::mem::MaybeUninit;
    use core::num::NonZeroU32;
    use core::pin::Pin;

    /// A 32-bit handle to a slab slot; `None` is the NIL handle.
    type Key = Option<NonZeroU32>;

    /// Number of slots per arena chunk (a power of two).
    const CHUNK_BITS: u32 = 9;
    const CHUNK_SIZE: usize = 1 << CHUNK_BITS;

    /// One slot in the slab.
    pub struct Slot<T> {
        /// Next node in the dependents ring (circular, through the sentinel).
        next: Cell<Key>,
        /// Previous node in the dependents ring.
        prev: Cell<Key>,
        /// Next node in the owner chain. Doubles as the free-list link for free
        /// slots, and stores the sentinel's own key for sentinel slots.
        owner_next: Cell<Key>,
        /// The tracked value. Left uninitialized for sentinel and free slots.
        payload: Cell<MaybeUninit<T>>,
    }

    impl<T> Slot<T> {
        const fn blank() -> Self {
            Self {
                next: Cell::new(None),
                prev: Cell::new(None),
                owner_next: Cell::new(None),
                payload: Cell::new(MaybeUninit::uninit()),
            }
        }
    }

    /// A chunked arena of [`Slot`]s with an embedded free list.
    ///
    /// Chunks are boxed and only ever appended, so a slot's address is stable for
    /// the lifetime of the slab; only [`Key`]s are used to link slots together.
    pub struct Slab<T> {
        chunks: Vec<Box<[Slot<T>; CHUNK_SIZE]>>,
        /// Head of the free list (chained via `owner_next`).
        free: Key,
        /// Number of slots ever created (index of the next fresh slot).
        len: u32,
    }

    impl<T> Slab<T> {
        pub const fn new() -> Self {
            Self { chunks: Vec::new(), free: None, len: 0 }
        }

        #[inline]
        fn slot(&self, key: NonZeroU32) -> &Slot<T> {
            let index = (key.get() - 1) as usize;
            let chunk_index = index >> CHUNK_BITS;
            let slot_index = index & (CHUNK_SIZE - 1);
            debug_assert!(chunk_index < self.chunks.len());
            // SAFETY: keys are only minted by `alloc`, so `chunk_index` is always in range
            // and `slot_index` is masked < CHUNK_SIZE.
            unsafe { self.chunks.get_unchecked(chunk_index).get_unchecked(slot_index) }
        }

        /// Allocate a slot with all links NIL and return its key.
        fn alloc(&mut self) -> NonZeroU32 {
            if let Some(key) = self.free {
                let slot = self.slot(key);
                let next_free = slot.owner_next.get();
                slot.next.set(None);
                slot.prev.set(None);
                slot.owner_next.set(None);
                self.free = next_free;
                return key;
            }
            let index = self.len;
            if (index as usize) >> CHUNK_BITS >= self.chunks.len() {
                // Manually initialize the slice as that produces much smaller and faster code in
                // debug mode compared to using a Vec.
                let mut chunk = Box::<[Slot<T>; CHUNK_SIZE]>::new_uninit();
                let p = chunk.as_mut_ptr() as *mut Slot<T>;
                // SAFETY: `p` is the start of an allocation for CHUNK_SIZE slots; every index in
                // 0..CHUNK_SIZE is written exactly once, so the array is fully initialized below.
                for i in 0..CHUNK_SIZE {
                    unsafe { p.add(i).write(Slot::blank()) };
                }
                let chunk = unsafe { chunk.assume_init() };
                self.chunks.push(chunk);
            }
            // `+ 1` so the key is never zero (enabling the niche of `Option<NonZeroU32>`).
            let key = index.checked_add(1).expect("dependency slab exhausted");
            self.len = key;
            NonZeroU32::new(key).unwrap()
        }

        /// Return a slot to the free list. It must already be unlinked from any
        /// dependents ring.
        fn free(&mut self, key: NonZeroU32) {
            let old_free = self.free;
            let slot = self.slot(key);
            slot.next.set(None);
            slot.prev.set(None);
            slot.owner_next.set(old_free);
            // No need to clear `payload`: `T: Copy` has no destructor, and a freed
            // slot's payload is never read before the next `alloc` overwrites it.
            self.free = Some(key);
        }
    }

    /// Unlink a node from whatever dependents ring (or detached chain) it is in,
    /// leaving its slot allocated with NIL links. A no-op on an already unlinked
    /// node. Does not free the slot.
    #[inline]
    fn unlink<T>(slab: &Slab<T>, node: NonZeroU32) {
        let n = slab.slot(node);
        let prev = n.prev.get();
        let next = n.next.get();
        if let Some(p) = prev {
            slab.slot(p).next.set(next);
        }
        if let Some(nx) = next {
            slab.slot(nx).prev.set(prev);
        }
        n.next.set(None);
        n.prev.set(None);
    }

    /// Implemented for the concrete payload types that can be tracked. Provides
    /// access to a per-thread slab dedicated to that type.
    ///
    /// A `thread_local!` cannot be generic, and a `static` inside a generic fn is
    /// shared across monomorphizations, so each concrete type wires up its own
    /// slab through this trait — see the impls in `properties.rs` and `model_peer.rs`.
    pub trait SlabbedDep: Copy + 'static {
        /// Access the per-thread slab. Returns `None` only if the slab's
        /// thread-local storage has already been destroyed, which can happen while
        /// a thread is tearing down and a still-live tracker is dropped after the
        /// slab. In that case the whole slab is being reclaimed anyway, so the
        /// `Drop` paths treat `None` as "nothing to do".
        fn try_with_slab<R>(f: impl FnOnce(&mut Slab<Self>) -> R) -> Option<R>;

        /// Access the per-thread slab, panicking if it is unavailable. Used by all
        /// the non-`Drop` operations, which can only run while the slab is alive.
        fn with_slab<R>(f: impl FnOnce(&mut Slab<Self>) -> R) -> R {
            Self::try_with_slab(f).expect("dependency slab accessed after thread-local destruction")
        }
    }

    /// Head of a circular doubly-linked "dependents" list.
    ///
    /// An empty list stores a null pointer; a non-empty list lazily allocates a
    /// sentinel slot and stores a (stable) pointer to it. Because the nodes
    /// reference the sentinel by key and the sentinel never moves, relocating the
    /// head ([`Self::mem_move`]/[`Self::swap`]) is a plain pointer copy with no
    /// fix-up — which is why this is `#[repr(transparent)]` over a single pointer
    /// and can share storage with the tagged `PropertyHandle`.
    #[repr(transparent)]
    pub struct DependencyListHead<T: SlabbedDep>(Cell<*const Slot<T>>);

    impl<T: SlabbedDep> Default for DependencyListHead<T> {
        fn default() -> Self {
            Self(Cell::new(core::ptr::null()))
        }
    }

    impl<T: SlabbedDep> Drop for DependencyListHead<T> {
        fn drop(&mut self) {
            unsafe { DependencyListHead::drop(self as *mut Self) };
        }
    }

    impl<T: SlabbedDep> DependencyListHead<T> {
        /// Move the list head from `from` to `to`. Since the head only stores a
        /// pointer to the (immovable) sentinel, this is a plain pointer copy.
        ///
        /// Safety: both pointers must be valid and point to a `DependencyListHead`.
        pub unsafe fn mem_move(from: *mut Self, to: *mut Self) {
            unsafe {
                (*to).0.set((*from).0.get());
                (*from).0.set(core::ptr::null());
            }
        }

        /// Swap two list heads.
        pub fn swap(from: Pin<&Self>, to: Pin<&Self>) {
            Cell::swap(&from.0, &to.0);
        }

        /// The sentinel's own key, recovered from the stable sentinel pointer.
        ///
        /// Safety: `sentinel` must point to a live sentinel slot.
        unsafe fn sentinel_key(sentinel: *const Slot<T>) -> NonZeroU32 {
            unsafe { (*sentinel).owner_next.get().unwrap() }
        }

        /// Return true if the list has no nodes.
        pub fn is_empty(&self) -> bool {
            let s = self.0.get();
            if s.is_null() {
                return true;
            }
            // An empty ring has `sentinel.next == sentinel` (its own key, kept in
            // `owner_next`).
            unsafe { (*s).next.get() == (*s).owner_next.get() }
        }

        /// Drop the list head: detach the surviving nodes (which are owned
        /// elsewhere) into a NIL-terminated chain and free the sentinel.
        ///
        /// Safety: `_self` must point to a valid `DependencyListHead`.
        pub unsafe fn drop(_self: *mut Self) {
            let s = unsafe { (*_self).0.get() };
            unsafe { (*_self).0.set(core::ptr::null()) };
            if s.is_null() {
                return;
            }
            // All accesses to the (possibly already-reclaimed) sentinel slot happen
            // inside the closure, which does not run if the slab is gone.
            let _ = T::try_with_slab(|slab| {
                let sentinel_key = unsafe { Self::sentinel_key(s) };
                let first = slab.slot(sentinel_key).next.get();
                let last = slab.slot(sentinel_key).prev.get();
                if first != Some(sentinel_key) {
                    // Real nodes remain: turn the ring into a NIL-terminated chain
                    // so they can later be unlinked safely by their owners.
                    slab.slot(first.unwrap()).prev.set(None);
                    slab.slot(last.unwrap()).next.set(None);
                }
                slab.free(sentinel_key);
            });
        }

        /// Append a node (identified by its slab key) to this list, first
        /// unlinking it from any list it is currently in.
        pub fn append(&self, node: NonZeroU32) {
            T::with_slab(|slab| {
                unlink(slab, node);
                // Lazily create the sentinel on first use.
                let sentinel_key = {
                    let s = self.0.get();
                    if s.is_null() {
                        let key = slab.alloc();
                        let sentinel = slab.slot(key);
                        sentinel.next.set(Some(key));
                        sentinel.prev.set(Some(key));
                        sentinel.owner_next.set(Some(key));
                        self.0.set(slab.slot(key) as *const Slot<T>);
                        key
                    } else {
                        unsafe { Self::sentinel_key(s) }
                    }
                };
                // Insert `node` right after the sentinel.
                let first = slab.slot(sentinel_key).next.get();
                {
                    let n = slab.slot(node);
                    n.prev.set(Some(sentinel_key));
                    n.next.set(first);
                }
                slab.slot(sentinel_key).next.set(Some(node));
                slab.slot(first.unwrap()).prev.set(Some(node));
            });
        }

        /// Call `f` with the payload of each node in the list.
        pub fn for_each(&self, mut f: impl FnMut(&T)) {
            let slot = self.0.get();
            if slot.is_null() {
                return;
            }
            let sentinel_key = unsafe { Self::sentinel_key(slot) };
            let mut cur = unsafe { (*slot).next.get() };
            while cur != Some(sentinel_key) {
                let node = cur.unwrap();
                // Read the next link and a copy of the payload under a short slab
                // borrow, then run the callback with no borrow held — it may
                // re-enter the slab (mark dependencies dirty, remove this node…).
                let (next, payload) = T::with_slab(|slab| {
                    let n = slab.slot(node);
                    (n.next.get(), unsafe { n.payload.get().assume_init() })
                });
                f(&payload);
                cur = next;
            }
        }

        /// Remove and return the payload of the first node, if any. Only unlinks
        /// the node from this list; the slot stays allocated (owned elsewhere).
        pub fn take_head(&self) -> Option<T> {
            let slot = self.0.get();
            if slot.is_null() {
                return None;
            }
            let sentinel_key = unsafe { Self::sentinel_key(slot) };
            let first = unsafe { (*slot).next.get() };
            if first == Some(sentinel_key) {
                return None;
            }
            let node = first.unwrap();
            Some(T::with_slab(|slab| {
                let payload = unsafe { slab.slot(node).payload.get().assume_init() };
                unlink(slab, node);
                payload
            }))
        }
    }

    /// The set of nodes a single binding registered, chained through each slot's
    /// `owner_next`. Dropping the chain unlinks each node from its dependents ring
    /// and frees its slot — this is what reclaims dependency nodes on re-evaluation.
    pub struct OwnerChain<T: SlabbedDep> {
        head: Cell<Key>,
        _t: PhantomData<T>,
    }

    impl<T: SlabbedDep> Default for OwnerChain<T> {
        fn default() -> Self {
            Self { head: Cell::new(None), _t: PhantomData }
        }
    }

    impl<T: SlabbedDep> OwnerChain<T> {
        /// Allocate a node carrying `payload`, prepend it to the chain, and return
        /// its key so it can be appended to a dependents list.
        pub fn push_front(&self, payload: T) -> NonZeroU32 {
            T::with_slab(|slab| {
                let key = slab.alloc();
                let slot = slab.slot(key);
                slot.payload.set(MaybeUninit::new(payload));
                slot.owner_next.set(self.head.get());
                self.head.set(Some(key));
                key
            })
        }

        pub fn is_empty(&self) -> bool {
            self.head.get().is_none()
        }

        /// The most recently added node, if any.
        pub fn first(&self) -> Option<NonZeroU32> {
            self.head.get()
        }

        /// The number of nodes in this owner chain.
        #[cfg(test)]
        #[allow(dead_code)]
        pub fn count(&self) -> usize {
            T::with_slab(|slab| {
                let mut n = 0;
                let mut cur = self.head.get();
                while let Some(key) = cur {
                    n += 1;
                    cur = slab.slot(key).owner_next.get();
                }
                n
            })
        }
    }

    impl<T: SlabbedDep> Drop for OwnerChain<T> {
        fn drop(&mut self) {
            let _ = T::try_with_slab(|slab| {
                let mut cur = self.head.get();
                while let Some(key) = cur {
                    let next = slab.slot(key).owner_next.get();
                    unlink(slab, key);
                    slab.free(key);
                    cur = next;
                }
            });
            self.head.set(None);
        }
    }

    /// Allocate a standalone node carrying `payload` (owned by the caller directly,
    /// not by an owner chain). Used for model peers, which own exactly one node.
    pub fn alloc_node<T: SlabbedDep>(payload: T) -> NonZeroU32 {
        T::with_slab(|slab| {
            let key = slab.alloc();
            slab.slot(key).payload.set(MaybeUninit::new(payload));
            key
        })
    }

    /// Unlink a standalone node from its dependents ring and free its slot.
    /// Called from `Drop`, so it tolerates the slab already being torn down.
    pub fn free_node<T: SlabbedDep>(node: NonZeroU32) {
        let _ = T::try_with_slab(|slab| {
            unlink(slab, node);
            slab.free(node);
        });
    }
}

type DependencyListHead = dependency_tracker::DependencyListHead<*const BindingHolder>;
type OwnerChain = dependency_tracker::OwnerChain<*const BindingHolder>;

/// Wires up the per-thread slab that stores the dependency nodes whose payload is
/// a `*const BindingHolder` (all property and tracker dependents share it).
impl dependency_tracker::SlabbedDep for *const BindingHolder {
    #[inline]
    fn try_with_slab<R>(f: impl FnOnce(&mut dependency_tracker::Slab<Self>) -> R) -> Option<R> {
        crate::thread_local!(static SLAB: UnsafeCell<dependency_tracker::Slab<*const BindingHolder>>
            = const { UnsafeCell::new(dependency_tracker::Slab::new()) });
        // SAFETY: no `with_slab` closure re-enters `with_slab` (`for_each` releases its
        // slab borrow before running the callback), and the `&mut` never escapes `f`
        // (every closure returns Copy/owned data), so this reference is never aliased.
        SLAB.try_with(|s| f(unsafe { &mut *s.get() })).ok()
    }
}

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
    /// Owner chain of the slab nodes that link this binding into the dependency
    /// lists of the properties it reads (freed together on re-evaluation/drop).
    /// UnsafeCell allows replacing the whole chain (`= Default::default()`).
    dep_nodes: UnsafeCell<OwnerChain>,
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
        // Safety: self_ptr is valid and pinned
        unsafe {
            let dep_nodes = &*(*self_ptr).dep_nodes.get();
            let node = dep_nodes.push_front(self_ptr);
            (*property_that_will_notify).append(node);
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

struct StateInfoBinding<F> {
    dirty_time: Cell<Option<crate::animations::Instant>>,
    binding: F,
}

unsafe impl<F: Fn() -> i32> crate::properties::BindingCallable<StateInfo> for StateInfoBinding<F> {
    fn evaluate(self: Pin<&Self>, value: &mut StateInfo) -> BindingResult {
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
