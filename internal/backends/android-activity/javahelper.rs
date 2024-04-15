// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use super::*;
use i_slint_core::api::{PhysicalPosition, PhysicalSize};
use i_slint_core::graphics::{euclid, Color};
use i_slint_core::items::{ColorScheme, InputType};
use i_slint_core::platform::WindowAdapter;
use i_slint_core::SharedString;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jboolean, jint};
use jni::JNIEnv;
use std::time::Duration;

#[track_caller]
pub fn print_jni_error(app: &AndroidApp, e: jni::errors::Error) -> ! {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut _) }.unwrap();
    let env = vm.attach_current_thread().unwrap();
    let _ = env.exception_describe();
    panic!("JNI error: {e:?}")
}

pub struct JavaHelper(jni::objects::GlobalRef, AndroidApp);

fn load_java_helper(app: &AndroidApp) -> Result<jni::objects::GlobalRef, jni::errors::Error> {
    // Safety: as documented in android-activity to obtain a jni::JavaVM
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut _) }?;
    let native_activity = unsafe { JObject::from_raw(app.activity_as_ptr() as *mut _) };

    let mut env = vm.attach_current_thread()?;

    let dex_data = include_bytes!(concat!(env!("OUT_DIR"), "/classes.dex"));

    // Safety: dex_data is 'static and the InMemoryDexClassLoader will not mutate it it
    let dex_buffer =
        unsafe { env.new_direct_byte_buffer(dex_data.as_ptr() as *mut _, dex_data.len()).unwrap() };

    let dex_loader = env.new_object(
        "dalvik/system/InMemoryDexClassLoader",
        "(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V",
        &[JValue::Object(&dex_buffer), JValue::Object(&JObject::null())],
    )?;

    let class_name = env.new_string("SlintAndroidJavaHelper")?;
    let helper_class = env
        .call_method(
            dex_loader,
            "findClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&class_name)],
        )?
        .l()?;
    let helper_class: JClass = helper_class.into();

    let methods = [
        jni::NativeMethod {
            name: "updateText".into(),
            sig: "(Ljava/lang/String;IIII)V".into(),
            fn_ptr: Java_SlintAndroidJavaHelper_updateText as *mut _,
        },
        jni::NativeMethod {
            name: "setNightMode".into(),
            sig: "(I)V".into(),
            fn_ptr: Java_SlintAndroidJavaHelper_setNightMode as *mut _,
        },
        jni::NativeMethod {
            name: "moveCursorHandle".into(),
            sig: "(III)V".into(),
            fn_ptr: Java_SlintAndroidJavaHelper_moveCursorHandle as *mut _,
        },
        jni::NativeMethod {
            name: "popupMenuAction".into(),
            sig: "(I)V".into(),
            fn_ptr: Java_SlintAndroidJavaHelper_popupMenuAction as *mut _,
        },
    ];
    env.register_native_methods(&helper_class, &methods)?;

    let helper_instance = env.new_object(
        helper_class,
        "(Landroid/app/Activity;)V",
        &[JValue::Object(&native_activity)],
    )?;
    Ok(env.new_global_ref(&helper_instance)?)
}

impl JavaHelper {
    pub fn new(app: &AndroidApp) -> Result<Self, jni::errors::Error> {
        Ok(Self(load_java_helper(app)?, app.clone()))
    }

