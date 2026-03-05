// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(target_arch = "wasm32")]
#![allow(clippy::await_holding_refcell_ref)]

pub mod common;
mod fmt;
mod language;
#[cfg(feature = "preview-engine")]
mod preview;
mod request_handler;
pub mod util;

use common::{DocumentCache, LspToPreview, LspToPreviewMessage, Result, VersionedUrl};
use js_sys::Function;
pub use language::Context;
use lsp_types::Url;
pub use request_handler::RequestHandler;
use std::any::{Any, TypeId};
use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::future::Future;
use std::io::ErrorKind;
use std::rc::{Rc, Weak};
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use lsp_protocol::wasm_prelude::*;

use crate::util::LocalThreadWrapper;
#[cfg(not(feature = "preview-engine"))]
use crate::util::LocalThreadWrapper;

type JsResult<T> = std::result::Result<T, JsError>;

#[derive(Clone)]
pub struct ServerNotifier {
    send_notification: Function,
    send_request: Function,
    events: HashMap<TypeId, Box<dyn Fn(&mut Context, &dyn Any) -> Result<()>>>,
    context: Weak<RefCell<Context>>,
}

impl ServerNotifier {
    pub fn send_notification<N: lsp_types::notification::Notification>(
        &self,
        params: N::Params,
    ) -> Result<()> {
        self.send_notification
            .call2(&JsValue::UNDEFINED, &N::METHOD.into(), &to_value(&params)?)
            .map_err(|x| format!("Error calling send_notification: {x:?}"))?;
        Ok(())
    }

    pub fn send_request<T: lsp_types::request::Request>(
        &self,
        request: T::Params,
    ) -> impl Future<Output = Result<T::Result>> {
        let promise = self
            .send_request
            .call2(&JsValue::UNDEFINED, &T::METHOD.into(), &to_value(&request)?)
            .map_err(|x| format!("Error calling send_request: {x:?}"));
        let future = promise
            .map(|promise| wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise)));
        async move {
            future?.await.map_err(|e| format!("{e:?}").into()).and_then(|v| {
                serde_wasm_bindgen::from_value(v).map_err(|e| format!("{e:?}").into())
            })
        }
    }

    pub fn send_event<E: 'static>(&self, event: E) -> Result<()> {
        if let (Some(handler), Some(context)) =
            (self.events.get(&TypeId::of::<E>()), self.context.upgrade())
        {
            handler(&mut context.borrow_mut(), &event as &dyn Any)
        } else {
            anyhow::bail!("Event handler not registered")
        }
    }

    pub fn register_event<E: 'static>(&mut self, handler: impl Fn(&mut Context, &E) -> Result<()>) {
        self.events.insert(
            TypeId::of::<E>(),
            Box::new(|context, event| {
                if let Some(event) = event.downcast_ref() {
                    handler(context, event)
                } else {
                    unreachable!()
                }
            }),
        );
    }
}

impl RequestHandler {
    async fn handle_request(
        &self,
        method: String,
        params: JsValue,
        ctx: &mut Context,
    ) -> Result<JsValue> {
        if let Some(f) = self.0.get(&method.as_str()) {
            let param = serde_wasm_bindgen::from_value(params)
                .map_err(|x| format!("invalid param to handle_request: {x:?}"))?;
            let r = f(ctx, param).await.map_err(|e| e.message)?;
            to_value(&r).map_err(|e| e.to_string().into())
        } else {
            Err("Cannot handle request".into())
        }
    }
}

#[derive(Default)]
struct ReentryGuard {
    locked: bool,
    waker: Vec<std::task::Waker>,
}

