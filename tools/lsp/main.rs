// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![cfg(not(target_arch = "wasm32"))]

mod completion;
mod goto;
mod lsp_ext;
#[cfg(feature = "preview")]
mod preview;
mod properties;
mod semantic_tokens;
mod server_loop;
#[cfg(test)]
mod test;
mod util;

use i_slint_compiler::CompilerConfiguration;
use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidOpenTextDocument, Notification,
};
use lsp_types::{DidChangeTextDocumentParams, DidOpenTextDocumentParams, InitializeParams};
use server_loop::*;

use clap::Parser;
use lsp_server::{Connection, Message, RequestId, Response};
use std::collections::HashMap;
use std::sync::{atomic, Arc, Mutex};

#[derive(Clone, clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(
        short = 'I',
        name = "Add include paths for the import statements",
        number_of_values = 1,
        action
    )]
    include_paths: Vec<std::path::PathBuf>,

    /// The style name for the preview ('native' or 'fluent')
    #[arg(long, name = "style name", default_value_t, action)]
    style: String,

    /// The backend used for the preview ('GL' or 'Qt')
    #[arg(long, name = "backend", default_value_t, action)]
    backend: String,
}

type OutgoingRequestQueue = Arc<
    Mutex<HashMap<RequestId, Box<dyn FnOnce(lsp_server::Response, &mut DocumentCache) + Send>>>,
>;

/// A handle that can be used to communicate with the client
///
/// This type is duplicated, with the same interface, in wasm_main.rs
#[derive(Clone)]
pub struct ServerNotifier(crossbeam_channel::Sender<Message>, OutgoingRequestQueue);
impl ServerNotifier {
    pub fn send_notification(
        &self,
        method: String,
        params: impl serde::Serialize,
    ) -> Result<(), Error> {
        self.0.send(Message::Notification(lsp_server::Notification::new(method, params)))?;
        Ok(())
    }

    pub fn send_request<T: lsp_types::request::Request>(
        &self,
        request: T::Params,
        f: impl FnOnce(Result<T::Result, String>, &mut DocumentCache) + Send + 'static,
    ) -> Result<(), Error> {
        static REQ_ID: atomic::AtomicI32 = atomic::AtomicI32::new(0);
        let id = RequestId::from(REQ_ID.fetch_add(1, atomic::Ordering::Relaxed));
        let msg =
            Message::Request(lsp_server::Request::new(id.clone(), T::METHOD.to_string(), request));
        self.0.send(msg)?;
        self.1.lock().unwrap().insert(
            id,
            Box::new(move |r, c| {
                if let Some(r) = r.result {
                    f(serde_json::from_value(r).map_err(|e| e.to_string()), c)
                } else if let Some(r) = r.error {
                    f(Err(r.message), c)
                }
            }),
        );
        Ok(())
    }
}

/// The interface for a request received from the client
///
/// This type is duplicated, with the same interface, in wasm_main.rs
pub struct RequestHolder(lsp_server::Request, ServerNotifier);
impl RequestHolder {
    pub fn handle_request<
        Kind: lsp_types::request::Request,
        F: FnOnce(Kind::Params) -> Result<Kind::Result, Error>,
    >(
        &self,
        f: F,
    ) -> Result<bool, Error> {
        let (id, param) = match self.0.clone().extract::<Kind::Params>(Kind::METHOD) {
            Ok(value) => value,
            Err(lsp_server::ExtractError::MethodMismatch(_)) => {
                return Ok(false);
            }
            Err(e) => {
                return Err(format!("error when deserializing request: {e:?}").into());
            }
        };

        match f(param) {
            Ok(r) => self.1 .0.send(Message::Response(Response::new_ok(id, r)))?,
            Err(e) => {
                self.1 .0.send(Message::Response(Response::new_err(id, 23, format!("{}", e))))?
            }
        };

        Ok(true)
    }

    pub fn server_notifier(&self) -> ServerNotifier {
        self.1.clone()
    }
}

fn main() {
    let args: Cli = Cli::parse();
    if !args.backend.is_empty() {
        std::env::set_var("SLINT_BACKEND", &args.backend);
    }

    #[cfg(feature = "preview")]
    {
        let lsp_thread = std::thread::spawn(|| {
            /// Make sure we quit the event loop even if we panic
            struct QuitEventLoop;
            impl Drop for QuitEventLoop {
                fn drop(&mut self) {
                    preview::quit_ui_event_loop();
                }
            }
            let _quit_ui_loop = QuitEventLoop;

            match run_lsp_server() {
                Ok(_) => {}
                Err(error) => {
                    eprintln!("Error running LSP server: {}", error);
                }
            }
        });

        preview::start_ui_event_loop();
        lsp_thread.join().unwrap();
    }
    #[cfg(not(feature = "preview"))]
    match run_lsp_server() {
        Ok(_) => {}
        Err(error) => {
            eprintln!("Error running LSP server: {}", error);
        }
    }
}

pub fn run_lsp_server() -> Result<(), Error> {
    let (connection, io_threads) = Connection::stdio();
    let (id, params) = connection.initialize_start()?;

    let initialize_result = serde_json::to_value(server_loop::server_initialize_result())?;
    connection.initialize_finish(id, initialize_result)?;

    main_loop(&connection, params)?;
    io_threads.join()?;
    Ok(())
}

fn main_loop(connection: &Connection, params: serde_json::Value) -> Result<(), Error> {
    let params: InitializeParams = serde_json::from_value(params).unwrap();
    let mut compiler_config =
        CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Interpreter);

    let cli_args = Cli::parse();
    compiler_config.style =
        Some(if cli_args.style.is_empty() { "fluent".into() } else { cli_args.style });
    compiler_config.include_paths = cli_args.include_paths;

    let mut document_cache = DocumentCache::new(compiler_config);
    let request_queue = OutgoingRequestQueue::default();

    let server_notifier = ServerNotifier(connection.sender.clone(), request_queue.clone());

    load_configuration(&server_notifier)?;

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                handle_request(
                    RequestHolder(req, server_notifier.clone()),
                    &params,
                    &mut document_cache,
                )?;
            }
            Message::Response(resp) => {
                let f = request_queue
                    .lock()
                    .unwrap()
                    .remove(&resp.id)
                    .ok_or("Response to unknown request")?;
                f(resp, &mut document_cache);
            }
            Message::Notification(notification) => {
                handle_notification(&server_notifier, notification, &mut document_cache)?
            }
        }
    }
    Ok(())
}

pub fn handle_notification(
    connection: &ServerNotifier,
    req: lsp_server::Notification,
    document_cache: &mut DocumentCache,
) -> Result<(), Error> {
    match &*req.method {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(req.params)?;
            spin_on::spin_on(reload_document(
                connection,
                params.text_document.text,
                params.text_document.uri,
                params.text_document.version,
                document_cache,
            ))?;
        }
        DidChangeTextDocument::METHOD => {
            let mut params: DidChangeTextDocumentParams = serde_json::from_value(req.params)?;
            spin_on::spin_on(reload_document(
                connection,
                params.content_changes.pop().unwrap().text,
                params.text_document.uri,
                params.text_document.version,
                document_cache,
            ))?;
        }
        DidChangeConfiguration::METHOD => {
            load_configuration(connection)?;
        }

        #[cfg(feature = "preview")]
        "slint/showPreview" => {
            show_preview_command(
                req.params.as_array().map_or(&[], |x| x.as_slice()),
                connection,
                &document_cache.documents.compiler_config,
            )?;
        }
        _ => (),
    }
    Ok(())
}
