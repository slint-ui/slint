// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! This wasm library can be loaded from JS to load and display the content of .slint files
#![cfg(target_arch = "wasm32")]

use std::{future::Future, path::Path, pin::Pin, rc::Rc};
use wasm_bindgen::prelude::*;

use slint_interpreter::ComponentHandle;

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
export type ResourceUrlMapperFunction = (url: string) => Promise<string | undefined>;
type ImportCallbackFunction = (url: string) => Promise<string>;
type CurrentElementInformationCallbackFunction = (url: string, start_line: number, start_column: number, end_line: number, end_column: number) => void;
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "ResourceUrlMapperFunction")]
    pub type ResourceUrlMapperFunction;

    #[wasm_bindgen(typescript_type = "ImportCallbackFunction")]
    pub type ImportCallbackFunction;

    #[wasm_bindgen(typescript_type = "CurrentElementInformationCallbackFunction")]
    pub type CurrentElementInformationCallbackFunction;
    #[wasm_bindgen(typescript_type = "Promise<WrappedInstance>")]
    pub type InstancePromise;
}

fn resource_url_mapper_from_js(
    rum: ResourceUrlMapperFunction,
) -> Option<Rc<dyn Fn(&str) -> Pin<Box<dyn Future<Output = Option<String>>>>>> {
    let callback = js_sys::Function::from((*rum).clone());

    Some(Rc::new(move |url: &str| {
        let Some(promise) = callback.call1(&JsValue::UNDEFINED, &url.into()).ok() else {
            return Box::pin(std::future::ready(None));
        };
        let future = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise));
        Box::pin(async move { future.await.ok().and_then(|v| v.as_string()) })
    }))
}

/// Compile the content of a string.
///
/// Returns a promise to a compiled component which can be run with ".run()"
#[wasm_bindgen]
pub async fn compile_from_string(
    source: String,
    base_url: String,
    resource_url_mapper: Option<ResourceUrlMapperFunction>,
    optional_import_callback: Option<ImportCallbackFunction>,
) -> Result<CompilationResult, JsValue> {
    compile_from_string_with_style(
        source,
        base_url,
        String::new(),
        resource_url_mapper,
        optional_import_callback,
    )
    .await
}

/// Same as [`compile_from_string`], but also takes a style parameter
#[wasm_bindgen]
pub async fn compile_from_string_with_style(
    source: String,
    base_url: String,
    style: String,
    resource_url_mapper: Option<ResourceUrlMapperFunction>,
    optional_import_callback: Option<ImportCallbackFunction>,
) -> Result<CompilationResult, JsValue> {
    console_error_panic_hook::set_once();

    let mut compiler = slint_interpreter::ComponentCompiler::default();

    #[cfg(target_arch = "wasm32")]
    if let Some(rum) = resource_url_mapper {
        let cc = compiler.compiler_configuration(i_slint_core::InternalToken);
        cc.resource_url_mapper = resource_url_mapper_from_js(rum);
    }

    if !style.is_empty() {
        compiler.set_style(style)
    }

    if let Some(load_callback) = optional_import_callback {
        let open_import_fallback = move |file_name: &Path| -> core::pin::Pin<
            Box<dyn core::future::Future<Output = Option<std::io::Result<String>>>>,
        > {
            Box::pin({
                let load_callback = js_sys::Function::from(load_callback.clone());
                let file_name: String = file_name.to_string_lossy().into();
                async move {
                    let result = load_callback.call1(&JsValue::UNDEFINED, &file_name.into());
                    let promise: js_sys::Promise = result.unwrap().into();
                    let future = wasm_bindgen_futures::JsFuture::from(promise);
                    match future.await {
                        Ok(js_ok) => Some(Ok(js_ok.as_string().unwrap_or_default())),
                        Err(js_err) => Some(Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            js_err.as_string().unwrap_or_default(),
                        ))),
                    }
                }
            })
        };
        compiler.set_file_loader(open_import_fallback);
    }

    let c = compiler.build_from_source(source, base_url.into()).await;

    let line_key = JsValue::from_str("lineNumber");
    let column_key = JsValue::from_str("columnNumber");
    let message_key = JsValue::from_str("message");
    let file_key = JsValue::from_str("fileName");
    let level_key = JsValue::from_str("level");
    let mut error_as_string = String::new();
    let array = js_sys::Array::new();
    for d in compiler.diagnostics().into_iter() {
        let filename =
            d.source_file().as_ref().map_or(String::new(), |sf| sf.to_string_lossy().into());

        let filename_js = JsValue::from_str(&filename);
        let (line, column) = d.line_column();

        if d.level() == slint_interpreter::DiagnosticLevel::Error {
            if !error_as_string.is_empty() {
                error_as_string.push_str("\n");
            }
            use std::fmt::Write;

            write!(&mut error_as_string, "{}:{}:{}", filename, line, d).unwrap();
        }

        let error_obj = js_sys::Object::new();
        js_sys::Reflect::set(&error_obj, &message_key, &JsValue::from_str(&d.message()))?;
        js_sys::Reflect::set(&error_obj, &line_key, &JsValue::from_f64(line as f64))?;
        js_sys::Reflect::set(&error_obj, &column_key, &JsValue::from_f64(column as f64))?;
        js_sys::Reflect::set(&error_obj, &file_key, &filename_js)?;
        js_sys::Reflect::set(&error_obj, &level_key, &JsValue::from_f64(d.level() as i8 as f64))?;
        array.push(&error_obj);
    }

    Ok(CompilationResult {
        component: c.map(|c| WrappedCompiledComp(c)),
        diagnostics: array,
        error_string: error_as_string,
    })
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct WrappedCompiledComp(slint_interpreter::ComponentDefinition);

