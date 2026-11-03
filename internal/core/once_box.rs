// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![allow(unsafe_code)]

use alloc::boxed::Box;
use core::cell::Cell;
use core::ffi::c_void;
use core::pin::Pin;
use core::ptr::NonNull;

/// A heap-allocated value that's created on first access.
///
/// Use it in an item for state that most instances never need.
/// It's a single null pointer until first access, and C++ constructs it without calling into Rust.
#[repr(transparent)]
pub struct OnceBox<T>(Cell<Option<NonNull<OnceBoxAlloc<T>>>>);

/// The allocation starts with its drop function, so that C++ can free it through `slint_once_box_drop`.
#[repr(C)]
struct OnceBoxAlloc<T> {
    drop: unsafe extern "C" fn(*mut c_void),
    value: T,
}

impl<T> Default for OnceBox<T> {
    fn default() -> Self {
        Self(Cell::new(None))
    }
}

impl<T> OnceBox<T> {
    /// Returns the value, or `None` if it isn't initialized yet.
    pub fn get(&self) -> Option<&T> {
        // Safety: the allocation lives until `self` is dropped.
        self.0.get().map(|alloc| unsafe { &(*alloc.as_ptr()).value })
    }

    /// Returns the value, initializing it with `init` if it isn't initialized yet.
    ///
    /// Panics if `init` initializes this box.
    pub fn get_or_init(&self, init: impl FnOnce() -> T) -> &T {
        if let Some(value) = self.get() {
            return value;
        }
        let value = init();
        assert!(self.0.get().is_none(), "OnceBox initialized during its own initialization");
        let alloc =
            NonNull::from(Box::leak(Box::new(OnceBoxAlloc { drop: drop_alloc::<T>, value })));
        self.0.set(Some(alloc));
        // Safety: the allocation lives until `self` is dropped.
        unsafe { &(*alloc.as_ptr()).value }
    }

    /// Like [`Self::get_or_init`], but returns the value pinned.
    pub fn get_or_init_pin(&self, init: impl FnOnce() -> T) -> Pin<&T> {
        // Safety: the value is never moved out of its allocation, and the allocation is only
        // freed by dropping the value in place.
        unsafe { Pin::new_unchecked(self.get_or_init(init)) }
    }
}

impl<T: Default> OnceBox<T> {
    /// Returns the value, initializing it with `T::default()` if it isn't initialized yet.
    pub fn get_or_default(&self) -> &T {
        self.get_or_init(T::default)
    }
}

/// Initializes the value with `T::default()` on first access.
impl<T: Default> core::ops::Deref for OnceBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.get_or_default()
    }
}

impl<T> Drop for OnceBox<T> {
    fn drop(&mut self) {
        if let Some(alloc) = self.0.take() {
            // Safety: the allocation was leaked from a Box in `get_or_init`.
            drop(unsafe { Box::from_raw(alloc.as_ptr()) });
        }
    }
}

/// # Safety
/// `alloc` must be a pointer to an `OnceBoxAlloc<T>` that was leaked from a Box.
unsafe extern "C" fn drop_alloc<T>(alloc: *mut c_void) {
    drop(unsafe { Box::from_raw(alloc as *mut OnceBoxAlloc<T>) });
}

/// Frees the allocation of a non-empty `OnceBox`, which C++ calls in its destructor.
///
/// # Safety
/// `alloc` must be the non-null pointer of a `OnceBox`, which must not be used afterwards.
#[cfg(feature = "ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_once_box_drop(alloc: *mut c_void) {
    // Safety: every allocation starts with its drop function, see `OnceBoxAlloc`.
    unsafe {
        let drop = *(alloc as *const unsafe extern "C" fn(*mut c_void));
        drop(alloc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;

    struct DropCounter(Rc<Cell<u32>>);

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn initializes_once() {
        let once_box = OnceBox::<u32>::default();
        assert_eq!(once_box.get(), None);
        assert_eq!(*once_box.get_or_init(|| 1), 1);
        assert_eq!(*once_box.get_or_init(|| 2), 1);
        assert_eq!(once_box.get(), Some(&1));
    }

    #[test]
    fn deref_initializes_default() {
        let once_box = OnceBox::<u32>::default();
        assert_eq!(*once_box, 0);
        assert_eq!(once_box.get(), Some(&0));
    }

    #[test]
    fn drops_value_once() {
        let drops = Rc::new(Cell::new(0));
        drop(OnceBox::<DropCounter>::default());
        let once_box = OnceBox::<DropCounter>::default();
        once_box.get_or_init(|| DropCounter(drops.clone()));
        drop(once_box);
        assert_eq!(drops.get(), 1);
    }

    #[cfg(feature = "ffi")]
    #[test]
    fn ffi_drop() {
        let drops = Rc::new(Cell::new(0));
        let once_box = core::mem::ManuallyDrop::new(OnceBox::<DropCounter>::default());
        once_box.get_or_init(|| DropCounter(drops.clone()));
        let alloc = once_box.0.get().unwrap().as_ptr();
        unsafe { slint_once_box_drop(alloc as *mut c_void) };
        assert_eq!(drops.get(), 1);
    }

    #[test]
    #[should_panic]
    fn reentrant_init_panics() {
        let once_box = OnceBox::<u32>::default();
        once_box.get_or_init(|| *once_box.get_or_init(|| 1));
    }
}
