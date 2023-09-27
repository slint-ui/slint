// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! This wasm library can be loaded from JS to load and display the content of .slint files
#![cfg(target_arch = "wasm32")]

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

use wasm_bindgen::prelude::*;

use slint_interpreter::ComponentHandle;

use crate::{common::PreviewComponent, lsp_ext::Health};

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
type ImportCallbackFunction = (url: string) => Promise<string>;
type CurrentElementInformationCallbackFunction = (url: string, start_line: number, start_column: number, end_line: number, end_column: number) => void;
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "ImportCallbackFunction")]
    pub type ImportCallbackFunction;

    #[wasm_bindgen(typescript_type = "CurrentElementInformationCallbackFunction")]
    pub type CurrentElementInformationCallbackFunction;
    #[wasm_bindgen(typescript_type = "Promise<WrappedInstance>")]
    pub type InstancePromise;
    #[wasm_bindgen(typescript_type = "Promise<PreviewConnector>")]
    pub type PreviewConnectorPromise;

    // Make console.log available:
    #[allow(unused)]
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

/// Compile the content of a string.
///
/// Returns a promise to a compiled component which can be run with ".run()"
#[wasm_bindgen]
pub async fn compile_from_string(
    source: String,
    base_url: String,
    optional_import_callback: Option<ImportCallbackFunction>,
) -> Result<CompilationResult, JsValue> {
    compile_from_string_with_style(source, base_url, String::new(), optional_import_callback).await
}