impl ReentryGuard {
    pub async fn lock(this: Rc<RefCell<Self>>) -> ReentryGuardLock {
        struct ReentryGuardLocker(Rc<RefCell<ReentryGuard>>);

        impl std::future::Future for ReentryGuardLocker {
            type Output = ReentryGuardLock;
            fn poll(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Self::Output> {
                let mut s = self.0.borrow_mut();
                if s.locked {
                    s.waker.push(cx.waker().clone());
                    std::task::Poll::Pending
                } else {
                    s.locked = true;
                    std::task::Poll::Ready(ReentryGuardLock(self.0.clone()))
                }
            }
        }
        ReentryGuardLocker(this).await
    }
}

struct ReentryGuardLock(Rc<RefCell<ReentryGuard>>);

impl Drop for ReentryGuardLock {
    fn drop(&mut self) {
        let mut s = self.0.borrow_mut();
        s.locked = false;
        let wakers = std::mem::take(&mut s.waker);
        drop(s);
        for w in wakers {
            w.wake()
        }
    }
}

#[wasm_bindgen(typescript_custom_section)]
const IMPORT_CALLBACK_FUNCTION_SECTION: &'static str = r#"
type ImportCallbackFunction = (url: string) => Promise<string>;
type SendRequestFunction = (method: string, r: any) => Promise<any>;
type HighlightInPreviewFunction = (file: string, offset: number) => void;
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "ImportCallbackFunction")]
    pub type ImportCallbackFunction;

    #[wasm_bindgen(typescript_type = "SendRequestFunction")]
    pub type SendRequestFunction;

    #[wasm_bindgen(typescript_type = "HighlightInPreviewFunction")]
    pub type HighlightInPreviewFunction;

    // Make console.log available:
    #[allow(unused)]
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub struct SlintServer {
    ctx: Rc<RefCell<Context>>,
    reentry_guard: Rc<RefCell<ReentryGuard>>,
    rh: Rc<RequestHandler>,
}

#[wasm_bindgen]
pub fn create(
    init_param: JsValue,
    send_notification: Function,
    send_request: SendRequestFunction,
    load_file: ImportCallbackFunction,
) -> JsResult<SlintServer> {
    console_error_panic_hook::set_once();

    let send_request = Function::from(send_request.clone());
    let init_param = serde_wasm_bindgen::from_value(init_param)?;
    let mut rh = RequestHandler::default();
    let reentry_guard = Rc::new(RefCell::new(ReentryGuard::default()));
    let ctx = Rc::new_cyclic(move |context| {
        let server_notifier = ServerNotifier {
            send_notification,
            send_request,
            events: HashMap::new(),
            context: context.clone(),
        };

        let mut compiler_config = crate::common::document_cache::CompilerConfiguration::default();

        #[cfg(not(feature = "preview-engine"))]
        let to_preview = LocalThreadWrapper::new_local(
            preview::connector::SwitchableLspToPreview::with_one(common::DummyLspToPreview {}),
        );
        #[cfg(feature = "preview-engine")]
        let to_preview =
            LocalThreadWrapper::new(preview::connector::SwitchableLspToPreview::with_one(
                preview::connector::WasmLspToPreview::new(server_notifier.clone()),
            ));

        let to_preview_clone = to_preview.clone();
        compiler_config.open_import_callback = Some(Rc::new(move |path| {
            let load_file = Function::from(load_file.clone());
            let to_preview = to_preview_clone.clone();
            Box::pin(async move {
                let contents = self::load_file(path.clone(), &load_file).await;
                let Ok(url) = Url::from_file_path(&path) else {
                    return Some(contents.map(|c| (None, c)));
                };
                if let Ok(contents) = &contents {
                    to_preview.send(&LspToPreviewMessage::SetContents {
                        url: VersionedUrl::new(url, None),
                        contents: contents.clone().into(),
                    });
                }
                Some(contents.map(|c| (None, c)))
            })
        }));
        let document_cache = LocalThreadWrapper::new_local(DocumentCache::new(compiler_config));

        language::register_request_handlers(&mut rh);
        RefCell::new(Context {
            document_cache,
            preview_config: Default::default(),
            init_param,
            server_notifier,
            to_show: Default::default(),
            open_urls: Default::default(),
            to_preview,
            pending_recompile: Default::default(),
            recompile_timer: None,
        })
    });

    Ok(SlintServer { ctx, reentry_guard, rh: Rc::new(rh) })
}

