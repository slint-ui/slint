// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

/*!
This module contains the builtin text related items.

When adding an item or a property, it needs to be kept in sync with different place.
Lookup the [`crate::items`] module documentation.
*/
use super::{
    InputType, Item, ItemConsts, ItemRc, KeyEventResult, KeyEventType, PointArg,
    PointerEventButton, RenderingResult, TextHorizontalAlignment, TextOverflow, TextStrokeStyle,
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
use crate::platform::Clipboard;
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::{InputMethodProperties, InputMethodRequest, WindowAdapter, WindowInner};
use crate::{Callback, Coord, Property, SharedString, SharedVector};
use alloc::rc::Rc;
#[cfg(not(feature = "std"))]
use alloc::string::String;
use const_field_offset::FieldOffsets;
use core::cell::Cell;
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
    pub font_italic: Property<bool>,
    pub color: Property<Brush>,
    pub horizontal_alignment: Property<TextHorizontalAlignment>,
    pub vertical_alignment: Property<TextVerticalAlignment>,
    pub wrap: Property<TextWrap>,
    pub overflow: Property<TextOverflow>,
    pub letter_spacing: Property<LogicalLength>,
    pub stroke: Property<Brush>,
    pub stroke_width: Property<LogicalLength>,
    pub stroke_style: Property<TextStrokeStyle>,
    pub width: Property<LogicalLength>,
    pub height: Property<LogicalLength>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Text {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

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
        size: LogicalSize,
    ) -> RenderingResult {
        (*backend).draw_text(self, self_rc, size);
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
            italic: self.font_italic(),
        }
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy, PartialEq)]
/// Similar as `Option<core::ops::Range<i32>>` but `repr(C)`
///
/// This is the selection within a preedit
struct PreEditSelection {
    valid: bool,
    start: i32,
    end: i32,
}

impl From<Option<core::ops::Range<i32>>> for PreEditSelection {
    fn from(value: Option<core::ops::Range<i32>>) -> Self {
        value.map_or_else(Default::default, |r| Self { valid: true, start: r.start, end: r.end })
    }
}

impl PreEditSelection {
    fn as_option(self) -> Option<core::ops::Range<i32>> {
        self.valid.then_some(self.start..self.end)
    }
}

#[repr(C)]
#[derive(Clone)]
enum UndoItemKind {
    TextInsert,
    TextRemove,
}

#[repr(C)]
#[derive(Clone)]
struct UndoItem {
    pos: usize,
    text: SharedString,
    cursor: usize,
    anchor: usize,
    kind: UndoItemKind,
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
    pub font_italic: Property<bool>,
    pub color: Property<Brush>,
    pub selection_foreground_color: Property<Color>,
    pub selection_background_color: Property<Color>,
    pub horizontal_alignment: Property<TextHorizontalAlignment>,
    pub vertical_alignment: Property<TextVerticalAlignment>,
    pub wrap: Property<TextWrap>,
    pub input_type: Property<InputType>,
    pub letter_spacing: Property<LogicalLength>,
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
    pub single_line: Property<bool>,
    pub read_only: Property<bool>,
    pub preedit_text: Property<SharedString>,
    /// A selection within the preedit (cursor and anchor)
    preedit_selection: Property<PreEditSelection>,
    pub cached_rendering_data: CachedRenderingData,
    // The x position where the cursor wants to be.
    // It is not updated when moving up and down even when the line is shorter.
    preferred_x_pos: Cell<Coord>,
    /// 0 = not pressed, 1 = single press, 2 = double clicked+press , ...
    pressed: Cell<u8>,
    undo_items: Cell<SharedVector<UndoItem>>,
    redo_items: Cell<SharedVector<UndoItem>>,
}

impl Item for TextInput {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

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
                let clicked_offset = self.byte_offset_for_position(position, window_adapter) as i32;
                self.as_ref().pressed.set((click_count % 3) + 1);

                if !window_adapter.window().0.modifiers.get().shift() {
                    self.as_ref().anchor_position_byte_offset.set(clicked_offset);
                }

                #[cfg(not(target_os = "android"))]
                self.ensure_focus_and_ime(window_adapter, self_rc);

                match click_count % 3 {
                    0 => self.set_cursor_position(
                        clicked_offset,
                        true,
                        TextChangeNotify::TriggerCallbacks,
                        window_adapter,
                        self_rc,
                    ),
                    1 => self.select_word(window_adapter, self_rc),
                    2 => self.select_paragraph(window_adapter, self_rc),
                    _ => unreachable!(),
                };