#[wasm_bindgen]
impl WrappedCompiledComp {
    /// Run this compiled component in a canvas.
    /// The HTML must contains a <canvas> element with the given `canvas_id`
    /// where the result is gonna be rendered
    #[wasm_bindgen]
    pub fn run(&self, canvas_id: String) {
        let component = self.0.create_with_canvas_id(&canvas_id).unwrap();
        component.show().unwrap();
        slint_interpreter::spawn_event_loop().unwrap();
    }
    /// Creates this compiled component in a canvas, wrapped in a promise.
    /// The HTML must contains a <canvas> element with the given `canvas_id`
    /// where the result is gonna be rendered.
    /// You need to call `show()` on the returned instance for rendering.
    ///
    /// Note that the promise will only be resolved after calling `slint.run_event_loop()`.
    #[wasm_bindgen]
    pub fn create(&self, canvas_id: String) -> Result<InstancePromise, JsValue> {
        Ok(JsValue::from(js_sys::Promise::new(&mut |resolve, reject| {
            let comp = send_wrapper::SendWrapper::new(self.0.clone());
            let canvas_id = canvas_id.clone();
            let resolve = send_wrapper::SendWrapper::new(resolve);
            if let Err(e) = slint_interpreter::invoke_from_event_loop(move || {
                let instance =
                    WrappedInstance(comp.take().create_with_canvas_id(&canvas_id).unwrap());
                resolve.take().call1(&JsValue::UNDEFINED, &JsValue::from(instance)).unwrap_throw();
            }) {
                reject
                    .call1(
                        &JsValue::UNDEFINED,
                        &JsValue::from(
                            format!("internal error: Failed to queue closure for event loop invocation: {e}"),
                        ),
                    )
                    .unwrap_throw();
            }
        })).unchecked_into::<InstancePromise>())
    }
    /// Creates this compiled component in the canvas of the provided instance, wrapped in a promise.
    /// For this to work, the provided instance needs to be visible (show() must've been
    /// called) and the event loop must be running (`slint.run_event_loop()`). After this
    /// call the provided instance is not rendered anymore and can be discarded.
    ///
    /// Note that the promise will only be resolved after calling `slint.run_event_loop()`.
    #[wasm_bindgen]
    pub fn create_with_existing_window(
        &self,
        instance: WrappedInstance,
    ) -> Result<InstancePromise, JsValue> {
        Ok(JsValue::from(js_sys::Promise::new(&mut |resolve, reject| {
            let params = send_wrapper::SendWrapper::new((self.0.clone(), instance.0.clone_strong(), resolve));
            if let Err(e) = slint_interpreter::invoke_from_event_loop(move || {
                let (comp, instance, resolve) = params.take();
                let instance =
                    WrappedInstance(comp.create_with_existing_window(instance.window()).unwrap());
                resolve.call1(&JsValue::UNDEFINED, &JsValue::from(instance)).unwrap_throw();
            }) {
                reject
                    .call1(
                        &JsValue::UNDEFINED,
                        &JsValue::from(
                            format!("internal error: Failed to queue closure for event loop invocation: {e}"),
                        ),
                    )
                    .unwrap_throw();
            }
        })).unchecked_into::<InstancePromise>())
    }
}

#[wasm_bindgen]
pub struct WrappedInstance(slint_interpreter::ComponentInstance);

