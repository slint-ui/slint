/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
This module contains the builtin text related items.

When adding an item or a property, it needs to be kept in sync with different place.
Lookup the [`crate::items`] module documentation.
*/

use super::{Item, ItemConsts, ItemRc, PointArg, PointerEventButton, VoidArg};
use crate::graphics::{Brush, Color, FontRequest, Rect};
use crate::input::{
    FocusEvent, InputEventResult, KeyEvent, KeyEventResult, KeyEventType, KeyboardModifiers,
    MouseEvent,
};
use crate::input::{InputEventFilterResult, InternalKeyCode};
use crate::item_rendering::{CachedRenderingData, ItemRenderer};
use crate::layout::{LayoutInfo, Orientation};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowRc;
use crate::{Callback, Property, SharedString};
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use sixtyfps_corelib_macros::*;

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::EnumString, strum_macros::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum TextHorizontalAlignment {
    left,
    center,
    right,
}

impl Default for TextHorizontalAlignment {
    fn default() -> Self {
        Self::left
    }
}

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::EnumString, strum_macros::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum TextVerticalAlignment {
    top,
    center,
    bottom,
}

impl Default for TextVerticalAlignment {
    fn default() -> Self {
        Self::top
    }
}

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::EnumString, strum_macros::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum TextWrap {
    no_wrap,
    word_wrap,
}

impl Default for TextWrap {
    fn default() -> Self {
        Self::no_wrap
    }
}

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::EnumString, strum_macros::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum TextOverflow {
    clip,
    elide,
}

impl Default for TextOverflow {
    fn default() -> Self {
        Self::clip
    }
}

