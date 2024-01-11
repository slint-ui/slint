// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#![cfg(target_arch = "wasm32")]

mod common;
mod language;
pub mod lsp_ext;
#[cfg(feature = "preview-engine")]
mod preview;
pub mod util;

use common::{ComponentInformation, PreviewApi, Result, VersionedUrl};
use i_slint_compiler::{pathutils::to_url, CompilerConfiguration};
use js_sys::Function;
pub use language::{Context, DocumentCache, RequestHandler};
use lsp_types::Url;
use serde::Serialize;
use std::cell::RefCell;
use std::future::Future;
use std::io::ErrorKind;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

type JsResult<T> = std::result::Result<T, JsError>;

pub mod wasm_prelude {
    use std::path::{Path, PathBuf};

    /// lsp_url doesn't have method to convert to and from PathBuf for wasm, so just make some
    pub trait UrlWasm {
        fn to_file_path(&self) -> Result<PathBuf, ()>;
        fn from_file_path<P: AsRef<Path>>(path: P) -> Result<lsp_types::Url, ()>;
    }
    impl UrlWasm for lsp_types::Url {
        fn to_file_path(&self) -> Result<PathBuf, ()> {
            Ok(self.to_string().into())
        }
        fn from_file_path<P: AsRef<Path>>(path: P) -> Result<Self, ()> {
            Self::parse(path.as_ref().to_str().ok_or(())?).map_err(|_| ())
        }
    }
}

struct Previewer {
    server_notifier: ServerNotifier,
    to_show: RefCell<Option<common::PreviewComponent>>,
}

impl PreviewApi for Previewer {
    fn set_use_external_previewer(&self, _use_external: bool) {
        // The WASM LSP always needs to use the WASM preview!
    }

    fn set_contents(&self, url: &VersionedUrl, contents: &str) {
        #[cfg(feature = "preview-external")]
        let _ = self.server_notifier.send_notification(
            "slint/lsp_to_preview".to_string(),
            crate::common::LspToPreviewMessage::SetContents {
                url: url.clone(),
                contents: contents.to_string(),
            },
        );
    }

    fn load_preview(&self, component: common::PreviewComponent) {
        self.to_show.replace(Some(component.clone()));

        #[cfg(feature = "preview-external")]
        let _ = self.server_notifier.send_notification(
            "slint/lsp_to_preview".to_string(),
            crate::common::LspToPreviewMessage::ShowPreview(component),
        );
    }

    fn config_changed(&self, config: common::PreviewConfig) {
        #[cfg(feature = "preview-external")]
        let _ = self.server_notifier.send_notification(
            "slint/lsp_to_preview".to_string(),
            crate::common::LspToPreviewMessage::SetConfiguration { config },
        );
    }

    fn highlight(&self, url: Option<Url>, offset: u32) -> Result<()> {
        #[cfg(feature = "preview-external")]
        self.server_notifier.send_notification(
            "slint/lsp_to_preview".to_string(),
            crate::common::LspToPreviewMessage::HighlightFromEditor { url, offset },
        )
    }

    fn current_component(&self) -> Option<crate::common::PreviewComponent> {
        self.to_show.borrow().clone()
    }

    fn report_known_components(
        &self,
        _url: Option<VersionedUrl>,
        _components: Vec<ComponentInformation>,
    ) {
        #[cfg(feature = "preview-external")]
        {
            let _ = self.server_notifier.send_notification(
                "slint/lsp_to_preview".to_string(),
                crate::common::LspToPreviewMessage::KnownComponents {
                    url: _url,
                    components: _components,
                },
            );
        }
    }
}

#[derive(Clone)]
pub struct ServerNotifier {
    send_notification: Function,
    send_request: Function,
}

impl ServerNotifier {
    pub fn send_notification(&self, method: String, params: impl Serialize) -> Result<()> {
        self.send_notification
            .call2(&JsValue::UNDEFINED, &method.into(), &to_value(&params)?)
            .map_err(|x| format!("Error calling send_notification: {x:?}"))?;
        Ok(())
    }

    pub fn send_request<T: lsp_types::request::Request>(
        &self,
        request: T::Params,
    ) -> Result<impl Future<Output = Result<T::Result>>> {
        let promise = self
            .send_request
            .call2(&JsValue::UNDEFINED, &T::METHOD.into(), &to_value(&request)?)
            .map_err(|x| format!("Error calling send_request: {x:?}"))?;
        let future = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise));
        Ok(async move {
            future.await.map_err(|e| format!("{e:?}").into()).and_then(|v| {
                serde_wasm_bindgen::from_value(v).map_err(|e| format!("{e:?}").into())
            })
        })
    }
}

