// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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

    let parent_class_loader = env
        .call_method(&native_activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
        .l()?;

    let os_build_class = env.find_class("android/os/Build$VERSION")?;
    let sdk_ver = env.get_static_field(os_build_class, "SDK_INT", "I")?.i()?;

    let dex_loader = if sdk_ver >= 26 {
        env.new_object(
            "dalvik/system/InMemoryDexClassLoader",
            "(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V",
            &[JValue::Object(&dex_buffer), JValue::Object(&parent_class_loader)],
        )?
    } else {
        // writes the dex data into the application internal storage
        let dex_data_len = dex_data.len() as i32;
        let dex_byte_array = env.byte_array_from_slice(dex_data).unwrap();

        let dex_dir = env.new_string("dex")?;
        let dex_dir_path = env
            .call_method(
                &native_activity,
                "getDir",
                "(Ljava/lang/String;I)Ljava/io/File;",
                &[JValue::Object(&dex_dir), JValue::from(0 as jint)],
            )?
            .l()?;
        let dex_name = env.new_string(env!("CARGO_CRATE_NAME").to_string() + ".dex")?;
        let dex_path = env.new_object(
            "java/io/File",
            "(Ljava/io/File;Ljava/lang/String;)V",
            &[JValue::Object(&dex_dir_path), JValue::Object(&dex_name)],
        )?;
        let dex_path =
            env.call_method(dex_path, "getAbsolutePath", "()Ljava/lang/String;", &[])?.l()?;

        // prepares the folder for optimized dex generated while creating `DexClassLoader`
        let out_dex_dir = env.new_string("outdex")?;
        let out_dex_dir_path = env
            .call_method(
                &native_activity,
                "getDir",
                "(Ljava/lang/String;I)Ljava/io/File;",
                &[JValue::Object(&out_dex_dir), JValue::from(0 as jint)],
            )?
            .l()?;
        let out_dex_dir_path = env
            .call_method(&out_dex_dir_path, "getAbsolutePath", "()Ljava/lang/String;", &[])?
            .l()?;

        // writes the dex data
        let write_stream = env.new_object(
            "java/io/FileOutputStream",
            "(Ljava/lang/String;)V",
            &[(&dex_path).into()],
        )?;
        env.call_method(
            &write_stream,
            "write",
            "([BII)V",
            &[
                JValue::Object(&dex_byte_array),
                JValue::from(0 as jint),
                JValue::from(dex_data_len as jint),
            ],
        )?;
        env.call_method(&write_stream, "close", "()V", &[])?;

        // loads the dex file
        env.new_object(
            "dalvik/system/DexClassLoader",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/ClassLoader;)V",
            &[
                JValue::Object(&dex_path),
                JValue::Object(&out_dex_dir_path),
                JValue::Object(&JObject::null()),
                JValue::Object(&parent_class_loader),
            ],
        )?
    };

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
            let text = &env.auto_local(env.new_string(text.as_str())?);

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
            env.delete_local_ref(class_it)?;

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
            let rect = env.auto_local(rect);
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
            let long_press_timeout = env
                .call_static_method(
                    "android/view/ViewConfiguration",
                    "getLongPressTimeout",
                    "()I",
                    &[],
                )?
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
            let text = env.auto_local(env.new_string(text)?);
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
            let j_string = env
                .call_method(helper, "get_clipboard", "()Ljava/lang/String;", &[])?
                .l()
                .map(|l| env.auto_local(l))?;
            let string = jni_get_string(j_string.as_ref(), env)?.into();
            Ok(string)
        })
    }
}

#[unsafe(no_mangle)]
extern "system" fn Java_SlintAndroidJavaHelper_updateText(
    mut env: JNIEnv,
    _class: JClass,
    text: JString,
    cursor_position: jint,
    anchor_position: jint,
    preedit_start: jint,
    preedit_end: jint,
) {
    let Ok(java_str) = jni_get_string(&text, &mut env) else { return };
    let decoded: std::borrow::Cow<str> = (&java_str).into();
    let text = SharedString::from(decoded.as_ref());

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

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
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

/// Workaround before <https://github.com/jni-rs/jni-rs/pull/557> is merged.
fn jni_get_string<'e, 'a>(
    obj: &'a JObject<'a>,
    env: &mut JNIEnv<'e>,
) -> Result<jni::strings::JavaStr<'e, 'a, 'a>, jni::errors::Error> {
    use jni::errors::{Error::*, JniError};

    let string_class = env.find_class("java/lang/String")?;
    let obj_class = env.get_object_class(obj)?;
    let obj_class = env.auto_local(obj_class);
    if !env.is_assignable_from(string_class, obj_class)? {
        return Err(JniCall(JniError::InvalidArguments));
    }

    let j_string: &jni::objects::JString<'_> = obj.into();
    // SAFETY: We check that the passed in Object is actually a java.lang.String
    unsafe { env.get_string_unchecked(j_string) }
}