fn forward_workspace_edit(
    server_notifier: ServerNotifier,
    label: Option<String>,
    edit: Result<lsp_types::WorkspaceEdit>,
) {
    let Ok(edit) = edit else {
        return;
    };

    wasm_bindgen_futures::spawn_local(async move {
        let fut = server_notifier.send_request::<lsp_types::request::ApplyWorkspaceEdit>(
            lsp_types::ApplyWorkspaceEditParams { label, edit },
        );
        if let Ok(fut) = fut {
            // We ignore errors: If the LSP can not be reached, then all is lost
            // anyway. The other thing that might go wrong is that our Workspace Edit
            // refers to some outdated text. In that case the update is most likely
            // in flight already and will cause the preview to re-render, which also
            // invalidates all our state
            let _ = fut.await;
        }
    });
}

#[wasm_bindgen]
impl SlintServer {
    #[cfg(all(feature = "preview-engine", feature = "preview-external"))]
    #[wasm_bindgen]
    pub async fn process_preview_to_lsp_message(
        &self,
        value: JsValue,
    ) -> std::result::Result<(), JsValue> {
        use crate::common::PreviewToLspMessage as M;

        let guard = self.reentry_guard.clone();
        let _lock = ReentryGuard::lock(guard).await;

        let Ok(message) = serde_wasm_bindgen::from_value::<M>(value) else {
            return Err(JsValue::from("Failed to convert value to PreviewToLspMessage"));
        };

        match message {
            M::Diagnostics { diagnostics, version, uri } => {
                crate::common::lsp_to_editor::notify_lsp_diagnostics(
                    &self.ctx.borrow().server_notifier,
                    uri,
                    version,
                    diagnostics,
                );
            }
            M::ShowDocument { file, selection, .. } => {
                let sn = self.ctx.server_notifier.borrow().clone();
                wasm_bindgen_futures::spawn_local(async move {
                    crate::common::lsp_to_editor::send_show_document_to_editor(
                        sn, file, selection, true,
                    )
                    .await
                });
            }
            M::PreviewTypeChanged { is_external: _ } => {
                // Nothing to do!
            }
            M::RequestState { .. } => {
                crate::language::send_state_to_preview(&self.ctx.borrow());
            }
            M::SendWorkspaceEdit { label, edit } => {
                forward_workspace_edit(self.ctx.borrow().server_notifier.clone(), label, Ok(edit));
            }
            M::SendShowMessage { message } => {
                let _ = self
                    .ctx
                    .borrow()
                    .server_notifier
                    .send_notification::<lsp_types::notification::ShowMessage>(message);
            }
            M::TelemetryEvent(object) => {
                let _ = self
                    .ctx
                    .borrow()
                    .server_notifier
                    .send_notification::<lsp_types::notification::TelemetryEvent>(
                        lsp_types::OneOf::Left(object),
                    );
            }
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub fn server_initialize_result(&self, cap: JsValue) -> JsResult<JsValue> {
        Ok(to_value(&language::server_initialize_result(&serde_wasm_bindgen::from_value(cap)?))?)
    }

    #[wasm_bindgen]
    pub async fn startup_lsp(&self) -> js_sys::Promise {
        let ctx = self.ctx.borrow();
        let params = ctx.init_param.clone();
        let server_notifier = ctx.server_notifier.clone();
        let document_cache = self.ctx.document_cache.clone();
        let guard = self.reentry_guard.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            let _lock = ReentryGuard::lock(guard).await;
            if let Some(config) = language::startup_lsp(&params, &server_notifier, document_cache)
                .await
                .map_err(|e| JsError::new(&e.to_string()))?
            {
                todo!()
            }
            Ok(JsValue::UNDEFINED)
        })
    }

