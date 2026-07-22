// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Types and helpers related to the [`DataTransfer`] type, which implements type-indexed arbitrary
//! data transfer both within an application and between applications.

use alloc::rc::Rc;
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
    /// Special-cased support for plain text, as the precise implementation of transferring
    /// text differs between platforms.
    plain_text: Option<SharedString>,
}

/// `DataTransfer` abstracts over the various ways of transferring data within an application
/// and between applications.
///
/// The details will depend on the current platform, but the common features are:
///
/// - Each `DataTransfer` contains multiple views over the same data in different formats
/// - The `DataTransfer` may contain an in-memory representation of the data, which can be
///   sent and received within the current application
/// - Serializing to/deserializing from a given format may be done eagerly or lazily[^lazy-note]
///
/// [^lazy-note]: Platforms differ on which formats can and cannot be lazy, but all support it in
/// some capacity. Reading data from a `DataTransfer` cannot be assumed to be a cheap operation.
///
/// Currently, only plain text and image data is supported. Precisely how this maps to the
/// backend will depend on platform and features. Work to expand this API is ongoing, see
/// [the tracking issue for drag-and-drop][dnd-tracking-issue] to follow its progress.
///
/// [dnd-tracking-issue]: https://github.com/slint-ui/slint/issues/1967
///
/// The easiest way to construct this type is with the [`Default`] implementation, followed
/// by [`set_plain_text`](DataTransfer::set_plain_text) or [`set_image`](DataTransfer::set_image).
/// There are also implementations of [`From<SharedString>`](SharedString) and [`From<Image>`](Image)
/// which construct a new `DataTransfer` using those methods respectively. The opposites of these
/// operations are [`plain_text`](DataTransfer::plain_text) and
/// [`image`](DataTransfer::image).
///
/// ```rust
/// # use i_slint_core::{DataTransfer, string::ToSharedString as _};
///
/// let message = "Hello, world!";
/// let data = DataTransfer::from(message.to_shared_string());
/// assert_eq!(data.plain_text().unwrap(), message);
/// ```
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
            .field("has_plain_text", &self.has_plain_text())
            .field("has_image", &self.has_image())
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

        out.set_plain_text(value);

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
    TypeNotFound,
}

impl core::error::Error for DataTransferError {}

impl core::fmt::Display for DataTransferError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TypeNotFound => {
                write!(f, "Type not supplied by data transfer")
            }
        }
    }
}

impl DataTransfer {
    /// Sets an image to be transferred by this [`DataTransfer`].
    ///
    /// The image can be read using [`image`](DataTransfer::image). If
    /// you only need the [`DataTransfer`] to have an image representation, use
    /// [`From<Image>`](Image).
    ///
    /// Each [`DataTransfer`] can only have a single image set at once. If this
    /// method is called multiple times, the previous image will be overwritten.
    /// However, you can have, for example, both an image representation and a
    /// plain text representation set simultaneously on the same [`DataTransfer`].
    ///
    /// Passing a default-constructed `Image` clears the previously-set image
    /// instead of storing it, so afterwards [`has_image`](DataTransfer::has_image)
    /// returns `false`. If the resulting `DataTransfer` carries no plain text,
    /// image, or user data, it compares equal to [`DataTransfer::default()`].
    pub fn set_image(&mut self, image: Image) -> &mut Self {
        if matches!(image.0, crate::ImageInner::None) {
            self.clear_image();
        } else {
            Rc::make_mut(self.inner.get_or_insert_default()).image = Some(image);
        }
        self
    }

    /// Sets unstyled, basic text to be transferred by this [`DataTransfer`].
    ///
    /// The image can be read using [`plain_text`](DataTransfer::plain_text).
    /// If you only need the [`DataTransfer`] to have a plain text representation,
    /// use [`From<SharedString>`](SharedString).
    ///
    /// Each [`DataTransfer`] can only have a single plain text representation
    /// set at once. If this method is called multiple times, the previous text
    /// will be overwritten. However, you can have, for example, both an image
    /// representation and a plain text representation set simultaneously on the
    /// same [`DataTransfer`].
    ///
    /// Passing an empty string clears the previously-set plain text instead of
    /// storing it, so afterwards [`has_plain_text`](DataTransfer::has_plain_text)
    /// returns `false`. If the resulting `DataTransfer` carries no plain text,
    /// image, or user data, it compares equal to [`DataTransfer::default()`].
    pub fn set_plain_text(&mut self, plain_text: SharedString) -> &mut Self {
        if plain_text.is_empty() {
            self.clear_plain_text();
        } else {
            Rc::make_mut(self.inner.get_or_insert_default()).plain_text = Some(plain_text);
        }
        self
    }

    fn clear_image(&mut self) {
        let Some(inner_rc) = self.inner.as_mut() else { return };
        if inner_rc.image.is_some() {
            Rc::make_mut(inner_rc).image = None;
        }
        if inner_rc.image.is_none() && inner_rc.plain_text.is_none() {
            self.inner = None;
        }
    }

