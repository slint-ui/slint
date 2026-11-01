// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This is the module contain data structures for a scene of items that can be rendered

use super::box_shadow::BoxShadowCommand;
use super::{
    Fixed, PhysicalBorderRadius, PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalRegion,
    PhysicalSize, PremultipliedRgbaColor, RenderingRotation,
};
use alloc::vec::Vec;
use euclid::Length;
use i_slint_core::Color;
#[cfg(feature = "owned-pre-rendered-resources")]
use i_slint_core::graphics::{OwnedStorage, StaticTextures};
use i_slint_core::graphics::{SharedImageBuffer, StaticTexture, TexturePixelFormat};
use i_slint_core::lengths::{PhysicalPx, PointLengths as _, SizeLengths as _};

#[derive(Default)]
pub struct SceneVectors {
    pub textures: Vec<SceneTexture<'static>>,
    pub rounded_rectangles: Vec<RoundedRectangle>,
    pub shared_buffers: Vec<SharedBufferCommand>,
    pub linear_gradients: Vec<LinearGradientCommand>,
    pub radial_gradients: Vec<RadialGradientCommand>,
    pub conic_gradients: Vec<ConicGradientCommand>,
    pub box_shadows: Vec<BoxShadowCommand>,
    pub clips: Vec<ShapeClip>,
}

pub struct Scene {
    /// the next line to be processed
    pub(super) current_line: PhysicalLength,

    /// The items are sorted like so:
    /// - `items[future_items_index..]` are the items that have `y > current_line`.
    ///   They must be sorted by `y` (top to bottom), then by `z` (front to back)
    /// - `items[..current_items_index]` are the items that overlap with the current_line,
    ///   sorted by z (front to back)
    pub(super) items: Vec<SceneItem>,

    pub(super) vectors: SceneVectors,

    pub(super) future_items_index: usize,
    pub(super) current_items_index: usize,

    pub(super) dirty_region: PhysicalRegion,

    pub(super) current_line_ranges: Vec<core::ops::Range<i16>>,
    pub(super) range_valid_until_line: PhysicalLength,

    items_valid_until_line: PhysicalLength,
}

impl Scene {
    pub fn new(
        mut items: Vec<SceneItem>,
        vectors: SceneVectors,
        dirty_region: PhysicalRegion,
    ) -> Self {
        let current_line =
            dirty_region.iter_box().map(|x| x.min.y_length()).min().unwrap_or_default();
        items.retain(|i| i.pos.y_length() + i.size.height_length() > current_line);
        sort_items(&mut items, scene_item_key);
        let current_items_index = items.partition_point(|i| i.pos.y_length() <= current_line);
        sort_items(&mut items[..current_items_index], z_desc_key);
        let mut r = Self {
            items,
            current_line,
            current_items_index,
            future_items_index: current_items_index,
            vectors,
            dirty_region,
            current_line_ranges: Default::default(),
            range_valid_until_line: Default::default(),
            items_valid_until_line: Default::default(),
        };
        r.recompute_ranges();
        r.update_items_valid_until_line();
        debug_assert_eq!(r.current_line, r.dirty_region.bounding_rect().origin.y_length());
        r
    }