                return InputEventResult::GrabMouse;
            }
            MouseEvent::Pressed { .. } => {
                #[cfg(not(target_os = "android"))]
                self.ensure_focus_and_ime(window_adapter, self_rc);
            }
            MouseEvent::Released { button: PointerEventButton::Left, .. } => {
                self.as_ref().pressed.set(0);
                self.copy_clipboard(window_adapter, Clipboard::SelectionClipboard);
                #[cfg(target_os = "android")]
                self.ensure_focus_and_ime(window_adapter, self_rc);
            }
            MouseEvent::Released { position, button: PointerEventButton::Middle, .. } => {
                let clicked_offset = self.byte_offset_for_position(position, window_adapter) as i32;
                self.as_ref().anchor_position_byte_offset.set(clicked_offset);
                self.set_cursor_position(
                    clicked_offset,
                    true,
                    // We trigger the callbacks because paste_clipboard might not if there is no clipboard
                    TextChangeNotify::TriggerCallbacks,
                    window_adapter,
                    self_rc,
                );
                self.paste_clipboard(window_adapter, self_rc, Clipboard::SelectionClipboard);
            }
            MouseEvent::Exit => {
                if let Some(x) = window_adapter.internal(crate::InternalToken) {
                    x.set_mouse_cursor(super::MouseCursor::Default);
                }
                self.as_ref().pressed.set(0)
            }
            MouseEvent::Moved { position } => {
                if let Some(x) = window_adapter.internal(crate::InternalToken) {
                    x.set_mouse_cursor(super::MouseCursor::Text);
                }
                let pressed = self.as_ref().pressed.get();
                if pressed > 0 {
                    let clicked_offset =
                        self.byte_offset_for_position(position, window_adapter) as i32;
                    self.set_cursor_position(
                        clicked_offset,
                        true,
                        if (pressed - 1) % 3 == 0 {
                            TextChangeNotify::TriggerCallbacks
                        } else {
                            TextChangeNotify::SkipCallbacks
                        },
                        window_adapter,
                        self_rc,
                    );
                    match (pressed - 1) % 3 {
                        0 => (),
                        1 => self.select_word(window_adapter, self_rc),
                        2 => self.select_paragraph(window_adapter, self_rc),
                        _ => unreachable!(),
                    }
                    return InputEventResult::GrabMouse;
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
                                TextChangeNotify::TriggerCallbacks,
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

                if let Some(shortcut) = event.shortcut() {
                    match shortcut {
                        StandardShortcut::SelectAll => {
                            self.select_all(window_adapter, self_rc);
                            return KeyEventResult::EventAccepted;
                        }
                        StandardShortcut::Copy => {
                            self.copy(window_adapter, self_rc);
                            return KeyEventResult::EventAccepted;
                        }
                        StandardShortcut::Paste if !self.read_only() => {
                            self.paste(window_adapter, self_rc);
                            return KeyEventResult::EventAccepted;
                        }
                        StandardShortcut::Cut if !self.read_only() => {
                            self.cut(window_adapter, self_rc);
                            return KeyEventResult::EventAccepted;
                        }
                        StandardShortcut::Paste | StandardShortcut::Cut => {
                            return KeyEventResult::EventIgnored;
                        }
                        StandardShortcut::Undo => {
                            self.undo(window_adapter, self_rc);
                            return KeyEventResult::EventAccepted;
                        }
                        StandardShortcut::Redo => {
                            self.redo(window_adapter, self_rc);
                            return KeyEventResult::EventAccepted;
                        }
                        _ => (),
                    }
                }

                let input_type = self.input_type();
                if input_type == InputType::Number
                    && !event.text.as_str().chars().all(|ch| ch.is_ascii_digit())
                {
                    return KeyEventResult::EventIgnored;
                }
                if input_type == InputType::Decimal {
                    let text = self.text().clone() + event.text.as_str();
                    if text.as_str() != "." && text.as_str() != "-" && text.parse::<f64>().is_err()
                    {
                        return KeyEventResult::EventIgnored;
                    }
                }

                if self.read_only() || event.modifiers.control {
                    return KeyEventResult::EventIgnored;
                }

                // save real anchor/cursor for undo/redo
                let (real_cursor, real_anchor) = {
                    let text = self.text();
                    (self.cursor_position(&text), self.anchor_position(&text))
                };

                self.delete_selection(window_adapter, self_rc, TextChangeNotify::SkipCallbacks);

                let mut text: String = self.text().into();

                // FIXME: respect grapheme boundaries
                let insert_pos = self.selection_anchor_and_cursor().1;
                text.insert_str(insert_pos, &event.text);

                self.add_undo_item(UndoItem {
                    pos: insert_pos,
                    text: event.text.clone(),
                    cursor: real_cursor,
                    anchor: real_anchor,
                    kind: UndoItemKind::TextInsert,
                });

                self.as_ref().text.set(text.into());
                let new_cursor_pos = (insert_pos + event.text.len()) as i32;
                self.as_ref().anchor_position_byte_offset.set(new_cursor_pos);
                self.set_cursor_position(
                    new_cursor_pos,
                    true,
                    TextChangeNotify::TriggerCallbacks,
                    window_adapter,
                    self_rc,
                );

                // Keep the cursor visible when inserting text. Blinking should only occur when
                // nothing is entered or the cursor isn't moved.
                self.as_ref().show_cursor(window_adapter);

                Self::FIELD_OFFSETS.edited.apply_pin(self).call(&());

                KeyEventResult::EventAccepted
            }
            KeyEventType::UpdateComposition | KeyEventType::CommitComposition => {
                let cursor = self.cursor_position(&self.text()) as i32;
                self.preedit_text.set(event.preedit_text.clone());
                self.preedit_selection.set(event.preedit_selection.clone().into());

                if let Some(r) = &event.replacement_range {
                    // Set the selection so the call to insert erases it
                    self.anchor_position_byte_offset.set(cursor.saturating_add(r.start));
                    self.cursor_position_byte_offset.set(cursor.saturating_add(r.end));
                    if event.text.is_empty() {
                        self.delete_selection(
                            window_adapter,
                            self_rc,
                            if event.cursor_position.is_none() {
                                TextChangeNotify::TriggerCallbacks
                            } else {
                                // will be updated by the set_cursor_position later
                                TextChangeNotify::SkipCallbacks
                            },
                        );
                    }
                }
                self.insert(&event.text, window_adapter, self_rc);
                if let Some(cursor) = event.cursor_position {
                    self.anchor_position_byte_offset.set(event.anchor_position.unwrap_or(cursor));
                    self.set_cursor_position(
                        cursor,
                        true,
                        TextChangeNotify::TriggerCallbacks,
                        window_adapter,
                        self_rc,
                    );
                }
                KeyEventResult::EventAccepted
            }
            _ => KeyEventResult::EventIgnored,
        }
    }

    fn focus_event(
        self: Pin<&Self>,
        event: &FocusEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) -> FocusEventResult {
        match event {
            FocusEvent::FocusIn | FocusEvent::WindowReceivedFocus => {
                self.has_focus.set(true);
                self.show_cursor(window_adapter);
                WindowInner::from_pub(window_adapter.window()).set_text_input_focused(true);
                // FIXME: This should be tracked by a PropertyTracker in window and toggled when read_only() toggles.
                if !self.read_only() {
                    if let Some(w) = window_adapter.internal(crate::InternalToken) {
                        w.input_method_request(InputMethodRequest::Enable(
                            self.ime_properties(window_adapter, self_rc),
                        ));
                    }
                }
            }
            FocusEvent::FocusOut | FocusEvent::WindowLostFocus => {
                self.has_focus.set(false);
                self.hide_cursor();
                if matches!(event, FocusEvent::FocusOut) {
                    self.as_ref()
                        .anchor_position_byte_offset
                        .set(self.as_ref().cursor_position_byte_offset());
                }
                WindowInner::from_pub(window_adapter.window()).set_text_input_focused(false);
                if !self.read_only() {
                    if let Some(window_adapter) = window_adapter.internal(crate::InternalToken) {
                        window_adapter.input_method_request(InputMethodRequest::Disable);
                        self.preedit_text.set(Default::default());
                    }
                }
            }
        }
        FocusEventResult::FocusAccepted
    }

    fn render(
        self: Pin<&Self>,
        backend: &mut &mut dyn ItemRenderer,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) -> RenderingResult {
        crate::properties::evaluate_no_tracking(|| {
            if self.has_focus() && self.text() != *backend.window().last_ime_text.borrow() {
                let window_adapter = &backend.window().window_adapter();
                if let Some(w) = window_adapter.internal(crate::InternalToken) {
                    w.input_method_request(InputMethodRequest::Update(
                        self.ime_properties(window_adapter, self_rc),
                    ));
                }
            }
        });
        (*backend).draw_text_input(self, self_rc, size);
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

/// Argument to [`TextInput::delete_selection`] that determines whether to trigger the
/// `edited` and cursor position callbacks and issue an input method request update.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TextChangeNotify {
    /// Trigger the callbacks.
    TriggerCallbacks,
    /// Skip triggering the callbacks, as a subsequent operation will trigger them.
    SkipCallbacks,
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
        .unwrap_or(text.len())
}

