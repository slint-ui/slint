// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use i_slint_core::{
    graphics::{Image, SharedImageBuffer, SharedPixelBuffer},
    ImageInner,
};
use napi::bindgen_prelude::{Buffer, External};

#[napi(js_name = ImageData)]
pub struct JsImageData {
    inner: Image,
}

impl From<Image> for JsImageData {
    fn from(image: Image) -> Self {
        Self { inner: image }
    }
}

#[napi]
impl JsImageData {
    #[napi(constructor)]
    pub fn new(width: u32, height: u32) -> Self {
        Self { inner: Image::from_rgba8(SharedPixelBuffer::new(width, height)) }
    }

    // FIXME: constructor with buffer causes trouble
    #[napi(constructor)]
    pub fn from_data_array(data_array: Buffer, width: u32) -> Self {
        Self {
            inner: Image::from_rgba8(SharedPixelBuffer::clone_from_slice(
                data_array.as_ref(),
                width,
                width / data_array.len() as u32,
            )),
        }
    }

    #[napi(getter)]
    pub fn width(&self) -> u32 {
        self.inner.size().width
    }

    #[napi(getter)]
    pub fn height(&self) -> u32 {
        self.inner.size().height
    }

    #[napi(getter)]
    pub fn data(&self) -> Buffer {
        let image_inner: &ImageInner = (&self.inner).into();
        if let Some(buffer) = image_inner.render_to_buffer(None) {
            match buffer {
                SharedImageBuffer::RGB8(buffer) => return Buffer::from(buffer.as_bytes()),
                SharedImageBuffer::RGBA8(buffer) => return Buffer::from(buffer.as_bytes()),
                SharedImageBuffer::RGBA8Premultiplied(buffer) => {
                    return Buffer::from(buffer.as_bytes())
                }
            }
        }

        Buffer::from(vec![0; (self.width() * self.height() * 4) as usize])
    }

    #[napi(getter)]
    pub fn image(&self) -> External<Image> {
        External::new(self.inner.clone())
    }
}
