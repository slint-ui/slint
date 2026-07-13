// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This wasm library can be loaded from JS to load and display the content of .slint files
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

use slint_interpreter::{ComponentHandle, Value, ValueType};

mod types;
pub use types::*;
mod value_conversion;
use value_conversion::{js_to_value_typed, value_to_js};

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
    let mut error_as_string = String::new();
    let array = js_sys::Array::new();
    for d in compiler.diagnostics() {
        let data = crate::shared::DiagnosticData::from(&d);

        if data.level == slint_interpreter::DiagnosticLevel::Error {
            if !error_as_string.is_empty() {
                error_as_string.push_str("\n");
            }
            use std::fmt::Write;
            write!(
                &mut error_as_string,
                "{}:{}:{}",
                data.file_name.as_deref().unwrap_or_default(),
                data.line_number,
                d
            )
            .unwrap();
        }

        let error_obj = js_sys::Object::new();
        js_sys::Reflect::set(&error_obj, &"message".into(), &JsValue::from_str(&data.message))
            .unwrap_throw();
        js_sys::Reflect::set(
            &error_obj,
            &"lineNumber".into(),
            &JsValue::from_f64(data.line_number as f64),
        )
        .unwrap_throw();
        js_sys::Reflect::set(
            &error_obj,
            &"columnNumber".into(),
            &JsValue::from_f64(data.column_number as f64),
        )
        .unwrap_throw();
        js_sys::Reflect::set(
            &error_obj,
            &"fileName".into(),
            &JsValue::from_str(data.file_name.as_deref().unwrap_or_default()),
        )
        .unwrap_throw();
        js_sys::Reflect::set(
            &error_obj,
            &"level".into(),
            &JsValue::from_f64(if data.level == slint_interpreter::DiagnosticLevel::Error {
                0.0
            } else {
                1.0
            }),
        )
        .unwrap_throw();
        array.push(&error_obj);
    }
    (array, error_as_string)
}

