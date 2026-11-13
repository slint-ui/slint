// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This wasm library can be loaded from JS to load and display the content of .slint files
#![cfg(target_arch = "wasm32")]

#[cfg(all(feature = "browser", feature = "headless"))]
compile_error!(
    "Select either browser or headless; use --no-default-features --features headless for Node"
);

#[cfg(feature = "browser")]
const _: () = assert!(
    !i_slint_core::IMAGE_DECODERS_ENABLED,
    "The browser interpreter requires HTML image decoding; disable i-slint-core/image-decoders"
);

use std::rc::Rc;
use wasm_bindgen::prelude::*;

use i_slint_compiler::project_file::SlintEdition;
use i_slint_compiler::source_path::SourcePath;

#[cfg(feature = "browser")]
mod browser;
#[cfg(feature = "headless")]
mod headless;

#[wasm_bindgen]
#[allow(dead_code)]
pub struct CompilationResult {
    component: Option<WrappedCompiledComp>,
    diagnostics: js_sys::Array,
    error_string: String,
}

#[wasm_bindgen]
impl CompilationResult {
    #[wasm_bindgen(getter)]
    pub fn component(&self) -> Option<WrappedCompiledComp> {
        self.component.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn diagnostics(&self) -> js_sys::Array {
        self.diagnostics.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn error_string(&self) -> String {
        self.error_string.clone()
    }
}

#[wasm_bindgen(typescript_custom_section)]
const CALLBACK_FUNCTION_SECTION: &'static str = r#"
type ImportCallbackFunction = (url: string) => Promise<string | null>;
type ResourceLoaderFunction = (url: string) => Promise<Uint8Array>;
type CurrentElementInformationCallbackFunction = (url: string, start_line: number, start_column: number, end_line: number, end_column: number) => void;
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "ImportCallbackFunction")]
    pub type ImportCallbackFunction;

    #[wasm_bindgen(typescript_type = "ResourceLoaderFunction")]
    pub type ResourceLoaderFunction;

    #[wasm_bindgen(typescript_type = "CurrentElementInformationCallbackFunction")]
    pub type CurrentElementInformationCallbackFunction;
    #[wasm_bindgen(typescript_type = "Promise<WrappedInstance>")]
    pub type InstancePromise;
}

/// Compile the content of a string.
///
/// Returns a promise to a compiled component which can be run with ".run()"
///
/// `optional_import_callback` returns the source of an imported `.slint` file,
/// or `null` if the file doesn't exist, so that the search continues in the next include path.
/// `optional_resource_loader` returns the bytes of an imported font file.
/// The headless build also uses it to load image files.
/// Without it, the browser interpreter fetches the font from its URL.
/// `optional_project_root` is the URL of a project file (`slint-project.json`) or a directory that contains one.
/// The interpreter loads the project file through `optional_import_callback`.
///
/// It compiles with the edition of its own release.
/// Use [`compile_from_string_with_style`] to select another edition.
#[wasm_bindgen]
pub async fn compile_from_string(
    source: String,
    base_url: String,
    optional_import_callback: Option<ImportCallbackFunction>,
    optional_resource_loader: Option<ResourceLoaderFunction>,
    optional_project_root: Option<String>,
) -> Result<CompilationResult, JsValue> {
    compile_from_string_with_style(
        source,
        base_url,
        String::new(),
        optional_import_callback,
        optional_resource_loader,
        None,
        optional_project_root,
    )
    .await
}

