// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
This module contains a cache helper for caching box shadow textures.
*/

use alloc::boxed::Box;
use alloc::vec::Vec;
use std::cell::{Cell, RefCell};

use super::boxshadow::BoxShadowOptions;
use crate::items::ItemRc;
use crate::lengths::ScaleFactor;

/// Upper bound on the number of shadow textures kept alive by a [`BoxShadowCache`].
const MAX_CACHED_SHADOWS: usize = 16;

struct CacheEntry<ImageType> {
    image: Option<ImageType>,
    /// Value of the cache's access counter when this entry was last returned, for LRU eviction.
    last_used: u64,
}

/// Cache to hold box textures for given box shadow options.
pub struct BoxShadowCache<ImageType> {
    // Failed renders precede textures so an exact failure takes priority over an equivalent mask.
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
            self.get_or_render(
                BoxShadowOptions::new(item_rc, box_shadow, scale_factor)?,
                shadow_render_fn,
            )
        })
    }

    fn get_or_render(
        &self,
        mut options: BoxShadowOptions,
        render: impl FnOnce(&BoxShadowOptions) -> Option<ImageType>,
    ) -> Option<ImageType> {
        let mut entries = self.entries.borrow_mut();
        let stamp = self.access_counter.get() + 1;
        self.access_counter.set(stamp);

        let first_texture =
            entries.iter().position(|(_, e)| e.image.is_some()).unwrap_or(entries.len());
        let (failures, textures) = entries.split_at_mut(first_texture);
        if let Some((_, entry)) = failures.iter_mut().find(|(key, _)| *key == options) {
            entry.last_used = stamp;
            return None;
        }

        let opaque =
            options.source.as_ref().is_some_and(|(paint, _)| has_opaque_shadow_mask(paint));
        let hit = if opaque {
            let mut mask_key = None;
            textures.iter_mut().find(|(key, _)| {
                key.width == options.width
                    && key.height == options.height
                    && key.blur == options.blur
                    && key.color == options.color
                    && key
                        == mask_key.get_or_insert_with(|| {
                            let mut key = options.clone();
                            set_opaque_mask(&mut key);
                            key
                        })
            })
        } else {
            textures.iter_mut().find(|(key, _)| *key == options)
        };
        if let Some((_, entry)) = hit {
            entry.last_used = stamp;
            return entry.image.clone();
        }

        if entries.len() >= MAX_CACHED_SHADOWS {
            let oldest =
                entries.iter().enumerate().min_by_key(|(_, (_, e))| e.last_used).unwrap().0;
            if oldest < first_texture {
                entries.remove(oldest);
            } else {
                entries.swap_remove(oldest);
            }
        }

        let image = render(&options);
        let entry = CacheEntry { image: image.clone(), last_used: stamp };
        if image.is_some() {
            if opaque {
                set_opaque_mask(&mut options);
            }
            entries.push((options, entry));
        } else {
            entries.insert(0, (options, entry));
        }
        image
    }
}

const OPAQUE_MASK: crate::Brush =
    crate::Brush::SolidColor(crate::Color::from_rgb_u8(255, 255, 255));

fn set_opaque_mask(key: &mut BoxShadowOptions) {
    let (background, _) = key.source.as_mut().expect("opaque shadow key requires a source");
    *background = OPAQUE_MASK;
}