fn extract_structs_and_enums(
    compiler: &slint_interpreter::CompilationResult,
) -> (JsValue, JsValue) {
    let types: Vec<_> = compiler.structs_and_enums(i_slint_core::InternalToken {}).collect();

    let structs_obj = js_sys::Object::new();
    for (name, default_struct) in crate::shared::extract_structs(types.iter().copied()) {
        js_sys::Reflect::set(
            &structs_obj,
            &JsValue::from_str(&name),
            &value_to_js(&Value::Struct(default_struct)),
        )
        .unwrap_throw();
    }

    let enums_obj = js_sys::Object::new();
    for (name, values) in crate::shared::extract_enums(types.iter().copied()) {
        let enum_obj = js_sys::Object::new();
        for js_name in values {
            js_sys::Reflect::set(
                &enum_obj,
                &JsValue::from_str(&js_name),
                &JsValue::from_str(&js_name),
            )
            .unwrap_throw();
        }
        js_sys::Reflect::set(&enums_obj, &JsValue::from_str(&name), &enum_obj).unwrap_throw();
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
    // The test cases run by the browser test driver may use experimental
    // syntax; SLINT_ENABLE_EXPERIMENTAL_FEATURES cannot be set on wasm.
    #[cfg(feature = "testing")]
    {
        compiler.compiler_configuration(i_slint_core::InternalToken).enable_experimental = true;
    }
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

    /// Simulates a mouse click at the given logical position (testing builds only).
    #[cfg(feature = "testing")]
    #[wasm_bindgen(js_name = "sendMouseClick")]
    pub fn send_mouse_click(&self, x: f64, y: f64) {
        let window_adapter =
            i_slint_core::window::WindowInner::from_pub(self.0.window()).window_adapter();
        i_slint_backend_testing::testing_backend::send_mouse_click(
            x as f32,
            y as f32,
            &window_adapter,
        );
    }

    /// Types the given string as individual key press/release events (testing builds only).
    #[cfg(feature = "testing")]
    #[wasm_bindgen(js_name = "sendKeyboardStringSequence")]
    pub fn send_keyboard_string_sequence(&self, sequence: String) {
        let window_adapter =
            i_slint_core::window::WindowInner::from_pub(self.0.window()).window_adapter();
        i_slint_backend_testing::testing_backend::send_keyboard_string_sequence(
            &sequence.into(),
            &window_adapter,
        );
    }

    /// Presses all the given keys, then releases them in reverse order (testing builds only).
    #[cfg(feature = "testing")]
    #[wasm_bindgen(js_name = "sendKeyCombo")]
    pub fn send_key_combo(&self, keys: Vec<String>) {
        use i_slint_core::platform::WindowEvent;
        let window = self.0.window();
        for key in &keys {
            window.dispatch_event(WindowEvent::KeyPressed { text: key.into() });
        }
        for key in keys.iter().rev() {
            window.dispatch_event(WindowEvent::KeyReleased { text: key.into() });
        }
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
        let (ty, _) = self
            .0
            .definition()
            .properties_and_callbacks()
            .find_map(|(n, t)| if n == name { Some(t) } else { None })
            .ok_or_else(|| {
                JsValue::from(js_sys::Error::new(&format!(
                    "Property {name} not found in the component"
                )))
            })?;
        let val = js_to_value_typed(&value, &ty)?;
        let result =
            self.0.set_property(name, val).map_err(|e| JsValue::from_str(&format!("{e:?}")));
        wake_event_loop();
        result
    }

    /// Registers a callback handler.
    #[wasm_bindgen(js_name = "setCallback")]
    pub fn set_callback(&self, name: &str, callback: js_sys::Function) -> Result<(), JsValue> {
        let (ty, _) = self
            .0
            .definition()
            .properties_and_callbacks()
            .find_map(|(n, t)| if n == name { Some(t) } else { None })
            .ok_or_else(|| {
                JsValue::from(js_sys::Error::new(&format!(
                    "Callback {name} not found in the component"
                )))
            })?;
        let i_slint_compiler::langtype::Type::Callback(cb_type) = ty else {
            return Err(js_sys::Error::new(&format!("{name} is not a callback")).into());
        };
        self.0
            .set_callback(
                name,
                make_callback_handler(callback, cb_type.return_type.clone(), name.to_string()),
            )
            .map_err(|e| JsValue::from_str(&format!("{e:?}")))
    }

    /// Invokes a callback or function by name.
    #[wasm_bindgen]
    pub fn invoke(&self, name: &str, args: js_sys::Array) -> Result<JsValue, JsValue> {
        let (ty, _) = self
            .0
            .definition()
            .properties_and_callbacks()
            .find_map(|(n, t)| if n == name { Some(t) } else { None })
            .ok_or_else(|| {
                JsValue::from(js_sys::Error::new(&format!(
                    "Callback {name} not found in the component"
                )))
            })?;
        let values = invoke_args(name, &ty, args)?;
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
        let (ty, _) = self
            .0
            .definition()
            .global_properties_and_callbacks(global_name)
            .ok_or_else(|| {
                JsValue::from(js_sys::Error::new(&format!("Global {global_name} not found")))
            })?
            .find_map(|(n, t)| if n == name { Some(t) } else { None })
            .ok_or_else(|| {
                JsValue::from(js_sys::Error::new(&format!(
                    "Property {name} of global {global_name} not found in the component"
                )))
            })?;
        let val = js_to_value_typed(&value, &ty)?;
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
        let (ty, _) = self
            .0
            .definition()
            .global_properties_and_callbacks(global_name)
            .ok_or_else(|| {
                JsValue::from(js_sys::Error::new(&format!("Global {global_name} not found")))
            })?
            .find_map(|(n, t)| if n == name { Some(t) } else { None })
            .ok_or_else(|| {
                JsValue::from(js_sys::Error::new(&format!(
                    "Callback {name} of global {global_name} not found in the component"
                )))
            })?;
        let i_slint_compiler::langtype::Type::Callback(cb_type) = ty else {
            return Err(js_sys::Error::new(&format!("{name} is not a callback")).into());
        };
        self.0
            .set_global_callback(
                global_name,
                name,
                make_callback_handler(callback, cb_type.return_type.clone(), name.to_string()),
            )
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
        let (ty, _) = self
            .0
            .definition()
            .global_properties_and_callbacks(global_name)
            .and_then(|mut props| props.find_map(|(n, t)| if n == name { Some(t) } else { None }))
            .ok_or_else(|| {
                JsValue::from(js_sys::Error::new(&format!(
                    "Callback {name} not found in the global {global_name}"
                )))
            })?;
        let values = invoke_args(name, &ty, args)?;
        let result = self
            .0
            .invoke_global(global_name, name, &values)
            .map(|v| value_to_js(&v))
            .map_err(|e| JsValue::from_str(&format!("{e:?}")));
        wake_event_loop();
        result
    }

    /// Returns the {@link Window} associated with this component instance.
    #[wasm_bindgen]
    pub fn window(&self) -> Result<WrappedWindow, JsValue> {
        if !self.0.definition().is_window() {
            return Err(js_sys::Error::new(
                "this component is not windowed (for example because it inherits from SystemTrayIcon) and has no window",
            )
            .into());
        }
        Ok(WrappedWindow {
            inner: i_slint_core::window::WindowInner::from_pub(self.0.window()).window_adapter(),
        })
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

/// Build the Rust closure for `setCallback` / `setGlobalCallback`: convert
/// the arguments to JS, call the JS function, and convert the return value
/// back with the callback's declared return type (mirroring the Node.js
/// binding's handler, including its console errors).
fn make_callback_handler(
    callback: js_sys::Function,
    return_type: i_slint_compiler::langtype::Type,
    name: String,
) -> impl Fn(&[Value]) -> Value {
    use i_slint_compiler::langtype::Type;
    move |args| {
        let js_args: js_sys::Array = args.iter().map(|v| value_to_js(v)).collect();
        let result = match callback.apply(&JsValue::UNDEFINED, &js_args) {
            Ok(result) => result,
            Err(err) => {
                web_sys::console::error_2(
                    &format!("Invoking callback '{name}' failed:").into(),
                    &err,
                );
                return Value::Void;
            }
        };
        if matches!(return_type, Type::Void) {
            Value::Void
        } else if let Ok(value) = js_to_value_typed(&result, &return_type) {
            value
        } else {
            web_sys::console::error_1(
                &format!("cannot convert return type of callback {name}").into(),
            );
            slint_interpreter::default_value_for_type(&return_type)
        }
    }
}

/// Validate the argument count against the callback's declared signature and
/// convert the arguments with their declared types. The error messages match
/// the Node.js binding.
fn invoke_args(
    name: &str,
    ty: &i_slint_compiler::langtype::Type,
    args: js_sys::Array,
) -> Result<Vec<Value>, JsValue> {
    use i_slint_compiler::langtype::Type;
    let arg_types = match ty {
        Type::Callback(f) | Type::Function(f) => &f.args,
        _ => {
            return Err(
                js_sys::Error::new(&format!("{name} is not a callback or a function")).into()
            );
        }
    };
    if args.length() as usize != arg_types.len() {
        return Err(js_sys::Error::new(&format!(
            "{name} expect {} arguments, but {} where provided",
            arg_types.len(),
            args.length()
        ))
        .into());
    }
    args.iter().zip(arg_types).map(|(a, ty)| js_to_value_typed(&a, ty)).collect()
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

/// With the `testing` feature, the module installs the deterministic testing
/// backend (mocked time, synthetic input, no canvas) instead of the winit web
/// backend. This build flavor exists for the browser test driver
/// (tests/driver/browser); the regular published wasm module never enables it.
#[cfg(feature = "testing")]
#[wasm_bindgen(start)]
pub fn init() -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
    // Consistent results for OS-dependent behavior like dialog button order,
    // like the interpreter test driver.
    i_slint_core::OPERATING_SYSTEM_OVERRIDE
        .set(Some(i_slint_core::items::OperatingSystemType::Windows));
    i_slint_backend_testing::init_integration_test_with_mock_time();
    Ok(())
}

/// Advance the mocked animation/timer time (testing builds; no-op otherwise).
#[wasm_bindgen(js_name = "mockElapsedTime")]
pub fn mock_elapsed_time(_ms: f64) {
    #[cfg(feature = "testing")]
    i_slint_backend_testing::mock_elapsed_time(_ms as u64);
}

/// Returns the current mocked time in milliseconds (testing builds; 0 otherwise).
#[wasm_bindgen(js_name = "getMockedTime")]
pub fn get_mocked_time() -> f64 {
    #[cfg(feature = "testing")]
    return i_slint_backend_testing::get_mocked_time() as f64;
    #[cfg(not(feature = "testing"))]
    return 0.0;
}

#[cfg(not(feature = "testing"))]
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
