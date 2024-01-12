// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! This wasm library can be loaded from JS to load and display the content of .slint files
#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, future::Future, pin::Pin, rc::Rc, rc::Weak};

use wasm_bindgen::prelude::*;

use i_slint_compiler::object_tree::{ElementRc, ElementWeak};
use i_slint_core::lengths::LogicalRect;
use lsp_types::Url;
use slint::VecModel;
use slint_interpreter::{highlight::ComponentPositions, ComponentHandle, ComponentInstance};

use crate::lsp_ext::Health;

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

/// Register DOM event handlers on all instance and set up the event loop for that.
/// You can call this function only once. It will throw an exception but that is safe
/// to ignore.
#[wasm_bindgen]
pub fn run_event_loop() -> Result<(), JsValue> {
    slint_interpreter::spawn_event_loop().map_err(|e| -> JsValue { format!("{e}").into() })
}

#[derive(Default)]
struct PreviewState {
    ui: Option<super::ui::PreviewUi>,
    handle: Rc<RefCell<Option<slint_interpreter::ComponentInstance>>>,
    lsp_notifier: Option<SignalLspFunction>,
    resource_url_mapper: Option<ResourceUrlMapperFunction>,
    selected_element: Option<ElementWeak>,
}
thread_local! {static PREVIEW_STATE: std::cell::RefCell<PreviewState> = Default::default();}

pub fn selected_element() -> Option<ElementRc> {
    PREVIEW_STATE.with(move |preview_state| {
        let preview_state = preview_state.borrow();
        let Some(weak) = &preview_state.selected_element else {
            return None;
        };
        Weak::upgrade(&weak)
    })
}

pub fn component_instance() -> Option<ComponentInstance> {
    PREVIEW_STATE.with(move |preview_state| {
        preview_state.borrow().handle.borrow().as_ref().map(|ci| ci.clone_strong())
    })
}

