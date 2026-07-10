// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! module for the SharedVector and related things
#![allow(unsafe_code)]
#![warn(missing_docs)]
use core::fmt::Debug;
use core::mem::MaybeUninit;
use core::ops::Deref;
use core::ptr::NonNull;

use portable_atomic as atomic;

#[repr(C)]
struct SharedVectorHeader {
    refcount: atomic::AtomicIsize,
    size: usize,
    capacity: usize,
}

#[repr(C)]
struct SharedVectorInner<T> {
    header: SharedVectorHeader,
    data: MaybeUninit<T>,
}

fn compute_inner_layout<T>(capacity: usize) -> core::alloc::Layout {
    core::alloc::Layout::new::<SharedVectorHeader>()
        .extend(core::alloc::Layout::array::<T>(capacity).unwrap())
        .unwrap()
        .0
}

/// Returns a raw pointer to the data with full allocation provenance.
///
/// Must not go through `&SharedVectorInner<T>` or `&mut SharedVectorInner<T>` because
/// the declared struct size is smaller than the actual allocation.
fn data_ptr<T>(inner: NonNull<SharedVectorInner<T>>) -> *mut T {
    // Safety: inner.as_ptr() is a valid raw pointer; &raw mut avoids creating a reference.
    unsafe { &raw mut (*inner.as_ptr()).data as *mut T }
}

/// # Safety
/// Caller must ensure refcount is 0 and no other references to `inner` exist.
unsafe fn drop_inner<T>(inner: NonNull<SharedVectorInner<T>>) {
    unsafe {
        debug_assert_eq!((*inner.as_ptr()).header.refcount.load(atomic::Ordering::Relaxed), 0);
        let data = data_ptr(inner);
        let size = (*inner.as_ptr()).header.size;
        for x in 0..size {
            core::ptr::drop_in_place(data.add(x));
        }
        alloc::alloc::dealloc(
            inner.as_ptr() as *mut u8,
            compute_inner_layout::<T>((*inner.as_ptr()).header.capacity),
        )
    }
}

/// Allocate the memory for the SharedVector with the given capacity. Return the inner with size and refcount set to 1
fn alloc_with_capacity<T>(capacity: usize) -> NonNull<SharedVectorInner<T>> {
    let ptr = unsafe { ::alloc::alloc::alloc(compute_inner_layout::<T>(capacity)) };
    assert!(!ptr.is_null(), "allocation of {capacity:?} bytes failed");
    unsafe {
        core::ptr::write(
            ptr as *mut SharedVectorHeader,
            SharedVectorHeader { refcount: 1.into(), size: 0, capacity },
        );
    }
    NonNull::new(ptr).unwrap().cast()
}

/// Return a new capacity suitable for this vector
/// Loosely based on alloc::raw_vec::RawVec::grow_amortized.
fn capacity_for_grow(current_cap: usize, required_cap: usize, elem_size: usize) -> usize {
    if current_cap >= required_cap {
        return current_cap;
    }
    let cap = core::cmp::max(current_cap * 2, required_cap);
    let min_non_zero_cap = if elem_size == 1 {
        8
    } else if elem_size <= 1024 {
        4
    } else {
        1
    };
    core::cmp::max(min_non_zero_cap, cap)
}

#[repr(C)]
/// SharedVector holds a reference-counted read-only copy of `[T]`.
pub struct SharedVector<T> {
    inner: NonNull<SharedVectorInner<T>>,
}

// Safety: We use atomic reference counting, and if T is Send and Sync, we can send the vector to another thread
unsafe impl<T: Send + Sync> Send for SharedVector<T> {}
// Safety: We use atomic reference counting, and if T is Send and Sync, we can access the vector from multiple threads
unsafe impl<T: Send + Sync> Sync for SharedVector<T> {}

impl<T> Drop for SharedVector<T> {
    fn drop(&mut self) {
        // Safety: inner is always a valid pointer (either a real allocation or SHARED_NULL).
        unsafe {
            let header = &raw const (*self.inner.as_ptr()).header;
            if (*header).refcount.load(atomic::Ordering::Relaxed) < 0 {
                return;
            }
            if (*header).refcount.fetch_sub(1, atomic::Ordering::SeqCst) == 1 {
                drop_inner(self.inner)
            }
        }
    }
}

