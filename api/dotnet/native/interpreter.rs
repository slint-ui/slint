// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use base64::Engine as _;
use i_slint_compiler::source_path::clean_path;

use slint_interpreter::{Compiler, ComponentDefinition, Diagnostic, DiagnosticLevel};

use crate::instance::DotnetInstance;
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

/// The directory with `<lang>/LC_MESSAGES/<domain>.po` files to bundle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compiler_set_bundled_translations_path(
    compiler: &mut Compiler,
    path: Utf8Str,
) -> bool {
    guard_bool(|| {
        compiler.set_bundled_translations_path(unsafe { path.as_str() }?);
        Ok(())
    })
}

/// Without a default, `@tr()` strings have no context unless they give one, instead of the component name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compiler_set_default_translation_context(
    compiler: &mut Compiler,
    component_name: bool,
) -> bool {
    guard_bool(|| {
        compiler.set_default_translation_context(if component_name {
            slint_interpreter::DefaultTranslationContext::ComponentName
        } else {
            slint_interpreter::DefaultTranslationContext::None
        });
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

/// Makes the compiler read these files from memory instead of from the file system:
/// the `.slint` files that it loads, the images that they show, as `data:` URLs, and the fonts that they import.
/// A path that isn't among them still loads from the file system.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compiler_set_embedded_files(
    compiler: &mut Compiler,
    paths: *const Utf8Str,
    contents: *const *const u8,
    lengths: *const usize,
    count: usize,
) -> bool {
    guard_bool(|| {
        let paths = unsafe { utf8_slice(paths, count) }?;
        let mut files = HashMap::new();
        for (index, path) in paths.into_iter().enumerate() {
            let data =
                unsafe { std::slice::from_raw_parts(*contents.add(index), *lengths.add(index)) };
            files.insert(clean_path(Path::new(path)), Rc::<[u8]>::from(data));
        }
        let files = Rc::new(files);
        let get = {
            let files = files.clone();
            move |path: &Path| files.get(&clean_path(path)).cloned()
        };

        let sources = get.clone();
        compiler.set_file_loader(move |path| {
            let source = sources(path).map(|data| Ok(String::from_utf8_lossy(&data).into_owned()));
            Box::pin(async move { source })
        });

        let images = get.clone();
        compiler.compiler_configuration(i_slint_core::InternalToken).resource_url_mapper =
            Some(Rc::new(move |url| {
                let mapped = url.to_file_path().ok().and_then(|path| {
                    let data = images(&path)?;
                    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
                    let subtype = match extension.as_str() {
                        "jpg" => "jpeg",
                        "svg" | "svgz" => "svg+xml",
                        other => other,
                    };
                    let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                    url::Url::parse(&format!("data:image/{subtype};base64,{encoded}")).ok()
                });
                Box::pin(async move { mapped })
            }));

        compiler.set_resource_loader(
            move |path| {
                let data = match path.as_native_path() {
                    Some(native) => get(native).map(|data| Ok(data.to_vec())),
                    None => None,
                };
                let data = data.unwrap_or_else(|| match path.as_native_path() {
                    Some(native) => std::fs::read(native),
                    None => Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
                });
                Box::pin(async move { data })
            },
            i_slint_core::InternalToken,
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

/// Runs a unit that `slint-compiler -f csharp --csharp-unit` wrote, instead of compiling,
/// and fails for a unit of another Slint version.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slint_dotnet_compilation_result_from_unit(
    data: *const u8,
    len: usize,
) -> *mut CompilationResult {
    guard(std::ptr::null_mut(), || {
        let bytes = unsafe { std::slice::from_raw_parts(data, len) };
        let bundle = i_slint_compiler::llr::bundle::from_bytes(bytes)?;
        let resources = bundle.resources.into_iter().map(|resource| {
            slint_interpreter::LoadedResource::new(resource, i_slint_core::InternalToken)
        });
        Ok(into_raw(
            slint_interpreter::CompilationResult::from_compilation_unit_with_fonts(
                bundle.unit,
                resources,
                bundle.fonts,
                i_slint_core::InternalToken,
            )
            .into(),
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
) -> *mut DotnetInstance {
    guard(std::ptr::null_mut(), || {
        definition.create().map(|i| into_raw(DotnetInstance::Static(i))).map_err(|e| e.to_string())
    })
}
