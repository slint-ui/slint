// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
This module contains the builtin text related items.

When adding an item or a property, it needs to be kept in sync with different place.
Lookup the [`crate::items`] module documentation.
*/

use super::{
    InputType, Item, ItemConsts, ItemRc, KeyEventResult, KeyEventType, PointArg,
    PointerEventButton, RenderingResult, TextHorizontalAlignment, TextOverflow,
    TextVerticalAlignment, TextWrap, VoidArg,
};
use crate::graphics::{Brush, Color, FontRequest};
use crate::input::{
    key_codes, FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent,
    KeyboardModifiers, MouseEvent, StandardShortcut, TextShortcut,
};
use crate::item_rendering::{CachedRenderingData, ItemRenderer};
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize, ScaleFactor};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::{WindowAdapter, WindowInner};
use crate::{Callback, Coord, Property, SharedString};
use alloc::rc::Rc;
use alloc::string::String;
use const_field_offset::FieldOffsets;
use core::pin::Pin;
#[allow(unused)]
use euclid::num::Ceil;
use i_slint_core_macros::*;
use unicode_segmentation::UnicodeSegmentation;

/// The implementation of the `Text` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct Text {
    pub text: Property<SharedString>,
    pub font_family: Property<SharedString>,
    pub font_size: Property<LogicalLength>,
    pub font_weight: Property<i32>,
    pub color: Property<Brush>,
    pub horizontal_alignment: Property<TextHorizontalAlignment>,
    pub vertical_alignment: Property<TextVerticalAlignment>,
    pub wrap: Property<TextWrap>,
    pub overflow: Property<TextOverflow>,
    pub letter_spacing: Property<LogicalLength>,
    pub x: Property<LogicalLength>,
    pub y: Property<LogicalLength>,
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Text {
    fn init(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn geometry(self: Pin<&Self>) -> LogicalRect {
        LogicalRect::new(
            LogicalPoint::from_lengths(self.x(), self.y()),
            LogicalSize::from_lengths(self.width(), self.height()),
        )
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        let window_inner = WindowInner::from_pub(window_adapter.window());
        let implicit_size = |max_width| {
            window_adapter.renderer().text_size(
                self.font_request(window_inner),
                self.text().as_str(),
                max_width,
                ScaleFactor::new(window_adapter.window().scale_factor()),
            )
        };

        // Stretch uses `round_layout` to explicitly align the top left and bottom right of layout nodes
        // to pixel boundaries. To avoid rounding down causing the minimum width to become so little that
        // letters will be cut off, apply the ceiling here.
        match orientation {
            Orientation::Horizontal => {
                let implicit_size = implicit_size(None);
                let min = match self.overflow() {
                    TextOverflow::Elide => implicit_size.width.min(
                        window_adapter
                            .renderer()
                            .text_size(
                                self.font_request(window_inner),
                                "…",
                                None,
                                ScaleFactor::new(window_inner.scale_factor()),
                            )
                            .width,
                    ),
                    TextOverflow::Clip => match self.wrap() {
                        TextWrap::NoWrap => implicit_size.width,
                        TextWrap::WordWrap => 0 as Coord,
                    },
                };
                LayoutInfo {
                    min: min.ceil(),
                    preferred: implicit_size.width.ceil(),
                    ..LayoutInfo::default()
                }
            }
            Orientation::Vertical => {
                let h = match self.wrap() {
                    TextWrap::NoWrap => implicit_size(None).height,
                    TextWrap::WordWrap => implicit_size(Some(self.width())).height,
                }
                .ceil();
                LayoutInfo { min: h, preferred: h, ..LayoutInfo::default() }
            }
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        _: &KeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn render(
        self: Pin<&Self>,
        backend: &mut &mut dyn ItemRenderer,
        self_rc: &ItemRc,
    ) -> RenderingResult {
        (*backend).draw_text(self, self_rc);
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for Text {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Text, CachedRenderingData> =
        Text::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

impl Text {
    pub fn font_request(self: Pin<&Self>, window: &WindowInner) -> FontRequest {
        let window_item = window.window_item();

        FontRequest {
            family: {
                let maybe_family = self.font_family();
                if !maybe_family.is_empty() {
                    Some(maybe_family)
                } else {
                    window_item.as_ref().and_then(|item| item.as_pin_ref().font_family())
                }
            },
            weight: {
                let weight = self.font_weight();
                if weight == 0 {
                    window_item.as_ref().and_then(|item| item.as_pin_ref().font_weight())
                } else {
                    Some(weight)
                }
            },
            pixel_size: {
                let font_size = self.font_size();
                if font_size.get() == 0 as Coord {
                    window_item.as_ref().and_then(|item| item.as_pin_ref().font_size())
                } else {
                    Some(font_size)
                }
            },
            letter_spacing: Some(self.letter_spacing()),
        }
    }
}

/// The implementation of the `TextInput` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct TextInput {
    pub text: Property<SharedString>,
    pub font_family: Property<SharedString>,
    pub font_size: Property<LogicalLength>,
    pub font_weight: Property<i32>,
    pub color: Property<Brush>,
    pub selection_foreground_color: Property<Color>,
    pub selection_background_color: Property<Color>,
    pub horizontal_alignment: Property<TextHorizontalAlignment>,
    pub vertical_alignment: Property<TextVerticalAlignment>,
    pub wrap: Property<TextWrap>,
    pub input_type: Property<InputType>,
    pub letter_spacing: Property<LogicalLength>,
    pub x: Property<LogicalLength>,
    pub y: Property<LogicalLength>,
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub cursor_position_byte_offset: Property<i32>,
    pub anchor_position_byte_offset: Property<i32>,
    pub text_cursor_width: Property<LogicalLength>,
    pub cursor_visible: Property<bool>,
    pub has_focus: Property<bool>,
    pub enabled: Property<bool>,
    pub accepted: Callback<VoidArg>,
    pub cursor_position_changed: Callback<PointArg>,
    pub edited: Callback<VoidArg>,
    pub pressed: core::cell::Cell<bool>,
    pub single_line: Property<bool>,
    pub read_only: Property<bool>,
    pub preedit_text: Property<SharedString>,
    pub preedit_selection_start: Property<i32>, // byte offset, relative to cursor
    pub preedit_selection_end: Property<i32>,   // byte offset, relative to cursor
    pub cached_rendering_data: CachedRenderingData,
    // The x position where the cursor wants to be.
    // It is not updated when moving up and down even when the line is shorter.
    preferred_x_pos: core::cell::Cell<Coord>,
}

impl Item for TextInput {
    fn init(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    // FIXME: width / height.  or maybe it doesn't matter?  (
    fn geometry(self: Pin<&Self>) -> LogicalRect {
        LogicalRect::new(
            LogicalPoint::from_lengths(self.x(), self.y()),
            LogicalSize::from_lengths(self.width(), self.height()),
        )
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        let text = self.text();
        let implicit_size = |max_width| {
            window_adapter.renderer().text_size(
                self.font_request(window_adapter),
                {
                    if text.is_empty() {
                        "*"
                    } else {
                        text.as_str()
                    }
                },
                max_width,
                ScaleFactor::new(window_adapter.window().scale_factor()),
            )
        };

        // Stretch uses `round_layout` to explicitly align the top left and bottom right of layout nodes
        // to pixel boundaries. To avoid rounding down causing the minimum width to become so little that
        // letters will be cut off, apply the ceiling here.
        match orientation {
            Orientation::Horizontal => {
                let implicit_size = implicit_size(None);
                let min = match self.wrap() {
                    TextWrap::NoWrap => implicit_size.width,
                    TextWrap::WordWrap => 0 as Coord,
                };
                LayoutInfo {
                    min: min.ceil(),
                    preferred: implicit_size.width.ceil(),
                    ..LayoutInfo::default()
                }
            }
            Orientation::Vertical => {
                let h = match self.wrap() {
                    TextWrap::NoWrap => implicit_size(None).height,
                    TextWrap::WordWrap => implicit_size(Some(self.width())).height,
                }
                .ceil();
                LayoutInfo { min: h, preferred: h, ..LayoutInfo::default() }
            }
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) -> InputEventResult {
        if !self.enabled() {
            return InputEventResult::EventIgnored;
        }
        match event {
            MouseEvent::Pressed { position, button: PointerEventButton::Left, click_count } => {
                let clicked_offset =
                    window_adapter.renderer().text_input_byte_offset_for_position(self, position)
                        as i32;
                self.as_ref().pressed.set(true);
                self.as_ref().anchor_position_byte_offset.set(clicked_offset);

                match click_count % 3 {
                    0 => self.set_cursor_position(clicked_offset, true, window_adapter, self_rc),
                    1 => self.select_word(window_adapter, self_rc),
                    2 => self.select_paragraph(window_adapter, self_rc),
                    _ => unreachable!(),
                };

                if !self.has_focus() {
                    WindowInner::from_pub(window_adapter.window()).set_focus_item(self_rc);
                }
            }
            MouseEvent::Released { button: PointerEventButton::Left, .. } => {
                self.as_ref().pressed.set(false)
            }
            MouseEvent::Exit => {
                window_adapter.set_mouse_cursor(super::MouseCursor::Default);
                self.as_ref().pressed.set(false)
            }
            MouseEvent::Moved { position } => {
                window_adapter.set_mouse_cursor(super::MouseCursor::Text);
                if self.as_ref().pressed.get() {
                    let clicked_offset = window_adapter
                        .renderer()
                        .text_input_byte_offset_for_position(self, position)
                        as i32;
                    self.set_cursor_position(clicked_offset, true, window_adapter, self_rc);
                }
            }
            _ => return InputEventResult::EventIgnored,
        }
        InputEventResult::EventAccepted
    }

    fn key_event(
        self: Pin<&Self>,
        event: &KeyEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) -> KeyEventResult {
        if !self.enabled() {
            return KeyEventResult::EventIgnored;
        }

        match event.event_type {
            KeyEventType::KeyPressed => {
                match event.text_shortcut() {
                    Some(text_shortcut) if !self.read_only() => match text_shortcut {
                        TextShortcut::Move(direction) => {
                            TextInput::move_cursor(
                                self,
                                direction,
                                event.modifiers.into(),
                                window_adapter,
                                self_rc,
                            );
                            return KeyEventResult::EventAccepted;
                        }
                        TextShortcut::DeleteForward => {
                            TextInput::select_and_delete(
                                self,
                                TextCursorDirection::Forward,
                                window_adapter,
                                self_rc,
                            );
                            return KeyEventResult::EventAccepted;
                        }
                        TextShortcut::DeleteBackward => {
                            // Special case: backspace breaks the grapheme and selects the previous character
                            TextInput::select_and_delete(
                                self,
                                TextCursorDirection::PreviousCharacter,
                                window_adapter,
                                self_rc,
                            );
                            return KeyEventResult::EventAccepted;
                        }
                        TextShortcut::DeleteWordForward => {
                            TextInput::select_and_delete(
                                self,
                                TextCursorDirection::ForwardByWord,
                                window_adapter,
                                self_rc,
                            );
                            return KeyEventResult::EventAccepted;
                        }
                        TextShortcut::DeleteWordBackward => {
                            TextInput::select_and_delete(
                                self,
                                TextCursorDirection::BackwardByWord,
                                window_adapter,
                                self_rc,
                            );
                            return KeyEventResult::EventAccepted;
                        }
                    },
                    Some(_) => {
                        return KeyEventResult::EventIgnored;
                    }
                    None => (),
                };

                if let Some(keycode) = event.text.chars().next() {
                    if keycode == key_codes::Return && !self.read_only() && self.single_line() {
                        Self::FIELD_OFFSETS.accepted.apply_pin(self).call(&());
                        return KeyEventResult::EventAccepted;
                    }
                }

                // Only insert/interpreter non-control character strings
                if event.text.is_empty()
                    || event.text.as_str().chars().any(|ch| {
                        // exclude the private use area as we encode special keys into it
                        ('\u{f700}'..='\u{f7ff}').contains(&ch) || (ch.is_control() && ch != '\n')
                    })
                {
                    return KeyEventResult::EventIgnored;
                }
                match event.shortcut() {
                    Some(shortcut) => match shortcut {
                        StandardShortcut::SelectAll => {
                            self.select_all(window_adapter, self_rc);
                            return KeyEventResult::EventAccepted;
                        }
                        StandardShortcut::Copy => {
                            self.copy();
                            return KeyEventResult::EventAccepted;
                        }
                        StandardShortcut::Paste if !self.read_only() => {
                            self.paste(window_adapter, self_rc);
                            return KeyEventResult::EventAccepted;
                        }
                        StandardShortcut::Cut if !self.read_only() => {
                            self.copy();
                            self.delete_selection(window_adapter, self_rc);
                            return KeyEventResult::EventAccepted;
                        }
                        StandardShortcut::Paste | StandardShortcut::Cut => {
                            return KeyEventResult::EventIgnored;
                        }
                        _ => (),
                    },
                    None => (),
                }
                if self.read_only() || event.modifiers.control {
                    return KeyEventResult::EventIgnored;
                }
                self.delete_selection(window_adapter, self_rc);

                let mut text: String = self.text().into();

                // FIXME: respect grapheme boundaries
                let insert_pos = self.selection_anchor_and_cursor().1;
                text.insert_str(insert_pos, &event.text);

                self.as_ref().text.set(text.into());
                let new_cursor_pos = (insert_pos + event.text.len()) as i32;
                self.as_ref().anchor_position_byte_offset.set(new_cursor_pos);
                self.set_cursor_position(new_cursor_pos, true, window_adapter, self_rc);

                // Keep the cursor visible when inserting text. Blinking should only occur when
                // nothing is entered or the cursor isn't moved.
                self.as_ref().show_cursor(window_adapter);

                Self::FIELD_OFFSETS.edited.apply_pin(self).call(&());

                KeyEventResult::EventAccepted
            }
            KeyEventType::UpdateComposition => {
                self.as_ref().preedit_text.set(event.text.clone());

                self.as_ref().preedit_selection_start.set(event.preedit_selection_start as i32);
                self.as_ref().preedit_selection_end.set(event.preedit_selection_end as i32);

                KeyEventResult::EventAccepted
            }
            KeyEventType::CommitComposition => {
                // Winit says that it will always send an event to empty the pre-edit area, but with
                // korean IME on Windows for example that's not the case. Qt also doesn't make that guarantee,
                // so clear it by hand here.
                self.preedit_text.set(Default::default());
                self.preedit_selection_start.set(0);
                self.preedit_selection_end.set(0);
                self.insert(&event.text, window_adapter, self_rc);
                KeyEventResult::EventAccepted
            }
            _ => KeyEventResult::EventIgnored,
        }
    }

    fn focus_event(
        self: Pin<&Self>,
        event: &FocusEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        match event {
            FocusEvent::FocusIn | FocusEvent::WindowReceivedFocus => {
                self.has_focus.set(true);
                self.show_cursor(window_adapter);
                window_adapter.enable_input_method(self.input_type());
            }
            FocusEvent::FocusOut | FocusEvent::WindowLostFocus => {
                self.has_focus.set(false);
                self.hide_cursor();
                window_adapter.disable_input_method();
            }
        }
        FocusEventResult::FocusAccepted
    }

    fn render(
        self: Pin<&Self>,
        backend: &mut &mut dyn ItemRenderer,
        self_rc: &ItemRc,
    ) -> RenderingResult {
        (*backend).draw_text_input(self, self_rc);
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for TextInput {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        TextInput,
        CachedRenderingData,
    > = TextInput::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

pub enum TextCursorDirection {
    Forward,
    Backward,
    ForwardByWord,
    BackwardByWord,
    NextLine,
    PreviousLine,
    PreviousCharacter, // breaks grapheme boundaries, so only used by delete-previous-char
    StartOfLine,
    EndOfLine,
    StartOfParagraph, // These don't care about wrapping
    EndOfParagraph,
    StartOfText,
    EndOfText,
}

impl core::convert::TryFrom<char> for TextCursorDirection {
    type Error = ();

    fn try_from(value: char) -> Result<Self, Self::Error> {
        Ok(match value {
            key_codes::LeftArrow => Self::Backward,
            key_codes::RightArrow => Self::Forward,
            key_codes::UpArrow => Self::PreviousLine,
            key_codes::DownArrow => Self::NextLine,
            // On macos this scrolls to the top or the bottom of the page
            #[cfg(not(target_os = "macos"))]
            key_codes::Home => Self::StartOfLine,
            #[cfg(not(target_os = "macos"))]
            key_codes::End => Self::EndOfLine,
            _ => return Err(()),
        })
    }
}

enum AnchorMode {
    KeepAnchor,
    MoveAnchor,
}

impl From<KeyboardModifiers> for AnchorMode {
    fn from(modifiers: KeyboardModifiers) -> Self {
        if modifiers.shift {
            Self::KeepAnchor
        } else {
            Self::MoveAnchor
        }
    }
}

fn safe_byte_offset(unsafe_byte_offset: i32, text: &str) -> usize {
    if unsafe_byte_offset <= 0 {
        return 0;
    }
    let byte_offset_candidate = unsafe_byte_offset as usize;

    if byte_offset_candidate >= text.len() {
        return text.len();
    }

    if text.is_char_boundary(byte_offset_candidate) {
        return byte_offset_candidate;
    }

    // Use std::floor_char_boundary once stabilized.
    text.char_indices()
        .find_map(|(offset, _)| if offset >= byte_offset_candidate { Some(offset) } else { None })
        .unwrap_or_else(|| text.len())
}

/// This struct holds the fields needed for rendering a TextInput item after applying any
/// on-going composition. This way the renderer's don't have to duplicate the code for extracting
/// and applying the pre-edit text, cursor placement within, etc.
pub struct TextInputVisualRepresentation {
    /// The text to be rendered including any pre-edit string
    pub text: String,
    /// If set, this field specifies the range as byte offsets within the text field where the composition
    /// is in progress. Renderers typically provide visual feedback for the currently composed text, such as
    /// by using underlines.
    pub preedit_range: core::ops::Range<usize>,
    /// If set, specifies the range as byte offsets within the text where to draw the selection.
    pub selection_range: core::ops::Range<usize>,
    /// The position where to draw the cursor, as byte offset within the text.
    pub cursor_position: Option<usize>,
}

impl TextInput {
    fn show_cursor(&self, window_adapter: &Rc<dyn WindowAdapter>) {
        WindowInner::from_pub(window_adapter.window())
            .set_cursor_blink_binding(&self.cursor_visible);
    }

    fn hide_cursor(&self) {
        self.cursor_visible.set(false);
    }

    /// Moves the cursor (and/or anchor) and returns true if the cursor position changed; false otherwise.
    fn move_cursor(
        self: Pin<&Self>,
        direction: TextCursorDirection,
        anchor_mode: AnchorMode,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) -> bool {
        let text = self.text();
        if text.is_empty() {
            return false;
        }

        let renderer = window_adapter.renderer();

        let last_cursor_pos = self.cursor_position(&text);

        let mut grapheme_cursor =
            unicode_segmentation::GraphemeCursor::new(last_cursor_pos, text.len(), true);

        let font_height = renderer
            .text_size(
                self.font_request(window_adapter),
                " ",
                None,
                ScaleFactor::new(window_adapter.window().scale_factor()),
            )
            .height;

        let mut reset_preferred_x_pos = true;

        let new_cursor_pos = match direction {
            TextCursorDirection::Forward => {
                grapheme_cursor.next_boundary(&text, 0).ok().flatten().unwrap_or_else(|| text.len())
            }
            TextCursorDirection::Backward => {
                grapheme_cursor.prev_boundary(&text, 0).ok().flatten().unwrap_or(0)
            }
            TextCursorDirection::NextLine => {
                reset_preferred_x_pos = false;

                let cursor_rect =
                    renderer.text_input_cursor_rect_for_byte_offset(self, last_cursor_pos);
                let mut cursor_xy_pos = cursor_rect.center();

                cursor_xy_pos.y += font_height;
                cursor_xy_pos.x = self.preferred_x_pos.get();
                renderer.text_input_byte_offset_for_position(self, cursor_xy_pos)
            }
            TextCursorDirection::PreviousLine => {
                reset_preferred_x_pos = false;

                let cursor_rect =
                    renderer.text_input_cursor_rect_for_byte_offset(self, last_cursor_pos);
                let mut cursor_xy_pos = cursor_rect.center();

                cursor_xy_pos.y -= font_height;
                cursor_xy_pos.x = self.preferred_x_pos.get();
                renderer.text_input_byte_offset_for_position(self, cursor_xy_pos)
            }
            TextCursorDirection::PreviousCharacter => {
                let mut i = last_cursor_pos;
                loop {
                    i = i.checked_sub(1).unwrap_or_default();
                    if text.is_char_boundary(i) {
                        break i;
                    }
                }
            }
            // Currently moving by word behaves like macos: next end of word(forward) or previous beginning of word(backward)
            TextCursorDirection::ForwardByWord => text
                .unicode_word_indices()
                .skip_while(|(offset, slice)| *offset + slice.len() <= last_cursor_pos)
                .next()
                .map_or(text.len(), |(offset, slice)| offset + slice.len()),
            TextCursorDirection::BackwardByWord => {
                let mut word_offset = 0;

                for (current_word_offset, _) in text.unicode_word_indices() {
                    if current_word_offset < last_cursor_pos {
                        word_offset = current_word_offset;
                    } else {
                        break;
                    }
                }

                word_offset
            }
            TextCursorDirection::StartOfLine => {
                let cursor_rect =
                    renderer.text_input_cursor_rect_for_byte_offset(self, last_cursor_pos);
                let mut cursor_xy_pos = cursor_rect.center();

                cursor_xy_pos.x = 0 as Coord;
                renderer.text_input_byte_offset_for_position(self, cursor_xy_pos)
            }
            TextCursorDirection::EndOfLine => {
                let cursor_rect =
                    renderer.text_input_cursor_rect_for_byte_offset(self, last_cursor_pos);
                let mut cursor_xy_pos = cursor_rect.center();

                cursor_xy_pos.x = Coord::MAX;
                renderer.text_input_byte_offset_for_position(self, cursor_xy_pos)
            }
            TextCursorDirection::StartOfParagraph => text
                .as_bytes()
                .iter()
                .enumerate()
                .rev()
                .skip(text.len() - last_cursor_pos + 1)
                .find(|(_, &c)| c == b'\n')
                .map(|(new_pos, _)| new_pos + 1)
                .unwrap_or(0),
            TextCursorDirection::EndOfParagraph => text
                .as_bytes()
                .iter()
                .enumerate()
                .skip(last_cursor_pos + 1)
                .find(|(_, &c)| c == b'\n')
                .map(|(new_pos, _)| new_pos)
                .unwrap_or(text.len()),
            TextCursorDirection::StartOfText => 0,
            TextCursorDirection::EndOfText => text.len(),
        };

        match anchor_mode {
            AnchorMode::KeepAnchor => {}
            AnchorMode::MoveAnchor => {
                self.as_ref().anchor_position_byte_offset.set(new_cursor_pos as i32);
            }
        }
        self.set_cursor_position(
            new_cursor_pos as i32,
            reset_preferred_x_pos,
            window_adapter,
            self_rc,
        );

        // Keep the cursor visible when moving. Blinking should only occur when
        // nothing is entered or the cursor isn't moved.
        self.as_ref().show_cursor(window_adapter);

        new_cursor_pos != last_cursor_pos
    }

    fn set_cursor_position(
        self: Pin<&Self>,
        new_position: i32,
        reset_preferred_x_pos: bool,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) {
        self.cursor_position_byte_offset.set(new_position);
        if new_position >= 0 {
            let pos = window_adapter
                .renderer()
                .text_input_cursor_rect_for_byte_offset(self, new_position as usize)
                .origin
                .to_untyped();
            if reset_preferred_x_pos {
                self.preferred_x_pos.set(pos.x);
            }
            Self::FIELD_OFFSETS.cursor_position_changed.apply_pin(self).call(&(pos,));
            self.update_ime_position(window_adapter, self_rc);
        }
    }

    fn update_ime_position(
        self: Pin<&Self>,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) {
        let cursor_position = self.cursor_position(&self.text());
        let cursor_point_relative = window_adapter
            .renderer()
            .text_input_cursor_rect_for_byte_offset(self, cursor_position)
            .to_box2d()
            .max;
        let cursor_point_absolute = self_rc.map_to_window(cursor_point_relative);
        window_adapter.set_ime_position(cursor_point_absolute);
    }

    fn select_and_delete(
        self: Pin<&Self>,
        step: TextCursorDirection,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) {
        if !self.has_selection() {
            self.move_cursor(step, AnchorMode::KeepAnchor, window_adapter, self_rc);
        }
        self.delete_selection(window_adapter, self_rc);
    }

    fn delete_selection(
        self: Pin<&Self>,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) {
        let text: String = self.text().into();
        if text.is_empty() {
            return;
        }

        let (anchor, cursor) = self.selection_anchor_and_cursor();
        if anchor == cursor {
            return;
        }

        let text = [text.split_at(anchor).0, text.split_at(cursor).1].concat();
        self.text.set(text.into());
        self.anchor_position_byte_offset.set(anchor as i32);
        self.set_cursor_position(anchor as i32, true, window_adapter, self_rc);
        Self::FIELD_OFFSETS.edited.apply_pin(self).call(&());
    }

    pub fn anchor_position(self: Pin<&Self>, text: &str) -> usize {
        safe_byte_offset(self.anchor_position_byte_offset(), text)
    }

    pub fn cursor_position(self: Pin<&Self>, text: &str) -> usize {
        safe_byte_offset(self.cursor_position_byte_offset(), text)
    }

    // Avoid accessing self.cursor_position()/self.anchor_position() directly, always
    // use this bounds-checking function.
    pub fn selection_anchor_and_cursor(self: Pin<&Self>) -> (usize, usize) {
        let text = self.text();
        let cursor_pos = self.cursor_position(&text);
        let anchor_pos = self.anchor_position(&text);

        if anchor_pos > cursor_pos {
            (cursor_pos as _, anchor_pos as _)
        } else {
            (anchor_pos as _, cursor_pos as _)
        }
    }

    pub fn has_selection(self: Pin<&Self>) -> bool {
        let (anchor_pos, cursor_pos) = self.selection_anchor_and_cursor();
        anchor_pos != cursor_pos
    }

    fn insert(
        self: Pin<&Self>,
        text_to_insert: &str,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) {
        self.delete_selection(window_adapter, self_rc);
        let mut text: String = self.text().into();
        let cursor_pos = self.selection_anchor_and_cursor().1;
        if text_to_insert.contains('\n') && self.single_line() {
            text.insert_str(cursor_pos, &text_to_insert.replace('\n', " "));
        } else {
            text.insert_str(cursor_pos, text_to_insert);
        }
        let cursor_pos = cursor_pos + text_to_insert.len();
        self.text.set(text.into());
        self.anchor_position_byte_offset.set(cursor_pos as i32);
        self.set_cursor_position(cursor_pos as i32, true, window_adapter, self_rc);
        Self::FIELD_OFFSETS.edited.apply_pin(self).call(&());
    }

    fn select_all(self: Pin<&Self>, window_adapter: &Rc<dyn WindowAdapter>, self_rc: &ItemRc) {
        self.move_cursor(
            TextCursorDirection::StartOfText,
            AnchorMode::MoveAnchor,
            window_adapter,
            self_rc,
        );
        self.move_cursor(
            TextCursorDirection::EndOfText,
            AnchorMode::KeepAnchor,
            window_adapter,
            self_rc,
        );
    }

    fn select_word(self: Pin<&Self>, window_adapter: &Rc<dyn WindowAdapter>, self_rc: &ItemRc) {
        self.move_cursor(
            TextCursorDirection::BackwardByWord,
            AnchorMode::MoveAnchor,
            window_adapter,
            self_rc,
        );
        self.move_cursor(
            TextCursorDirection::ForwardByWord,
            AnchorMode::KeepAnchor,
            window_adapter,
            self_rc,
        );
    }

    fn select_paragraph(
        self: Pin<&Self>,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) {
        self.move_cursor(
            TextCursorDirection::StartOfParagraph,
            AnchorMode::MoveAnchor,
            window_adapter,
            self_rc,
        );
        self.move_cursor(
            TextCursorDirection::EndOfParagraph,
            AnchorMode::KeepAnchor,
            window_adapter,
            self_rc,
        );
    }

    fn copy(self: Pin<&Self>) {
        let (anchor, cursor) = self.selection_anchor_and_cursor();
        if anchor == cursor {
            return;
        }
        let text = self.text();
        crate::platform::PLATFORM_INSTANCE.with(|p| {
            if let Some(backend) = p.get() {
                backend.set_clipboard_text(&text[anchor..cursor]);
            }
        });
    }

    fn paste(self: Pin<&Self>, window_adapter: &Rc<dyn WindowAdapter>, self_rc: &ItemRc) {
        if let Some(text) =
            crate::platform::PLATFORM_INSTANCE.with(|p| p.get().and_then(|p| p.clipboard_text()))
        {
            self.insert(&text, window_adapter, self_rc);
        }
    }

    pub fn font_request(self: Pin<&Self>, window_adapter: &Rc<dyn WindowAdapter>) -> FontRequest {
        let window_item = WindowInner::from_pub(window_adapter.window()).window_item();

        FontRequest {
            family: {
                let maybe_family = self.font_family();
                if !maybe_family.is_empty() {
                    Some(maybe_family)
                } else {
                    window_item.as_ref().and_then(|item| item.as_pin_ref().font_family())
                }
            },
            weight: {
                let weight = self.font_weight();
                if weight == 0 {
                    window_item.as_ref().and_then(|item| item.as_pin_ref().font_weight())
                } else {
                    Some(weight)
                }
            },
            pixel_size: {
                let font_size = self.font_size();
                if font_size.get() == 0 as Coord {
                    window_item.as_ref().and_then(|item| item.as_pin_ref().font_size())
                } else {
                    Some(font_size)
                }
            },
            letter_spacing: Some(self.letter_spacing()),
        }
    }

    pub fn visual_representation(self: Pin<&Self>) -> TextInputVisualRepresentation {
        let mut text: String = self.text().into();

        let preedit_text = self.preedit_text();

        let (preedit_range, selection_range, cursor_position) = if !preedit_text.is_empty() {
            let cursor_position = self.cursor_position(&text);

            text.insert_str(cursor_position, &preedit_text);

            let preedit_selection_start = self.preedit_selection_start() as usize;
            let preedit_selection_end = self.preedit_selection_end() as usize;

            let selection_range =
                cursor_position + preedit_selection_start..cursor_position + preedit_selection_end;

            let preedit_range = cursor_position..cursor_position + preedit_text.len();

            let cursor_position = Some(cursor_position + preedit_selection_end);

            (preedit_range, selection_range, cursor_position)
        } else {
            let preedit_range = Default::default();

            let (selection_anchor_pos, selection_cursor_pos) = self.selection_anchor_and_cursor();
            let selection_range = selection_anchor_pos..selection_cursor_pos;

            let cursor_position = self.cursor_position(&text);
            let cursor_visible = self.cursor_visible() && self.enabled() && !self.read_only();
            let cursor_position = if cursor_visible { Some(cursor_position) } else { None };

            (preedit_range, selection_range, cursor_position)
        };

        TextInputVisualRepresentation { text, preedit_range, selection_range, cursor_position }
    }
}