    fn with_jni_env<R>(
        &self,
        f: impl FnOnce(&mut JNIEnv, &JObject<'static>) -> Result<R, jni::errors::Error>,
    ) -> Result<R, jni::errors::Error> {
        // Safety: as documented in android-activity to obtain a jni::JavaVM
        let vm = unsafe { jni::JavaVM::from_raw(self.1.vm_as_ptr() as *mut _) }?;
        let mut env = vm.attach_current_thread()?;
        let helper = self.0.as_obj();
        f(&mut env, helper)
    }

    /// Unfortunately, the way that the android-activity crate uses to show or hide the virtual keyboard doesn't
    /// work with native-activity. So do it manually with JNI
    pub fn show_or_hide_soft_input(&self, show: bool) -> Result<(), jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            if show {
                env.call_method(helper, "show_keyboard", "()V", &[])?;
            } else {
                env.call_method(helper, "hide_keyboard", "()V", &[])?;
            };
            Ok(())
        })
    }

    pub fn set_imm_data(
        &self,
        data: &i_slint_core::window::InputMethodProperties,
        scale_factor: f32,
        show_cursor_handles: bool,
    ) -> Result<(), jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            let mut text = data.text.to_string();
            let mut cursor_position = data.cursor_position;
            let mut anchor_position = data.anchor_position.unwrap_or(data.cursor_position);

            if !data.preedit_text.is_empty() {
                text.insert_str(data.preedit_offset, data.preedit_text.as_str());
                if cursor_position >= data.preedit_offset {
                    cursor_position += data.preedit_text.len()
                }
                if anchor_position >= data.preedit_offset {
                    anchor_position += data.preedit_text.len()
                }
            }

            let to_utf16 = |x| convert_utf8_index_to_utf16(&text, x as usize);
            let text = &env.new_string(text.as_str())?;

            let class_it = env.find_class("android/text/InputType")?;
            let input_type = match data.input_type {
                InputType::Text => env.get_static_field(&class_it, "TYPE_CLASS_TEXT", "I")?.i()?,
                InputType::Password => {
                    env.get_static_field(&class_it, "TYPE_TEXT_VARIATION_PASSWORD", "I")?.i()?
                        | env.get_static_field(&class_it, "TYPE_CLASS_TEXT", "I")?.i()?
                }
                InputType::Number => {
                    env.get_static_field(&class_it, "TYPE_CLASS_NUMBER", "I")?.i()?
                }
                InputType::Decimal => {
                    env.get_static_field(&class_it, "TYPE_CLASS_NUMBER", "I")?.i()?
                        | env.get_static_field(&class_it, "TYPE_NUMBER_FLAG_DECIMAL", "I")?.i()?
                }
                _ => 0 as jint,
            };

            let cur_origin = data.cursor_rect_origin.to_physical(scale_factor);
            let anchor_origin = data.anchor_point.to_physical(scale_factor);
            let cur_size = data.cursor_rect_size.to_physical(scale_factor);

            // Add 2*cur_size.width to the y position to be a bit under the cursor
            let cursor_height = cur_size.height as i32 + 2 * cur_size.width as i32;
            let cur_x = cur_origin.x + cur_size.width as i32 / 2;
            let cur_y = cur_origin.y + cursor_height;
            let anchor_x = anchor_origin.x;
            let anchor_y = anchor_origin.y + 2 * cur_size.width as i32;

            env.call_method(
                helper,
                "set_imm_data",
                "(Ljava/lang/String;IIIIIIIIIIZ)V",
                &[
                    JValue::Object(&text),
                    JValue::from(to_utf16(cursor_position) as jint),
                    JValue::from(to_utf16(anchor_position) as jint),
                    JValue::from(to_utf16(data.preedit_offset) as jint),
                    JValue::from(to_utf16(data.preedit_offset + data.preedit_text.len()) as jint),
                    JValue::from(cur_x as jint),
                    JValue::from(cur_y as jint),
                    JValue::from(anchor_x as jint),
                    JValue::from(anchor_y as jint),
                    JValue::from(cursor_height as jint),
                    JValue::from(input_type),
                    JValue::from(show_cursor_handles as jboolean),
                ],
            )?;

            Ok(())
        })
    }

    pub fn color_scheme(&self) -> Result<i32, jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            Ok(env.call_method(helper, "color_scheme", "()I", &[])?.i()?)
        })
    }

    pub fn get_view_rect(&self) -> Result<(PhysicalPosition, PhysicalSize), jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            let rect =
                env.call_method(helper, "get_view_rect", "()Landroid/graphics/Rect;", &[])?.l()?;
            let x = env.get_field(&rect, "left", "I")?.i()?;
            let y = env.get_field(&rect, "top", "I")?.i()?;
            let width = env.get_field(&rect, "right", "I")?.i()? - x;
            let height = env.get_field(&rect, "bottom", "I")?.i()? - y;
            Ok((PhysicalPosition::new(x as _, y as _), PhysicalSize::new(width as _, height as _)))
        })
    }

    pub fn set_handle_color(&self, color: Color) -> Result<(), jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            env.call_method(
                helper,
                "set_handle_color",
                "(I)V",
                &[JValue::from(color.as_argb_encoded() as jint)],
            )?;
            Ok(())
        })
    }

    pub fn long_press_timeout(&self) -> Result<Duration, jni::errors::Error> {
        self.with_jni_env(|env, _helper| {
            let view_configuration = env.find_class("android/view/ViewConfiguration")?;
            let view_configuration = JClass::from(view_configuration);
            let long_press_timeout = env
                .call_static_method(view_configuration, "getLongPressTimeout", "()I", &[])?
                .i()?;
            Ok(Duration::from_millis(long_press_timeout as _))
        })
    }

    pub fn show_action_menu(&self) -> Result<(), jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            env.call_method(helper, "show_action_menu", "()V", &[])?;
            Ok(())
        })
    }

    pub fn set_clipboard(&self, text: &str) -> Result<(), jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            let text = &env.new_string(text)?;
            env.call_method(
                helper,
                "set_clipboard",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&text)],
            )?;
            Ok(())
        })
    }

    pub fn get_clipboard(&self) -> Result<String, jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            let j_string =
                env.call_method(helper, "get_clipboard", "()Ljava/lang/String;", &[])?.l()?;
            let string = env.get_string(&j_string.into())?.into();
            Ok(string)
        })
    }
}

