// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This is the module contain data structures for a scene of items that can be rendered

use super::{
    Fixed, PhysicalBorderRadius, PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalRegion,
    PhysicalSize, PremultipliedRgbaColor, RenderingRotation,
};
use crate::graphics::{SharedImageBuffer, TexturePixelFormat};
use crate::lengths::{PointLengths as _, SizeLengths as _};
use crate::Color;
use alloc::rc::Rc;
use alloc::vec::Vec;
use euclid::Length;

#[derive(Default)]
pub struct SceneVectors {
    pub textures: Vec<SceneTexture<'static>>,
    pub rounded_rectangles: Vec<RoundedRectangle>,
    pub shared_buffers: Vec<SharedBufferCommand>,
    pub linear_gradients: Vec<LinearGradientCommand>,
    pub radial_gradients: Vec<RadialGradientCommand>,
    pub conic_gradients: Vec<ConicGradientCommand>,
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
        items.sort_unstable_by(compare_scene_item);
        let current_items_index = items.partition_point(|i| i.pos.y_length() <= current_line);
        items[..current_items_index].sort_unstable_by(|a, b| b.z.cmp(&a.z));
        let mut r = Self {
            items,
            current_line,
            current_items_index,
            future_items_index: current_items_index,
            vectors,
            dirty_region,
            current_line_ranges: Default::default(),
            range_valid_until_line: Default::default(),
        };
        r.recompute_ranges();
        debug_assert_eq!(r.current_line, r.dirty_region.bounding_rect().origin.y_length());
        r
    }

    /// Updates `current_items_index` and `future_items_index` to match the invariant
    pub fn next_line(&mut self) {
        self.current_line += PhysicalLength::new(1);

        let skipped = self.current_line >= self.range_valid_until_line && self.recompute_ranges();

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
                if item.pos.y_length() + item.size.height_length() < self.current_line {
                    continue;
                }
                self.items[i] = item;
                i += 1;
            }
            self.items[0..i].sort_unstable_by(|a, b| b.z.cmp(&a.z));
            self.current_items_index = i;
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
                    if future_next_z.map_or(true, |z| self.items[tmp1].z > z) {
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
                    if future_next_z.map_or(true, |z| item.z > z) {
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
                self.items[sort_begin..i].sort_unstable_by(|a, b| b.z.cmp(&a.z));
                break;
            }
            self.items[i] = item;
            i += 1;
        }
        self.current_items_index = i;
        // check that current items are properly sorted
        debug_assert!(self.items[0..self.current_items_index].windows(2).all(|x| x[0].z >= x[1].z));
    }

    // return true if lines were skipped
    fn recompute_ranges(&mut self) -> bool {
        let validity = super::region_line_ranges(
            &self.dirty_region,
            self.current_line.get(),
            &mut self.current_line_ranges,
        );
        if self.current_line_ranges.is_empty() {
            if let Some(next) = validity {
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
        }
        self.range_valid_until_line = Length::new(validity.unwrap_or_default());
        false
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SceneItem {
    pub pos: PhysicalPoint,
    pub size: PhysicalSize,
    // this is the order of the item from which it is in the item tree
    pub z: u16,
    pub command: SceneCommand,
}

fn compare_scene_item(a: &SceneItem, b: &SceneItem) -> core::cmp::Ordering {
    // First, order by line (top to bottom)
    match a.pos.y.partial_cmp(&b.pos.y) {
        None | Some(core::cmp::Ordering::Equal) => {}
        Some(ord) => return ord,
    }
    // Then by the reverse z (front to back)
    match a.z.partial_cmp(&b.z) {
        None | Some(core::cmp::Ordering::Equal) => {}
        Some(ord) => return ord.reverse(),
    }

    // anything else, we don't care
    core::cmp::Ordering::Equal
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
    pub dx: Fixed<u16, 8>,
    pub dy: Fixed<u16, 8>,
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
        let geometry: PhysicalRect = euclid::rect(
            texture.dst_x as i16,
            texture.dst_y as i16,
            texture.dst_width as i16,
            texture.dst_height as i16,
        );
        let geometry = geometry.to_box2d();
        let clipped_geometry = geometry.intersection(&clip.to_box2d())?;

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
            tiling.gap_x;
            tiling.gap_y;

            (Fixed::from_f32(tiling.scale_x)?, Fixed::from_f32(tiling.scale_y)?)
        } else {
            let (dst_w, dst_h) = if texture.rotation.is_transpose() {
                (texture.dst_height as i32, texture.dst_width as i32)
            } else {
                (texture.dst_width as i32, texture.dst_height as i32)
            };
            let dx = Fixed::<i32, 8>::from_fraction(source_size.width, dst_w);
            let dy = Fixed::<i32, 8>::from_fraction(source_size.height, dst_h);
            (dx, dy)
        };

        Some((
            Self {
                colorize: texture.colorize.unwrap_or_default(),
                alpha: texture.alpha,
                rotation: texture.rotation,
                dx: Fixed::try_from_fixed(dx).ok()?,
                dy: Fixed::try_from_fixed(dy).ok()?,
                off_x: Fixed::try_from_fixed(dx * offset.x as i32).ok()?,
                off_y: Fixed::try_from_fixed(dy * offset.y as i32).ok()?,
            },
            clipped_geometry.to_rect(),
        ))
    }
}