/// The implementation of the `Text` element
#[repr(C)]
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
pub struct Text {
    pub text: Property<SharedString>,
    pub font_family: Property<SharedString>,
    pub font_size: Property<f32>,
    pub font_weight: Property<i32>,
    pub color: Property<Brush>,
    pub horizontal_alignment: Property<TextHorizontalAlignment>,
    pub vertical_alignment: Property<TextVerticalAlignment>,
    pub wrap: Property<TextWrap>,
    pub overflow: Property<TextOverflow>,
    pub letter_spacing: Property<f32>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Text {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, orientation: Orientation, window: &WindowRc) -> LayoutInfo {
        let implicit_size = |max_width| {
            window.text_size(self.unresolved_font_request(), self.text().as_str(), max_width)
        };

        // Stretch uses `round_layout` to explicitly align the top left and bottom right of layout nodes
        // to pixel boundaries. To avoid rounding down causing the minimum width to become so little that
        // letters will be cut off, apply the ceiling here.
        match orientation {
            Orientation::Horizontal => {
                let implicit_size = implicit_size(None);
                let min = match self.overflow() {
                    TextOverflow::elide => implicit_size
                        .width
                        .min(window.text_size(self.unresolved_font_request(), "…", None).width),
                    TextOverflow::clip => match self.wrap() {
                        TextWrap::no_wrap => implicit_size.width,
                        TextWrap::word_wrap => 0.,
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
                    TextWrap::no_wrap => implicit_size(None).height,
                    TextWrap::word_wrap => implicit_size(Some(self.width())).height,
                }
                .ceil();
                LayoutInfo { min: h, preferred: h, ..LayoutInfo::default() }
            }
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardAndIgnore
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &WindowRc) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &WindowRc) {}

    fn render(self: Pin<&Self>, backend: &mut &mut dyn ItemRenderer) {
        (*backend).draw_text(self)
    }
}

impl ItemConsts for Text {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Text, CachedRenderingData> =
        Text::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

impl Text {
    pub fn unresolved_font_request(self: Pin<&Self>) -> FontRequest {
        FontRequest {
            family: {
                let maybe_family = self.font_family();
                if !maybe_family.is_empty() {
                    Some(maybe_family)
                } else {
                    None
                }
            },
            weight: {
                let weight = self.font_weight();
                if weight == 0 {
                    None
                } else {
                    Some(weight)
                }
            },
            pixel_size: {
                let font_size = self.font_size();
                if font_size == 0.0 {
                    None
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
#[derive(FieldOffsets, Default, SixtyFPSElement)]
#[pin]
pub struct TextInput {
    pub text: Property<SharedString>,
    pub font_family: Property<SharedString>,
    pub font_size: Property<f32>,
    pub font_weight: Property<i32>,
    pub color: Property<Brush>,
    pub selection_foreground_color: Property<Color>,
    pub selection_background_color: Property<Color>,
    pub horizontal_alignment: Property<TextHorizontalAlignment>,
    pub vertical_alignment: Property<TextVerticalAlignment>,
    pub wrap: Property<TextWrap>,
    pub letter_spacing: Property<f32>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cursor_position: Property<i32>, // byte offset,
    pub anchor_position: Property<i32>, // byte offset
    pub text_cursor_width: Property<f32>,
    pub cursor_visible: Property<bool>,
    pub has_focus: Property<bool>,
    pub enabled: Property<bool>,
    pub accepted: Callback<VoidArg>,
    pub cursor_position_changed: Callback<PointArg>,
    pub edited: Callback<VoidArg>,
    pub pressed: std::cell::Cell<bool>,
    pub single_line: Property<bool>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for TextInput {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    // FIXME: width / height.  or maybe it doesn't matter?  (
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, orientation: Orientation, window: &WindowRc) -> LayoutInfo {
        let text = self.text();
        let implicit_size = |max_width| {
            window.text_size(
                self.unresolved_font_request(),
                {
                    if text.is_empty() {
                        "*"
                    } else {
                        text.as_str()
                    }
                },
                max_width,
            )
        };

        // Stretch uses `round_layout` to explicitly align the top left and bottom right of layout nodes
        // to pixel boundaries. To avoid rounding down causing the minimum width to become so little that
        // letters will be cut off, apply the ceiling here.
        match orientation {
            Orientation::Horizontal => {
                let implicit_size = implicit_size(None);
                let min = match self.wrap() {
                    TextWrap::no_wrap => implicit_size.width,
                    TextWrap::word_wrap => 0.,
                };
                LayoutInfo {
                    min: min.ceil(),
                    preferred: implicit_size.width.ceil(),
                    ..LayoutInfo::default()
                }
            }
            Orientation::Vertical => {
                let h = match self.wrap() {
                    TextWrap::no_wrap => implicit_size(None).height,
                    TextWrap::word_wrap => implicit_size(Some(self.width())).height,
                }
                .ceil();
                LayoutInfo { min: h, preferred: h, ..LayoutInfo::default() }
            }
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        window: &WindowRc,
        self_rc: &ItemRc,
    ) -> InputEventResult {
        if !self.enabled() {
            return InputEventResult::EventIgnored;
        }
        match event {
            MouseEvent::MousePressed { pos, button: PointerEventButton::left } => {
                let clicked_offset = window.text_input_byte_offset_for_position(self, pos) as i32;
                self.as_ref().pressed.set(true);
                self.as_ref().anchor_position.set(clicked_offset);
                self.set_cursor_position(clicked_offset, window);
                if !self.has_focus() {
                    window.clone().set_focus_item(self_rc);
                }
            }
            MouseEvent::MouseReleased { button: PointerEventButton::left, .. }
            | MouseEvent::MouseExit => self.as_ref().pressed.set(false),
            MouseEvent::MouseMoved { pos } => {
                if self.as_ref().pressed.get() {
                    let clicked_offset =
                        window.text_input_byte_offset_for_position(self, pos) as i32;
                    self.set_cursor_position(clicked_offset, window);
                }
            }
            _ => return InputEventResult::EventIgnored,
        }
        InputEventResult::EventAccepted
    }

    fn key_event(self: Pin<&Self>, event: &KeyEvent, window: &WindowRc) -> KeyEventResult {
        use std::convert::TryFrom;

        if !self.enabled() {
            return KeyEventResult::EventIgnored;
        }

        match event.event_type {
            KeyEventType::KeyPressed => {
                if let Some(keycode) = InternalKeyCode::try_decode_from_string(&event.text) {
                    if let Ok(text_cursor_movement) = TextCursorDirection::try_from(keycode.clone())
                    {
                        TextInput::move_cursor(
                            self,
                            text_cursor_movement,
                            event.modifiers.into(),
                            window,
                        );
                        return KeyEventResult::EventAccepted;
                    } else if keycode == InternalKeyCode::Back {
                        TextInput::delete_previous(self, window);
                        return KeyEventResult::EventAccepted;
                    } else if keycode == InternalKeyCode::Delete {
                        TextInput::delete_char(self, window);
                        return KeyEventResult::EventAccepted;
                    } else if keycode == InternalKeyCode::Return && self.single_line() {
                        Self::FIELD_OFFSETS.accepted.apply_pin(self).call(&());
                        return KeyEventResult::EventAccepted;
                    }
                }

                // Only insert/interpreter non-control character strings
                if event.text.is_empty()
                    || event.text.as_str().chars().any(|ch| ch.is_control() && ch != '\n')
                {
                    return KeyEventResult::EventIgnored;
                }
                if event.modifiers.control {
                    if event.text == "a" {
                        self.select_all(window);
                        return KeyEventResult::EventAccepted;
                    } else if event.text == "c" {
                        self.copy();
                        return KeyEventResult::EventAccepted;
                    } else if event.text == "v" {
                        self.paste(window);
                        return KeyEventResult::EventAccepted;
                    }
                    return KeyEventResult::EventIgnored;
                }
                self.delete_selection(window);

                let mut text: String = self.text().into();

                // FIXME: respect grapheme boundaries
                let insert_pos = self.selection_anchor_and_cursor().1;
                text.insert_str(insert_pos, &event.text);

                self.as_ref().text.set(text.into());
                let new_cursor_pos = (insert_pos + event.text.len()) as i32;
                self.as_ref().anchor_position.set(new_cursor_pos);
                self.set_cursor_position(new_cursor_pos, window);

                // Keep the cursor visible when inserting text. Blinking should only occur when
                // nothing is entered or the cursor isn't moved.
                self.as_ref().show_cursor(window);

                Self::FIELD_OFFSETS.edited.apply_pin(self).call(&());

                KeyEventResult::EventAccepted
            }
            _ => KeyEventResult::EventIgnored,
        }
    }

    fn focus_event(self: Pin<&Self>, event: &FocusEvent, window: &WindowRc) {
        match event {
            FocusEvent::FocusIn | FocusEvent::WindowReceivedFocus => {
                self.has_focus.set(true);
                self.show_cursor(window);
            }
            FocusEvent::FocusOut | FocusEvent::WindowLostFocus => {
                self.has_focus.set(false);
                self.hide_cursor()
            }
        }
    }

    fn render(self: Pin<&Self>, backend: &mut &mut dyn ItemRenderer) {
        (*backend).draw_text_input(self)
    }
}

impl ItemConsts for TextInput {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        TextInput,
        CachedRenderingData,
    > = TextInput::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

enum TextCursorDirection {
    Forward,
    Backward,
    PreviousCharacter, // breaks grapheme boundaries, so only used by delete-previous-char
    StartOfLine,
    EndOfLine,
    StartOfText,
    EndOfText,
}

impl std::convert::TryFrom<InternalKeyCode> for TextCursorDirection {
    type Error = ();

    fn try_from(value: InternalKeyCode) -> Result<Self, Self::Error> {
        Ok(match value {
            InternalKeyCode::Left => Self::Backward,
            InternalKeyCode::Right => Self::Forward,
            InternalKeyCode::Home => Self::StartOfLine,
            InternalKeyCode::End => Self::EndOfLine,
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

impl TextInput {
    fn show_cursor(&self, window: &WindowRc) {
        window.set_cursor_blink_binding(&self.cursor_visible);
    }

    fn hide_cursor(&self) {
        self.cursor_visible.set(false);
    }

    fn move_cursor(
        self: Pin<&Self>,
        direction: TextCursorDirection,
        anchor_mode: AnchorMode,
        window: &WindowRc,
    ) -> bool {
        let text = self.text();
        if text.is_empty() {
            return false;
        }

        let last_cursor_pos = (self.cursor_position() as usize).max(0).min(text.len());

        let mut grapheme_cursor =
            unicode_segmentation::GraphemeCursor::new(last_cursor_pos, text.len(), true);

        let new_cursor_pos = match direction {
            TextCursorDirection::Forward => {
                grapheme_cursor.next_boundary(&text, 0).ok().flatten().unwrap_or_else(|| text.len())
            }
            TextCursorDirection::Backward => {
                grapheme_cursor.prev_boundary(&text, 0).ok().flatten().unwrap_or(0)
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
            // FIXME: StartOfLine and EndOfLine should respect line boundaries
            TextCursorDirection::StartOfLine => 0,
            TextCursorDirection::EndOfLine => text.len(),
            TextCursorDirection::StartOfText => 0,
            TextCursorDirection::EndOfText => text.len(),
        };

        match anchor_mode {
            AnchorMode::KeepAnchor => {}
            AnchorMode::MoveAnchor => {
                self.as_ref().anchor_position.set(new_cursor_pos as i32);
            }
        }
        self.set_cursor_position(new_cursor_pos as i32, window);

        // Keep the cursor visible when moving. Blinking should only occur when
        // nothing is entered or the cursor isn't moved.
        self.as_ref().show_cursor(window);

        new_cursor_pos != last_cursor_pos
    }

    fn set_cursor_position(self: Pin<&Self>, new_position: i32, window: &WindowRc) {
        self.cursor_position.set(new_position);
        if new_position >= 0 {
            let pos = window.text_input_position_for_byte_offset(self, new_position as usize);
            Self::FIELD_OFFSETS.cursor_position_changed.apply_pin(self).call(&(pos,));
        }
    }

    fn delete_char(self: Pin<&Self>, window: &WindowRc) {
        if !self.has_selection() {
            self.move_cursor(TextCursorDirection::Forward, AnchorMode::KeepAnchor, window);
        }
        self.delete_selection(window);
    }

    fn delete_previous(self: Pin<&Self>, window: &WindowRc) {
        if self.has_selection() {
            self.delete_selection(window);
            return;
        }
        if self.move_cursor(TextCursorDirection::PreviousCharacter, AnchorMode::MoveAnchor, window)
        {
            self.delete_char(window);
        }
    }

    fn delete_selection(self: Pin<&Self>, window: &WindowRc) {
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
        self.anchor_position.set(anchor as i32);
        self.set_cursor_position(anchor as i32, window);
        Self::FIELD_OFFSETS.edited.apply_pin(self).call(&());
    }

    // Avoid accessing self.cursor_position()/self.anchor_position() directly, always
    // use this bounds-checking function.
    pub fn selection_anchor_and_cursor(self: Pin<&Self>) -> (usize, usize) {
        let max_pos = self.text().len() as i32;
        let cursor_pos = self.cursor_position().max(0).min(max_pos);
        let anchor_pos = self.anchor_position().max(0).min(max_pos);

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

    fn selected_text(self: Pin<&Self>) -> String {
        let (anchor, cursor) = self.selection_anchor_and_cursor();
        let text: String = self.text().into();
        text.split_at(anchor).1.split_at(cursor - anchor).0.to_string()
    }

    fn insert(self: Pin<&Self>, text_to_insert: &str, window: &WindowRc) {
        self.delete_selection(window);
        let mut text: String = self.text().into();
        let cursor_pos = self.selection_anchor_and_cursor().1;
        if text_to_insert.contains('\n') && self.single_line() {
            text.insert_str(cursor_pos, &text_to_insert.replace('\n', " "));
        } else {
            text.insert_str(cursor_pos, text_to_insert);
        }
        let cursor_pos = cursor_pos + text_to_insert.len();
        self.text.set(text.into());
        self.anchor_position.set(cursor_pos as i32);
        self.set_cursor_position(cursor_pos as i32, window);
        Self::FIELD_OFFSETS.edited.apply_pin(self).call(&());
    }

    fn select_all(self: Pin<&Self>, window: &WindowRc) {
        self.move_cursor(TextCursorDirection::StartOfText, AnchorMode::MoveAnchor, window);
        self.move_cursor(TextCursorDirection::EndOfText, AnchorMode::KeepAnchor, window);
    }

    fn copy(self: Pin<&Self>) {
        if let Some(backend) = crate::backend::instance() {
            backend.set_clipboard_text(self.selected_text());
        }
    }

    fn paste(self: Pin<&Self>, window: &WindowRc) {
        if let Some(text) = crate::backend::instance().and_then(|backend| backend.clipboard_text())
        {
            self.insert(&text, window);
        }
    }

    pub fn unresolved_font_request(self: Pin<&Self>) -> FontRequest {
        FontRequest {
            family: {
                let maybe_family = self.font_family();
                if !maybe_family.is_empty() {
                    Some(maybe_family)
                } else {
                    None
                }
            },
            weight: {
                let weight = self.font_weight();
                if weight == 0 {
                    None
                } else {
                    Some(weight)
                }
            },
            pixel_size: {
                let font_size = self.font_size();
                if font_size == 0.0 {
                    None
                } else {
                    Some(font_size)
                }
            },
            letter_spacing: Some(self.letter_spacing()),
        }
    }
}
