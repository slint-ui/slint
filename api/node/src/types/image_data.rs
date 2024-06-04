// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::vec;

use i_slint_core::{
    graphics::{Image, SharedImageBuffer, SharedPixelBuffer},
    ImageInner,
};
use napi::{
    bindgen_prelude::{Buffer, External},
    Env, JsUnknown,
};

// This is needed for typedoc check JsImageData::image
pub type ImageData = Image;

/// SlintPoint implements {@link ImageData}.
#[napi]
pub struct SlintImageData {
    inner: Image,
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
        let image_inner: &ImageInner = (&self.inner).into();
        if let Some(buffer) = image_inner.render_to_buffer(None) {
            match buffer {
                SharedImageBuffer::RGB8(buffer) => {
                    return Buffer::from(rgb_to_rgba(
                        buffer.as_bytes(),
                        (self.width() * self.height()) as usize,
                    ))
                }
                SharedImageBuffer::RGBA8(buffer) => return Buffer::from(buffer.as_bytes()),
                SharedImageBuffer::RGBA8Premultiplied(buffer) => {
                    return Buffer::from(rgb_to_rgba(
                        buffer.as_bytes(),
                        (self.width() * self.height()) as usize,
                    ))
                }
            }
        }

        Buffer::from(vec![0; (self.width() * self.height() * 4) as usize])
    }

    #[napi(getter)]
    pub fn path(&self, env: Env) -> napi::Result<JsUnknown> {
        self.inner.path().map_or_else(
            || env.get_undefined().map(|v| v.into_unknown()),
            |p| env.create_string(p.to_string_lossy().as_ref()).map(|v| v.into_unknown()),
        )
    }

    /// @hidden
    #[napi(getter)]
    pub fn image(&self) -> External<ImageData> {
        External::new(self.inner.clone())
    }
}

fn rgb_to_rgba(bytes: &[u8], size: usize) -> Vec<u8> {
    let mut rgba_bytes = vec![];

    for i in 0..size {
        if (i * 3) + 2 >= bytes.len() {
            continue;
        }

        rgba_bytes.push(bytes[i * 3]);
        rgba_bytes.push(bytes[(i * 3) + 1]);
        rgba_bytes.push(bytes[(i * 3) + 2]);
        rgba_bytes.push(255);
    }

    rgba_bytes
}
