// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::vec;

use i_slint_core::graphics::{Image, SharedPixelBuffer};
use napi::{
    Env,
    bindgen_prelude::{Buffer, External, ToNapiValue, Unknown},
};

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
        Buffer::from(crate::shared::image_to_rgba8(&self.inner))
    }

    #[napi(getter)]
    pub fn path<'a>(&self, env: &'a Env) -> napi::Result<Unknown<'a>> {
        match self.inner.path() {
            None => ().into_unknown(env),
            Some(p) => env.create_string(p.to_string_lossy().as_ref())?.into_unknown(env),
        }
    }

    /// @hidden
    #[napi(getter)]
    pub fn image(&self) -> External<ImageData> {
        External::new(self.inner.clone())
    }
}
