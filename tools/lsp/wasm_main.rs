// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![cfg(target_arch = "wasm32")]

mod completion;
mod goto;
mod lsp_ext;
mod properties;
mod semantic_tokens;
mod server_loop;
mod util;

use i_slint_compiler::CompilerConfiguration;
use js_sys::Function;
use lsp_types::InitializeParams;
use serde::Serialize;
pub use server_loop::{DocumentCache, Error};
use std::cell::RefCell;
use std::io::ErrorKind;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

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

#[derive(Clone)]
pub struct ServerNotifier(Function);
impl ServerNotifier {
    pub fn send_notification(&self, method: String, params: impl Serialize) -> Result<(), Error> {
        self.0
            .call2(&JsValue::UNDEFINED, &method.into(), &JsValue::from_serde(&params)?)
            .map_err(|x| format!("Error calling send_notification: {x:?}"))?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    method: String,
    params: serde_json::Value,
}
pub struct RequestHolder {
    req: Request,
    /// The result will be assigned there
    reply: Rc<RefCell<Option<JsValue>>>,

    notifier: ServerNotifier,
}
impl RequestHolder {
    pub fn handle_request<
        Kind: lsp_types::request::Request,
        F: FnOnce(Kind::Params) -> Result<Kind::Result, Error>,
    >(
        &self,
        f: F,
    ) -> Result<bool, Error> {
        if self.req.method != Kind::METHOD {
            return Ok(false);
        }
        let result = f(serde_json::from_value(self.req.params.clone())?)?;
        *self.reply.borrow_mut() = Some(JsValue::from_serde(&result)?);
        Ok(true)
    }

    pub fn server_notifier(&self) -> ServerNotifier {
        self.notifier.clone()
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

#[wasm_bindgen]
pub struct SlintServer {
    document_cache: Rc<RefCell<DocumentCache>>,
    init_param: InitializeParams,
    notifier: ServerNotifier,
    reentry_guard: Rc<RefCell<ReentryGuard>>,
}

#[wasm_bindgen]
pub fn create(
    init_param: JsValue,
    send_notification: Function,
    load_file: Function,
) -> Result<SlintServer, JsError> {
    console_error_panic_hook::set_once();

    let init_param = init_param.into_serde()?;

    let mut compiler_config =
        CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Interpreter);
    compiler_config.open_import_fallback = Some(Rc::new(move |path| {
        let load_file = load_file.clone();
        Box::pin(async move { Some(self::load_file(path, &load_file).await) })
    }));

    let document_cache = DocumentCache::new(compiler_config);

    Ok(SlintServer {
        document_cache: Rc::new(RefCell::new(document_cache)),
        init_param,
        notifier: ServerNotifier(send_notification),
        reentry_guard: Default::default(),
    })
}

#[wasm_bindgen]
impl SlintServer {
    #[wasm_bindgen]
    pub fn capabilities(&self) -> Result<JsValue, JsError> {
        Ok(JsValue::from_serde(&server_loop::server_capabilities())?)
    }

    #[wasm_bindgen]
    pub fn reload_document(&self, content: String, uri: JsValue) -> js_sys::Promise {
        let document_cache = self.document_cache.clone();
        let notifier = self.notifier.clone();
        let guard = self.reentry_guard.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            let _lock = ReentryGuard::lock(guard).await;
            let uri: lsp_types::Url = uri.into_serde().map_err(|e| JsError::new(&e.to_string()))?;
            server_loop::reload_document(&notifier, content, uri, &mut document_cache.borrow_mut())
                .await
                .map_err(|e| JsError::new(&e.to_string()))?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /*  #[wasm_bindgen]
    pub fn show_preview(&self, params: JsValue) -> Result<(), JsError> {
        server_loop::show_preview_command(
            &params.into_serde()?,
            &ServerNotifier,
            &mut self.0.borrow_mut(),
        )
        .map_err(|e| JsError::new(&e.to_string()));
    }*/

    #[wasm_bindgen]
    pub fn handle_request(&self, _id: JsValue, method: String, params: JsValue) -> js_sys::Promise {
        let document_cache = self.document_cache.clone();
        let notifier = self.notifier.clone();
        let guard = self.reentry_guard.clone();
        let init_param = self.init_param.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            let _lock = ReentryGuard::lock(guard).await;
            let req = Request {
                method,
                params: params
                    .into_serde()
                    .map_err(|x| format!("invalid param to handle_request: {x:?}"))?,
            };
            let result = Rc::new(RefCell::new(None));
            server_loop::handle_request(
                RequestHolder { req, reply: result.clone(), notifier: notifier.clone() },
                &init_param,
                &mut document_cache.borrow_mut(),
            )
            .map_err(|e| JsError::new(&e.to_string()))?;

            let result = result.borrow_mut().take();
            Ok(result.ok_or(JsError::new("Empty reply".into()))?)
        })
    }
}

async fn load_file(path: String, load_file: &Function) -> std::io::Result<String> {
    let value = load_file
        .call1(&JsValue::UNDEFINED, &path.into())
        .map_err(|x| std::io::Error::new(ErrorKind::Other, format!("{x:?}")))?;
    let array = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(value))
        .await
        .map_err(|e| std::io::Error::new(ErrorKind::Other, format!("{e:?}")))?;
    String::from_utf8(js_sys::Uint8Array::from(array).to_vec())
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e.to_string()))
}