    fn clear_plain_text(&mut self) {
        let Some(inner_rc) = self.inner.as_mut() else { return };
        if inner_rc.plain_text.is_some() {
            Rc::make_mut(inner_rc).plain_text = None;
        }
        if inner_rc.image.is_none() && inner_rc.plain_text.is_none() {
            self.inner = None;
        }
    }

    /// Returns `true` if this data transfer advertises that it is readable as an [`Image`].
    ///
    /// This does not necessarily mean that `image` will return `Ok`, as an I/O error
    /// may occur.
    pub fn has_image(&self) -> bool {
        self.inner.as_ref().is_some_and(|inner| inner.image.is_some())
    }

    /// Returns `true` if this data transfer advertises that it is readable as plain text.
    ///
    /// This does not necessarily mean that `plain_text` will return `Ok`, as an I/O
    /// error may occur.
    pub fn has_plain_text(&self) -> bool {
        self.inner.as_ref().is_some_and(|inner| inner.plain_text.is_some())
    }

    /// Returns `true` if this data transfer carries no data: no plain text, no image,
    /// and no user data. A `DataTransfer` constructed via [`Default::default`] is empty.
    pub fn is_empty(&self) -> bool {
        !self.has_plain_text() && !self.has_image() && self.user_data.is_none()
    }

    /// Set the application-internal data represented by this [`DataTransfer`].
    /// This can be read with [`DataTransfer::user_data`], and allows circumventing
    /// serialize/deserializing the data to bytes when a drag-and-drop or copy-paste
    /// operation stays within the application.
    pub fn set_user_data(&mut self, value: Rc<dyn Any>) -> &mut Self {
        self.user_data = Some(value);
        self
    }

    /// Helper to read this [`DataTransfer`] as plain text, supporting multiple encodings.
    ///
    /// The caller should assume that this method call may do I/O.
    pub fn plain_text(&self) -> Result<SharedString, DataTransferError> {
        self.inner
            .as_ref()
            .and_then(|inner| inner.plain_text.clone())
            .ok_or(DataTransferError::TypeNotFound)
    }

    /// Helper to read this [`DataTransfer`] as an image, supporting multiple image types.
    ///
    /// The caller should assume that this method call may do I/O.
    pub fn image(&self) -> Result<Image, DataTransferError> {
        self.inner
            .as_ref()
            .and_then(|inner| inner.image.clone())
            .ok_or(DataTransferError::TypeNotFound)
    }

    /// Get the application-internal data represented by this [`DataTransfer`], if
    /// one exists.
    pub fn user_data(&self) -> Option<Rc<dyn Any>> {
        self.user_data.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::{Rgba8Pixel, SharedPixelBuffer};

    #[test]
    fn set_plain_text_with_empty_string_clears() {
        let mut dt = DataTransfer::default();
        dt.set_plain_text("hello".into());
        assert!(dt.has_plain_text());
        dt.set_plain_text("".into());
        assert!(!dt.has_plain_text());
        assert!(dt.plain_text().is_err());
    }

    #[test]
    fn set_image_with_default_image_clears() {
        let mut dt = DataTransfer::default();
        let buffer = SharedPixelBuffer::<Rgba8Pixel>::new(2, 2);
        dt.set_image(Image::from_rgba8(buffer));
        assert!(dt.has_image());
        dt.set_image(Image::default());
        assert!(!dt.has_image());
        assert!(dt.image().is_err());
    }

    #[test]
    fn set_plain_text_with_empty_string_on_default_stays_empty() {
        let mut dt = DataTransfer::default();
        dt.set_plain_text("".into());
        assert!(!dt.has_plain_text());
        assert!(dt.is_empty());
        // The clear path on an already-default transfer must not allocate an
        // inner Rc — otherwise it would compare unequal to `default()` even
        // though both are observably empty.
        assert!(dt.inner.is_none());
        assert_eq!(dt, DataTransfer::default());
    }

    #[test]
    fn set_image_with_default_image_on_default_stays_empty() {
        let mut dt = DataTransfer::default();
        dt.set_image(Image::default());
        assert!(!dt.has_image());
        assert!(dt.is_empty());
        assert!(dt.inner.is_none());
        assert_eq!(dt, DataTransfer::default());
    }

    #[test]
    fn cleared_transfer_compares_equal_to_default() {
        // After clearing every field, the inner Rc must be released so the
        // transfer compares equal to a freshly-constructed default.
        let mut dt = DataTransfer::default();
        dt.set_plain_text("hello".into());
        dt.set_image(Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::new(2, 2)));
        assert!(!dt.is_empty());
        dt.set_plain_text("".into());
        assert!(dt.inner.is_some(), "image still set, inner must remain");
        dt.set_image(Image::default());
        assert!(dt.is_empty());
        assert!(dt.inner.is_none());
        assert_eq!(dt, DataTransfer::default());
    }
}
