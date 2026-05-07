// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Types and helpers related to the [`DataTransfer`] type, which implements type-indexed arbitrary
//! data transfer both within an application and between applications.

use alloc::{boxed::Box, rc::Rc};
use core::any::Any;

use crate::{SharedString, api::Image};

#[cfg(feature = "ffi")]
pub mod ffi;

/// Hidden type to make `DataTransfer` smaller and easier to use in FFI.
///
/// In particular, `Image` is a different size depending on feature flags, so this
/// allows `DataTransfer` to have a normalized size no matter what flags are enabled.
#[derive(Default, Clone, PartialEq)]
struct DataTransferInner {
    // TODO: Custom binary data providers with custom MIME types.
    /// Special-cased support for images, as the precise implementation of transferring
    /// images differs between platforms.
    image: Option<Image>,
    /// Special-cased support for plaintext, as the precise implementation of transferring
    /// text differs between platforms.
    plaintext: Option<SharedString>,
}

/// `DataTransfer` abstracts over the various ways of transferring data within an application
/// and between applications.
///
/// The details will depend on the current platform, but the common features are:
///
/// - Each `DataTransfer` contains multiple views over the same data in different formats,
///   specified by MIME type
/// - The `DataTransfer` may contain an in-memory representation of the data, which can be
///   sent and received within the current application
/// - Serializing to a given format may be done eagerly or lazily
///
/// Wayland and X11 have direct support for specifying data transfer types using the MIME
/// standard, but on platforms that have a different system, MIME types will be mapped to
/// the host OS's types. For example, on Windows there is special-cased support for bitmaps
/// using [`DataPackage.SetBitmap`][windows-bmp]. That would be mapped to the `image/bmp`
/// MIME type.
///
/// [windows-bmp]: https://learn.microsoft.com/en-us/uwp/api/windows.applicationmodel.datatransfer.datapackage.setbitmap
///
/// > Note: The full mapping between OS type and MIME type for each supported platform will be
/// > elaborated on here once the drag-and-drop feature has been completed. To track its
/// > progress, see [the tracking issue][dnd-tracking-issue].
///
/// [dnd-tracking-issue]: https://github.com/slint-ui/slint/issues/1967
///
/// The easiest way to construct this type is via the [`From<SharedString>`](SharedString) and
/// [`From<Image>`](Image) implementations. The opposite of these operations are
/// [`fetch_plaintext`](DataTransfer::fetch_plaintext) and [`fetch_image`](DataTransfer::fetch_image)
/// respectively.
///
/// The `From<SharedString>` implementation will create a plaintext `DataTransfer`. This
/// currently means that it will provide `text/plain` and `text/plain;charset=utf-8`.
///
/// The `From<Image>` implementation will create a `DataTransfer` that can be read as any of
/// the image formats supported by Slint. Currently, the MIME types initialized by the
/// `From<Image>` implementation are:
///
/// - `image/jpeg`
/// - `image/gif`
/// - `image/png`
/// - `image/bmp`
/// - `image/svg+xml` (if the `svg` feature is enabled)
#[derive(Clone, Default)]
#[repr(C)]
pub struct DataTransfer {
    /// Special-cased types. `Option<Rc>` to prevent allocating if this `DataTransfer`
    /// only contains `user_data`.
    inner: Option<Rc<DataTransferInner>>,
    /// A custom in-memory value. No MIME type-based dispatch is done here - if the user
    /// wants to store one of a set of possible values, they should store their own enum
    /// and handle the dispatch themselves.
    user_data: Option<Rc<dyn Any>>,
}

impl core::fmt::Debug for DataTransfer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DataTransfer")
            .field("mime_types", &self.mime_types().collect::<alloc::vec::Vec<_>>())
            .field("has_user_data", &self.user_data.is_some())
            .finish()
    }
}

// `PartialEq` doesn't really make sense for `DataTransfer`, but since it's required for values
// that Slint interacts with, we can at least make a best-effort attempt. This will return true
// if `other` is an unmodified clone of `self`, but if any modification has been done to either
// value since cloning then this will return false even if the two values are semantically
// identical.
impl PartialEq for DataTransfer {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
            && self.user_data.as_ref().map(Rc::as_ptr) == other.user_data.as_ref().map(Rc::as_ptr)
    }
}

impl From<SharedString> for DataTransfer {
    fn from(value: SharedString) -> Self {
        let mut out = DataTransfer::default();

        out.set_plaintext(value);

        out
    }
}

impl From<Image> for DataTransfer {
    fn from(value: Image) -> Self {
        let mut out = DataTransfer::default();

        out.set_image(value);

        out
    }
}

/// An error which can occur while fetching data from a `DataTransfer`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum DataTransferError {
    /// The type was not listed in the set of available MIME types.
    TypeNotFound(SharedString),
    /// Some error occurred while running a provider.
    Provider(ProviderError),
}

impl core::fmt::Display for DataTransferError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DataTransferError::TypeNotFound(mime_type) => {
                write!(f, "Type not supplied by data transfer: {mime_type}")
            }
            DataTransferError::Provider(provider_error) => {
                write!(f, "Error running data transfer provider: {provider_error}")
            }
        }
    }
}

