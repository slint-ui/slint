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
use serde::Serialize;
pub use server_loop::{Context, DocumentCache, Error, RequestHandler};
use std::cell::RefCell;
use std::future::Future;
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
pub struct ServerNotifier {
    send_notification: Function,
    send_request: Function,
}
impl ServerNotifier {
    pub fn send_notification(&self, method: String, params: impl Serialize) -> Result<(), Error> {
        self.send_notification
            .call2(&JsValue::UNDEFINED, &method.into(), &to_value(&params)?)
            .map_err(|x| format!("Error calling send_notification: {x:?}"))?;
        Ok(())
    }

    pub fn send_request<T: lsp_types::request::Request>(
        &self,
        request: T::Params,
    ) -> Result<impl Future<Output = Result<T::Result, Error>>, Error> {
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

    pub async fn progress_reporter(
        &self,
        token: Option<lsp_types::ProgressToken>,
        title: String,
        message: Option<String>,
        percentage: Option<u32>,
        cancellable: Option<bool>,
    ) -> Result<server_loop::ProgressReporter, Error> {
        let token = if let Some(t) = token {
            t
        } else {
            server_loop::ProgressReporter::create_server_side_token(self).await?
        };
        server_loop::ProgressReporter::new(
            self.clone(),
            token,
            title,
            message,
            percentage,
            cancellable,
        )
    }
}

impl RequestHandler {
    async fn handle_request(
        &self,
        method: String,
        params: JsValue,
        ctx: Rc<Context>,
    ) -> Result<JsValue, Error> {
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
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "ImportCallbackFunction")]
    pub type ImportCallbackFunction;

    #[wasm_bindgen(typescript_type = "SendRequestFunction")]
    pub type SendRequestFunction;
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
) -> Result<SlintServer, JsError> {
    console_error_panic_hook::set_once();

    let init_param = serde_wasm_bindgen::from_value(init_param)?;

    let mut compiler_config =
        CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Interpreter);
    compiler_config.open_import_fallback = Some(Rc::new(move |path| {
        let load_file = Function::from(load_file.clone());
        Box::pin(async move { Some(self::load_file(path, &load_file).await) })
    }));

    let document_cache = RefCell::new(DocumentCache::new(compiler_config));
    let send_request = Function::from(send_request.clone());
    let reentry_guard = Rc::new(RefCell::new(ReentryGuard::default()));

    let mut rh = RequestHandler::default();
    server_loop::register_request_handlers(&mut rh);

    Ok(SlintServer {
        ctx: Rc::new(Context {
            document_cache,
            init_param,
            server_notifier: ServerNotifier { send_notification, send_request },
        }),
        reentry_guard,
        rh: Rc::new(rh),
    })
}

#[wasm_bindgen]
impl SlintServer {
    #[wasm_bindgen]
    pub fn server_initialize_result(&self) -> Result<JsValue, JsError> {
        Ok(to_value(&server_loop::server_initialize_result())?)
    }

    #[wasm_bindgen]
    pub fn reload_document(&self, content: String, uri: JsValue, version: i32) -> js_sys::Promise {
        let ctx = self.ctx.clone();
        let guard = self.reentry_guard.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            let _lock = ReentryGuard::lock(guard).await;
            let uri: lsp_types::Url = serde_wasm_bindgen::from_value(uri)?;
            let progress = ctx
                .server_notifier
                .progress_reporter(
                    None,
                    "Opening document".into(),
                    Some(format!("Opening: {}@{}", uri.clone(), version)),
                    None,
                    None,
                )
                .await
                .map_err(|e| JsError::new(&e.to_string()))?;
            server_loop::reload_document(
                &ctx.server_notifier,
                content,
                uri.clone(),
                version,
                &mut ctx.document_cache.borrow_mut(),
            )
            .await
            .map_err(|e| JsError::new(&e.to_string()))?;
            progress
                .finish(Some(format!("Updated: {}@{}", uri, version)))
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
    pub async fn reload_config(&self) -> Result<(), JsError> {
        let guard = self.reentry_guard.clone();
        let _lock = ReentryGuard::lock(guard).await;
        server_loop::load_configuration(&self.ctx).await.map_err(|e| JsError::new(&e.to_string()))
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
