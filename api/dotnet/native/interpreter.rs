// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::ffi::c_void;
use std::path::PathBuf;

use slint_interpreter::{
    Compiler, ComponentDefinition, ComponentHandle, ComponentInstance, Diagnostic, DiagnosticLevel,
    Value,
};

use crate::value::value_type_tag;
use crate::{NameList, OwnedString, Utf8Str, guard, guard_bool, into_raw, release, utf8_slice};

#[unsafe(no_mangle)]
pub extern "C" fn slint_dotnet_compiler_new() -> *mut Compiler {
    guard(std::ptr::null_mut(), || Ok(into_raw(Compiler::default())))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compiler_free(compiler: *mut Compiler) {
    unsafe { release(compiler) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compiler_set_style(
    compiler: &mut Compiler,
    style: Utf8Str,
) -> bool {
    guard_bool(|| {
        compiler.set_style(unsafe { style.as_str() }?.into());
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compiler_set_translation_domain(
    compiler: &mut Compiler,
    domain: Utf8Str,
) -> bool {
    guard_bool(|| {
        compiler.set_translation_domain(unsafe { domain.as_str() }?.into());
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compiler_set_include_paths(
    compiler: &mut Compiler,
    paths: *const Utf8Str,
    count: usize,
) -> bool {
    guard_bool(|| {
        let paths = unsafe { utf8_slice(paths, count) }?;
        compiler.set_include_paths(paths.into_iter().map(PathBuf::from).collect());
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compiler_set_library_paths(
    compiler: &mut Compiler,
    names: *const Utf8Str,
    paths: *const Utf8Str,
    count: usize,
) -> bool {
    guard_bool(|| {
        let names = unsafe { utf8_slice(names, count) }?;
        let paths = unsafe { utf8_slice(paths, count) }?;
        compiler.set_library_paths(
            names.into_iter().map(String::from).zip(paths.into_iter().map(PathBuf::from)).collect(),
        );
        Ok(())
    })
}

pub struct CompilationResult {
    result: slint_interpreter::CompilationResult,
    diagnostics: Vec<Diagnostic>,
}

impl From<slint_interpreter::CompilationResult> for CompilationResult {
    fn from(result: slint_interpreter::CompilationResult) -> Self {
        let diagnostics = result.diagnostics().collect();
        Self { result, diagnostics }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compiler_build_from_path(
    compiler: &Compiler,
    path: Utf8Str,
) -> *mut CompilationResult {
    guard(std::ptr::null_mut(), || {
        let path = unsafe { path.as_str() }?;
        Ok(into_raw(spin_on::spin_on(compiler.build_from_path(path)).into()))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compiler_build_from_source(
    compiler: &Compiler,
    source: Utf8Str,
    path: Utf8Str,
) -> *mut CompilationResult {
    guard(std::ptr::null_mut(), || {
        let (source, path) = unsafe { (source.as_str()?, path.as_str()?) };
        Ok(into_raw(
            spin_on::spin_on(compiler.build_from_source(source.into(), path.into())).into(),
        ))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compilation_result_free(result: *mut CompilationResult) {
    unsafe { release(result) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compilation_result_diagnostics_len(
    result: &CompilationResult,
) -> usize {
    result.diagnostics.len()
}

/// Matches `Slint.Diagnostic` in C#; the strings belong to the caller.
#[repr(C)]
pub struct DiagnosticInfo {
    level: i32,
    line: usize,
    column: usize,
    message: OwnedString,
    source_file: OwnedString,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compilation_result_diagnostic(
    result: &CompilationResult,
    index: usize,
    out: &mut DiagnosticInfo,
) -> bool {
    guard_bool(|| {
        let diagnostic = result.diagnostics.get(index).ok_or("diagnostic index out of range")?;
        let (line, column) = diagnostic.line_column();
        let source_file = diagnostic
            .source_file()
            .map(|p| p.to_string_lossy().into_owned())
            .or_else(|| diagnostic.source_url());
        *out = DiagnosticInfo {
            level: match diagnostic.level() {
                DiagnosticLevel::Error => 0,
                DiagnosticLevel::Warning => 1,
                DiagnosticLevel::Info => 2,
                _ => 3,
            },
            line,
            column,
            message: diagnostic.message().to_string().into(),
            source_file: source_file.map_or_else(OwnedString::none, OwnedString::from),
        };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compilation_result_component_names(
    result: &CompilationResult,
) -> *mut NameList {
    Box::into_raw(NameList::untagged(result.result.component_names().map(String::from)))
}

/// Returns null if there's no component `name`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compilation_result_component(
    result: &CompilationResult,
    name: Utf8Str,
) -> *mut ComponentDefinition {
    guard(std::ptr::null_mut(), || {
        Ok(result
            .result
            .component(unsafe { name.as_str() }?)
            .map_or(std::ptr::null_mut(), into_raw))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_definition_free(
    definition: *mut ComponentDefinition,
) {
    unsafe { release(definition) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_definition_name(
    definition: &ComponentDefinition,
) -> OwnedString {
    definition.name().to_string().into()
}

/// Property names, each tagged with its `ValueType`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_definition_properties(
    definition: &ComponentDefinition,
) -> *mut NameList {
    Box::into_raw(Box::new(NameList(
        definition.properties().map(|(n, ty)| (n, value_type_tag(ty))).collect(),
    )))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_definition_callbacks(
    definition: &ComponentDefinition,
) -> *mut NameList {
    Box::into_raw(NameList::untagged(definition.callbacks()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_definition_functions(
    definition: &ComponentDefinition,
) -> *mut NameList {
    Box::into_raw(NameList::untagged(definition.functions()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_definition_globals(
    definition: &ComponentDefinition,
) -> *mut NameList {
    Box::into_raw(NameList::untagged(definition.globals()))
}

/// Returns null if there's no global `global`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_definition_global_properties(
    definition: &ComponentDefinition,
    global: Utf8Str,
) -> *mut NameList {
    guard(std::ptr::null_mut(), || {
        Ok(definition.global_properties(unsafe { global.as_str() }?).map_or(
            std::ptr::null_mut(),
            |props| {
                Box::into_raw(Box::new(NameList(
                    props.map(|(n, ty)| (n, value_type_tag(ty))).collect(),
                )))
            },
        ))
    })
}

/// Returns null if there's no global `global`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_definition_global_callbacks(
    definition: &ComponentDefinition,
    global: Utf8Str,
) -> *mut NameList {
    guard(std::ptr::null_mut(), || {
        Ok(definition
            .global_callbacks(unsafe { global.as_str() }?)
            .map_or(std::ptr::null_mut(), |names| Box::into_raw(NameList::untagged(names))))
    })
}

/// Returns null if there's no global `global`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_definition_global_functions(
    definition: &ComponentDefinition,
    global: Utf8Str,
) -> *mut NameList {
    guard(std::ptr::null_mut(), || {
        Ok(definition
            .global_functions(unsafe { global.as_str() }?)
            .map_or(std::ptr::null_mut(), |names| Box::into_raw(NameList::untagged(names))))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_definition_create(
    definition: &ComponentDefinition,
) -> *mut ComponentInstance {
    guard(std::ptr::null_mut(), || definition.create().map(into_raw).map_err(|e| e.to_string()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_free(instance: *mut ComponentInstance) {
    unsafe { release(instance) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_show(
    instance: &ComponentInstance,
) -> bool {
    guard_bool(|| instance.show().map_err(|e| e.to_string()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_hide(
    instance: &ComponentInstance,
) -> bool {
    guard_bool(|| instance.hide().map_err(|e| e.to_string()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_run(instance: &ComponentInstance) -> bool {
    guard_bool(|| instance.run().map_err(|e| e.to_string()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_get_property(
    instance: &ComponentInstance,
    name: Utf8Str,
) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        instance.get_property(unsafe { name.as_str() }?).map(into_raw).map_err(|e| e.to_string())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_set_property(
    instance: &ComponentInstance,
    name: Utf8Str,
    value: &Value,
) -> bool {
    guard_bool(|| {
        instance.set_property(unsafe { name.as_str() }?, value.clone()).map_err(|e| e.to_string())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_get_global_property(
    instance: &ComponentInstance,
    global: Utf8Str,
    name: Utf8Str,
) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        let (global, name) = unsafe { (global.as_str()?, name.as_str()?) };
        instance.get_global_property(global, name).map(into_raw).map_err(|e| e.to_string())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_set_global_property(
    instance: &ComponentInstance,
    global: Utf8Str,
    name: Utf8Str,
    value: &Value,
) -> bool {
    guard_bool(|| {
        let (global, name) = unsafe { (global.as_str()?, name.as_str()?) };
        instance.set_global_property(global, name, value.clone()).map_err(|e| e.to_string())
    })
}

unsafe fn borrowed_args(args: *const *const Value, count: usize) -> Vec<Value> {
    if count == 0 {
        return Vec::new();
    }
    unsafe { std::slice::from_raw_parts(args, count) }
        .iter()
        .map(|v| unsafe { (**v).clone() })
        .collect()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_invoke(
    instance: &ComponentInstance,
    name: Utf8Str,
    args: *const *const Value,
    count: usize,
) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        let args = unsafe { borrowed_args(args, count) };
        instance.invoke(unsafe { name.as_str() }?, &args).map(into_raw).map_err(|e| e.to_string())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_invoke_global(
    instance: &ComponentInstance,
    global: Utf8Str,
    name: Utf8Str,
    args: *const *const Value,
    count: usize,
) -> *mut Value {
    guard(std::ptr::null_mut(), || {
        let args = unsafe { borrowed_args(args, count) };
        let (global, name) = unsafe { (global.as_str()?, name.as_str()?) };
        instance.invoke_global(global, name, &args).map(into_raw).map_err(|e| e.to_string())
    })
}

/// Receives borrowed arguments and stores an owned return value in `result`, or leaves it null for void.
pub type CallbackFn = unsafe extern "C" fn(
    user_data: *mut c_void,
    args: *const *const Value,
    count: usize,
    result: *mut *mut Value,
);

pub type DropUserDataFn = unsafe extern "C" fn(user_data: *mut c_void);

/// A managed delegate; dropping it releases `user_data` through `drop_user_data`.
struct ManagedCallback {
    callback: CallbackFn,
    user_data: *mut c_void,
    drop_user_data: DropUserDataFn,
}

impl ManagedCallback {
    fn call(&self, args: &[Value]) -> Value {
        let args: Vec<*const Value> = args.iter().map(|v| v as *const Value).collect();
        let mut result = std::ptr::null_mut();
        unsafe { (self.callback)(self.user_data, args.as_ptr(), args.len(), &mut result) };
        if result.is_null() { Value::Void } else { *unsafe { Box::from_raw(result) } }
    }
}

impl Drop for ManagedCallback {
    fn drop(&mut self) {
        unsafe { (self.drop_user_data)(self.user_data) }
    }
}

/// Takes ownership of `user_data`, also on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_set_callback(
    instance: &ComponentInstance,
    name: Utf8Str,
    callback: CallbackFn,
    user_data: *mut c_void,
    drop_user_data: DropUserDataFn,
) -> bool {
    let callback = ManagedCallback { callback, user_data, drop_user_data };
    guard_bool(|| {
        instance
            .set_callback(unsafe { name.as_str() }?, move |args| callback.call(args))
            .map_err(|e| e.to_string())
    })
}

/// Takes ownership of `user_data`, also on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_component_instance_set_global_callback(
    instance: &ComponentInstance,
    global: Utf8Str,
    name: Utf8Str,
    callback: CallbackFn,
    user_data: *mut c_void,
    drop_user_data: DropUserDataFn,
) -> bool {
    let callback = ManagedCallback { callback, user_data, drop_user_data };
    guard_bool(|| {
        let (global, name) = unsafe { (global.as_str()?, name.as_str()?) };
        instance
            .set_global_callback(global, name, move |args| callback.call(args))
            .map_err(|e| e.to_string())
    })
}