    /// Updates `current_items_index` and `future_items_index` to match the invariant
    pub fn next_line(&mut self) {
        self.current_line += PhysicalLength::new(1);

        let skipped = self.current_line >= self.range_valid_until_line && self.recompute_ranges();
        if !skipped && self.current_line < self.items_valid_until_line {
            return;
        }

        // The items array is split in part:
        // 1. [0..i] are the items that have already been processed, that are on this line
        // 2. [j..current_items_index] are the items from the previous line that might still be
        //   valid on this line
        // 3. [tmp1, tmp2] is a buffer where we swap items so we can make room for the items in [0..i]
        // 4. [future_items_index..] are the items which might get processed now
        // 5. [current_items_index..tmp1], [tmp2..future_items_index] and [i..j] is garbage
        //
        // At each step, we selecting the item with the higher z from the list 2 or 3 or 4 and take it from
        // that list. Then we add it to the list [0..i] if it needs more processing. If needed,
        // we move the first  item from list  2. to list 3. to make some room

        let (mut i, mut j, mut tmp1, mut tmp2) =
            (0, 0, self.current_items_index, self.current_items_index);

        if skipped {
            // Merge sort doesn't work in that case.
            while j < self.current_items_index {
                let item = self.items[j];
                if item.pos.y_length() + item.size.height_length() > self.current_line {
                    self.items[i] = item;
                    i += 1;
                }
                j += 1;
            }
            while self.future_items_index < self.items.len() {
                let item = self.items[self.future_items_index];
                if item.pos.y_length() > self.current_line {
                    break;
                }
                self.future_items_index += 1;
                if item.pos.y_length() + item.size.height_length() <= self.current_line {
                    continue;
                }
                self.items[i] = item;
                i += 1;
            }
            sort_items(&mut self.items[0..i], z_desc_key);
            self.current_items_index = i;
            self.update_items_valid_until_line();
            return;
        }

        'outer: loop {
            let future_next_z = self
                .items
                .get(self.future_items_index)
                .filter(|i| i.pos.y_length() <= self.current_line)
                .map(|i| i.z);
            let item = loop {
                if tmp1 != tmp2 {
                    if future_next_z.is_none_or(|z| self.items[tmp1].z > z) {
                        let idx = tmp1;
                        tmp1 += 1;
                        if tmp1 == tmp2 {
                            tmp1 = self.current_items_index;
                            tmp2 = self.current_items_index;
                        }
                        break self.items[idx];
                    }
                } else if j < self.current_items_index {
                    let item = &self.items[j];
                    if item.pos.y_length() + item.size.height_length() <= self.current_line {
                        j += 1;
                        continue;
                    }
                    if future_next_z.is_none_or(|z| item.z > z) {
                        j += 1;
                        break *item;
                    }
                }
                if future_next_z.is_some() {
                    self.future_items_index += 1;
                    break self.items[self.future_items_index - 1];
                }
                break 'outer;
            };
            if i != j {
                // there is room
            } else if j >= self.current_items_index && tmp1 == tmp2 {
                // the current_items list is empty
                j += 1
            } else if self.items[j].pos.y_length() + self.items[j].size.height_length()
                <= self.current_line
            {
                // next item in the current_items array is no longer in this line
                j += 1;
            } else if tmp2 < self.future_items_index && j < self.current_items_index {
                // move the next item in current_items
                let to_move = self.items[j];
                self.items[tmp2] = to_move;
                j += 1;
                tmp2 += 1;
            } else {
                debug_assert!(tmp1 >= self.current_items_index);
                let sort_begin = i;
                // merge sort doesn't work because we don't have enough tmp space, just bring all items and use a normal sort.
                while j < self.current_items_index {
                    let item = self.items[j];
                    if item.pos.y_length() + item.size.height_length() > self.current_line {
                        self.items[i] = item;
                        i += 1;
                    }
                    j += 1;
                }
                self.items.copy_within(tmp1..tmp2, i);
                i += tmp2 - tmp1;
                debug_assert!(i < self.future_items_index);
                self.items[i] = item;
                i += 1;
                while self.future_items_index < self.items.len() {
                    let item = self.items[self.future_items_index];
                    if item.pos.y_length() > self.current_line {
                        break;
                    }
                    self.future_items_index += 1;
                    self.items[i] = item;
                    i += 1;
                }
                sort_items(&mut self.items[sort_begin..i], z_desc_key);
                break;
            }
            self.items[i] = item;
            i += 1;
        }
        self.current_items_index = i;
        // check that current items are properly sorted
        debug_assert!(
            self.items[0..self.current_items_index].array_windows().all(|[a, b]| a.z >= b.z)
        );
        self.update_items_valid_until_line();
    }

    fn update_items_valid_until_line(&mut self) {
        let next_start = self.items.get(self.future_items_index).map(|i| i.pos.y_length());
        let next_end = self.items[..self.current_items_index]
            .iter()
            .map(|i| i.pos.y_length() + i.size.height_length())
            .min();
        self.items_valid_until_line =
            next_start.into_iter().chain(next_end).min().unwrap_or(PhysicalLength::new(i16::MAX));
    }

    // return true if lines were skipped
    fn recompute_ranges(&mut self) -> bool {
        let validity = super::region_line_ranges(
            &self.dirty_region,
            self.current_line.get(),
            &mut self.current_line_ranges,
        );
        if self.current_line_ranges.is_empty()
            && let Some(next) = validity
        {
            self.current_line = Length::new(next);
            self.range_valid_until_line = Length::new(
                super::region_line_ranges(
                    &self.dirty_region,
                    self.current_line.get(),
                    &mut self.current_line_ranges,
                )
                .unwrap_or_default(),
            );
            return true;
        }
        self.range_valid_until_line = Length::new(validity.unwrap_or_default());
        false
    }

    /// Whether drawing `item` overwrites every pixel it spans on the current line.
    pub fn covers_line(&self, item: &SceneItem) -> bool {
        if item.clip_len != 0 {
            return false;
        }
        match &item.command {
            SceneCommand::Rectangle { color } => color.alpha == 255,
            SceneCommand::Texture { texture_index } => {
                let texture = &self.vectors.textures[*texture_index as usize];
                // Below alpha 255 the texture blends rather than overwrites (see
                // draw_texture_line), so it is not opaque.
                let opaque_format = match texture.format {
                    TexturePixelFormat::Rgb => true,
                    #[cfg(feature = "image-pixel-format-rgb565")]
                    TexturePixelFormat::Rgb565 => true,
                    #[cfg(feature = "image-pixel-format-gray8")]
                    TexturePixelFormat::Gray8 => true,
                    _ => false,
                };
                opaque_format && texture.extra.alpha == 255
            }
            SceneCommand::SharedBuffer { shared_buffer_index } => {
                let buffer = &self.vectors.shared_buffers[*shared_buffer_index as usize];
                let opaque_format = match buffer.buffer {
                    SharedBufferData::SharedImage(SharedImageBuffer::RGB8(_)) => true,
                    #[cfg(feature = "image-pixel-format-rgb565")]
                    SharedBufferData::SharedImage(SharedImageBuffer::RGB565(_)) => true,
                    #[cfg(feature = "image-pixel-format-gray8")]
                    SharedBufferData::SharedImage(SharedImageBuffer::Gray8(_)) => true,
                    SharedBufferData::SharedImage(
                        SharedImageBuffer::RGBA8(_) | SharedImageBuffer::RGBA8Premultiplied(_),
                    ) => false,
                    _ => false,
                };
                opaque_format && buffer.extra.alpha == 255
            }
            SceneCommand::LinearGradient { linear_gradient_index } => {
                self.vectors.linear_gradients[*linear_gradient_index as usize].covers_line()
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SceneItem {
    pub pos: PhysicalPoint,
    pub size: PhysicalSize,
    // this is the order of the item from which it is in the item tree
    pub z: u16,
    pub command: SceneCommand,
    /// Index in [`SceneVectors::clips`] of this item's clip run.
    /// The item is drawn only inside every shape of the run:
    /// the rounded `Clip` ancestors whose corners it reaches, and a gradient's own shape.
    pub clip_start: u16,
    /// Number of clips in the run; 0 means the item isn't clipped.
    pub clip_len: u16,
}

impl SceneItem {
    pub fn clips<'a>(&self, vectors: &'a SceneVectors) -> &'a [ShapeClip] {
        &vectors.clips[self.clip_start as usize..][..self.clip_len as usize]
    }
}

/// Orders by `y` (top to bottom), then by `z` (front to back).
fn scene_item_key(item: &SceneItem) -> u32 {
    // Flipping the sign bit maps i16 order onto u16 order.
    let y = (item.pos.y as u16) ^ 0x8000;
    (y as u32) << 16 | !item.z as u32
}

/// Orders by `z` (front to back).
fn z_desc_key(item: &SceneItem) -> u32 {
    !item.z as u32
}

/// Takes the key as a function pointer, so all sorts share one `sort_unstable` instantiation,
/// which costs several KB of code.
#[inline(never)]
fn sort_items(items: &mut [SceneItem], key: fn(&SceneItem) -> u32) {
    items.sort_unstable_by_key(key);
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum SceneCommand {
    Rectangle {
        color: PremultipliedRgbaColor,
    },
    /// texture_index is an index in the [`SceneVectors::textures`] array
    Texture {
        texture_index: u16,
    },
    /// shared_buffer_index is an index in [`SceneVectors::shared_buffers`]
    SharedBuffer {
        shared_buffer_index: u16,
    },
    /// rectangle_index is an index in the [`SceneVectors::rounded_rectangle`] array
    RoundedRectangle {
        rectangle_index: u16,
    },
    /// linear_gradient_index is an index in the [`SceneVectors::linear_gradients`] array
    LinearGradient {
        linear_gradient_index: u16,
    },
    /// radial_gradient_index is an index in the [`SceneVectors::radial_gradients`] array
    RadialGradient {
        radial_gradient_index: u16,
    },
    /// conic_gradient_index is an index in the [`SceneVectors::conic_gradients`] array
    ConicGradient {
        conic_gradient_index: u16,
    },
    /// box_shadow_index is an index in the [`SceneVectors::box_shadows`] array
    BoxShadow {
        box_shadow_index: u16,
    },
}

pub struct SceneTexture<'a> {
    /// This should have a size so that the entire slice is ((height - 1) * pixel_stride + width) * bpp
    pub data: &'a [u8],
    pub format: TexturePixelFormat,
    /// number of pixels between two lines in the source
    pub pixel_stride: u16,

    pub extra: SceneTextureExtra,
}

impl<'a> SceneTexture<'a> {
    pub fn source_size(&self) -> PhysicalSize {
        let mut len = self.data.len();
        if self.format == TexturePixelFormat::SignedDistanceField {
            len -= 1;
        } else {
            len /= self.format.bpp();
        }
        let stride = self.pixel_stride as usize;
        let h = len / stride;
        let w = len % stride;
        if w == 0 {
            PhysicalSize::new(stride as _, h as _)
        } else {
            PhysicalSize::new(w as _, (h + 1) as _)
        }
    }

    pub fn from_target_texture(
        texture: &'a super::target_pixel_buffer::DrawTextureArgs,
        clip: &PhysicalRect,
    ) -> Option<(Self, PhysicalRect)> {
        let (extra, geometry) = SceneTextureExtra::from_target_texture(texture, clip)?;
        let source = texture.source();
        Some((
            Self {
                data: source.data,
                pixel_stride: (source.byte_stride / source.pixel_format.bpp()) as u16,
                format: source.pixel_format,
                extra,
            },
            geometry,
        ))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SceneTextureExtra {
    /// Delta x: the amount of "image pixel" that we need to skip for each physical pixel in the target buffer
    pub dx: Fixed<i64, 32>,
    pub dy: Fixed<i64, 32>,
    /// Offset which is the coordinate of the "image pixel" which going to be drawn at location SceneItem::pos
    pub off_x: Fixed<u16, 4>,
    pub off_y: Fixed<u16, 4>,
    /// Color to colorize. When not transparent, consider that the image is an alpha map and always use that color.
    /// The alpha of this color is ignored. (it is supposed to be mixed in `Self::alpha`)
    pub colorize: Color,
    pub alpha: u8,
    pub rotation: RenderingRotation,
}

impl SceneTextureExtra {
    pub fn from_target_texture(
        texture: &super::target_pixel_buffer::DrawTextureArgs,
        clip: &PhysicalRect,
    ) -> Option<(Self, PhysicalRect)> {
        let geometry: euclid::Rect<i32, PhysicalPx> = euclid::rect(
            texture.dst_x as i32,
            texture.dst_y as i32,
            texture.dst_width as i32,
            texture.dst_height as i32,
        );
        let geometry = geometry.to_box2d();
        let clipped_geometry = geometry.intersection(&clip.cast::<i32>().to_box2d())?;

        let mut offset = match texture.rotation {
            RenderingRotation::NoRotation => clipped_geometry.min - geometry.min,
            RenderingRotation::Rotate90 => euclid::vec2(
                clipped_geometry.min.y - geometry.min.y,
                geometry.max.x - clipped_geometry.max.x,
            ),
            RenderingRotation::Rotate180 => geometry.max - clipped_geometry.max,
            RenderingRotation::Rotate270 => euclid::vec2(
                geometry.max.y - clipped_geometry.max.y,
                clipped_geometry.min.x - geometry.min.x,
            ),
        };

        let source_size = texture.source_size().cast::<i32>();
        let (dx, dy) = if let Some(tiling) = &texture.tiling {
            offset -= euclid::vec2(tiling.offset_x, tiling.offset_y).cast();

            // FIXME: gap
            let _ = tiling.gap_x;
            let _ = tiling.gap_y;

            let dx = Fixed::<i32, 8>::from_f32(tiling.scale_x)?;
            let dy = Fixed::<i32, 8>::from_f32(tiling.scale_y)?;
            (Fixed::<i64, 32>::from_fixed(dx), Fixed::<i64, 32>::from_fixed(dy))
        } else {
            let (dst_w, dst_h) = if texture.rotation.is_transpose() {
                (texture.dst_height as i32, texture.dst_width as i32)
            } else {
                (texture.dst_width as i32, texture.dst_height as i32)
            };
            let source_step = |source_size: i32, dst_size: i32| {
                let q8_step = Fixed::<i32, 8>::from_fraction(source_size, dst_size).max(Fixed(1));
                let last_source_index = (q8_step * (dst_size - 1)).truncate();
                if source_size < dst_size
                    && (last_source_index < source_size - 1 || last_source_index >= source_size)
                {
                    // 32 fractional bits keep the final sample inside the source for u16 destination sizes.
                    Fixed::<i64, 32>::from_fraction(source_size as i64, dst_size as i64)
                        .max(Fixed(1))
                } else {
                    Fixed::<i64, 32>::from_fixed(q8_step)
                }
            };
            (source_step(source_size.width, dst_w), source_step(source_size.height, dst_h))
        };
        Fixed::<u16, 8>::try_from_fixed(dx).ok()?;
        Fixed::<u16, 8>::try_from_fixed(dy).ok()?;

        Some((
            Self {
                colorize: texture.colorize.unwrap_or_default(),
                alpha: texture.alpha,
                rotation: texture.rotation,
                dx,
                dy,
                off_x: Fixed::try_from_fixed(dx * offset.x as i64).ok()?,
                off_y: Fixed::try_from_fixed(dy * offset.y as i64).ok()?,
            },
            clipped_geometry.to_rect().cast(),
        ))
    }
}

#[derive(Clone)]
pub enum SharedBufferData {
    SharedImage(SharedImageBuffer),
    AlphaMap {
        data: crate::fonts::GlyphAlphaMap,
        width: u16,
        sdf: bool,
    },
    #[cfg(feature = "owned-pre-rendered-resources")]
    Texture {
        textures: alloc::rc::Rc<StaticTextures<OwnedStorage>>,
        texture: StaticTexture,
    },
}

impl SharedBufferData {
    pub fn width(&self) -> usize {
        match self {
            SharedBufferData::SharedImage(image) => image.width() as usize,
            SharedBufferData::AlphaMap { width, .. } => *width as usize,
            #[cfg(feature = "owned-pre-rendered-resources")]
            SharedBufferData::Texture { texture, .. } => texture.rect.width() as usize,
        }
    }
    pub fn height(&self) -> usize {
        match self {
            SharedBufferData::SharedImage(image) => image.height() as usize,
            SharedBufferData::AlphaMap { data, width, .. } => {
                data.as_slice().len() / *width as usize
            }
            #[cfg(feature = "owned-pre-rendered-resources")]
            SharedBufferData::Texture { texture, .. } => texture.rect.height() as usize,
        }
    }
}

/// The pixels in `range` of `texture`, out of the `data` of its textures.
pub fn texture_pixels<'a>(
    data: &'a [u8],
    texture: &StaticTexture,
    range: core::ops::Range<usize>,
) -> &'a [u8] {
    let bpp = texture.format.bpp();
    &data[texture.index..][range.start * bpp..range.end * bpp]
}

pub struct SharedBufferCommand {
    pub buffer: SharedBufferData,
    /// The source rectangle that is mapped into this command span
    pub source_rect: PhysicalRect,
    pub extra: SceneTextureExtra,
}

impl SharedBufferCommand {
    pub fn as_texture(&self) -> SceneTexture<'_> {
        let stride = self.buffer.width();
        let core::ops::Range { start, end } = compute_range_in_buffer(&self.source_rect, stride);

        match &self.buffer {
            SharedBufferData::SharedImage(SharedImageBuffer::RGB8(b)) => SceneTexture {
                data: &b.as_bytes()[start * 3..end * 3],
                pixel_stride: stride as u16,
                format: TexturePixelFormat::Rgb,
                extra: self.extra,
            },
            SharedBufferData::SharedImage(SharedImageBuffer::RGBA8(b)) => SceneTexture {
                data: &b.as_bytes()[start * 4..end * 4],
                pixel_stride: stride as u16,
                format: TexturePixelFormat::Rgba,
                extra: self.extra,
            },
            SharedBufferData::SharedImage(SharedImageBuffer::RGBA8Premultiplied(b)) => {
                SceneTexture {
                    data: &b.as_bytes()[start * 4..end * 4],
                    pixel_stride: stride as u16,
                    format: TexturePixelFormat::RgbaPremultiplied,
                    extra: self.extra,
                }
            }
            #[cfg(feature = "image-pixel-format-rgb565")]
            SharedBufferData::SharedImage(SharedImageBuffer::RGB565(b)) => SceneTexture {
                data: &b.as_bytes()[start * 2..end * 2],
                pixel_stride: stride as u16,
                format: TexturePixelFormat::Rgb565,
                extra: self.extra,
            },
            #[cfg(feature = "image-pixel-format-gray8")]
            SharedBufferData::SharedImage(SharedImageBuffer::Gray8(b)) => SceneTexture {
                data: &b.as_bytes()[start..end],
                pixel_stride: stride as u16,
                format: TexturePixelFormat::Gray8,
                extra: self.extra,
            },
            SharedBufferData::AlphaMap { data, width, sdf } => SceneTexture {
                data: &data.as_slice()[start..end],
                pixel_stride: *width,
                format: if *sdf {
                    TexturePixelFormat::SignedDistanceField
                } else {
                    TexturePixelFormat::AlphaMap
                },
                extra: self.extra,
            },
            #[cfg(feature = "owned-pre-rendered-resources")]
            SharedBufferData::Texture { textures, texture } => SceneTexture {
                data: texture_pixels(&textures.data, texture, start..end),
                pixel_stride: stride as u16,
                format: texture.format,
                extra: self.extra,
            },
            SharedBufferData::SharedImage(_) => unreachable!(
                "pixel format without the matching image-pixel-format feature on i-slint-renderer-software"
            ),
        }
    }
}

