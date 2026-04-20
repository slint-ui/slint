// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Dynamically-typed data, for runtime-generic code such as [`ClipboardData`](crate::clipboard::Clipboard).

use crate::SharedString;

/// A piece of data of unspecified type. Use the accessor methods to downcast this to a specific type.
#[derive(Clone)]
pub struct AnyData {
    // Eventually this will be something like `Rc<dyn Any>`, but since we only support `SharedString`
    // for now, this simplifies the implementation.
    inner: SharedString,
}

impl AnyData {
    /// Returns a reference to the inner value if it is of type `T`, or `None` if it isn’t.
    pub fn as_string(&self) -> Option<SharedString> {
        Some(self.inner.clone())
    }
}

impl From<SharedString> for AnyData {
    fn from(value: SharedString) -> Self {
        Self { inner: value }
    }
}
