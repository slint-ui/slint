// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains the renderer-independent geometry of a box shadow.
*/

use crate::items::ItemRc;
use crate::{
    Color, Coord,
    lengths::{
        LogicalLength, LogicalRect, LogicalVector, PhysicalBorderRadius, PhysicalPx, RectLengths,
        ScaleFactor,
    },
};

/// Struct to store options affecting the rendering of a box shadow
#[derive(Clone, PartialEq, Debug, Default)]
pub struct BoxShadowOptions {
    /// The width of the element in physical pixels.
    pub width: euclid::Length<f32, PhysicalPx>,
    /// The height of the element in physical pixels.
    pub height: euclid::Length<f32, PhysicalPx>,
    /// The color for the box shadow.
    pub color: Color,
    /// The blur to apply to the box shadow in pixels.
    pub blur: euclid::Length<f32, PhysicalPx>,
    /// The radii of the box shadow.
    pub radius: PhysicalBorderRadius,
    /// The spread radius in physical pixels. Positive grows the shadow shape, negative shrinks it.
    pub spread: euclid::Length<f32, PhysicalPx>,
    /// Whether the shadow is rendered inside the element's geometry.
    pub inset: bool,
    /// Horizontal offset in physical pixels.
    /// Only set for inset shadows: renderers apply a drop shadow's offset when drawing it.
    pub offset_x_inset: f32,
    /// Vertical offset in physical pixels. Only used by inset shadows.
    pub offset_y_inset: f32,
}

impl Eq for BoxShadowOptions {}
impl Ord for BoxShadowOptions {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let lhs = (
            self.width,
            self.height,
            self.color,
            self.blur,
            self.radius.top_left.to_bits(),
            self.radius.top_right.to_bits(),
            self.radius.bottom_right.to_bits(),
            self.radius.bottom_left.to_bits(),
            self.spread,
            self.inset,
            self.offset_x_inset.to_bits(),
            self.offset_y_inset.to_bits(),
        );
        let rhs = (
            other.width,
            other.height,
            other.color,
            other.blur,
            other.radius.top_left.to_bits(),
            other.radius.top_right.to_bits(),
            other.radius.bottom_right.to_bits(),
            other.radius.bottom_left.to_bits(),
            other.spread,
            other.inset,
            other.offset_x_inset.to_bits(),
            other.offset_y_inset.to_bits(),
        );
        if rhs < lhs {
            core::cmp::Ordering::Less
        } else if lhs < rhs {
            core::cmp::Ordering::Greater
        } else {
            core::cmp::Ordering::Equal
        }
    }
}

impl PartialOrd for BoxShadowOptions {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The geometry of a box shadow, derived from [`BoxShadowOptions`].
///
/// The CSS rules the renderers agree on: the shadow shape is the element's geometry
/// grown by the spread, and its corner radii grow with it.
impl BoxShadowOptions {
    /// The size of the shadow shape: the element's size grown by the spread on each
    /// side. A negative spread shrinks it, down to nothing.
    pub fn shape_size(&self) -> euclid::Size2D<f32, PhysicalPx> {
        euclid::size2(
            (self.width.get() + 2. * self.spread.get()).max(0.),
            (self.height.get() + 2. * self.spread.get()).max(0.),
        )
    }

    /// The size of the texture a drop shadow is rendered into: the shape padded by the
    /// blur on each side.
    pub fn drop_texture_size(&self) -> euclid::Size2D<f32, PhysicalPx> {
        self.shape_size() + euclid::size2(2. * self.blur.get(), 2. * self.blur.get())
    }

    /// Where the shape sits within the drop shadow texture, i.e. the blur padding.
    pub fn shape_origin(&self) -> euclid::Point2D<f32, PhysicalPx> {
        euclid::point2(self.blur.get(), self.blur.get())
    }

    /// The corner radii of the shadow shape: `max(0, radius + spread)`.
    pub fn outer_radius(&self) -> PhysicalBorderRadius {
        (self.radius + PhysicalBorderRadius::new_uniform(self.spread.get())).max(Default::default())
    }

    /// The corner radii of the hole an inset shadow leaves: `max(0, radius - spread)`.
    pub fn inner_radius(&self) -> PhysicalBorderRadius {
        (self.radius - PhysicalBorderRadius::new_uniform(self.spread.get())).max(Default::default())
    }

    /// The Gaussian sigma corresponding to the CSS blur radius.
    pub fn blur_sigma(&self) -> f32 {
        self.blur.get() / 2.
    }

    /// Extracts the rendering specific properties from the BoxShadow item and scales the logical
    /// coordinates to physical pixels used in the BoxShadowOptions. Returns None if for example the
    /// alpha on the box shadow would imply that no shadow is to be rendered.
    pub fn new(
        item_rc: &ItemRc,
        box_shadow: core::pin::Pin<&crate::items::BoxShadow>,
        scale_factor: ScaleFactor,
    ) -> Option<Self> {
        let color = box_shadow.color();
        if color.alpha() == 0 {
            return None;
        }
        let geometry = item_rc.geometry();
        let width = geometry.width_length().cast() * scale_factor;
        let height = geometry.height_length().cast() * scale_factor;
        if width.get() < 1. || height.get() < 1. {
            return None;
        }
        let inset = box_shadow.inset();
        let (offset_x_inset, offset_y_inset) = if inset {
            (
                (box_shadow.offset_x().cast() * scale_factor).get(),
                (box_shadow.offset_y().cast() * scale_factor).get(),
            )
        } else {
            (0., 0.)
        };
        Some(Self {
            width,
            height,
            color,
            blur: box_shadow.blur().cast() * scale_factor, // This effectively becomes the blur radius, so scale to physical pixels
            radius: box_shadow.logical_border_radius().cast() * scale_factor,
            spread: box_shadow.spread().cast() * scale_factor,
            inset,
            offset_x_inset,
            offset_y_inset,
        })
    }
}

/// The area a drop shadow paints, in the coordinates of the element's `geometry`.
pub fn drop_shadow_bounding_rect(
    geometry: LogicalRect,
    offset: LogicalVector,
    blur: LogicalLength,
    spread: LogicalLength,
) -> LogicalRect {
    let pad = blur + LogicalLength::new(spread.get().max(0 as Coord));
    geometry.outer_rect(euclid::SideOffsets2D::from_length_all_same(pad)).translate(offset)
}