/// Given a rectangle of coordinate in a buffer and a stride, compute the range, in pixel
pub fn compute_range_in_buffer(
    source_rect: &PhysicalRect,
    pixel_stride: usize,
) -> core::ops::Range<usize> {
    let start = pixel_stride * source_rect.min_y() as usize + source_rect.min_x() as usize;
    let end = pixel_stride * (source_rect.max_y() - 1) as usize + source_rect.max_x() as usize;
    start..end
}

/// The outline of a rounded rectangle relative to the rectangle a command draws in.
#[derive(Debug, Default, Clone, Copy)]
pub struct RoundedShape {
    pub radius: PhysicalBorderRadius,
    /// The clips is the amount of pixels of the rounded rectangle that is clipped away.
    /// For example, if left_clip > border width, then the left border will not be visible, and
    /// if left_clip > radius, then no radius will be seen in the left side
    pub left_clip: PhysicalLength,
    pub right_clip: PhysicalLength,
    pub top_clip: PhysicalLength,
    pub bottom_clip: PhysicalLength,
}

/// A rounded shape that clips a command.
#[derive(Debug, Default, Clone, Copy)]
pub struct ShapeClip {
    pub shape: RoundedShape,
    /// The width of an opaque border drawn over the command, or 0.
    /// When set, the command is clipped without anti-aliasing to the border's inner edge.
    /// This keeps it from bleeding through the border's anti-aliased outer edge.
    pub opaque_border: PhysicalLength,
}

