// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Copies of embedded images at their destination size, unrotated, in RAM,
//! for a [`TargetPixelBuffer`](super::target_pixel_buffer::TargetPixelBuffer) to draw.

use super::draw_functions::{self, PremultipliedRgbaColor};
use super::target_pixel_buffer::{
    DrawTextureArgs, TextureData, TextureDataContainer, TexturePixelFormat,
};
use super::{PhysicalLength, PhysicalRect, PhysicalSize, RenderingRotation, Texture};
use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use i_slint_core::Color;

/// An image in RGBA, or an alpha map when it's colorized.
pub struct PreparedTexture {
    pixels: Vec<u8>,
    format: TexturePixelFormat,
    width: u32,
    height: u32,
}

impl PreparedTexture {
    /// Prepares the image of `texture`.
    /// Returns `None` for textures that aren't embedded images, or that are tiled.
    fn new(texture: &DrawTextureArgs, blue_first: bool) -> Option<Self> {
        let TextureDataContainer::Static(source) = &texture.data else { return None };
        if texture.tiling.is_some() {
            return None;
        }
        let (width, height) = (texture.dst_width, texture.dst_height);
        let colorized = texture.colorize.is_some();
        let format =
            if colorized { TexturePixelFormat::AlphaMap } else { TexturePixelFormat::Rgba };
        let size = PhysicalSize::new(i16::try_from(width).ok()?, i16::try_from(height).ok()?);
        let sampled = DrawTextureArgs {
            data: TextureDataContainer::Static(source.clone()),
            // Sampled as coverage, so that the colorize color stays in the texture.
            colorize: colorized.then_some(Color::from_rgb_u8(255, 255, 255)),
            alpha: u8::MAX,
            dst_x: 0,
            dst_y: 0,
            dst_width: width,
            dst_height: height,
            rotation: texture.rotation,
            tiling: None,
        };
        let geometry = PhysicalRect::from_size(size);
        let (sampler, _) = Texture::from_target_texture(&sampled, &geometry)?;
        let bpp = format.bpp();
        let mut pixels = vec![0; width * height * bpp];
        let mut row = vec![PremultipliedRgbaColor::default(); width];
        for (line, out) in pixels.chunks_exact_mut(width * bpp).enumerate() {
            row.fill(PremultipliedRgbaColor::default());
            draw_functions::draw_texture_line(
                &geometry,
                PhysicalLength::new(line as i16),
                &sampler,
                &mut row,
                0,
                0,
            );
            for (pixel, &PremultipliedRgbaColor { red, green, blue, alpha }) in
                out.chunks_exact_mut(bpp).zip(&row)
            {
                if colorized {
                    pixel[0] = alpha;
                } else {
                    let unmultiply = |channel: u8| match alpha {
                        0 => 0,
                        _ => ((channel as u16 * 255 + alpha as u16 / 2) / alpha as u16).min(255)
                            as u8,
                    };
                    let (first, third) = if blue_first { (blue, red) } else { (red, blue) };
                    pixel.copy_from_slice(&[
                        unmultiply(first),
                        unmultiply(green),
                        unmultiply(third),
                        alpha,
                    ]);
                }
            }
        }
        Some(Self { pixels, format, width: width as u32, height: height as u32 })
    }

    /// The texture to draw in place of `texture`.
    pub fn at(self: &Rc<Self>, texture: &DrawTextureArgs) -> DrawTextureArgs {
        DrawTextureArgs {
            data: TextureDataContainer::Prepared(self.clone()),
            colorize: texture.colorize,
            alpha: texture.alpha,
            dst_x: texture.dst_x,
            dst_y: texture.dst_y,
            dst_width: texture.dst_width,
            dst_height: texture.dst_height,
            rotation: RenderingRotation::NoRotation,
            tiling: None,
        }
    }

    pub fn source(&self) -> TextureData<'_> {
        TextureData {
            data: &self.pixels,
            pixel_format: self.format,
            byte_stride: self.width as usize * self.format.bpp(),
            width: self.width,
            height: self.height,
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    data: usize,
    source_size: (u32, u32),
    dst_size: (usize, usize),
    rotation: u16,
    colorized: bool,
}

/// Images prepared for a [`TargetPixelBuffer`](super::target_pixel_buffer::TargetPixelBuffer),
/// kept from frame to frame by the thread that rasterizes.
pub struct PreparedTextures {
    textures: BTreeMap<Key, Rc<PreparedTexture>>,
    blue_first: bool,
    bytes: usize,
    max_bytes: usize,
}

impl Default for PreparedTextures {
    fn default() -> Self {
        Self { textures: BTreeMap::new(), blue_first: false, bytes: 0, max_bytes: 8 << 20 }
    }
}

#[cfg_attr(not(feature = "experimental"), allow(dead_code))]
impl PreparedTextures {
    /// Creates an empty cache that holds up to 8 MiB of images.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets how many bytes the prepared images may take.
    /// When the next image doesn't fit, the cache drops all images.
    /// Images larger than this aren't prepared, so 0 turns preparing off.
    pub fn set_max_bytes(&mut self, max_bytes: usize) {
        self.max_bytes = max_bytes;
        if self.bytes > max_bytes {
            self.textures.clear();
            self.bytes = 0;
        }
    }

    /// Sets whether prepared RGBA images store their color channels blue first, as BGRA,
    /// for a buffer that reads them in that order. Their pixel format stays `Rgba`.
    pub fn set_blue_first(&mut self, blue_first: bool) {
        if self.blue_first != blue_first {
            self.blue_first = blue_first;
            self.textures.clear();
            self.bytes = 0;
        }
    }

    /// Returns the prepared image of `texture`, preparing it the first time.
    pub(crate) fn get(&mut self, texture: &DrawTextureArgs) -> Option<Rc<PreparedTexture>> {
        let TextureDataContainer::Static(source) = &texture.data else { return None };
        let key = Key {
            data: source.data.as_ptr() as usize,
            source_size: (source.width, source.height),
            dst_size: (texture.dst_width, texture.dst_height),
            rotation: texture.rotation.angle() as u16,
            colorized: texture.colorize.is_some(),
        };
        if let Some(prepared) = self.textures.get(&key) {
            return Some(prepared.clone());
        }
        let bytes = texture.dst_width * texture.dst_height * if key.colorized { 1 } else { 4 };
        if bytes > self.max_bytes {
            return None;
        }
        #[cfg(feature = "tracing")]
        let _span =
            tracing::info_span!("prepare_texture", w = texture.dst_width, h = texture.dst_height)
                .entered();
        let prepared = Rc::new(PreparedTexture::new(texture, self.blue_first)?);
        if self.bytes + bytes > self.max_bytes {
            self.textures.clear();
            self.bytes = 0;
        }
        self.bytes += bytes;
        self.textures.insert(key, prepared.clone());
        Some(prepared)
    }
}
