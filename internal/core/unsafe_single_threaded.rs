// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Unsafe module that is only enabled when the `std` feature is off.
//! It re-implements the thread_local macro with statics

#![allow(unsafe_code)]
#[macro_export]
macro_rules! SLINT__thread_local_inner {
    ($(#[$($meta:tt)*])* $vis:vis $ident:ident $ty:ty $block:block) => {
        $(#[$($meta)*])*
        $vis static $ident: crate::unsafe_single_threaded::FakeThreadStorage<$ty> = {
            fn init() -> $ty $block
            crate::unsafe_single_threaded::FakeThreadStorage::new(init)
        };
    };
}

#[macro_export]
macro_rules! thread_local_ {
    // Taken from stdlib!

    // empty (base case for the recursion)
    () => {};

    ($(#[$attr:meta])* $vis:vis static $name:ident: $t:ty = const $init:block; $($rest:tt)*) => (
        $crate::SLINT__thread_local_inner!($(#[$attr])* $vis $name $t $init);
        $crate::thread_local!($($rest)*);
    );

    ($(#[$attr:meta])* $vis:vis static $name:ident: $t:ty = const $init:block) => (
        $crate::SLINT__thread_local_inner!($(#[$attr])* $vis $name $t $init);
    );

    // process multiple declarations
    ($(#[$attr:meta])* $vis:vis static $name:ident: $t:ty = $init:expr; $($rest:tt)*) => (
        $crate::SLINT__thread_local_inner!($(#[$attr])* $vis $name $t  { $init });
        $crate::thread_local!($($rest)*);
    );

    // handle a single declaration
    ($(#[$attr:meta])* $vis:vis static $name:ident: $t:ty = $init:expr) => (
        $crate::SLINT__thread_local_inner!($(#[$attr])* $vis $name $t { $init });
    );
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
        Ok(self.with(f))
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
