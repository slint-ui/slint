// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Types and helpers related to the [`DataTransfer`] type, which implements type-indexed arbitrary
//! data transfer both within an application and between applications.

#![deny(missing_docs)]

use alloc::{boxed::Box, rc::Rc};
use core::{any::Any, cell::LazyCell};

use crate::{SharedString, SharedVector, api::Image};

mod mime;

type BytesResult = Result<SharedVector<u8>, ProviderError>;
type DataTransferProvider = Rc<LazyCell<BytesResult, Box<dyn FnOnce() -> BytesResult>>>;
type ProviderList = SharedVector<(SharedString, DataTransferProvider)>;

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
/// currently means that it will provide `text/plain` and `text/plain;charset=utf-8`, as well
/// as supplying a `user_data` that can be downcast to [`SharedString`]. To get this list of
/// types programmatically, see [`DataTransfer::PLAINTEXT_MIME_TYPES`].
///
/// The `From<Image>` implementation will create a `DataTransfer` that can be read as any of
/// the image formats supported by Slint. As above, this will set a `user_data` which can be
/// downcast to [`Image`], but it will also provide various MIME types. Currently, the MIME
/// types initialized by the `From<Image>` implementation are:
///
/// - `image/jpeg`
/// - `image/gif`
/// - `image/png`
/// - `image/bmp`
/// - `image/svg+xml` (if the `svg` feature is enabled)
///
/// To get this list of types programmatically, see [`DataTransfer::IMAGE_MIME_TYPES`].
#[derive(Clone, Default)]
#[repr(C)]
pub struct DataTransfer {
    /// A set of possibly-lazy providers. As `DataTransfer` is expected to be initialized
    /// in a single shot, with a relatively small number of elements, a vector is used for
    /// simplicity and to reduce cache misses.
    providers: ProviderList,
    /// A custom in-memory value. No MIME type-based dispatch is done here - if the user
    /// wants to store one of a set of possible values, they should store their own enum
    /// and handle the dispatch themselves.
    user_data: Option<Rc<dyn Any>>,
}

impl core::fmt::Debug for DataTransfer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DataTransfer")
            .field(
                "mime_types",
                &self.providers.iter().map(|(type_, _)| type_).collect::<alloc::vec::Vec<_>>(),
            )
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
        self.providers.as_ptr_range() == other.providers.as_ptr_range()
            && self.user_data.as_ref().map(Rc::as_ptr) == other.user_data.as_ref().map(Rc::as_ptr)
    }
}

/// Should match `DataTransfer` in `slint_data_transfer.h`
#[repr(C)]
struct DataTransferCppSizeMock([*const core::ffi::c_void; 3]);

const _: () = {
    assert!(
        core::mem::align_of::<DataTransfer>() == core::mem::align_of::<DataTransferCppSizeMock>()
    );
    assert!(
        core::mem::size_of::<DataTransfer>() == core::mem::size_of::<DataTransferCppSizeMock>()
    );
};

impl From<SharedString> for DataTransfer {
    fn from(value: SharedString) -> Self {
        let mut out = DataTransfer::default();

        out.set_user_data(Rc::new(value.clone()));

        let plaintext_no_null = SharedVector::from_slice(value.as_str().as_bytes());

        for mime_type in Self::PLAINTEXT_MIME_TYPES.iter().copied() {
            out.set_data(mime_type.into(), plaintext_no_null.clone());
        }

        // TODO: handle reading as UTF-16

        out
    }
}

