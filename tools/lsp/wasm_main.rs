// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![cfg(target_arch = "wasm32")]

mod completion;
mod goto;
mod lsp_ext;
mod semantic_tokens;
mod server_loop;
mod util;

use i_slint_compiler::CompilerConfiguration;
use lsp_types::InitializeParams;
use serde::Serialize;
pub use server_loop::{DocumentCache, Error};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

pub mod wasm_prelude {
    use std::path::{Path, PathBuf};

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
pub struct ServerNotifier(js_sys::Function);
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

#[wasm_bindgen]
pub struct SlintServer {
    document_cache: Rc<RefCell<DocumentCache<'static>>>,
    init_param: InitializeParams,
    notifier: ServerNotifier,
}

#[wasm_bindgen]
pub fn create(
    init_param: JsValue,
    send_notification: js_sys::Function,
) -> Result<SlintServer, JsError> {
    console_error_panic_hook::set_once();

    let init_param = init_param.into_serde()?;

    let compiler_config =
        CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Interpreter);

    // FIXME: we leak one compiler_config
    let document_cache = DocumentCache::new(Box::leak(Box::new(compiler_config)));

    Ok(SlintServer {
        document_cache: Rc::new(RefCell::new(document_cache)),
        init_param,
        notifier: ServerNotifier(send_notification),
    })
}

#[wasm_bindgen]
impl SlintServer {
    #[wasm_bindgen]
    pub fn capabilities(&self) -> Result<JsValue, JsError> {
        Ok(JsValue::from_serde(&server_loop::server_capabilities())?)
    }

    #[wasm_bindgen]
    pub fn reload_document(&self, content: String, uri: JsValue) -> Result<(), JsError> {
        server_loop::reload_document(
            &self.notifier,
            content,
            uri.into_serde()?,
            &mut self.document_cache.borrow_mut(),
        )
        .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(())
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
    pub fn handle_request(
        &self,
        _id: JsValue,
        method: String,
        params: JsValue,
    ) -> Result<JsValue, JsError> {
        let req = Request { method, params: params.into_serde()? };
        let result = Rc::new(RefCell::new(None));
        server_loop::handle_request(
            RequestHolder { req, reply: result.clone(), notifier: self.notifier.clone() },
            &self.init_param,
            &mut self.document_cache.borrow_mut(),
        )
        .map_err(|e| JsError::new(&e.to_string()))?;

        let result = result.borrow_mut().take();
        Ok(result.ok_or(JsError::new("Empty reply".into()))?)
    }
}
