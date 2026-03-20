// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::JsComponentDefinition;
use super::JsDiagnostic;
use i_slint_compiler::langtype::Type;
use itertools::Itertools;
use napi::bindgen_prelude::*;
use napi::{Env, JsValue};
use slint_interpreter::Compiler;
use slint_interpreter::Value;
use smol_str::StrExt;
use std::collections::HashMap;
use std::path::PathBuf;

/// ComponentCompiler is the entry point to the Slint interpreter that can be used
/// to load .slint files or compile them on-the-fly from a string.
#[napi(js_name = "ComponentCompiler")]
pub struct JsComponentCompiler {
    internal: Compiler,
    structs_and_enums: Vec<Type>,
    diagnostics: Vec<slint_interpreter::Diagnostic>,
}

#[napi]
impl JsComponentCompiler {
    /// Returns a new ComponentCompiler.
    #[napi(constructor)]
    pub fn new() -> Self {
        let mut compiler = Compiler::default();
        let include_paths = match std::env::var_os("SLINT_INCLUDE_PATH") {
            Some(paths) => {
                std::env::split_paths(&paths).filter(|path| !path.as_os_str().is_empty()).collect()
            }
            None => Vec::new(),
        };
        let library_paths = match std::env::var_os("SLINT_LIBRARY_PATH") {
            Some(paths) => std::env::split_paths(&paths)
                .filter_map(|entry| {
                    entry
                        .to_str()
                        .unwrap_or_default()
                        .split('=')
                        .collect_tuple()
                        .map(|(k, v)| (k.into(), v.into()))
                })
                .collect(),
            None => HashMap::new(),
        };

        compiler.set_include_paths(include_paths);
        compiler.set_library_paths(library_paths);
        Self { internal: compiler, diagnostics: Vec::new(), structs_and_enums: vec![] }
    }

    #[napi(setter)]
    pub fn set_include_paths(&mut self, include_paths: Vec<String>) {
        self.internal.set_include_paths(include_paths.iter().map(PathBuf::from).collect());
    }

    #[napi(getter)]
    pub fn include_paths(&self) -> Vec<String> {
        self.internal
            .include_paths()
            .iter()
            .map(|p| p.to_str().unwrap_or_default().to_string())
            .collect()
    }

    #[napi(setter)]
    pub fn set_library_paths(&mut self, paths: HashMap<String, String>) {
        let mut library_paths = HashMap::new();
        for (key, path) in paths {
            library_paths.insert(key, PathBuf::from(path));
        }

        self.internal.set_library_paths(library_paths);
    }

    #[napi(getter)]
    pub fn library_paths(&self) -> HashMap<String, String> {
        let mut library_paths = HashMap::new();

        for (key, path) in self.internal.library_paths() {
            library_paths.insert(key.clone(), path.to_str().unwrap_or_default().to_string());
        }

        library_paths
    }

    #[napi(setter)]
    pub fn set_style(&mut self, style: String) {
        self.internal.set_style(style);
    }

    #[napi(getter)]
    pub fn style(&self) -> Option<String> {
        self.internal.style().cloned()
    }

    #[napi(getter)]
    pub fn diagnostics(&self) -> Vec<JsDiagnostic> {
        self.diagnostics.iter().map(|d| JsDiagnostic::from(d.clone())).collect()
    }

