// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains a cache helper for caching box shadow textures.
*/

use std::{cell::RefCell, collections::BTreeMap};

use crate::items::ItemRc;
use crate::lengths::RectLengths;
use crate::{
    lengths::{PhysicalPx, ScaleFactor},
    Color,
};

/// Struct to store options affecting the rendering of a box shadow
#[derive(Clone, PartialEq, Debug, Default)]
pub struct BoxShadowOptions {
    /// The width of the box shadow texture in physical pixels.
    pub width: euclid::Length<f32, PhysicalPx>,
    /// The height of the box shadow texture in physical pixels.
    pub height: euclid::Length<f32, PhysicalPx>,
    /// The color for the box shadow.
    pub color: Color,
    /// The blur to apply to the box shadow in pixels.
    pub blur: euclid::Length<f32, PhysicalPx>,
    /// The radius of the box shadow.
    pub radius: euclid::Length<f32, PhysicalPx>,
}

impl Eq for BoxShadowOptions {}
impl Ord for BoxShadowOptions {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if (other.width, other.height, other.color, other.blur, other.radius)
            < (self.width, self.height, self.color, self.blur, self.radius)
        {
            std::cmp::Ordering::Less
        } else if (self.width, self.height, self.color, self.blur, self.radius)
            < (other.width, other.height, other.color, other.blur, other.radius)
        {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    }
}

impl PartialOrd for BoxShadowOptions {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl BoxShadowOptions {
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
        Some(Self {
            width,
            height,
            color,
            blur: box_shadow.blur() * scale_factor, // This effectively becomes the blur radius, so scale to physical pixels
            radius: box_shadow.border_radius() * scale_factor,
        })
    }
}

/// Cache to hold box textures for given box shadow options.
pub struct BoxShadowCache<ImageType>(RefCell<BTreeMap<BoxShadowOptions, Option<ImageType>>>);

impl<ImageType> Default for BoxShadowCache<ImageType> {
    fn default() -> Self {
        Self(Default::default())
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
            self.0
                .borrow_mut()
                .entry(shadow_options.clone())
                .or_insert_with(|| shadow_render_fn(&shadow_options))
                .clone()
        })
    }
}
