// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Dynamically-typed data, for runtime-generic code such as [`ClipboardData`](crate::clipboard::ClipboardData).

use crate::{SharedString, api::Image};

#[derive(Clone)]
enum AnyDataInner {
    String(SharedString),
    Image(Image),
}

/// A piece of data of unspecified type. Use the accessor methods to downcast this to a specific type.
#[derive(Clone)]
pub struct AnyData {
    inner: AnyDataInner,
}

impl AnyData {
    /// Returns a reference to the inner value if it is a `SharedString`, or `None` if it isn’t.
    pub fn as_string(&self) -> Option<SharedString> {
        match &self.inner {
            AnyDataInner::String(string) => Some(string.clone()),
            _ => None,
        }
    }

    /// Returns a reference to the inner value if it is an `Image`, or `None` if it isn’t.
    pub fn as_image(&self) -> Option<Image> {
        match &self.inner {
            AnyDataInner::Image(image) => Some(image.clone()),
            _ => None,
        }
    }
}

impl From<SharedString> for AnyData {
    fn from(value: SharedString) -> Self {
        Self { inner: AnyDataInner::String(value) }
    }
}

impl From<Image> for AnyData {
    fn from(value: Image) -> Self {
        Self { inner: AnyDataInner::Image(value) }
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct AnyDataCastError;

impl core::fmt::Display for AnyDataCastError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Could not cast AnyData to target type")
    }
}

impl core::error::Error for AnyDataCastError {}

impl TryFrom<AnyData> for SharedString {
    type Error = AnyDataCastError;

    fn try_from(value: AnyData) -> Result<Self, Self::Error> {
        value.as_string().ok_or(AnyDataCastError)
    }
}

impl TryFrom<AnyData> for Image {
    type Error = AnyDataCastError;

    fn try_from(value: AnyData) -> Result<Self, Self::Error> {
        value.as_image().ok_or(AnyDataCastError)
    }
}