impl Clone for WrappedInstance {
    fn clone(&self) -> Self {
        Self(self.0.clone_strong())
    }
}

#[wasm_bindgen]
impl WrappedInstance {
    /// Marks this instance for rendering and input handling.
    ///
    /// Note that the promise will only be resolved after calling `slint.run_event_loop()`.
    #[wasm_bindgen]
    pub fn show(&self) -> Result<js_sys::Promise, JsValue> {
        self.invoke_from_event_loop_wrapped_in_promise(|instance| instance.show())
    }
    /// Hides this instance and prevents further updates of the canvas element.
    ///
    /// Note that the promise will only be resolved after calling `slint.run_event_loop()`.
    #[wasm_bindgen]
    pub fn hide(&self) -> Result<js_sys::Promise, JsValue> {
        self.invoke_from_event_loop_wrapped_in_promise(|instance| instance.hide())
    }

    fn invoke_from_event_loop_wrapped_in_promise(
        &self,
        callback: impl FnOnce(
                &slint_interpreter::ComponentInstance,
            ) -> Result<(), slint_interpreter::PlatformError>
            + 'static,
    ) -> Result<js_sys::Promise, JsValue> {
        let callback = std::cell::RefCell::new(Some(callback));
        Ok(js_sys::Promise::new(&mut |resolve, reject| {
            let inst_weak = self.0.as_weak();

            if let Err(e) = slint_interpreter::invoke_from_event_loop({
                let params = send_wrapper::SendWrapper::new((
                    resolve,
                    reject.clone(),
                    callback.take().unwrap(),
                ));
                move || {
                    let (resolve, reject, callback) = params.take();
                    match inst_weak.upgrade() {
                        Some(instance) => match callback(&instance) {
                            Ok(()) => {
                                resolve.call0(&JsValue::UNDEFINED).unwrap_throw();
                            }
                            Err(e) => {
                                reject
                                    .call1(
                                        &JsValue::UNDEFINED,
                                        &JsValue::from(format!(
                                            "Invocation on ComponentInstance from within event loop failed: {e}"
                                        )),
                                    )
                                    .unwrap_throw();
                            }
                        },
                        None => {
                            reject
                            .call1(
                                &JsValue::UNDEFINED,
                                &JsValue::from(format!(
                                    "Invocation on ComponentInstance failed because instance was deleted too soon"
                                )),
                            )
                            .unwrap_throw();
                        }
                    }
                }
            }) {
                reject
                .call1(
                    &JsValue::UNDEFINED,
                    &JsValue::from(
                        format!("internal error: Failed to queue closure for event loop invocation: {e}"),
                    ),
                )
                .unwrap_throw();
            }
        }))
    }

    /// THIS FUNCTION IS NOT PART THE PUBLIC API!
    /// Highlights instances of the requested component
    #[wasm_bindgen]
    pub fn highlight(&self, _path: &str, _offset: u32) {
        self.0.highlight(_path.into(), _offset);
        let _ = slint_interpreter::invoke_from_event_loop(|| {}); // wake event loop
    }

    /// THIS FUNCTION IS NOT PART THE PUBLIC API!
    /// Request information on what to highlight in the editor based on clicks in the UI
    #[wasm_bindgen]
    pub fn set_design_mode(&self, active: bool) {
        self.0.set_design_mode(active);
        let _ = slint_interpreter::invoke_from_event_loop(|| {}); // wake event loop
    }

    /// THIS FUNCTION IS NOT PART THE PUBLIC API!
    /// Request information on what to highlight in the editor based on clicks in the UI
    #[wasm_bindgen]
    pub fn on_element_selected(&self, callback: CurrentElementInformationCallbackFunction) {
        self.0.on_element_selected(Box::new(
            move |url: &str, start_line: u32, start_column: u32, end_line: u32, end_column: u32| {
                let args = js_sys::Array::of5(
                    &url.into(),
                    &start_line.into(),
                    &start_column.into(),
                    &end_line.into(),
                    &end_column.into(),
                );
                let callback = js_sys::Function::from(callback.clone());
                let _ = callback.apply(&JsValue::UNDEFINED, &args);
            },
        ));
    }
}

/// Register DOM event handlers on all instance and set up the event loop for that.
/// You can call this function only once. It will throw an exception but that is safe
/// to ignore.
#[wasm_bindgen]
pub fn run_event_loop() -> Result<(), JsValue> {
    slint_interpreter::run_event_loop().map_err(|e| -> JsValue { format!("{e}").into() })?;
    Ok(())
}
