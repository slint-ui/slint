// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use i_slint_core::SharedString;
use i_slint_core::api::{PhysicalPosition, PhysicalSize};
use i_slint_core::graphics::{Color, euclid};
use i_slint_core::items::{ColorScheme, InputType};
use i_slint_core::lengths::PhysicalEdges;
use i_slint_core::platform::WindowAdapter;
use jni::objects::{JClass, JClassLoader, JString, LoaderContext};
use jni::sys::jint;
use jni::{Env, JavaVM, bind_java_type};
use std::sync::OnceLock;
use std::time::Duration;

const DEX_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/classes.dex"));

bind_java_type! {
    SlintAndroidJavaHelper => ".SlintAndroidJavaHelper",
    type_map = {
        AndroidActivity => "android.app.Activity",
        AndroidRect => "android.graphics.Rect",
    },
    constructors {
        fn new(activity: AndroidActivity),
    },
    methods {
        fn color_scheme {
            name = "color_scheme",
            sig = () -> jint,
        },
        fn get_clipboard {
            name = "get_clipboard",
            sig = () -> JString,
        },
        fn get_safe_area {
            name = "get_safe_area",
            sig = () -> AndroidRect,
        },
        fn get_view_rect {
            name = "get_view_rect",
            sig = () -> AndroidRect,
        },
        fn hide_keyboard {
            name = "hide_keyboard",
            sig = (),
        },
        fn set_clipboard {
            name = "set_clipboard",
            sig = (text: JString),
        },
        fn set_handle_color {
            name = "set_handle_color",
            sig = (color: jint),
        },
        fn set_imm_data {
            name = "set_imm_data",
            sig = (
                text: JString,
                cursor_position: jint,
                anchor_position: jint,
                preedit_start: jint,
                preedit_end: jint,
                cur_x: jint,
                cur_y: jint,
                anchor_x: jint,
                anchor_y: jint,
                cursor_height: jint,
                input_type: jint,
                show_cursor_handles: jboolean
            ),
        },
        fn show_action_menu {
            name = "show_action_menu",
            sig = (),
        },
        fn show_keyboard {
            name = "show_keyboard",
            sig = (),
        },
    },
    native_methods_export = false,
    native_methods {
        pub static fn move_cursor_handle {
            sig = (id: jint, pos_x: jint, pos_y: jint) -> (),
            fn = callback_move_cursor_handle,
        },
        pub static fn popup_menu_action {
            sig = (id: jint) -> (),
            fn = callback_popup_menu_action,
        },
        pub static fn set_insets {
            sig = (
                window_top: jint,
                window_left: jint,
                window_bottom: jint,
                window_right: jint,
                safe_area_top: jint,
                safe_area_left: jint,
                safe_area_bottom: jint,
                safe_area_right: jint,
                keyboard_top: jint,
                keyboard_left: jint,
                keyboard_bottom: jint,
                keyboard_right: jint
            ) -> (),
            fn = callback_set_insets,
        },
        pub static fn set_night_mode {
            sig = (night_mode: jint) -> (),
            fn = callback_set_night_mode,
        },
        pub static fn update_text {
            sig = (
                    text: JString,
                    cursor_position: jint,
                    anchor_position: jint,
                    preedit_start: jint,
                    preedit_offset: jint
            ) -> (),
            fn = callback_update_text,
        },
    },
}

bind_java_type! {
    AndroidActivity => "android.app.Activity",
    type_map = {
        AndroidContext => "android.content.Context",
    },
    is_instance_of = {
        AndroidContext,
    }
}

bind_java_type! {
    AndroidContext => "android.content.Context",
    type_map = {
        JFile => "java.io.File",
    },
    methods {
        fn get_files_dir() -> JFile,
        fn get_cache_dir() -> JFile,
        fn get_code_cache_dir() -> JFile, // requires API level >= 21
        fn get_class_loader() -> JClassLoader,
        fn get_package_name() -> JString,
    }
}

bind_java_type! {
    AndroidBuildVersion => "android.os.Build$VERSION",
    fields {
        #[allow(non_snake_case)]
        static SDK_INT {
            sig = jint,
            get = SDK_INT,
        },
    },
}

bind_java_type! {
    AndroidRect => "android.graphics.Rect",
    fields {
        bottom: jint,
        left: jint,
        right: jint,
        top: jint,
    },
}

