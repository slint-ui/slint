// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore cdylib

//! C ABI over `slint-interpreter`, shaped for `dart:ffi`.
//!
//! `dart:ffi` speaks plain C: opaque pointers, `const char *`, and function
//! pointers. So this layer deliberately exposes nothing of Slint's Rust ABI —
//! no `SharedString`, no `SharedVector`, no `Box<Value>` — which spares the
//! Dart side from re-modelling layouts that carry no stability guarantee.
//!
//! Two conventions carry everything:
//!
//! * **Values travel as JSON.** The interpreter already converts between
//!   [`Value`] and JSON for the viewer and the LSP preview
//!   (`slint_interpreter::json`), and Dart already has `dart:convert`. Reusing
//!   both means neither side needs per-type marshalling code.
//! * **Fallible calls return a JSON envelope**: `{"ok": <value>}` or
//!   `{"err": "<message>"}`, as a heap-allocated NUL-terminated string that the
//!   caller releases with [`slint_dart_free_string`]. One decode step on the
//!   Dart side turns any error into an exception.
//!
//! Everything here must be called from the thread that runs the Slint event
//! loop, which is the Dart main isolate's thread. That matches the constraint
//! the Python and Node.js bindings already impose.

use i_slint_compiler::langtype::{Function, Type};
use i_slint_core::timers::{Timer, TimerMode};
use slint_interpreter::json::{value_from_json_str, value_to_json};
use slint_interpreter::{
    CompilationResult, Compiler, ComponentDefinition, ComponentHandle, ComponentInstance, Value,
};
use std::collections::BTreeSet;
use std::ffi::{CStr, CString, c_char, c_void};
use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;

mod embedded;

// ---------------------------------------------------------------------------
// Envelope and string helpers
// ---------------------------------------------------------------------------

/// Move a string onto the heap for the Dart side, which frees it again with
/// [`slint_dart_free_string`].
pub(crate) fn into_c_string(s: String) -> *mut c_char {
    // JSON never contains an interior NUL, but a malformed payload shouldn't
    // take the process down, so fall back to an empty string instead.
    CString::new(s).unwrap_or_default().into_raw()
}

pub(crate) fn ok(value: serde_json::Value) -> *mut c_char {
    into_c_string(serde_json::json!({ "ok": value }).to_string())
}

pub(crate) fn ok_void() -> *mut c_char {
    ok(serde_json::Value::Null)
}

pub(crate) fn err(message: impl std::fmt::Display) -> *mut c_char {
    into_c_string(serde_json::json!({ "err": message.to_string() }).to_string())
}

/// Turn an unwind into an error envelope.
///
/// A panic that reaches an `extern "C"` frame aborts the process, which would
/// take the whole Dart application down. Slint panics for legitimate reasons
/// the caller can act on — creating a window off the main thread, for one — so
/// every entry point that can reach interpreter code stops the unwind here in
/// builds whose panic strategy supports unwinding and reports it like any other
/// error. The workspace's release profile uses `panic = "abort"`, so release
/// builds cannot intercept a panic.
pub(crate) fn guard(body: impl FnOnce() -> *mut c_char) -> *mut c_char {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body))
        .unwrap_or_else(|panic| err(panic_message(&panic)))
}

pub(crate) fn panic_message(panic: &Box<dyn std::any::Any + Send>) -> String {
    let detail = panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown reason".into());
    format!("Slint panicked: {detail}")
}

/// Borrow a C string as `&str`. A null pointer becomes `None`.
///
/// # Safety
/// `s` must be null or point to a NUL-terminated string that outlives the call.
pub(crate) unsafe fn opt_str<'a>(s: *const c_char) -> Option<&'a str> {
    (!s.is_null()).then(|| unsafe { CStr::from_ptr(s) }.to_str().unwrap_or_default())
}

/// Same as [`opt_str`], but treats null as the empty string.
///
/// # Safety
/// See [`opt_str`].
pub(crate) unsafe fn str_or_empty<'a>(s: *const c_char) -> &'a str {
    unsafe { opt_str(s) }.unwrap_or_default()
}

/// Release a string returned by any of the functions in this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

// ---------------------------------------------------------------------------
// Type lookup
//
// Converting JSON into a `Value` needs the declared type of the target
// property, callback argument, or return value. `ComponentDefinition` is the
// only place that knows it, so every conversion starts from the instance's
// definition, the same way the viewer's `--load-data` does.
// ---------------------------------------------------------------------------

fn lookup_type(def: &ComponentDefinition, global: Option<&str>, name: &str) -> Option<Type> {
    let found = match global {
        None => def.properties_and_callbacks().find(|(n, _)| n == name),
        Some(global) => def.global_properties_and_callbacks(global)?.find(|(n, _)| n == name),
    };
    found.map(|(_, (ty, _))| ty)
}

/// The signature of a callback or a public function, whichever `ty` is.
fn as_function(ty: &Type) -> Option<&Function> {
    match ty {
        Type::Callback(f) | Type::Function(f) => Some(f),
        _ => None,
    }
}

fn parse_args(json: &str, signature: &Function) -> Result<Vec<Value>, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("invalid argument list: {e}"))?;
    let array = parsed.as_array().ok_or_else(|| "arguments must be a JSON array".to_string())?;
    if array.len() != signature.args.len() {
        return Err(format!("expected {} argument(s), got {}", signature.args.len(), array.len()));
    }
    array
        .iter()
        .zip(signature.args.iter())
        .map(|(v, ty)| slint_interpreter::json::value_from_json(ty, v))
        .collect()
}

