// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This wasm library can be loaded from JS to load and display the content of .slint files
#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;

use crate::common;
use crate::wasm_prelude::*;
use slint_interpreter::ComponentHandle;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
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
                super::PREVIEW_STATE.with(move |preview_state| {
                    if preview_state.borrow().ui.is_some() {
                        reject_c.take().call1(&JsValue::UNDEFINED,
                            &JsValue::from("PreviewConnector already set up.")).unwrap_throw();
                    } else {
                        match super::ui::create_ui(style, experimental) {
                            Ok(ui) => {
                                preview_state.borrow_mut().ui = Some(ui);
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
        super::get_current_style().into()
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
            lsp_to_preview_message(message);
        })
        .map_err(|e| -> JsValue { format!("{e:?}").into() })?;
        Ok(())
    }
}

fn invoke_from_event_loop_wrapped_in_promise(
    callback: impl FnOnce(&super::ui::PreviewUi) -> Result<(), slint_interpreter::PlatformError>
        + 'static,
) -> Result<js_sys::Promise, JsValue> {
    let callback = std::cell::RefCell::new(Some(callback));
    Ok(js_sys::Promise::new(&mut |resolve, reject| {
        super::PREVIEW_STATE.with(|preview_state| {
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

pub fn resource_url_mapper(
) -> Option<Rc<dyn Fn(&str) -> Pin<Box<dyn Future<Output = Option<String>>>>>> {
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

pub fn send_message_to_lsp(message: common::PreviewToLspMessage) {
    WASM_CALLBACKS.with_borrow(|callbacks| {
        if let Some(callbacks) = &callbacks {
            let notifier = js_sys::Function::from((callbacks.lsp_notifier).clone());
            let value = serde_wasm_bindgen::to_value(&message).unwrap();
            let _ = notifier.call1(&JsValue::UNDEFINED, &value);
        }
    })
}

pub fn notify_diagnostics(
    diagnostics: HashMap<lsp_types::Url, (common::SourceFileVersion, Vec<lsp_types::Diagnostic>)>,
) -> Option<()> {
    for (uri, (version, diagnostics)) in diagnostics {
        send_message_to_lsp(common::PreviewToLspMessage::Diagnostics { uri, version, diagnostics });
    }
    Some(())
}

pub fn ask_editor_to_show_document(file: &str, selection: lsp_types::Range, take_focus: bool) {
    let Ok(file) = lsp_types::Url::from_file_path(file) else { return };
    send_message_to_lsp(common::PreviewToLspMessage::ShowDocument { file, selection, take_focus });
}

fn lsp_to_preview_message(message: common::LspToPreviewMessage) {
    super::lsp_to_preview_message_impl(message);
}
