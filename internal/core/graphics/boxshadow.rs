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
    /// The source paint and geometry for non-inset shadows.
    pub source: Option<(crate::Brush, crate::item_rendering::BorderRectLayout)>,
    /// Scale used to resolve absolute gradient coordinates.
    pub scale_factor: ScaleFactor,
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

impl BoxShadowOptions {
    /// Returns the painted outer radii when the source background is opaque.
    pub fn opaque_source_radius(&self) -> Option<PhysicalBorderRadius> {
        let (background, layout) = self.source.as_ref()?;
        background.is_opaque().then_some(layout.outer_radius)
    }

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
        let source = if inset {
            None
        } else {
            let mut layout = crate::item_rendering::BorderRectLayout::new(
                box_shadow,
                geometry.size,
                scale_factor,
            )?;
            let spread = (box_shadow.spread().cast() * scale_factor).get();
            if spread != 0. {
                // Fill under the border before spreading it. An opaque border normally
                // lets us inset the fill, but shrinking both independently opens a gap.
                layout.background_rect =
                    euclid::Rect::from_size(layout.brush_size).inflate(spread, spread);
                layout.background_radius = (layout.outer_radius
                    + PhysicalBorderRadius::new_uniform(spread))
                .max(Default::default());
            }
            if layout.border_width.get() > 0. {
                layout.border_width =
                    euclid::Length::new((layout.border_width.get() + 2. * spread).max(0.));
            }
            Some((box_shadow.background(), layout))
        };
        Some(Self {
            source,
            scale_factor,
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