fn values_to_json(values: &[Value]) -> Result<serde_json::Value, String> {
    values.iter().map(value_to_json).collect::<Result<Vec<_>, _>>().map(serde_json::Value::Array)
}

fn diagnostics_json(
    diagnostics: &i_slint_compiler::diagnostics::BuildDiagnostics,
) -> Vec<serde_json::Value> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let (line, column) = diagnostic.line_column();
            serde_json::json!({
                "level": match diagnostic.level() {
                    i_slint_compiler::diagnostics::DiagnosticLevel::Error => "error",
                    _ => "warning",
                },
                "message": diagnostic.message(),
                "file": diagnostic.source_file().map(|path| path.display().to_string()),
                "line": line,
                "column": column,
            })
        })
        .collect()
}

fn dart_generation_dependencies(
    input_path: PathBuf,
    dependencies: impl IntoIterator<Item = PathBuf>,
) -> BTreeSet<String> {
    dependencies
        .into_iter()
        .chain(std::iter::once(input_path))
        .filter(|path| !path.to_string_lossy().starts_with("builtin:"))
        .map(|path| std::path::absolute(&path).unwrap_or(path).to_string_lossy().into_owned())
        .collect()
}

fn dart_generation_result(
    source: Option<String>,
    error: Option<String>,
    dependencies: BTreeSet<String>,
    diagnostics: &i_slint_compiler::diagnostics::BuildDiagnostics,
) -> serde_json::Value {
    serde_json::json!({
        "source": source,
        "error": error,
        "dependencies": dependencies,
        "diagnostics": diagnostics_json(diagnostics),
    })
}

#[derive(Default)]
struct DartGenerationOptions {
    include_paths: Vec<PathBuf>,
    style: Option<String>,
}

fn parse_dart_generation_options(json: &str) -> Result<DartGenerationOptions, String> {
    if json.is_empty() {
        return Ok(Default::default());
    }
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|error| format!("Invalid Dart generation options: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "Dart generation options must be a JSON object".to_string())?;
    for name in object.keys() {
        if name != "include_paths" && name != "style" {
            return Err(format!("Unknown Dart generation option {name:?}"));
        }
    }

    let style = match object.get("style") {
        None => None,
        Some(serde_json::Value::String(style)) => Some(style.clone()),
        Some(_) => return Err("Dart generation option \"style\" must be a string".into()),
    };
    let include_paths = match object.get("include_paths") {
        None => Vec::new(),
        Some(serde_json::Value::Array(paths)) => paths
            .iter()
            .map(|path| {
                path.as_str().map(PathBuf::from).ok_or_else(|| {
                    "Dart generation option \"include_paths\" must be a list of strings".to_string()
                })
            })
            .collect::<Result<_, _>>()?,
        Some(_) => {
            return Err("Dart generation option \"include_paths\" must be a list of strings".into());
        }
    };

    Ok(DartGenerationOptions { include_paths, style })
}

// ---------------------------------------------------------------------------
// Compiler
// ---------------------------------------------------------------------------

fn generate_dart_bindings(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
    options: DartGenerationOptions,
) -> Result<serde_json::Value, String> {
    use i_slint_compiler::diagnostics::BuildDiagnostics;
    use i_slint_compiler::generator::OutputFormat;

    let input_path = std::path::absolute(input_path).map_err(|error| error.to_string())?;
    let output_path = std::path::absolute(output_path).map_err(|error| error.to_string())?;
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = i_slint_compiler::parser::parse_file(&input_path, &mut diagnostics);
    if diagnostics.has_errors() {
        let error = diagnostics.to_string_vec().join("\n");
        let dependencies = dart_generation_dependencies(input_path, std::iter::empty::<PathBuf>());
        return Ok(dart_generation_result(None, Some(error), dependencies, &diagnostics));
    }
    let Some(syntax_node) = syntax_node else {
        let dependencies = dart_generation_dependencies(input_path, std::iter::empty::<PathBuf>());
        return Ok(dart_generation_result(
            None,
            Some("The Slint parser produced no document".into()),
            dependencies,
            &diagnostics,
        ));
    };
    let format = OutputFormat::Dart;
    let mut compiler_config = i_slint_compiler::CompilerConfiguration::new(format.clone());
    compiler_config.include_paths = options.include_paths;
    compiler_config.style = options.style;
    let (document, diagnostics, loader) = spin_on::spin_on(i_slint_compiler::compile_syntax_node(
        syntax_node,
        diagnostics,
        compiler_config,
    ));
    let dependencies = dart_generation_dependencies(input_path, loader.all_files_to_watch());
    if diagnostics.has_errors() {
        let error = diagnostics.to_string_vec().join("\n");
        return Ok(dart_generation_result(None, Some(error), dependencies, &diagnostics));
    }

    let mut generated = Cursor::new(Vec::new());
    if let Err(error) = i_slint_compiler::generator::generate(
        format,
        &mut generated,
        Some(&output_path),
        &document,
        &loader.compiler_config,
    ) {
        return Ok(dart_generation_result(
            None,
            Some(error.to_string()),
            dependencies,
            &diagnostics,
        ));
    }
    let source = match String::from_utf8(generated.into_inner()) {
        Ok(source) => source,
        Err(error) => {
            return Ok(dart_generation_result(
                None,
                Some(error.to_string()),
                dependencies,
                &diagnostics,
            ));
        }
    };

    Ok(dart_generation_result(Some(source), None, dependencies, &diagnostics))
}

/// Generate typed Dart bindings for a `.slint` file.
///
/// The success envelope contains every file whose contents can affect the
/// result, plus either generated source or a generation error. The Dart builder
/// registers the dependencies before it reports the error so watch mode can
/// recover when an imported file changes.
///
/// # Safety
/// `input_path`, `output_path`, and `options_json` must point to NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_generate(
    input_path: *const c_char,
    output_path: *const c_char,
    options_json: *const c_char,
) -> *mut c_char {
    guard(|| {
        let input_path = PathBuf::from(unsafe { str_or_empty(input_path) });
        let output_path = PathBuf::from(unsafe { str_or_empty(output_path) });
        let options_json = unsafe { str_or_empty(options_json) };
        if input_path.as_os_str().is_empty() {
            return err("The Slint input path is empty");
        }
        if output_path.as_os_str().is_empty() {
            return err("The Dart output path is empty");
        }
        let options = match parse_dart_generation_options(options_json) {
            Ok(options) => options,
            Err(error) => return err(error),
        };
        generate_dart_bindings(&input_path, &output_path, options).map_or_else(err, ok)
    })
}