#[no_mangle]
extern "system" fn Java_SlintAndroidJavaHelper_updateText(
    mut env: JNIEnv,
    _class: JClass,
    text: JString,
    cursor_position: jint,
    anchor_position: jint,
    preedit_start: jint,
    preedit_end: jint,
) {
    fn make_shared_string(env: &mut JNIEnv, string: &JString) -> Option<SharedString> {
        let java_str = env.get_string(&string).ok()?;
        let decoded: std::borrow::Cow<str> = (&java_str).into();
        Some(SharedString::from(decoded.as_ref()))
    }
    let Some(text) = make_shared_string(&mut env, &text) else { return };

    let cursor_position = convert_utf16_index_to_utf8(&text, cursor_position as usize);
    let anchor_position = convert_utf16_index_to_utf8(&text, anchor_position as usize);
    let preedit_start = convert_utf16_index_to_utf8(&text, preedit_start as usize);
    let preedit_end = convert_utf16_index_to_utf8(&text, preedit_end as usize);

    i_slint_core::api::invoke_from_event_loop(move || {
        if let Some(adaptor) = CURRENT_WINDOW.with_borrow(|x| x.upgrade()) {
            adaptor.show_cursor_handles.set(false);
            let runtime_window = i_slint_core::window::WindowInner::from_pub(&adaptor.window);
            let event = if preedit_start != preedit_end {
                let adjust = |pos| if pos <= preedit_start { pos } else if pos >= preedit_end { pos - preedit_end + preedit_start } else { preedit_start } as i32;
                i_slint_core::input::KeyEvent {
                    event_type: i_slint_core::input::KeyEventType::UpdateComposition,
                    text: i_slint_core::format!( "{}{}", &text[..preedit_start], &text[preedit_end..]),
                    preedit_text: text[preedit_start..preedit_end].into(),
                    preedit_selection: Some(0..(preedit_end - preedit_start) as i32),
                    replacement_range: Some(i32::MIN..i32::MAX),
                    cursor_position: Some(adjust(cursor_position)),
                    anchor_position: Some(adjust(anchor_position)),
                    ..Default::default()
                }
            } else {
                i_slint_core::input::KeyEvent {
                    event_type: i_slint_core::input::KeyEventType::CommitComposition,
                    text,
                    replacement_range: Some(i32::MIN..i32::MAX),
                    cursor_position: Some(cursor_position as _),
                    anchor_position: Some(anchor_position as _),
                    ..Default::default()
                }
            };
            runtime_window.process_key_input(event);
        }
    })
    .unwrap()
}

fn convert_utf16_index_to_utf8(in_str: &str, utf16_index: usize) -> usize {
    let mut utf16_counter = 0;

    for (utf8_index, c) in in_str.char_indices() {
        if utf16_counter >= utf16_index {
            return utf8_index;
        }
        utf16_counter += c.len_utf16();
    }
    in_str.len()
}

fn convert_utf8_index_to_utf16(in_str: &str, utf8_index: usize) -> usize {
    in_str[..utf8_index].encode_utf16().count()
}

