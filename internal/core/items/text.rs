// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*!
This module contains the builtin text related items.

When adding an item or a property, it needs to be kept in sync with different place.
Lookup the [`crate::items`] module documentation.
*/

use super::{Item, ItemConsts, ItemRc, PointArg, PointerEventButton, RenderingResult, VoidArg};
use crate::graphics::{Brush, Color, FontRequest, Rect};
use crate::input::{
    key_codes, FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent,
    KeyEventResult, KeyEventType, KeyboardModifiers, MouseEvent,
};
use crate::item_rendering::{CachedRenderingData, ItemRenderer};
use crate::layout::{LayoutInfo, Orientation};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowRc;
use crate::{Callback, Property, SharedString};
use alloc::string::String;
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use i_slint_core_macros::*;

#[cfg(not(feature = "std"))]
use num_traits::float::Float;

/// This enum defines the input type in a text input which for now only distinguishes a normal
/// input from a password input
#[derive(Copy, Clone, Debug, PartialEq, strum::EnumString, strum::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum InputType {
    /// This type is used for a normal text input
    text,
    /// This type is used for password inputs where the characters are represented as *'s
    password,
}
impl Default for InputType {
    fn default() -> Self {
        Self::text
    }
}

#[derive(Copy, Clone, Debug, PartialEq, strum::EnumString, strum::Display)]
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

#[derive(Copy, Clone, Debug, PartialEq, strum::EnumString, strum::Display)]
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

#[derive(Copy, Clone, Debug, PartialEq, strum::EnumString, strum::Display)]
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