impl ShapeClip {
    /// Whether a command of `size` reaches into one of the shape's rounded corners.
    /// Outside of them, the clip has no effect.
    pub fn touches_corner(&self, size: PhysicalSize) -> bool {
        let s = &self.shape;
        let left = s.left_clip.get() as i32;
        let top = s.top_clip.get() as i32;
        let right = left + size.width as i32;
        let bottom = top + size.height as i32;
        let width = right + s.right_clip.get() as i32;
        let height = bottom + s.bottom_clip.get() as i32;
        let touches = |r: i16, x: i32, y: i32| {
            let r = r as i32;
            r > 0 && x < right && x + r > left && y < bottom && y + r > top
        };
        let r = &s.radius;
        touches(r.top_left, 0, 0)
            || touches(r.top_right, width - r.top_right as i32, 0)
            || touches(
                r.bottom_right,
                width - r.bottom_right as i32,
                height - r.bottom_right as i32,
            )
            || touches(r.bottom_left, 0, height - r.bottom_left as i32)
    }
}

#[derive(Debug)]
pub struct RoundedRectangle {
    pub shape: RoundedShape,
    /// the border's width
    pub width: PhysicalLength,
    pub border_color: PremultipliedRgbaColor,
    pub inner_color: PremultipliedRgbaColor,
}