impl<T> Clone for SharedVector<T> {
    fn clone(&self) -> Self {
        // Safety: inner is always a valid pointer (either a real allocation or SHARED_NULL).
        unsafe {
            let header = &raw const (*self.inner.as_ptr()).header;
            if (*header).refcount.load(atomic::Ordering::Relaxed) > 0 {
                (*header).refcount.fetch_add(1, atomic::Ordering::SeqCst);
            }
            SharedVector { inner: self.inner }
        }
    }
}

impl<T> SharedVector<T> {
    /// Create a new empty array with a pre-allocated capacity in number of items
    pub fn with_capacity(capacity: usize) -> Self {
        Self { inner: alloc_with_capacity(capacity) }
    }

    fn as_ptr(&self) -> *const T {
        data_ptr(self.inner)
    }

    /// Number of elements in the array
    pub fn len(&self) -> usize {
        // Safety: header is always fully allocated (even for SHARED_NULL).
        unsafe { (*self.inner.as_ptr()).header.size }
    }

    /// Return true if the SharedVector is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return a slice to the array
    pub fn as_slice(&self) -> &[T] {
        if self.is_empty() {
            &[]
        } else {
            // Safety: len > 0 ensures data_ptr is valid for len elements.
            unsafe { core::slice::from_raw_parts(self.as_ptr(), self.len()) }
        }
    }

    /// Returns the number of elements the vector can hold without reallocating, when not shared
    fn capacity(&self) -> usize {
        // Safety: header is always fully allocated (even for SHARED_NULL).
        unsafe { (*self.inner.as_ptr()).header.capacity }
    }
}

impl<T: Clone + PartialEq> SharedVector<T> {
    /// Replaces `from` by `to` in `self` `count` times
    /// `count` - number of times to do the replacements
    pub(crate) fn replace_range(&mut self, from: &[T], to: &[T], mut count: usize) {
        if from.is_empty() || count == 0 || from.len() != to.len() {
            return;
        }
        let s = self.make_mut_slice();
        if s.len() < from.len() {
            return;
        }

        let mut index = 0;
        let from_len = from.len();
        let max_start = s.len() - from_len;

        while index <= max_start && count > 0 {
            if s[index..index + from_len] == *from {
                for (dst, src) in s[index..index + from_len].iter_mut().zip(to.iter()) {
                    *dst = src.clone();
                }
                count -= 1;
                index += from_len;
            } else {
                index += 1;
            }
        }
    }
}

impl<T: Clone> SharedVector<T> {
    /// Create a SharedVector from a slice
    pub fn from_slice(slice: &[T]) -> SharedVector<T> {
        Self::from(slice)
    }

    /// Ensure that the reference count is 1 so the array can be changed.
    /// If that's not the case, the array will be cloned
    fn detach(&mut self, new_capacity: usize) {
        // Acquire: if refcount == 1, synchronize with the Release in the last Drop to
        // ensure prior writes to size/data are visible before we mutate.
        let is_shared =
            unsafe { (*self.inner.as_ptr()).header.refcount.load(atomic::Ordering::Acquire) } != 1;
        if !is_shared && new_capacity <= self.capacity() {
            return;
        }
        let mut new_array = SharedVector::with_capacity(new_capacity);
        core::mem::swap(&mut self.inner, &mut new_array.inner);
        let mut size = 0;
        for x in new_array.into_iter() {
            assert_ne!(size, new_capacity);
            unsafe {
                core::ptr::write(data_ptr(self.inner).add(size), x);
                size += 1;
                (*self.inner.as_ptr()).header.size = size;
            }
            if size == new_capacity {
                break;
            }
        }
    }

    /// Return a mutable slice to the array. If the array was shared, this will make a copy of the array.
    pub fn make_mut_slice(&mut self) -> &mut [T] {
        self.detach(self.len());
        unsafe { core::slice::from_raw_parts_mut(self.as_ptr() as *mut T, self.len()) }
    }