    #[wasm_bindgen]
    pub fn trigger_file_watcher(&mut self, url: JsValue, typ: JsValue) -> js_sys::Promise {
        let guard = self.reentry_guard.clone();

        let url: lsp_types::Url = serde_wasm_bindgen::from_value(url)?;
        let typ: lsp_types::FileChangeType = serde_wasm_bindgen::from_value(typ)?;
        let future = language::trigger_file_watcher(&mut self.ctx.borrow_mut(), url, typ);
        wasm_bindgen_futures::future_to_promise(async move {
            let _lock = ReentryGuard::lock(guard).await;
            future.await.map_err(|e| JsError::new(&e.to_string()))?;
            Ok(JsValue::UNDEFINED)
        })
    }

    #[wasm_bindgen]
    pub fn open_document(
        &mut self,
        content: String,
        uri: JsValue,
        version: i32,
    ) -> js_sys::Promise {
        let guard = self.reentry_guard.clone();
        let uri: lsp_types::Url = serde_wasm_bindgen::from_value(uri)?;
        let future = language::open_document(
            &mut self.ctx.borrow_mut(),
            content,
            uri.clone(),
            Some(version),
        );
        wasm_bindgen_futures::future_to_promise(async move {
            let _lock = ReentryGuard::lock(guard).await;
            future.await.map_err(|e| JsError::new(&e.to_string()))?;
            Ok(JsValue::UNDEFINED)
        })
    }

    #[wasm_bindgen]
    pub fn load_document(
        &mut self,
        content: String,
        uri: JsValue,
        version: i32,
    ) -> js_sys::Promise {
        let guard = self.reentry_guard.clone();
        let uri: lsp_types::Url = serde_wasm_bindgen::from_value(uri)?;
        let future = language::load_document(
            &mut self.ctx.borrow_mut(),
            content,
            uri.clone(),
            Some(version),
        );
        wasm_bindgen_futures::future_to_promise(async move {
            let _lock = ReentryGuard::lock(guard).await;
            future.await.map_err(|e| JsError::new(&e.to_string()))?;
            Ok(JsValue::UNDEFINED)
        })
    }

    #[wasm_bindgen]
    pub fn close_document(&mut self, uri: JsValue) -> js_sys::Promise {
        let guard = self.reentry_guard.clone();
        let uri: lsp_types::Url = serde_wasm_bindgen::from_value(uri)?;
        let future = language::close_document(&mut self.ctx.borrow_mut(), uri);
        wasm_bindgen_futures::future_to_promise(async move {
            let _lock = ReentryGuard::lock(guard).await;
            future.await.map_err(|e| JsError::new(&e.to_string()))?;
            Ok(JsValue::UNDEFINED)
        })
    }

    #[wasm_bindgen]
    pub fn handle_request(&self, _id: JsValue, method: String, params: JsValue) -> js_sys::Promise {
        let guard = self.reentry_guard.clone();
        let ctx = self.ctx.clone();
        let rh = self.rh.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            let _lock = ReentryGuard::lock(guard).await;
            rh.handle_request(method, params, &mut ctx.borrow_mut())
                .await
                .map_err(|e| JsError::new(&e.to_string()).into())
        })
    }

    #[wasm_bindgen]
    pub async fn reload_config(&self) -> JsResult<()> {
        let guard = self.reentry_guard.clone();
        let _lock = ReentryGuard::lock(guard).await;
        let ctx = self.ctx.borrow();
        language::load_configuration(ctx.server_notifier.clone(), ctx.document_cache.clone())
            .await
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

async fn load_file(path: String, load_file: &Function) -> std::io::Result<String> {
    let string_promise = load_file
        .call1(&JsValue::UNDEFINED, &path.into())
        .map_err(|x| std::io::Error::new(ErrorKind::Other, format!("{x:?}")))?;
    let string_future = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(string_promise));
    let js_value =
        string_future.await.map_err(|e| std::io::Error::new(ErrorKind::Other, format!("{e:?}")))?;
    return Ok(js_value.as_string().unwrap_or_default());
}

// Use a JSON friendly representation to avoid using ES maps instead of JS objects.
fn to_value<T: serde::Serialize + ?Sized>(
    value: &T,
) -> std::result::Result<wasm_bindgen::JsValue, serde_wasm_bindgen::Error> {
    value.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
}
