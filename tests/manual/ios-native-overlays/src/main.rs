// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::ComponentHandle;
use slint::winit_030::WinitWindowAccessor;
use std::ffi::{CStr, c_char, c_void};

slint::slint! {
    export component NativeOverlayDemo inherits Window {
        title: "Slint native overlays";
        background: #f4f6fa;

        in-out property<string> editor-text: "Edit this text";
        in-out property<string> second-editor-text: "This second editor spans multiple lines.\nSelect and edit either line with native iOS controls.";
        in-out property<int> active-editor: -1;
        in-out property<bool> menu-button-pressed;
        in-out property<string> menu-result: "No context-menu action selected";
        in-out property<length> scroll-offset: 0px;
        callback scroll-geometry-changed();
        changed scroll-offset => root.scroll-geometry-changed();

        out property<float> editor-x: editor.x / 1px;
        out property<float> editor-y: editor.y / 1px;
        out property<float> editor-width: editor.width / 1px;
        out property<float> editor-height: editor.height / 1px;
        out property<float> second-editor-x: (scroll-clip.x + second-editor.x) / 1px;
        out property<float> second-editor-y: (scroll-clip.y + root.scroll-offset + second-editor.y) / 1px;
        out property<float> second-editor-width: second-editor.width / 1px;
        out property<float> second-editor-height: second-editor.height / 1px;
        out property<float> scroll-clip-x: scroll-clip.x / 1px;
        out property<float> scroll-clip-y: scroll-clip.y / 1px;
        out property<float> scroll-clip-width: scroll-clip.width / 1px;
        out property<float> scroll-clip-height: scroll-clip.height / 1px;
        out property<float> scroll-up-x: scroll-up.x / 1px;
        out property<float> scroll-up-y: scroll-up.y / 1px;
        out property<float> scroll-up-width: scroll-up.width / 1px;
        out property<float> scroll-up-height: scroll-up.height / 1px;
        out property<float> scroll-down-x: scroll-down.x / 1px;
        out property<float> scroll-down-y: scroll-down.y / 1px;
        out property<float> scroll-down-width: scroll-down.width / 1px;
        out property<float> scroll-down-height: scroll-down.height / 1px;
        out property<float> native-editor-x: native-editor.x / 1px;
        out property<float> native-editor-y: native-editor.y / 1px;
        out property<float> native-editor-width: native-editor.width / 1px;
        out property<float> native-editor-height: native-editor.height / 1px;
        out property<float> menu-x: menu-card.x / 1px;
        out property<float> menu-y: menu-card.y / 1px;
        out property<float> menu-width: menu-card.width / 1px;
        out property<float> menu-height: menu-card.height / 1px;

        Text {
            x: 24px;
            y: 70px;
            width: root.width - 48px;
            text: "Native UIKit over Slint";
            font-size: 26px;
            font-weight: 700;
            color: #17233b;
        }

        Text {
            x: 24px;
            y: 112px;
            width: root.width - 48px;
            text: "Tap a field, or press the Slint button for a native menu.";
            font-size: 14px;
            color: #536078;
        }

        editor := Rectangle {
            x: 24px;
            y: 150px;
            width: root.width - 48px;
            height: 56px;
            background: #ffffff;
            border-width: 1px;
            border-color: root.active-editor == 0 ? #1473e6 : #c5cad3;
            border-radius: 12px;

            editor-input := TextInput {
                x: 16px;
                width: parent.width - 32px;
                height: parent.height;
                text <=> root.editor-text;
                font-size: 17px;
                color: #17233b;
                vertical-alignment: center;
                single-line: true;
                accessible-role: none;
            }
        }

        Text {
            x: 24px;
            y: 218px;
            width: root.width - 48px;
            text: "Scrollable Slint multiline field";
            font-size: 16px;
            font-weight: 600;
            color: #17233b;
        }

        scroll-clip := Rectangle {
            x: 24px;
            y: 246px;
            width: root.width - 128px;
            height: 140px;
            background: #dce4ef;
            border-radius: 12px;
            clip: true;

            scroll-view := Flickable {
                width: parent.width;
                height: parent.height;
                content-width: self.width;
                content-height: 240px;
                content-y <=> root.scroll-offset;

                second-editor := Rectangle {
                    x: 0px;
                    y: 14px;
                    width: parent.width;
                    height: 112px;
                    background: #ffffff;
                    border-width: 1px;
                    border-color: root.active-editor == 1 ? #1473e6 : #c5cad3;
                    border-radius: 12px;

                    second-editor-input := TextInput {
                        x: 16px;
                        y: 12px;
                        width: parent.width - 32px;
                        height: parent.height - 24px;
                        text <=> root.second-editor-text;
                        font-size: 17px;
                        color: #17233b;
                        vertical-alignment: top;
                        single-line: false;
                        wrap: word-wrap;
                        accessible-role: none;
                    }
                }

                Text {
                    x: 12px;
                    y: 164px;
                    width: parent.width - 24px;
                    text: "Drag this empty area to scroll with the selection active.";
                    wrap: word-wrap;
                    font-size: 13px;
                    color: #536078;
                }
            }
        }

        scroll-up := Rectangle {
            x: root.width - 92px;
            y: 246px;
            width: 68px;
            height: 64px;
            background: #1473e6;
            border-radius: 12px;

            Text {
                width: parent.width;
                height: parent.height;
                text: "Scroll\nup";
                color: #ffffff;
                font-size: 14px;
                font-weight: 600;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        scroll-down := Rectangle {
            x: root.width - 92px;
            y: 322px;
            width: 68px;
            height: 64px;
            background: #1473e6;
            border-radius: 12px;

            Text {
                width: parent.width;
                height: parent.height;
                text: "Scroll\ndown";
                color: #ffffff;
                font-size: 14px;
                font-weight: 600;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        Text {
            x: 24px;
            y: 406px;
            width: root.width - 48px;
            text: "Pure UIKit multiline baseline";
            font-size: 18px;
            font-weight: 700;
            color: #17233b;
        }

        native-editor := Rectangle {
            x: 24px;
            y: 434px;
            width: root.width - 48px;
            height: 112px;
            background: #ffffff;
            border-width: 1px;
            border-color: #c5cad3;
            border-radius: 12px;
        }

        Text {
            x: 24px;
            y: 558px;
            width: root.width - 48px;
            text: "Native menu from a Slint button";
            font-size: 18px;
            font-weight: 700;
            color: #17233b;
        }

        menu-card := Rectangle {
            x: 24px;
            y: 586px;
            width: root.width - 48px;
            height: 56px;
            background: root.menu-button-pressed ? #0f66cf : #1473e6;
            border-radius: 12px;

            Text {
                width: parent.width;
                height: parent.height;
                text: "Show native menu";
                font-size: 17px;
                font-weight: 600;
                color: #ffffff;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        Rectangle {
            x: 24px;
            y: 654px;
            width: root.width - 48px;
            height: 54px;
            background: #e7edf6;
            border-radius: 10px;

            Text {
                x: 14px;
                width: parent.width - 28px;
                height: parent.height;
                text: root.menu-result;
                font-size: 14px;
                color: #24415f;
                vertical-alignment: center;
            }
        }

        Text {
            x: 24px;
            y: root.height - 70px;
            width: root.width - 48px;
            text: "The fields and menu button remain Slint-rendered.";
            font-size: 13px;
            color: #69758a;
            horizontal-alignment: center;
        }
    }
}

thread_local! {
    static APP: std::cell::RefCell<Option<slint::Weak<NativeOverlayDemo>>> = const { std::cell::RefCell::new(None) };
    static EDITOR_ITEMS: std::cell::RefCell<[Option<i_slint_core::items::ItemRc>; 2]> = const { std::cell::RefCell::new([None, None]) };
}

fn editor_item(
    app: &NativeOverlayDemo,
    editor_index: usize,
) -> Option<i_slint_core::items::ItemRc> {
    if editor_index >= 2 {
        return None;
    }
    if let Some(item) = EDITOR_ITEMS.with(|items| items.borrow()[editor_index].clone()) {
        return Some(item);
    }

    fn find(
        item: i_slint_core::items::ItemRc,
        remaining: &mut usize,
    ) -> Option<i_slint_core::items::ItemRc> {
        if item.downcast::<i_slint_core::items::TextInput>().is_some() {
            if *remaining == 0 {
                return Some(item);
            }
            *remaining -= 1;
        }
        let mut child = item.first_child();
        while let Some(candidate) = child {
            child = candidate.next_sibling();
            if let Some(found) = find(candidate, remaining) {
                return Some(found);
            }
        }
        None
    }

    let root = i_slint_core::window::WindowInner::from_pub(app.window()).component();
    let mut remaining = editor_index;
    let item = find(i_slint_core::items::ItemRc::new_root(root), &mut remaining);
    if let Some(item) = &item {
        EDITOR_ITEMS.with(|items| items.borrow_mut()[editor_index] = Some(item.clone()));
    }
    item
}

fn utf16_to_byte_offset(text: &str, utf16_offset: usize) -> usize {
    let mut units = 0;
    for (byte_offset, ch) in text.char_indices() {
        if units >= utf16_offset {
            return byte_offset;
        }
        units += ch.len_utf16();
        if units > utf16_offset {
            return byte_offset;
        }
    }
    text.len()
}

fn byte_to_utf16_offset(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset.min(text.len())].encode_utf16().count()
}

fn update_selection(
    app: &NativeOverlayDemo,
    editor_index: usize,
    selection_anchor_utf16: usize,
    selection_focus_utf16: usize,
) {
    let Some(item) = editor_item(app, editor_index) else {
        return;
    };
    let input = item.downcast::<i_slint_core::items::TextInput>().unwrap();
    let text = input.as_pin_ref().text();
    let adapter = i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter();
    input.as_pin_ref().set_selection_offsets(
        &adapter,
        &item,
        utf16_to_byte_offset(&text, selection_anchor_utf16) as i32,
        utf16_to_byte_offset(&text, selection_focus_utf16) as i32,
    );
    app.window().request_redraw();
}

#[unsafe(no_mangle)]
extern "C" fn native_text_state_changed(
    editor_index: usize,
    text: *const c_char,
    selection_anchor_utf16: usize,
    selection_focus_utf16: usize,
) {
    if text.is_null() {
        return;
    }
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    APP.with(|slot| {
        if let Some(app) = slot.borrow().as_ref().and_then(slint::Weak::upgrade) {
            match editor_index {
                0 => app.set_editor_text(text.as_ref().into()),
                1 => app.set_second_editor_text(text.as_ref().into()),
                _ => return,
            }
            update_selection(&app, editor_index, selection_anchor_utf16, selection_focus_utf16);
        }
    });
}

#[unsafe(no_mangle)]
extern "C" fn native_selection_changed(
    editor_index: usize,
    selection_anchor_utf16: usize,
    selection_focus_utf16: usize,
) {
    APP.with(|slot| {
        if let Some(app) = slot.borrow().as_ref().and_then(slint::Weak::upgrade) {
            update_selection(&app, editor_index, selection_anchor_utf16, selection_focus_utf16);
        }
    });
}

#[repr(C)]
struct NativeRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

fn caret_rect(
    app: &NativeOverlayDemo,
    editor_index: usize,
    utf16_offset: usize,
) -> Option<NativeRect> {
    let item = editor_item(app, editor_index)?;
    let input = item.downcast::<i_slint_core::items::TextInput>()?;
    let text = input.as_pin_ref().text();
    let byte_offset = utf16_to_byte_offset(&text, utf16_offset);
    let adapter = i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter();
    let rect = i_slint_core::textlayout::sharedparley::text_input_cursor_rect_for_byte_offset(
        adapter.renderer(),
        input.as_pin_ref(),
        &item,
        byte_offset,
        i_slint_core::items::TextCursorAffinity::NextCharacter,
        adapter.renderer().text_layout_cache(),
    );
    let item_origin = item.map_to_native_window(item.geometry().origin);
    Some(NativeRect {
        x: item_origin.x + rect.origin.x,
        y: item_origin.y + rect.origin.y,
        width: rect.size.width,
        height: rect.size.height,
    })
}

#[unsafe(no_mangle)]
extern "C" fn native_caret_rect(
    editor_index: usize,
    utf16_offset: usize,
    result: *mut NativeRect,
) -> bool {
    if result.is_null() {
        return false;
    }
    APP.with(|slot| {
        let Some(app) = slot.borrow().as_ref().and_then(slint::Weak::upgrade) else {
            return false;
        };
        let Some(rect) = caret_rect(&app, editor_index, utf16_offset) else {
            return false;
        };
        unsafe { *result = rect };
        true
    })
}

#[unsafe(no_mangle)]
extern "C" fn native_closest_text_position(
    editor_index: usize,
    window_x: f32,
    window_y: f32,
) -> usize {
    APP.with(|slot| {
        let Some(app) = slot.borrow().as_ref().and_then(slint::Weak::upgrade) else {
            return 0;
        };
        let Some(item) = editor_item(&app, editor_index) else {
            return 0;
        };
        let Some(input) = item.downcast::<i_slint_core::items::TextInput>() else {
            return 0;
        };
        let item_origin = item.map_to_native_window(item.geometry().origin);
        let local = i_slint_core::lengths::LogicalPoint::new(
            window_x - item_origin.x,
            window_y - item_origin.y,
        );
        let adapter = i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter();
        let (byte_offset, _) =
            i_slint_core::textlayout::sharedparley::text_input_byte_offset_for_position(
                adapter.renderer(),
                input.as_pin_ref(),
                &item,
                local,
                adapter.renderer().text_layout_cache(),
            );
        byte_to_utf16_offset(&input.as_pin_ref().text(), byte_offset)
    })
}

#[unsafe(no_mangle)]
extern "C" fn native_selection_color(editor_index: usize) -> u32 {
    APP.with(|slot| {
        let Some(app) = slot.borrow().as_ref().and_then(slint::Weak::upgrade) else {
            return 0xff007aff;
        };
        let Some(item) = editor_item(&app, editor_index) else {
            return 0xff007aff;
        };
        let Some(input) = item.downcast::<i_slint_core::items::TextInput>() else {
            return 0xff007aff;
        };
        input.as_pin_ref().selection_background_color().with_alpha(1.).as_argb_encoded()
    })
}

#[unsafe(no_mangle)]
extern "C" fn native_editor_active(editor_index: usize, active: bool) {
    APP.with(|slot| {
        if let Some(app) = slot.borrow().as_ref().and_then(slint::Weak::upgrade) {
            if active {
                app.set_active_editor(editor_index as i32);
            } else if app.get_active_editor() == editor_index as i32 {
                app.set_active_editor(-1);
            }
            if let Some(item) = editor_item(&app, editor_index) {
                let input = item.downcast::<i_slint_core::items::TextInput>().unwrap();
                let window = i_slint_core::window::WindowInner::from_pub(app.window());
                if active {
                    for other_index in 0..2 {
                        if other_index == editor_index {
                            continue;
                        }
                        if let Some(other_item) = editor_item(&app, other_index) {
                            if let Some(other_input) =
                                other_item.downcast::<i_slint_core::items::TextInput>()
                            {
                                other_input.as_pin_ref().has_focus.set(false);
                                other_input.as_pin_ref().cursor_visible.set(false);
                            }
                        }
                    }
                    let focused_item = window.focus_item.borrow().clone().upgrade();
                    if let Some(focused_item) = focused_item {
                        window.set_focus_item(
                            &focused_item,
                            false,
                            i_slint_core::input::FocusReason::Programmatic,
                        );
                    }
                    input.as_pin_ref().has_focus.set(true);
                    window.set_cursor_blink_binding(&input.as_pin_ref().cursor_visible);
                } else {
                    input.as_pin_ref().has_focus.set(false);
                    input.as_pin_ref().cursor_visible.set(false);
                }
            }
        }
    });
}

#[unsafe(no_mangle)]
extern "C" fn native_menu_button_pressed(pressed: bool) {
    APP.with(|slot| {
        if let Some(app) = slot.borrow().as_ref().and_then(slint::Weak::upgrade) {
            app.set_menu_button_pressed(pressed);
        }
    });
}

fn update_scroll_geometry(app: &NativeOverlayDemo) {
    unsafe {
        update_native_scroll_geometry(
            app.get_second_editor_x(),
            app.get_second_editor_y(),
            app.get_second_editor_width(),
            app.get_second_editor_height(),
            app.get_scroll_clip_x(),
            app.get_scroll_clip_y(),
            app.get_scroll_clip_width(),
            app.get_scroll_clip_height(),
        );
    }
}

#[unsafe(no_mangle)]
extern "C" fn native_scroll_requested(direction: i32) {
    APP.with(|slot| {
        if let Some(app) = slot.borrow().as_ref().and_then(slint::Weak::upgrade) {
            let delta = if direction < 0 { -36. } else { 36. };
            app.set_scroll_offset((app.get_scroll_offset() + delta).clamp(-100., 0.));
        }
    });
}

#[unsafe(no_mangle)]
extern "C" fn native_context_action(action: i32) {
    let result = match action {
        0 => "Rename selected",
        1 => "Duplicate selected",
        2 => "Delete selected",
        _ => "Unknown action",
    };
    APP.with(|slot| {
        if let Some(app) = slot.borrow().as_ref().and_then(slint::Weak::upgrade) {
            app.set_menu_result(result.into());
        }
    });
}

unsafe extern "C" {
    fn install_native_overlays(
        host: *mut c_void,
        editor_x: f32,
        editor_y: f32,
        editor_width: f32,
        editor_height: f32,
        second_editor_x: f32,
        second_editor_y: f32,
        second_editor_width: f32,
        second_editor_height: f32,
        scroll_clip_x: f32,
        scroll_clip_y: f32,
        scroll_clip_width: f32,
        scroll_clip_height: f32,
        scroll_up_x: f32,
        scroll_up_y: f32,
        scroll_up_width: f32,
        scroll_up_height: f32,
        scroll_down_x: f32,
        scroll_down_y: f32,
        scroll_down_width: f32,
        scroll_down_height: f32,
        native_editor_x: f32,
        native_editor_y: f32,
        native_editor_width: f32,
        native_editor_height: f32,
        menu_x: f32,
        menu_y: f32,
        menu_width: f32,
        menu_height: f32,
        initial_text: *const c_char,
        second_initial_text: *const c_char,
    );
    fn update_native_scroll_geometry(
        second_editor_x: f32,
        second_editor_y: f32,
        second_editor_width: f32,
        second_editor_height: f32,
        scroll_clip_x: f32,
        scroll_clip_y: f32,
        scroll_clip_width: f32,
        scroll_clip_height: f32,
    );
}

fn main() {
    let app = NativeOverlayDemo::new().unwrap();
    APP.with(|slot| *slot.borrow_mut() = Some(app.as_weak()));
    let weak = app.as_weak();
    app.on_scroll_geometry_changed(move || {
        if let Some(app) = weak.upgrade() {
            update_scroll_geometry(&app);
        }
    });
    let weak = app.as_weak();
    slint::spawn_local(async move {
        let app = weak.unwrap();
        let window = app.window().winit_window().await.unwrap();
        let RawWindowHandle::UiKit(handle) = window.window_handle().unwrap().as_raw() else {
            panic!("This demo requires iOS");
        };
        let text = std::ffi::CString::new(app.get_editor_text().as_str()).unwrap();
        let second_text = std::ffi::CString::new(app.get_second_editor_text().as_str()).unwrap();
        unsafe {
            install_native_overlays(
                handle.ui_view.as_ptr(),
                app.get_editor_x(),
                app.get_editor_y(),
                app.get_editor_width(),
                app.get_editor_height(),
                app.get_second_editor_x(),
                app.get_second_editor_y(),
                app.get_second_editor_width(),
                app.get_second_editor_height(),
                app.get_scroll_clip_x(),
                app.get_scroll_clip_y(),
                app.get_scroll_clip_width(),
                app.get_scroll_clip_height(),
                app.get_scroll_up_x(),
                app.get_scroll_up_y(),
                app.get_scroll_up_width(),
                app.get_scroll_up_height(),
                app.get_scroll_down_x(),
                app.get_scroll_down_y(),
                app.get_scroll_down_width(),
                app.get_scroll_down_height(),
                app.get_native_editor_x(),
                app.get_native_editor_y(),
                app.get_native_editor_width(),
                app.get_native_editor_height(),
                app.get_menu_x(),
                app.get_menu_y(),
                app.get_menu_width(),
                app.get_menu_height(),
                text.as_ptr(),
                second_text.as_ptr(),
            );
        }
    })
    .unwrap();
    app.run().unwrap();
}
