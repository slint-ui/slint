// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::vec;

use i_slint_core::graphics::{Image, SharedPixelBuffer};
use napi::bindgen_prelude::{Buffer, Either, External, Uint8Array, Undefined};

// This is needed for typedoc check JsImageData::image
pub type ImageData = Image;

/// SlintPoint implements {@link ImageData}.
#[napi]
pub struct SlintImageData {
    pub(crate) inner: Image,
}

impl From<Image> for SlintImageData {
    fn from(image: Image) -> Self {
        Self { inner: image }
    }
}

#[napi]
impl SlintImageData {
    /// Constructs a new image with the given width and height.
    /// Each pixel will set to red = 0, green = 0, blue = 0 and alpha = 0.
    #[napi(constructor)]
    pub fn new(width: u32, height: u32) -> Self {
        Self { inner: Image::from_rgba8(SharedPixelBuffer::new(width, height)) }
    }

    /// Loads the image from the file at `path`.
    ///
    /// @throws {Error} if the file can't be read or decoded.
    #[napi(factory)]
    pub fn load_from_path(path: String) -> napi::Result<Self> {
        Image::load_from_path(std::path::Path::new(&path))
            .map(Self::from)
            .map_err(|e| napi::Error::from_reason(format!("{path}: {e}")))
    }

    /// Decodes the image from `data`, the content of an image file.
    ///
    /// @throws {Error} if the data can't be decoded.
    #[napi(factory)]
    pub fn load_from_data(data: Uint8Array, format: Option<String>) -> napi::Result<Self> {
        Image::load_from_data(&data, format.as_deref())
            .map(Self::from)
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    /// Returns the width of the image in pixels.
    #[napi(getter)]
    pub fn width(&self) -> u32 {
        self.inner.size().width
    }

    /// Returns the height of the image in pixels.
    #[napi(getter)]
    pub fn height(&self) -> u32 {
        self.inner.size().height
    }

    /// Returns the image as buffer.
    /// A Buffer is a subclass of Uint8Array.
    #[napi(getter)]
    pub fn data(&self) -> Buffer {
        match self.inner.to_rgba8() {
            Some(buffer) => Buffer::from(buffer.as_bytes()),
            None => Buffer::from(vec![0; (self.width() * self.height() * 4) as usize]),
        }
    }

    #[napi(getter)]
    pub fn path(&self) -> Either<String, Undefined> {
        match self.inner.path() {
            None => Either::B(()),
            Some(p) => Either::A(p.to_string_lossy().into_owned()),
        }
    }

    /// @hidden
    #[napi(getter)]
    pub fn image(&self) -> External<ImageData> {
        External::new(self.inner.clone())
    }
}
