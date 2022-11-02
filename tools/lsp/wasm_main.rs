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
            .call2(&JsValue::UNDEFINED, &method.into(), &to_value(&params)?)
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
        *self.reply.borrow_mut() = Some(to_value(&result)?);
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

#[wasm_bindgen(typescript_custom_section)]
const IMPORT_CALLBACK_FUNCTION_SECTION: &'static str = r#"
type ImportCallbackFunction = (url: string) => Promise<string>;
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "ImportCallbackFunction")]
    pub type ImportCallbackFunction;
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
    load_file: ImportCallbackFunction,
) -> Result<SlintServer, JsError> {
    console_error_panic_hook::set_once();

    let init_param = serde_wasm_bindgen::from_value(init_param)?;

    let mut compiler_config =
        CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Interpreter);
    compiler_config.open_import_fallback = Some(Rc::new(move |path| {
        let load_file = Function::from(load_file.clone());
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
    pub fn server_initialize_result(&self) -> Result<JsValue, JsError> {
        Ok(to_value(&server_loop::server_initialize_result())?)
    }

    #[wasm_bindgen]
    pub fn reload_document(&self, content: String, uri: JsValue) -> js_sys::Promise {
        let document_cache = self.document_cache.clone();
        let notifier = self.notifier.clone();
        let guard = self.reentry_guard.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            let _lock = ReentryGuard::lock(guard).await;
            let uri: lsp_types::Url = serde_wasm_bindgen::from_value(uri)?;
            server_loop::reload_document(&notifier, content, uri, &mut document_cache.borrow_mut())
                .await
                .map_err(|e| JsError::new(&e.to_string()))?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /*  #[wasm_bindgen]
    pub fn show_preview(&self, params: JsValue) -> Result<(), JsError> {
        server_loop::show_preview_command(
            &serde_wasm_bindgen::from_value(params)?,
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
                params: serde_wasm_bindgen::from_value(params)
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
) -> Result<wasm_bindgen::JsValue, serde_wasm_bindgen::Error> {
    value.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
}
