// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*! This crate just exposes the functions used by the Swift integration */

#![no_std]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "slint-interpreter")]
#[allow(private_interfaces)]
mod interpreter_swift {
    use alloc::boxed::Box;
    use alloc::string::{String, ToString};
    use alloc::vec::Vec;
    use core::ffi::c_char;
    use i_slint_core::SharedString;
    use slint_interpreter::{
        ComponentDefinition, ComponentHandle, ComponentInstance, DiagnosticLevel, Struct, Value,
    };

    // -----------------------------------------------------------------------
    // Value helpers
    // -----------------------------------------------------------------------

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_swift_value_new_void() -> *mut Value {
        Box::into_raw(Box::new(Value::default()))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_swift_value_new_double(d: f64) -> *mut Value {
        Box::into_raw(Box::new(Value::Number(d)))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_swift_value_new_bool(b: bool) -> *mut Value {
        Box::into_raw(Box::new(Value::Bool(b)))
    }

    /// # Safety
    /// `bytes` must be valid UTF-8 of length `len`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_value_new_string(
        bytes: *const c_char,
        len: usize,
    ) -> *mut Value {
        let s = unsafe {
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(bytes as *const u8, len))
        };
        Box::into_raw(Box::new(Value::String(s.into())))
    }

    /// # Safety
    /// `stru` must be a valid pointer to a `SlintInterpreterStructOpaque` (i.e. a `Box<Struct>`).
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_value_new_struct(stru: *const Struct) -> *mut Value {
        Box::into_raw(Box::new(Value::Struct(unsafe { (*stru).clone() })))
    }

