#![cfg(target_arch = "wasm32")]

mod completion;
mod goto;
mod lsp_ext;
mod preview;
mod semantic_tokens;
mod server_loop;
mod util;

use std::cell::RefCell;
use std::rc::Rc;

use i_slint_compiler::CompilerConfiguration;
use lsp_types::InitializeParams;
use serde::{Deserialize, Serialize};
pub use server_loop::{DocumentCache, Error};
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
/*
#[wasm_bindgen]
extern "C" {
    fn send_notification(method: String, params: JsValue) -> bool;
    fn send_response(id: JsValue, result: JsValue) -> bool;
}
*/
#[derive(Clone)]
pub struct ServerNotifier;
impl ServerNotifier {
    pub fn send_notification(&self, method: String, params: impl Serialize) -> Result<(), Error> {
        match false /*send_notification(method, JsValue::from_serde(&params)?)*/ {
            true => Ok(()),
            false => Err("Failed to send notification".into()),
        }
    }
}

//#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    // #[serde(default = "serde_json::Value::default")]
    id: JsValue,
    method: String,
    // #[serde(default = "serde_json::Value::default")]
    params: serde_json::Value,
}
pub struct RequestHolder(Request);
impl RequestHolder {
    pub fn handle_request<
        Kind: lsp_types::request::Request,
        F: FnOnce(Kind::Params) -> Result<Kind::Result, Error>,
    >(
        &self,
        f: F,
    ) -> Result<bool, Error> {
        if self.0.method != Kind::METHOD {
            return Ok(false);
        }
        let result = f(serde_json::from_value(self.0.params.clone())?)?;
        /*if !send_response(self.0.id.clone(), JsValue::from_serde(&result)?) {
            return Err("Failed to send response".into());
        }*/
        Ok(true)
    }

    pub fn server_notifier(&self) -> ServerNotifier {
        ServerNotifier
    }
}

#[wasm_bindgen]
pub struct SlintServer(Rc<RefCell<DocumentCache<'static>>>, InitializeParams);

#[wasm_bindgen]
pub fn create(init_param: JsValue) -> Result<SlintServer, JsError> {
    let init_param = init_param.into_serde()?;

    let compiler_config =
        CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Interpreter);

    // FIXME: we leak one compiler_config
    let document_cache = DocumentCache::new(Box::leak(Box::new(compiler_config)));

    Ok(SlintServer(Rc::new(RefCell::new(document_cache)), init_param))
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
            &ServerNotifier,
            content,
            uri.into_serde()?,
            &mut self.0.borrow_mut(),
        )
        .map_err(|e| JsError::new(&e.to_string()))
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
        id: JsValue,
        method: String,
        params: JsValue,
    ) -> Result<(), JsError> {
        let r = Request { id, method, params: params.into_serde()? };
        server_loop::handle_request(RequestHolder(r), &self.1, &mut self.0.borrow_mut())
            .map_err(|e| JsError::new(&e.to_string()))
    }
}