bind_java_type! {
    AndroidInputType => "android.text.InputType",
    fields {
        #[allow(non_snake_case)]
        static TYPE_CLASS_NUMBER {
            sig = jint,
            get = TYPE_CLASS_NUMBER,
        },
        #[allow(non_snake_case)]
        static TYPE_CLASS_TEXT {
            sig = jint,
            get = TYPE_CLASS_TEXT,
        },
        #[allow(non_snake_case)]
        static TYPE_TEXT_VARIATION_PASSWORD {
            sig = jint,
            get = TYPE_TEXT_VARIATION_PASSWORD,
        },
        #[allow(non_snake_case)]
        static TYPE_NUMBER_FLAG_DECIMAL {
            sig = jint,
            get = TYPE_NUMBER_FLAG_DECIMAL,
        },
    }
}

bind_java_type! {
    AndroidViewConfiguration => "android.view.ViewConfiguration",
    methods {
        static fn get_long_press_timeout() -> jint,
    }
}

bind_java_type! {
    JFile => "java.io.File",
    methods {
        fn get_absolute_path() -> JString,
    }
}

bind_java_type! {
    InMemoryDexClassLoader => "dalvik.system.InMemoryDexClassLoader",
    constructors {
        fn new(dex_buffer: JByteBuffer, parent: JClassLoader),
    },
    is_instance_of = {
        JClassLoader,
    }
}

bind_java_type! {
    DexFileClassLoader => "dalvik.system.DexClassLoader",
    constructors {
        fn new(dex_path: JString, optimized_directory: JString, library_search_path: JString, parent: JClassLoader),
    },
    is_instance_of = {
        JClassLoader,
    }
}

// See `AttachmentExceptionPolicy::PreReThrowPostCatch` in `jni` crate.
#[track_caller]
pub fn print_jni_error(_app: &AndroidApp, e: jni::errors::Error) -> ! {
    panic!("JNI error: {e:#?}")
}

#[allow(dead_code)]
pub struct JavaHelper(jni::refs::Global<SlintAndroidJavaHelper<'static>>, AndroidApp);

fn get_helper_class_loader(
    env: &mut Env,
    native_activity: &AndroidActivity<'_>,
) -> Result<&'static JClassLoader<'static>, jni::errors::Error> {
    static DEX_CLASS_LOADER: OnceLock<jni::refs::Global<JClassLoader<'static>>> = OnceLock::new();

    fn build_dex_class_loader<'local>(
        env: &mut Env<'local>,
        native_activity: &AndroidActivity<'_>,
    ) -> Result<JClassLoader<'local>, jni::errors::Error> {
        let native_activity = env.new_local_ref(native_activity)?;
        let app_context = AndroidContext::cast_local(env, native_activity)?;
        let context_class_loader = app_context.get_class_loader(env)?;

        if AndroidBuildVersion::SDK_INT(env)? >= 26 {
            // Safety: DEX_DATA is 'static and the `InMemoryDexClassLoader`` will not mutate it
            let dex_buffer =
                unsafe { env.new_direct_byte_buffer(DEX_DATA.as_ptr() as *mut _, DEX_DATA.len()) }?;
            let dex_loader = InMemoryDexClassLoader::new(env, &dex_buffer, &context_class_loader)?;
            JClassLoader::cast_local(env, dex_loader)
        } else {
            // The dex data must be written in a file; this determines the output
            // directory path inside the application code cache directory.
            let code_cache_path = app_context
                .get_code_cache_dir(env)?
                .get_absolute_path(env)
                .map(|p| std::path::PathBuf::from(p.to_string()))?;

            let dex_name = env!("CARGO_CRATE_NAME").to_string() + ".dex";
            let dex_file_path = code_cache_path.join(dex_name);
            std::fs::write(&dex_file_path, DEX_DATA).unwrap(); // Note: this panics on failure
            let dex_file_path = JString::new(env, dex_file_path.to_string_lossy())?;

            // creates the oats directory
            let oats_dir_path = code_cache_path.join("oats");
            let _ = std::fs::create_dir(&oats_dir_path);
            let oats_dir_path = JString::new(env, oats_dir_path.to_string_lossy())?;

            // loads the dex file
            let dex_loader = DexFileClassLoader::new(
                env,
                &dex_file_path,
                &oats_dir_path,
                JString::null(),
                &context_class_loader,
            )?;
            JClassLoader::cast_local(env, dex_loader)
        }
    }

    if DEX_CLASS_LOADER.get().is_none() {
        let loader = build_dex_class_loader(env, native_activity)?;
        let loader = env.new_global_ref(loader)?;
        let _ = DEX_CLASS_LOADER.set(loader);
    }
    Ok(DEX_CLASS_LOADER.get().unwrap())
}