#[derive(Copy, Clone, Debug, PartialEq, strum::EnumString, strum::Display)]
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
#[derive(FieldOffsets, Default, SlintElement)]
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

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &WindowRc) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn render(
        self: Pin<&Self>,
        backend: &mut &mut dyn ItemRenderer,
        _self_rc: &ItemRc,
    ) -> RenderingResult {
        (*backend).draw_text(self);
        RenderingResult::ContinueRenderingChildren
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
#[derive(FieldOffsets, Default, SlintElement)]
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
    pub input_type: Property<InputType>,
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
    pub pressed: core::cell::Cell<bool>,
    pub single_line: Property<bool>,
    pub cached_rendering_data: CachedRenderingData,
    // The x position where the cursor wants to be.
    // It is not updated when moving up and down even when the line is shorter.
    preferred_x_pos: core::cell::Cell<f32>,
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
                self.set_cursor_position(clicked_offset, true, window);
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
                    self.set_cursor_position(clicked_offset, true, window);
                }
            }
            _ => return InputEventResult::EventIgnored,
        }
        InputEventResult::EventAccepted
    }

    fn key_event(self: Pin<&Self>, event: &KeyEvent, window: &WindowRc) -> KeyEventResult {
        if !self.enabled() {
            return KeyEventResult::EventIgnored;
        }

        match event.event_type {
            KeyEventType::KeyPressed => {
                match event.text_shortcut() {
                    Some(text_shortcut) => match text_shortcut {
                        TextShortcut::Move(direction) => {
                            TextInput::move_cursor(self, direction, event.modifiers.into(), window);
                            return KeyEventResult::EventAccepted;
                        }
                        TextShortcut::DeleteForward => {
                            TextInput::delete_char(self, window);
                            return KeyEventResult::EventAccepted;
                        }
                        TextShortcut::DeleteBackward => {
                            TextInput::delete_previous(self, window);
                            return KeyEventResult::EventAccepted;
                        }
                    },
                    None => (),
                };

                if let Some(keycode) = event.text.chars().next() {
                    if keycode == key_codes::Return && self.single_line() {
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
                    } else if event.text == "x" {
                        self.copy();
                        self.delete_selection(window);
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
                self.set_cursor_position(new_cursor_pos, true, window);

                // Keep the cursor visible when inserting text. Blinking should only occur when
                // nothing is entered or the cursor isn't moved.
                self.as_ref().show_cursor(window);

                Self::FIELD_OFFSETS.edited.apply_pin(self).call(&());

                KeyEventResult::EventAccepted
            }
            _ => KeyEventResult::EventIgnored,
        }
    }

    fn focus_event(self: Pin<&Self>, event: &FocusEvent, window: &WindowRc) -> FocusEventResult {
        match event {
            FocusEvent::FocusIn | FocusEvent::WindowReceivedFocus => {
                self.has_focus.set(true);
                self.show_cursor(window);
                window.show_virtual_keyboard(self.input_type());
            }
            FocusEvent::FocusOut | FocusEvent::WindowLostFocus => {
                self.has_focus.set(false);
                self.hide_cursor();
                window.hide_virtual_keyboard();
            }
        }
        FocusEventResult::FocusAccepted
    }

    fn render(
        self: Pin<&Self>,
        backend: &mut &mut dyn ItemRenderer,
        _self_rc: &ItemRc,
    ) -> RenderingResult {
        (*backend).draw_text_input(self);
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for TextInput {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        TextInput,
        CachedRenderingData,
    > = TextInput::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

impl KeyEvent {
    fn text_shortcut(&self) -> Option<TextShortcut> {
        let keycode = self.text.chars().next()?;

        let by_word = if cfg!(target_os = "macos") {
            self.modifiers.alt && !self.modifiers.control && !self.modifiers.meta
        } else {
            self.modifiers.control && !self.modifiers.alt && !self.modifiers.meta
        };
        match TextCursorDirection::try_from(keycode) {
            Ok(TextCursorDirection::Forward) => {
                if by_word {
                    // return Some(TextShortcut::Move(TextCursorDirection::ForwardByWord));
                } else {
                    return Some(TextShortcut::Move(TextCursorDirection::Forward));
                }
            }
            Ok(TextCursorDirection::Backward) => {
                if by_word {
                    // return Some(TextShortcut::Move(TextCursorDirection::BackwardByWord));
                } else {
                    return Some(TextShortcut::Move(TextCursorDirection::Backward));
                }
            }
            Ok(direction) => return Some(TextShortcut::Move(direction)),
            _ => (),
        };

        match keycode {
            key_codes::Backspace => Some(TextShortcut::DeleteBackward),
            key_codes::Delete => Some(TextShortcut::DeleteForward),
            _ => None,
        }
    }
}

enum TextShortcut {
    Move(TextCursorDirection),
    DeleteForward,
    DeleteBackward,
}

enum TextCursorDirection {
    Forward,
    Backward,
    // ForwardByWord,
    // BackwardByWord,
    NextLine,
    PreviousLine,
    PreviousCharacter, // breaks grapheme boundaries, so only used by delete-previous-char
    StartOfLine,
    EndOfLine,
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
            key_codes::Home => Self::StartOfLine,
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

        let font_height = window.text_size(self.unresolved_font_request(), " ", None).height;

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
                    window.text_input_cursor_rect_for_byte_offset(self, last_cursor_pos);
                let mut cursor_xy_pos = cursor_rect.center();

                cursor_xy_pos.y += font_height;
                cursor_xy_pos.x = self.preferred_x_pos.get();
                window.text_input_byte_offset_for_position(self, cursor_xy_pos)
            }
            TextCursorDirection::PreviousLine => {
                reset_preferred_x_pos = false;

                let cursor_rect =
                    window.text_input_cursor_rect_for_byte_offset(self, last_cursor_pos);
                let mut cursor_xy_pos = cursor_rect.center();

                cursor_xy_pos.y -= font_height;
                cursor_xy_pos.x = self.preferred_x_pos.get();
                window.text_input_byte_offset_for_position(self, cursor_xy_pos)
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
        self.set_cursor_position(new_cursor_pos as i32, reset_preferred_x_pos, window);

        // Keep the cursor visible when moving. Blinking should only occur when
        // nothing is entered or the cursor isn't moved.
        self.as_ref().show_cursor(window);

        new_cursor_pos != last_cursor_pos
    }

    fn set_cursor_position(
        self: Pin<&Self>,
        new_position: i32,
        reset_preferred_x_pos: bool,
        window: &WindowRc,
    ) {
        self.cursor_position.set(new_position);
        if new_position >= 0 {
            let pos =
                window.text_input_cursor_rect_for_byte_offset(self, new_position as usize).origin;
            if reset_preferred_x_pos {
                self.preferred_x_pos.set(pos.x);
            }
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
        self.set_cursor_position(anchor as i32, true, window);
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
        text.split_at(anchor).1.split_at(cursor - anchor).0.into()
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
        self.set_cursor_position(cursor_pos as i32, true, window);
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