pub enum AnyGradientCommand {
    Linear(LinearGradientCommand),
    Radial(RadialGradientCommand),
    Conic(ConicGradientCommand),
}

/// A linear gradient, as the gradient position at each pixel center.
///
/// Positions are along the gradient line: 0 at its start, 1 at its end.
#[derive(Debug)]
pub struct LinearGradientCommand {
    pub stops: i_slint_core::SharedVector<LinearGradientStop>,
    /// The position at the center of the rect's top-left pixel.
    pub position: LinearGradientPosition,
    /// The position's change per pixel to the right.
    pub dx: LinearGradientPosition,
    /// The position's change per line down.
    pub dy: LinearGradientPosition,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LinearGradientStop {
    pub color: PremultipliedRgbaColor,
    /// See [`LinearGradientCommand`].
    pub position: LinearGradientPosition,
    /// The next stop's color minus this one's, per channel (red, green, blue, alpha).
    pub delta: [i32; 4],
    /// The interpolation factor's change per Q16 position towards the next stop, in Q31,
    /// rounded down. 0 when the next stop isn't after this one.
    pub factor_step: u32,
    /// The change of each Q15 channel per pixel to the right, rounded towards zero.
    pub step: [i32; 4],
}

impl LinearGradientStop {
    /// Keeps the distance between two positions within an i32.
    const MAX_POSITION: f32 = 63.;

