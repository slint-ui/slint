/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
This module contains the builtin text related items.

When adding an item or a property, it needs to be kept in sync with different place.
(This is less than ideal and maybe we can have some automation later)

 - It needs to be changed in this module
 - In the compiler: builtins.60
 - In the interpreter: dynamic_component.rs
 - For the C++ code (new item only): the cbindgen.rs to export the new item, and the `using` declaration in sixtyfps.h
 - Don't forget to update the documentation
*/

use super::{Item, ItemConsts, ItemRc};
use crate::graphics::{Color, Point, Rect, Size};
use crate::input::{
    FocusEvent, InputEventResult, KeyEvent, KeyEventResult, KeyboardModifiers, MouseEvent,
    MouseEventType,
};
use crate::item_rendering::{CachedRenderingData, ItemRenderer};
use crate::layout::LayoutInfo;
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::ComponentWindow;
use crate::{Callback, Property, SharedString};
use const_field_offset::FieldOffsets;
use core::pin::Pin;
use sixtyfps_corelib_macros::*;

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::EnumString, strum_macros::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum TextHorizontalAlignment {
    align_left,
    align_center,
    align_right,
}

impl Default for TextHorizontalAlignment {
    fn default() -> Self {
        Self::align_left
    }
}

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::EnumString, strum_macros::Display)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum TextVerticalAlignment {
    align_top,
    align_center,
    align_bottom,
}