fn load_java_helper(
    app: &AndroidApp,
) -> Result<jni::refs::Global<SlintAndroidJavaHelper<'static>>, jni::errors::Error> {
    let jvm = JavaVM::singleton().unwrap_or_else(|_| unsafe {
        // Safety: as documented in android-activity to obtain a jni::JavaVM
        JavaVM::from_raw(app.vm_as_ptr() as *mut _) // this initializes the `JavaVM::singleton()`
    });
    jvm.attach_current_thread(|env| {
        let native_activity_ptr = app.activity_as_ptr().cast();
        let native_activity =
            unsafe { env.as_cast_raw::<jni::refs::Global<AndroidActivity>>(&native_activity_ptr)? };
        let loader = LoaderContext::Loader(get_helper_class_loader(env, native_activity.as_ref())?);
        let _ = SlintAndroidJavaHelperAPI::get(env, &loader)?;
        let helper_instance = SlintAndroidJavaHelper::new(env, native_activity)?;
        env.new_global_ref(&helper_instance)
    })
}

impl JavaHelper {
    pub fn new(app: &AndroidApp) -> Result<Self, jni::errors::Error> {
        Ok(Self(load_java_helper(app)?, app.clone()))
    }

    fn with_jni_env<R>(
        &self,
        f: impl FnOnce(&mut Env, &SlintAndroidJavaHelper<'static>) -> Result<R, jni::errors::Error>,
    ) -> Result<R, jni::errors::Error> {
        JavaVM::singleton()?.attach_current_thread(|env| {
            let helper = self.0.as_ref();
            f(env, helper)
        })
    }

    /// Unfortunately, the way that the android-activity crate uses to show or hide the virtual keyboard doesn't
    /// work with native-activity. So do it manually with JNI
    pub fn show_or_hide_soft_input(&self, show: bool) -> Result<(), jni::errors::Error> {
        self.with_jni_env(
            |env, helper| {
                if show { helper.show_keyboard(env) } else { helper.hide_keyboard(env) }
            },
        )
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

            let to_utf16 = |x| convert_utf8_index_to_utf16(&text, x);
            let text = JString::new(env, text.as_str())?;

            let input_type = match data.input_type {
                InputType::Text => AndroidInputType::TYPE_CLASS_TEXT(env)?,
                InputType::Password => {
                    AndroidInputType::TYPE_TEXT_VARIATION_PASSWORD(env)?
                        | AndroidInputType::TYPE_CLASS_TEXT(env)?
                }
                InputType::Number => AndroidInputType::TYPE_CLASS_NUMBER(env)?,
                InputType::Decimal => {
                    AndroidInputType::TYPE_CLASS_NUMBER(env)?
                        | AndroidInputType::TYPE_NUMBER_FLAG_DECIMAL(env)?
                }
                _ => 0 as jint,
            };

            let cur_origin = data.cursor_rect_origin.to_physical(scale_factor); // i32
            let anchor_origin = data.anchor_point.to_physical(scale_factor);
            let cur_size = data.cursor_rect_size.to_physical(scale_factor);

            let cur_visible = data.clip_rect.map_or(true, |r| {
                r.contains(i_slint_core::lengths::logical_point_from_api(data.cursor_rect_origin))
            });
            let anchor_visible = data.clip_rect.map_or(true, |r| {
                r.contains(i_slint_core::lengths::logical_point_from_api(data.anchor_point))
            });

            // Add 2*cur_size.width to the y position to be a bit under the cursor
            let cursor_height = cur_size.height as i32 + 2 * cur_size.width as i32;
            let cur_x = if cur_visible { cur_origin.x + cur_size.width as i32 / 2 } else { -1 };
            let cur_y = cur_origin.y + cursor_height;
            let anchor_x = if anchor_visible { anchor_origin.x } else { -1 };
            let anchor_y = anchor_origin.y + 2 * cur_size.width as i32;

            helper.set_imm_data(
                env,
                &text,
                to_utf16(cursor_position) as i32,
                to_utf16(anchor_position) as i32,
                to_utf16(data.preedit_offset) as i32,
                to_utf16(data.preedit_offset + data.preedit_text.len()) as i32,
                cur_x,
                cur_y,
                anchor_x,
                anchor_y,
                cursor_height,
                input_type,
                show_cursor_handles,
            )?;

            Ok(())
        })
    }

    pub fn color_scheme(&self) -> Result<i32, jni::errors::Error> {
        self.with_jni_env(|env, helper| helper.color_scheme(env))
    }

    pub fn get_view_rect(&self) -> Result<(PhysicalPosition, PhysicalSize), jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            let rect = helper.get_view_rect(env)?;
            let x = rect.left(env)?;
            let y = rect.top(env)?;
            let width = rect.right(env)? - x;
            let height = rect.bottom(env)? - y;
            Ok((PhysicalPosition::new(x as _, y as _), PhysicalSize::new(width as _, height as _)))
        })
    }

    pub fn get_safe_area(&self) -> Result<PhysicalEdges, jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            let rect = helper.get_safe_area(env)?;
            let left = rect.left(env)?;
            let top = rect.top(env)?;
            let right = rect.right(env)?;
            let bottom = rect.bottom(env)?;
            Ok(PhysicalEdges::new(top, bottom, left, right))
        })
    }

    pub fn set_handle_color(&self, color: Color) -> Result<(), jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            helper.set_handle_color(env, color.as_argb_encoded() as i32)
        })
    }

    pub fn long_press_timeout(&self) -> Result<Duration, jni::errors::Error> {
        self.with_jni_env(|env, _helper| {
            let long_press_timeout = AndroidViewConfiguration::get_long_press_timeout(env)?;
            Ok(Duration::from_millis(long_press_timeout as _))
        })
    }

    pub fn show_action_menu(&self) -> Result<(), jni::errors::Error> {
        self.with_jni_env(|env, helper| helper.show_action_menu(env))
    }

    pub fn set_clipboard(&self, text: &str) -> Result<(), jni::errors::Error> {
        self.with_jni_env(|env, helper| {
            let text = JString::new(env, text)?;
            helper.set_clipboard(env, &text)
        })
    }

    pub fn get_clipboard(&self) -> Result<String, jni::errors::Error> {
        self.with_jni_env(|env, helper| Ok(helper.get_clipboard(env)?.to_string()))
    }
}

