// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

//! Unsafe module that is only enabled when the `std` feature is off.
//! It re-implements the thread_local macro with statics

#![allow(unsafe_code)]
#[macro_export]
macro_rules! thread_local_ {
    ($(#[$($meta:tt)*])* $vis:vis static $ident:ident : $ty:ty = $expr:expr) => {
        $(#[$($meta)*])*
        $vis static $ident: crate::unsafe_single_threaded::FakeThreadStorage<$ty> = {
            fn init() -> $ty { $expr }
            crate::unsafe_single_threaded::FakeThreadStorage::new(init)
        };
    };
}
pub struct FakeThreadStorage<T, F = fn() -> T>(once_cell::unsync::OnceCell<T>, F);
impl<T, F> FakeThreadStorage<T, F> {
    pub const fn new(f: F) -> Self {
        Self(once_cell::unsync::OnceCell::new(), f)
    }
}
impl<T> FakeThreadStorage<T> {
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(self.0.get_or_init(self.1))
    }
    pub fn try_with<R>(&self, f: impl FnOnce(&T) -> R) -> Result<R, ()> {
        Ok(f(self.0.get().ok_or(())?))
    }
}
// Safety: the unsafe_single_threaded feature means we will only be called from a single thread
unsafe impl<T, F> Send for FakeThreadStorage<T, F> {}
unsafe impl<T, F> Sync for FakeThreadStorage<T, F> {}

pub use thread_local_ as thread_local;

pub struct OnceCell<T>(once_cell::unsync::OnceCell<T>);
impl<T> OnceCell<T> {
    pub const fn new() -> Self {
        Self(once_cell::unsync::OnceCell::new())
    }
    pub fn get(&self) -> Option<&T> {
        self.0.get()
    }
    pub fn set(&self, value: T) -> Result<(), T> {
        self.0.set(value)
    }
}

// Safety: the unsafe_single_threaded feature means we will only be called from a single thread
unsafe impl<T> Send for OnceCell<T> {}
unsafe impl<T> Sync for OnceCell<T> {}
