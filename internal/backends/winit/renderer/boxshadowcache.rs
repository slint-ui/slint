// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::{cell::RefCell, collections::BTreeMap};

use i_slint_core::graphics::euclid;
use i_slint_core::items::ItemRc;
use i_slint_core::{
    lengths::{PhysicalPx, ScaleFactor},
    Color,
};

#[derive(Clone, PartialEq, Debug, Default)]
pub struct BoxShadowOptions {
    pub width: euclid::Length<f32, PhysicalPx>,
    pub height: euclid::Length<f32, PhysicalPx>,
    pub color: Color,
    pub blur: euclid::Length<f32, PhysicalPx>,
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
    fn new(
        box_shadow: std::pin::Pin<&i_slint_core::items::BoxShadow>,
        scale_factor: ScaleFactor,
    ) -> Option<Self> {
        let color = box_shadow.color();
        if color.alpha() == 0 {
            return None;
        }
        Some(Self {
            width: box_shadow.logical_width() * scale_factor,
            height: box_shadow.logical_height() * scale_factor,
            color,
            blur: box_shadow.logical_blur() * scale_factor, // This effectively becomes the blur radius, so scale to physical pixels
            radius: box_shadow.logical_border_radius() * scale_factor,
        })
    }
}

pub struct BoxShadowCache<ImageType>(RefCell<BTreeMap<BoxShadowOptions, ImageType>>);

impl<ImageType> Default for BoxShadowCache<ImageType> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<ImageType: Clone> BoxShadowCache<ImageType> {
    pub fn get_box_shadow(
        &self,
        item_rc: &ItemRc,
        item_cache: &i_slint_core::item_rendering::ItemCache<Option<ImageType>>,
        box_shadow: std::pin::Pin<&i_slint_core::items::BoxShadow>,
        scale_factor: ScaleFactor,
        shadow_render_fn: impl FnOnce(&BoxShadowOptions) -> ImageType,
    ) -> Option<ImageType> {
        item_cache.get_or_update_cache_entry(item_rc, || {
            let shadow_options = BoxShadowOptions::new(box_shadow, scale_factor)?;
            self.0
                .borrow_mut()
                .entry(shadow_options.clone())
                .or_insert_with(|| shadow_render_fn(&shadow_options))
                .clone()
                .into()
        })
    }
}