#[derive(Clone)]
pub enum SharedBufferData {
    SharedImage(SharedImageBuffer),
    AlphaMap { data: Rc<[u8]>, width: u16 },
}

impl SharedBufferData {
    pub fn width(&self) -> usize {
        match self {
            SharedBufferData::SharedImage(image) => image.width() as usize,
            SharedBufferData::AlphaMap { width, .. } => *width as usize,
        }
    }
    #[allow(unused)]
    pub fn height(&self) -> usize {
        match self {
            SharedBufferData::SharedImage(image) => image.height() as usize,
            SharedBufferData::AlphaMap { data, width, .. } => data.len() / *width as usize,
        }
    }
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
            SharedBufferData::AlphaMap { data, width } => SceneTexture {
                data: &data[start..end],
                pixel_stride: *width,
                format: TexturePixelFormat::AlphaMap,
                extra: self.extra,
            },
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

#[derive(Debug)]
pub struct RoundedRectangle {
    pub radius: PhysicalBorderRadius,
    /// the border's width
    pub width: PhysicalLength,
    pub border_color: PremultipliedRgbaColor,
    pub inner_color: PremultipliedRgbaColor,
    /// The clips is the amount of pixels of the rounded rectangle that is clipped away.
    /// For example, if left_clip > width, then the left border will not be visible, and
    /// if left_clip > radius, then no radius will be seen in the left side
    pub left_clip: PhysicalLength,
    pub right_clip: PhysicalLength,
    pub top_clip: PhysicalLength,
    pub bottom_clip: PhysicalLength,
}

/// Goes from color 1 to color2
///
/// depending of `flags & 0b1`
///  - if false: on the left side, goes from `start` to 1, on the right side, goes from 0 to `1-start`
///  - if true: on the left side, goes from 0 to `1-start`, on the right side, goes from `start` to `1`
#[derive(Debug)]
pub struct LinearGradientCommand {
    pub color1: PremultipliedRgbaColor,
    pub color2: PremultipliedRgbaColor,
    pub start: u8,
    /// bit 0: if the slope is positive or negative
    /// bit 1: if we should fill with color1 on the left side when left_clip is negative (or transparent)
    /// bit 2: if we should fill with color2 on the left side when right_clip is negative (or transparent)
    pub flags: u8,
    /// If positive, the clip has the same meaning as in RoundedRectangle.
    /// If negative, that means the "stop" is only starting or stopping at that point
    pub left_clip: PhysicalLength,
    pub right_clip: PhysicalLength,
    pub top_clip: PhysicalLength,
    pub bottom_clip: PhysicalLength,
}

/// Radial gradient that interpolates colors from the center outward
///
/// Unlike LinearGradientCommand, radial gradients don't have clipping fields
/// because they radiate uniformly in all directions from the center point.
/// The gradient is naturally clipped by the rectangle bounds during rendering.
#[derive(Debug)]
pub struct RadialGradientCommand {
    /// The gradient stops (colors and positions)
    pub stops: crate::SharedVector<crate::graphics::GradientStop>,
    /// Center of the gradient relative to the item position
    pub center_x: PhysicalLength,
    pub center_y: PhysicalLength,
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
    pub stops: crate::SharedVector<crate::graphics::GradientStop>,
}