    /// Add an element to the array. If the array was shared, this will make a copy of the array.
    pub fn push(&mut self, value: T) {
        self.detach(capacity_for_grow(self.capacity(), self.len() + 1, core::mem::size_of::<T>()));
        // Safety: detach ensures exclusive ownership and sufficient capacity.
        unsafe {
            let size = (*self.inner.as_ptr()).header.size;
            core::ptr::write(data_ptr(self.inner).add(size), value);
            (*self.inner.as_ptr()).header.size = size + 1;
        }
    }

    /// Removes element at the given index from the array.
    /// If the array was shared, this will make a copy of the array.
    pub fn remove(&mut self, row: usize) {
        if row >= self.len() {
            return;
        }
        self.detach(self.len());
        unsafe {
            let data = data_ptr(self.inner);
            core::ptr::drop_in_place(data.add(row));
            let size = (*self.inner.as_ptr()).header.size;
            core::ptr::copy(data.add(row + 1), data.add(row), size - 1 - row);
            (*self.inner.as_ptr()).header.size = size - 1;
        }
    }

    /// Inserts element at the given index in the array.
    /// If the array was shared, this will make a copy of the array.
    pub fn insert(&mut self, row: usize, value: T) {
        if row > self.len() {
            return;
        }
        self.detach(capacity_for_grow(self.capacity(), self.len() + 1, core::mem::size_of::<T>()));
        unsafe {
            let data = data_ptr(self.inner);
            let size = (*self.inner.as_ptr()).header.size;
            core::ptr::copy(data.add(row), data.add(row + 1), size - row);
            core::ptr::write(data.add(row), value);
            (*self.inner.as_ptr()).header.size = size + 1;
        }
    }

    /// Removes last element from the array and returns it.
    /// If the array was shared, this will make a copy of the array.
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            self.detach(self.len());
            // Safety: detach ensures exclusive ownership; len > 0 guarantees an element exists.
            unsafe {
                let size = (*self.inner.as_ptr()).header.size - 1;
                (*self.inner.as_ptr()).header.size = size;
                Some(core::ptr::read(data_ptr(self.inner).add(size)))
            }
        }
    }

    /// Resize the array to the given size.
    /// If the array was smaller new elements will be initialized with the value.
    /// If the array was bigger, extra elements will be discarded
    ///
    /// ```
    /// use i_slint_core::SharedVector;
    /// let mut shared_vector = SharedVector::<u32>::from_slice(&[1, 2, 3]);
    /// shared_vector.resize(5, 8);
    /// assert_eq!(shared_vector.as_slice(), &[1, 2, 3, 8, 8]);
    /// shared_vector.resize(2, 0);
    /// assert_eq!(shared_vector.as_slice(), &[1, 2]);
    /// ```
    pub fn resize(&mut self, new_len: usize, value: T) {
        if self.len() == new_len {
            return;
        }
        self.detach(new_len);
        // Safety: detach ensured that the array is not shared.
        let header = unsafe { &mut (*self.inner.as_ptr()).header };

        if header.size >= new_len {
            self.shrink(new_len);
        } else {
            let data_ptr = data_ptr(self.inner);
            while header.size < new_len {
                // Safety: The array must have a capacity of at least new_len because of the detach call earlier
                unsafe {
                    core::ptr::write(data_ptr.add(header.size), value.clone());
                }
                header.size += 1;
            }
        }
    }

    fn shrink(&mut self, new_len: usize) {
        if self.len() == new_len {
            return;
        }

        assert!(
            unsafe { (*self.inner.as_ptr()).header.refcount.load(atomic::Ordering::Relaxed) } == 1
        );
        // Safety: caller (and above assert) must ensure that the array is not shared.
        let header = unsafe { &mut (*self.inner.as_ptr()).header };
        let data_ptr = data_ptr(self.inner);

        while header.size > new_len {
            header.size -= 1;
            // Safety: The array was of size header.size, so there should be an element there
            unsafe {
                core::ptr::drop_in_place(data_ptr.add(header.size));
            }
        }
    }

    /// Clears the vector and removes all elements.
    pub fn clear(&mut self) {
        let is_shared =
            unsafe { (*self.inner.as_ptr()).header.refcount.load(atomic::Ordering::Acquire) } != 1;
        if is_shared {
            *self = SharedVector::default();
        } else {
            self.shrink(0)
        }
    }

    /// Reserves capacity for at least `additional` bytes more than the current vector's length.
    pub fn reserve(&mut self, additional: usize) {
        self.detach((self.len() + additional).max(self.capacity()))
    }
}