    /// # Safety
    /// `val` must be a valid `*mut Value` returned by a `slint_swift_value_*` function.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_value_clone(val: *const Value) -> *mut Value {
        Box::into_raw(Box::new(unsafe { (*val).clone() }))
    }

    /// # Safety
    /// `val` must be a valid `*mut Value` returned by a `slint_swift_value_*` function.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_value_drop(val: *mut Value) {
        if !val.is_null() {
            unsafe { drop(Box::from_raw(val)) }
        }
    }

    /// # Safety
    /// `val` must be a valid non-null `*const Value`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_value_type(
        val: *const Value,
    ) -> slint_interpreter::ValueType {
        unsafe { (*val).value_type() }
    }

    /// # Safety
    /// `val` and `out` must be valid non-null pointers.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_value_to_double(val: *const Value, out: *mut f64) -> bool {
        if let Value::Number(n) = unsafe { &*val } {
            unsafe { *out = *n };
            true
        } else {
            false
        }
    }

    /// # Safety
    /// `val` and `out` must be valid non-null pointers.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_value_to_bool(val: *const Value, out: *mut bool) -> bool {
        if let Value::Bool(b) = unsafe { &*val } {
            unsafe { *out = *b };
            true
        } else {
            false
        }
    }

    /// Writes UTF-8 pointer + length of the string held in `val`.
    /// The pointer is valid as long as `val` lives.
    ///
    /// # Safety
    /// All pointers must be valid and non-null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_value_to_string(
        val: *const Value,
        out_ptr: *mut *const c_char,
        out_len: *mut usize,
    ) -> bool {
        if let Value::String(s) = unsafe { &*val } {
            unsafe {
                *out_ptr = s.as_str().as_ptr() as *const c_char;
                *out_len = s.as_str().len();
            }
            true
        } else {
            false
        }
    }

    /// Returns a heap-allocated clone of the Struct inside `val`, or NULL.
    /// The caller must free it with `slint_swift_struct_drop`.
    ///
    /// # Safety
    /// `val` must be a valid non-null `*const Value`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_value_to_struct(val: *const Value) -> *mut Struct {
        if let Value::Struct(s) = unsafe { &*val } {
            Box::into_raw(Box::new(s.clone()))
        } else {
            core::ptr::null_mut()
        }
    }

    // -----------------------------------------------------------------------
    // Struct helpers
    // -----------------------------------------------------------------------

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_swift_struct_new() -> *mut Struct {
        Box::into_raw(Box::new(Struct::default()))
    }

    /// # Safety
    /// `stru` must be a valid non-null `*const Struct`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_struct_clone(stru: *const Struct) -> *mut Struct {
        Box::into_raw(Box::new(unsafe { (*stru).clone() }))
    }

    /// # Safety
    /// `stru` must be a valid `*mut Struct` returned by `slint_swift_struct_new` or similar.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_struct_drop(stru: *mut Struct) {
        if !stru.is_null() {
            unsafe { drop(Box::from_raw(stru)) }
        }
    }

    /// Returns a heap-allocated clone of the named field, or NULL if absent.
    ///
    /// # Safety
    /// `stru`, `name` must be valid non-null pointers; `name` has byte length `name_len`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_struct_get_field(
        stru: *const Struct,
        name: *const c_char,
        name_len: usize,
    ) -> *mut Value {
        let name_str = unsafe {
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(name as *const u8, name_len))
        };
        match unsafe { (*stru).get_field(name_str) } {
            Some(v) => Box::into_raw(Box::new(v.clone())),
            None => core::ptr::null_mut(),
        }
    }

    /// # Safety
    /// All pointers must be valid and non-null; `name` has byte length `name_len`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_struct_set_field(
        stru: *mut Struct,
        name: *const c_char,
        name_len: usize,
        value: *const Value,
    ) {
        let name_str = unsafe {
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(name as *const u8, name_len))
        };
        unsafe { (*stru).set_field(name_str.into(), (*value).clone()) }
    }

    /// # Safety
    /// `stru` must be a valid non-null `*const Struct`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_struct_field_count(stru: *const Struct) -> usize {
        unsafe { (*stru).iter().count() }
    }

    /// Writes the UTF-8 name pointer + length of the field at `index`.
    /// The pointer is valid as long as `stru` lives.
    ///
    /// # Safety
    /// All pointers must be valid; `out_ptr`/`out_len` are written on success.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_struct_field_name_at(
        stru: *const Struct,
        index: usize,
        out_ptr: *mut *const c_char,
        out_len: *mut usize,
    ) -> bool {
        if let Some((name, _)) = unsafe { (*stru).iter().nth(index) } {
            unsafe {
                *out_ptr = name.as_ptr() as *const c_char;
                *out_len = name.len();
            }
            true
        } else {
            false
        }
    }

    // -----------------------------------------------------------------------
    // ComponentCompiler helpers
    // -----------------------------------------------------------------------

    struct SwiftCompiler {
        compiler: slint_interpreter::Compiler,
        diagnostics: Vec<slint_interpreter::Diagnostic>,
        /// Strings stored here to keep their backing memory alive for the diagnostic accessors.
        _string_storage: Vec<String>,
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_swift_compiler_new() -> *mut SwiftCompiler {
        Box::into_raw(Box::new(SwiftCompiler {
            compiler: slint_interpreter::Compiler::default(),
            diagnostics: Vec::new(),
            _string_storage: Vec::new(),
        }))
    }

    /// # Safety
    /// `compiler` must be a valid pointer returned by `slint_swift_compiler_new`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_compiler_drop(compiler: *mut SwiftCompiler) {
        if !compiler.is_null() {
            unsafe { drop(Box::from_raw(compiler)) }
        }
    }

    /// # Safety
    /// `compiler` and `style` must be valid non-null pointers; `style` has byte length `style_len`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_compiler_set_style(
        compiler: *mut SwiftCompiler,
        style: *const c_char,
        style_len: usize,
    ) {
        let s = unsafe {
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                style as *const u8,
                style_len,
            ))
        }
        .to_string();
        unsafe { (*compiler).compiler.set_style(s) };
    }

    /// Compiles source code. Returns a heap-allocated `ComponentDefinition` on success, NULL
    /// on failure (check diagnostics).  The caller must free with `slint_swift_definition_drop`.
    ///
    /// # Safety
    /// All pointer parameters must be valid; byte lengths match.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_compiler_build_from_source(
        compiler: *mut SwiftCompiler,
        source: *const c_char,
        source_len: usize,
        path: *const c_char,
        path_len: usize,
    ) -> *mut ComponentDefinition {
        let source_str = unsafe {
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                source as *const u8,
                source_len,
            ))
        }
        .to_string();
        let path_str = unsafe {
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(path as *const u8, path_len))
        };

        let result = spin_on::spin_on(unsafe {
            (*compiler).compiler.build_from_source(source_str, path_str.into())
        });
        unsafe { (*compiler).diagnostics = result.diagnostics().collect() };

        match result.component_names().next().and_then(|n| result.component(n)) {
            Some(d) => Box::into_raw(Box::new(d)),
            None => core::ptr::null_mut(),
        }
    }

    /// # Safety
    /// `compiler` must be a valid non-null pointer.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_compiler_diagnostics_count(
        compiler: *const SwiftCompiler,
    ) -> usize {
        unsafe { (*compiler).diagnostics.len() }
    }

    /// # Safety
    /// `compiler` must be a valid non-null pointer.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_compiler_has_errors(
        compiler: *const SwiftCompiler,
    ) -> bool {
        unsafe { (*compiler).diagnostics.iter().any(|d| d.level() == DiagnosticLevel::Error) }
    }

    /// # Safety
    /// All pointers must be valid; output pointers are written on success.
    /// The returned string pointers are valid as long as `compiler` lives and no new compilation
    /// is triggered.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_compiler_get_diagnostic(
        compiler: *const SwiftCompiler,
        index: usize,
        message_ptr: *mut *const c_char,
        message_len: *mut usize,
        file_ptr: *mut *const c_char,
        file_len: *mut usize,
        line: *mut usize,
        column: *mut usize,
        level: *mut u8,
    ) -> bool {
        let diags = unsafe { &(*compiler).diagnostics };
        match diags.get(index) {
            Some(d) => {
                let msg = d.message();
                let (l, c) = d.line_column();
                let file = d.source_file().and_then(|p| p.to_str()).unwrap_or("");
                unsafe {
                    *message_ptr = msg.as_ptr() as *const c_char;
                    *message_len = msg.len();
                    *file_ptr = file.as_ptr() as *const c_char;
                    *file_len = file.len();
                    *line = l;
                    *column = c;
                    *level = match d.level() {
                        DiagnosticLevel::Error => 0,
                        DiagnosticLevel::Warning => 1,
                        _ => 2,
                    };
                }
                true
            }
            None => false,
        }
    }

    // -----------------------------------------------------------------------
    // ComponentDefinition helpers
    // -----------------------------------------------------------------------

    /// # Safety
    /// `def` must be a valid non-null `*const ComponentDefinition`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_definition_clone(
        def: *const ComponentDefinition,
    ) -> *mut ComponentDefinition {
        Box::into_raw(Box::new(unsafe { (*def).clone() }))
    }

    /// # Safety
    /// `def` must be a valid `*mut ComponentDefinition` returned by a
    /// `slint_swift_compiler_build_from_source` or `slint_swift_definition_clone`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_definition_drop(def: *mut ComponentDefinition) {
        if !def.is_null() {
            unsafe { drop(Box::from_raw(def)) }
        }
    }

    /// Writes the component name as a `SharedString` into `name_out`.
    /// The caller must call `slint_shared_string_drop` on `name_out`.
    ///
    /// # Safety
    /// `def` and `name_out` must be valid non-null pointers.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_definition_name(
        def: *const ComponentDefinition,
        name_out: *mut SharedString,
    ) {
        let name: SharedString = unsafe { (*def).name().into() };
        unsafe { core::ptr::write(name_out, name) };
    }

    /// # Safety
    /// `def` must be a valid non-null pointer.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_definition_properties_count(
        def: *const ComponentDefinition,
    ) -> usize {
        unsafe { (*def).properties().count() }
    }

    /// Writes the name (as `SharedString`) and type of property at `index` into `name_out` and
    /// `type_out`. Returns `SLINT_VALUE_TYPE_OTHER` if `index` is out of range; in that case
    /// `name_out` is not written.  The caller must call `slint_shared_string_drop` on `name_out`
    /// on a successful return.
    ///
    /// # Safety
    /// `def`, `name_out`, and `type_out` must be valid non-null pointers.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_definition_property_at(
        def: *const ComponentDefinition,
        index: usize,
        name_out: *mut SharedString,
        type_out: *mut slint_interpreter::ValueType,
    ) -> bool {
        if let Some((name, vt)) = unsafe { (*def).properties().nth(index) } {
            unsafe {
                core::ptr::write(name_out, name.into());
                *type_out = vt;
            }
            true
        } else {
            false
        }
    }

    /// # Safety
    /// `def` must be a valid non-null pointer.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_definition_callbacks_count(
        def: *const ComponentDefinition,
    ) -> usize {
        unsafe { (*def).callbacks().count() }
    }

    /// Writes the callback name at `index` as a `SharedString` into `name_out`.
    /// Returns true if `index` is valid; the caller must call `slint_shared_string_drop`.
    ///
    /// # Safety
    /// `def` and `name_out` must be valid non-null pointers.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_definition_callback_at(
        def: *const ComponentDefinition,
        index: usize,
        name_out: *mut SharedString,
    ) -> bool {
        if let Some(name) = unsafe { (*def).callbacks().nth(index) } {
            unsafe { core::ptr::write(name_out, name.into()) };
            true
        } else {
            false
        }
    }

    /// Creates a heap-allocated `ComponentInstance`. Returns NULL on failure.
    /// The caller must call `slint_swift_instance_drop`.
    ///
    /// # Safety
    /// `def` must be a valid non-null `*const ComponentDefinition`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_definition_create_instance(
        def: *const ComponentDefinition,
    ) -> *mut ComponentInstance {
        match unsafe { (*def).create() } {
            Ok(inst) => Box::into_raw(Box::new(inst)),
            Err(_) => core::ptr::null_mut(),
        }
    }

    // -----------------------------------------------------------------------
    // ComponentInstance helpers
    // -----------------------------------------------------------------------

    /// # Safety
    /// `inst` must be a valid `*mut ComponentInstance` from `slint_swift_definition_create_instance`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_instance_drop(inst: *mut ComponentInstance) {
        if !inst.is_null() {
            unsafe { drop(Box::from_raw(inst)) }
        }
    }

    /// # Safety
    /// `inst` must be a valid non-null pointer.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_instance_show(
        inst: *const ComponentInstance,
        visible: bool,
    ) {
        if visible {
            unsafe { (*inst).show().ok() };
        } else {
            unsafe { (*inst).hide().ok() };
        }
    }

    /// Returns a heap-allocated `Value` for the named property, or NULL on failure.
    /// The caller must call `slint_swift_value_drop`.
    ///
    /// # Safety
    /// `inst`, `name` must be valid non-null pointers; `name` has byte length `name_len`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_instance_get_property(
        inst: *const ComponentInstance,
        name: *const c_char,
        name_len: usize,
    ) -> *mut Value {
        let name_str = unsafe {
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(name as *const u8, name_len))
        };
        match unsafe { (*inst).get_property(name_str) } {
            Ok(val) => Box::into_raw(Box::new(val)),
            Err(_) => core::ptr::null_mut(),
        }
    }

    /// # Safety
    /// All pointers must be valid; `name` has byte length `name_len`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_instance_set_property(
        inst: *const ComponentInstance,
        name: *const c_char,
        name_len: usize,
        value: *const Value,
    ) -> bool {
        let name_str = unsafe {
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(name as *const u8, name_len))
        };
        unsafe { (*inst).set_property(name_str, (*value).clone()).is_ok() }
    }

    /// Invokes a callback or function by name.
    /// `args` is a C array of `*const Value` of length `args_count`.
    /// Returns a heap-allocated `Value` on success, NULL on failure.
    ///
    /// # Safety
    /// All pointers must be valid; `name` has byte length `name_len`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swift_instance_invoke(
        inst: *const ComponentInstance,
        name: *const c_char,
        name_len: usize,
        args: *const *const Value,
        args_count: usize,
    ) -> *mut Value {
        let name_str = unsafe {
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(name as *const u8, name_len))
        };
        let arg_ptrs = unsafe { core::slice::from_raw_parts(args, args_count) };
        let owned: Vec<Value> = arg_ptrs.iter().map(|&p| unsafe { (*p).clone() }).collect();
        match unsafe { (*inst).invoke(name_str, &owned) } {
            Ok(val) => Box::into_raw(Box::new(val)),
            Err(_) => core::ptr::null_mut(),
        }
    }
}