/// Same as [`compile_from_string`], but also takes a style parameter
#[wasm_bindgen]
pub async fn compile_from_string_with_style(
    source: String,
    base_url: String,
    style: String,
    optional_import_callback: Option<ImportCallbackFunction>,
) -> Result<CompilationResult, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let mut compiler = slint_interpreter::ComponentCompiler::default();
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
        component.run().unwrap();
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
            if let Err(e) = slint::invoke_from_event_loop(move || {
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
    slint::run_event_loop().map_err(|e| -> JsValue { format!("{e}").into() })
}

#[derive(Default)]
struct PreviewState {
    ui: Option<super::ui::PreviewUi>,
    handle: Rc<RefCell<Option<slint_interpreter::ComponentInstance>>>,
}
thread_local! {static PREVIEW_STATE: std::cell::RefCell<PreviewState> = Default::default();}

#[wasm_bindgen]
pub struct PreviewConnector {
    current_previewed_component: RefCell<Option<PreviewComponent>>,
}

#[wasm_bindgen]
impl PreviewConnector {
    #[wasm_bindgen]
    pub fn create() -> Result<PreviewConnectorPromise, JsValue> {
        console_error_panic_hook::set_once();

        Ok(JsValue::from(js_sys::Promise::new(&mut |resolve, reject| {
            let resolve = send_wrapper::SendWrapper::new(resolve);
            let reject_c = send_wrapper::SendWrapper::new(reject.clone());
            if let Err(e) = slint_interpreter::invoke_from_event_loop(move || {
                PREVIEW_STATE.with(|preview_state| {
                    if preview_state.borrow().ui.is_some() {
                        reject_c.take().call1(&JsValue::UNDEFINED,
                            &JsValue::from("PreviewConnector already set up.")).unwrap_throw();
                    } else {
                        match super::ui::create_ui() {
                            Ok(ui) => {
                                preview_state.borrow_mut().ui = Some(ui);
                                resolve.take().call1(&JsValue::UNDEFINED,
                                        &JsValue::from(Self { current_previewed_component: RefCell::new(None) })).unwrap_throw()
                            }
                            Err(e) => reject_c.take().call1(&JsValue::UNDEFINED,
                                        &JsValue::from(format!("Failed to construct Preview UI: {e}"))).unwrap_throw(),
                        };
                    }
                })
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
        })).unchecked_into::<PreviewConnectorPromise>())
    }

    #[wasm_bindgen]
    pub fn show_ui(&self) -> Result<js_sys::Promise, JsValue> {
        {
            let mut cache = super::CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
            cache.ui_is_visible = true;
        }
        invoke_from_event_loop_wrapped_in_promise(|instance| instance.show())
    }

    #[wasm_bindgen]
    pub async fn process_lsp_to_preview_message(&self, value: JsValue) -> Result<(), JsValue> {
        use crate::common::LspToPreviewMessage as M;

        let message: M = serde_wasm_bindgen::from_value(value)
            .map_err(|e| -> JsValue { format!("{e:?}").into() })?;
        match message {
            M::SetContents { path, contents } => {
                super::set_contents(&PathBuf::from(&path), contents);
                if self.current_previewed_component.borrow().is_none() {
                    let pc = PreviewComponent {
                        path: PathBuf::from(path),
                        component: None,
                        style: String::new(),
                        include_paths: vec![],
                    };
                    *self.current_previewed_component.borrow_mut() = Some(pc.clone());
                    load_preview(pc);
                }
                Ok(())
            }
            M::SetConfiguration { style, include_paths } => {
                let ip: Vec<PathBuf> = include_paths.iter().map(|p| PathBuf::from(p)).collect();
                super::config_changed(&style, &ip);
                Ok(())
            }
            M::ShowPreview { path, component, style, include_paths } => {
                let pc = PreviewComponent {
                    path: PathBuf::from(path),
                    component,
                    style,
                    include_paths: include_paths.iter().map(|p| PathBuf::from(p)).collect(),
                };
                *self.current_previewed_component.borrow_mut() = Some(pc.clone());
                load_preview(pc);
                Ok(())
            }
            M::HighlightFromEditor { path, offset } => {
                super::highlight(path.map(|s| PathBuf::from(s)), offset);
                Ok(())
            }
        }
    }
}

fn invoke_from_event_loop_wrapped_in_promise(
    callback: impl FnOnce(&super::ui::PreviewUi) -> Result<(), slint_interpreter::PlatformError>
        + 'static,
) -> Result<js_sys::Promise, JsValue> {
    let callback = std::cell::RefCell::new(Some(callback));
    Ok(js_sys::Promise::new(&mut |resolve, reject| {
        PREVIEW_STATE.with(|preview_state|{
        let Some(inst_weak) = preview_state.borrow().ui.as_ref().map(|ui| ui.as_weak()) else {
            reject.call1(&JsValue::UNDEFINED, &JsValue::from("Ui is not up yet")).unwrap_throw();
            return;
        };

        if let Err(e) = slint::invoke_from_event_loop({
            let params =
                send_wrapper::SendWrapper::new((resolve, reject.clone(), callback.take().unwrap()));
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
                                            "Invocation on PreviewUi from within event loop failed: {e}"
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
                                    "Invocation on PreviewUi failed because instance was deleted too soon"
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
                    &JsValue::from(format!(
                        "internal error: Failed to queue closure for event loop invocation: {e}"
                    )),
                )
                .unwrap_throw();
        }

        })
    }))
}

pub fn configure_design_mode(enabled: bool) {
    slint::invoke_from_event_loop(move || {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow();
            let handle = preview_state.handle.borrow();
            if let Some(handle) = &*handle {
                handle.set_design_mode(enabled);

                handle.on_element_selected(Box::new(
                    move |file: &str,
                          start_line: u32,
                          start_column: u32,
                          end_line: u32,
                          end_column: u32| {
                        ask_editor_to_show_document(
                            file,
                            start_line,
                            start_column,
                            end_line,
                            end_column,
                        );
                    },
                ));
            }
        })
    })
    .unwrap();
}

pub fn load_preview(component: PreviewComponent) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static PENDING_EVENTS: AtomicU32 = AtomicU32::new(0);
    if PENDING_EVENTS.load(Ordering::SeqCst) > 0 {
        return;
    }
    PENDING_EVENTS.fetch_add(1, Ordering::SeqCst);
    slint::invoke_from_event_loop(move || {
        PENDING_EVENTS.fetch_sub(1, Ordering::SeqCst);
        i_slint_core::future::spawn_local(super::reload_preview(component)).unwrap();
    })
    .unwrap();
}

pub fn send_status(_message: &str, _health: Health) {
    // Do nothing for now...
}

pub fn notify_diagnostics(_diagnostics: &[slint_interpreter::Diagnostic]) -> Option<()> {
    // Do nothing for now...
    Some(())
}

pub fn ask_editor_to_show_document(
    _file: &str,
    _start_line: u32,
    _start_column: u32,
    _end_line: u32,
    _end_column: u32,
) {
    // Do nothing for now...
}

pub fn update_preview_area(compiled: slint_interpreter::ComponentDefinition) {
    PREVIEW_STATE.with(|preview_state| {
        let preview_state = preview_state.borrow_mut();

        let shared_handle = preview_state.handle.clone();

        super::set_preview_factory(
            preview_state.ui.as_ref().unwrap(),
            compiled,
            Box::new(move |instance| {
                shared_handle.replace(Some(instance));
            }),
        );
    })
}

pub fn update_highlight(path: PathBuf, offset: u32) {
    slint::invoke_from_event_loop(move || {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow();
            let handle = preview_state.handle.borrow();
            if let Some(handle) = &*handle {
                handle.highlight(path, offset);
            }
        })
    })
    .unwrap();
}
