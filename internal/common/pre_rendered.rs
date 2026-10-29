// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Images pre-rendered for the software renderer.
//!
//! The same [`StaticTextures`] holds its data in statics when generated code embeds it,
//! or in `Vec`s when the compiler builds it, see [`Storage`].

use crate::bitmapfont::{BitmapFont, OwnedStorage, Storage};

/// 2D Rectangle with integer coordinates
pub type IntRect = euclid::default::Rect<i32>;
/// 2D Size in integer coordinates
pub type IntSize = euclid::default::Size2D<u32>;

#[repr(u8)]
#[derive(Clone, PartialEq, Debug, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The pixel format used for textures.
pub enum TexturePixelFormat {
    /// red, green, blue. 24bits.
    Rgb,
    /// Red, green, blue, alpha. 32bits.
    Rgba,
    /// Red, green, blue, alpha. 32bits. The color are premultiplied by alpha
    RgbaPremultiplied,
    /// Alpha map. 8bits. Each pixel is an alpha value. The color is specified separately.
    AlphaMap,
    /// Distance field. 8bit interpreted as i8.
    /// The range is such that i8::MIN corresponds to 3 pixels outside of the shape,
    /// and i8::MAX corresponds to 3 pixels inside the shape.
    /// The array must be width * height +1 bytes long. (the extra bit is read but never used)
    SignedDistanceField,
    /// Grayscale. 8bits. Each pixel is a luminance value rendered as (v, v, v, 255).
    ///
    /// This variant is only available with the `image-pixel-format-gray8` feature.
    #[cfg(feature = "image-pixel-format-gray8")]
    Gray8,
    /// 16 bits per pixel: 5 red bits, 6 green bits, and 5 blue bits, in native byte order.
    ///
    /// This variant is only available with the `image-pixel-format-rgb565` feature.
    #[cfg(feature = "image-pixel-format-rgb565")]
    Rgb565,
}

impl TexturePixelFormat {
    /// The number of bytes in a pixel
    pub fn bpp(self) -> usize {
        match self {
            TexturePixelFormat::Rgb => 3,
            TexturePixelFormat::Rgba => 4,
            TexturePixelFormat::RgbaPremultiplied => 4,
            TexturePixelFormat::AlphaMap => 1,
            TexturePixelFormat::SignedDistanceField => 1,
            #[cfg(feature = "image-pixel-format-gray8")]
            TexturePixelFormat::Gray8 => 1,
            #[cfg(feature = "image-pixel-format-rgb565")]
            TexturePixelFormat::Rgb565 => 2,
        }
    }
}

#[repr(C)]
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Some raw pixel data which is typically stored in the binary
pub struct StaticTexture {
    /// The position and size of the texture within the image
    pub rect: IntRect,
    /// The pixel format of this texture
    pub format: TexturePixelFormat,
    /// The color of the alpha map ones, as `0xAARRGGBB`
    pub color: u32,
    /// index in the data array
    pub index: usize,
}

/// A texture is stored in read-only memory and may be composed of sub-textures.
#[repr(C)]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StaticTextures<S: Storage> {
    /// The total size of the image (this might not be the size of the full image
    /// as some transparent part are not part of any texture)
    pub size: IntSize,
    /// The size of the image before the compiler applied any scaling
    pub original_size: IntSize,
    /// The pixel data referenced by the textures
    pub data: S::Slice<u8>,
    /// The list of textures
    pub textures: S::Slice<StaticTexture>,
}

/// A texture or a font that the compiler pre-rendered, as an interpreter takes it.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PreRenderedResource {
    Texture(StaticTextures<OwnedStorage>),
    BitmapFont(BitmapFont<OwnedStorage>),
}
