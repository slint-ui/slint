// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This wasm library can be loaded from JS to load and display the content of .slint files
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

use slint_interpreter::{ComponentHandle, Value, ValueType};

mod value_conversion;
use value_conversion::{js_to_value, value_to_js};

use i_slint_core::model::ModelNotify;
use std::cell::Cell;
use std::collections::HashMap;

thread_local! {
    /// Maps wasm-side notify IDs to their backing `Rc<ModelNotify>`.
    /// Entries live as long as the corresponding [`WasmSharedModelNotify`]
    /// wrapper on the JS side; removed in its `Drop`.
    static NOTIFY_REGISTRY: RefCell<HashMap<u32, std::rc::Rc<ModelNotify>>> = Default::default();
    static NEXT_NOTIFY_ID: Cell<u32> = const { Cell::new(1) };
}

/// JavaScript-side handle for a `ModelNotify`. Holds a stable integer ID
/// looked up in [`NOTIFY_REGISTRY`]; the wrapper's `Drop` removes the entry
/// so the underlying `Rc<ModelNotify>` is freed once Rust no longer holds
/// any clones.
#[wasm_bindgen]
pub struct WasmSharedModelNotify {
    id: u32,
}

#[wasm_bindgen]
impl WasmSharedModelNotify {
    /// Exposed so JS can read the ID and Rust can look the notify up via
    /// `Reflect.get` when wrapping a JS Model.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> u32 {
        self.id
    }
}

impl Drop for WasmSharedModelNotify {
    fn drop(&mut self) {
        NOTIFY_REGISTRY.with(|r| {
            r.borrow_mut().remove(&self.id);
        });
    }
}

/// Look up the underlying notify by ID. Returns `None` if the JS wrapper
/// has been collected.
pub(crate) fn notify_from_id(id: u32) -> Option<std::rc::Rc<ModelNotify>> {
    NOTIFY_REGISTRY.with(|r| r.borrow().get(&id).cloned())
}

#[wasm_bindgen]
pub fn wasm_model_notify_new() -> WasmSharedModelNotify {
    let id = NEXT_NOTIFY_ID.with(|c| {
        let id = c.get();
        c.set(id + 1);
        id
    });
    NOTIFY_REGISTRY.with(|r| {
        r.borrow_mut().insert(id, std::rc::Rc::new(ModelNotify::default()));
    });
    WasmSharedModelNotify { id }
}

fn with_notify(id: u32, f: impl FnOnce(&ModelNotify)) {
    NOTIFY_REGISTRY.with(|r| {
        if let Some(n) = r.borrow().get(&id) {
            f(n);
        }
    });
}

#[wasm_bindgen]
pub fn wasm_model_notify_row_data_changed(notify: &WasmSharedModelNotify, row: u32) {
    with_notify(notify.id, |n| n.row_changed(row as usize));
    wake_event_loop();
}

#[wasm_bindgen]
pub fn wasm_model_notify_row_added(notify: &WasmSharedModelNotify, row: u32, count: u32) {
    with_notify(notify.id, |n| n.row_added(row as usize, count as usize));
    wake_event_loop();
}

#[wasm_bindgen]
pub fn wasm_model_notify_row_removed(notify: &WasmSharedModelNotify, row: u32, count: u32) {
    with_notify(notify.id, |n| n.row_removed(row as usize, count as usize));
    wake_event_loop();
}

#[wasm_bindgen]
pub fn wasm_model_notify_reset(notify: &WasmSharedModelNotify) {
    with_notify(notify.id, |n| n.reset());
    wake_event_loop();
}

/// Wake the winit event loop so that any pending `request_redraw` queued by a
/// property change is actually processed. On wasm, `request_redraw` alone does
/// not wake the loop (see the `WakeEventLoopWorkaround` comment in
/// `i_slint_backend_winit::event_loop`). Posting an empty closure via
/// `invoke_from_event_loop` posts the wake event as a side effect.
fn wake_event_loop() {
    let _ = slint_interpreter::invoke_from_event_loop(|| {});
}

#[wasm_bindgen]
#[allow(dead_code)]
pub struct CompilationResult {
    #[wasm_bindgen(skip)]
    pub definitions: Vec<WrappedDefinition>,
    diagnostics: js_sys::Array,
    error_string: String,
    #[wasm_bindgen(skip)]
    pub structs: JsValue,
    #[wasm_bindgen(skip)]
    pub enums: JsValue,
}

