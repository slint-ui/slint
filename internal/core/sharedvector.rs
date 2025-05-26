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

unsafe fn drop_inner<T>(mut inner: NonNull<SharedVectorInner<T>>) {
    debug_assert_eq!(inner.as_ref().header.refcount.load(atomic::Ordering::Relaxed), 0);
    let data_ptr = inner.as_mut().data.as_mut_ptr();
    for x in 0..inner.as_ref().header.size {
        core::ptr::drop_in_place(data_ptr.add(x));
    }
    alloc::alloc::dealloc(
        inner.as_ptr() as *mut u8,
        compute_inner_layout::<T>(inner.as_ref().header.capacity),
    )
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
        unsafe {
            if self
                .inner
                .cast::<SharedVectorHeader>()
                .as_ref()
                .refcount
                .load(atomic::Ordering::Relaxed)
                < 0
            {
                return;
            }
            if self.inner.as_ref().header.refcount.fetch_sub(1, atomic::Ordering::SeqCst) == 1 {
                drop_inner(self.inner)
            }
        }
    }
}

impl<T> Clone for SharedVector<T> {
    fn clone(&self) -> Self {
        unsafe {
            if self
                .inner
                .cast::<SharedVectorHeader>()
                .as_ref()
                .refcount
                .load(atomic::Ordering::Relaxed)
                > 0
            {
                self.inner.as_ref().header.refcount.fetch_add(1, atomic::Ordering::SeqCst);
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
        unsafe { self.inner.as_ref().data.as_ptr() }
    }

    /// Number of elements in the array
    pub fn len(&self) -> usize {
        unsafe { self.inner.cast::<SharedVectorHeader>().as_ref().size }
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
            // Safety: When len > 0, we know that the pointer holds an array of the size of len
            unsafe { core::slice::from_raw_parts(self.as_ptr(), self.len()) }
        }
    }

    /// Returns the number of elements the vector can hold without reallocating, when not shared
    fn capacity(&self) -> usize {
        unsafe { self.inner.cast::<SharedVectorHeader>().as_ref().capacity }
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
        let is_shared =
            unsafe { self.inner.as_ref().header.refcount.load(atomic::Ordering::Relaxed) } != 1;
        if !is_shared && new_capacity <= self.capacity() {
            return;
        }
        let mut new_array = SharedVector::with_capacity(new_capacity);
        core::mem::swap(&mut self.inner, &mut new_array.inner);
        let mut size = 0;
        for x in new_array.into_iter() {
            assert_ne!(size, new_capacity);
            unsafe {
                core::ptr::write(self.inner.as_mut().data.as_mut_ptr().add(size), x);
                size += 1;
                self.inner.as_mut().header.size = size;
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
        unsafe {
            core::ptr::write(
                self.inner.as_mut().data.as_mut_ptr().add(self.inner.as_mut().header.size),
                value,
            );
            self.inner.as_mut().header.size += 1;
        }
    }

    /// Removes last element from the array and returns it.
    /// If the array was shared, this will make a copy of the array.
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            self.detach(self.len());
            unsafe {
                self.inner.as_mut().header.size -= 1;
                Some(core::ptr::read(self.inner.as_mut().data.as_mut_ptr().add(self.len())))
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
        let inner = unsafe { self.inner.as_mut() };

        if inner.header.size >= new_len {
            self.shrink(new_len);
        } else {
            while inner.header.size < new_len {
                // Safety: The array must have a capacity of at least new_len because of the detach call earlier
                unsafe {
                    core::ptr::write(inner.data.as_mut_ptr().add(inner.header.size), value.clone());
                }
                inner.header.size += 1;
            }
        }
    }

    fn shrink(&mut self, new_len: usize) {
        if self.len() == new_len {
            return;
        }

        assert!(
            unsafe { self.inner.as_ref().header.refcount.load(atomic::Ordering::Relaxed) } == 1
        );
        // Safety: caller (and above debug_assert) must ensure that the array is not shared.
        let inner = unsafe { self.inner.as_mut() };

        while inner.header.size > new_len {
            inner.header.size -= 1;
            // Safety: The array was of size inner.header.size, so there should be an element there
            unsafe {
                core::ptr::drop_in_place(inner.data.as_mut_ptr().add(inner.header.size));
            }
        }
    }

    /// Clears the vector and removes all elements.
    pub fn clear(&mut self) {
        let is_shared =
            unsafe { self.inner.as_ref().header.refcount.load(atomic::Ordering::Relaxed) } != 1;
        if is_shared {
            *self = SharedVector::default();
        } else {
            self.shrink(0)
        }
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
        let mut result = Self::with_capacity(capacity);
        for x in slice {
            unsafe {
                core::ptr::write(
                    result.inner.as_mut().data.as_mut_ptr().add(result.inner.as_mut().header.size),
                    x.clone(),
                );
                result.inner.as_mut().header.size += 1;
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
                    result.inner.as_ref().header.refcount.store(0, atomic::Ordering::Relaxed)
                };
                let mut iter = IntoIter(IntoIterInner::UnShared(result.inner, 0));
                result.inner = alloc_with_capacity::<T>(capacity);
                match &mut iter.0 {
                    IntoIterInner::UnShared(old_inner, begin) => {
                        while *begin < size {
                            unsafe {
                                core::ptr::write(
                                    result.inner.as_mut().data.as_mut_ptr().add(*begin),
                                    core::ptr::read(old_inner.as_ref().data.as_ptr().add(*begin)),
                                );
                                *begin += 1;
                                result.inner.as_mut().header.size = *begin;
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            }
            debug_assert_eq!(result.len(), size);
            debug_assert!(result.capacity() > size);
            unsafe {
                core::ptr::write(result.inner.as_mut().data.as_mut_ptr().add(size), x);
                size += 1;
                result.inner.as_mut().header.size = size;
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

static SHARED_NULL: SharedVectorHeader =
    SharedVectorHeader { refcount: atomic::AtomicIsize::new(-1), size: 0, capacity: 0 };

impl<T> Default for SharedVector<T> {
    fn default() -> Self {
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
            if self.inner.as_ref().header.refcount.load(atomic::Ordering::Relaxed) == 1 {
                let inner = self.inner;
                core::mem::forget(self);
                inner.as_ref().header.refcount.store(0, atomic::Ordering::Relaxed);
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
            IntoIterInner::UnShared(mut inner, begin) => unsafe {
                debug_assert_eq!(inner.as_ref().header.refcount.load(atomic::Ordering::Relaxed), 0);
                let data_ptr = inner.as_mut().data.as_mut_ptr();
                for x in (*begin)..inner.as_ref().header.size {
                    core::ptr::drop_in_place(data_ptr.add(x));
                }
                ::alloc::alloc::dealloc(
                    inner.as_ptr() as *mut u8,
                    compute_inner_layout::<T>(inner.as_ref().header.capacity),
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
                if *begin < inner.as_ref().header.size {
                    let r = core::ptr::read(inner.as_ref().data.as_ptr().add(*begin));
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
#[should_panic]
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
        alloc::alloc::alloc(alloc::alloc::Layout::from_size_align(size, align).unwrap())
    }

    #[unsafe(no_mangle)]
    /// This function is used for the low-level C++ interface to deallocate the backing vector of a SharedVector
    pub unsafe extern "C" fn slint_shared_vector_free(ptr: *mut u8, size: usize, align: usize) {
        alloc::alloc::dealloc(ptr, alloc::alloc::Layout::from_size_align(size, align).unwrap())
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
