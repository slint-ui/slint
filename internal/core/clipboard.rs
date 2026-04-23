// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use alloc::{boxed::Box, rc::Rc};

use crate::{AnyData, SharedString, api::PlatformError};

pub mod mime;

pub trait PlatformClipboard {
    fn set(&self, clipboard: crate::platform::Clipboard, value: ClipboardData);
    fn get(&self, clipboard: crate::platform::Clipboard) -> Result<ClipboardData, PlatformError>;

    fn clear(&self, clipboard: crate::platform::Clipboard) {
        self.set(clipboard, Rc::new(()).into())
    }
}

impl<T> From<Rc<T>> for ClipboardData
where
    T: ClipboardDataProvider + 'static,
{
    fn from(value: Rc<T>) -> Self {
        Self { provider: value }
    }
}

impl From<Rc<dyn ClipboardDataProvider>> for ClipboardData {
    fn from(value: Rc<dyn ClipboardDataProvider>) -> Self {
        Self { provider: value }
    }
}

#[derive(Clone)]
pub struct ClipboardData {
    provider: Rc<dyn ClipboardDataProvider>,
}

impl ClipboardData {
    pub fn mime_types(&self) -> &[&str] {
        self.provider.mime_types()
    }

    pub fn read<T>(&self, type_: &str) -> Result<T, PlatformError>
    where
        T: TryFrom<AnyData>,
        T::Error: core::error::Error + Send + Sync + 'static,
    {
        self.provider.clone().read(type_).and_then(|any| {
            any.try_into().map_err(|err| {
                PlatformError::OtherError(
                    Box::new(err) as Box<dyn core::error::Error + Send + Sync + 'static>
                )
            })
        })
    }
}

pub struct DummyPlatformClipboard;

impl PlatformClipboard for DummyPlatformClipboard {
    fn set(&self, _: crate::platform::Clipboard, _: ClipboardData) {}
    fn get(&self, _: crate::platform::Clipboard) -> Result<ClipboardData, PlatformError> {
        Ok(Rc::new(()).into())
    }
}

/// This trait is intended to be implemented by platforms for some internal custom data type, or they
/// can use the `TypeMap` implementation which is for application-internal use only. For example,
/// most embedded platforms will be able to just use `TypeMap`.
///
/// # Standard Types
///
/// Some MIME types should return specific, standardized types that consumers can rely on. This
/// is not a hard requirement, and this list may be expanded later, but if these conventions are
/// not followed then consumers may act inconsistently.
///
/// - `text/*`: `SharedString` (this will require boxing the `SharedString` inside an `Rc`)
pub trait ClipboardDataProvider {
    /// This should be called before `read`, and returns the set of available MIME types.
    fn mime_types(&self) -> &[&str];

    /// If this type can be interpreted as the given MIME type, return that type wrapped in an `Rc`.
    fn read(self: Rc<Self>, type_: &str) -> Result<AnyData, PlatformError>;
}

impl ClipboardDataProvider for SharedString {
    fn mime_types(&self) -> &[&str] {
        self::mime::PLAINTEXT
    }

    fn read(self: Rc<Self>, type_: &str) -> Result<AnyData, PlatformError> {
        if self.mime_types().contains(&type_) {
            Ok((*self).clone().into())
        } else {
            Err(PlatformError::ClipboardTypeNotFound(type_.into()))
        }
    }
}

// Dummy implementation of `ClipboardData` that does nothing, used to clear the clipboard.
impl ClipboardDataProvider for () {
    fn mime_types(&self) -> &[&str] {
        &[]
    }

    fn read(self: Rc<Self>, type_: &str) -> Result<AnyData, PlatformError> {
        Err(PlatformError::ClipboardTypeNotFound(type_.into()))
    }
}
