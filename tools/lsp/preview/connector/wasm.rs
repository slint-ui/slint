// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This wasm library can be loaded from JS to load and display the content of .slint files
#![cfg(target_arch = "wasm32")]

use crate::common;
use crate::preview::{self, connector, ui};

use slint_interpreter::ComponentHandle;

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const CALLBACK_FUNCTION_SECTION: &'static str = r#"
export type ResourceUrlMapperFunction = (url: string) => Promise<string | undefined>;
export type SignalLspFunction = (data: any) => void;
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "ResourceUrlMapperFunction")]
    pub type ResourceUrlMapperFunction;
    #[wasm_bindgen(typescript_type = "SignalLspFunction")]
    pub type SignalLspFunction;

    #[wasm_bindgen(typescript_type = "Promise<PreviewConnector>")]
    pub type PreviewConnectorPromise;
}

// We have conceptually two threads: The UI thread and the JS runtime, even though
// the WASM is strictly single threaded.
//
// So we use a thread local variable to transfer data between the two conceptual threads.
struct WasmCallbacks {
    lsp_notifier: SignalLspFunction,
    resource_url_mapper: ResourceUrlMapperFunction,
}
thread_local! {static WASM_CALLBACKS: RefCell<Option<WasmCallbacks>> = Default::default();}

#[wasm_bindgen(start)]
pub fn init_backend() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    // Initialize the winit backend when we're used in the browser's main thread.
    if web_sys::window().is_some() {
        let backend =
            i_slint_backend_winit::Backend::builder().with_spawn_event_loop(true).build().unwrap();
        i_slint_core::platform::set_platform(Box::new(backend))
            .map_err(|e| -> JsValue { format!("{e}").into() })?;
    }

    Ok(())
}

/// Register DOM event handlers on all instance and set up the event loop for that.
/// You can call this function only once.
#[wasm_bindgen]
pub fn run_event_loop() -> Result<(), JsValue> {
    slint_interpreter::run_event_loop().map_err(|e| -> JsValue { format!("{e}").into() })
}

#[wasm_bindgen]
pub struct PreviewConnector {}

#[wasm_bindgen]
impl PreviewConnector {
    #[wasm_bindgen]
    pub fn create(
        lsp_notifier: SignalLspFunction,
        resource_url_mapper: ResourceUrlMapperFunction,
        style: String,
        experimental: bool,
    ) -> Result<PreviewConnectorPromise, JsValue> {
        console_error_panic_hook::set_once();

        WASM_CALLBACKS.set(Some(WasmCallbacks { lsp_notifier, resource_url_mapper }));

        Ok(JsValue::from(js_sys::Promise::new(&mut move |resolve, reject| {
            let resolve = send_wrapper::SendWrapper::new(resolve);
            let reject_c = send_wrapper::SendWrapper::new(reject.clone());
            let style = style.clone();
            if let Err(e) = slint_interpreter::invoke_from_event_loop(move || {
                let to_lsp: Rc<dyn common::PreviewToLsp> = Rc::new(WasmPreviewToLsp::default());

                preview::PREVIEW_STATE.with(move |preview_state| {
                    if preview_state.borrow().ui.is_some() {
                        reject_c.take().call1(&JsValue::UNDEFINED,
                            &JsValue::from("PreviewConnector already set up.")).unwrap_throw();
                    } else {
                        match ui::create_ui(&to_lsp, &style, experimental) {
                            Ok(ui) => {
                                preview_state.borrow_mut().ui = Some(ui);
                                *preview_state.borrow().to_lsp.borrow_mut() = Some(to_lsp);

                                resolve.take().call1(&JsValue::UNDEFINED,
                                    &JsValue::from(Self { })).unwrap_throw()
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
    pub fn current_style(&self) -> JsValue {
        preview::get_current_style().into()
    }

    #[wasm_bindgen]
    pub fn show_ui(&self) -> Result<js_sys::Promise, JsValue> {
        invoke_from_event_loop_wrapped_in_promise(|instance| instance.show())
    }

    #[wasm_bindgen]
    pub fn process_lsp_to_preview_message(&self, value: JsValue) -> Result<(), JsValue> {
        let message = serde_wasm_bindgen::from_value(value)
            .map_err(|e| -> JsValue { format!("{e:?}").into() })?;
        i_slint_core::api::invoke_from_event_loop(move || {
            connector::lsp_to_preview(message);
        })
        .map_err(|e| -> JsValue { format!("{e:?}").into() })?;
        Ok(())
    }
}

fn invoke_from_event_loop_wrapped_in_promise(
    callback: impl FnOnce(&ui::PreviewUi) -> Result<(), slint_interpreter::PlatformError> + 'static,
) -> Result<js_sys::Promise, JsValue> {
    let callback = std::cell::RefCell::new(Some(callback));
    Ok(js_sys::Promise::new(&mut |resolve, reject| {
        preview::PREVIEW_STATE.with(|preview_state| {
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
                                &JsValue::from("Invocation on PreviewUi failed because instance was deleted too soon"),
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

pub fn resource_url_mapper() -> Option<i_slint_compiler::ResourceUrlMapper> {
    let callback = WASM_CALLBACKS.with_borrow(|callbacks| {
        callbacks.as_ref().map(|cb| js_sys::Function::from((cb.resource_url_mapper).clone()))
    })?;

    Some(Rc::new(move |url: &str| {
        let Some(promise) = callback.call1(&JsValue::UNDEFINED, &url.into()).ok() else {
            return Box::pin(std::future::ready(None));
        };
        let future = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise));
        Box::pin(async move { future.await.ok().and_then(|v| v.as_string()) })
    }))
}

pub struct WasmLspToPreview {
    server_notifier: crate::ServerNotifier,
}

impl WasmLspToPreview {
    pub fn new(server_notifier: crate::ServerNotifier) -> Self {
        Self { server_notifier }
    }
}

impl common::LspToPreview for WasmLspToPreview {
    fn send(&self, message: &common::LspToPreviewMessage) -> common::Result<()> {
        self.server_notifier.send_notification::<common::LspToPreviewMessage>(message.clone())
    }

    fn preview_target(&self) -> common::PreviewTarget {
        common::PreviewTarget::EmbeddedWasm
    }

    fn set_preview_target(&self, _: common::PreviewTarget) -> common::Result<()> {
        Err("Can not change the preview target".into())
    }
}

#[derive(Default)]
struct WasmPreviewToLsp {}

impl common::PreviewToLsp for WasmPreviewToLsp {
    fn send(&self, message: &common::PreviewToLspMessage) -> common::Result<()> {
        WASM_CALLBACKS.with_borrow(|callbacks| {
            let notifier = js_sys::Function::from(
                (callbacks.as_ref().expect("Callbacks were set up earlier").lsp_notifier).clone(),
            );
            let value = serde_wasm_bindgen::to_value(&message)?;
            notifier
                .call1(&JsValue::UNDEFINED, &value)
                .map_err(|_| "Failed to send message to LSP".to_string())?;
            Ok(())
        })
    }
}