    pub fn collect(
        stops: impl Iterator<Item = (PremultipliedRgbaColor, f32)>,
        dx: LinearGradientPosition,
    ) -> i_slint_core::SharedVector<Self> {
        let mut previous = Fixed(i32::MIN);
        let mut stops: i_slint_core::SharedVector<Self> = stops
            .map(|(color, position)| {
                let position =
                    Fixed::from_f32(position.clamp(-Self::MAX_POSITION, Self::MAX_POSITION))
                        .unwrap_or_default();
                // As in CSS, a stop before the previous one is moved to it.
                previous = previous.max(position);
                Self { color, position: previous, ..Self::default() }
            })
            .collect();
        let out = stops.make_mut_slice();
        for i in 1..out.len() {
            let next = out[i];
            out[i - 1].link(&next, dx);
        }
        stops
    }

    pub fn channels(&self) -> [u8; 4] {
        let c = self.color;
        [c.red, c.green, c.blue, c.alpha]
    }

    fn link(&mut self, next: &Self, dx: LinearGradientPosition) {
        let distance = next.position - self.position;
        if distance.0 <= 0 {
            return;
        }
        let (a, b) = (self.channels(), next.channels());
        self.delta = core::array::from_fn(|i| b[i] as i32 - a[i] as i32);
        self.factor_step =
            (1u32 << 31).checked_div(GradientPosition::from_fixed(distance).0 as u32).unwrap_or(0);
        // The factor's change per pixel in Q22, made smaller by one so that rounding
        // can't step past the next stop's color. Clamped for a segment narrower than
        // a pixel, which never steps.
        let limit = (1 << 23) as f32;
        let factor_dx =
            (dx.0 as f32 / distance.0 as f32 * (1 << 22) as f32).clamp(-limit, limit) as i32;
        let factor_dx = factor_dx - factor_dx.signum();
        self.step = core::array::from_fn(|i| self.delta[i] * factor_dx / 128);
    }
}

pub(crate) type GradientPosition = Fixed<i32, 16>;
pub(crate) type LinearGradientPosition = Fixed<i32, 24>;
/// An interpolation factor between two stops, or its rate of change.
pub(crate) type GradientFactor = Fixed<u32, 12>;

#[derive(Clone, Copy, Debug)]
pub struct PremultipliedGradientStop {
    pub color: PremultipliedRgbaColor,
    pub position: GradientPosition,
    /// The factor's rate of change per unit of position, towards the next stop.
    /// 0 when the next stop isn't after this one.
    pub step: GradientFactor,
}

impl PremultipliedGradientStop {
    pub fn collect(
        stops: impl Iterator<Item = (PremultipliedRgbaColor, f32)>,
    ) -> i_slint_core::SharedVector<Self> {
        // Keeps the distance between any two stops within an i32.
        const MAX_POSITION: f32 = (1 << (30 - GradientPosition::SHIFT)) as f32;
        let mut stops = stops
            .map(|(color, position)| Self {
                color,
                position: GradientPosition::from_f32(position.clamp(-MAX_POSITION, MAX_POSITION))
                    .unwrap_or_default(),
                step: Fixed(0),
            })
            .peekable();
        core::iter::from_fn(|| {
            let mut stop = stops.next()?;
            if let Some(next) = stops.peek() {
                let distance = next.position - stop.position;
                if distance.0 > 0 {
                    stop.step = Fixed::from_fraction(
                        GradientPosition::from_integer(1).0 as u32,
                        distance.0 as u32,
                    );
                }
            }
            Some(stop)
        })
        .collect()
    }

