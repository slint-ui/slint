// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Types and helpers related to the [`DataTransfer`] type, which implements type-indexed arbitrary
//! data transfer both within an application and between applications.

use alloc::rc::Rc;
use core::{
    any::Any,
    cell::RefCell,
    fmt::{self, Pointer},
};

use crate::{SharedString, api::Image};

#[cfg(feature = "ffi")]
pub mod ffi;

#[derive(Clone)]
enum FetcherState<T> {
    Loaded(T),
    Unloaded(Rc<dyn Fn() -> Option<T>>),
    // Separate "empty" state instead of `Self::Unloaded(|| None)` so we don't
    // need to actually call the getter to know if the data exists.
    Empty,
}

#[derive(Clone)]
struct Fetcher<T> {
    state: RefCell<FetcherState<T>>,
}

impl<T> Default for Fetcher<T> {
    fn default() -> Self {
        Self { state: FetcherState::Empty.into() }
    }
}

impl<T> PartialEq for Fetcher<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match (&*self.state.borrow(), &*other.state.borrow()) {
            (FetcherState::Loaded(self_value), FetcherState::Loaded(other_value)) => {
                self_value == other_value
            }
            (FetcherState::Unloaded(self_getter), FetcherState::Unloaded(other_getter)) => {
                core::ptr::addr_eq(&**self_getter, &**other_getter)
            }
            _ => false,
        }
    }
}

impl<T> fmt::Debug for Fetcher<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self.state.borrow() {
            FetcherState::Loaded(value) => value.fmt(f),
            FetcherState::Unloaded(_) => write!(f, "{{unloaded}}"),
            FetcherState::Empty => write!(f, "{{none}}"),
        }
    }
}

impl<T> Fetcher<T> {
    fn unloaded<F: Fn() -> Option<T> + 'static>(func: F) -> Self {
        Self { state: FetcherState::Unloaded(Rc::new(func)).into() }
    }

    fn loaded(value: T) -> Self {
        Self { state: FetcherState::Loaded(value).into() }
    }

    fn is_empty(&self) -> bool {
        matches!(&*self.state.borrow(), FetcherState::Empty)
    }
}

impl<T> Fetcher<T>
where
    T: Clone,
{
    fn get(&self) -> Option<T> {
        let state = self.state.borrow();
        match &*state {
            FetcherState::Loaded(val) => Some(val.clone()),
            FetcherState::Unloaded(func) => {
                // Short-circuiting here means that calling `get` again will re-run
                // the function. It's not yet clear if this is correct behaviour.
                let value = func()?;

                core::mem::drop(state);

                self.state.replace(FetcherState::Loaded(value));

                self.get()
            }
            FetcherState::Empty => None,
        }
    }
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
/// Currently, only plaintext and image data is supported. Precisely how this maps to the
/// backend will depend on platform and features. Work to expand this API is ongoing, see
/// [the tracking issue for drag-and-drop][dnd-tracking-issue] to follow its progress.
///
/// [dnd-tracking-issue]: https://github.com/slint-ui/slint/issues/1967
///
/// The easiest way to construct this type is with the [`Default`] implementation, followed
/// by [`set_plaintext`](DataTransfer::set_plaintext) or [`set_image`](DataTransfer::set_image).
/// There are also implementations of [`From<SharedString>`](SharedString) and [`From<Image>`](Image)
/// which construct a new `DataTransfer` using those methods respectively. The opposites of these
/// operations are [`fetch_plaintext`](DataTransfer::fetch_plaintext) and
/// [`fetch_image`](DataTransfer::fetch_image).
///
/// ```rust
/// # use i_slint_core::{DataTransfer, string::ToSharedString as _};
///
/// let message = "Hello, world!";
/// let data = DataTransfer::from(message.to_shared_string());
/// assert_eq!(data.fetch_plaintext().unwrap(), message);
/// ```
#[derive(Clone, Default)]
#[repr(C)]
pub struct DataTransfer {
    // TODO: Custom binary data providers with custom MIME types.
    /// Special-cased support for images, as the precise implementation of transferring
    /// images differs between platforms.
    image: Fetcher<Image>,
    /// Special-cased support for plaintext, as the precise implementation of transferring
    /// text differs between platforms.
    plaintext: Fetcher<SharedString>,
    /// A custom in-memory value. No MIME type-based dispatch is done here - if the user
    /// wants to store one of a set of possible values, they should store their own enum
    /// and handle the dispatch themselves.
    user_data: Option<Rc<dyn Any>>,
}

impl core::fmt::Debug for DataTransfer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DataTransfer")
            .field("has_plaintext", &self.has_plaintext())
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
        self.image == other.image
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

// =================
// Free functions for internal use, so we don't need to re-export them from the main Slint library.
// =================

/// Set a lazy getter for plaintext data
pub fn data_transfer_set_plaintext_getter<F: Fn() -> Option<SharedString> + 'static>(
    transfer: &mut DataTransfer,
    getter: F,
) {
    transfer.plaintext = Fetcher::unloaded(getter);
}