#[wasm_bindgen]
impl CompilationResult {
    #[wasm_bindgen(getter)]
    pub fn component(&self) -> Option<WrappedCompiledComp> {
        // Back-compat: return the first definition as a WrappedCompiledComp
        self.definitions.first().map(|d| WrappedCompiledComp(d.0.clone()))
    }
    #[wasm_bindgen(getter)]
    pub fn diagnostics(&self) -> js_sys::Array {
        self.diagnostics.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn error_string(&self) -> String {
        self.error_string.clone()
    }
    /// Returns an object mapping component names to WrappedDefinition instances.
    #[wasm_bindgen(getter, js_name = "definitions")]
    pub fn get_definitions(&self) -> JsValue {
        let obj = js_sys::Object::new();
        for def in &self.definitions {
            let name = JsValue::from_str(def.0.name());
            js_sys::Reflect::set(&obj, &name, &JsValue::from(def.clone())).unwrap_throw();
        }
        obj.into()
    }
    /// Returns an object mapping struct names to default-value objects.
    #[wasm_bindgen(getter, js_name = "structs")]
    pub fn get_structs(&self) -> JsValue {
        self.structs.clone()
    }
    /// Returns an object mapping enum names to enum-value objects.
    #[wasm_bindgen(getter, js_name = "enums")]
    pub fn get_enums(&self) -> JsValue {
        self.enums.clone()
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
}

fn build_diagnostics(compiler: &slint_interpreter::CompilationResult) -> (js_sys::Array, String) {
    let line_key = JsValue::from_str("lineNumber");
    let column_key = JsValue::from_str("columnNumber");
    let message_key = JsValue::from_str("message");
    let file_key = JsValue::from_str("fileName");
    let level_key = JsValue::from_str("level");
    let mut error_as_string = String::new();
    let array = js_sys::Array::new();
    for d in compiler.diagnostics() {
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
        js_sys::Reflect::set(&error_obj, &message_key, &JsValue::from_str(&d.message()))
            .unwrap_throw();
        js_sys::Reflect::set(&error_obj, &line_key, &JsValue::from_f64(line as f64)).unwrap_throw();
        js_sys::Reflect::set(&error_obj, &column_key, &JsValue::from_f64(column as f64))
            .unwrap_throw();
        js_sys::Reflect::set(&error_obj, &file_key, &filename_js).unwrap_throw();
        js_sys::Reflect::set(&error_obj, &level_key, &JsValue::from_f64(d.level() as i8 as f64))
            .unwrap_throw();
        array.push(&error_obj);
    }
    (array, error_as_string)
}

fn extract_structs_and_enums(
    compiler: &slint_interpreter::CompilationResult,
) -> (JsValue, JsValue) {
    use i_slint_compiler::langtype::Type;

    let structs_obj = js_sys::Object::new();
    let enums_obj = js_sys::Object::new();

    for ty in compiler.structs_and_enums(i_slint_core::InternalToken {}) {
        match ty {
            Type::Struct(s) => {
                if let Some(name) = s.name.slint_name() {
                    let default_obj = js_sys::Object::new();
                    for (field_name, field_type) in &s.fields {
                        let default_val = slint_interpreter::default_value_for_type(field_type);
                        js_sys::Reflect::set(
                            &default_obj,
                            &JsValue::from_str(field_name),
                            &value_to_js(&default_val),
                        )
                        .unwrap_throw();
                    }
                    js_sys::Reflect::set(&structs_obj, &JsValue::from_str(&name), &default_obj)
                        .unwrap_throw();
                }
            }
            Type::Enumeration(en) => {
                let enum_obj = js_sys::Object::new();
                for v in en.values.iter() {
                    let js_name = v.replace("-", "_");
                    js_sys::Reflect::set(
                        &enum_obj,
                        &JsValue::from_str(&js_name),
                        &JsValue::from_str(&js_name),
                    )
                    .unwrap_throw();
                }
                js_sys::Reflect::set(&enums_obj, &JsValue::from_str(en.name.as_str()), &enum_obj)
                    .unwrap_throw();
            }
            _ => {}
        }
    }
    (structs_obj.into(), enums_obj.into())
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
    let mut compiler = slint_interpreter::Compiler::default();
    if !style.is_empty() {
        compiler.set_style(style)
    }

    if let Some(load_callback) = optional_import_callback {
        let open_import_callback = move |file_name: &Path| -> core::pin::Pin<
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
        compiler.set_file_loader(open_import_callback);
    }

    let result = compiler.build_from_source(source, base_url.into()).await;

    let (diagnostics, error_as_string) = build_diagnostics(&result);
    let (structs, enums) = extract_structs_and_enums(&result);

    let definitions: Vec<WrappedDefinition> = result.components().map(WrappedDefinition).collect();

    Ok(CompilationResult {
        definitions,
        diagnostics,
        error_string: error_as_string,
        structs,
        enums,
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
        NEXT_CANVAS_ID.with(|next_id| {
            *next_id.borrow_mut() = Some(canvas_id);
        });
        let component = self.0.create().unwrap();
        component.show().unwrap();
        // Merely spawns the event loop, but does not block.
        slint_interpreter::run_event_loop().unwrap();
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
                NEXT_CANVAS_ID.with(|next_id| {
                    *next_id.borrow_mut() = Some(canvas_id);
                });
                let instance = WrappedInstance(comp.take().create().unwrap());
                resolve.take().call1(&JsValue::UNDEFINED, &JsValue::from(instance)).unwrap_throw();
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
        }))
        .unchecked_into::<InstancePromise>())
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
            let params = send_wrapper::SendWrapper::new((
                self.0.clone(),
                instance.0.clone_strong(),
                resolve,
            ));
            if let Err(e) = slint_interpreter::invoke_from_event_loop(move || {
                let (comp, instance, resolve) = params.take();
                let instance =
                    WrappedInstance(comp.create_with_existing_window(instance.window()).unwrap());
                resolve.call1(&JsValue::UNDEFINED, &JsValue::from(instance)).unwrap_throw();
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
        }))
        .unchecked_into::<InstancePromise>())
    }
}