pub fn set_selected_element(
    element_position: Option<(&ElementRc, LogicalRect)>,
    positions: ComponentPositions,
) {
    PREVIEW_STATE.with(move |preview_state| {
        let mut preview_state = preview_state.borrow_mut();
        if let Some((e, _)) = element_position.as_ref() {
            preview_state.selected_element = Some(Rc::downgrade(e));
        } else {
            preview_state.selected_element = None;
        }

        super::set_selections(preview_state.ui.as_ref(), element_position, positions);
    })
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
    ) -> Result<PreviewConnectorPromise, JsValue> {
        console_error_panic_hook::set_once();

        PREVIEW_STATE.with(|preview_state| {
            preview_state.borrow_mut().lsp_notifier = Some(lsp_notifier);
            preview_state.borrow_mut().resource_url_mapper = Some(resource_url_mapper);
        });

        Ok(JsValue::from(js_sys::Promise::new(&mut move |resolve, reject| {
            let resolve = send_wrapper::SendWrapper::new(resolve);
            let reject_c = send_wrapper::SendWrapper::new(reject.clone());
            let style = style.clone();
            if let Err(e) = slint_interpreter::invoke_from_event_loop(move || {
                PREVIEW_STATE.with(move |preview_state| {
                    if preview_state.borrow().ui.is_some() {
                        reject_c.take().call1(&JsValue::UNDEFINED,
                            &JsValue::from("PreviewConnector already set up.")).unwrap_throw();
                    } else {
                        match super::ui::create_ui(style) {
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
        crate::preview::wasm::get_current_style().into()
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
    pub fn process_lsp_to_preview_message(&self, value: JsValue) -> Result<(), JsValue> {
        use crate::common::LspToPreviewMessage as M;

        let message: M = serde_wasm_bindgen::from_value(value)
            .map_err(|e| -> JsValue { format!("{e:?}").into() })?;
        match message {
            M::SetContents { url, contents } => {
                super::set_contents(&url, contents);
                Ok(())
            }
            M::SetConfiguration { config } => {
                super::config_changed(config);
                Ok(())
            }
            M::ShowPreview(pc) => {
                super::load_preview(pc);
                Ok(())
            }
            M::HighlightFromEditor { url, offset } => {
                super::highlight(url, offset);
                Ok(())
            }
            M::KnownComponents { url, components } => {
                super::known_components(&url, components);
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
        PREVIEW_STATE.with(|preview_state| {
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

pub fn run_in_ui_thread<F: Future<Output = ()> + 'static>(
    create_future: impl Send + FnOnce() -> F + 'static,
) {
    i_slint_core::future::spawn_local(create_future()).unwrap();
}

pub fn resource_url_mapper(
) -> Option<Rc<dyn Fn(&str) -> Pin<Box<dyn Future<Output = Option<String>>>>>> {
    let callback = PREVIEW_STATE.with(|preview_state| {
        preview_state
            .borrow()
            .resource_url_mapper
            .as_ref()
            .map(|rum| js_sys::Function::from((*rum).clone()))
    })?;

    Some(Rc::new(move |url: &str| {
        let Some(promise) = callback.call1(&JsValue::UNDEFINED, &url.into()).ok() else {
            return Box::pin(std::future::ready(None));
        };
        let future = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise));
        Box::pin(async move { future.await.ok().and_then(|v| v.as_string()) })
    }))
}

pub fn send_message_to_lsp(message: crate::common::PreviewToLspMessage) {
    PREVIEW_STATE.with(|preview_state| {
        if let Some(callback) = &preview_state.borrow().lsp_notifier {
            let callback = js_sys::Function::from((*callback).clone());
            let value = serde_wasm_bindgen::to_value(&message).unwrap();
            let _ = callback.call1(&JsValue::UNDEFINED, &value);
        }
    })
}

pub fn set_show_preview_ui(show_preview_ui: bool) {
    PREVIEW_STATE.with(move |preview_state| {
        let preview_state = preview_state.borrow_mut();
        if let Some(ui) = &preview_state.ui {
            ui.set_show_preview_ui(show_preview_ui)
        }
    });
}

pub fn set_current_style(style: String) {
    PREVIEW_STATE.with(move |preview_state| {
        let preview_state = preview_state.borrow_mut();
        if let Some(ui) = &preview_state.ui {
            ui.set_current_style(style.into())
        }
    });
}

pub fn get_current_style() -> String {
    PREVIEW_STATE.with(|preview_state| {
        let preview_state = preview_state.borrow();
        if let Some(ui) = &preview_state.ui {
            ui.get_current_style().as_str().to_string()
        } else {
            String::new()
        }
    })
}

pub fn set_status_text(text: &str) {
    let text = text.to_string();

    i_slint_core::api::invoke_from_event_loop(move || {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow_mut();
            if let Some(ui) = &preview_state.ui {
                ui.set_status_text(text.into());
            }
        });
    })
    .unwrap();
}

pub fn set_diagnostics(diagnostics: &[slint_interpreter::Diagnostic]) {
    let data = crate::preview::ui::convert_diagnostics(diagnostics);

    i_slint_core::api::invoke_from_event_loop(move || {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow_mut();
            if let Some(ui) = &preview_state.ui {
                let model = VecModel::from(data);
                ui.set_diagnostics(Rc::new(model).into());
            }
        });
    })
    .unwrap();
}

pub fn send_status(message: &str, health: Health) {
    send_message_to_lsp(crate::common::PreviewToLspMessage::Status {
        message: message.to_string(),
        health,
    });
}

pub fn notify_diagnostics(diagnostics: &[slint_interpreter::Diagnostic]) -> Option<()> {
    set_diagnostics(diagnostics);
    let diags = crate::preview::convert_diagnostics(diagnostics);

    for (uri, diagnostics) in diags {
        send_message_to_lsp(crate::common::PreviewToLspMessage::Diagnostics { uri, diagnostics });
    }
    Some(())
}

pub fn ask_editor_to_show_document(file: String, selection: lsp_types::Range) {
    send_message_to_lsp(crate::common::PreviewToLspMessage::ShowDocument { file, selection })
}

pub fn update_preview_area(compiled: Option<slint_interpreter::ComponentDefinition>) {
    PREVIEW_STATE.with(|preview_state| {
        if let Some(compiled) = compiled {
            let preview_state = preview_state.borrow_mut();

            let shared_handle = preview_state.handle.clone();

            let ui = preview_state.ui.as_ref().unwrap();
            super::set_preview_factory(
                ui,
                compiled,
                Box::new(move |instance| {
                    shared_handle.replace(Some(instance));
                }),
            );
            super::reset_selections(ui);
        }
    })
}

pub fn update_highlight(url: Option<Url>, offset: u32) {
    slint::invoke_from_event_loop(move || {
        let handle = PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow();
            let result = preview_state.handle.borrow().as_ref().map(|h| h.clone_strong());
            result
        });

        if let Some(handle) = handle {
            let path = url.as_ref().map(|u| u.to_string().into()).unwrap_or_default();
            let element_positions = handle.component_positions(path, offset);
            set_selected_element(None, element_positions);
        }
    })
    .unwrap();
}