/// Create a compiler. Release it with [`slint_dart_compiler_free`].
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_compiler_new() -> *mut Compiler {
    Box::into_raw(Box::new(Compiler::default()))
}

/// # Safety
/// `compiler` must come from [`slint_dart_compiler_new`] and not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_compiler_free(compiler: *mut Compiler) {
    if !compiler.is_null() {
        drop(unsafe { Box::from_raw(compiler) });
    }
}

/// # Safety
/// `compiler` must be a live compiler, `style` a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_compiler_set_style(
    compiler: &mut Compiler,
    style: *const c_char,
) {
    compiler.set_style(unsafe { str_or_empty(style) }.to_string());
}

/// # Safety
/// `compiler` must be a live compiler, `path` a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_compiler_add_include_path(
    compiler: &mut Compiler,
    path: *const c_char,
) {
    let mut paths = compiler.include_paths().clone();
    paths.push(PathBuf::from(unsafe { str_or_empty(path) }));
    compiler.set_include_paths(paths);
}

/// Compile a `.slint` file. Inspect the result with
/// [`slint_dart_result_diagnostics`] and [`slint_dart_result_component`]; it is
/// null only if the compiler itself panicked.
///
/// # Safety
/// `compiler` must be a live compiler, `path` a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_compiler_build_from_path(
    compiler: &Compiler,
    path: *const c_char,
) -> *mut CompilationResult {
    let path = PathBuf::from(unsafe { str_or_empty(path) });
    into_raw_or_null(|| spin_on::spin_on(compiler.build_from_path(path)))
}

/// Compile `.slint` source code. `path` is only used for diagnostics and to
/// resolve relative imports. See [`slint_dart_compiler_build_from_path`].
///
/// # Safety
/// `compiler` must be a live compiler, `source` and `path` NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_compiler_build_from_source(
    compiler: &Compiler,
    source: *const c_char,
    path: *const c_char,
) -> *mut CompilationResult {
    let source = unsafe { str_or_empty(source) }.to_string();
    let path = PathBuf::from(unsafe { str_or_empty(path) });
    into_raw_or_null(|| spin_on::spin_on(compiler.build_from_source(source, path)))
}

/// Box the result of `body`, or return null if it panicked. See [`guard`].
fn into_raw_or_null<T>(body: impl FnOnce() -> T) -> *mut T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(value) => Box::into_raw(Box::new(value)),
        Err(panic) => {
            eprintln!("{}", panic_message(&panic));
            std::ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// Compilation result
// ---------------------------------------------------------------------------

/// # Safety
/// `result` must come from a `build_from_*` call and not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_result_free(result: *mut CompilationResult) {
    if !result.is_null() {
        drop(unsafe { Box::from_raw(result) });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_result_has_errors(result: &CompilationResult) -> bool {
    result.has_errors()
}

/// All diagnostics as a JSON array of
/// `{"level": "error"|"warning", "message": …, "file": …, "line": …, "column": …}`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_result_diagnostics(result: &CompilationResult) -> *mut c_char {
    let diagnostics = result
        .diagnostics()
        .map(|d| {
            let (line, column) = d.line_column();
            serde_json::json!({
                "level": match d.level() {
                    i_slint_compiler::diagnostics::DiagnosticLevel::Error => "error",
                    _ => "warning",
                },
                "message": d.message(),
                "file": d.source_file().map(|p| p.display().to_string()),
                "line": line,
                "column": column,
            })
        })
        .collect::<Vec<_>>();
    ok(serde_json::Value::Array(diagnostics))
}

/// The names of every component that can be instantiated, as a JSON array.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_result_component_names(result: &CompilationResult) -> *mut c_char {
    ok(result.component_names().collect::<Vec<_>>().into())
}

/// Look up a component by name; a null `name` picks the last exported one.
/// Returns null when there is no such component.
///
/// # Safety
/// `name` must be null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_result_component(
    result: &CompilationResult,
    name: *const c_char,
) -> *mut ComponentDefinition {
    let definition = match unsafe { opt_str(name) } {
        Some(name) => result.component(name),
        None => result.components().last(),
    };
    definition.map_or(std::ptr::null_mut(), |d| Box::into_raw(Box::new(d)))
}

// ---------------------------------------------------------------------------
// Component definition
// ---------------------------------------------------------------------------