use alloc::rc::Rc;
use core::ffi::c_void;
use i_slint_core::SharedString;
use i_slint_core::window::{WindowAdapter, ffi::WindowAdapterRcOpaque};

#[cfg(feature = "i-slint-backend-selector")]
use i_slint_backend_selector::with_platform;

#[cfg(not(feature = "i-slint-backend-selector"))]
pub fn with_platform<R>(
    f: impl FnOnce(
        &dyn i_slint_core::platform::Platform,
    ) -> Result<R, i_slint_core::platform::PlatformError>,
) -> Result<R, i_slint_core::platform::PlatformError> {
    i_slint_core::with_platform(|| Err(i_slint_core::platform::PlatformError::NoPlatform), f)
}

use alloc::boxed::Box;
use i_slint_core::graphics::Image;

// ---------------------------------------------------------------------------
// Testing backend
// ---------------------------------------------------------------------------

/// Initializes the headless testing backend. Call this before creating any
/// component instances in environments without a display server (e.g. CI).
///
/// This function is only available when the `backend-testing` feature is enabled.
#[cfg(feature = "i-slint-backend-testing")]
#[unsafe(no_mangle)]
pub extern "C" fn slint_swift_testing_init() {
    i_slint_backend_testing::init_no_event_loop();
}

// ---------------------------------------------------------------------------
// Phase 4: Platform Integration — Event dispatch helpers
// ---------------------------------------------------------------------------

