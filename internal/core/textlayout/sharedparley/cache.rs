// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore RAII

//! The cache of shaped paragraphs, keyed by item.
//!
//! An entry holds the output of [`shaping`](super::shaping) for one item, and is invalidated by
//! the property dependencies the shaping registered.
//! [`cached_paragraphs`] is only meant to be called through `with_text_layout`, which pairs it
//! with the one shaping function every path must share.

use super::shaping::TextParagraph;
use super::*;

/// Shaped paragraphs together with the wrap mode they were shaped with.
///
/// The glyph geometry only depends on (text, font, wrap, scale factor): the width is applied
/// later by `break_all_lines`, and the fill/stroke/selection brushes only change colors, not
/// positions. So one entry serves measuring, hit-testing and drawing alike -- but only for the
/// wrap mode it was shaped with, because parley bakes the break opportunities into the shaped
/// layout via `WordBreak`/`OverflowWrap`/`TextWrapMode` (see `ranged_builder`). The scale factor
/// is the other input baked into the shaping, but it applies to every entry at once and so is
/// handled by the cache as a whole.
struct CachedParagraphs {
    wrap: TextWrap,
    /// `None` while a [`CachedParagraphsGuard`] has the paragraphs checked out; the guard puts
    /// them back when it drops. Finding `None` here therefore means the previous caller returned
    /// without handing them back, and the entry has to be reshaped rather than served empty.
    paragraphs: Option<Vec<TextParagraph>>,
}

type InnerTextLayoutCache = crate::item_rendering::ItemCache<CachedParagraphs>;

/// Cache for shaped text paragraphs (before line breaking), keyed by ItemRc.
pub struct TextLayoutCache {
    inner: InnerTextLayoutCache,
    #[cfg(feature = "testing")]
    cache_miss_count: std::cell::Cell<u64>,
}

#[allow(clippy::derivable_impls)] // clippy doesn't see the feature = "testing" code
impl Default for TextLayoutCache {
    fn default() -> Self {
        Self {
            inner: Default::default(),
            #[cfg(feature = "testing")]
            cache_miss_count: std::cell::Cell::new(0),
        }
    }
}

impl TextLayoutCache {
    /// Drops everything shaped for the previous scale factor. Glyph advances are in physical
    /// pixels, so a new scale factor invalidates every entry at once. Called on the way into the
    /// cache rather than when rendering starts, because the layout pass that follows a scale
    /// factor change measures before anything renders.
    fn clear_if_scale_factor_changed(&self, window: &crate::api::Window) {
        self.inner.clear_cache_if_scale_factor_changed(window);
    }
    pub fn component_destroyed(&self, component: crate::item_tree::ItemTreeRef) {
        self.inner.component_destroyed(component);
    }
    pub fn clear_all(&self) {
        self.inner.clear_all();
    }
}

#[cfg(feature = "testing")]
impl TextLayoutCache {
    pub fn cache_miss_count(&self) -> u64 {
        self.cache_miss_count.get()
    }
    pub fn reset_cache_miss_count(&self) {
        self.cache_miss_count.set(0);
    }
}

/// RAII guard: takes the shaped paragraphs out of the cache on creation, puts them back on drop.
pub(super) struct CachedParagraphsGuard<'a> {
    paragraphs: Option<Vec<TextParagraph>>,
    container: Option<std::cell::RefMut<'a, CachedParagraphs>>,
}

impl CachedParagraphsGuard<'_> {
    /// Lends the paragraphs to [`layout`], which hands them back as part of its `Layout`.
    pub(super) fn take(&mut self) -> Vec<TextParagraph> {
        self.paragraphs.take().unwrap_or_default()
    }

    /// Returns the paragraphs, so that the next caller reuses the shaping.
    pub(super) fn restore(&mut self, paragraphs: Vec<TextParagraph>) {
        self.paragraphs = Some(paragraphs);
    }
}

impl Drop for CachedParagraphsGuard<'_> {
    fn drop(&mut self) {
        if let (Some(paragraphs), Some(container)) = (self.paragraphs.take(), &mut self.container) {
            container.paragraphs = Some(paragraphs);
        }
    }
}

/// Shapes the text of `item_rc` for `wrap`, reusing the `TextLayoutCache` entry when it holds
/// paragraphs shaped for the same wrap mode and none of the properties `shape` read have changed
/// since. Without a cache or item it just shapes, so the caller doesn't need to special-case that.
///
/// `shape` runs inside the entry's dependency tracker, so everything it reads (the text and the
/// font request, at least) invalidates the entry when it changes. Properties evaluated by the
/// caller before this point are clean by then and thus can't re-enter here.
pub(super) fn cached_paragraphs<'a>(
    cache: Option<&'a TextLayoutCache>,
    item_rc: Option<&crate::item_tree::ItemRc>,
    wrap: TextWrap,
    window: &crate::api::Window,
    font_context: &mut parley::FontContext,
    shape: &dyn Fn(&mut parley::FontContext) -> Vec<TextParagraph>,
) -> CachedParagraphsGuard<'a> {
    let Some((cache, item_rc)) = cache.zip(item_rc) else {
        return CachedParagraphsGuard { paragraphs: Some(shape(font_context)), container: None };
    };

    cache.clear_if_scale_factor_changed(window);

    // Shaped geometry must never be mixed across wrap modes, and the entry only holds one mode
    // at a time. Drop a mismatching one up front so the shaping below happens in the regular
    // (vacant) path, inside a fresh dependency tracker and without the cache borrowed.
    //
    // Paragraphs that were never handed back can't be served either.
    let stale = cache
        .inner
        .with_entry(item_rc, |entry| {
            (entry.wrap != wrap || entry.paragraphs.is_none()).then_some(())
        })
        .is_some();
    if stale {
        cache.inner.release(item_rc);
    }

    let mut entry = cache.inner.get_or_update_cache_entry_ref(item_rc, || {
        #[cfg(feature = "testing")]
        cache.cache_miss_count.set(cache.cache_miss_count.get() + 1);
        CachedParagraphs { wrap, paragraphs: Some(shape(font_context)) }
    });
    let paragraphs = entry.paragraphs.take().unwrap_or_default();
    CachedParagraphsGuard { paragraphs: Some(paragraphs), container: Some(entry) }
}