/// This struct holds the fields needed for rendering a TextInput item after applying any
/// on-going composition. This way the renderer's don't have to duplicate the code for extracting
/// and applying the pre-edit text, cursor placement within, etc.
#[derive(Debug)]
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
    /// The color of the (unselected) text
    pub text_color: Brush,
    /// The color of the blinking cursor
    pub cursor_color: Color,
    text_without_password: Option<String>,
    password_character: char,
}

impl TextInputVisualRepresentation {
    /// If the given `TextInput` renders a password, then all characters in this `TextInputVisualRepresentation` are replaced
    /// with the password character and the selection/preedit-ranges/cursor position are adjusted.
    /// If `password_character_fn` is Some, it is called lazily to query the password character, otherwise a default is used.
    fn apply_password_character_substitution(
        &mut self,
        text_input: Pin<&TextInput>,
        password_character_fn: Option<fn() -> char>,
    ) {
        if !matches!(text_input.input_type(), InputType::Password) {
            return;
        }

        let password_character = password_character_fn.map_or('●', |f| f());

        let text = &mut self.text;
        let fixup_range = |r: &mut core::ops::Range<usize>| {
            if !core::ops::Range::is_empty(r) {
                r.start = text[..r.start].chars().count() * password_character.len_utf8();
                r.end = text[..r.end].chars().count() * password_character.len_utf8();
            }
        };
        fixup_range(&mut self.preedit_range);
        fixup_range(&mut self.selection_range);
        if let Some(cursor_pos) = self.cursor_position.as_mut() {
            *cursor_pos = text[..*cursor_pos].chars().count() * password_character.len_utf8();
        }
        self.text_without_password = Some(core::mem::replace(
            text,
            core::iter::repeat(password_character).take(text.chars().count()).collect(),
        ));
        self.password_character = password_character;
    }