/// Dispatches a pointer-pressed event to the window.
///
/// # Safety
///
/// `handle` must be a valid `WindowAdapterRcOpaque` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_dispatch_pointer_pressed(
    handle: *const WindowAdapterRcOpaque,
    x: f32,
    y: f32,
    button: u32,
) {
    let event = i_slint_core::platform::WindowEvent::PointerPressed {
        position: i_slint_core::api::LogicalPosition::new(x, y),
        button: pointer_event_button_from_u32(button),
    };
    unsafe {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event);
    }
}

/// Dispatches a pointer-released event to the window.
///
/// # Safety
///
/// `handle` must be a valid `WindowAdapterRcOpaque` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_dispatch_pointer_released(
    handle: *const WindowAdapterRcOpaque,
    x: f32,
    y: f32,
    button: u32,
) {
    let event = i_slint_core::platform::WindowEvent::PointerReleased {
        position: i_slint_core::api::LogicalPosition::new(x, y),
        button: pointer_event_button_from_u32(button),
    };
    unsafe {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event);
    }
}

/// Dispatches a pointer-moved event to the window.
///
/// # Safety
///
/// `handle` must be a valid `WindowAdapterRcOpaque` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_dispatch_pointer_moved(
    handle: *const WindowAdapterRcOpaque,
    x: f32,
    y: f32,
) {
    let event = i_slint_core::platform::WindowEvent::PointerMoved {
        position: i_slint_core::api::LogicalPosition::new(x, y),
    };
    unsafe {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event);
    }
}

/// Dispatches a pointer-scrolled event to the window.
///
/// # Safety
///
/// `handle` must be a valid `WindowAdapterRcOpaque` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_dispatch_pointer_scrolled(
    handle: *const WindowAdapterRcOpaque,
    x: f32,
    y: f32,
    delta_x: f32,
    delta_y: f32,
) {
    let event = i_slint_core::platform::WindowEvent::PointerScrolled {
        position: i_slint_core::api::LogicalPosition::new(x, y),
        delta_x,
        delta_y,
    };
    unsafe {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event);
    }
}

/// Dispatches a pointer-exited event to the window.
///
/// # Safety
///
/// `handle` must be a valid `WindowAdapterRcOpaque` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_dispatch_pointer_exited(handle: *const WindowAdapterRcOpaque) {
    let event = i_slint_core::platform::WindowEvent::PointerExited;
    unsafe {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event);
    }
}

