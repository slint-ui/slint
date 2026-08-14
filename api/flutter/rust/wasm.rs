// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The web build's entry points.
//!
//! A browser cannot call the C ABI the other platforms use, so this wraps it
//! for `wasm-bindgen`: JavaScript passes strings and numbers, handles cross as
//! their wasm addresses, and every fallible call still returns the JSON
//! envelope the Dart side already knows how to decode. The wrappers hold no
//! logic of their own — they marshal and delegate.
//!
//! Only the runtime is exposed. Code generation runs on the host through the
//! `slint_generator` package, never in a browser, so `slint_dart_generate` has
//! no wrapper here.
//!
//! Rendering goes through [`crate::embedded`], the same software renderer the
//! Flutter desktop build uses: there is no window and no event loop, the caller
//! asks for frames and feeds input back.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_void};

use wasm_bindgen::prelude::*;

/// Route Rust panics to `console.error`. Without this the browser only sees an
/// `unreachable` trap, because wasm builds abort instead of unwinding — which
/// also means the `catch_unwind` guards elsewhere in this crate cannot help
/// here.
#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
}

// ---------------------------------------------------------------------------
// Marshalling
// ---------------------------------------------------------------------------

/// Take ownership of a string the C layer allocated, and free the original.
fn take(pointer: *mut c_char) -> String {
    if pointer.is_null() {
        return String::new();
    }
    let value = unsafe { CStr::from_ptr(pointer) }.to_string_lossy().into_owned();
    unsafe { crate::slint_dart_free_string(pointer) };
    value
}

/// A NUL-terminated copy of `value`, to borrow for the length of one call.
fn c_string(value: &str) -> CString {
    CString::new(value).unwrap_or_default()
}

/// An optional string argument: JavaScript's `null`/`undefined` becomes the
/// null pointer the C side reads as "not set".
fn opt_c_string(value: Option<String>) -> Option<CString> {
    value.map(|value| c_string(&value))
}

fn as_ptr(value: &Option<CString>) -> *const c_char {
    value.as_ref().map_or(std::ptr::null(), |value| value.as_ptr())
}

/// Reborrow a handle JavaScript is holding. Handles are wasm addresses, so
/// they are only meaningful inside this module instance.
unsafe fn handle<'a, T>(handle: u32) -> &'a T {
    unsafe { &*(handle as usize as *const T) }
}

unsafe fn handle_mut<'a, T>(handle: u32) -> &'a mut T {
    unsafe { &mut *(handle as usize as *mut T) }
}

fn envelope_err(message: &str) -> String {
    serde_json::json!({ "err": message }).to_string()
}

// ---------------------------------------------------------------------------
// Compiler
// ---------------------------------------------------------------------------

#[wasm_bindgen]
pub fn compiler_new() -> u32 {
    crate::slint_dart_compiler_new() as usize as u32
}

#[wasm_bindgen]
pub fn compiler_free(compiler: u32) {
    unsafe { crate::slint_dart_compiler_free(compiler as usize as *mut _) };
}

#[wasm_bindgen]
pub fn compiler_set_style(compiler: u32, style: &str) {
    let style = c_string(style);
    unsafe { crate::slint_dart_compiler_set_style(handle_mut(compiler), style.as_ptr()) };
}

#[wasm_bindgen]
pub fn compiler_add_include_path(compiler: u32, path: &str) {
    let path = c_string(path);
    unsafe { crate::slint_dart_compiler_add_include_path(handle_mut(compiler), path.as_ptr()) };
}

/// Compile `.slint` source. `path` only names the source in diagnostics and
/// anchors relative imports — the browser has no filesystem, so there is no
/// `build_from_path` counterpart.
#[wasm_bindgen]
pub fn build_from_source(compiler: u32, source: &str, path: &str) -> u32 {
    let source = c_string(source);
    let path = c_string(path);
    unsafe {
        crate::slint_dart_compiler_build_from_source(
            handle(compiler),
            source.as_ptr(),
            path.as_ptr(),
        ) as usize as u32
    }
}

#[wasm_bindgen]
pub fn result_free(result: u32) {
    unsafe { crate::slint_dart_result_free(result as usize as *mut _) };
}

#[wasm_bindgen]
pub fn result_has_errors(result: u32) -> bool {
    crate::slint_dart_result_has_errors(unsafe { handle(result) })
}

#[wasm_bindgen]
pub fn result_diagnostics(result: u32) -> String {
    take(crate::slint_dart_result_diagnostics(unsafe { handle(result) }))
}