impl Default for TextVerticalAlignment {
    fn default() -> Self {
        Self::align_top
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
    pub color: Property<Color>,
    pub horizontal_alignment: Property<TextHorizontalAlignment>,
    pub vertical_alignment: Property<TextVerticalAlignment>,
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Text {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    // FIXME: width / height.  or maybe it doesn't matter?  (
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layouting_info(self: Pin<&Self>, window: &ComponentWindow) -> LayoutInfo {
        let text = self.text();

        if let Some(font_metrics) = window.0.font_metrics(self.font_request()) {
            let width = font_metrics.text_width(&text);
            let height = font_metrics.height();
            LayoutInfo { min_width: width, min_height: height, ..LayoutInfo::default() }
        } else {
            LayoutInfo::default()
        }
    }

    fn implicit_size(self: Pin<&Self>, window: &ComponentWindow) -> Size {
        window
            .0
            .font_metrics(self.font_request())
            .map(|metrics| euclid::size2(metrics.text_width(&self.text()), metrics.height()))
            .unwrap_or_default()
    }

    fn input_event(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &ComponentWindow,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        InputEventResult::EventIgnored
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &ComponentWindow) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &ComponentWindow) {}

    fn render(self: Pin<&Self>, pos: Point, backend: &mut &mut dyn ItemRenderer) {
        (*backend).draw_text(pos, self)
    }
}

impl ItemConsts for Text {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Text, CachedRenderingData> =
        Text::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

impl Text {
    pub fn font_request(self: Pin<&Self>) -> crate::graphics::FontRequest {
        crate::graphics::FontRequest {
            family: self.font_family(),
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
    pub color: Property<Color>,
    pub selection_foreground_color: Property<Color>,
    pub selection_background_color: Property<Color>,
    pub horizontal_alignment: Property<TextHorizontalAlignment>,
    pub vertical_alignment: Property<TextVerticalAlignment>,
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
    pub accepted: Callback<()>,
    pub edited: Callback<()>,
    pub pressed: std::cell::Cell<bool>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for TextInput {
    fn init(self: Pin<&Self>, _window: &ComponentWindow) {}

    // FIXME: width / height.  or maybe it doesn't matter?  (
    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layouting_info(self: Pin<&Self>, window: &ComponentWindow) -> LayoutInfo {
        if let Some(font_metrics) = window.0.font_metrics(self.font_request()) {
            let width = font_metrics.text_width("********************");
            let height = font_metrics.height();

            LayoutInfo {
                min_width: width,
                min_height: height,
                horizontal_stretch: 1.,
                ..LayoutInfo::default()
            }
        } else {
            LayoutInfo::default()
        }
    }

    fn implicit_size(self: Pin<&Self>, window: &ComponentWindow) -> Size {
        window
            .0
            .font_metrics(self.font_request())
            .map(|metrics| euclid::size2(metrics.text_width(&self.text()), metrics.height()))
            .unwrap_or_default()
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        window: &ComponentWindow,
        self_rc: &ItemRc,
    ) -> InputEventResult {
        if !self.enabled() {
            return InputEventResult::EventIgnored;
        }

        let text = self.text();
        let font_metrics = match window.0.font_metrics(self.font_request()) {
            Some(font) => font,
            None => return InputEventResult::EventIgnored,
        };
        let clicked_offset = font_metrics.text_offset_for_x_position(&text, event.pos.x) as i32;

        if matches!(event.what, MouseEventType::MousePressed) {
            self.as_ref().pressed.set(true);
            self.as_ref().anchor_position.set(clicked_offset);
            self.as_ref().cursor_position.set(clicked_offset);
            if !self.has_focus() {
                window.set_focus_item(self_rc);
            }
        }

        match event.what {
            MouseEventType::MouseReleased => {
                self.as_ref().pressed.set(false);
            }
            MouseEventType::MouseMoved if self.as_ref().pressed.get() => {
                self.as_ref().cursor_position.set(clicked_offset);
            }
            _ => {}
        }

        InputEventResult::EventAccepted
    }

    fn key_event(self: Pin<&Self>, event: &KeyEvent, window: &ComponentWindow) -> KeyEventResult {
        use std::convert::TryFrom;

        if !self.enabled() {
            return KeyEventResult::EventIgnored;
        }

        match event {
            KeyEvent::CharacterInput { unicode_scalar, .. } => {
                self.delete_selection();

                let mut text: String = self.text().into();

                // FIXME: respect grapheme boundaries
                let insert_pos = self.cursor_position() as usize;
                let ch = char::try_from(*unicode_scalar).unwrap().to_string();
                text.insert_str(insert_pos, &ch);

                self.as_ref().text.set(text.into());
                let new_cursor_pos = (insert_pos + ch.len()) as i32;
                self.as_ref().cursor_position.set(new_cursor_pos);
                self.as_ref().anchor_position.set(new_cursor_pos);

                // Keep the cursor visible when inserting text. Blinking should only occur when
                // nothing is entered or the cursor isn't moved.
                self.as_ref().show_cursor(window);

                Self::FIELD_OFFSETS.edited.apply_pin(self).emit(&());

                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyPressed { code, modifiers } if *code == crate::input::KeyCode::Right => {
                TextInput::move_cursor(
                    self,
                    TextCursorDirection::Forward,
                    (*modifiers).into(),
                    window,
                );
                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyPressed { code, modifiers } if *code == crate::input::KeyCode::Left => {
                TextInput::move_cursor(
                    self,
                    TextCursorDirection::Backward,
                    (*modifiers).into(),
                    window,
                );
                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyPressed { code, modifiers } if *code == crate::input::KeyCode::Home => {
                TextInput::move_cursor(
                    self,
                    TextCursorDirection::StartOfLine,
                    (*modifiers).into(),
                    window,
                );
                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyPressed { code, modifiers } if *code == crate::input::KeyCode::End => {
                TextInput::move_cursor(
                    self,
                    TextCursorDirection::EndOfLine,
                    (*modifiers).into(),
                    window,
                );
                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyPressed { code, .. } if *code == crate::input::KeyCode::Back => {
                TextInput::delete_previous(self, window);
                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyPressed { code, .. } if *code == crate::input::KeyCode::Delete => {
                TextInput::delete_char(self, window);
                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyPressed { code, .. } if *code == crate::input::KeyCode::Return => {
                Self::FIELD_OFFSETS.accepted.apply_pin(self).emit(&());
                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyReleased { code, modifiers }
                if modifiers.test_exclusive(crate::input::COPY_PASTE_MODIFIER)
                    && *code == crate::input::KeyCode::C =>
            {
                self.copy();
                KeyEventResult::EventAccepted
            }
            KeyEvent::KeyReleased { code, modifiers }
                if modifiers.test_exclusive(crate::input::COPY_PASTE_MODIFIER)
                    && *code == crate::input::KeyCode::V =>
            {
                self.paste();
                KeyEventResult::EventAccepted
            }
            _ => KeyEventResult::EventIgnored,
        }
    }

    fn focus_event(self: Pin<&Self>, event: &FocusEvent, window: &ComponentWindow) {
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

    fn render(self: Pin<&Self>, pos: Point, backend: &mut &mut dyn ItemRenderer) {
        (*backend).draw_text_input(pos, self)
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
    StartOfLine,
    EndOfLine,
}

enum AnchorMode {
    KeepAnchor,
    MoveAnchor,
}

impl From<KeyboardModifiers> for AnchorMode {
    fn from(modifiers: KeyboardModifiers) -> Self {
        if modifiers.shift() {
            Self::KeepAnchor
        } else {
            Self::MoveAnchor
        }
    }
}

impl TextInput {
    fn show_cursor(&self, window: &ComponentWindow) {
        window.set_cursor_blink_binding(&self.cursor_visible);
    }

    fn hide_cursor(&self) {
        self.cursor_visible.set(false);
    }

    fn move_cursor(
        self: Pin<&Self>,
        direction: TextCursorDirection,
        anchor_mode: AnchorMode,
        window: &ComponentWindow,
    ) -> bool {
        let text = self.text();
        if text.len() == 0 {
            return false;
        }

        let last_cursor_pos = self.cursor_position() as usize;

        let new_cursor_pos = match direction {
            TextCursorDirection::Forward => {
                let mut i = last_cursor_pos;
                loop {
                    i = i.checked_add(1).unwrap_or_default().min(text.len());
                    if text.is_char_boundary(i) {
                        break i;
                    }
                }
            }
            TextCursorDirection::Backward => {
                let mut i = last_cursor_pos;
                loop {
                    i = i.checked_sub(1).unwrap_or_default();
                    if text.is_char_boundary(i) {
                        break i;
                    }
                }
            }
            TextCursorDirection::StartOfLine => 0,
            TextCursorDirection::EndOfLine => text.len(),
        };

        self.as_ref().cursor_position.set(new_cursor_pos as i32);

        match anchor_mode {
            AnchorMode::KeepAnchor => {}
            AnchorMode::MoveAnchor => {
                self.as_ref().anchor_position.set(new_cursor_pos as i32);
            }
        }

        // Keep the cursor visible when moving. Blinking should only occur when
        // nothing is entered or the cursor isn't moved.
        self.as_ref().show_cursor(window);

        new_cursor_pos != last_cursor_pos
    }

    fn delete_char(self: Pin<&Self>, window: &ComponentWindow) {
        if !self.has_selection() {
            self.move_cursor(TextCursorDirection::Forward, AnchorMode::KeepAnchor, window);
        }
        self.delete_selection();
    }

    fn delete_previous(self: Pin<&Self>, window: &ComponentWindow) {
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        if self.move_cursor(TextCursorDirection::Backward, AnchorMode::MoveAnchor, window) {
            self.delete_char(window);
        }
    }

    fn delete_selection(self: Pin<&Self>) {
        let text: String = self.text().into();
        if text.len() == 0 {
            return;
        }

        let (anchor, cursor) = self.selection_anchor_and_cursor();
        if anchor == cursor {
            return;
        }

        let text = [text.split_at(anchor).0, text.split_at(cursor).1].concat();
        self.cursor_position.set(anchor as i32);
        self.anchor_position.set(anchor as i32);
        self.text.set(text.into());
        Self::FIELD_OFFSETS.edited.apply_pin(self).emit(&());
    }

    pub fn selection_anchor_and_cursor(self: Pin<&Self>) -> (usize, usize) {
        let cursor_pos = self.cursor_position().max(0);
        let anchor_pos = self.anchor_position().max(0);

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

    fn insert(self: Pin<&Self>, text_to_insert: &str) {
        self.delete_selection();
        let mut text: String = self.text().into();
        let cursor_pos = self.selection_anchor_and_cursor().1;
        text.insert_str(cursor_pos, text_to_insert);
        let cursor_pos = cursor_pos + text_to_insert.len();
        self.cursor_position.set(cursor_pos as i32);
        self.anchor_position.set(cursor_pos as i32);
        self.text.set(text.into());
        Self::FIELD_OFFSETS.edited.apply_pin(self).emit(&());
    }

    fn copy(self: Pin<&Self>) {
        use copypasta::ClipboardProvider;
        CLIPBOARD.with(|clipboard| clipboard.borrow_mut().set_contents(self.selected_text()).ok());
    }

    fn paste(self: Pin<&Self>) {
        use copypasta::ClipboardProvider;
        if let Some(text) = CLIPBOARD.with(|clipboard| clipboard.borrow_mut().get_contents().ok()) {
            self.insert(&text);
        }
    }

    pub fn font_request(self: Pin<&Self>) -> crate::graphics::FontRequest {
        crate::graphics::FontRequest {
            family: self.font_family(),
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
        }
    }
}

thread_local!(pub(crate) static CLIPBOARD : std::cell::RefCell<copypasta::ClipboardContext> = std::cell::RefCell::new(copypasta::ClipboardContext::new().unwrap()));
