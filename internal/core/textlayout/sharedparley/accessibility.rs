// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! AccessKit emission for `TextInput`: turns the layout the renderer draws with into TextRun
//! children, so that a screen reader sees per-character metrics and the selection rather than one
//! opaque value.
use super::*;
use accesskit::{Node, NodeId, TextPosition, TextSelection, TreeUpdate};
use alloc::string::ToString;
use parley::{Cursor, LayoutAccessibility};

/// What one `TextInput` needs to keep between emissions.
///
/// Only the `LayoutAccessibility` maps, which is what makes `build_nodes` reuse a `NodeId` for a
/// span that survived an edit. The shaped paragraphs aren't kept, so the geometry reported is
/// always the one on screen.
pub struct CachedTextInputAccessibilityState {
    layout_access: Vec<LayoutAccessibility>,
    /// Never reset while this state lives, so that a later emission can't reuse an index for a
    /// span an assistive technology may still remember.
    next_sub_index: u32,
}

impl Default for CachedTextInputAccessibilityState {
    fn default() -> Self {
        Self { layout_access: Vec::new(), next_sub_index: 1 }
    }
}

impl CachedTextInputAccessibilityState {
    /// Emits TextRun children of `parent_node` describing the input's text, and sets its value and
    /// selection.
    ///
    /// `physical_offset` is where the wrapper sits in the AccessKit tree's coordinates, and
    /// `encode_sub_node_id` mints a NodeId for a span, given its parent.
    pub fn emit(
        &mut self,
        renderer: &dyn crate::renderer::Renderer,
        text_input: Pin<&crate::items::TextInput>,
        item_rc: &crate::item_tree::ItemRc,
        size: LogicalSize,
        update: &mut TreeUpdate,
        parent_node: &mut Node,
        parent_id: NodeId,
        physical_offset: (f64, f64),
        encode_sub_node_id: impl Fn(NodeId, u32) -> NodeId,
    ) -> bool {
        let (visible_anchor, visible_cursor, cursor_affinity) = selection_offsets(text_input);

        // Before the layout, so it survives a renderer with none to lend. The displayed
        // text, not `accessible-value`: that one is the raw `text`, which for a password field
        // would hand out the characters themselves.
        if let PlainOrStyledText::Plain(displayed) =
            crate::item_rendering::RenderString::text(text_input)
        {
            parent_node.set_value(displayed.as_str().to_string());
        }

        with_text_input_layout(renderer, text_input, item_rc, size, |layout| {
            let paragraphs: alloc::vec::Vec<_> = layout.paragraphs().collect();

            // Grow or shrink to one entry per paragraph; the ones that stay keep their NodeIds.
            self.layout_access.resize_with(paragraphs.len(), Default::default);

            // Captures the persistent counter, so the indices keep growing across emissions.
            let next_sub_index = &mut self.next_sub_index;
            let mut allocate_sub = || {
                let sub = *next_sub_index;
                *next_sub_index = next_sub_index.saturating_add(1);
                encode_sub_node_id(parent_id, sub)
            };

            let total_paragraphs = paragraphs.len();
            for (i, (para, la)) in paragraphs.iter().zip(self.layout_access.iter_mut()).enumerate()
            {
                let para_y_offset = physical_offset.1 + para.y.get() as f64;
                let nodes_before = update.nodes.len();
                la.build_nodes(
                    para.text,
                    para.layout,
                    update,
                    parent_node,
                    &mut allocate_sub,
                    physical_offset.0,
                    para_y_offset,
                    // A `TextInput`'s text is plain, so it carries no styled spans.
                    |_node, _style| {},
                );

                // We split the text at `\n` before shaping, so parley never sees the newline as
                // a cluster. AccessKit expects it in the paragraph's last TextRun, otherwise a
                // caret crossing the break announces the next line's first character instead.
                if i + 1 < total_paragraphs
                    && update.nodes.len() > nodes_before
                    && let Some((_, node)) = update.nodes.last_mut()
                {
                    let mut value = node.value().map(|s| s.to_string()).unwrap_or_default();
                    let mut lengths: alloc::vec::Vec<u8> = node.character_lengths().to_vec();
                    let mut widths: alloc::vec::Vec<f32> =
                        node.character_widths().map(|s| s.to_vec()).unwrap_or_default();
                    let mut positions: alloc::vec::Vec<f32> =
                        node.character_positions().map(|s| s.to_vec()).unwrap_or_default();

                    let last_x = positions.last().copied().unwrap_or(0.0)
                        + widths.last().copied().unwrap_or(0.0);
                    value.push('\n');
                    lengths.push(1);
                    positions.push(last_x);
                    widths.push(0.0);

                    node.set_value(value);
                    node.set_character_lengths(lengths);
                    node.set_character_widths(widths);
                    node.set_character_positions(positions);
                }
            }

            if let Some(selection) = compose_text_selection(
                &paragraphs,
                &self.layout_access,
                visible_anchor,
                visible_cursor,
                cursor_affinity,
            ) {
                parent_node.set_text_selection(selection);
            }
        })
        .is_some()
    }