/// # Safety
/// `definition` must come from [`slint_dart_result_component`] and not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_definition_free(definition: *mut ComponentDefinition) {
    if !definition.is_null() {
        drop(unsafe { Box::from_raw(definition) });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_definition_name(definition: &ComponentDefinition) -> *mut c_char {
    into_c_string(definition.name().to_string())
}

/// The public API of the component, as
/// `{"properties": {name: type}, "callbacks": [...], "functions": [...], "globals": [...]}`.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_definition_api(definition: &ComponentDefinition) -> *mut c_char {
    let properties = definition
        .properties()
        .map(|(name, ty)| (name, serde_json::Value::from(format!("{ty:?}"))))
        .collect::<serde_json::Map<_, _>>();
    ok(serde_json::json!({
        "properties": properties,
        "callbacks": definition.callbacks().collect::<Vec<_>>(),
        "functions": definition.functions().collect::<Vec<_>>(),
        "globals": definition.globals().collect::<Vec<_>>(),
    }))
}

/// Instantiate the component. Returns null on failure, with the reason written
/// to `error` (release it with [`slint_dart_free_string`]).
///
/// # Safety
/// `error` must be a valid pointer to a `*mut c_char`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_definition_create(
    definition: &ComponentDefinition,
    error: *mut *mut c_char,
) -> *mut ComponentInstance {
    // Creating the window adapter is where Slint learns it is on the wrong
    // thread or has no usable backend, and it says so by panicking.
    let created = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| definition.create()));
    match created {
        Ok(Ok(instance)) => Box::into_raw(Box::new(instance)),
        Ok(Err(e)) => {
            unsafe { *error = into_c_string(e.to_string()) };
            std::ptr::null_mut()
        }
        Err(panic) => {
            unsafe { *error = into_c_string(panic_message(&panic)) };
            std::ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// Component instance
// ---------------------------------------------------------------------------

/// # Safety
/// `instance` must come from [`slint_dart_definition_create`] and not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_instance_free(instance: *mut ComponentInstance) {
    if !instance.is_null() {
        drop(unsafe { Box::from_raw(instance) });
    }
}

/// Read a property. Pass a non-null `global` to read it from a global singleton.
///
/// # Safety
/// `global` must be null or NUL-terminated, `name` NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_instance_get_property(
    instance: &ComponentInstance,
    global: *const c_char,
    name: *const c_char,
) -> *mut c_char {
    guard(|| {
        let name = unsafe { str_or_empty(name) };
        let value = match unsafe { opt_str(global) } {
            None => instance.get_property(name).map_err(|e| e.to_string()),
            Some(global) => instance.get_global_property(global, name).map_err(|e| e.to_string()),
        };
        match value.and_then(|v| value_to_json(&v)) {
            Ok(json) => ok(json),
            Err(e) => err(e),
        }
    })
}

/// Write a property from its JSON representation. Pass a non-null `global` to
/// write it into a global singleton.
///
/// # Safety
/// `global` must be null or NUL-terminated, `name` and `json` NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_instance_set_property(
    instance: &ComponentInstance,
    global: *const c_char,
    name: *const c_char,
    json: *const c_char,
) -> *mut c_char {
    guard(|| {
        let global = unsafe { opt_str(global) };
        let name = unsafe { str_or_empty(name) };
        let json = unsafe { str_or_empty(json) };

        let Some(ty) = lookup_type(&instance.definition(), global, name) else {
            return err(format!("no such property: {name}"));
        };
        let value = match value_from_json_str(&ty, json) {
            Ok(value) => value,
            Err(e) => return err(e),
        };
        let result = match global {
            None => instance.set_property(name, value).map_err(|e| e.to_string()),
            Some(global) => {
                instance.set_global_property(global, name, value).map_err(|e| e.to_string())
            }
        };
        result.map_or_else(err, |()| ok_void())
    })
}

/// Call a callback or a public function with a JSON array of arguments, and
/// return its result. Pass a non-null `global` to reach into a global singleton.
///
/// # Safety
/// `global` must be null or NUL-terminated, `name` and `args_json` NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_instance_invoke(
    instance: &ComponentInstance,
    global: *const c_char,
    name: *const c_char,
    args_json: *const c_char,
) -> *mut c_char {
    guard(|| {
        let global = unsafe { opt_str(global) };
        let name = unsafe { str_or_empty(name) };
        let args_json = unsafe { str_or_empty(args_json) };

        let definition = instance.definition();
        let Some(ty) = lookup_type(&definition, global, name) else {
            return err(format!("no such callback or function: {name}"));
        };
        let Some(signature) = as_function(&ty) else {
            return err(format!("{name} is a property, not a callback or function"));
        };
        let args = match parse_args(args_json, signature) {
            Ok(args) => args,
            Err(e) => return err(e),
        };
        let result = match global {
            None => instance.invoke(name, &args).map_err(|e| e.to_string()),
            Some(global) => instance.invoke_global(global, name, &args).map_err(|e| e.to_string()),
        };
        match result.and_then(|v| value_to_json(&v)) {
            Ok(json) => ok(json),
            Err(e) => err(e),
        }
    })
}

/// The Dart handler for a Slint callback.
///
/// It receives the arguments as a JSON array and returns the result as a JSON
/// string it allocated itself, or null for a void callback. This module hands
/// that string straight back to `free_result` once it has been read, so the
/// two sides never free each other's allocations.
pub type DartCallback =
    unsafe extern "C" fn(user_data: *mut c_void, args_json: *const c_char) -> *mut c_char;

/// Releases a string returned by a [`DartCallback`].
pub type DartFree = unsafe extern "C" fn(s: *mut c_char);

struct DartHandler {
    callback: DartCallback,
    free_result: DartFree,
    user_data: *mut c_void,
    return_type: Type,
}

