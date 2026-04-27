// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use alloc::{boxed::Box, rc::Rc};

use crate::{
    AnyData, SharedString,
    api::{Image, PlatformError},
};

pub mod mime;

pub trait PlatformClipboard {
    fn set(&self, clipboard: crate::platform::Clipboard, value: ClipboardData);
    fn get(&self, clipboard: crate::platform::Clipboard) -> Result<ClipboardData, PlatformError>;

    fn clear(&self, clipboard: crate::platform::Clipboard) {
        self.set(clipboard, Default::default())
    }
}

/// Wrapper around [`dyn ClipboardDataProvider`](crate::clipboard::ClipboardDataProvider) so we can implement the traits that we
/// need for usage in Slint (such as `PartialEq` and `Debug`), limit the interface, and expose to non-Rust runtimes.
#[derive(Clone)]
pub struct ClipboardData {
    provider: Rc<dyn ClipboardDataProvider>,
}

impl<T> From<T> for ClipboardData
where
    T: ClipboardDataProvider + 'static,
{
    fn from(value: T) -> Self {
        Self { provider: Rc::new(value) }
    }
}

impl From<Rc<dyn ClipboardDataProvider>> for ClipboardData {
    fn from(value: Rc<dyn ClipboardDataProvider>) -> Self {
        Self { provider: value }
    }
}

impl core::fmt::Debug for ClipboardData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ClipboardData")
            .field("mime_types", &self.provider.mime_types())
            .finish_non_exhaustive()
    }
}

impl Default for ClipboardData {
    fn default() -> Self {
        Self::from(())
    }
}

impl PartialEq for ClipboardData {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::addr_eq(&*self.provider, &*other.provider)
    }
}

impl ClipboardData {
    pub const PLAINTEXT_MIME_TYPES: &[&str] = mime::PLAINTEXT;
    pub const IMAGE_MIME_TYPES: &[&str] = mime::IMAGE;

    #[inline]
    pub fn mime_types(&self) -> &[&str] {
        self.provider.mime_types()
    }

    #[inline]
    pub fn has_any_type(&self, types: &[&str]) -> bool {
        self.mime_types().iter().any(|type_| types.contains(type_))
    }

    #[inline]
    pub fn has_plaintext(&self) -> bool {
        self.has_any_type(Self::PLAINTEXT_MIME_TYPES)
    }

    #[inline]
    pub fn has_image(&self) -> bool {
        self.has_any_type(Self::IMAGE_MIME_TYPES)
    }

    /// Read the inner type as one of the supplied MIME types, in order. For each type in `types`, if
    /// [`self.mime_types()`](ClipboardData::mime_types) contains the type, then [`ClipboardDataProvider::read`]
    /// will be called. If this either fails or casting it to `T` fails, then the next type will be tried
    /// until one succeeds or the list is exhausted.
    #[inline]
    pub fn read<T>(&self, types: &[&str]) -> Result<T, PlatformError>
    where
        T: TryFrom<AnyData>,
        T::Error: core::error::Error + Send + Sync + 'static,
    {
        let mut last_err = None;
        for type_ in types {
            // Theoretically somewhat inefficient to loop over MIME types for every element of `types`,
            // but in practice both `self.mime_types()` and the `types` argument will be very small.
            if !self.has_any_type(&[type_]) {
                continue;
            }

            match self.provider.clone().read(type_).and_then(|any| {
                any.try_into().map_err(|err| {
                    PlatformError::OtherError(
                        Box::new(err) as Box<dyn core::error::Error + Send + Sync + 'static>
                    )
                })
            }) {
                Ok(value) => return Ok(value),
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            PlatformError::ClipboardTypeNotFound((*types.first().unwrap_or(&"{unknown}")).into())
        }))
    }
}

pub struct DummyPlatformClipboard;

impl PlatformClipboard for DummyPlatformClipboard {
    fn set(&self, _: crate::platform::Clipboard, _: ClipboardData) {}
    fn get(&self, _: crate::platform::Clipboard) -> Result<ClipboardData, PlatformError> {
        Ok(Default::default())
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

impl ClipboardDataProvider for Image {
    fn mime_types(&self) -> &[&str] {
        self::mime::IMAGE
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