/// Set a lazy getter for image data
pub fn data_transfer_set_image_getter<F: Fn() -> Option<Image> + 'static>(
    transfer: &mut DataTransfer,
    getter: F,
) {
    transfer.image = Fetcher::unloaded(getter);
}

impl DataTransfer {
    /// Sets an image to be transferred by this [`DataTransfer`].
    ///
    /// The image can be read using [`fetch_image`](DataTransfer::fetch_image). If
    /// you only need the [`DataTransfer`] to have an image representation, use
    /// [`From<Image>`](Image).
    ///
    /// Each [`DataTransfer`] can only have a single image set at once. If this
    /// method is called multiple times, the previous image will be overwritten.
    /// However, you can have, for example, both an image representation and a
    /// plaintext representation set simultaneously on the same [`DataTransfer`].
    pub fn set_image(&mut self, image: Image) -> &mut Self {
        self.image = Fetcher::loaded(image);
        self
    }

    /// Sets unstyled, basic text to be transferred by this [`DataTransfer`].
    ///
    /// The image can be read using [`fetch_plaintext`](DataTransfer::fetch_plaintext).
    /// If you only need the [`DataTransfer`] to have a plaintext representation,
    /// use [`From<SharedString>`](SharedString).
    ///
    /// Each [`DataTransfer`] can only have a single plaintext representation
    /// set at once. If this method is called multiple times, the previous text
    /// will be overwritten. However, you can have, for example, both an image
    /// representation and a plaintext representation set simultaneously on the
    /// same [`DataTransfer`].
    pub fn set_plaintext(&mut self, plaintext: SharedString) -> &mut Self {
        self.plaintext = Fetcher::loaded(plaintext);
        self
    }

    /// Returns `true` if this data transfer advertises that it is readable as an [`Image`].
    ///
    /// This does not necessarily mean that `fetch_image` will return `Ok`, as an I/O error
    /// may occur.
    pub fn has_image(&self) -> bool {
        !self.image.is_empty()
    }

    /// Returns `true` if this data transfer advertises that it is readable as plaintext.
    ///
    /// This does not necessarily mean that `fetch_plaintext` will return `Ok`, as an I/O
    /// error may occur.
    pub fn has_plaintext(&self) -> bool {
        !self.plaintext.is_empty()
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
    /// The caller should assume that this method call may do I/O.
    pub fn fetch_plaintext(&self) -> Result<SharedString, DataTransferError> {
        self.plaintext.get().ok_or(DataTransferError::TypeNotFound)
    }

    /// Helper to read this [`DataTransfer`] as an image, supporting multiple image types.
    ///
    /// The caller should assume that this method call may do I/O.
    pub fn fetch_image(&self) -> Result<Image, DataTransferError> {
        self.image.get().ok_or(DataTransferError::TypeNotFound)
    }

    /// Get the application-internal data represented by this [`DataTransfer`], if
    /// one exists.
    pub fn user_data(&self) -> Option<Rc<dyn Any>> {
        self.user_data.clone()
    }
}