    #[napi(getter)]
    pub fn structs<'a>(&self, env: &'a Env) -> HashMap<String, Unknown<'a>> {
        fn convert_type<'a>(env: &'a Env, ty: &Type) -> Option<(String, Unknown<'a>)> {
            match ty {
                Type::Struct(s) if s.node().is_some() => {
                    let name = s.name.slint_name().unwrap();
                    let struct_instance = crate::to_js_unknown(
                        env,
                        &Value::Struct(slint_interpreter::Struct::from_iter(s.fields.iter().map(
                            |(name, field_type)| {
                                (
                                    name.to_string(),
                                    slint_interpreter::default_value_for_type(field_type),
                                )
                            },
                        ))),
                    );

                    Some((name.to_string(), struct_instance.ok()?))
                }
                _ => None,
            }
        }

        self.structs_and_enums
            .iter()
            .filter_map(|ty| convert_type(env, ty))
            .collect::<HashMap<String, Unknown<'a>>>()
    }

    #[napi(getter)]
    pub fn enums<'a>(&self, env: &'a Env) -> HashMap<String, Unknown<'a>> {
        fn convert_type<'a>(env: &'a Env, ty: &Type) -> Option<(String, Unknown<'a>)> {
            match ty {
                Type::Enumeration(en) => {
                    let mut o = Object::new(env).ok()?;

                    for value in en.values.iter() {
                        let value = value.replace_smolstr("-", "_");
                        let str_val = env.create_string(&value).ok()?;
                        o.set_named_property(&value, str_val).ok()?;
                    }
                    Some((en.name.to_string(), o.into_unknown(env).ok()?))
                }
                _ => None,
            }
        }

        self.structs_and_enums
            .iter()
            .filter_map(|ty| convert_type(env, ty))
            .collect::<HashMap<String, Unknown<'a>>>()
    }

    #[napi(setter)]
    pub fn set_file_loader(
        &mut self,
        env: &Env,
        callback: crate::DynFunction<'_>,
    ) -> napi::Result<()> {
        let stored_fn = std::rc::Rc::new(crate::StoredFunction::new(&callback)?);
        let env = *env;

        self.internal.set_file_loader(move |path| {
            let path = PathBuf::from(path);
            let stored_fn = stored_fn.clone();
            Box::pin({
                async move {
                    let Ok(path_str) = env.create_string(path.display().to_string().as_str())
                    else {
                        return Some(Err(std::io::Error::other(
                            "Node.js: wrong argument for callback file_loader.",
                        )));
                    };

                    let Ok(result) = stored_fn.call(&env, vec![path_str.raw()]) else {
                        return Some(Err(std::io::Error::other(
                            "Node.js: file loader callback failed.",
                        )));
                    };

                    let js_string = result.coerce_to_string();
                    let Ok(js_string) = js_string else {
                        return Some(Err(std::io::Error::other(
                            "Node.js: cannot read return value of file loader callback as js string.",
                        )));
                    };

                    let Ok(utf8_string) = js_string.into_utf8() else {
                        return Some(Err(std::io::Error::other(
                        "Node.js: cannot convert return value of file loader callback into utf8.",
                    )));
                    };

                    if let Ok(str) = utf8_string.as_str() {
                        let string = str.to_string();

                        return Some(Ok(string));
                    };

                    Some(Err(std::io::Error::other(
                        "Node.js: cannot convert return value of file loader callback into string.",
                    )))
                }
            })
        });

        Ok(())
    }

    /// Compile a .slint file into a ComponentDefinition
    ///
    /// Returns the compiled `ComponentDefinition` if there were no errors.
    #[napi]
    pub fn build_from_path(&mut self, path: String) -> HashMap<String, JsComponentDefinition> {
        let r = spin_on::spin_on(self.internal.build_from_path(PathBuf::from(path)));
        self.structs_and_enums =
            r.structs_and_enums(i_slint_core::InternalToken {}).cloned().collect::<Vec<_>>();
        self.diagnostics = r.diagnostics().collect();
        r.components().map(|c| (c.name().to_owned(), c.into())).collect()
    }

    /// Compile some .slint code into a ComponentDefinition
    #[napi]
    pub fn build_from_source(
        &mut self,
        source_code: String,
        path: String,
    ) -> HashMap<String, JsComponentDefinition> {
        let r = spin_on::spin_on(self.internal.build_from_source(source_code, PathBuf::from(path)));
        self.diagnostics = r.diagnostics().collect();
        self.structs_and_enums =
            r.structs_and_enums(i_slint_core::InternalToken {}).cloned().collect::<Vec<_>>();
        r.components().map(|c| (c.name().to_owned(), c.into())).collect()
    }
}