    /// The interpolation factor towards the next stop at `position`,
    /// which is between this stop and the next.
    pub fn factor(&self, position: GradientPosition) -> GradientFactor {
        let offset = (position - self.position).0 as u32;
        let half = 1 << (GradientPosition::SHIFT - 1);
        Fixed((offset * self.step.0 + half) >> GradientPosition::SHIFT)
    }
}

/// Radial gradient that interpolates colors from the center outward
///
/// Unlike LinearGradientCommand, radial gradients don't have clipping fields
/// because they radiate uniformly in all directions from the center point.
/// The gradient is naturally clipped by the rectangle bounds during rendering.
#[derive(Debug)]
pub struct RadialGradientCommand {
    /// The gradient stops (colors and positions)
    pub stops: i_slint_core::SharedVector<PremultipliedGradientStop>,
    /// Center in physical pixels, relative to the clipped rect's top-left corner.
    /// Stored as f32 to avoid i16 saturation for off-bbox centers at high scale factors.
    pub center_x: f32,
    pub center_y: f32,
    /// Radii in physical pixels. Always resolved (positive) before command construction.
    pub radius_x: f32,
    pub radius_y: f32,
}

/// Conic gradient that interpolates colors around a center point
///
/// The gradient creates a color transition that rotates around the center of the
/// rectangle being drawn. The angle positions are specified in the gradient stops,
/// where 0 = 0 degrees (north) and 1 = 360 degrees. Colors are interpolated based
/// on the angle from north, going clockwise.
#[derive(Debug)]
pub struct ConicGradientCommand {
    /// The gradient stops (colors and normalized angle positions)
    /// Position 0 = 0 degrees (north), 1 = 360 degrees
    pub stops: i_slint_core::SharedVector<PremultipliedGradientStop>,
    /// Center in physical pixels, relative to the clipped rect's top-left corner.
    /// Stored as f32 to avoid i16 saturation for off-bbox centers at high scale factors.
    pub center_x: f32,
    pub center_y: f32,
    /// Clockwise rotation of the whole gradient, in radians.
    pub rotation: f32,
}

#[test]
fn next_line_skipping_a_gap_drops_items_ending_at_the_new_line() {
    // Two dirty boxes with a gap at lines 10..20, so `next_line` jumps from line 10 to 20.
    let mut dirty_region = PhysicalRegion {
        rectangles: [euclid::Box2D::zero(); super::PHYSICAL_REGION_MAX_SIZE],
        count: 2,
    };
    dirty_region.rectangles[0] = euclid::Box2D::new((0, 0).into(), (10, 10).into());
    dirty_region.rectangles[1] = euclid::Box2D::new((0, 20).into(), (10, 30).into());

    let item = |y, height, z| SceneItem {
        pos: PhysicalPoint::new(0, y),
        size: PhysicalSize::new(10, height),
        z,
        command: SceneCommand::Rectangle { color: PremultipliedRgbaColor::default() },
        clip_start: 0,
        clip_len: 0,
    };
    // Both start inside the gap: the first covers lines 15..20, the second lines 15..21.
    let mut scene =
        Scene::new(alloc::vec![item(15, 5, 1), item(15, 6, 2)], Default::default(), dirty_region);

    while scene.current_line.get() < 10 {
        scene.next_line();
    }
    assert_eq!(scene.current_line.get(), 20);

    let active: Vec<_> = scene.items[..scene.current_items_index].iter().map(|i| i.z).collect();
    assert_eq!(active, [2]);
}

#[test]
fn next_line_keeps_exactly_the_items_on_the_current_line() {
    let mut dirty_region = PhysicalRegion {
        rectangles: [euclid::Box2D::zero(); super::PHYSICAL_REGION_MAX_SIZE],
        count: 3,
    };
    dirty_region.rectangles[0] = euclid::Box2D::new((0, 0).into(), (100, 40).into());
    dirty_region.rectangles[1] = euclid::Box2D::new((0, 55).into(), (100, 80).into());
    dirty_region.rectangles[2] = euclid::Box2D::new((0, 90).into(), (100, 120).into());

    // Deterministic mix of tall and short items starting and ending at varied lines.
    // Heights start at 1 because next_line expects non-empty items.
    let mut seed = 0x2545_f491_u32;
    let mut next = |max: u32| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed % max) as i16
    };
    let items: Vec<SceneItem> = (0..200)
        .map(|z| SceneItem {
            pos: PhysicalPoint::new(0, next(120)),
            size: PhysicalSize::new(10, 1 + if z % 5 == 0 { next(120) } else { next(12) }),
            z,
            command: SceneCommand::Rectangle { color: PremultipliedRgbaColor::default() },
            clip_start: 0,
            clip_len: 0,
        })
        .collect();

    let mut scene = Scene::new(items.clone(), Default::default(), dirty_region);
    while scene.current_line.get() < 120 {
        let line = scene.current_line.get();
        let mut expected: Vec<_> = items
            .iter()
            .filter(|i| i.pos.y <= line && line < i.pos.y + i.size.height)
            .map(|i| i.z)
            .collect();
        expected.sort_unstable_by(|a, b| b.cmp(a));
        let active: Vec<_> = scene.items[..scene.current_items_index].iter().map(|i| i.z).collect();
        assert_eq!(active, expected, "line {line}");
        scene.next_line();
    }
}

