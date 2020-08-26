/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! module for the SharedArray and related things
#![allow(unsafe_code)]
#![warn(missing_docs)]
use core::mem::MaybeUninit;
use std::{fmt::Debug, fmt::Display, ops::Deref};
use triomphe::{Arc, HeaderWithLength, ThinArc};

#[derive(Clone)]
#[repr(C)]
/// SharedArray holds a reference-counted read-only copy of `[T]`.
pub struct SharedArray<T: 'static> {
    /// Invariant: The usize header is the `len` of the vector, the contained buffer is `[T]`
    inner: ThinArc<usize, MaybeUninit<T>>,
}

struct PaddingFillingIter<'a, U> {
    iter: &'a mut dyn Iterator<Item = MaybeUninit<U>>,
    pos: usize,
    len: usize,
    padding_elements: usize,
}

impl<'a, U> PaddingFillingIter<'a, U> {
    fn new(len: usize, iter: &'a mut dyn Iterator<Item = MaybeUninit<U>>) -> Self {
        let alignment = core::mem::align_of::<usize>();
        let mut padding_elements = if len == 0 { 1 } else { 0 }; // ThinArc can't deal with empty arrays, so add padding for empty arrays.

        // Add padding to ensure that the size in bytes is a multiple of the pointer alignment. This can mean different
        // increments depending on whether sizeof(U) is less or greater than align_of(usize).
        loop {
            let size_in_bytes = (len + padding_elements) * core::mem::size_of::<U>();
            let byte_aligned_size = (size_in_bytes + alignment - 1) & !(alignment - 1);
            let padding_bytes = byte_aligned_size - size_in_bytes;
            if padding_bytes == 0 {
                break;
            }
            padding_elements += 1;
        }

        Self { iter, pos: 0, len, padding_elements }
    }
}

impl<'a, U: Clone> Iterator for PaddingFillingIter<'a, U> {
    type Item = MaybeUninit<U>;
    fn next(&mut self) -> Option<MaybeUninit<U>> {
        let pos = self.pos;
        self.pos += 1;
        if pos < self.len {
            self.iter.next()
        } else if pos < self.len + self.padding_elements {
            Some(MaybeUninit::uninit())
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let l = self.len + self.padding_elements;
        (l, Some(l))
    }
}
impl<'a, U: Clone> core::iter::ExactSizeIterator for PaddingFillingIter<'a, U> {}

impl<T: Clone> SharedArray<T> {
    fn as_ptr(&self) -> *const T {
        self.inner.slice.as_ptr() as *const T
    }

    /// Size of the string, in bytes
    pub fn len(&self) -> usize {
        self.inner.header.header
    }

    /// Return a slice to the array
    pub fn as_slice(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.as_ptr(), self.len()) }
    }

    /// Constructs a new SharedArray from the given iterator.
    pub fn from_iter(iter: impl Iterator<Item = T> + ExactSizeIterator) -> Self {
        let len = iter.len();
        let item_iter = &mut iter.map(|item| MaybeUninit::new(item));
        let iter = PaddingFillingIter::new(len, item_iter);

        SharedArray {
            inner: Arc::into_thin(Arc::from_header_and_iter(
                HeaderWithLength::new(len, iter.size_hint().0),
                iter,
            )),
        }
    }

    /// Constructs a new SharedArray from the given slice.
    pub fn from(slice: &[T]) -> Self {
        SharedArray::from_iter(slice.iter().cloned())
    }
}

impl<T: Clone> Deref for SharedArray<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}
trait StaticNull: Sized + 'static {
    const NULL: once_cell::sync::Lazy<ThinArc<usize, MaybeUninit<Self>>>;
}
impl<T: Clone + Copy + Default + Sized + 'static> StaticNull for T {
    const NULL: once_cell::sync::Lazy<ThinArc<usize, MaybeUninit<T>>> =
        once_cell::sync::Lazy::new(|| {
            let len = 0;
            let null_iter = &mut std::iter::empty();
            let iter = PaddingFillingIter::new(len, null_iter);

            Arc::into_thin(Arc::from_header_and_iter(
                HeaderWithLength::new(len, iter.size_hint().0),
                iter,
            ))
        });
}

impl<T: Clone + Copy + Default> Default for SharedArray<T> {
    fn default() -> Self {
        SharedArray { inner: StaticNull::NULL.clone() }
    }
}

impl<T: Clone + Debug> Debug for SharedArray<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl<T: Clone + Debug> Display for SharedArray<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl<T: Clone> AsRef<[T]> for SharedArray<T> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, U> PartialEq<U> for SharedArray<T>
where
    U: ?Sized + AsRef<[T]>,
    T: Clone + PartialEq,
{
    fn eq(&self, other: &U) -> bool {
        self.as_slice() == other.as_ref()
    }
}

impl<T: Clone + PartialEq> Eq for SharedArray<T> {}

#[test]
fn simple_test() {
    let x: SharedArray<i32> = SharedArray::from(&[1, 2, 3]);
    let y: SharedArray<i32> = SharedArray::from(&[3, 2, 1]);
    assert_eq!(x, x.clone());
    assert_ne!(x, y);
    let z: [i32; 3] = [1, 2, 3];
    assert_eq!(z, x.as_slice());
    let vec: Vec<i32> = vec![1, 2, 3];
    assert_eq!(x, vec);
    let def: SharedArray<i32> = Default::default();
    assert_eq!(def, SharedArray::<i32>::default());
    assert_ne!(def, x);
}

pub(crate) mod ffi {
    use super::*;

    #[no_mangle]
    /// This function is used for the low-level C++ interface to allocate the backing vector for an empty shared array.
    pub unsafe extern "C" fn sixtyfps_shared_array_new_null(out: *mut SharedArray<u8>) {
        core::ptr::write(out, SharedArray::<u8>::default());
    }

    #[no_mangle]
    /// This function is used for the low-level C++ interface to clone a shared array by increasing its reference count.
    pub unsafe extern "C" fn sixtyfps_shared_array_clone(
        out: *mut SharedArray<u8>,
        source: &SharedArray<u8>,
    ) {
        core::ptr::write(out, source.clone());
    }

    #[no_mangle]
    /// This function is used for the low-level C++ interface to decrease the reference count of a shared array.
    pub unsafe extern "C" fn sixtyfps_shared_array_drop(out: *mut SharedArray<u8>) {
        // ?? This won't call drop on the right type...
        core::ptr::read(out);
    }
}
