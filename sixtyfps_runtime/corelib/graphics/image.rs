/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use crate::slice::Slice;
use crate::{SharedString, SharedVector};

/// A resource is a reference to binary data, for example images. They can be accessible on the file
/// system or embedded in the resulting binary. Or they might be URLs to a web server and a downloaded
/// is necessary before they can be used.
#[derive(Clone, PartialEq, Debug)]
#[repr(u8)]
pub enum ImageReference {
    /// A resource that does not represent any data.
    None,
    /// A resource that points to a file in the file system
    AbsoluteFilePath(SharedString),
    /// A resource that is embedded in the program and accessible via pointer
    /// The format is the same as in a file
    EmbeddedData(Slice<'static, u8>),
    /// Raw ARGB
    #[allow(missing_docs)]
    EmbeddedRgbaImage { width: u32, height: u32, data: SharedVector<u32> },
}

impl Default for ImageReference {
    fn default() -> Self {
        ImageReference::None
    }
}

/// Error generated if an image cannot be loaded for any reasons.
#[derive(Default, Debug, PartialEq)]
pub struct LoadImageError(());

/// An image type that can be displayed by the Image element
#[repr(transparent)]
#[derive(Default, Clone, Debug, PartialEq)]
// FIXME: the inner should be private
pub struct Image(pub ImageReference);

impl Image {
    /// Load an Image from a path to a file containing an image
    pub fn load_from_path(path: &std::path::Path) -> Result<Self, LoadImageError> {
        Ok(Image(ImageReference::AbsoluteFilePath(path.to_str().ok_or(LoadImageError(()))?.into())))
    }
    /*
    /// Create an image from argb pixels.
    ///
    /// Returns an error if the `data` does not have a size of `width * height`.
    pub fn load_from_argb(
        width: u32,
        height: u32,
        data: SharedVector<u32>,
    ) -> Result<Self, LoadImageError> {
        if data.len() != width as usize * height as usize {
            Err(LoadImageError(()))
        } else {
            Ok(Image(ImageReference::EmbeddedRgbaImage { width, height, data }))
        }
    }
    */
}
