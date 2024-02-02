// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use super::*;
use i_slint_core::api::{PhysicalPosition, PhysicalSize};
use i_slint_core::items::InputType;
use i_slint_core::SharedString;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jboolean, jint};
use jni::JNIEnv;

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
            sig: "(Ljava/lang/String;IILjava/lang/String;I)V".into(),
            fn_ptr: Java_SlintAndroidJavaHelper_updateText as *mut _,
        },
        jni::NativeMethod {
            name: "setDarkMode".into(),
            sig: "(Z)V".into(),
            fn_ptr: Java_SlintAndroidJavaHelper_setDarkMode as *mut _,
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
    ) -> Result<(), jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            let text = &env.new_string(data.text.as_str())?;
            let preedit_text = env.new_string(data.preedit_text.as_str())?;
            let to_utf16 = |x| convert_utf8_index_to_utf16(&data.text, x as usize);

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
            env.call_method(
                helper,
                "set_imm_data",
                "(Ljava/lang/String;IILjava/lang/String;IIIIII)V",
                &[
                    JValue::Object(&text),
                    JValue::from(to_utf16(data.cursor_position) as jint),
                    JValue::from(
                        to_utf16(data.anchor_position.unwrap_or(data.cursor_position)) as jint
                    ),
                    JValue::Object(&preedit_text),
                    JValue::from(to_utf16(data.preedit_offset) as jint),
                    JValue::from(data.cursor_rect_origin.x as jint),
                    JValue::from(data.cursor_rect_origin.y as jint),
                    JValue::from(data.cursor_rect_size.width as jint),
                    JValue::from(data.cursor_rect_size.height as jint),
                    JValue::from(input_type),
                ],
            )?;

            Ok(())
        })
    }

    pub fn dark_color_scheme(&self) -> Result<bool, jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            Ok(env.call_method(helper, "dark_color_scheme", "()Z", &[])?.z()?)
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
}

#[no_mangle]
extern "system" fn Java_SlintAndroidJavaHelper_updateText(
    mut env: JNIEnv,
    _class: JClass,
    text: JString,
    cursor_position: jint,
    anchor_position: jint,
    preedit: JString,
    preedit_offset: jint,
) {
    fn make_shared_string(env: &mut JNIEnv, string: &JString) -> Option<SharedString> {
        let java_str = env.get_string(&string).ok()?;
        let decoded: std::borrow::Cow<str> = (&java_str).into();
        Some(SharedString::from(decoded.as_ref()))
    }
    let Some(text) = make_shared_string(&mut env, &text) else { return };
    let Some(preedit) = make_shared_string(&mut env, &preedit) else { return };
    let cursor_position = convert_utf16_index_to_utf8(&text, cursor_position as usize);
    let anchor_position = convert_utf16_index_to_utf8(&text, anchor_position as usize);
    let preedit_offset = convert_utf16_index_to_utf8(&text, preedit_offset as usize) as i32;

    i_slint_core::api::invoke_from_event_loop(move || {
        if let Some(adaptor) = CURRENT_WINDOW.with_borrow(|x| x.upgrade()) {
            let runtime_window = i_slint_core::window::WindowInner::from_pub(&adaptor.window);
            let event = i_slint_core::input::KeyEvent {
                event_type: i_slint_core::input::KeyEventType::UpdateComposition,
                text,
                replacement_range: Some(i32::MIN..i32::MAX),
                cursor_position: Some(cursor_position as _),
                anchor_position: Some(anchor_position as _),
                preedit_selection: (!preedit.is_empty())
                    .then(|| preedit_offset..(preedit_offset + preedit.len() as i32)),
                preedit_text: preedit,
                ..Default::default()
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
extern "system" fn Java_SlintAndroidJavaHelper_setDarkMode(
    _env: JNIEnv,
    _class: JClass,
    dark: jboolean,
) {
    i_slint_core::api::invoke_from_event_loop(move || {
        if let Some(w) = CURRENT_WINDOW.with_borrow(|x| x.upgrade()) {
            w.dark_color_scheme.as_ref().set(dark == jni::sys::JNI_TRUE);
        }
    })
    .unwrap()
}
