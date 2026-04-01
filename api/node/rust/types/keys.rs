// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::input::Keys;

/// Represents a key binding created by the `@keys(...)` macro in Slint.
///
/// This is an opaque type — instances are only obtained from Slint properties.
/// Use `toString()` to get a platform-native representation of the key binding
/// (e.g. "Ctrl+A" on Linux/Windows, "⌘A" on macOS).
#[napi]
pub struct SlintKeys {
    pub(crate) inner: Keys,
}

impl From<Keys> for SlintKeys {
    fn from(keys: Keys) -> Self {
        Self { inner: keys }
    }
}

#[napi]
impl SlintKeys {
    /// Create a `Keys` from a list of string parts, e.g. `["Control", "Shift?", "Z"]`.
    ///
    /// Each element is either a modifier name or a key name. Throws on parse failure.
    #[napi(factory)]
    pub fn from_parts(parts: Vec<String>) -> napi::Result<Self> {
        Keys::from_parts(parts.iter().map(|s| s.as_str()))
            .map(|k| Self { inner: k })
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    /// Returns the platform-native string representation of this key binding.
    #[napi]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }

    /// Returns `true` if this key binding is equal to `other`.
    #[napi]
    pub fn equals(&self, other: &SlintKeys) -> bool {
        self.inner == other.inner
    }
}