#[wasm_bindgen]
pub fn result_component_names(result: u32) -> String {
    take(crate::slint_dart_result_component_names(unsafe { handle(result) }))
}

#[wasm_bindgen]
pub fn result_component(result: u32, name: Option<String>) -> u32 {
    let name = opt_c_string(name);
    unsafe { crate::slint_dart_result_component(handle(result), as_ptr(&name)) as usize as u32 }
}

#[wasm_bindgen]
pub fn definition_free(definition: u32) {
    unsafe { crate::slint_dart_definition_free(definition as usize as *mut _) };
}

#[wasm_bindgen]
pub fn definition_name(definition: u32) -> String {
    take(crate::slint_dart_definition_name(unsafe { handle(definition) }))
}

#[wasm_bindgen]
pub fn definition_api(definition: u32) -> String {
    take(crate::slint_dart_definition_api(unsafe { handle(definition) }))
}

/// Instantiate the component. The handle comes back inside the usual envelope
/// because the C entry point reports its failure through an out-parameter,
/// which JavaScript has no way to pass.
#[wasm_bindgen]
pub fn definition_create(definition: u32) -> String {
    let mut error: *mut c_char = std::ptr::null_mut();
    let instance = unsafe { crate::slint_dart_definition_create(handle(definition), &mut error) };
    if instance.is_null() {
        return envelope_err(&take(error));
    }
    serde_json::json!({ "ok": instance as usize as u32 }).to_string()
}

// ---------------------------------------------------------------------------
// Component instance
// ---------------------------------------------------------------------------

#[wasm_bindgen]
pub fn instance_free(instance: u32) {
    unsafe { crate::slint_dart_instance_free(instance as usize as *mut _) };
}

#[wasm_bindgen]
pub fn instance_get_property(instance: u32, global: Option<String>, name: &str) -> String {
    let global = opt_c_string(global);
    let name = c_string(name);
    take(unsafe {
        crate::slint_dart_instance_get_property(handle(instance), as_ptr(&global), name.as_ptr())
    })
}

#[wasm_bindgen]
pub fn instance_set_property(
    instance: u32,
    global: Option<String>,
    name: &str,
    json: &str,
) -> String {
    let global = opt_c_string(global);
    let name = c_string(name);
    let json = c_string(json);
    take(unsafe {
        crate::slint_dart_instance_set_property(
            handle(instance),
            as_ptr(&global),
            name.as_ptr(),
            json.as_ptr(),
        )
    })
}

#[wasm_bindgen]
pub fn instance_invoke(
    instance: u32,
    global: Option<String>,
    name: &str,
    args_json: &str,
) -> String {
    let global = opt_c_string(global);
    let name = c_string(name);
    let args_json = c_string(args_json);
    take(unsafe {
        crate::slint_dart_instance_invoke(
            handle(instance),
            as_ptr(&global),
            name.as_ptr(),
            args_json.as_ptr(),
        )
    })
}

#[wasm_bindgen]
pub fn instance_show(instance: u32, visible: bool) -> String {
    take(crate::slint_dart_instance_show(unsafe { handle(instance) }, visible))
}

/// Install a handler for a callback. `id` comes back to the dispatcher set by
/// [`set_callback_dispatcher`], which is what identifies the Dart closure.
#[wasm_bindgen]
pub fn instance_set_callback(instance: u32, global: Option<String>, name: &str, id: u32) -> String {
    let global = opt_c_string(global);
    let name = c_string(name);
    take(unsafe {
        crate::slint_dart_instance_set_callback(
            handle(instance),
            as_ptr(&global),
            name.as_ptr(),
            call_dispatcher,
            free_dispatcher_result,
            id as usize as *mut c_void,
        )
    })
}

// ---------------------------------------------------------------------------
// Callbacks and timers
//
// Both cross back into JavaScript through one dispatcher each, called with the
// id the caller registered. That is the same indirection the native binding
// uses, where the id travels as `user_data`; here it saves handing wasm
// function pointers out to JavaScript.
// ---------------------------------------------------------------------------

thread_local! {
    static CALLBACK_DISPATCHER: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
    static TIMER_DISPATCHER: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
}

/// Install the function that runs Slint callbacks. It receives the handler id
/// and the arguments as JSON, and returns the result as JSON.
#[wasm_bindgen]
pub fn set_callback_dispatcher(dispatcher: js_sys::Function) {
    CALLBACK_DISPATCHER.with_borrow_mut(|slot| *slot = Some(dispatcher));
}