/// Dispatches a key-pressed event to the window.
///
/// # Safety
///
/// `handle` must be a valid `WindowAdapterRcOpaque` pointer.
/// `text` must be a valid `SharedString` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_dispatch_key_pressed(
    handle: *const WindowAdapterRcOpaque,
    text: &SharedString,
) {
    let event = i_slint_core::platform::WindowEvent::KeyPressed { text: text.clone() };
    unsafe {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event);
    }
}

/// Dispatches a key-press-repeated event to the window.
///
/// # Safety
///
/// `handle` must be a valid `WindowAdapterRcOpaque` pointer.
/// `text` must be a valid `SharedString` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_dispatch_key_press_repeated(
    handle: *const WindowAdapterRcOpaque,
    text: &SharedString,
) {
    let event = i_slint_core::platform::WindowEvent::KeyPressRepeated { text: text.clone() };
    unsafe {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event);
    }
}

/// Dispatches a key-released event to the window.
///
/// # Safety
///
/// `handle` must be a valid `WindowAdapterRcOpaque` pointer.
/// `text` must be a valid `SharedString` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_dispatch_key_released(
    handle: *const WindowAdapterRcOpaque,
    text: &SharedString,
) {
    let event = i_slint_core::platform::WindowEvent::KeyReleased { text: text.clone() };
    unsafe {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event);
    }
}

/// Dispatches a scale-factor-changed event to the window.
///
/// # Safety
///
/// `handle` must be a valid `WindowAdapterRcOpaque` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_dispatch_scale_factor_changed(
    handle: *const WindowAdapterRcOpaque,
    scale_factor: f32,
) {
    let event = i_slint_core::platform::WindowEvent::ScaleFactorChanged { scale_factor };
    unsafe {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event);
    }
}

/// Dispatches a resized event to the window.
///
/// # Safety
///
/// `handle` must be a valid `WindowAdapterRcOpaque` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_dispatch_resized(
    handle: *const WindowAdapterRcOpaque,
    width: f32,
    height: f32,
) {
    let event = i_slint_core::platform::WindowEvent::Resized {
        size: i_slint_core::api::LogicalSize::new(width, height),
    };
    unsafe {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event);
    }
}

/// Dispatches a close-requested event to the window.
///
/// # Safety
///
/// `handle` must be a valid `WindowAdapterRcOpaque` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_dispatch_close_requested(
    handle: *const WindowAdapterRcOpaque,
) {
    let event = i_slint_core::platform::WindowEvent::CloseRequested;
    unsafe {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event);
    }
}

/// Dispatches a window-active-changed event to the window.
///
/// # Safety
///
/// `handle` must be a valid `WindowAdapterRcOpaque` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_dispatch_window_active_changed(
    handle: *const WindowAdapterRcOpaque,
    active: bool,
) {
    let event = i_slint_core::platform::WindowEvent::WindowActiveChanged(active);
    unsafe {
        let window_adapter = &*(handle as *const Rc<dyn WindowAdapter>);
        window_adapter.window().dispatch_event(event);
    }
}

fn pointer_event_button_from_u32(button: u32) -> i_slint_core::items::PointerEventButton {
    use i_slint_core::items::PointerEventButton;
    match button {
        0 => PointerEventButton::Other,
        1 => PointerEventButton::Left,
        2 => PointerEventButton::Right,
        3 => PointerEventButton::Middle,
        4 => PointerEventButton::Back,
        5 => PointerEventButton::Forward,
        _ => PointerEventButton::Other,
    }
}

// ---------------------------------------------------------------------------
// Phase 4: Platform Integration — Custom WindowAdapter + Platform registration
// ---------------------------------------------------------------------------

/// A custom window adapter with an attached renderer reference.
struct SwiftWindowAdapterWithRenderer {
    window: i_slint_core::api::Window,
    user_data: *mut c_void,
    drop_fn: unsafe extern "C" fn(*mut c_void),
    set_visible_fn: unsafe extern "C" fn(*mut c_void, bool),
    request_redraw_fn: unsafe extern "C" fn(*mut c_void),
    size_fn: unsafe extern "C" fn(*mut c_void, *mut u32, *mut u32),
    set_size_fn: unsafe extern "C" fn(*mut c_void, u32, u32),
    position_fn: unsafe extern "C" fn(*mut c_void, *mut i32, *mut i32) -> bool,
    set_position_fn: unsafe extern "C" fn(*mut c_void, i32, i32),
    update_window_properties_fn:
        Option<unsafe extern "C" fn(*mut c_void, *const SharedString, bool, bool, bool)>,
    /// Safety: must outlive this struct and be a valid `&dyn Renderer` transmuted to two pointers
    renderer: RendererRefOpaque,
}

/// Opaque representation of `&dyn Renderer` as two pointers (data + vtable).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RendererRefOpaque {
    _0: *const c_void,
    _1: *const c_void,
}

impl Drop for SwiftWindowAdapterWithRenderer {
    fn drop(&mut self) {
        unsafe { (self.drop_fn)(self.user_data) }
    }
}