fn callback_update_text<'local>(
    _env: &mut Env<'local>,
    _class: JClass<'local>,
    text: JString<'local>,
    cursor_position: jint,
    anchor_position: jint,
    preedit_start: jint,
    preedit_end: jint,
) -> Result<(), jni::errors::Error> {
    let java_str = text.to_string();
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
                let adjust = |pos| {
                    if pos <= preedit_start {
                        pos
                    } else if pos >= preedit_end {
                        preedit_start + (pos - preedit_end)
                    } else {
                        preedit_start
                    }
                } as i32;
                i_slint_core::input::KeyEvent {
                    event_type: i_slint_core::input::KeyEventType::UpdateComposition,
                    text: i_slint_core::format!(
                        "{}{}",
                        &text[..preedit_start],
                        &text[preedit_end..]
                    ),
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
    .unwrap();
    Ok(())
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

fn callback_set_night_mode<'local>(
    _env: &mut Env<'local>,
    _class: JClass<'local>,
    night_mode: jint,
) -> Result<(), jni::errors::Error> {
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
    .unwrap();
    Ok(())
}

fn callback_move_cursor_handle<'local>(
    _env: &mut Env<'local>,
    _class: JClass<'local>,
    id: jint,
    pos_x: jint,
    pos_y: jint,
) -> Result<(), jni::errors::Error> {
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
                        .font_request(&focus_item)
                        .pixel_size
                        .unwrap_or_default()
                        .get();
                    let pos =
                        euclid::point2(
                            pos_x as f32 / scale_factor,
                            pos_y as f32 / scale_factor - size / 2.,
                        ) - focus_item.map_to_window(focus_item.geometry().origin).to_vector();
                    let text_pos = text_input.as_pin_ref().byte_offset_for_position(
                        pos,
                        &adaptor,
                        &focus_item,
                    );

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
    .unwrap();
    Ok(())
}

fn callback_popup_menu_action<'local>(
    _env: &mut Env<'local>,
    _class: JClass<'local>,
    id: jint,
) -> Result<(), jni::errors::Error> {
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
    .unwrap();
    Ok(())
}

fn callback_set_insets<'local>(
    _env: &mut Env<'local>,
    _class: JClass<'local>,
    window_top: jint,
    window_left: jint,
    window_bottom: jint,
    window_right: jint,
    safe_area_top: jint,
    safe_area_left: jint,
    safe_area_bottom: jint,
    safe_area_right: jint,
    keyboard_top: jint,
    keyboard_left: jint,
    keyboard_bottom: jint,
    keyboard_right: jint,
) -> Result<(), jni::errors::Error> {
    i_slint_core::api::invoke_from_event_loop(move || {
        if let Some(w) = CURRENT_WINDOW.with_borrow(|x| x.upgrade()) {
            w.update_window_insets(
                PhysicalPosition::new(window_left as _, window_top as _),
                PhysicalSize::new(
                    (window_right - window_left) as _,
                    (window_bottom - window_top) as _,
                ),
                PhysicalEdges::new(
                    safe_area_top as _,
                    safe_area_bottom as _,
                    safe_area_left as _,
                    safe_area_right as _,
                ),
                PhysicalEdges::new(
                    keyboard_top as _,
                    keyboard_bottom as _,
                    keyboard_left as _,
                    keyboard_right as _,
                ),
            );
        }
    })
    .unwrap();
    Ok(())
}