    /// Use this function to make a byte offset in the text used for rendering back to a byte offset in the
    /// TextInput's text. The offsets might differ for example for password text input fields.
    pub fn map_byte_offset_from_byte_offset_in_visual_text(&self, byte_offset: usize) -> usize {
        if let Some(text_without_password) = self.text_without_password.as_ref() {
            text_without_password
                .char_indices()
                .nth(byte_offset / self.password_character.len_utf8())
                .map_or(text_without_password.len(), |(r, _)| r)
        } else {
            byte_offset
        }
    }
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
        trigger_callbacks: TextChangeNotify,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) -> bool {
        let text = self.text();
        if text.is_empty() {
            return false;
        }

        let last_cursor_pos = self.cursor_position(&text);

        let mut grapheme_cursor =
            unicode_segmentation::GraphemeCursor::new(last_cursor_pos, text.len(), true);

        let font_height = window_adapter
            .renderer()
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

                let cursor_rect = self.cursor_rect_for_byte_offset(last_cursor_pos, window_adapter);
                let mut cursor_xy_pos = cursor_rect.center();

                cursor_xy_pos.y += font_height;
                cursor_xy_pos.x = self.preferred_x_pos.get();
                self.byte_offset_for_position(cursor_xy_pos, window_adapter)
            }
            TextCursorDirection::PreviousLine => {
                reset_preferred_x_pos = false;

                let cursor_rect = self.cursor_rect_for_byte_offset(last_cursor_pos, window_adapter);
                let mut cursor_xy_pos = cursor_rect.center();

                cursor_xy_pos.y -= font_height;
                cursor_xy_pos.x = self.preferred_x_pos.get();
                self.byte_offset_for_position(cursor_xy_pos, window_adapter)
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
            TextCursorDirection::ForwardByWord => next_word_boundary(&text, last_cursor_pos + 1),
            TextCursorDirection::BackwardByWord => {
                prev_word_boundary(&text, last_cursor_pos.saturating_sub(1))
            }
            TextCursorDirection::StartOfLine => {
                let cursor_rect = self.cursor_rect_for_byte_offset(last_cursor_pos, window_adapter);
                let mut cursor_xy_pos = cursor_rect.center();

                cursor_xy_pos.x = 0 as Coord;
                self.byte_offset_for_position(cursor_xy_pos, window_adapter)
            }
            TextCursorDirection::EndOfLine => {
                let cursor_rect = self.cursor_rect_for_byte_offset(last_cursor_pos, window_adapter);
                let mut cursor_xy_pos = cursor_rect.center();

                cursor_xy_pos.x = Coord::MAX;
                self.byte_offset_for_position(cursor_xy_pos, window_adapter)
            }
            TextCursorDirection::StartOfParagraph => {
                prev_paragraph_boundary(&text, last_cursor_pos.saturating_sub(1))
            }
            TextCursorDirection::EndOfParagraph => {
                next_paragraph_boundary(&text, last_cursor_pos + 1)
            }
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
            trigger_callbacks,
            window_adapter,
            self_rc,
        );

        // Keep the cursor visible when moving. Blinking should only occur when
        // nothing is entered or the cursor isn't moved.
        self.as_ref().show_cursor(window_adapter);

        new_cursor_pos != last_cursor_pos
    }

    pub fn set_cursor_position(
        self: Pin<&Self>,
        new_position: i32,
        reset_preferred_x_pos: bool,
        trigger_callbacks: TextChangeNotify,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) {
        self.cursor_position_byte_offset.set(new_position);
        if new_position >= 0 {
            let pos = self
                .cursor_rect_for_byte_offset(new_position as usize, window_adapter)
                .origin
                .to_untyped();
            if reset_preferred_x_pos {
                self.preferred_x_pos.set(pos.x);
            }
            if trigger_callbacks == TextChangeNotify::TriggerCallbacks {
                Self::FIELD_OFFSETS.cursor_position_changed.apply_pin(self).call(&(pos,));
                self.update_ime(window_adapter, self_rc);
            }
        }
    }

    fn update_ime(self: Pin<&Self>, window_adapter: &Rc<dyn WindowAdapter>, self_rc: &ItemRc) {
        if self.read_only() || !self.has_focus() {
            return;
        }
        if let Some(w) = window_adapter.internal(crate::InternalToken) {
            w.input_method_request(InputMethodRequest::Update(
                self.ime_properties(window_adapter, self_rc),
            ));
        }
    }

    fn select_and_delete(
        self: Pin<&Self>,
        step: TextCursorDirection,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) {
        if !self.has_selection() {
            self.move_cursor(
                step,
                AnchorMode::KeepAnchor,
                TextChangeNotify::SkipCallbacks,
                window_adapter,
                self_rc,
            );
        }
        self.delete_selection(window_adapter, self_rc, TextChangeNotify::TriggerCallbacks);
    }

    pub fn delete_selection(
        self: Pin<&Self>,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
        trigger_callbacks: TextChangeNotify,
    ) {
        let text: String = self.text().into();
        if text.is_empty() {
            return;
        }

        let (anchor, cursor) = self.selection_anchor_and_cursor();
        if anchor == cursor {
            return;
        }

        let removed_text: SharedString = text[anchor..cursor].into();
        // save real anchor/cursor for undo/redo
        let (real_cursor, real_anchor) = {
            let text = self.text();
            (self.cursor_position(&text), self.anchor_position(&text))
        };

        let text = [text.split_at(anchor).0, text.split_at(cursor).1].concat();
        self.text.set(text.into());
        self.anchor_position_byte_offset.set(anchor as i32);

        self.add_undo_item(UndoItem {
            pos: anchor,
            text: removed_text,
            cursor: real_cursor,
            anchor: real_anchor,
            kind: UndoItemKind::TextRemove,
        });

        if trigger_callbacks == TextChangeNotify::TriggerCallbacks {
            self.set_cursor_position(
                anchor as i32,
                true,
                trigger_callbacks,
                window_adapter,
                self_rc,
            );
            Self::FIELD_OFFSETS.edited.apply_pin(self).call(&());
        } else {
            self.cursor_position_byte_offset.set(anchor as i32);
        }
    }

    pub fn anchor_position(self: Pin<&Self>, text: &str) -> usize {
        safe_byte_offset(self.anchor_position_byte_offset(), text)
    }

    pub fn cursor_position(self: Pin<&Self>, text: &str) -> usize {
        safe_byte_offset(self.cursor_position_byte_offset(), text)
    }

    fn ime_properties(
        self: Pin<&Self>,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) -> InputMethodProperties {
        let text = self.text();
        WindowInner::from_pub(window_adapter.window()).last_ime_text.replace(text.clone());
        let cursor_position = self.cursor_position(&text);
        let anchor_position = self.anchor_position(&text);
        let cursor_relative = self.cursor_rect_for_byte_offset(cursor_position, window_adapter);
        let geometry = self_rc.geometry();
        let origin = self_rc.map_to_window(geometry.origin).to_vector();
        let cursor_rect_origin =
            crate::api::LogicalPosition::from_euclid(cursor_relative.origin + origin);
        let cursor_rect_size = crate::api::LogicalSize::from_euclid(cursor_relative.size);
        let anchor_point = crate::api::LogicalPosition::from_euclid(
            self.cursor_rect_for_byte_offset(anchor_position, window_adapter).origin
                + origin
                + cursor_relative.size,
        );

        InputMethodProperties {
            text,
            cursor_position,
            anchor_position: (cursor_position != anchor_position).then_some(anchor_position),
            preedit_text: self.preedit_text(),
            preedit_offset: cursor_position,
            cursor_rect_origin,
            cursor_rect_size,
            anchor_point,
            input_type: self.input_type(),
        }
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
        if text_to_insert.is_empty() {
            return;
        }

        let (real_cursor, real_anchor) = {
            let text = self.text();
            (self.cursor_position(&text), self.anchor_position(&text))
        };

        self.delete_selection(window_adapter, self_rc, TextChangeNotify::SkipCallbacks);
        let mut text: String = self.text().into();
        let cursor_pos = self.selection_anchor_and_cursor().1;
        let mut inserted_text: SharedString = text_to_insert.into();
        if text_to_insert.contains('\n') && self.single_line() {
            inserted_text = text_to_insert.replace('\n', " ").into();
            text.insert_str(cursor_pos, &inserted_text);
        } else {
            text.insert_str(cursor_pos, text_to_insert);
        }

        self.add_undo_item(UndoItem {
            pos: cursor_pos,
            text: inserted_text,
            cursor: real_cursor,
            anchor: real_anchor,
            kind: UndoItemKind::TextInsert,
        });

        let cursor_pos = cursor_pos + text_to_insert.len();
        self.text.set(text.into());
        self.anchor_position_byte_offset.set(cursor_pos as i32);
        self.set_cursor_position(
            cursor_pos as i32,
            true,
            TextChangeNotify::TriggerCallbacks,
            window_adapter,
            self_rc,
        );
        Self::FIELD_OFFSETS.edited.apply_pin(self).call(&());
    }

    pub fn cut(self: Pin<&Self>, window_adapter: &Rc<dyn WindowAdapter>, self_rc: &ItemRc) {
        self.copy(window_adapter, self_rc);
        self.delete_selection(window_adapter, self_rc, TextChangeNotify::TriggerCallbacks);
    }

    pub fn set_selection_offsets(
        self: Pin<&Self>,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
        start: i32,
        end: i32,
    ) {
        let text = self.text();
        let safe_start = safe_byte_offset(start, &text);
        let safe_end = safe_byte_offset(end, &text);

        self.as_ref().anchor_position_byte_offset.set(safe_start as i32);
        self.set_cursor_position(
            safe_end as i32,
            true,
            TextChangeNotify::TriggerCallbacks,
            window_adapter,
            self_rc,
        );
    }

    pub fn select_all(self: Pin<&Self>, window_adapter: &Rc<dyn WindowAdapter>, self_rc: &ItemRc) {
        self.move_cursor(
            TextCursorDirection::StartOfText,
            AnchorMode::MoveAnchor,
            TextChangeNotify::SkipCallbacks,
            window_adapter,
            self_rc,
        );
        self.move_cursor(
            TextCursorDirection::EndOfText,
            AnchorMode::KeepAnchor,
            TextChangeNotify::TriggerCallbacks,
            window_adapter,
            self_rc,
        );
    }

    pub fn clear_selection(self: Pin<&Self>, _: &Rc<dyn WindowAdapter>, _: &ItemRc) {
        self.as_ref().anchor_position_byte_offset.set(self.as_ref().cursor_position_byte_offset());
    }

    pub fn select_word(self: Pin<&Self>, window_adapter: &Rc<dyn WindowAdapter>, self_rc: &ItemRc) {
        let text = self.text();
        let anchor = self.anchor_position(&text);
        let cursor = self.cursor_position(&text);
        let (new_a, new_c) = if anchor <= cursor {
            (prev_word_boundary(&text, anchor), next_word_boundary(&text, cursor))
        } else {
            (next_word_boundary(&text, anchor), prev_word_boundary(&text, cursor))
        };
        self.as_ref().anchor_position_byte_offset.set(new_a as i32);
        self.set_cursor_position(
            new_c as i32,
            true,
            TextChangeNotify::TriggerCallbacks,
            window_adapter,
            self_rc,
        );
    }

    fn select_paragraph(
        self: Pin<&Self>,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) {
        let text = self.text();
        let anchor = self.anchor_position(&text);
        let cursor = self.cursor_position(&text);
        let (new_a, new_c) = if anchor <= cursor {
            (prev_paragraph_boundary(&text, anchor), next_paragraph_boundary(&text, cursor))
        } else {
            (next_paragraph_boundary(&text, anchor), prev_paragraph_boundary(&text, cursor))
        };
        self.as_ref().anchor_position_byte_offset.set(new_a as i32);
        self.set_cursor_position(
            new_c as i32,
            true,
            TextChangeNotify::TriggerCallbacks,
            window_adapter,
            self_rc,
        );
    }

    pub fn copy(self: Pin<&Self>, w: &Rc<dyn WindowAdapter>, _: &ItemRc) {
        self.copy_clipboard(w, Clipboard::DefaultClipboard);
    }

    fn copy_clipboard(
        self: Pin<&Self>,
        window_adapter: &Rc<dyn WindowAdapter>,
        clipboard: Clipboard,
    ) {
        let (anchor, cursor) = self.selection_anchor_and_cursor();
        if anchor == cursor {
            return;
        }
        let text = self.text();

        WindowInner::from_pub(window_adapter.window())
            .ctx
            .0
            .platform
            .set_clipboard_text(&text[anchor..cursor], clipboard);
    }

    pub fn paste(self: Pin<&Self>, window_adapter: &Rc<dyn WindowAdapter>, self_rc: &ItemRc) {
        self.paste_clipboard(window_adapter, self_rc, Clipboard::DefaultClipboard);
    }

    fn paste_clipboard(
        self: Pin<&Self>,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
        clipboard: Clipboard,
    ) {
        if let Some(text) =
            WindowInner::from_pub(window_adapter.window()).ctx.0.platform.clipboard_text(clipboard)
        {
            self.preedit_text.set(Default::default());
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
            italic: self.font_italic(),
        }
    }

    /// Returns a [`TextInputVisualRepresentation`] struct that contains all the fields necessary for rendering the text input,
    /// after making adjustments such as applying a substitution of characters for password input fields, or making sure
    /// that the selection start is always less or equal than the selection end.
    pub fn visual_representation(
        self: Pin<&Self>,
        password_character_fn: Option<fn() -> char>,
    ) -> TextInputVisualRepresentation {
        let mut text: String = self.text().into();

        let preedit_text = self.preedit_text();
        let (preedit_range, selection_range, cursor_position) = if !preedit_text.is_empty() {
            let cursor_position = self.cursor_position(&text);

            text.insert_str(cursor_position, &preedit_text);
            let preedit_range = cursor_position..cursor_position + preedit_text.len();

            if let Some(preedit_sel) = self.preedit_selection().as_option() {
                let preedit_selection = cursor_position + preedit_sel.start as usize
                    ..cursor_position + preedit_sel.end as usize;
                (preedit_range, preedit_selection, Some(cursor_position + preedit_sel.end as usize))
            } else {
                let cur = preedit_range.end;
                (preedit_range, cur..cur, None)
            }
        } else {
            let preedit_range = Default::default();
            let (selection_anchor_pos, selection_cursor_pos) = self.selection_anchor_and_cursor();
            let selection_range = selection_anchor_pos..selection_cursor_pos;
            let cursor_position = self.cursor_position(&text);
            let cursor_visible = self.cursor_visible() && self.enabled() && !self.read_only();
            let cursor_position = if cursor_visible && selection_range.is_empty() {
                Some(cursor_position)
            } else {
                None
            };
            (preedit_range, selection_range, cursor_position)
        };

        let text_color = self.color();

        let cursor_color =
            if cfg!(any(target_os = "android", target_os = "macos", target_os = "ios")) {
                if cursor_position.is_some() {
                    self.selection_background_color().with_alpha(1.)
                } else {
                    Default::default()
                }
            } else {
                text_color.color()
            };

        let mut repr = TextInputVisualRepresentation {
            text,
            preedit_range,
            selection_range,
            cursor_position,
            text_without_password: None,
            password_character: Default::default(),
            text_color,
            cursor_color,
        };
        repr.apply_password_character_substitution(self, password_character_fn);
        repr
    }

    fn cursor_rect_for_byte_offset(
        self: Pin<&Self>,
        byte_offset: usize,
        window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LogicalRect {
        window_adapter.renderer().text_input_cursor_rect_for_byte_offset(
            self,
            byte_offset,
            self.font_request(window_adapter),
            ScaleFactor::new(window_adapter.window().scale_factor()),
        )
    }

    pub fn byte_offset_for_position(
        self: Pin<&Self>,
        pos: LogicalPoint,
        window_adapter: &Rc<dyn WindowAdapter>,
    ) -> usize {
        window_adapter.renderer().text_input_byte_offset_for_position(
            self,
            pos,
            self.font_request(window_adapter),
            ScaleFactor::new(window_adapter.window().scale_factor()),
        )
    }

    /// When pressing the mouse (or releasing the finger, on android) we should take the focus if we don't have it already.
    /// Setting the focus will show the virtual keyboard, otherwise we should make sure that the keyboard is shown if it was hidden by the user
    fn ensure_focus_and_ime(
        self: Pin<&Self>,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) {
        if !self.has_focus() {
            WindowInner::from_pub(window_adapter.window()).set_focus_item(self_rc, true);
        } else if !self.read_only() {
            if let Some(w) = window_adapter.internal(crate::InternalToken) {
                w.input_method_request(InputMethodRequest::Enable(
                    self.ime_properties(window_adapter, self_rc),
                ));
            }
        }
    }

    fn add_undo_item(self: Pin<&Self>, item: UndoItem) {
        let mut items = self.undo_items.take();
        // try to merge with the last item
        if let Some(last) = items.make_mut_slice().last_mut() {
            match (&item.kind, &last.kind) {
                (UndoItemKind::TextInsert, UndoItemKind::TextInsert) => {
                    let is_new_line = item.text == "\n";
                    let last_is_new_line = last.text == "\n";
                    // if the last item or current item is a new_line
                    // we insert it as a standalone item, no merging
                    if item.pos == last.pos + last.text.len() && !is_new_line && !last_is_new_line {
                        last.text += &item.text;
                    } else {
                        items.push(item);
                    }
                }
                (UndoItemKind::TextRemove, UndoItemKind::TextRemove) => {
                    if item.pos + item.text.len() == last.pos {
                        last.pos = item.pos;
                        let old_text = last.text.clone();
                        last.text = item.text;
                        last.text += &old_text;
                        // prepend
                    } else {
                        items.push(item);
                    }
                }
                _ => {
                    items.push(item);
                }
            }
        } else {
            items.push(item);
        }

        self.undo_items.set(items);
    }

    fn undo(self: Pin<&Self>, window_adapter: &Rc<dyn WindowAdapter>, self_rc: &ItemRc) {
        let mut items = self.undo_items.take();
        let Some(last) = items.pop() else {
            return;
        };

        match last.kind {
            UndoItemKind::TextInsert => {
                let text: String = self.text().into();
                let text = [text.split_at(last.pos).0, text.split_at(last.pos + last.text.len()).1]
                    .concat();
                self.text.set(text.into());

                self.anchor_position_byte_offset.set(last.anchor as i32);
                self.set_cursor_position(
                    last.cursor as i32,
                    true,
                    TextChangeNotify::TriggerCallbacks,
                    window_adapter,
                    self_rc,
                );
            }
            UndoItemKind::TextRemove => {
                let mut text: String = self.text().into();
                text.insert_str(last.pos, &last.text);
                self.text.set(text.into());

                self.anchor_position_byte_offset.set(last.anchor as i32);
                self.set_cursor_position(
                    last.cursor as i32,
                    true,
                    TextChangeNotify::TriggerCallbacks,
                    window_adapter,
                    self_rc,
                );
            }
        }
        self.undo_items.set(items);

        let mut redo = self.redo_items.take();
        redo.push(last);
        self.redo_items.set(redo);
    }

    fn redo(self: Pin<&Self>, window_adapter: &Rc<dyn WindowAdapter>, self_rc: &ItemRc) {
        let mut items = self.redo_items.take();
        let Some(last) = items.pop() else {
            return;
        };

        match last.kind {
            UndoItemKind::TextInsert => {
                let mut text: String = self.text().into();
                text.insert_str(last.pos, &last.text);
                self.text.set(text.into());

                self.anchor_position_byte_offset.set(last.anchor as i32);
                self.set_cursor_position(
                    last.cursor as i32,
                    true,
                    TextChangeNotify::TriggerCallbacks,
                    window_adapter,
                    self_rc,
                );
            }
            UndoItemKind::TextRemove => {
                let text: String = self.text().into();
                let text = [text.split_at(last.pos).0, text.split_at(last.pos + last.text.len()).1]
                    .concat();
                self.text.set(text.into());

                self.anchor_position_byte_offset.set(last.anchor as i32);
                self.set_cursor_position(
                    last.cursor as i32,
                    true,
                    TextChangeNotify::TriggerCallbacks,
                    window_adapter,
                    self_rc,
                );
            }
        }

        self.redo_items.set(items);

        let mut undo_items = self.undo_items.take();
        undo_items.push(last);
        self.undo_items.set(undo_items);
    }
}