impl DartHandler {
    fn call(&self, args: &[Value]) -> Value {
        let args_json = match values_to_json(args) {
            Ok(json) => json.to_string(),
            Err(e) => {
                eprintln!("Slint: cannot pass callback arguments to Dart: {e}");
                return Value::Void;
            }
        };
        let Ok(args_json) = CString::new(args_json) else {
            return Value::Void;
        };

        let returned = unsafe { (self.callback)(self.user_data, args_json.as_ptr()) };
        if returned.is_null() {
            return Value::Void;
        }
        let json = unsafe { CStr::from_ptr(returned) }.to_string_lossy().into_owned();
        unsafe { (self.free_result)(returned) };

        match value_from_json_str(&self.return_type, &json) {
            Ok(value) => value,
            Err(e) => {
                eprintln!("Slint: cannot convert the Dart callback result: {e}");
                Value::Void
            }
        }
    }
}

/// Install a Dart handler for a callback. Pass a non-null `global` to reach
/// into a global singleton.
///
/// # Safety
/// `global` must be null or NUL-terminated and `name` NUL-terminated.
/// `callback` and `free_result` must stay valid, and `user_data` must stay
/// meaningful, until the instance is destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_instance_set_callback(
    instance: &ComponentInstance,
    global: *const c_char,
    name: *const c_char,
    callback: DartCallback,
    free_result: DartFree,
    user_data: *mut c_void,
) -> *mut c_char {
    guard(|| {
        let global = unsafe { opt_str(global) };
        let name = unsafe { str_or_empty(name) };

        let definition = instance.definition();
        let Some(ty) = lookup_type(&definition, global, name) else {
            return err(format!("no such callback: {name}"));
        };
        let Some(signature) = as_function(&ty) else {
            return err(format!("{name} is a property, not a callback"));
        };

        let handler = DartHandler {
            callback,
            free_result,
            user_data,
            return_type: signature.return_type.clone(),
        };
        let result = match global {
            None => instance
                .set_callback(name, move |args| handler.call(args))
                .map_err(|e| e.to_string()),
            Some(global) => instance
                .set_global_callback(global, name, move |args| handler.call(args))
                .map_err(|e| e.to_string()),
        };
        result.map_or_else(err, |()| ok_void())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_instance_show(
    instance: &ComponentInstance,
    visible: bool,
) -> *mut c_char {
    guard(|| {
        let result = if visible { instance.show() } else { instance.hide() };
        result.map_or_else(err, |()| ok_void())
    })
}

/// Show the window and run the event loop until the last window closes or
/// [`slint_dart_quit_event_loop`] is called.
#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_instance_run(instance: &ComponentInstance) -> *mut c_char {
    guard(|| instance.run().map_or_else(err, |()| ok_void()))
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_run_event_loop() -> *mut c_char {
    guard(|| slint_interpreter::run_event_loop().map_or_else(err, |()| ok_void()))
}

#[unsafe(no_mangle)]
pub extern "C" fn slint_dart_quit_event_loop() {
    let _ = i_slint_core::api::quit_event_loop();
}

// ---------------------------------------------------------------------------
// Timers
//
// Dart's own timers never fire while `slint_dart_instance_run` owns the
// thread, so periodic work has to be driven by Slint's event loop.
// ---------------------------------------------------------------------------

/// Start a timer. Release it with [`slint_dart_timer_free`], which also stops it.
///
/// # Safety
/// `callback` must stay valid, and `user_data` meaningful, until the returned
/// timer is freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_timer_start(
    repeated: bool,
    interval_ms: u64,
    callback: unsafe extern "C" fn(user_data: *mut c_void),
    user_data: *mut c_void,
) -> *mut Timer {
    let mode = if repeated { TimerMode::Repeated } else { TimerMode::SingleShot };
    let timer = Box::new(Timer::default());
    // Raw pointers aren't `Send`, but Slint timers only ever fire on the event
    // loop thread, which is the same thread that installed them.
    let user_data = user_data as usize;
    timer.start(mode, Duration::from_millis(interval_ms), move || unsafe {
        callback(user_data as *mut c_void)
    });
    Box::into_raw(timer)
}

/// # Safety
/// `timer` must come from [`slint_dart_timer_start`] and not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dart_timer_free(timer: *mut Timer) {
    if !timer.is_null() {
        drop(unsafe { Box::from_raw(timer) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read back and release an envelope, the way the Dart side does.
    pub(crate) fn envelope(ptr: *mut c_char) -> serde_json::Value {
        assert!(!ptr.is_null());
        let json = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string();
        unsafe { slint_dart_free_string(ptr) };
        serde_json::from_str(&json).unwrap()
    }

    pub(crate) fn unwrap_ok(ptr: *mut c_char) -> serde_json::Value {
        let value = envelope(ptr);
        assert!(value.get("err").is_none(), "unexpected error: {value}");
        value["ok"].clone()
    }

    pub(crate) fn unwrap_err(ptr: *mut c_char) -> String {
        let value = envelope(ptr);
        value["err"].as_str().expect("expected an error envelope").to_string()
    }

    pub(crate) fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    /// Compile `source` and instantiate its only component.
    fn instantiate(source: &str) -> ComponentInstance {
        i_slint_backend_testing::init_no_event_loop();
        let compiler = Compiler::default();
        let result = spin_on::spin_on(
            compiler.build_from_source(source.into(), PathBuf::from("test.slint")),
        );
        assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
        result.components().last().unwrap().create().unwrap()
    }

    const COUNTER: &str = r#"
        export struct Item { title: string, checked: bool }
        export global Logic {
            in-out property <int> offset: 3;
            callback shout(string) -> string;
            callback noted();
        }
        export component App {
            in-out property <int> value: 42;
            in-out property <string> label: "hello";
            in-out property <[Item]> items: [{ title: "a", checked: true }];
            callback add(string) -> int;
            public function double(v: int) -> int { v * 2 }
        }
    "#;

    #[test]
    fn get_and_set_properties() {
        let instance = instantiate(COUNTER);

        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "value") }), 42);
        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "label") }), "hello");

        unwrap_ok(unsafe { set(&instance, None, "value", "7") });
        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "value") }), 7);

        unwrap_ok(unsafe { set(&instance, None, "label", "\"bye\"") });
        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "label") }), "bye");
    }

    #[test]
    fn models_round_trip_as_json_arrays() {
        let instance = instantiate(COUNTER);

        assert_eq!(
            unwrap_ok(unsafe { get(&instance, None, "items") }),
            serde_json::json!([{ "title": "a", "checked": true }])
        );

        unwrap_ok(unsafe {
            set(
                &instance,
                None,
                "items",
                r#"[{"title": "b", "checked": false}, {"title": "c", "checked": true}]"#,
            )
        });
        assert_eq!(
            unwrap_ok(unsafe { get(&instance, None, "items") }),
            serde_json::json!([
                { "title": "b", "checked": false },
                { "title": "c", "checked": true },
            ])
        );
    }

    #[test]
    fn global_properties() {
        let instance = instantiate(COUNTER);
        assert_eq!(unwrap_ok(unsafe { get(&instance, Some("Logic"), "offset") }), 3);
        unwrap_ok(unsafe { set(&instance, Some("Logic"), "offset", "9") });
        assert_eq!(unwrap_ok(unsafe { get(&instance, Some("Logic"), "offset") }), 9);
    }

    #[test]
    fn unknown_names_report_errors_instead_of_panicking() {
        let instance = instantiate(COUNTER);
        assert!(unwrap_err(unsafe { get(&instance, None, "nope") }).contains("no such property"));
        assert!(
            unwrap_err(unsafe { set(&instance, None, "nope", "1") }).contains("no such property")
        );
        assert!(
            unwrap_err(unsafe { get(&instance, Some("Nope"), "offset") })
                .contains("no such property")
        );
    }

    #[test]
    fn setting_a_property_to_the_wrong_type_reports_an_error() {
        let instance = instantiate(COUNTER);
        let message = unwrap_err(unsafe { set(&instance, None, "value", "\"not a number\"") });
        assert!(!message.is_empty(), "expected a conversion error");
        // The property keeps its previous value.
        assert_eq!(unwrap_ok(unsafe { get(&instance, None, "value") }), 42);
    }

    #[test]
    fn invoke_a_public_function() {
        let instance = instantiate(COUNTER);
        let name = c("double");
        let args = c("[21]");
        let result = unsafe {
            slint_dart_instance_invoke(&instance, std::ptr::null(), name.as_ptr(), args.as_ptr())
        };
        assert_eq!(unwrap_ok(result), 42);
    }

    #[test]
    fn invoke_checks_the_argument_count() {
        let instance = instantiate(COUNTER);
        let name = c("double");
        let args = c("[]");
        let result = unsafe {
            slint_dart_instance_invoke(&instance, std::ptr::null(), name.as_ptr(), args.as_ptr())
        };
        assert!(unwrap_err(result).contains("expected 1 argument(s), got 0"));
    }

    // A callback handler standing in for the Dart side: it records the
    // arguments it saw and answers with a JSON string it allocated itself.
    thread_local! {
        static SEEN_ARGS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    unsafe extern "C" fn recording_handler(
        user_data: *mut c_void,
        args_json: *const c_char,
    ) -> *mut c_char {
        let args = unsafe { CStr::from_ptr(args_json) }.to_str().unwrap().to_string();
        SEEN_ARGS.with(|seen| seen.borrow_mut().push(args));
        // `user_data` is the id the caller passed in; echo it back as the result.
        into_c_string((user_data as usize).to_string())
    }

    unsafe extern "C" fn shout_handler(
        _user_data: *mut c_void,
        args_json: *const c_char,
    ) -> *mut c_char {
        let args: serde_json::Value =
            serde_json::from_str(unsafe { CStr::from_ptr(args_json) }.to_str().unwrap()).unwrap();
        into_c_string(serde_json::Value::from(args[0].as_str().unwrap().to_uppercase()).to_string())
    }

    unsafe extern "C" fn free_handler_result(s: *mut c_char) {
        unsafe { slint_dart_free_string(s) };
    }

    #[test]
    fn callbacks_receive_arguments_and_return_values() {
        let instance = instantiate(COUNTER);
        SEEN_ARGS.with(|seen| seen.borrow_mut().clear());

        let name = c("add");
        unwrap_ok(unsafe {
            slint_dart_instance_set_callback(
                &instance,
                std::ptr::null(),
                name.as_ptr(),
                recording_handler,
                free_handler_result,
                17 as *mut c_void,
            )
        });

        let args = c(r#"["milk"]"#);
        let result = unsafe {
            slint_dart_instance_invoke(&instance, std::ptr::null(), name.as_ptr(), args.as_ptr())
        };
        assert_eq!(unwrap_ok(result), 17);
        SEEN_ARGS.with(|seen| assert_eq!(seen.borrow().as_slice(), [r#"["milk"]"#]));
    }

    #[test]
    fn global_callbacks() {
        let instance = instantiate(COUNTER);
        let global = c("Logic");
        let name = c("shout");
        unwrap_ok(unsafe {
            slint_dart_instance_set_callback(
                &instance,
                global.as_ptr(),
                name.as_ptr(),
                shout_handler,
                free_handler_result,
                std::ptr::null_mut(),
            )
        });

        let args = c(r#"["hello"]"#);
        let result = unsafe {
            slint_dart_instance_invoke(&instance, global.as_ptr(), name.as_ptr(), args.as_ptr())
        };
        assert_eq!(unwrap_ok(result), "HELLO");
    }

    #[test]
    fn a_void_callback_may_answer_with_null() {
        let instance = instantiate(COUNTER);

        unsafe extern "C" fn void_handler(
            _user_data: *mut c_void,
            _args_json: *const c_char,
        ) -> *mut c_char {
            SEEN_ARGS.with(|seen| seen.borrow_mut().push("noted".into()));
            std::ptr::null_mut()
        }

        SEEN_ARGS.with(|seen| seen.borrow_mut().clear());
        let global = c("Logic");
        let name = c("noted");
        unwrap_ok(unsafe {
            slint_dart_instance_set_callback(
                &instance,
                global.as_ptr(),
                name.as_ptr(),
                void_handler,
                free_handler_result,
                std::ptr::null_mut(),
            )
        });

        let args = c("[]");
        unwrap_ok(unsafe {
            slint_dart_instance_invoke(&instance, global.as_ptr(), name.as_ptr(), args.as_ptr())
        });
        SEEN_ARGS.with(|seen| assert_eq!(seen.borrow().len(), 1));
    }

    #[test]
    fn a_panic_becomes_an_error_envelope_instead_of_aborting() {
        // Silence the panic hook so the deliberate panic below doesn't look
        // like a test failure in the output.
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = guard(|| panic!("boom"));
        std::panic::set_hook(previous);

        let message = unwrap_err(result);
        assert!(message.contains("boom"), "{message}");
    }

    #[test]
    fn compile_errors_are_reported_as_diagnostics() {
        i_slint_backend_testing::init_no_event_loop();
        let compiler = slint_dart_compiler_new();
        let source = c("export component Broken { this is not slint }");
        let path = c("broken.slint");
        let result = unsafe {
            slint_dart_compiler_build_from_source(&*compiler, source.as_ptr(), path.as_ptr())
        };

        assert!(slint_dart_result_has_errors(unsafe { &*result }));
        let diagnostics = unwrap_ok(slint_dart_result_diagnostics(unsafe { &*result }));
        let diagnostics = diagnostics.as_array().unwrap();
        assert!(!diagnostics.is_empty());
        assert_eq!(diagnostics[0]["level"], "error");
        assert!(diagnostics[0]["message"].as_str().is_some_and(|m| !m.is_empty()));

        unsafe { slint_dart_result_free(result) };
        unsafe { slint_dart_compiler_free(compiler) };
    }

    #[test]
    fn dart_generation_uses_camel_case_and_reports_imports() {
        let directory = std::env::temp_dir().join(format!(
            "slint-dart-codegen-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let shared = directory.join("shared.slint");
        let input = directory.join("app.slint");
        let output = directory.join("app.slint.dart");
        std::fs::write(&shared, "export component Shared { }").unwrap();
        std::fs::write(
            &input,
            r#"
                import { Shared } from "shared.slint";
                export component MainWindow inherits Shared {
                    in-out property <int> todo-model;
                    callback todo_added(string);
                    public function do_work(value: int) -> int { value }
                }
            "#,
        )
        .unwrap();

        let input_string = c(&input.to_string_lossy());
        let output_string = c(&output.to_string_lossy());
        let options = c("{}");
        let generated = unwrap_ok(unsafe {
            slint_dart_generate(input_string.as_ptr(), output_string.as_ptr(), options.as_ptr())
        });
        let source = generated["source"].as_str().unwrap();
        assert!(source.contains("int get todoModel"), "{source}");
        assert!(source.contains("void onTodoAdded"), "{source}");
        assert!(source.contains("int invokeDoWork"), "{source}");
        assert!(source.contains("getProperty(\"todo-model\")"), "{source}");
        assert!(source.contains("factory MainWindow.load("), "{source}");
        assert!(source.contains("factory MainWindow.loadSource("), "{source}");
        assert!(source.contains("slint.loadSource("), "{source}");

        let dependencies = generated["dependencies"].as_array().unwrap();
        assert!(dependencies.iter().any(|path| path.as_str().is_some_and(|path| {
            std::path::Path::new(path).file_name().is_some_and(|name| name == "app.slint")
        })));
        assert!(dependencies.iter().any(|path| path.as_str().is_some_and(|path| {
            std::path::Path::new(path).file_name().is_some_and(|name| name == "shared.slint")
        })));

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn dart_generation_rejects_camel_case_collisions() {
        let directory = std::env::temp_dir().join(format!(
            "slint-dart-collision-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let input = directory.join("app.slint");
        let output = directory.join("app.slint.dart");
        std::fs::write(
            &input,
            r#"
                export component App {
                    in-out property <int> foo-bar;
                    in-out property <int> fooBar;
                }
            "#,
        )
        .unwrap();

        let input_string = c(&input.to_string_lossy());
        let output_string = c(&output.to_string_lossy());
        let options = c("{}");
        let generated = unwrap_ok(unsafe {
            slint_dart_generate(input_string.as_ptr(), output_string.as_ptr(), options.as_ptr())
        });
        let message = generated["error"].as_str().unwrap();
        assert!(generated["source"].is_null());
        assert!(message.contains("foo-bar"), "{message}");
        assert!(message.contains("fooBar"), "{message}");
        assert!(message.contains("both generate"), "{message}");

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn dart_generation_reports_dependencies_with_import_errors() {
        let directory = std::env::temp_dir().join(format!(
            "slint-dart-import-error-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let shared = directory.join("shared.slint");
        let input = directory.join("app.slint");
        let output = directory.join("app.slint.dart");
        std::fs::write(&shared, "export component Shared { this is not slint }").unwrap();
        std::fs::write(
            &input,
            r#"
                import { Shared } from "shared.slint";
                export component App inherits Shared { }
            "#,
        )
        .unwrap();

        let input_string = c(&input.to_string_lossy());
        let output_string = c(&output.to_string_lossy());
        let options = c("{}");
        let generated = unwrap_ok(unsafe {
            slint_dart_generate(input_string.as_ptr(), output_string.as_ptr(), options.as_ptr())
        });

        assert!(generated["source"].is_null());
        assert!(generated["error"].as_str().is_some_and(|message| !message.is_empty()));
        let dependencies = generated["dependencies"].as_array().unwrap();
        assert!(dependencies.iter().any(|path| path.as_str().is_some_and(|path| {
            std::path::Path::new(path).file_name().is_some_and(|name| name == "app.slint")
        })));
        assert!(dependencies.iter().any(|path| path.as_str().is_some_and(|path| {
            std::path::Path::new(path).file_name().is_some_and(|name| name == "shared.slint")
        })));

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn dart_generation_applies_include_paths_and_style_to_runtime_defaults() {
        let directory = std::env::temp_dir().join(format!(
            "slint-dart-options-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let includes = directory.join("includes");
        std::fs::create_dir_all(&includes).unwrap();
        let shared = includes.join("shared.slint");
        let input = directory.join("app.slint");
        let output = directory.join("app.slint.dart");
        std::fs::write(&shared, "export component Shared { }").unwrap();
        std::fs::write(
            &input,
            r#"
                import { Shared } from "shared.slint";
                export component App inherits Shared { }
            "#,
        )
        .unwrap();

        let input_string = c(&input.to_string_lossy());
        let output_string = c(&output.to_string_lossy());
        let options = c(&serde_json::json!({
            "include_paths": [includes.to_string_lossy()],
            "style": "material",
        })
        .to_string());
        let generated = unwrap_ok(unsafe {
            slint_dart_generate(input_string.as_ptr(), output_string.as_ptr(), options.as_ptr())
        });

        assert!(generated["error"].is_null(), "{generated}");
        let source = generated["source"].as_str().unwrap();
        assert!(source.contains("String? style = \"material\""), "{source}");
        assert!(source.contains("List<String> includePaths = const ["), "{source}");
        assert!(
            !source.contains(&format!(
                "List<String> includePaths = const [{}]",
                serde_json::to_string(&includes.to_string_lossy()).unwrap()
            )),
            "{source}"
        );
        let dependencies = generated["dependencies"].as_array().unwrap();
        assert!(dependencies.iter().any(|path| path.as_str().is_some_and(|path| {
            std::path::Path::new(path).file_name().is_some_and(|name| name == "shared.slint")
        })));

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn dart_generation_rejects_invalid_options() {
        let input = c("app.slint");
        let output = c("app.slint.dart");
        let options = c(r#"{"include_paths":"not-a-list"}"#);
        let message = unwrap_err(unsafe {
            slint_dart_generate(input.as_ptr(), output.as_ptr(), options.as_ptr())
        });
        assert!(message.contains("include_paths"), "{message}");
        assert!(message.contains("list of strings"), "{message}");
    }

    #[test]
    fn a_missing_component_returns_null() {
        i_slint_backend_testing::init_no_event_loop();
        let compiler = slint_dart_compiler_new();
        let source = c("export component App { }");
        let path = c("app.slint");
        let result = unsafe {
            slint_dart_compiler_build_from_source(&*compiler, source.as_ptr(), path.as_ptr())
        };

        let names = unwrap_ok(slint_dart_result_component_names(unsafe { &*result }));
        assert_eq!(names, serde_json::json!(["App"]));

        let missing = c("Nope");
        assert!(
            unsafe { slint_dart_result_component(&*result, missing.as_ptr()) }.is_null(),
            "an unknown component name must not produce a definition"
        );

        // A null name picks a component without having to know its name.
        let definition = unsafe { slint_dart_result_component(&*result, std::ptr::null()) };
        assert!(!definition.is_null());
        let name = slint_dart_definition_name(unsafe { &*definition });
        assert_eq!(unsafe { CStr::from_ptr(name) }.to_str().unwrap(), "App");

        unsafe { slint_dart_free_string(name) };
        unsafe { slint_dart_definition_free(definition) };
        unsafe { slint_dart_result_free(result) };
        unsafe { slint_dart_compiler_free(compiler) };
    }

    // Thin wrappers so the tests above read as calls rather than pointer juggling.
    unsafe fn get(instance: &ComponentInstance, global: Option<&str>, name: &str) -> *mut c_char {
        let global = global.map(c);
        let name = c(name);
        unsafe {
            slint_dart_instance_get_property(
                instance,
                global.as_ref().map_or(std::ptr::null(), |g| g.as_ptr()),
                name.as_ptr(),
            )
        }
    }

    unsafe fn set(
        instance: &ComponentInstance,
        global: Option<&str>,
        name: &str,
        json: &str,
    ) -> *mut c_char {
        let global = global.map(c);
        let name = c(name);
        let json = c(json);
        unsafe {
            slint_dart_instance_set_property(
                instance,
                global.as_ref().map_or(std::ptr::null(), |g| g.as_ptr()),
                name.as_ptr(),
                json.as_ptr(),
            )
        }
    }
}