#[no_mangle]
extern "system" fn Java_SlintAndroidJavaHelper_setNightMode(
    _env: JNIEnv,
    _class: JClass,
    night_mode: jint,
) {
    i_slint_core::api::invoke_from_event_loop(move || {
        if let Some(w) = CURRENT_WINDOW.with_borrow(|x| x.upgrade()) {
            w.color_scheme.as_ref().set(match night_mode {
                0x10 => ColorScheme::Light,  // UI_MODE_NIGHT_NO(0x10)
                0x20 => ColorScheme::Dark,   // UI_MODE_NIGHT_YES(0x20)
                0x0 => ColorScheme::Unknown, // UI_MODE_NIGHT_UNDEFINED
                _ => ColorScheme::Unknown,
            });
        }
    })
    .unwrap()
}

#[no_mangle]
extern "system" fn Java_SlintAndroidJavaHelper_moveCursorHandle(
    _env: JNIEnv,
    _class: JClass,
    id: jint,
    pos_x: jint,
    pos_y: jint,
) {
    i_slint_core::api::invoke_from_event_loop(move || {
        if let Some(adaptor) = CURRENT_WINDOW.with_borrow(|x| x.upgrade()) {
            if let Some(focus_item) = i_slint_core::window::WindowInner::from_pub(&adaptor.window)
                .focus_item
                .borrow()
                .upgrade()
            {
                if let Some(text_input) = focus_item.downcast::<i_slint_core::items::TextInput>() {
                    let scale_factor = adaptor.window.scale_factor();
                    let adaptor = adaptor.clone() as Rc<dyn WindowAdapter>;
                    let size = text_input
                        .as_pin_ref()
                        .font_request(&adaptor)
                        .pixel_size
                        .unwrap_or_default()
                        .get();
                    let pos =
                        euclid::point2(
                            pos_x as f32 / scale_factor,
                            pos_y as f32 / scale_factor - size / 2.,
                        ) - focus_item.map_to_window(focus_item.geometry().origin).to_vector();
                    let text_pos = text_input.as_pin_ref().byte_offset_for_position(pos, &adaptor);

                    let cur_pos = if id == 0 {
                        text_input.anchor_position_byte_offset.set(text_pos as i32);
                        text_pos as i32
                    } else {
                        let current_cursor = text_input.as_pin_ref().cursor_position_byte_offset();
                        let current_anchor = text_input.as_pin_ref().anchor_position_byte_offset();
                        if (id == 1 && current_anchor < current_cursor)
                            || (id == 2 && current_anchor > current_cursor)
                        {
                            if current_cursor == text_pos as i32 {
                                return;
                            }
                            text_input.anchor_position_byte_offset.set(text_pos as i32);
                            current_cursor
                        } else {
                            if current_anchor == text_pos as i32 {
                                return;
                            }
                            text_pos as i32
                        }
                    };

                    text_input.as_pin_ref().set_cursor_position(
                        cur_pos,
                        true,
                        i_slint_core::items::TextChangeNotify::TriggerCallbacks,
                        &adaptor,
                        &focus_item,
                    );
                }
            }
        }
    })
    .unwrap()
}

#[no_mangle]
extern "system" fn Java_SlintAndroidJavaHelper_popupMenuAction(
    _env: JNIEnv,
    _class: JClass,
    id: jint,
) {
    i_slint_core::api::invoke_from_event_loop(move || {
        if let Some(adaptor) = CURRENT_WINDOW.with_borrow(|x| x.upgrade()) {
            if let Some(focus_item) = i_slint_core::window::WindowInner::from_pub(&adaptor.window)
                .focus_item
                .borrow()
                .upgrade()
            {
                if let Some(text_input) = focus_item.downcast::<i_slint_core::items::TextInput>() {
                    let text_input = text_input.as_pin_ref();
                    let adaptor = adaptor.clone() as Rc<dyn WindowAdapter>;
                    match id {
                        0 => text_input.cut(&adaptor, &focus_item),
                        1 => text_input.copy(&adaptor, &focus_item),
                        2 => text_input.paste(&adaptor, &focus_item),
                        3 => text_input.select_all(&adaptor, &focus_item),
                        _ => (),
                    }
                }
            }
        }
    })
    .unwrap()
}