fn next_paragraph_boundary(text: &str, last_cursor_pos: usize) -> usize {
    text.as_bytes()
        .iter()
        .enumerate()
        .skip(last_cursor_pos)
        .find(|(_, &c)| c == b'\n')
        .map(|(new_pos, _)| new_pos)
        .unwrap_or(text.len())
}

fn prev_paragraph_boundary(text: &str, last_cursor_pos: usize) -> usize {
    text.as_bytes()
        .iter()
        .enumerate()
        .rev()
        .skip(text.len() - last_cursor_pos)
        .find(|(_, &c)| c == b'\n')
        .map(|(new_pos, _)| new_pos + 1)
        .unwrap_or(0)
}

fn prev_word_boundary(text: &str, last_cursor_pos: usize) -> usize {
    let mut word_offset = 0;

    for (current_word_offset, _) in text.unicode_word_indices() {
        if current_word_offset <= last_cursor_pos {
            word_offset = current_word_offset;
        } else {
            break;
        }
    }

    word_offset
}

fn next_word_boundary(text: &str, last_cursor_pos: usize) -> usize {
    text.unicode_word_indices()
        .find(|(offset, slice)| *offset + slice.len() >= last_cursor_pos)
        .map_or(text.len(), |(offset, slice)| offset + slice.len())
}

