// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Types and helpers related to the [`DataTransfer`] type, which implements type-indexed arbitrary
//! data transfer both within an application and between applications.

#![deny(missing_docs)]

use alloc::{boxed::Box, rc::Rc, string::String};
use core::{any::Any, cell::LazyCell};

use crate::{
    SharedString, SharedVector,
    api::{Image, PlatformError},
};

mod mime;

type BytesResult = Result<SharedVector<u8>, PlatformError>;
type DataTransferProvider = Rc<LazyCell<BytesResult, Box<dyn FnOnce() -> BytesResult>>>;
type ProviderList = SharedVector<(SharedString, DataTransferProvider)>;

/// `DataTransfer` abstracts over the various ways of transferring data within an application
/// and between applications. The details will depend on the current platform, but the common
/// features are:
///
/// - Each `DataTransfer` contains multiple views over the same data in different formats,
///   specified by MIME type
/// - The `DataTransfer` may contain an in-memory representation of the data, which can be
///   sent and received within the current application
/// - Serializing to a given format may be done eagerly or lazily
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
            .field("has_internal", &self.user_data.is_some())
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

        for mime_type in mime::PLAINTEXT.iter().copied() {
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

        let available_image_types =
            if cfg!(feature = "svg") { mime::IMAGE } else { mime::PIXMAP_IMAGE };

        for mime_type in available_image_types.iter().copied() {
            out.set_provider(mime_type.into(), || {
                Err(PlatformError::Other("Serializing images not yet implemented".into()))
            });
        }

        out
    }
}

/// Helper so that [`BytesResult`] can be cloned when reading.
fn clone_platform_result<T>(res: &Result<T, PlatformError>) -> Result<T, PlatformError>
where
    T: Clone,
{
    fn clone_error(err: &PlatformError) -> PlatformError {
        match err {
            PlatformError::NoPlatform => PlatformError::NoPlatform,
            PlatformError::NoEventLoopProvider => PlatformError::NoEventLoopProvider,
            PlatformError::SetPlatformError(err) => PlatformError::SetPlatformError(err.clone()),
            PlatformError::Unsupported => PlatformError::Unsupported,
            PlatformError::Other(msg) => PlatformError::Other(msg.clone()),
            PlatformError::OtherError(err) => PlatformError::Other(alloc::format!("{err}")),
            PlatformError::DataTransferTypeNotFound(err) => {
                PlatformError::DataTransferTypeNotFound(err.clone())
            }
        }
    }

    res.as_ref().cloned().map_err(clone_error)
}

impl DataTransfer {
    /// The set of MIME types recognized by [`DataTransfer::fetch_plaintext`].
    pub const PLAINTEXT_MIME_TYPES: &[&str] = mime::PLAINTEXT;
    /// The set of MIME types recognized by [`DataTransfer::fetch_image`].
    pub const IMAGE_MIME_TYPES: &[&str] = mime::IMAGE;

    /// Set the application-internal data represented by this [`DataTransfer`].
    /// This can be read with [`DataTransfer::user_data`], and allows circumventing
    /// serialize/deserializing the data to bytes when a drag-and-drop or copy-paste
    /// operation stays within the application.
    pub fn set_user_data(&mut self, value: Rc<dyn Any>) -> &mut Self {
        self.user_data = Some(value);
        self
    }

    /// Set a lazily-evaluated provider for a given MIME type, returning a byte vector
    /// that can be transferred between applications.
    pub fn set_provider(
        &mut self,
        mime_type: SharedString,
        provider: impl FnOnce() -> Result<SharedVector<u8>, PlatformError> + 'static,
    ) -> &mut Self {
        self.providers.push((mime_type, Rc::new(LazyCell::new(Box::new(provider)))));
        self
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

    /// Helper to read this [`DataTransfer`] as plaintext, supporting multiple encodings.
    ///
    /// The caller should assume that this method call may do IO.
    pub fn fetch_plaintext(&self) -> Result<SharedString, PlatformError> {
        if let Some(internal_str) =
            self.user_data().and_then(|any| any.downcast_ref::<SharedString>().cloned())
        {
            return Ok(internal_str);
        }

        // This only handles UTF-8 text for now, so we can ignore the actual MIME type,
        // but on Windows this should handle WTF-16 to UTF-8 conversion.
        self.find_type(mime::PLAINTEXT).and_then(|(_, text)| {
            Ok(String::from_utf8(text.to_vec())
                .map_err(|e| PlatformError::OtherError(Box::new(e)))?
                .into())
        })
    }

    /// Helper to read this [`DataTransfer`] as an image, supporting multiple image types.
    ///
    /// The caller should assume that this method call may do IO.
    pub fn fetch_image(&self) -> Result<Image, PlatformError> {
        if let Some(internal_image) =
            self.user_data().and_then(|any| any.downcast_ref::<Image>().cloned())
        {
            return Ok(internal_image);
        }

        #[cfg(feature = "image-decoders")]
        {
            let available_image_types =
                if cfg!(feature = "svg") { mime::IMAGE } else { mime::PIXMAP_IMAGE };

            // This only handles UTF-8 text for now, so we can ignore the actual MIME type,
            // but on Windows this should handle WTF-16 to UTF-8 conversion.
            self.find_type(available_image_types).and_then(|(type_, image_data)| {
                let image_ext = match type_ {
                    mime::image::BMP => "bmp",
                    mime::image::GIF => "gif",
                    mime::image::JPEG => "jpeg",
                    mime::image::PNG => "png",
                    mime::image::SVG => "svg",
                    _ => "",
                };

                Image::load_from_dynamic_data(&image_data, image_ext)
                    .map_err(|err| PlatformError::Other(alloc::format!("{err}")))
            })
        }

        #[cfg(not(feature = "image-decoders"))]
        {
            Err(PlatformError::DataTransferTypeNotFound("image".into()))
        }
    }

    /// Fetch the binary representation of this [`DataTransfer`] as the specified MIME
    /// type.
    ///
    /// The caller should assume that this method call may do IO.
    pub fn fetch(&self, mime_type: &str) -> Result<SharedVector<u8>, PlatformError> {
        self.providers
            .iter()
            .find_map(|(type_, value)| (type_ == mime_type).then(|| clone_platform_result(value)))
            .unwrap_or_else(|| Err(PlatformError::DataTransferTypeNotFound(mime_type.into())))
    }

    /// Fetch the binary representation of this [`DataTransfer`] as one of the specified
    /// MIME types, with the preference order specified by this type (the order of `mime_types`
    /// is not taken into account).
    ///
    /// The caller should assume that this method call may do IO.
    fn find_type(&self, mime_types: &[&str]) -> Result<(&str, SharedVector<u8>), PlatformError> {
        // TODO: Should we prefer to go in the order specified in the data transfer or in the
        // `mime_types` argument? Only X11 and Wayland have a proper concept of source-defined
        // type ordering, so maybe it makes more sense for the destination to define the
        // preference order.
        self.providers
            .iter()
            .find_map(|(type_, value)| {
                mime_types
                    .contains(&&**type_)
                    .then(|| clone_platform_result(value).map(|val| (&**type_, val)))
            })
            .unwrap_or_else(|| {
                if let Some(last) = mime_types.last().copied() {
                    Err(PlatformError::DataTransferTypeNotFound(last.into()))
                } else {
                    Err(PlatformError::Unsupported)
                }
            })
    }
}
