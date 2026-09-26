// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains a cache helper for caching box shadow textures.
*/

use alloc::boxed::Box;
use alloc::vec::Vec;
use std::cell::{Cell, RefCell};

use crate::items::ItemRc;
use crate::{
    Color,
    lengths::{PhysicalBorderRadius, PhysicalPx, RectLengths, ScaleFactor},
};

/// Struct to store options affecting the rendering of a box shadow
#[derive(Clone, PartialEq, Debug, Default)]
pub struct BoxShadowOptions {
    /// The source paint and geometry for non-inset shadows.
    pub source: Option<(crate::Brush, crate::item_rendering::BorderRectLayout)>,
    /// Scale used to resolve absolute gradient coordinates.
    pub scale_factor: ScaleFactor,
    /// The width of the box shadow texture in physical pixels.
    pub width: euclid::Length<f32, PhysicalPx>,
    /// The height of the box shadow texture in physical pixels.
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
    /// Horizontal offset in physical pixels. Only used by inset shadows (drop-shadow offset is
    /// applied at blit time and is not part of the cached image).
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
        box_shadow: std::pin::Pin<&crate::items::BoxShadow>,
        scale_factor: ScaleFactor,
    ) -> Option<Self> {
        let color = box_shadow.color();
        if color.alpha() == 0 {
            return None;
        }
        let geometry = item_rc.geometry();
        let width = geometry.width_length() * scale_factor;
        let height = geometry.height_length() * scale_factor;
        if width.get() < 1. || height.get() < 1. {
            return None;
        }
        let inset = box_shadow.inset();
        let (offset_x_inset, offset_y_inset) = if inset {
            (
                (box_shadow.offset_x() * scale_factor).get(),
                (box_shadow.offset_y() * scale_factor).get(),
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
            let spread = (box_shadow.spread() * scale_factor).get();
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
            blur: box_shadow.blur() * scale_factor, // This effectively becomes the blur radius, so scale to physical pixels
            radius: box_shadow.logical_border_radius() * scale_factor,
            spread: box_shadow.spread() * scale_factor,
            inset,
            offset_x_inset,
            offset_y_inset,
        })
    }
}

/// Upper bound on the number of shadow textures kept alive by a [`BoxShadowCache`].
const MAX_CACHED_SHADOWS: usize = 16;

struct CacheEntry<ImageType> {
    image: Option<ImageType>,
    /// Value of the cache's access counter when this entry was last returned, for LRU eviction.
    last_used: u64,
}

/// Cache to hold box textures for given box shadow options.
pub struct BoxShadowCache<ImageType> {
    entries: RefCell<Vec<(BoxShadowOptions, CacheEntry<ImageType>)>>,
    access_counter: Cell<u64>,
    /// Track if the window scale factor changes; used to clear the cache if necessary.
    window_scale_factor_tracker: core::pin::Pin<Box<crate::properties::PropertyTracker>>,
}

impl<ImageType> Default for BoxShadowCache<ImageType> {
    fn default() -> Self {
        Self {
            entries: Default::default(),
            access_counter: Default::default(),
            window_scale_factor_tracker: Box::pin(Default::default()),
        }
    }
}

impl<ImageType> BoxShadowCache<ImageType> {
    /// Removes all cached box shadow textures.
    pub fn clear(&self) {
        self.entries.borrow_mut().clear();
    }

    /// Clears the cache if the window's scale factor has changed since the last call, as the
    /// cached textures are rendered in physical pixels.
    pub fn clear_cache_if_scale_factor_changed(&self, window: &crate::api::Window) {
        if self.window_scale_factor_tracker.is_dirty() {
            self.window_scale_factor_tracker
                .as_ref()
                .evaluate_as_dependency_root(|| window.scale_factor());
            self.clear();
        }
    }
}

impl<ImageType: Clone> BoxShadowCache<ImageType> {
    /// Look up a box shadow texture for a given box shadow item, or create a new one if needed.
    pub fn get_box_shadow(
        &self,
        item_rc: &ItemRc,
        item_cache: &crate::item_rendering::ItemCache<Option<ImageType>>,
        box_shadow: std::pin::Pin<&crate::items::BoxShadow>,
        scale_factor: ScaleFactor,
        shadow_render_fn: impl FnOnce(&BoxShadowOptions) -> Option<ImageType>,
    ) -> Option<ImageType> {
        item_cache.get_or_update_cache_entry(item_rc, || {
            let shadow_options = BoxShadowOptions::new(item_rc, box_shadow, scale_factor)?;
            let mut entries = self.entries.borrow_mut();
            let stamp = self.access_counter.get() + 1;
            self.access_counter.set(stamp);
            if let Some((_, entry)) =
                entries.iter_mut().find(|(options, _)| *options == shadow_options)
            {
                entry.last_used = stamp;
                return entry.image.clone();
            }
            // Brushes have no total ordering, so the cache uses equality comparisons.
            if entries.len() >= MAX_CACHED_SHADOWS {
                let oldest = entries
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, (_, entry))| entry.last_used)
                    .map(|(index, _)| index)
                    .unwrap();
                entries.swap_remove(oldest);
            }
            let image = shadow_render_fn(&shadow_options);
            entries.push((shadow_options, CacheEntry { image: image.clone(), last_used: stamp }));
            image
        })
    }
}
