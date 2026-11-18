// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::RefCell;
use std::rc::Rc;

use slint_interpreter::ComponentHandle;
use wasm_bindgen::prelude::*;

use crate::{InstancePromise, WrappedCompiledComp};

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