// --- WrappedDefinition: exposes component definition metadata ---

#[wasm_bindgen]
#[derive(Clone)]
pub struct WrappedDefinition(slint_interpreter::ComponentDefinition);

fn property_info_to_js(name: &str, vt: ValueType) -> JsValue {
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"name".into(), &JsValue::from_str(name)).unwrap_throw();
    js_sys::Reflect::set(&obj, &"valueType".into(), &JsValue::from_f64(vt as u8 as f64))
        .unwrap_throw();
    obj.into()
}

#[wasm_bindgen]
impl WrappedDefinition {
    /// The name of this component.
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.0.name().into()
    }

    /// Whether this component creates a window.
    #[wasm_bindgen(getter, js_name = "isWindow")]
    pub fn is_window(&self) -> bool {
        self.0.is_window()
    }

    /// Returns an array of `{ name, valueType }` objects for each public property.
    #[wasm_bindgen(getter)]
    pub fn properties(&self) -> js_sys::Array {
        let arr = js_sys::Array::new();
        for (name, vt) in self.0.properties() {
            arr.push(&property_info_to_js(&name, vt));
        }
        arr
    }

    /// Returns an array of callback names.
    #[wasm_bindgen(getter)]
    pub fn callbacks(&self) -> js_sys::Array {
        self.0.callbacks().map(|s| JsValue::from_str(&s)).collect()
    }

    /// Returns an array of function names.
    #[wasm_bindgen(getter)]
    pub fn functions(&self) -> js_sys::Array {
        self.0.functions().map(|s| JsValue::from_str(&s)).collect()
    }

    /// Returns an array of exported global singleton names.
    #[wasm_bindgen(getter)]
    pub fn globals(&self) -> js_sys::Array {
        self.0.globals().map(|s| JsValue::from_str(&s)).collect()
    }

    /// Returns an array of property info objects for a global, or null.
    #[wasm_bindgen(js_name = "globalProperties")]
    pub fn global_properties(&self, global_name: &str) -> JsValue {
        match self.0.global_properties(global_name) {
            Some(props) => {
                let arr = js_sys::Array::new();
                for (name, vt) in props {
                    arr.push(&property_info_to_js(&name, vt));
                }
                arr.into()
            }
            None => JsValue::NULL,
        }
    }

    /// Returns an array of callback names for a global, or null.
    #[wasm_bindgen(js_name = "globalCallbacks")]
    pub fn global_callbacks(&self, global_name: &str) -> JsValue {
        match self.0.global_callbacks(global_name) {
            Some(cbs) => cbs.map(|s| JsValue::from_str(&s)).collect::<js_sys::Array>().into(),
            None => JsValue::NULL,
        }
    }

    /// Returns an array of function names for a global, or null.
    #[wasm_bindgen(js_name = "globalFunctions")]
    pub fn global_functions(&self, global_name: &str) -> JsValue {
        match self.0.global_functions(global_name) {
            Some(fns) => fns.map(|s| JsValue::from_str(&s)).collect::<js_sys::Array>().into(),
            None => JsValue::NULL,
        }
    }

    /// Creates a new instance of this component.
    /// If a canvas_id is provided, the component is rendered into that canvas.
    #[wasm_bindgen]
    pub fn create(&self, canvas_id: Option<String>) -> Result<WrappedInstance, JsValue> {
        if let Some(id) = canvas_id {
            NEXT_CANVAS_ID.with(|next_id| {
                *next_id.borrow_mut() = Some(id);
            });
        }
        self.0.create().map(WrappedInstance).map_err(|e| JsValue::from_str(&format!("{e}")))
    }
}