impl<T> Deref for SharedVector<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

/* FIXME: is this a good idea to implement DerefMut knowing what it might detach?
impl<T> DerefMut for SharedVector<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}*/

impl<T: Clone> From<&[T]> for SharedVector<T> {
    fn from(slice: &[T]) -> Self {
        let capacity = slice.len();
        let result = Self::with_capacity(capacity);
        for x in slice {
            unsafe {
                let size = (*result.inner.as_ptr()).header.size;
                core::ptr::write(data_ptr(result.inner).add(size), x.clone());
                (*result.inner.as_ptr()).header.size = size + 1;
            }
        }
        result
    }
}

impl<T, const N: usize> From<[T; N]> for SharedVector<T> {
    fn from(array: [T; N]) -> Self {
        array.into_iter().collect()
    }
}

impl<T> FromIterator<T> for SharedVector<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        let mut capacity = iter.size_hint().0;
        let mut result = Self::with_capacity(capacity);
        let mut size = 0;
        while let Some(x) = iter.next() {
            if size >= capacity {
                capacity = capacity_for_grow(
                    capacity,
                    size + 1 + iter.size_hint().0,
                    core::mem::size_of::<T>(),
                );
                unsafe {
                    (*result.inner.as_ptr()).header.refcount.store(0, atomic::Ordering::Relaxed)
                };
                let mut iter = IntoIter(IntoIterInner::UnShared(result.inner, 0));
                result.inner = alloc_with_capacity::<T>(capacity);
                match &mut iter.0 {
                    IntoIterInner::UnShared(old_inner, begin) => {
                        let old_data = data_ptr(*old_inner);
                        while *begin < size {
                            unsafe {
                                core::ptr::write(
                                    data_ptr(result.inner).add(*begin),
                                    core::ptr::read(old_data.add(*begin)),
                                );
                                *begin += 1;
                                (*result.inner.as_ptr()).header.size = *begin;
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            }
            debug_assert_eq!(result.len(), size);
            debug_assert!(result.capacity() > size);
            unsafe {
                core::ptr::write(data_ptr(result.inner).add(size), x);
                size += 1;
                (*result.inner.as_ptr()).header.size = size;
            }
        }
        result
    }
}

impl<T: Clone> Extend<T> for SharedVector<T> {
    fn extend<X: IntoIterator<Item = T>>(&mut self, iter: X) {
        let iter = iter.into_iter();
        let hint = iter.size_hint().0;
        if hint > 0 {
            self.detach(capacity_for_grow(
                self.capacity(),
                self.len() + hint,
                core::mem::size_of::<T>(),
            ));
        }
        for item in iter {
            self.push(item);
        }
    }
}

/// The empty singleton that `SharedVector::default()` for every element type T points to.
/// It only consists of a header, but the pointer is dereferenced as `SharedVectorInner<T>`,
/// so the static must satisfy the alignment of the most aligned `SharedVectorInner<T>` in
/// the program - not just `SharedVectorHeader`'s own alignment.
#[repr(C, align(16))]
struct SharedNull(SharedVectorHeader);

static SHARED_NULL: SharedNull =
    SharedNull(SharedVectorHeader { refcount: atomic::AtomicIsize::new(-1), size: 0, capacity: 0 });

impl<T> Default for SharedVector<T> {
    fn default() -> Self {
        const {
            assert!(
                core::mem::align_of::<SharedVectorInner<T>>()
                    <= core::mem::align_of::<SharedNull>(),
                "SharedVector element type is more aligned than the empty singleton"
            );
        }
        SharedVector { inner: NonNull::from(&SHARED_NULL).cast() }
    }
}

impl<T: Debug> Debug for SharedVector<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl<T> AsRef<[T]> for SharedVector<T> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, U> PartialEq<U> for SharedVector<T>
where
    U: ?Sized + AsRef<[T]>,
    T: PartialEq,
{
    fn eq(&self, other: &U) -> bool {
        self.as_slice() == other.as_ref()
    }
}

impl<T: Eq> Eq for SharedVector<T> {}

impl<T: Clone> IntoIterator for SharedVector<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(unsafe {
            if (*self.inner.as_ptr()).header.refcount.load(atomic::Ordering::Acquire) == 1 {
                let inner = self.inner;
                core::mem::forget(self);
                (*inner.as_ptr()).header.refcount.store(0, atomic::Ordering::Relaxed);
                IntoIterInner::UnShared(inner, 0)
            } else {
                IntoIterInner::Shared(self, 0)
            }
        })
    }
}