/// An error that can occur when a provider for a [`DataTransfer`] runs. See [`DataTransfer::set_provider`].
///
/// This is essentially just a wrapper around [`std::io::Error`] which removes the
/// ability to access the [`ErrorKind`](std::io::ErrorKind) on `no_std` targets.
#[derive(Debug, Clone)]
pub struct ProviderError {
    #[cfg(feature = "std")]
    inner: Rc<std::io::Error>,
    #[cfg(not(feature = "std"))]
    inner: Rc<dyn core::error::Error + Send + Sync + 'static>,
}

impl core::ops::Deref for ProviderError {
    type Target = dyn core::error::Error;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

#[cfg(feature = "std")]
impl ProviderError {
    /// Create a [`ProviderError`] with an unknown [`ErrorKind`](std::io::ErrorKind).
    pub fn other<E>(other: E) -> Self
    where
        E: Into<Box<dyn core::error::Error + Send + Sync + 'static>>,
    {
        std::io::Error::other(other).into()
    }

    /// Returns the corresponding [`ErrorKind`](std::io::ErrorKind) for this error.
    pub fn kind(&self) -> std::io::ErrorKind {
        self.inner.kind()
    }
}

#[cfg(not(feature = "std"))]
impl ProviderError {
    /// Create a [`ProviderError`] from some arbitrary error type.
    pub fn other<E>(other: E) -> Self
    where
        E: Into<Box<dyn core::error::Error + Send + Sync + 'static>>,
    {
        ProviderError { inner: Rc::from(other.into()) }
    }
}

impl core::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.inner.fmt(f)
    }
}

#[cfg(feature = "std")]
impl<T> From<T> for ProviderError
where
    T: Into<Rc<std::io::Error>>,
{
    fn from(value: T) -> Self {
        Self { inner: value.into() }
    }
}

impl<T> From<T> for DataTransferError
where
    T: Into<ProviderError>,
{
    fn from(value: T) -> Self {
        Self::Provider(value.into())
    }
}

const PLAINTEXT_MIME_TYPES: &[&str] = &["text/plain", "text/plain;charset=utf-8"];

impl DataTransfer {
    /// Set the internal special-cased image field
    fn set_image(&mut self, image: Image) -> &mut Self {
        Rc::make_mut(self.inner.get_or_insert_default()).image = Some(image);
        self
    }

    /// Set the internal special-cased plaintext field
    fn set_plaintext(&mut self, plaintext: SharedString) -> &mut Self {
        Rc::make_mut(self.inner.get_or_insert_default()).plaintext = Some(plaintext);
        self
    }

    /// Returns `true` if this data transfer advertises that it is readable as an [`Image`].
    ///
    /// This means that This does not necessarily mean that `fetch_image` will return `Ok`,
    /// as an I/O error may occur.
    pub fn has_image(&self) -> bool {
        self.inner.as_ref().is_some_and(|inner| inner.image.is_some())
    }

    /// Returns `true` if this data transfer advertises that it is readable as plaintext.
    ///
    /// This means that This does not necessarily mean that `fetch_plaintext` will return
    /// `Ok`, as an I/O error may occur.
    pub fn has_plaintext(&self) -> bool {
        self.inner.as_ref().is_some_and(|inner| inner.plaintext.is_some())
            || self.mime_types().any(|ty| PLAINTEXT_MIME_TYPES.contains(&ty))
    }

    /// Set the application-internal data represented by this [`DataTransfer`].
    /// This can be read with [`DataTransfer::user_data`], and allows circumventing
    /// serialize/deserializing the data to bytes when a drag-and-drop or copy-paste
    /// operation stays within the application.
    pub fn set_user_data(&mut self, value: Rc<dyn Any>) -> &mut Self {
        self.user_data = Some(value);
        self
    }

    /// Helper to read this [`DataTransfer`] as plaintext, supporting multiple encodings.
    ///
    /// The caller should assume that this method call may do IO.
    pub fn fetch_plaintext(&self) -> Result<SharedString, DataTransferError> {
        self.inner
            .as_ref()
            .and_then(|inner| inner.plaintext.clone())
            .ok_or_else(|| DataTransferError::TypeNotFound(PLAINTEXT_MIME_TYPES[0].into()))
    }

    /// Helper to read this [`DataTransfer`] as an image, supporting multiple image types.
    ///
    /// The caller should assume that this method call may do IO.
    pub fn fetch_image(&self) -> Result<Image, DataTransferError> {
        self.inner
            .as_ref()
            .and_then(|inner| inner.image.clone())
            .ok_or_else(|| DataTransferError::TypeNotFound("image".into()))
    }

    /// Get the set of available MIME types.
    pub fn mime_types(&self) -> impl Iterator<Item = &str> + '_ {
        let image_mime_types = self.has_image().then_some(
            // We can't extract this to a const because it's an iterator adapter,
            // and we can't extract it to a function because borrowck fails to
            // narrow `impl Iterator<Item = &'static str>` to `&'_ str`.
            image::ImageFormat::all()
                .filter(|fmt| fmt.writing_enabled())
                .map(|fmt| fmt.to_mime_type()),
        );
        let plaintext_mime_types =
            self.has_plaintext().then_some(PLAINTEXT_MIME_TYPES.iter().copied());

        plaintext_mime_types.into_iter().flatten().chain(image_mime_types.into_iter().flatten())
    }

    /// Get the application-internal data represented by this [`DataTransfer`], if
    /// one exists.
    pub fn user_data(&self) -> Option<Rc<dyn Any>> {
        self.user_data.clone()
    }
}