// --- WrappedInstance: property/callback access ---

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

    /// Returns the component definition for this instance.
    #[wasm_bindgen]
    pub fn definition(&self) -> WrappedDefinition {
        WrappedDefinition(self.0.definition())
    }

    /// Gets a property value by name.
    #[wasm_bindgen(js_name = "getProperty")]
    pub fn get_property(&self, name: &str) -> Result<JsValue, JsValue> {
        self.0
            .get_property(name)
            .map(|v| value_to_js(&v))
            .map_err(|e| JsValue::from_str(&format!("{e:?}")))
    }

    /// Sets a property value by name.
    #[wasm_bindgen(js_name = "setProperty")]
    pub fn set_property(&self, name: &str, value: JsValue) -> Result<(), JsValue> {
        let ty = self.0.definition().properties().find(|(n, _)| n == name).map(|(_, t)| t);
        let val = js_to_value(&value, ty.as_ref());
        let result =
            self.0.set_property(name, val).map_err(|e| JsValue::from_str(&format!("{e:?}")));
        wake_event_loop();
        result
    }

    /// Registers a callback handler.
    #[wasm_bindgen(js_name = "setCallback")]
    pub fn set_callback(&self, name: &str, callback: js_sys::Function) -> Result<(), JsValue> {
        self.0
            .set_callback(name, move |args| {
                let js_args: js_sys::Array = args.iter().map(|v| value_to_js(v)).collect();
                let result =
                    callback.apply(&JsValue::UNDEFINED, &js_args).unwrap_or(JsValue::UNDEFINED);
                js_to_value(&result, None)
            })
            .map_err(|e| JsValue::from_str(&format!("{e:?}")))
    }

    /// Invokes a callback or function by name.
    #[wasm_bindgen]
    pub fn invoke(&self, name: &str, args: js_sys::Array) -> Result<JsValue, JsValue> {
        let values: Vec<Value> = args.iter().map(|a| js_to_value(&a, None)).collect();
        let result = self
            .0
            .invoke(name, &values)
            .map(|v| value_to_js(&v))
            .map_err(|e| JsValue::from_str(&format!("{e:?}")));
        wake_event_loop();
        result
    }

    /// Gets a global property value.
    #[wasm_bindgen(js_name = "getGlobalProperty")]
    pub fn get_global_property(&self, global_name: &str, name: &str) -> Result<JsValue, JsValue> {
        self.0
            .get_global_property(global_name, name)
            .map(|v| value_to_js(&v))
            .map_err(|e| JsValue::from_str(&format!("{e:?}")))
    }

    /// Sets a global property value.
    #[wasm_bindgen(js_name = "setGlobalProperty")]
    pub fn set_global_property(
        &self,
        global_name: &str,
        name: &str,
        value: JsValue,
    ) -> Result<(), JsValue> {
        let ty = self
            .0
            .definition()
            .global_properties(global_name)
            .and_then(|mut props| props.find(|(n, _)| n == name).map(|(_, t)| t));
        let val = js_to_value(&value, ty.as_ref());
        let result = self
            .0
            .set_global_property(global_name, name, val)
            .map_err(|e| JsValue::from_str(&format!("{e:?}")));
        wake_event_loop();
        result
    }

    /// Registers a callback handler on a global.
    #[wasm_bindgen(js_name = "setGlobalCallback")]
    pub fn set_global_callback(
        &self,
        global_name: &str,
        name: &str,
        callback: js_sys::Function,
    ) -> Result<(), JsValue> {
        self.0
            .set_global_callback(global_name, name, move |args| {
                let js_args: js_sys::Array = args.iter().map(|v| value_to_js(v)).collect();
                let result =
                    callback.apply(&JsValue::UNDEFINED, &js_args).unwrap_or(JsValue::UNDEFINED);
                js_to_value(&result, None)
            })
            .map_err(|e| JsValue::from_str(&format!("{e:?}")))
    }

    /// Invokes a global callback or function by name.
    #[wasm_bindgen(js_name = "invokeGlobal")]
    pub fn invoke_global(
        &self,
        global_name: &str,
        name: &str,
        args: js_sys::Array,
    ) -> Result<JsValue, JsValue> {
        let values: Vec<Value> = args.iter().map(|a| js_to_value(&a, None)).collect();
        let result = self
            .0
            .invoke_global(global_name, name, &values)
            .map(|v| value_to_js(&v))
            .map_err(|e| JsValue::from_str(&format!("{e:?}")));
        wake_event_loop();
        result
    }

    /// Returns the window handle (not useful in WASM, included for API parity).
    #[wasm_bindgen]
    pub fn window(&self) -> JsValue {
        // Window management is canvas-based in WASM; return null for API parity.
        JsValue::NULL
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
                        &JsValue::from(format!(
                            "internal error: Failed to queue closure for event loop invocation: {e}"
                        )),
                    )
                    .unwrap_throw();
            }
        }))
    }
}

