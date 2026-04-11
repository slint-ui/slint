// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Minimal user-defined property storage.
//!
//! In Slint SC, user-declared properties map to a [`Property<T>`] field in
//! the generated component struct.  Bindings are *not* stored: they are
//! inlined as plain Rust expressions in the generated `render` method and
//! re-evaluated every frame.  This eliminates dependency tracking, dirty
//! flags and cached intermediate values — the major sources of complexity
//! (and state) in the normal Slint runtime.
//!
//! Because a Slint SC component is used through `&self` (not `&mut self`),
//! the storage has to provide interior mutability.  `Copy` types use
//! [`core::cell::Cell`].

use core::cell::Cell;

/// A property with a `Copy` value type (`i32`, `f32`, `bool`, `Color`, ...).
#[derive(Default)]
pub struct Property<T: Copy>(Cell<T>);

impl<T: Copy> Property<T> {
    /// Constructs a new property with the given initial value.
    pub const fn new(initial: T) -> Self {
        Self(Cell::new(initial))
    }

    /// Reads the current value.
    pub fn get(&self) -> T {
        self.0.get()
    }

    /// Writes a new value.
    pub fn set(&self, value: T) {
        self.0.set(value);
    }
}
