// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore unmultiply

//! Copies of embedded images at their destination size, unrotated, in RAM,
//! for a [`TargetPixelBuffer`](super::target_pixel_buffer::TargetPixelBuffer) to draw.

use super::draw_functions;
use super::target_pixel::PremultipliedRgbaColor;
use super::target_pixel_buffer::{
    DrawTextureArgs, TextureData, TextureDataContainer, TexturePixelFormat,
};
use super::{PhysicalLength, PhysicalRect, PhysicalSize, RenderingRotation, Texture};
use alloc::collections::BTreeMap;
use alloc::vec;
use i_slint_core::Color;
use i_slint_core::graphics::{Gray8Pixel, Rgba8Pixel, SharedPixelBuffer};

/// An image in RGBA, or an alpha map when it's colorized.
#[derive(Clone)]
pub enum PreparedTexture {
    Rgba(SharedPixelBuffer<Rgba8Pixel>),
    AlphaMap(SharedPixelBuffer<Gray8Pixel>),
}

impl PreparedTexture {
    /// Prepares the image of `texture`.
    /// Returns `None` for textures that aren't embedded images, or that are tiled.
    fn new(texture: &DrawTextureArgs) -> Option<Self> {
        let TextureDataContainer::Static(source) = &texture.data else { return None };
        if texture.tiling.is_some() {
            return None;
        }
        let (width, height) = (texture.dst_width, texture.dst_height);
        let colorized = texture.colorize.is_some();
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
        let (buffer_width, buffer_height) = (width as u32, height as u32);
        // In PSRAM on ESP-IDF, where the PPA can read it.
        #[cfg(any(target_os = "espidf", target_arch = "xtensa"))]
        let mut prepared = if colorized {
            Self::AlphaMap(SharedPixelBuffer::new_dma_reachable(buffer_width, buffer_height))
        } else {
            Self::Rgba(SharedPixelBuffer::new_dma_reachable(buffer_width, buffer_height))
        };
        #[cfg(not(any(target_os = "espidf", target_arch = "xtensa")))]
        let mut prepared = if colorized {
            Self::AlphaMap(SharedPixelBuffer::new(buffer_width, buffer_height))
        } else {
            Self::Rgba(SharedPixelBuffer::new(buffer_width, buffer_height))
        };
        let mut row = vec![PremultipliedRgbaColor::default(); width];
        for line in 0..height {
            row.fill(PremultipliedRgbaColor::default());
            draw_functions::draw_texture_line(
                &geometry,
                PhysicalLength::new(line as i16),
                &sampler,
                &mut row,
                0,
                0,
            );
            match &mut prepared {
                Self::AlphaMap(buffer) => {
                    let out = &mut buffer.make_mut_slice()[line * width..][..width];
                    for (pixel, color) in out.iter_mut().zip(&row) {
                        *pixel = Gray8Pixel(color.alpha);
                    }
                }
                Self::Rgba(buffer) => {
                    let out = &mut buffer.make_mut_slice()[line * width..][..width];
                    for (pixel, &PremultipliedRgbaColor { red, green, blue, alpha }) in
                        out.iter_mut().zip(&row)
                    {
                        let unmultiply =
                            |channel: u8| match alpha {
                                0 => 0,
                                _ => ((channel as u16 * 255 + alpha as u16 / 2) / alpha as u16)
                                    .min(255) as u8,
                            };
                        *pixel = Rgba8Pixel::new(
                            unmultiply(red),
                            unmultiply(green),
                            unmultiply(blue),
                            alpha,
                        );
                    }
                }
            }
        }
        Some(prepared)
    }

    /// The texture to draw in place of `texture`.
    pub fn at(&self, texture: &DrawTextureArgs) -> DrawTextureArgs {
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
        match self {
            Self::Rgba(buffer) => TextureData {
                data: buffer.as_bytes(),
                pixel_format: TexturePixelFormat::Rgba,
                byte_stride: buffer.width() as usize * TexturePixelFormat::Rgba.bpp(),
                width: buffer.width(),
                height: buffer.height(),
            },
            Self::AlphaMap(buffer) => TextureData {
                data: buffer.as_bytes(),
                pixel_format: TexturePixelFormat::AlphaMap,
                byte_stride: buffer.width() as usize * TexturePixelFormat::AlphaMap.bpp(),
                width: buffer.width(),
                height: buffer.height(),
            },
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    data: usize,
    source_size: (u32, u32),
    dst_size: (usize, usize),
    rotation: u16,
    colorized: bool,
}

/// Images prepared for a [`TargetPixelBuffer`](super::target_pixel_buffer::TargetPixelBuffer),
/// kept from frame to frame by the thread that rasterizes.
#[derive(Default)]
pub struct PreparedTextures {
    textures: BTreeMap<Key, Entry>,
    bytes: usize,
    max_bytes: usize,
    /// Counts the lookups, to tell which image was used last.
    clock: u64,
}

struct Entry {
    texture: PreparedTexture,
    bytes: usize,
    last_used: u64,
}

#[cfg_attr(not(feature = "experimental"), allow(dead_code))]
impl PreparedTextures {
    /// Creates an empty cache that prepares no images, until [`Self::set_max_bytes`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets how many bytes the prepared images may take.
    /// When the next image doesn't fit, the cache drops the images used longest ago.
    /// Images larger than this aren't prepared, so 0 turns preparing off.
    pub fn set_max_bytes(&mut self, max_bytes: usize) {
        self.max_bytes = max_bytes;
        self.drop_until_fits(0);
    }

    /// Drops the image used longest ago, until `extra` more bytes fit.
    fn drop_until_fits(&mut self, extra: usize) {
        while self.bytes + extra > self.max_bytes {
            let Some(oldest) = self
                .textures
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            else {
                return;
            };
            if let Some(entry) = self.textures.remove(&oldest) {
                self.bytes -= entry.bytes;
            }
        }
    }

    /// Returns the prepared image of `texture`, preparing it the first time.
    pub(crate) fn get(&mut self, texture: &DrawTextureArgs) -> Option<PreparedTexture> {
        let TextureDataContainer::Static(source) = &texture.data else { return None };
        let key = Key {
            data: source.data.as_ptr() as usize,
            source_size: (source.width, source.height),
            dst_size: (texture.dst_width, texture.dst_height),
            rotation: texture.rotation.angle() as u16,
            colorized: texture.colorize.is_some(),
        };
        self.clock += 1;
        if let Some(entry) = self.textures.get_mut(&key) {
            entry.last_used = self.clock;
            return Some(entry.texture.clone());
        }
        let bytes = texture.dst_width * texture.dst_height * if key.colorized { 1 } else { 4 };
        if bytes > self.max_bytes {
            return None;
        }
        #[cfg(feature = "tracing")]
        let _span =
            tracing::info_span!("prepare_texture", w = texture.dst_width, h = texture.dst_height)
                .entered();
        let texture = PreparedTexture::new(texture)?;
        self.drop_until_fits(bytes);
        self.bytes += bytes;
        self.textures.insert(key, Entry { texture: texture.clone(), bytes, last_used: self.clock });
        Some(texture)
    }
}