fn has_opaque_shadow_mask(brush: &crate::Brush) -> bool {
    match brush {
        crate::Brush::SolidColor(color) => color.alpha() == 255,
        crate::Brush::LinearGradient(gradient) => {
            // Resolving out-of-range stops can change their alpha before rendering.
            let mut stops = gradient.stops().peekable();
            gradient.angle().is_finite()
                && stops.peek().is_some()
                && stops
                    .all(|stop| stop.color.alpha() == 255 && (0.0..=1.0).contains(&stop.position))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::{GradientStop, LinearGradientBrush};
    use crate::item_rendering::BorderRectLayout;
    use crate::{Brush, Color};

    fn options(background: Brush) -> BoxShadowOptions {
        let rect = core::pin::pin!(crate::items::BasicBorderRectangle::default());
        let layout = BorderRectLayout::new(
            rect.as_ref(),
            euclid::size2(40, 20).cast(),
            ScaleFactor::new(1.),
        )
        .unwrap();
        BoxShadowOptions { source: Some((background, layout)), ..Default::default() }
    }

    fn shadow_with_width(width: usize) -> BoxShadowOptions {
        BoxShadowOptions { width: euclid::Length::new(width as f32), ..Default::default() }
    }

    fn gradient<const N: usize>(angle: f32, stops: [(f32, Color); N]) -> Brush {
        Brush::LinearGradient(LinearGradientBrush::new(
            angle,
            stops.map(|(position, color)| GradientStop { position, color }),
        ))
    }

    #[test]
    fn opaque_fills_share_a_shadow_but_preserve_the_original_paint_on_a_miss() {
        let cache = BoxShadowCache::default();
        let color = Color::from_rgb_u8(30, 60, 90);
        let first = (0., color);
        let last = (1., Color::from_rgb_u8(255, 255, 255));
        let translucent = Color::from_argb_u8(128, 30, 60, 90);
        let fill = gradient(45., [first, last]);
        assert_eq!(
            cache.get_or_render(options(fill.clone()), |opts| {
                assert_eq!(opts.source.as_ref().unwrap().0, fill);
                Some(1)
            }),
            Some(1)
        );
        assert_eq!(
            cache.get_or_render(options(Brush::SolidColor(color)), |_| panic!("cache miss")),
            Some(1)
        );
        for (i, fill) in [
            gradient(45., [(f32::NEG_INFINITY, color), last]),
            gradient(45., [(0., translucent), last]),
            gradient(45., [first, (1., translucent)]),
            gradient(45., []),
            gradient(f32::INFINITY, [first, last]),
            Brush::SolidColor(translucent),
        ]
        .into_iter()
        .enumerate()
        {
            let image = Some(i + 2);
            assert_eq!(cache.get_or_render(options(fill), |_| image), image);
        }
        let wider_shadow = BoxShadowOptions {
            spread: euclid::Length::new(1.),
            ..options(Brush::SolidColor(color))
        };
        assert_eq!(cache.get_or_render(wider_shadow, |_| Some(0)), Some(0));
    }

    #[test]
    fn a_full_cache_releases_the_oldest_image_before_render() {
        use alloc::rc::Rc;

        let cache = BoxShadowCache::default();
        let first = cache.get_or_render(shadow_with_width(0), |_| Some(Rc::new(()))).unwrap();
        let oldest = Rc::downgrade(&first);
        drop(first);
        for width in 1..MAX_CACHED_SHADOWS {
            cache.get_or_render(shadow_with_width(width), |_| Some(Rc::new(()))).unwrap();
        }
        assert!(
            cache
                .get_or_render(shadow_with_width(MAX_CACHED_SHADOWS), |_| {
                    assert!(oldest.upgrade().is_none());
                    None
                })
                .is_none()
        );
    }

    #[test]
    fn a_failed_opaque_fill_is_cached_only_for_its_original_paint() {
        let cache = BoxShadowCache::default();
        let white = || options(OPAQUE_MASK);
        let blue = || options(Brush::SolidColor(Color::from_rgb_u8(0, 0, 255)));
        assert_eq!(cache.get_or_render(white(), |_| None), None);
        assert_eq!(cache.get_or_render(shadow_with_width(1), |_| None), None);
        assert_eq!(cache.get_or_render(blue(), |_| Some(1)), Some(1));
        for width in 2..MAX_CACHED_SHADOWS - 1 {
            cache.get_or_render(shadow_with_width(width), |_| Some(2)).unwrap();
        }
        assert_eq!(cache.get_or_render(white(), |_| panic!("lost exact failure")), None);
        cache.get_or_render(shadow_with_width(MAX_CACHED_SHADOWS), |_| Some(3)).unwrap();
        assert_eq!(cache.get_or_render(white(), |_| panic!("retried failed paint")), None);
        assert_eq!(cache.get_or_render(blue(), |_| panic!("missed cached texture")), Some(1));
    }
}