#[cfg(feature = "serde")]
use serde::ser::SerializeSeq;
#[cfg(feature = "serde")]
impl<T> serde::Serialize for SharedVector<T>
where
    T: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for item in self.iter() {
            seq.serialize_element(item)?;
        }
        seq.end()
    }
}

#[cfg(feature = "serde")]
impl<'de, T> serde::Deserialize<'de> for SharedVector<T>
where
    T: Clone + serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut elements: alloc::vec::Vec<T> = serde::Deserialize::deserialize(deserializer)?;
        let mut shared_vec = SharedVector::with_capacity(elements.len());
        for elem in elements.drain(..) {
            shared_vec.push(elem);
        }
        Ok(shared_vec)
    }
}

enum IntoIterInner<T> {
    Shared(SharedVector<T>, usize),
    // Elements up to the usize member are already moved out
    UnShared(NonNull<SharedVectorInner<T>>, usize),
}

impl<T> Drop for IntoIterInner<T> {
    fn drop(&mut self) {
        match self {
            IntoIterInner::Shared(..) => { /* drop of SharedVector takes care of it */ }
            IntoIterInner::UnShared(inner, begin) => unsafe {
                debug_assert_eq!(
                    (*inner.as_ptr()).header.refcount.load(atomic::Ordering::Relaxed),
                    0
                );
                let data_ptr = data_ptr(*inner);
                for x in (*begin)..(*inner.as_ptr()).header.size {
                    core::ptr::drop_in_place(data_ptr.add(x));
                }
                ::alloc::alloc::dealloc(
                    inner.as_ptr() as *mut u8,
                    compute_inner_layout::<T>((*inner.as_ptr()).header.capacity),
                )
            },
        }
    }
}

/// An iterator that moves out of a SharedVector.
///
/// This `struct` is created by the `into_iter` method on [`SharedVector`] (provided
/// by the [`IntoIterator`] trait).
pub struct IntoIter<T>(IntoIterInner<T>);

impl<T: Clone> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            IntoIterInner::Shared(array, moved) => {
                let result = array.as_slice().get(*moved).cloned();
                *moved += 1;
                result
            }
            IntoIterInner::UnShared(inner, begin) => unsafe {
                if *begin < (*inner.as_ptr()).header.size {
                    let data_ptr = data_ptr(*inner);
                    let r = core::ptr::read(data_ptr.add(*begin));
                    *begin += 1;
                    Some(r)
                } else {
                    None
                }
            },
        }
    }
}

#[test]
fn simple_test() {
    let x: SharedVector<i32> = SharedVector::from([1, 2, 3]);
    let y: SharedVector<i32> = SharedVector::from([3, 2, 1]);
    assert_eq!(x, x.clone());
    assert_ne!(x, y);
    let z: [i32; 3] = [1, 2, 3];
    assert_eq!(z, x.as_slice());
    let vec: std::vec::Vec<i32> = std::vec![1, 2, 3];
    assert_eq!(x, vec);
    let def: SharedVector<i32> = Default::default();
    assert_eq!(def, SharedVector::<i32>::default());
    assert_ne!(def, x);
}

#[test]
fn push_test() {
    let mut x: SharedVector<i32> = SharedVector::from([1, 2, 3]);
    let y = x.clone();
    x.push(4);
    x.push(5);
    x.push(6);
    assert_eq!(x.as_slice(), &[1, 2, 3, 4, 5, 6]);
    assert_eq!(y.as_slice(), &[1, 2, 3]);
}

