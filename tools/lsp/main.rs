// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![cfg(not(target_arch = "wasm32"))]

mod completion;
mod goto;
mod lsp_ext;
mod preview;
mod semantic_tokens;
mod server_loop;
mod util;

use i_slint_compiler::CompilerConfiguration;
use lsp_types::notification::{DidChangeTextDocument, DidOpenTextDocument, Notification};
use lsp_types::{DidChangeTextDocumentParams, DidOpenTextDocumentParams, InitializeParams};
use server_loop::*;

use clap::Parser;
use lsp_server::{Connection, Message, Request, Response};

#[derive(Clone, clap::Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(
        short = 'I',
        name = "Add include paths for the import statements",
        number_of_values = 1,
        parse(from_os_str)
    )]
    include_paths: Vec<std::path::PathBuf>,

    /// The style name for the preview ('native' or 'fluent')
    #[clap(long, name = "style name", default_value_t)]
    style: String,

    /// The backend used for the preview ('GL' or 'Qt')
    #[clap(long, name = "backend", default_value_t)]
    backend: String,
}

#[derive(Clone)]
pub struct ServerNotifier(crossbeam_channel::Sender<Message>);
impl ServerNotifier {
    pub fn send_notification(
        &self,
        method: String,
        params: impl serde::Serialize,
    ) -> Result<(), Error> {
        self.0.send(Message::Notification(lsp_server::Notification::new(method, params)))?;
        Ok(())
    }
}

pub struct RequestHolder(Request, crossbeam_channel::Sender<Message>);
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

        let result = f(param)?;
        self.1.send(Message::Response(Response::new_ok(id, result)))?;

        Ok(true)
    }

    pub fn server_notifier(&self) -> ServerNotifier {
        ServerNotifier(self.1.clone())
    }
}

fn main() {
    let args: Cli = Cli::parse();
    if !args.backend.is_empty() {
        std::env::set_var("SLINT_BACKEND", &args.backend);
    }

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

pub fn run_lsp_server() -> Result<(), Error> {
    let (connection, io_threads) = Connection::stdio();
    let capabilities = server_loop::server_capabilities();
    let server_capabilities = serde_json::to_value(&capabilities).unwrap();
    let initialization_params = connection.initialize(server_capabilities)?;
    main_loop(&connection, initialization_params)?;
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

    let mut document_cache = DocumentCache::new(&compiler_config);
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                handle_request(
                    RequestHolder(req, connection.sender.clone()),
                    &params,
                    &mut document_cache,
                )?;
            }
            Message::Response(_resp) => {}
            Message::Notification(notifi) => {
                handle_notification(connection, notifi, &mut document_cache)?
            }
        }
    }
    Ok(())
}

pub fn handle_notification(
    connection: &Connection,
    req: lsp_server::Notification,
    document_cache: &mut DocumentCache,
) -> Result<(), Error> {
    match &*req.method {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(req.params)?;
            reload_document(
                &ServerNotifier(connection.sender.clone()),
                params.text_document.text,
                params.text_document.uri,
                document_cache,
            )?;
        }
        DidChangeTextDocument::METHOD => {
            let mut params: DidChangeTextDocumentParams = serde_json::from_value(req.params)?;
            reload_document(
                &ServerNotifier(connection.sender.clone()),
                params.content_changes.pop().unwrap().text,
                params.text_document.uri,
                document_cache,
            )?;
        }
        "slint/showPreview" => {
            show_preview_command(
                req.params.as_array().map_or(&[], |x| x.as_slice()),
                &ServerNotifier(connection.sender.clone()),
                document_cache,
            )?;
        }
        _ => (),
    }
    Ok(())
}
