// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! This wasm library can be loaded from JS to load and display the content of .slint files
#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, path::PathBuf, rc::Rc};

use wasm_bindgen::prelude::*;

use slint_interpreter::ComponentHandle;

use crate::{common::PreviewComponent, lsp_ext::Health};

#[wasm_bindgen(typescript_custom_section)]
const CALLBACK_FUNCTION_SECTION: &'static str = r#"
type SignalLspFunction = (data: any) => void;
"#;

#[wasm_bindgen]
extern "C" {
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
    slint::run_event_loop().map_err(|e| -> JsValue { format!("{e}").into() })
}

#[derive(Default)]
struct PreviewState {
    ui: Option<super::ui::PreviewUi>,
    handle: Rc<RefCell<Option<slint_interpreter::ComponentInstance>>>,
    lsp_notifier: Option<SignalLspFunction>,
}
thread_local! {static PREVIEW_STATE: std::cell::RefCell<PreviewState> = Default::default();}

#[wasm_bindgen]
pub struct PreviewConnector {}

#[wasm_bindgen]
impl PreviewConnector {
    #[wasm_bindgen]
    pub fn create(lsp_notifier: SignalLspFunction) -> Result<PreviewConnectorPromise, JsValue> {
        console_error_panic_hook::set_once();

        PREVIEW_STATE.with(|preview_state| {
            preview_state.borrow_mut().lsp_notifier = Some(lsp_notifier);
        });

        Ok(JsValue::from(js_sys::Promise::new(&mut move |resolve, reject| {
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
    pub fn show_ui(&self, width: u32, height: u32) -> Result<js_sys::Promise, JsValue> {
        {
            let mut cache = super::CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
            cache.ui_is_visible = true;
        }
        invoke_from_event_loop_wrapped_in_promise(move |instance| {
            instance.show()?;
            instance
                .window()
                .set_size(slint::LogicalSize { width: width as f32, height: height as f32 });
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn resize_ui(&self, width: u32, height: u32) -> Result<js_sys::Promise, JsValue> {
        {
            let cache = super::CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
            if !cache.ui_is_visible {
                return Ok(js_sys::Promise::resolve(&JsValue::UNDEFINED));
            }
        }
        invoke_from_event_loop_wrapped_in_promise(move |instance| {
            instance
                .window()
                .set_size(slint::LogicalSize { width: width as f32, height: height as f32 });
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn process_lsp_to_preview_message(&self, value: JsValue) -> Result<(), JsValue> {
        use crate::common::LspToPreviewMessage as M;

        let message: M = serde_wasm_bindgen::from_value(value)
            .map_err(|e| -> JsValue { format!("{e:?}").into() })?;
        match message {
            M::SetContents { path, contents } => {
                super::set_contents(&PathBuf::from(&path), contents);
                Ok(())
            }
            M::SetConfiguration { style, include_paths } => {
                let ip: Vec<PathBuf> = include_paths.iter().map(PathBuf::from).collect();
                super::config_changed(&style, &ip);
                Ok(())
            }
            M::ShowPreview { path, component, style, include_paths } => {
                let pc = PreviewComponent {
                    path: PathBuf::from(path),
                    component,
                    style,
                    include_paths: include_paths.iter().map(PathBuf::from).collect(),
                };
                load_preview(pc);
                Ok(())
            }
            M::HighlightFromEditor { path, offset } => {
                super::highlight(&path.map(PathBuf::from), offset);
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

pub fn send_message_to_lsp(message: crate::common::PreviewToLspMessage) {
    PREVIEW_STATE.with(|preview_state| {
        if let Some(callback) = &preview_state.borrow().lsp_notifier {
            let callback = js_sys::Function::from((*callback).clone());
            let value = serde_wasm_bindgen::to_value(&message).unwrap();
            let _ = callback.call1(&JsValue::UNDEFINED, &value);
        }
    })
}

pub fn send_status(message: &str, health: Health) {
    send_message_to_lsp(crate::common::PreviewToLspMessage::Status {
        message: message.to_string(),
        health,
    });
}

pub fn notify_diagnostics(diagnostics: &[slint_interpreter::Diagnostic]) -> Option<()> {
    let diags = crate::preview::convert_diagnostics(diagnostics);

    for (uri, diagnostics) in diags {
        send_message_to_lsp(crate::common::PreviewToLspMessage::Diagnostics { uri, diagnostics });
    }
    Some(())
}

pub fn ask_editor_to_show_document(
    file: &str,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
) {
    send_message_to_lsp(crate::common::PreviewToLspMessage::ShowDocument {
        file: file.to_string(),
        start_line,
        start_column,
        end_line,
        end_column,
    });
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
                handle.highlight(path.to_path_buf(), offset);
            }
        })
    })
    .unwrap();
}