impl WindowAdapter for SwiftWindowAdapterWithRenderer {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn set_visible(&self, visible: bool) -> Result<(), i_slint_core::platform::PlatformError> {
        unsafe { (self.set_visible_fn)(self.user_data, visible) };
        Ok(())
    }

    fn position(&self) -> Option<i_slint_core::api::PhysicalPosition> {
        let mut x: i32 = 0;
        let mut y: i32 = 0;
        if unsafe { (self.position_fn)(self.user_data, &mut x, &mut y) } {
            Some(i_slint_core::api::PhysicalPosition::new(x, y))
        } else {
            None
        }
    }

    fn set_position(&self, position: i_slint_core::api::WindowPosition) {
        let phys = position.to_physical(self.window.scale_factor());
        unsafe { (self.set_position_fn)(self.user_data, phys.x, phys.y) }
    }

    fn set_size(&self, size: i_slint_core::api::WindowSize) {
        let phys = size.to_physical(self.window.scale_factor());
        unsafe { (self.set_size_fn)(self.user_data, phys.width, phys.height) }
    }

    fn size(&self) -> i_slint_core::api::PhysicalSize {
        let mut w: u32 = 0;
        let mut h: u32 = 0;
        unsafe { (self.size_fn)(self.user_data, &mut w, &mut h) };
        i_slint_core::api::PhysicalSize::new(w, h)
    }

    fn renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        // Safety: the caller ensures the renderer outlives this adapter
        unsafe { core::mem::transmute(self.renderer) }
    }

    fn request_redraw(&self) {
        unsafe { (self.request_redraw_fn)(self.user_data) }
    }

    fn update_window_properties(&self, properties: i_slint_core::window::WindowProperties<'_>) {
        if let Some(f) = self.update_window_properties_fn {
            let title = properties.title();
            let fullscreen = properties.is_fullscreen();
            let minimized = properties.is_minimized();
            let maximized = properties.is_maximized();
            unsafe { f(self.user_data, &title, fullscreen, minimized, maximized) }
        }
    }
}

/// Creates a custom window adapter backed by Swift function pointers with an attached renderer.
///
/// `renderer` must be a valid `RendererRefOpaque` (e.g. from `slint_software_renderer_handle` or
/// `slint_skia_renderer_handle`) that outlives the window adapter.
///
/// Writes the resulting `Rc<dyn WindowAdapter>` into `target`.
///
/// # Safety
///
/// All function pointers must be valid. `target` must point to writable, properly-aligned memory
/// for a `WindowAdapterRcOpaque`. `user_data` must remain valid until `drop_fn` is called.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_window_adapter_new(
    user_data: *mut c_void,
    drop_fn: unsafe extern "C" fn(*mut c_void),
    set_visible_fn: unsafe extern "C" fn(*mut c_void, bool),
    request_redraw_fn: unsafe extern "C" fn(*mut c_void),
    size_fn: unsafe extern "C" fn(*mut c_void, *mut u32, *mut u32),
    set_size_fn: unsafe extern "C" fn(*mut c_void, u32, u32),
    position_fn: unsafe extern "C" fn(*mut c_void, *mut i32, *mut i32) -> bool,
    set_position_fn: unsafe extern "C" fn(*mut c_void, i32, i32),
    update_window_properties_fn: Option<
        unsafe extern "C" fn(*mut c_void, *const SharedString, bool, bool, bool),
    >,
    renderer: RendererRefOpaque,
    target: *mut WindowAdapterRcOpaque,
) {
    let window =
        Rc::<SwiftWindowAdapterWithRenderer>::new_cyclic(|w| SwiftWindowAdapterWithRenderer {
            window: i_slint_core::api::Window::new(w.clone()),
            user_data,
            drop_fn,
            set_visible_fn,
            request_redraw_fn,
            size_fn,
            set_size_fn,
            position_fn,
            set_position_fn,
            update_window_properties_fn,
            renderer,
        });
    unsafe {
        core::ptr::write(target as *mut Rc<dyn WindowAdapter>, window);
    }
}

// ---------------------------------------------------------------------------
// Phase 4: Custom Platform registration
// ---------------------------------------------------------------------------

/// Opaque task type matching the C++ PlatformTaskOpaque.
#[repr(C)]
pub struct SwiftPlatformTaskOpaque(*const c_void, *const c_void);

struct SwiftPlatform {
    user_data: *mut c_void,
    drop_fn: unsafe extern "C" fn(*mut c_void),
    window_factory_fn: unsafe extern "C" fn(*mut c_void, *mut WindowAdapterRcOpaque),
    run_event_loop_fn: unsafe extern "C" fn(*mut c_void),
    quit_event_loop_fn: unsafe extern "C" fn(*mut c_void),
    invoke_from_event_loop_fn: unsafe extern "C" fn(*mut c_void, SwiftPlatformTaskOpaque),
}

impl Drop for SwiftPlatform {
    fn drop(&mut self) {
        unsafe { (self.drop_fn)(self.user_data) }
    }
}

