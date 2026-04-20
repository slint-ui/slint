// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::any::Any;

use self::mime::Mime;
use alloc::rc::Rc;

use crate::{SharedString, api::PlatformError};

pub mod mime;

pub trait PlatformClipboard {
    fn set(&self, clipboard: crate::platform::Clipboard, value: Rc<dyn ClipboardData>);

    fn has_type(&self, clipboard: crate::platform::Clipboard, type_: &Mime) -> bool;

    fn read_plaintext(
        &self,
        clipboard: crate::platform::Clipboard,
    ) -> Result<SharedString, PlatformError> {
        for mime_type in [Mime::TEXT_PLAIN_UTF_8, Mime::TEXT_PLAIN] {
            if self.has_type(clipboard.clone(), &mime_type.clone().into()) {
                return self.read_string(clipboard.clone(), &mime_type);
            }
        }

        Err(PlatformError::ClipboardTypeNotFound(Mime::TEXT_PLAIN_UTF_8))
    }

    fn read_string(
        &self,
        clipboard: crate::platform::Clipboard,
        type_: &Mime,
    ) -> Result<SharedString, PlatformError> {
        let _ = clipboard;
        Err(PlatformError::ClipboardTypeNotFound(type_.clone()))
    }

    fn read_any(
        &self,
        clipboard: crate::platform::Clipboard,
        type_: &Mime,
    ) -> Result<Rc<dyn core::any::Any>, PlatformError> {
        let _ = clipboard;
        Err(PlatformError::ClipboardTypeNotFound(type_.clone()))
    }
}

pub struct DummyPlatformClipboard;

impl PlatformClipboard for DummyPlatformClipboard {
    fn set(&self, _: crate::platform::Clipboard, _: Rc<dyn ClipboardData>) {}

    fn has_type(&self, _: crate::platform::Clipboard, _: &Mime) -> bool {
        false
    }
}

/// This trait is intended to be implemented by platforms for some internal custom data type, or they
/// can use the `TypeMap` implementation which is for application-internal use only. For example,
/// most embedded platforms will be able to just use `TypeMap`.
pub trait ClipboardData {
    /// This should be called before `read_*`, and returns `true` if this `ClipboardData` can be interpreted as
    /// the given type.
    fn has_type(&self, type_: &Mime) -> bool;

    /// Helper to read plaintext via any of the supported plaintext formats
    fn read_plaintext(self: Rc<Self>) -> Result<SharedString, PlatformError> {
        for mime_type in [Mime::TEXT_PLAIN_UTF_8, Mime::TEXT_PLAIN] {
            if self.has_type(&mime_type) {
                return self.read_string(&mime_type);
            }
        }

        Err(PlatformError::ClipboardTypeNotFound(Mime::TEXT_PLAIN_UTF_8))
    }

    /// Read a string from the clipboard, in the specified MIME type.
    fn read_string(self: Rc<Self>, type_: &Mime) -> Result<SharedString, PlatformError> {
        Err(PlatformError::ClipboardTypeNotFound(type_.clone()))
    }

    fn read_any(self: Rc<Self>, type_: &Mime) -> Result<Rc<dyn Any>, PlatformError> {
        Err(PlatformError::ClipboardTypeNotFound(type_.clone()))
    }
}

impl ClipboardData for dyn Any + Send + Sync {
    fn has_type(&self, type_: &Mime) -> bool {
        *type_ == Mime::from(self.type_id())
    }

    fn read_any(self: Rc<Self>, type_: &Mime) -> Result<Rc<dyn Any>, PlatformError> {
        if self.has_type(type_) {
            Ok(self.clone())
        } else {
            Err(PlatformError::ClipboardTypeNotFound(type_.clone()))
        }
    }
}

impl ClipboardData for SharedString {
    fn has_type(&self, type_: &Mime) -> bool {
        type_.is_plaintext()
    }

    fn read_string(self: Rc<Self>, type_: &Mime) -> Result<SharedString, PlatformError> {
        let clipboard_type = type_.clone().into();
        if self.has_type(&clipboard_type) {
            Ok((*self).clone())
        } else {
            Err(PlatformError::ClipboardTypeNotFound(clipboard_type))
        }
    }
}

// Dummy implementation of `ClipboardData` that does nothing, used to clear the clipboard.
impl ClipboardData for () {
    fn has_type(&self, _: &Mime) -> bool {
        false
    }
}