/// Install the function that runs timer callbacks. It receives the timer id.
#[wasm_bindgen]
pub fn set_timer_dispatcher(dispatcher: js_sys::Function) {
    TIMER_DISPATCHER.with_borrow_mut(|slot| *slot = Some(dispatcher));
}

unsafe extern "C" fn call_dispatcher(
    user_data: *mut c_void,
    args_json: *const c_char,
) -> *mut c_char {
    let args = unsafe { CStr::from_ptr(args_json) }.to_string_lossy().into_owned();
    let id = user_data as usize as f64;
    let returned = CALLBACK_DISPATCHER.with_borrow(|dispatcher| {
        dispatcher.as_ref().and_then(|dispatcher| {
            dispatcher
                .call2(&JsValue::NULL, &JsValue::from_f64(id), &JsValue::from_str(&args))
                .ok()
                .and_then(|value| value.as_string())
        })
    });
    match returned {
        Some(value) => c_string(&value).into_raw(),
        None => std::ptr::null_mut(),
    }
}

unsafe extern "C" fn free_dispatcher_result(value: *mut c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) });
    }
}

unsafe extern "C" fn tick_dispatcher(user_data: *mut c_void) {
    let id = user_data as usize as f64;
    TIMER_DISPATCHER.with_borrow(|dispatcher| {
        if let Some(dispatcher) = dispatcher.as_ref() {
            let _ = dispatcher.call1(&JsValue::NULL, &JsValue::from_f64(id));
        }
    });
}

#[wasm_bindgen]
pub fn timer_start(repeated: bool, interval_ms: f64, id: u32) -> u32 {
    unsafe {
        crate::slint_dart_timer_start(
            repeated,
            interval_ms.max(0.0) as u64,
            tick_dispatcher,
            id as usize as *mut c_void,
        ) as usize as u32
    }
}

#[wasm_bindgen]
pub fn timer_free(timer: u32) {
    unsafe { crate::slint_dart_timer_free(timer as usize as *mut _) };
}

// ---------------------------------------------------------------------------
// Embedded rendering
// ---------------------------------------------------------------------------

thread_local! {
    static FRAME: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

#[wasm_bindgen]
pub fn embedded_init() -> String {
    take(crate::embedded::slint_dart_embedded_init())
}

#[wasm_bindgen]
pub fn embedded_resize(width: u32, height: u32, scale_factor: f32) -> String {
    take(crate::embedded::slint_dart_embedded_resize(width, height, scale_factor))
}

#[wasm_bindgen]
pub fn embedded_size() -> String {
    take(crate::embedded::slint_dart_embedded_size())
}

/// Draw the next frame, or return nothing when the renderer had no repaint to
/// do. The frame buffer lives here and keeps its contents between calls, which
/// is what lets the renderer repaint only what changed.
///
/// ponytail: copies the frame out to JavaScript. A `Uint8Array::view` over the
/// wasm memory would save the copy, but it dangles as soon as the memory
/// grows; revisit if a profile ever blames this.
#[wasm_bindgen]
pub fn embedded_render(width: u32, height: u32) -> Option<js_sys::Uint8Array> {
    FRAME.with_borrow_mut(|frame| {
        let bytes = width as usize * height as usize * 4;
        if frame.len() != bytes {
            frame.clear();
            frame.resize(bytes, 0);
        }
        let drawn = unsafe {
            crate::embedded::slint_dart_embedded_render(frame.as_mut_ptr(), width, height)
        };
        drawn.then(|| js_sys::Uint8Array::from(frame.as_slice()))
    })
}

#[wasm_bindgen]
pub fn embedded_tick() -> f64 {
    crate::embedded::slint_dart_embedded_tick() as f64
}

#[wasm_bindgen]
pub fn embedded_has_active_animations() -> bool {
    crate::embedded::slint_dart_embedded_has_active_animations()
}

#[wasm_bindgen]
pub fn embedded_pointer_event(
    kind: u32,
    x: f32,
    y: f32,
    button: u32,
    delta_x: f32,
    delta_y: f32,
) -> String {
    take(crate::embedded::slint_dart_embedded_pointer_event(kind, x, y, button, delta_x, delta_y))
}

#[wasm_bindgen]
pub fn embedded_key_event(kind: u32, text: &str) -> String {
    let text = c_string(text);
    take(unsafe { crate::embedded::slint_dart_embedded_key_event(kind, text.as_ptr()) })
}

#[wasm_bindgen]
pub fn embedded_focus_event(focused: bool) -> String {
    take(crate::embedded::slint_dart_embedded_focus_event(focused))
}