impl i_slint_core::platform::Platform for SwiftPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn WindowAdapter>, i_slint_core::platform::PlatformError> {
        let mut uninit = core::mem::MaybeUninit::<Rc<dyn WindowAdapter>>::uninit();
        unsafe {
            (self.window_factory_fn)(
                self.user_data,
                uninit.as_mut_ptr() as *mut WindowAdapterRcOpaque,
            );
            Ok(uninit.assume_init())
        }
    }

    fn run_event_loop(&self) -> Result<(), i_slint_core::platform::PlatformError> {
        unsafe { (self.run_event_loop_fn)(self.user_data) };
        Ok(())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn i_slint_core::platform::EventLoopProxy>> {
        Some(Box::new(SwiftEventLoopProxy {
            user_data: self.user_data,
            quit_event_loop_fn: self.quit_event_loop_fn,
            invoke_from_event_loop_fn: self.invoke_from_event_loop_fn,
        }))
    }
}

struct SwiftEventLoopProxy {
    user_data: *mut c_void,
    quit_event_loop_fn: unsafe extern "C" fn(*mut c_void),
    invoke_from_event_loop_fn: unsafe extern "C" fn(*mut c_void, SwiftPlatformTaskOpaque),
}

impl i_slint_core::platform::EventLoopProxy for SwiftEventLoopProxy {
    fn quit_event_loop(&self) -> Result<(), i_slint_core::api::EventLoopError> {
        unsafe { (self.quit_event_loop_fn)(self.user_data) };
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), i_slint_core::api::EventLoopError> {
        unsafe {
            (self.invoke_from_event_loop_fn)(
                self.user_data,
                core::mem::transmute::<*mut dyn FnOnce(), SwiftPlatformTaskOpaque>(Box::into_raw(
                    event,
                )),
            )
        };
        Ok(())
    }
}

unsafe impl Send for SwiftEventLoopProxy {}
unsafe impl Sync for SwiftEventLoopProxy {}

/// Registers a custom platform with the Slint runtime.
///
/// This must be called before any window is created. Once set, it cannot be changed.
///
/// # Safety
///
/// All function pointers must be valid. `user_data` must remain valid until `drop_fn` is called.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_platform_register(
    user_data: *mut c_void,
    drop_fn: unsafe extern "C" fn(*mut c_void),
    window_factory_fn: unsafe extern "C" fn(*mut c_void, *mut WindowAdapterRcOpaque),
    run_event_loop_fn: unsafe extern "C" fn(*mut c_void),
    quit_event_loop_fn: unsafe extern "C" fn(*mut c_void),
    invoke_from_event_loop_fn: unsafe extern "C" fn(*mut c_void, SwiftPlatformTaskOpaque),
) {
    let p = SwiftPlatform {
        user_data,
        drop_fn,
        window_factory_fn,
        run_event_loop_fn,
        quit_event_loop_fn,
        invoke_from_event_loop_fn,
    };
    i_slint_core::platform::set_platform(Box::new(p)).unwrap();
}

/// Runs a platform task (used by `invoke_from_event_loop`).
///
/// # Safety
///
/// `task` must be a valid `SwiftPlatformTaskOpaque` received from the `invoke_from_event_loop`
/// callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_platform_task_run(task: SwiftPlatformTaskOpaque) {
    unsafe {
        let f =
            Box::from_raw(core::mem::transmute::<SwiftPlatformTaskOpaque, *mut dyn FnOnce()>(task));
        f();
    }
}

/// Drops a platform task without running it.
///
/// # Safety
///
/// `task` must be a valid `SwiftPlatformTaskOpaque` received from the `invoke_from_event_loop`
/// callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_platform_task_drop(task: SwiftPlatformTaskOpaque) {
    unsafe {
        drop(Box::from_raw(core::mem::transmute::<SwiftPlatformTaskOpaque, *mut dyn FnOnce()>(
            task,
        )));
    }
}

/// Updates all timers and animations. Call this from your platform's event loop.
#[unsafe(no_mangle)]
pub extern "C" fn slint_swift_platform_update_timers_and_animations() {
    i_slint_core::platform::update_timers_and_animations()
}

/// Returns the duration in milliseconds until the next timer fires,
/// or `u64::MAX` if no timer is pending.
#[unsafe(no_mangle)]
pub extern "C" fn slint_swift_platform_duration_until_next_timer_update() -> u64 {
    i_slint_core::platform::duration_until_next_timer_update()
        .map_or(u64::MAX, |d| d.as_millis() as u64)
}

/// Returns whether the window has active animations.
///
/// # Safety
///
/// `handle` must be a valid `WindowAdapterRcOpaque` pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_window_has_active_animations(
    handle: *const WindowAdapterRcOpaque,
) -> bool {
    let window_adapter = unsafe { &*(handle as *const Rc<dyn WindowAdapter>) };
    window_adapter.window().has_active_animations()
}

/// Allocates a new default (empty) Image on the heap and returns a pointer.
/// The caller must eventually call `slint_swift_image_drop` to free it.
#[unsafe(no_mangle)]
pub extern "C" fn slint_swift_image_new() -> *mut Image {
    Box::into_raw(Box::new(Image::default()))
}

/// Drops a heap-allocated Image previously created by `slint_swift_image_new`
/// or `slint_swift_image_clone`.
///
/// # Safety
///
/// `image` must be a pointer returned by `slint_swift_image_new` or
/// `slint_swift_image_clone`, and must not be used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_image_drop(image: *mut Image) {
    if !image.is_null() {
        unsafe {
            drop(Box::from_raw(image));
        }
    }
}

