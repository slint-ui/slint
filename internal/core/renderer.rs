// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use core::pin::Pin;

use crate::graphics::{Point, Rect, Size};
use crate::Coord;

pub trait Renderer {
    /// Returns the size of the given text in logical pixels.
    /// When set, `max_width` means that one need to wrap the text so it does not go further than that
    fn text_size(
        &self,
        font_request: crate::graphics::FontRequest,
        text: &str,
        max_width: Option<Coord>,
        scale_factor: f32,
    ) -> Size;

    /// Returns the (UTF-8) byte offset in the text property that refers to the character that contributed to
    /// the glyph cluster that's visually nearest to the given coordinate. This is used for hit-testing,
    /// for example when receiving a mouse click into a text field. Then this function returns the "cursor"
    /// position.
    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&crate::items::TextInput>,
        pos: Point,
    ) -> usize;

    /// That's the opposite of [`Self::text_input_byte_offset_for_position`]
    /// It takes a (UTF-8) byte offset in the text property, and returns a Rectangle
    /// left to the char. It is one logical pixel wide and ends at the baseline.
    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&crate::items::TextInput>,
        byte_offset: usize,
    ) -> Rect;

    /// Clear the caches for the items that are being removed
    fn free_graphics_resources(
        &self,
        _items: &mut dyn Iterator<Item = Pin<crate::items::ItemRef<'_>>>,
    ) {
    }

    /// Mark a given region as dirty regardless whether the items actually are dirty.
    ///
    /// Example: when a PopupWindow disapear, the region under the popup needs to be redrawn
    fn mark_dirty_region(&self, _region: crate::item_rendering::DirtyRegion) {}
}
