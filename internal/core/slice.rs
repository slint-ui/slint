// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! FFI-friendly slice

#![allow(unsafe_code)]
#![warn(missing_docs)]

use core::{cmp::PartialEq, fmt::Debug, marker::PhantomData, ptr::NonNull};

/// That's basically the same as `&'a [T]`  but `repr(C)`
///
/// Can be constructed from a slice using the from trait.
///
/// ```
/// use i_slint_core::slice::Slice;
/// let x = Slice::from_slice(&[1, 2, 3]);
/// assert_eq!(x.len(), 3);
/// assert_eq!(x[1], 2);
/// let slice : &'static [u32] = x.as_slice();
/// ```
///
/// Comparing two Slice compare their pointer, not the content.
/// ```
/// use i_slint_core::slice::Slice;
/// let a = Slice::from_slice(&[1, 2, 3]);
/// let slice = [1, 2, 3, 4];
/// let b = Slice::from(&slice[..3]);
/// // two slice coming from the same pointer are equal.
/// assert_eq!(b, Slice::from(&slice[..3]));
/// // these are different because the pointers are different
/// assert_ne!(a, b);
/// // use as_slice to compare the contents
/// assert_eq!(a.as_slice(), b.as_slice());
/// ```
#[repr(C)]
#[derive(PartialEq)]
pub struct Slice<'a, T> {
    /// Invariant, this is a valid slice of len `len`
    ptr: NonNull<T>,
    len: usize,
    phantom: PhantomData<&'a [T]>,
}

impl<T: Debug> Debug for Slice<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_slice().fmt(f)
    }
}

// Need to implement manually otherwise it is not implemented if T do not implement Copy / Clone
impl<T> Copy for Slice<'_, T> {}

impl<T> Clone for Slice<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> Slice<'a, T> {
    /// Return a slice
    pub fn as_slice(self) -> &'a [T] {
        // Safety: it ptr is supposed to be a valid slice of given length
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    /// Create from a native slice
    pub const fn from_slice(slice: &'a [T]) -> Self {
        Slice {
            // Safety: a slice is never null
            ptr: unsafe { NonNull::new_unchecked(slice.as_ptr() as *mut T) },
            len: slice.len(),
            phantom: PhantomData,
        }
    }
}

impl<'a, T> From<&'a [T]> for Slice<'a, T> {
    fn from(slice: &'a [T]) -> Self {
        Self::from_slice(slice)
    }
}

impl<T> core::ops::Deref for Slice<'_, T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> Default for Slice<'_, T> {
    fn default() -> Self {
        Self::from_slice(&[])
    }
}

/// Safety: Slice is the same as a rust slice, and a slice of Sync T is Sync
unsafe impl<T: Sync> Sync for Slice<'_, T> {}
/// Safety: Slice is the same as a rust slice, and a slice of Send T is Sync
unsafe impl<T: Sync> Send for Slice<'_, T> {}