#[test]
fn remove_test() {
    let mut x: SharedVector<i32> = SharedVector::from([1, 2, 3, 4, 5, 6]);
    let y = x.clone();
    x.remove(0);
    x.remove(1);
    x.remove(3);
    x.remove(4);
    x.push(42);
    x.remove(2);
    assert_eq!(x.as_slice(), &[2, 4, 42]);
    assert_eq!(y.as_slice(), &[1, 2, 3, 4, 5, 6]);
}

#[test]
fn insert_test() {
    let mut x: SharedVector<i32> = SharedVector::from([1, 2, 3]);
    let y = x.clone();
    x.insert(0, 42);
    assert_eq!(x.as_slice(), &[42, 1, 2, 3]);
    x.insert(2, 24);
    x.insert(6, 84);
    assert_eq!(x.as_slice(), &[42, 1, 24, 2, 3]);
    assert_eq!(y.as_slice(), &[1, 2, 3]);
}

#[test]
#[should_panic]
#[cfg_attr(miri, ignore)] // Miri aborts on large allocations before the panic can fire
fn invalid_capacity_test() {
    let _: SharedVector<u8> = SharedVector::with_capacity(usize::MAX / 2 - 1000);
}

#[test]
fn collect_from_iter_with_no_size_hint() {
    use std::string::{String, ToString};
    struct NoSizeHintIter<'a> {
        data: &'a [&'a str],
        i: usize,
    }

    impl Iterator for NoSizeHintIter<'_> {
        type Item = String;

        fn next(&mut self) -> Option<Self::Item> {
            if self.i >= self.data.len() {
                return None;
            }
            let item = self.data[self.i];
            self.i += 1;
            Some(item.to_string())
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            (0, None)
        }
    }

    // 5 elements to be above the initial "grow"-capacity of 4 and thus require one realloc.
    let input = NoSizeHintIter { data: &["Hello", "sweet", "world", "of", "iterators"], i: 0 };

    let shared_vec: SharedVector<String> = input.collect();
    assert_eq!(shared_vec.as_slice(), &["Hello", "sweet", "world", "of", "iterators"]);
}

#[test]
fn test_capacity_grows_only_when_needed() {
    let mut vec: SharedVector<u8> = SharedVector::with_capacity(2);
    vec.push(0);
    assert_eq!(vec.capacity(), 2);
    vec.push(0);
    assert_eq!(vec.capacity(), 2);
    vec.push(0);
    assert_eq!(vec.len(), 3);
    assert!(vec.capacity() > 2);
}

#[test]
fn test_vector_clear() {
    let mut vec: SharedVector<std::string::String> = Default::default();
    vec.clear();
    vec.push("Hello".into());
    vec.push("World".into());
    vec.push("of".into());
    vec.push("Vectors".into());

    let mut copy = vec.clone();

    assert_eq!(vec.len(), 4);
    let orig_cap = vec.capacity();
    assert!(orig_cap >= vec.len());
    vec.clear();
    assert_eq!(vec.len(), 0);
    assert_eq!(vec.capacity(), 0); // vec was shared, so start with new empty vector.
    vec.push("Welcome back".into());
    assert_eq!(vec.len(), 1);
    assert!(vec.capacity() >= vec.len());

    assert_eq!(copy.len(), 4);
    assert_eq!(copy.capacity(), orig_cap);
    copy.clear(); // copy is not shared (anymore), retain capacity.
    assert_eq!(copy.capacity(), orig_cap);
}