#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn slint_textinput_set_selection_offsets(
    text_input: Pin<&TextInput>,
    window_adapter: *const crate::window::ffi::WindowAdapterRcOpaque,
    self_component: &vtable::VRc<crate::item_tree::ItemTreeVTable>,
    self_index: u32,
    start: i32,
    end: i32,
) {
    let window_adapter = &*(window_adapter as *const Rc<dyn WindowAdapter>);
    let self_rc = ItemRc::new(self_component.clone(), self_index);
    text_input.set_selection_offsets(window_adapter, &self_rc, start, end);
}

#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn slint_textinput_select_all(
    text_input: Pin<&TextInput>,
    window_adapter: *const crate::window::ffi::WindowAdapterRcOpaque,
    self_component: &vtable::VRc<crate::item_tree::ItemTreeVTable>,
    self_index: u32,
) {
    let window_adapter = &*(window_adapter as *const Rc<dyn WindowAdapter>);
    let self_rc = ItemRc::new(self_component.clone(), self_index);
    text_input.select_all(window_adapter, &self_rc);
}

#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn slint_textinput_clear_selection(
    text_input: Pin<&TextInput>,
    window_adapter: *const crate::window::ffi::WindowAdapterRcOpaque,
    self_component: &vtable::VRc<crate::item_tree::ItemTreeVTable>,
    self_index: u32,
) {
    let window_adapter = &*(window_adapter as *const Rc<dyn WindowAdapter>);
    let self_rc = ItemRc::new(self_component.clone(), self_index);
    text_input.clear_selection(window_adapter, &self_rc);
}

