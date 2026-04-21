// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use alloc::rc::Rc;

use crate::{AnyData, SharedString, api::PlatformError};

pub mod mime;

pub trait PlatformClipboard {
    fn set(&self, clipboard: crate::platform::Clipboard, value: Rc<dyn ClipboardData>);
    fn get(
        &self,
        clipboard: crate::platform::Clipboard,
    ) -> Result<Rc<dyn ClipboardData>, PlatformError>;

    fn clear(&self, clipboard: crate::platform::Clipboard) {
        self.set(clipboard, Rc::new(()))
    }
}

pub struct DummyPlatformClipboard;

impl PlatformClipboard for DummyPlatformClipboard {
    fn set(&self, _: crate::platform::Clipboard, _: Rc<dyn ClipboardData>) {}
    fn get(&self, _: crate::platform::Clipboard) -> Result<Rc<dyn ClipboardData>, PlatformError> {
        Ok(Rc::new(()))
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
pub trait ClipboardData {
    /// This should be called before `read`, and returns the set of available MIME types.
    fn mime_types(&self) -> &[&str];

    /// If this type can be interpreted as the given MIME type, return that type wrapped in an `Rc`.
    fn read(self: Rc<Self>, type_: &str) -> Result<AnyData, PlatformError>;
}

impl ClipboardData for SharedString {
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
impl ClipboardData for () {
    fn mime_types(&self) -> &[&str] {
        &[]
    }

    fn read(self: Rc<Self>, type_: &str) -> Result<AnyData, PlatformError> {
        Err(PlatformError::ClipboardTypeNotFound(type_.into()))
    }
}