impl RequestHandler {
    async fn handle_request(
        &self,
        method: String,
        params: JsValue,
        ctx: Rc<Context>,
    ) -> Result<JsValue> {
        if let Some(f) = self.0.get(&method.as_str()) {
            let param = serde_wasm_bindgen::from_value(params)
                .map_err(|x| format!("invalid param to handle_request: {x:?}"))?;
            let r = f(param, ctx).await?;
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
    ctx: Rc<Context>,
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
    let server_notifier = ServerNotifier { send_notification, send_request };
    let preview = Rc::new(Previewer {
        server_notifier: server_notifier.clone(),
        to_show: Default::default(),
    });

    let init_param = serde_wasm_bindgen::from_value(init_param)?;

    let mut compiler_config =
        CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Interpreter);

    let preview_notifier = preview.clone();
    compiler_config.open_import_fallback = Some(Rc::new(move |path| {
        let load_file = Function::from(load_file.clone());
        let preview_notifier = preview_notifier.clone();
        Box::pin(async move {
            let contents = self::load_file(path.clone(), &load_file).await;
            let Some(url) = to_url(&path) else {
                return Some(contents);
            };
            if let Ok(contents) = &contents {
                preview_notifier.set_contents(&VersionedUrl { url, version: None }, contents);
            }
            Some(contents)
        })
    }));
    let document_cache = RefCell::new(DocumentCache::new(compiler_config));
    let reentry_guard = Rc::new(RefCell::new(ReentryGuard::default()));

    let mut rh = RequestHandler::default();
    language::register_request_handlers(&mut rh);

    Ok(SlintServer {
        ctx: Rc::new(Context { document_cache, init_param, server_notifier, preview }),
        reentry_guard,
        rh: Rc::new(rh),
    })
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
            M::Status { message, health } => {
                crate::preview::send_status_notification(
                    &self.ctx.server_notifier,
                    &message,
                    health,
                );
            }
            M::Diagnostics { diagnostics, uri } => {
                crate::preview::notify_lsp_diagnostics(&self.ctx.server_notifier, uri, diagnostics);
            }
            M::ShowDocument { file, selection } => {
                send_show_document_to_editor(self.ctx.server_notifier.clone(), file, selection)
            }
            M::PreviewTypeChanged { is_external: _ } => {
                // Nothing to do!
            }
            M::RequestState { .. } => {
                crate::language::request_state(&self.ctx);
            }
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub fn server_initialize_result(&self, cap: JsValue) -> JsResult<JsValue> {
        Ok(to_value(&language::server_initialize_result(&serde_wasm_bindgen::from_value(cap)?))?)
    }

    #[wasm_bindgen]
    pub fn reload_document(&self, content: String, uri: JsValue, version: i32) -> js_sys::Promise {
        let ctx = self.ctx.clone();
        let guard = self.reentry_guard.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            let _lock = ReentryGuard::lock(guard).await;
            let uri: lsp_types::Url = serde_wasm_bindgen::from_value(uri)?;
            language::reload_document(
                &ctx,
                content,
                uri.clone(),
                Some(version),
                &mut ctx.document_cache.borrow_mut(),
            )
            .await
            .map_err(|e| JsError::new(&e.to_string()))?;
            Ok(JsValue::UNDEFINED)
        })
    }

    #[wasm_bindgen]
    pub fn handle_request(&self, _id: JsValue, method: String, params: JsValue) -> js_sys::Promise {
        let guard = self.reentry_guard.clone();
        let rh = self.rh.clone();
        let ctx = self.ctx.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            let fut = rh.handle_request(method, params, ctx);
            let _lock = ReentryGuard::lock(guard).await;
            fut.await.map_err(|e| JsError::new(&e.to_string()).into())
        })
    }

    #[wasm_bindgen]
    pub async fn reload_config(&self) -> JsResult<()> {
        let guard = self.reentry_guard.clone();
        let _lock = ReentryGuard::lock(guard).await;
        language::load_configuration(&self.ctx).await.map_err(|e| JsError::new(&e.to_string()))
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

pub fn send_show_document_to_editor(
    sender: ServerNotifier,
    file: String,
    selection: lsp_types::Range,
) {
    wasm_bindgen_futures::spawn_local(async move {
        let Some(params) =
            crate::preview::show_document_request_from_element_callback(&file, selection)
        else {
            return;
        };
        let Ok(fut) = sender.send_request::<lsp_types::request::ShowDocument>(params) else {
            return;
        };
        fut.await.unwrap();
    });
}
