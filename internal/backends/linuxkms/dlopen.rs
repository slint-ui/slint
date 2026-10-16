// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore dlerror dlsym keysym readelf soname unref usec xkbcommon
//! Loads the C libraries behind the `input`, `xkbcommon`, and `gbm` crates at run-time.
//!
//! These crates import their functions with `#[link]`, which records each library as `DT_NEEDED`,
//! so the dynamic loader refuses to load the binary on a system without them.
//! This module defines every imported function, forwarding to the library opened with `dlopen`.
//! A function opens its library on first use, and aborts if that fails;
//! call `load()` first to get the failure as an error instead.
//! The crates' references bind to these definitions,
//! and since rustc links with `--as-needed`, the linker then leaves the libraries out.
//! Run `readelf -d` on the binary to check: a function missing here brings its library back.

macro_rules! library {
    (
        $name:ident, $soname:literal, $($sys:ident)::+,
        { $($(#[$attr:meta])* fn $function:ident($($arg:ident: $type:ty),* $(,)?) $(-> $ret:ty)?;)* }
    ) => {
        pub mod $name {
            use $($sys)::+ as sys;
            #[allow(unused_imports)]
            use std::ffi::{c_char, c_int, c_uint, c_void};
            use sys::*;

            struct Functions {
                $($(#[$attr])* $function: unsafe extern "C" fn($($type),*) $(-> $ret)?,)*
                _library: libloading::Library,
            }

            fn functions() -> Result<&'static Functions, &'static str> {
                static FUNCTIONS: std::sync::OnceLock<Result<Functions, String>> =
                    std::sync::OnceLock::new();
                FUNCTIONS.get_or_init(open).as_ref().map_err(String::as_str)
            }

            fn open() -> Result<Functions, String> {
                // Safety: the library has no initializers with preconditions.
                let library = unsafe { libloading::Library::new($soname) }.map_err(|e| {
                    format!(
                        "Error loading {}, which the LinuxKMS backend needs: {}",
                        $soname,
                        super::describe(&e)
                    )
                })?;
                Ok(Functions {
                    $($(#[$attr])* $function: unsafe { super::symbol(&library, $soname, stringify!($function))? },)*
                    _library: library,
                })
            }

            pub fn load() -> Result<(), i_slint_core::platform::PlatformError> {
                functions().map(|_| ()).map_err(Into::into)
            }

            $(
                $(#[$attr])*
                #[unsafe(no_mangle)]
                unsafe extern "C" fn $function($($arg: $type),*) $(-> $ret)? {
                    let functions = functions().unwrap_or_else(|e| panic!("{e}"));
                    unsafe { (functions.$function)($($arg),*) }
                }

                $(#[$attr])*
                const _: unsafe extern "C" fn($($type),*) $(-> $ret)? = sys::$function;
            )*
        }
    };
}

/// # Safety
///
/// `T` must be the type of the function `name`.
unsafe fn symbol<T: Copy>(
    library: &libloading::Library,
    soname: &str,
    name: &str,
) -> Result<T, String> {
    unsafe { library.get::<T>(name.as_bytes()) }
        .map(|symbol| *symbol)
        .map_err(|e| format!("Error looking up {name} in {soname}: {}", describe(&e)))
}

/// libloading keeps the `dlerror()` message in the error's source.
fn describe(error: &libloading::Error) -> String {
    match std::error::Error::source(error) {
        Some(source) => format!("{error}: {source}"),
        None => error.to_string(),
    }
}

#[cfg(feature = "libinput")]
library!(libudev, "libudev.so.1", libudev_sys, {
    fn udev_new() -> *mut udev;
    fn udev_unref(udev: *mut udev) -> *mut udev;
});

#[cfg(feature = "libinput")]
library!(libinput, "libinput.so.10", input::ffi, {
    fn libinput_dispatch(libinput: *mut libinput) -> c_int;
    fn libinput_event_destroy(event: *mut libinput_event);
    fn libinput_event_device_notify_get_base_event(
        event: *mut libinput_event_device_notify,
    ) -> *mut libinput_event;
    fn libinput_event_gesture_get_base_event(
        event: *mut libinput_event_gesture,
    ) -> *mut libinput_event;
    fn libinput_event_get_device_notify_event(
        event: *mut libinput_event,
    ) -> *mut libinput_event_device_notify;
    fn libinput_event_get_gesture_event(event: *mut libinput_event) -> *mut libinput_event_gesture;
    fn libinput_event_get_keyboard_event(
        event: *mut libinput_event,
    ) -> *mut libinput_event_keyboard;
    fn libinput_event_get_pointer_event(event: *mut libinput_event) -> *mut libinput_event_pointer;
    fn libinput_event_get_switch_event(event: *mut libinput_event) -> *mut libinput_event_switch;
    fn libinput_event_get_tablet_pad_event(
        event: *mut libinput_event,
    ) -> *mut libinput_event_tablet_pad;
    fn libinput_event_get_tablet_tool_event(
        event: *mut libinput_event,
    ) -> *mut libinput_event_tablet_tool;
    fn libinput_event_get_touch_event(event: *mut libinput_event) -> *mut libinput_event_touch;
    fn libinput_event_get_type(event: *mut libinput_event) -> libinput_event_type;
    fn libinput_event_keyboard_get_base_event(
        event: *mut libinput_event_keyboard,
    ) -> *mut libinput_event;
    fn libinput_event_keyboard_get_key(event: *mut libinput_event_keyboard) -> u32;
    fn libinput_event_keyboard_get_key_state(
        event: *mut libinput_event_keyboard,
    ) -> libinput_key_state;
    fn libinput_event_pointer_get_absolute_x_transformed(
        event: *mut libinput_event_pointer,
        width: u32,
    ) -> f64;
    fn libinput_event_pointer_get_absolute_y_transformed(
        event: *mut libinput_event_pointer,
        height: u32,
    ) -> f64;
    fn libinput_event_pointer_get_base_event(
        event: *mut libinput_event_pointer,
    ) -> *mut libinput_event;
    fn libinput_event_pointer_get_button(event: *mut libinput_event_pointer) -> u32;
    fn libinput_event_pointer_get_button_state(
        event: *mut libinput_event_pointer,
    ) -> libinput_button_state;
    fn libinput_event_pointer_get_dx(event: *mut libinput_event_pointer) -> f64;
    fn libinput_event_pointer_get_dy(event: *mut libinput_event_pointer) -> f64;
    fn libinput_event_pointer_get_scroll_value(
        event: *mut libinput_event_pointer,
        axis: libinput_pointer_axis,
    ) -> f64;
    fn libinput_event_pointer_get_scroll_value_v120(
        event: *mut libinput_event_pointer,
        axis: libinput_pointer_axis,
    ) -> f64;
    fn libinput_event_pointer_has_axis(
        event: *mut libinput_event_pointer,
        axis: libinput_pointer_axis,
    ) -> c_int;
    fn libinput_event_switch_get_base_event(
        event: *mut libinput_event_switch,
    ) -> *mut libinput_event;
    fn libinput_event_tablet_pad_get_base_event(
        event: *mut libinput_event_tablet_pad,
    ) -> *mut libinput_event;
    fn libinput_event_tablet_tool_get_base_event(
        event: *mut libinput_event_tablet_tool,
    ) -> *mut libinput_event;
    fn libinput_event_touch_get_base_event(event: *mut libinput_event_touch)
    -> *mut libinput_event;
    fn libinput_event_touch_get_slot(event: *mut libinput_event_touch) -> i32;
    fn libinput_event_touch_get_time_usec(event: *mut libinput_event_touch) -> u64;
    fn libinput_event_touch_get_x_transformed(event: *mut libinput_event_touch, width: u32) -> f64;
    fn libinput_event_touch_get_y_transformed(event: *mut libinput_event_touch, height: u32)
    -> f64;
    fn libinput_get_event(libinput: *mut libinput) -> *mut libinput_event;
    fn libinput_get_fd(libinput: *mut libinput) -> c_int;
    fn libinput_ref(libinput: *mut libinput) -> *mut libinput;
    fn libinput_udev_assign_seat(libinput: *mut libinput, seat_id: *const c_char) -> c_int;
    fn libinput_udev_create_context(
        interface: *const libinput_interface,
        user_data: *mut c_void,
        udev: *mut udev,
    ) -> *mut libinput;
    fn libinput_unref(libinput: *mut libinput) -> *mut libinput;
});

#[cfg(feature = "libinput")]
library!(xkbcommon, "libxkbcommon.so.0", xkbcommon::xkb::ffi, {
    fn xkb_context_new(flags: xkb_context_flags) -> *mut xkb_context;
    fn xkb_context_unref(context: *mut xkb_context);
    fn xkb_keymap_new_from_names(
        context: *mut xkb_context,
        names: *const xkb_rule_names,
        flags: xkb_keymap_compile_flags,
    ) -> *mut xkb_keymap;
    fn xkb_keymap_unref(keymap: *mut xkb_keymap);
    fn xkb_keysym_to_utf32(keysym: xkb_keysym_t) -> u32;
    fn xkb_state_key_get_one_sym(state: *mut xkb_state, key: xkb_keycode_t) -> xkb_keysym_t;
    fn xkb_state_mod_name_is_active(
        state: *mut xkb_state,
        name: *const c_char,
        type_: xkb_state_component,
    ) -> c_int;
    fn xkb_state_new(keymap: *mut xkb_keymap) -> *mut xkb_state;
    fn xkb_state_unref(state: *mut xkb_state);
    fn xkb_state_update_key(
        state: *mut xkb_state,
        key: xkb_keycode_t,
        direction: xkb_key_direction,
    ) -> xkb_state_component;
});

#[cfg(feature = "gbm")]
library!(gbm, "libgbm.so.1", gbm_sys, {
    fn gbm_bo_create(
        gbm: *mut gbm_device,
        width: u32,
        height: u32,
        format: u32,
        flags: u32,
    ) -> *mut gbm_bo;
    // Mesa 21.3 added it.
    #[cfg(gbm_dmabuf)]
    fn gbm_bo_create_with_modifiers2(
        gbm: *mut gbm_device,
        width: u32,
        height: u32,
        format: u32,
        modifiers: *const u64,
        count: c_uint,
        flags: u32,
    ) -> *mut gbm_bo;
    fn gbm_bo_destroy(bo: *mut gbm_bo);
    fn gbm_bo_get_fd(bo: *mut gbm_bo) -> c_int;
    fn gbm_bo_get_format(bo: *mut gbm_bo) -> u32;
    fn gbm_bo_get_handle(bo: *mut gbm_bo) -> gbm_bo_handle;
    fn gbm_bo_get_handle_for_plane(bo: *mut gbm_bo, plane: c_int) -> gbm_bo_handle;
    fn gbm_bo_get_height(bo: *mut gbm_bo) -> u32;
    fn gbm_bo_get_modifier(bo: *mut gbm_bo) -> u64;
    fn gbm_bo_get_offset(bo: *mut gbm_bo, plane: c_int) -> u32;
    fn gbm_bo_get_plane_count(bo: *mut gbm_bo) -> c_int;
    fn gbm_bo_get_stride(bo: *mut gbm_bo) -> u32;
    fn gbm_bo_get_stride_for_plane(bo: *mut gbm_bo, plane: c_int) -> u32;
    fn gbm_bo_get_user_data(bo: *mut gbm_bo) -> *mut c_void;
    fn gbm_bo_get_width(bo: *mut gbm_bo) -> u32;
    fn gbm_bo_set_user_data(
        bo: *mut gbm_bo,
        data: *mut c_void,
        destroy_user_data: Option<unsafe extern "C" fn(arg1: *mut gbm_bo, arg2: *mut c_void)>,
    );
    fn gbm_create_device(fd: c_int) -> *mut gbm_device;
    fn gbm_device_destroy(gbm: *mut gbm_device);
    fn gbm_surface_create(
        gbm: *mut gbm_device,
        width: u32,
        height: u32,
        format: u32,
        flags: u32,
    ) -> *mut gbm_surface;
    fn gbm_surface_destroy(surface: *mut gbm_surface);
    fn gbm_surface_lock_front_buffer(surface: *mut gbm_surface) -> *mut gbm_bo;
    fn gbm_surface_release_buffer(surface: *mut gbm_surface, bo: *mut gbm_bo);
});