impl From<Image> for DataTransfer {
    fn from(value: Image) -> Self {
        let mut out = DataTransfer::default();

        out.set_user_data(Rc::new(value.clone()));

        for mime_type in Self::IMAGE_MIME_TYPES.iter().copied() {
            out.set_provider(mime_type.into(), || {
                Err(ProviderError::other("Serializing images not yet implemented"))
            });
        }

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

impl DataTransfer {
    /// The set of MIME types recognized by [`DataTransfer::fetch_plaintext`].
    pub const PLAINTEXT_MIME_TYPES: &[&str] = mime::PLAINTEXT;
    /// The set of MIME types recognized by [`DataTransfer::fetch_image`].
    pub const IMAGE_MIME_TYPES: &[&str] =
        if cfg!(feature = "svg") { mime::IMAGE } else { mime::PIXMAP_IMAGE };

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
        if let Some(internal_str) =
            self.user_data().and_then(|any| any.downcast_ref::<SharedString>().cloned())
        {
            return Ok(internal_str);
        }

        // This only handles UTF-8 text for now, so we can ignore the actual MIME type,
        // but on Windows this should handle WTF-16 to UTF-8 conversion.
        self.find_type(Self::PLAINTEXT_MIME_TYPES).and_then(|(_, text)| {
            Ok(alloc::string::String::from_utf8(text.to_vec())
                .map_err(ProviderError::other)?
                .into())
        })
    }

    /// Helper to read this [`DataTransfer`] as an image, supporting multiple image types.
    ///
    /// The caller should assume that this method call may do IO.
    pub fn fetch_image(&self) -> Result<Image, DataTransferError> {
        if let Some(internal_image) =
            self.user_data().and_then(|any| any.downcast_ref::<Image>().cloned())
        {
            return Ok(internal_image);
        }

        #[cfg(feature = "image-decoders")]
        {
            // This only handles UTF-8 text for now, so we can ignore the actual MIME type,
            // but on Windows this should handle WTF-16 to UTF-8 conversion.
            self.find_type(Self::IMAGE_MIME_TYPES).and_then(|(type_, image_data)| {
                let image_ext = match type_ {
                    mime::image::BMP => "bmp",
                    mime::image::GIF => "gif",
                    mime::image::JPEG => "jpeg",
                    mime::image::PNG => "png",
                    mime::image::SVG => "svg",
                    _ => "",
                };

                Ok(Image::load_from_dynamic_data(&image_data, image_ext)
                    .map_err(|err| ProviderError::other(alloc::format!("{err}")))?)
            })
        }

        #[cfg(not(feature = "image-decoders"))]
        {
            Err(DataTransferError::TypeNotFound("image".into()))
        }
    }

    /// Set the data for a given MIME type as a byte vector that can be transferred
    /// between applications.
    pub fn set_data(&mut self, mime_type: SharedString, data: SharedVector<u8>) -> &mut Self {
        self.set_provider(mime_type, || Ok(data))
    }

    /// Get the set of available MIME types.
    pub fn mime_types(&self) -> impl Iterator<Item = &str> + use<'_> {
        self.providers.iter().map(|(type_, _)| &**type_)
    }

    /// Get the application-internal data represented by this [`DataTransfer`], if
    /// one exists.
    pub fn user_data(&self) -> Option<Rc<dyn Any>> {
        self.user_data.clone()
    }

    /// Set a lazily-evaluated provider for a given MIME type. The provider should return a byte
    /// vector that can be transferred between applications, or an error if something goes wrong.
    pub fn set_provider(
        &mut self,
        mime_type: SharedString,
        provider: impl FnOnce() -> Result<SharedVector<u8>, ProviderError> + 'static,
    ) -> &mut Self {
        self.providers.push((mime_type, Rc::new(LazyCell::new(Box::new(provider)))));
        self
    }

    /// Fetch the binary representation of this [`DataTransfer`] as the specified MIME
    /// type.
    ///
    /// The caller should assume that this method call may do IO.
    pub fn fetch(&self, mime_type: &str) -> Result<SharedVector<u8>, DataTransferError> {
        self.providers
            .iter()
            .find_map(|(type_, value)| {
                (type_ == mime_type).then(|| (***value).clone().map_err(Into::into))
            })
            .unwrap_or_else(|| Err(DataTransferError::TypeNotFound(mime_type.into())))
    }

    /// Fetch the binary representation of this [`DataTransfer`] as one of the specified
    /// MIME types, with the preference order specified by this type (the order of `mime_types`
    /// is not taken into account).
    ///
    /// The caller should assume that this method call may do IO.
    fn find_type(
        &self,
        mime_types: &[&str],
    ) -> Result<(&str, SharedVector<u8>), DataTransferError> {
        // TODO: Should we prefer to go in the order specified in the data transfer or in the
        // `mime_types` argument? Only X11 and Wayland have a proper concept of source-defined
        // type ordering, so maybe it makes more sense for the destination to define the
        // preference order.
        self.providers
            .iter()
            .find_map(|(type_, value)| {
                mime_types
                    .contains(&&**type_)
                    .then(|| (***value).clone().map(|value| (&**type_, value)).map_err(Into::into))
            })
            .unwrap_or_else(|| {
                if let Some(last) = mime_types.last().copied() {
                    Err(DataTransferError::TypeNotFound(last.into()))
                } else {
                    // For now, this method is private, so we can afford to have a less-helpful error message
                    Err(ProviderError::other("Internal error: No MIME types supplied").into())
                }
            })
    }
}