#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn slint_textinput_cut(
    text_input: Pin<&TextInput>,
    window_adapter: *const crate::window::ffi::WindowAdapterRcOpaque,
    self_component: &vtable::VRc<crate::item_tree::ItemTreeVTable>,
    self_index: u32,
) {
    let window_adapter = &*(window_adapter as *const Rc<dyn WindowAdapter>);
    let self_rc = ItemRc::new(self_component.clone(), self_index);
    text_input.cut(window_adapter, &self_rc);
}

#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn slint_textinput_copy(
    text_input: Pin<&TextInput>,
    window_adapter: *const crate::window::ffi::WindowAdapterRcOpaque,
    self_component: &vtable::VRc<crate::item_tree::ItemTreeVTable>,
    self_index: u32,
) {
    let window_adapter = &*(window_adapter as *const Rc<dyn WindowAdapter>);
    let self_rc = ItemRc::new(self_component.clone(), self_index);
    text_input.copy(window_adapter, &self_rc);
}

#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn slint_textinput_paste(
    text_input: Pin<&TextInput>,
    window_adapter: *const crate::window::ffi::WindowAdapterRcOpaque,
    self_component: &vtable::VRc<crate::item_tree::ItemTreeVTable>,
    self_index: u32,
) {
    let window_adapter = &*(window_adapter as *const Rc<dyn WindowAdapter>);
    let self_rc = ItemRc::new(self_component.clone(), self_index);
    text_input.paste(window_adapter, &self_rc);
}