    /// Translates an incoming selection into byte offsets in the input's text.
    pub fn decode_selection(
        &self,
        renderer: &dyn crate::renderer::Renderer,
        text_input: Pin<&crate::items::TextInput>,
        item_rc: &crate::item_tree::ItemRc,
        size: LogicalSize,
        anchor: &TextPosition,
        focus: &TextPosition,
    ) -> Option<(usize, usize)> {
        let (anchor, focus) =
            with_text_input_layout(renderer, text_input, item_rc, size, |layout| {
                let paragraphs: alloc::vec::Vec<_> = layout.paragraphs().collect();
                // Both ends have to resolve, or the selection means nothing.
                self.byte_offset(&paragraphs, anchor).zip(self.byte_offset(&paragraphs, focus))
            })
            .flatten()?;
        Some((to_actual_offset(text_input, anchor), to_actual_offset(text_input, focus)))
    }

    fn byte_offset(
        &self,
        paragraphs: &[TextInputParagraph<'_>],
        pos: &TextPosition,
    ) -> Option<usize> {
        for (para, la) in paragraphs.iter().zip(self.layout_access.iter()) {
            if let Some(cursor) = Cursor::from_access_position(pos, para.layout, la) {
                return Some(para.range.start + cursor.index());
            }
        }
        None
    }
}

/// Maps a byte offset in the text the input *displays* back to one in the text it *holds*.
///
/// A password field displays [`crate::items::PASSWORD_CHARACTER`] per character, and that character
/// is three bytes in UTF-8 where the ones it stands in for are often one, so the two run out of step
/// as soon as anything past the first character is selected. The offsets an assistive technology
/// hands back index the displayed text, while `TextInput::set-selection-offsets` indexes the held
/// text, so they have to be converted on the way through.
///
/// This is what `TextInputVisualRepresentation::map_byte_offset_from_visual_text_to_actual_text`
/// does for the renderer's hit-testing; the accessibility pass can't reuse it, because reaching a
/// `TextInputVisualRepresentation` means reading `cursor_visible` and dirtying the AT subtree on
/// every blink.
fn to_actual_offset(text_input: Pin<&crate::items::TextInput>, displayed_offset: usize) -> usize {
    if !text_input.is_password() {
        return displayed_offset;
    }
    // One mask character per character of the text, so the offset divides out to an index.
    let unmasked = text_input.text_with_preedit().0;
    unmasked
        .char_indices()
        .nth(displayed_offset / crate::items::PASSWORD_CHARACTER.len_utf8())
        .map_or(unmasked.len(), |(offset, _)| offset)
}

/// The selection anchor and cursor, as byte offsets into the text the input displays, plus the
/// affinity that resolves the cursor's offset at a soft line break.
///
/// Not read off a `TextInputVisualRepresentation`, which would subscribe to `cursor_visible` and
/// invalidate the accessibility subtree on every blink.
fn selection_offsets(
    text_input: Pin<&crate::items::TextInput>,
) -> (usize, usize, crate::items::TextCursorAffinity) {
    // The splice the shaping path reads, so these offsets index the string it shaped.
    let (visible_pre_mask, composition) = text_input.text_with_preedit();

    let (raw_anchor, raw_cursor, affinity) = if !composition.is_empty() {
        // `preedit_selection` is private, so a selection inside the composition isn't reported;
        // the caret goes to the end of it.
        (composition.end, composition.end, crate::items::TextCursorAffinity::NextCharacter)
    } else {
        (
            text_input.anchor_position(&visible_pre_mask),
            text_input.cursor_position(&visible_pre_mask),
            text_input.cursor_position_affinity(),
        )
    };

    // Last, so the offsets end up indexing the masked string the layout was shaped from.
    if text_input.is_password() {
        let mask_char_len = crate::items::PASSWORD_CHARACTER.len_utf8();
        let to_masked = |actual_offset: usize| -> usize {
            visible_pre_mask[..actual_offset.min(visible_pre_mask.len())].chars().count()
                * mask_char_len
        };
        (to_masked(raw_anchor), to_masked(raw_cursor), affinity)
    } else {
        (raw_anchor, raw_cursor, affinity)
    }
}

fn position_for_byte_offset(
    paragraphs: &[TextInputParagraph<'_>],
    layout_access: &[LayoutAccessibility],
    offset: usize,
    affinity: crate::items::TextCursorAffinity,
) -> Option<TextPosition> {
    for (para, la) in paragraphs.iter().zip(layout_access.iter()) {
        if offset < para.range.start || offset > para.range.end {
            continue;
        }
        let local = offset - para.range.start;
        let cursor = Cursor::from_byte_index(para.layout, local, affinity.into());
        if let Some(pos) = cursor.to_access_position(para.layout, la) {
            return Some(pos);
        }
    }
    None
}

fn compose_text_selection(
    paragraphs: &[TextInputParagraph<'_>],
    layout_access: &[LayoutAccessibility],
    anchor: usize,
    focus: usize,
    focus_affinity: crate::items::TextCursorAffinity,
) -> Option<TextSelection> {
    let anchor_pos = position_for_byte_offset(
        paragraphs,
        layout_access,
        anchor,
        crate::items::TextCursorAffinity::NextCharacter,
    )?;
    let focus_pos = position_for_byte_offset(paragraphs, layout_access, focus, focus_affinity)?;
    Some(TextSelection { anchor: anchor_pos, focus: focus_pos })
}