#[test]
fn touches_corner() {
    let clip =
        |radius: PhysicalBorderRadius, left: i16, top: i16, right: i16, bottom: i16| ShapeClip {
            shape: RoundedShape {
                radius,
                left_clip: Length::new(left),
                top_clip: Length::new(top),
                right_clip: Length::new(right),
                bottom_clip: Length::new(bottom),
            },
            opaque_border: PhysicalLength::new(0),
        };
    let size = PhysicalSize::new(10, 10);
    let all = PhysicalBorderRadius::new_uniform(4);
    // The whole shape.
    assert!(clip(all, 0, 0, 0, 0).touches_corner(size));
    // Exactly next to the top-left corner box, and one pixel into it.
    let top_left = PhysicalBorderRadius::new(4, 0, 0, 0);
    assert!(!clip(top_left, 4, 0, 0, 0).touches_corner(PhysicalSize::new(6, 10)));
    assert!(clip(top_left, 3, 0, 0, 0).touches_corner(PhysicalSize::new(7, 10)));
    assert!(!clip(top_left, 0, 4, 0, 0).touches_corner(PhysicalSize::new(10, 6)));
    // Each other corner on its own.
    let top_right = PhysicalBorderRadius::new(0, 4, 0, 0);
    assert!(clip(top_right, 0, 0, 0, 0).touches_corner(size));
    assert!(!clip(top_right, 0, 0, 4, 0).touches_corner(PhysicalSize::new(6, 10)));
    let bottom_right = PhysicalBorderRadius::new(0, 0, 4, 0);
    assert!(!clip(bottom_right, 0, 0, 0, 4).touches_corner(PhysicalSize::new(10, 6)));
    assert!(clip(bottom_right, 0, 0, 0, 3).touches_corner(PhysicalSize::new(10, 7)));
    let bottom_left = PhysicalBorderRadius::new(0, 0, 0, 4);
    assert!(!clip(bottom_left, 4, 0, 0, 0).touches_corner(PhysicalSize::new(6, 10)));
    // A command in the middle touches no corner.
    assert!(!clip(all, 4, 4, 4, 4).touches_corner(PhysicalSize::new(2, 2)));
    // Zero radii have no corner at all.
    assert!(!clip(PhysicalBorderRadius::default(), 0, 0, 0, 0).touches_corner(size));
}