/// Register DOM event handlers on all instance and set up the event loop for that.
/// You can call this function only once. It will throw an exception but that is safe
/// to ignore.
#[wasm_bindgen]
pub fn run_event_loop() -> Result<(), JsValue> {
    // Merely spawns the event loop, but does not block.
    slint_interpreter::run_event_loop().map_err(|e| -> JsValue { format!("{e}").into() })?;
    Ok(())
}

/// Quit the running event loop. The winit loop stops processing input and
/// painting until `run_event_loop` is called again. The JS-side
/// `runEventLoop` Promise (in the TypeScript wrapper) resolves separately.
#[wasm_bindgen]
pub fn quit_event_loop() -> Result<(), JsValue> {
    slint_interpreter::quit_event_loop().map_err(|e| -> JsValue { format!("{e}").into() })
}

/// Set the HTML canvas element ID that the next created component instance
/// will render into. Consumed by the platform's window_attributes_hook on
/// the next window creation.
#[wasm_bindgen]
pub fn set_next_canvas_id(id: String) {
    NEXT_CANVAS_ID.with(|next_id| {
        *next_id.borrow_mut() = Some(id);
    });
}

thread_local!(
    static NEXT_CANVAS_ID: Rc<RefCell<Option<String>>> = Default::default();
);

#[wasm_bindgen(start)]
pub fn init() -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
    let backend = i_slint_backend_winit::Backend::builder()
        .with_spawn_event_loop(true)
        .with_window_attributes_hook(|mut attrs| {
            NEXT_CANVAS_ID.with(|next_id| {
                if let Some(canvas_id) = next_id.borrow_mut().take() {
                    use i_slint_backend_winit::winit::platform::web::WindowAttributesExtWebSys;

                    use wasm_bindgen::JsCast;

                    let html_canvas = web_sys::window()
                        .expect("wasm-interpreter: Could not retrieve DOM window")
                        .document()
                        .expect("wasm-interpreter: Could not retrieve DOM document")
                        .get_element_by_id(&canvas_id)
                        .unwrap_or_else(|| {
                            panic!(
                                "wasm-interpreter: Could not retrieve existing HTML Canvas element '{}'",
                                canvas_id
                            )
                        })
                        .dyn_into::<web_sys::HtmlCanvasElement>()
                        .unwrap_or_else(|_| {
                            panic!(
                                "winit backend: Specified DOM element '{}' is not a HTML Canvas",
                                canvas_id
                            )
                        });
                    attrs = attrs
                        .with_canvas(Some(html_canvas))
                        // Don't activate the window by default, as that will cause the page to scroll,
                        // ignoring any existing anchors.
                        .with_active(false);
                    attrs
                } else {
                    attrs
                }
            })
        })
        .build()
        .unwrap();
    i_slint_core::platform::set_platform(Box::new(backend))
        .map_err(|e| -> JsValue { format!("{e}").into() })?;
    Ok(())
}
