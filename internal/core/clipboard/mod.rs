// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::any::{Any, TypeId};

use self::mime::Mime;
use alloc::rc::Rc;

use crate::{SharedString, api::PlatformError};

pub mod mime;

pub trait PlatformClipboard {
    fn set(&self, clipboard: crate::platform::Clipboard, value: Rc<dyn ClipboardData>);

    fn has_type(&self, clipboard: crate::platform::Clipboard, type_: &ClipboardType) -> bool;

    fn read_plaintext(
        &self,
        clipboard: crate::platform::Clipboard,
    ) -> Result<SharedString, PlatformError> {
        for mime_type in [Mime::TEXT_PLAIN_UTF_8, Mime::TEXT_PLAIN] {
            if self.has_type(clipboard.clone(), &mime_type.clone().into()) {
                return self.read_string(clipboard.clone(), &mime_type);
            }
        }

        self.read_any(clipboard, TypeId::of::<SharedString>())
            .map(|arc_str| arc_str.downcast_ref::<SharedString>().unwrap().clone())
    }

    fn read_string(
        &self,
        clipboard: crate::platform::Clipboard,
        type_: &Mime,
    ) -> Result<SharedString, PlatformError> {
        let _ = clipboard;
        Err(PlatformError::ClipboardTypeNotFound(type_.clone().into()))
    }

    fn read_any(
        &self,
        clipboard: crate::platform::Clipboard,
        type_: core::any::TypeId,
    ) -> Result<Rc<dyn core::any::Any>, PlatformError> {
        let _ = clipboard;
        Err(PlatformError::ClipboardTypeNotFound(type_.into()))
    }
}

pub struct DummyPlatformClipboard;

impl PlatformClipboard for DummyPlatformClipboard {
    fn set(&self, _: crate::platform::Clipboard, _: Rc<dyn ClipboardData>) {}

    fn has_type(&self, _: crate::platform::Clipboard, _: &ClipboardType) -> bool {
        false
    }

    #[cfg(feature = "std")]
    fn read_string(
        &self,
        _: crate::platform::Clipboard,
        type_: &Mime,
    ) -> Result<SharedString, PlatformError> {
        Err(PlatformError::ClipboardTypeNotFound(type_.clone().into()))
    }

    fn read_any(
        &self,
        _: crate::platform::Clipboard,
        type_: TypeId,
    ) -> Result<Rc<dyn Any>, PlatformError> {
        Err(PlatformError::ClipboardTypeNotFound(type_.into()))
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClipboardType {
    Internal(TypeId),
    #[cfg(feature = "std")]
    External(Mime),
}

impl core::fmt::Display for ClipboardType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ClipboardType::Internal(type_id) => write!(f, "#{type_id:?}"),
            #[cfg(feature = "std")]
            ClipboardType::External(mime) => write!(f, "#{mime}"),
        }
    }
}

#[cfg(feature = "std")]
impl From<Mime> for ClipboardType {
    fn from(value: Mime) -> Self {
        Self::External(value)
    }
}

impl From<TypeId> for ClipboardType {
    fn from(value: TypeId) -> Self {
        Self::Internal(value)
    }
}

/// This trait is intended to be implemented by platforms for some internal custom data type, or they
/// can use the `TypeMap` implementation which is for application-internal use only. For example,
/// most embedded platforms will be able to just use `TypeMap`.
pub trait ClipboardData {
    /// This should be called before `read_*`, and returns `true` if this `ClipboardData` can be interpreted as
    /// the given type.
    fn has_type(&self, type_: &ClipboardType) -> bool;

    /// Helper to read plaintext via any of the supported plaintext formats
    fn read_plaintext(self: Rc<Self>) -> Result<SharedString, PlatformError> {
        #[cfg(feature = "std")]
        let out = self
            .clone()
            .read_string(&Mime::TEXT_PLAIN_UTF_8)
            .or_else(|_| self.clone().read_string(&Mime::TEXT_PLAIN))
            .or_else(|_| {
                self.read_any(TypeId::of::<SharedString>())
                    .map(|arc_str| arc_str.downcast_ref::<SharedString>().unwrap().clone())
            });

        #[cfg(not(feature = "std"))]
        let out = self
            .read_any(TypeId::of::<SharedString>())
            .map(|arc_str| arc_str.downcast_ref::<SharedString>().unwrap().clone());

        out
    }

    /// Read a string from the clipboard, in the specified MIME type.
    #[cfg(feature = "std")]
    fn read_string(self: Rc<Self>, type_: &Mime) -> Result<SharedString, PlatformError>;

    fn read_any(self: Rc<Self>, type_: TypeId) -> Result<Rc<dyn Any>, PlatformError>;
}

impl ClipboardData for dyn Any + Send + Sync {
    fn has_type(&self, type_: &ClipboardType) -> bool {
        *type_ == ClipboardType::Internal(self.type_id())
    }

    #[cfg(feature = "std")]
    fn read_string(self: Rc<Self>, type_: &Mime) -> Result<SharedString, PlatformError> {
        Err(PlatformError::ClipboardTypeNotFound(type_.clone().into()))
    }

    fn read_any(self: Rc<Self>, type_: TypeId) -> Result<Rc<dyn Any>, PlatformError> {
        if self.has_type(&type_.into()) {
            Ok(self.clone())
        } else {
            Err(PlatformError::ClipboardTypeNotFound(type_.into()))
        }
    }
}

impl ClipboardData for SharedString {
    fn has_type(&self, type_: &ClipboardType) -> bool {
        *type_ == Mime::TEXT_PLAIN.into() || *type_ == Mime::TEXT_PLAIN_UTF_8.into()
    }

    #[cfg(feature = "std")]
    fn read_string(self: Rc<Self>, type_: &Mime) -> Result<SharedString, PlatformError> {
        let clipboard_type = type_.clone().into();
        if self.has_type(&clipboard_type) {
            Ok((*self).clone())
        } else {
            Err(PlatformError::ClipboardTypeNotFound(clipboard_type))
        }
    }

    fn read_any(self: Rc<Self>, type_: TypeId) -> Result<Rc<dyn Any>, PlatformError> {
        if type_ == TypeId::of::<Self>() {
            // We might want to emit a warning here, telling the user that `read_string` is more efficient
            Ok(Rc::new(self.clone()))
        } else {
            Err(PlatformError::ClipboardTypeNotFound(type_.into()))
        }
    }
}

// Dummy implementation of `ClipboardData` that does nothing, used to clear the clipboard.
impl ClipboardData for () {
    fn has_type(&self, _: &ClipboardType) -> bool {
        false
    }

    #[cfg(feature = "std")]
    fn read_string(self: Rc<Self>, type_: &Mime) -> Result<SharedString, PlatformError> {
        Err(PlatformError::ClipboardTypeNotFound(type_.clone().into()))
    }

    fn read_any(self: Rc<Self>, type_: TypeId) -> Result<Rc<dyn Any>, PlatformError> {
        Err(PlatformError::ClipboardTypeNotFound(type_.into()))
    }
}