/// Clones a heap-allocated Image. Returns a new heap-allocated Image.
///
/// # Safety
///
/// `image` must be either null or a valid pointer to an `Image` previously
/// returned by `slint_swift_image_new` or `slint_swift_image_clone`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_image_clone(image: *const Image) -> *mut Image {
    if image.is_null() {
        return slint_swift_image_new();
    }
    unsafe { Box::into_raw(Box::new((*image).clone())) }
}

/// Loads an image from a file path into a heap-allocated Image.
/// Returns a pointer to the new Image.
///
/// # Safety
///
/// `path` must be a valid reference to a `SharedString` for the duration of
/// this call. The returned pointer must eventually be freed with
/// `slint_swift_image_drop`.
#[cfg(feature = "std")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_swift_image_load_from_path(path: &SharedString) -> *mut Image {
    let img = Image::load_from_path(std::path::Path::new(path.as_str())).unwrap_or_default();
    Box::into_raw(Box::new(img))
}

/// Initializes a `WindowAdapterRcOpaque` at `out` by writing a newly created
/// window adapter into it.
///
/// # Safety
///
/// `out` must point to a valid, properly aligned, writable location for a
/// `WindowAdapterRcOpaque`. The value at `out` must not be initialized prior
/// to this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_windowrc_init(out: *mut WindowAdapterRcOpaque) {
    assert_eq!(
        core::mem::size_of::<Rc<dyn WindowAdapter>>(),
        core::mem::size_of::<WindowAdapterRcOpaque>()
    );
    let win = with_platform(|b| b.create_window_adapter()).unwrap();
    unsafe {
        core::ptr::write(out as *mut Rc<dyn WindowAdapter>, win);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_ensure_backend() {
    with_platform(|_b| {
        // Nothing to do, just make sure a backend was created
        Ok(())
    })
    .unwrap()
}

#[unsafe(no_mangle)]
/// Enters the main event loop.
pub extern "C" fn slint_run_event_loop(quit_on_last_window_closed: bool) {
    with_platform(|b| {
        if !quit_on_last_window_closed {
            #[allow(deprecated)]
            b.set_event_loop_quit_on_last_window_closed(false);
        }
        b.run_event_loop()
    })
    .unwrap();
}

/// Schedules `event` to be called with `user_data` on the main event loop thread.
/// When `user_data` is no longer needed, `drop_user_data` is called to free it.
///
/// # Safety
///
/// `event` must be a valid function pointer. `user_data` must remain valid
/// until `event` is invoked. If provided, `drop_user_data` must be safe to
/// call with `user_data` exactly once after `event` has run.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_post_event(
    event: extern "C" fn(user_data: *mut c_void),
    user_data: *mut c_void,
    drop_user_data: Option<extern "C" fn(*mut c_void)>,
) {
    struct UserData {
        user_data: *mut c_void,
        drop_user_data: Option<extern "C" fn(*mut c_void)>,
    }
    impl Drop for UserData {
        fn drop(&mut self) {
            if let Some(x) = self.drop_user_data {
                x(self.user_data)
            }
        }
    }
    unsafe impl Send for UserData {}
    let ud = UserData { user_data, drop_user_data };

    i_slint_core::api::invoke_from_event_loop(move || {
        let ud = &ud;
        event(ud.user_data);
    })
    .unwrap();
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_quit_event_loop() {
    i_slint_core::api::quit_event_loop().unwrap();
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_string_to_float(string: &SharedString, value: &mut f32) -> bool {
    match string.as_str().parse::<f32>() {
        Ok(v) => {
            *value = v;
            true
        }
        Err(_) => false,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_string_character_count(string: &SharedString) -> usize {
    unicode_segmentation::UnicodeSegmentation::graphemes(string.as_str(), true).count()
}

#[cfg(not(feature = "std"))]
mod allocator {
    use core::alloc::Layout;
    use core::ffi::c_void;

    struct CAlloc;
    unsafe impl core::alloc::GlobalAlloc for CAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            unsafe extern "C" {
                pub fn malloc(size: usize) -> *mut c_void;
            }
            unsafe {
                let align = layout.align();
                if align <= core::mem::size_of::<usize>() {
                    malloc(layout.size()) as *mut u8
                } else {
                    let ptr = malloc(layout.size() + align) as *mut u8;
                    let shift = align - (ptr as usize % align);
                    let ptr = ptr.add(shift);
                    core::ptr::write(ptr.sub(1), shift as u8);
                    ptr
                }
            }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            let align = layout.align();
            unsafe extern "C" {
                pub fn free(p: *mut c_void);
            }
            unsafe {
                if align <= core::mem::size_of::<usize>() {
                    free(ptr as *mut c_void);
                } else {
                    let shift = core::ptr::read(ptr.sub(1)) as usize;
                    free(ptr.sub(shift) as *mut c_void);
                }
            }
        }
    }

    #[global_allocator]
    static ALLOCATOR: CAlloc = CAlloc;
}

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
