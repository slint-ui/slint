// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::any::{Any, TypeId};

use alloc::sync::Arc;

use crate::SharedString;

pub use mime;

pub trait PlatformClipboard {
    fn set(&self, clipboard: crate::platform::Clipboard, value: Arc<dyn ClipboardData>);

    fn read_plaintext(
        &self,
        clipboard: crate::platform::Clipboard,
    ) -> Result<SharedString, ClipboardError> {
        #[cfg(feature = "std")]
        let out = self
            .read_string(clipboard.clone(), &mime::TEXT_PLAIN_UTF_8)
            .or_else(|_| self.read_string(clipboard.clone(), &mime::TEXT_PLAIN))
            .or_else(|_| {
                self.read_any(clipboard, TypeId::of::<SharedString>())
                    .map(|arc_str| arc_str.downcast_ref::<SharedString>().unwrap().clone())
            });

        #[cfg(not(feature = "std"))]
        let out = self
            .read_any(clipboard, TypeId::of::<SharedString>())
            .map(|arc_str| arc_str.downcast_ref::<SharedString>().unwrap().clone());

        out
    }

    #[cfg(feature = "std")]
    fn read_string(
        &self,
        clipboard: crate::platform::Clipboard,
        type_: &mime::Mime,
    ) -> Result<SharedString, ClipboardError> {
        let _ = clipboard;
        Err(ClipboardError::TypeNotFound(type_.clone().into()))
    }

    fn read_any(
        &self,
        clipboard: crate::platform::Clipboard,
        type_: TypeId,
    ) -> Result<Arc<dyn Any>, ClipboardError> {
        let _ = clipboard;
        Err(ClipboardError::TypeNotFound(type_.into()))
    }
}

pub struct DummyPlatformClipboard;

impl PlatformClipboard for DummyPlatformClipboard {
    fn set(&self, _: crate::platform::Clipboard, _: Arc<dyn ClipboardData>) {}

    #[cfg(feature = "std")]
    fn read_string(
        &self,
        _: crate::platform::Clipboard,
        type_: &mime::Mime,
    ) -> Result<SharedString, ClipboardError> {
        Err(ClipboardError::TypeNotFound(type_.clone().into()))
    }

    fn read_any(
        &self,
        _: crate::platform::Clipboard,
        type_: TypeId,
    ) -> Result<Arc<dyn Any>, ClipboardError> {
        Err(ClipboardError::TypeNotFound(type_.into()))
    }
}

pub enum ClipboardError {
    /// Some IO error occurred while fetching clipboard data
    #[cfg(feature = "std")]
    Io(std::io::Error),
    #[cfg(feature = "std")]
    Other(alloc::boxed::Box<dyn std::error::Error + Send + Sync>),
    /// `read_*` was called on [`ClipboardData`], but no value of that type was provided. Consider calling [`ClipboardData::has_type`].
    TypeNotFound(ClipboardType),
    /// An explicit error message
    Message(alloc::string::String),
}

#[cfg(feature = "std")]
impl From<alloc::boxed::Box<dyn std::error::Error + Send + Sync>> for ClipboardError {
    fn from(value: alloc::boxed::Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::Other(value)
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for ClipboardError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<alloc::string::String> for ClipboardError {
    fn from(value: alloc::string::String) -> Self {
        Self::Message(value)
    }
}

impl core::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ClipboardError::Io(error) => write!(f, "{error}"),
            ClipboardError::TypeNotFound(clipboard_type) => {
                write!(f, "No value of type {clipboard_type} was provided")
            }
            ClipboardError::Other(err) => write!(f, "{err}"),
            ClipboardError::Message(msg) => write!(f, "{msg}"),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClipboardType {
    Internal(TypeId),
    #[cfg(feature = "std")]
    External(mime::Mime),
}

impl core::fmt::Display for ClipboardType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ClipboardType::Internal(type_id) => write!(f, "#{type_id:?}"),
            ClipboardType::External(mime) => write!(f, "#{mime}"),
        }
    }
}

impl From<mime::Mime> for ClipboardType {
    fn from(value: mime::Mime) -> Self {
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
    fn read_plaintext(self: Arc<Self>) -> Result<SharedString, ClipboardError> {
        #[cfg(feature = "std")]
        let out = self
            .clone()
            .read_string(&mime::TEXT_PLAIN_UTF_8)
            .or_else(|_| self.clone().read_string(&mime::TEXT_PLAIN))
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
    fn read_string(self: Arc<Self>, type_: &mime::Mime) -> Result<SharedString, ClipboardError>;

    fn read_any(self: Arc<Self>, type_: TypeId) -> Result<Arc<dyn Any>, ClipboardError>;
}

impl ClipboardData for dyn Any + Send + Sync {
    fn has_type(&self, type_: &ClipboardType) -> bool {
        *type_ == ClipboardType::Internal(self.type_id())
    }

    #[cfg(feature = "std")]
    fn read_string(self: Arc<Self>, type_: &mime::Mime) -> Result<SharedString, ClipboardError> {
        Err(ClipboardError::TypeNotFound(type_.clone().into()))
    }

    fn read_any(self: Arc<Self>, type_: TypeId) -> Result<Arc<dyn Any>, ClipboardError> {
        if self.has_type(&type_.into()) {
            Ok(self.clone())
        } else {
            Err(ClipboardError::TypeNotFound(type_.into()))
        }
    }
}

impl ClipboardData for SharedString {
    fn has_type(&self, type_: &ClipboardType) -> bool {
        *type_ == mime::TEXT_PLAIN.into() || *type_ == mime::TEXT_PLAIN_UTF_8.into()
    }

    #[cfg(feature = "std")]
    fn read_string(self: Arc<Self>, type_: &mime::Mime) -> Result<SharedString, ClipboardError> {
        let clipboard_type = type_.clone().into();
        if self.has_type(&clipboard_type) {
            Ok((*self).clone())
        } else {
            Err(ClipboardError::TypeNotFound(clipboard_type))
        }
    }

    fn read_any(self: Arc<Self>, type_: TypeId) -> Result<Arc<dyn Any>, ClipboardError> {
        if type_ == TypeId::of::<Self>() {
            // We might want to emit a warning here, telling the user that `read_string` is more efficient
            Ok(Arc::new(self.clone()))
        } else {
            Err(ClipboardError::TypeNotFound(type_.into()))
        }
    }
}

// Dummy implementation of `ClipboardData` that does nothing, used to clear the clipboard.
impl ClipboardData for () {
    fn has_type(&self, _: &ClipboardType) -> bool {
        false
    }

    #[cfg(feature = "std")]
    fn read_string(self: Arc<Self>, type_: &mime::Mime) -> Result<SharedString, ClipboardError> {
        Err(ClipboardError::TypeNotFound(type_.clone().into()))
    }

    fn read_any(self: Arc<Self>, type_: TypeId) -> Result<Arc<dyn Any>, ClipboardError> {
        Err(ClipboardError::TypeNotFound(type_.into()))
    }
}