#[test]
fn pop_test() {
    let mut x: SharedVector<i32> = SharedVector::from([1, 2, 3]);
    let y = x.clone();
    assert_eq!(x.pop(), Some(3));
    assert_eq!(x.pop(), Some(2));
    assert_eq!(x.pop(), Some(1));
    assert_eq!(x.pop(), None);
    assert!(x.is_empty());
    assert_eq!(y.as_slice(), &[1, 2, 3]);
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    use super::*;

    #[unsafe(no_mangle)]
    /// This function is used for the low-level C++ interface to allocate the backing vector of a SharedVector.
    pub unsafe extern "C" fn slint_shared_vector_allocate(size: usize, align: usize) -> *mut u8 {
        unsafe { alloc::alloc::alloc(alloc::alloc::Layout::from_size_align(size, align).unwrap()) }
    }

    #[unsafe(no_mangle)]
    /// This function is used for the low-level C++ interface to deallocate the backing vector of a SharedVector
    pub unsafe extern "C" fn slint_shared_vector_free(ptr: *mut u8, size: usize, align: usize) {
        unsafe {
            alloc::alloc::dealloc(ptr, alloc::alloc::Layout::from_size_align(size, align).unwrap())
        }
    }

    #[unsafe(no_mangle)]
    /// This function is used for the low-level C++ interface to initialize the empty SharedVector.
    pub unsafe extern "C" fn slint_shared_vector_empty() -> *const u8 {
        &SHARED_NULL as *const _ as *const u8
    }
}

#[cfg(feature = "serde")]
#[test]
fn test_serialize_deserialize_sharedvector() {
    let v = SharedVector::from([1, 2, 3]);
    let serialized = serde_json::to_string(&v).unwrap();
    let deserialized: SharedVector<i32> = serde_json::from_str(&serialized).unwrap();
    assert_eq!(v, deserialized);
}

#[test]
fn test_reserve() {
    let mut v = SharedVector::from([1, 2, 3]);
    assert_eq!(v.capacity(), 3);
    v.reserve(1);
    assert_eq!(v.capacity(), 4);
    assert_eq!(v.len(), 3);
    v.push(4);
    v.push(5);
    assert_eq!(v.len(), 5);
    assert_eq!(v.capacity(), 8);
    v.reserve(1);
    assert_eq!(v.capacity(), 8);
    v.reserve(8);
    assert_eq!(v.len(), 5);
    assert_eq!(v.capacity(), 13);
}

#[test]
fn test_replace_range_all_matches() {
    let mut v = SharedVector::from([1, 2, 3, 1, 2, 3, 1, 2, 3]);
    v.replace_range(&[1, 2, 3], &[4, 5, 6], usize::MAX);
    assert_eq!(v.as_slice(), &[4, 5, 6, 4, 5, 6, 4, 5, 6]);
}

#[test]
fn test_replace_range_count_limit() {
    let mut v = SharedVector::from([1, 2, 3, 1, 2, 3, 1, 2, 3]);
    v.replace_range(&[1, 2, 3], &[4, 5, 6], 2);
    assert_eq!(v.as_slice(), &[4, 5, 6, 4, 5, 6, 1, 2, 3]);
}

#[test]
fn test_replace_range_non_overlapping() {
    let mut v = SharedVector::from([1, 1, 1]);
    v.replace_range(&[1, 1], &[2, 2], usize::MAX);
    assert_eq!(v.as_slice(), &[2, 2, 1]);
}

#[test]
fn test_replace_range() {
    let mut v = SharedVector::from([1, 2, 3, 4]);
    v.replace_range(&[2, 3, 4, 5], &[7, 8, 9, 9], 1);
    assert_eq!(v.as_slice(), &[1, 2, 3, 4]);
}

#[test]
fn test_aligned_element_type() {
    // The empty singleton behind `SharedVector::default()` must satisfy the element
    // type's alignment (it is dereferenced as `SharedVectorInner<T>`). Use an element
    // type with the maximum supported alignment so Miri catches a misaligned singleton
    // on any host, like the 8-aligned element types on wasm32 did.
    #[repr(align(16))]
    #[derive(Clone, Debug, PartialEq)]
    struct Aligned(u8);

    let mut x: SharedVector<Aligned> = Default::default();
    assert!(x.is_empty());
    for i in 0..8 {
        x.push(Aligned(i));
    }
    let y = x.clone();
    assert_eq!(x.pop(), Some(Aligned(7)));
    x.resize(4, Aligned(0));
    x.clear();
    assert!(x.is_empty());
    assert_eq!(y.len(), 8);
    assert_eq!(
        y.as_slice().iter().map(|a| a.0).collect::<std::vec::Vec<_>>(),
        std::vec![0, 1, 2, 3, 4, 5, 6, 7]
    );
}