/// Same as [`compile_from_string`], but also takes a style parameter.
///
/// `optional_edition` is the edition to compile with, such as `"1.18"`.
/// Without it, it compiles with the edition of its own release.
/// An edition that this interpreter doesn't support is a compilation error.
/// The `edition` of a project file under `optional_project_root` takes precedence.
#[wasm_bindgen]
pub async fn compile_from_string_with_style(
    source: String,
    base_url: String,
    style: String,
    optional_import_callback: Option<ImportCallbackFunction>,
    optional_resource_loader: Option<ResourceLoaderFunction>,
    optional_edition: Option<String>,
    optional_project_root: Option<String>,
) -> Result<CompilationResult, JsValue> {
    let mut compiler = slint_interpreter::Compiler::default();
    let config = compiler.compiler_configuration(i_slint_core::InternalToken);
    config.is_preview = true;
    config.components_to_generate = i_slint_compiler::ComponentSelection::LastExported;
    // The default of SlintPad, so that a snippet behaves the same in the docs and in SlintPad.
    let edition = match optional_edition.as_deref().map(SlintEdition::parse_supported) {
        None => SlintEdition::current(),
        Some(Ok(edition)) => edition,
        Some(Err(message)) => {
            let error =
                diagnostic_object(&message, (0, 0), "", slint_interpreter::DiagnosticLevel::Error)?;
            return Ok(CompilationResult {
                component: None,
                diagnostics: js_sys::Array::of1(&error.into()),
                error_string: message,
            });
        }
    };
    config.edition = Some(edition);
    if !style.is_empty() {
        compiler.set_style(style)
    }
    if let Some(root) = optional_project_root {
        compiler.set_project_root(Some(SourcePath::new(root)), i_slint_core::InternalToken);
    }

    if let Some(load_callback) = optional_import_callback {
        let open_import_callback = move |file_name: SourcePath| -> core::pin::Pin<
            Box<dyn core::future::Future<Output = Option<std::io::Result<String>>>>,
        > {
            Box::pin({
                let load_callback = js_sys::Function::from(load_callback.clone());
                let file_name = file_name.to_string();
                async move {
                    let result = match load_callback.call1(&JsValue::UNDEFINED, &file_name.into()) {
                        Ok(promise) => {
                            wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
                                .await
                        }
                        Err(js_err) => Err(js_err),
                    };
                    Some(match result {
                        Ok(js_ok) if js_ok.is_null() => Err(std::io::ErrorKind::NotFound.into()),
                        Ok(js_ok) => Ok(js_ok.as_string().unwrap_or_default()),
                        Err(js_err) => Err(std::io::Error::other(
                            js_err
                                .as_string()
                                .or_else(|| {
                                    js_err.dyn_ref::<js_sys::Error>().map(|e| e.message().into())
                                })
                                .unwrap_or_default(),
                        )),
                    })
                }
            })
        };
        let open_import_callback: i_slint_compiler::OpenImportCallback =
            Rc::new(open_import_callback);
        compiler.compiler_configuration(i_slint_core::InternalToken).open_import_callback =
            Some(open_import_callback.clone());
        compiler.set_project_file_loader(open_import_callback, i_slint_core::InternalToken);
    }

    #[cfg(feature = "headless")]
    let resource_errors =
        headless::set_image_loader(&mut compiler, optional_resource_loader.as_ref());

    if let Some(load_resource) = optional_resource_loader {
        let load_resource = js_sys::Function::from(JsValue::from(load_resource));
        compiler.set_resource_loader(
            move |path| {
                // Called right away, so that the page's loads overlap.
                let result = load_resource.call1(&JsValue::UNDEFINED, &path.to_string().into());
                Box::pin(async move {
                    let error = |e: JsValue| {
                        std::io::Error::other(
                            e.dyn_ref::<js_sys::Error>()
                                .map_or_else(|| format!("{e:?}"), |e| e.message().into()),
                        )
                    };
                    let promise: js_sys::Promise = result.map_err(error)?.into();
                    let data =
                        wasm_bindgen_futures::JsFuture::from(promise).await.map_err(error)?;
                    Ok(js_sys::Uint8Array::new(&data).to_vec())
                })
            },
            i_slint_core::InternalToken,
        );
    }

    let result = compiler.build_from_source(source, base_url.into()).await;

    let mut error_as_string = String::new();
    let array = js_sys::Array::new();
    for d in result.diagnostics() {
        let filename = d
            .source_file()
            .map(|path| path.to_string_lossy().into_owned())
            .or_else(|| d.source_url())
            .unwrap_or_default();

        let (line, column) = d.line_column();

        if d.level() == slint_interpreter::DiagnosticLevel::Error {
            if !error_as_string.is_empty() {
                error_as_string.push_str("\n");
            }
            use std::fmt::Write;

            write!(&mut error_as_string, "{}:{}:{}", filename, line, d).unwrap();
        }

        array.push(&diagnostic_object(&d.message(), (line, column), &filename, d.level())?.into());
    }

    #[cfg(feature = "headless")]
    for (path, message) in resource_errors.borrow().iter() {
        array.push(
            &diagnostic_object(message, (0, 0), path, slint_interpreter::DiagnosticLevel::Error)?
                .into(),
        );
        if !error_as_string.is_empty() {
            error_as_string.push('\n');
        }
        error_as_string.push_str(message);
    }

    Ok(CompilationResult {
        component: if error_as_string.is_empty() {
            result.components().next().map(WrappedCompiledComp)
        } else {
            None
        },
        diagnostics: array,
        error_string: error_as_string,
    })
}

fn diagnostic_object(
    message: &str,
    (line, column): (usize, usize),
    file_name: &str,
    level: slint_interpreter::DiagnosticLevel,
) -> Result<js_sys::Object, JsValue> {
    let object = js_sys::Object::new();
    js_sys::Reflect::set(&object, &"message".into(), &JsValue::from_str(message))?;
    js_sys::Reflect::set(&object, &"lineNumber".into(), &JsValue::from_f64(line as f64))?;
    js_sys::Reflect::set(&object, &"columnNumber".into(), &JsValue::from_f64(column as f64))?;
    js_sys::Reflect::set(&object, &"fileName".into(), &JsValue::from_str(file_name))?;
    js_sys::Reflect::set(&object, &"level".into(), &JsValue::from_f64(level as i8 as f64))?;
    Ok(object)
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct WrappedCompiledComp(slint_interpreter::ComponentDefinition);

/// Register a font for use by the `font-family` property.
///
/// `data` is the content of a TrueType or OpenType file.
/// The families it declares become available to the components compiled afterwards,
/// under the names the font itself carries,
/// so a page that registers JetBrains Mono can then use `font-family: "JetBrains Mono"`.
///
/// The browser build has no system fonts to query,
/// so this is the only way to draw a component in a font of the host page's choosing.
/// A font the page loaded through `@font-face` doesn't count:
/// the canvas renderer doesn't consult the CSS font database.
///
/// Text that was already laid out keeps the font it was shaped with,
/// so register the font before compiling the component that uses it.
///
/// Throws if the platform isn't initialized yet, or if the data declares no font family.
#[wasm_bindgen]
pub fn register_font_from_memory(data: Vec<u8>) -> Result<(), JsValue> {
    use i_slint_core::textlayout::sharedparley::fontique;

    // The shared collection holds the bytes through its own `Arc`, so the font doesn't need the
    // `&'static [u8]` that `Renderer::register_font_from_memory` asks for, and nothing is leaked.
    // Taking the `Vec` by value hands over the copy wasm-bindgen already made.
    let blob = fontique::Blob::new(std::sync::Arc::new(data));

    let registered = i_slint_core::with_existing_context(|ctx| {
        ctx.font_context().borrow_mut().register_fonts(blob)
    })?;

    if registered.is_empty() {
        return Err("the data declares no font family, \
                    expected a TrueType or OpenType file"
            .into());
    }
    Ok(())
}

