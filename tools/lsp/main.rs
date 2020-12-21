use std::path::Path;

use lsp_types::notification::{DidChangeTextDocument, DidOpenTextDocument, Notification};
use lsp_types::request::Completion;
use lsp_types::{
    request::GotoDefinition, GotoDefinitionResponse, InitializeParams, ServerCapabilities,
};
use lsp_types::{
    CompletionItem, CompletionOptions, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    HoverProviderCapability, OneOf, Position, PublishDiagnosticsParams, Range,
    WorkDoneProgressOptions,
};

use lsp_server::{Connection, Message, Request, RequestId, Response};

type Error = Box<dyn std::error::Error>;

fn main() -> Result<(), Error> {
    let (connection, io_threads) = Connection::stdio();
    let capabilities = ServerCapabilities {
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(true),
            trigger_characters: None,
            work_done_progress_options: WorkDoneProgressOptions::default(),
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        document_highlight_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        definition_provider: Some(OneOf::Left(true)),

        ..ServerCapabilities::default()
    };
    let server_capabilities = serde_json::to_value(&capabilities).unwrap();
    let initialization_params = connection.initialize(server_capabilities)?;
    main_loop(&connection, initialization_params)?;
    io_threads.join()?;
    Ok(())
}

fn main_loop(connection: &Connection, params: serde_json::Value) -> Result<(), Error> {
    let _params: InitializeParams = serde_json::from_value(params).unwrap();
    eprintln!("starting example main loop");
    for msg in &connection.receiver {
        eprintln!("got msg: {:?}", msg);
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                handle_request(connection, req)?;
            }
            Message::Response(_resp) => {}
            Message::Notification(notifi) => handle_notification(connection, notifi)?,
        }
    }
    Ok(())
}

fn handle_request(connection: &Connection, req: Request) -> Result<(), Error> {
    let mut req = Some(req);
    if let Some((id, params)) = cast::<GotoDefinition>(&mut req) {
        eprintln!("got gotoDefinition request #{}: {:?}", id, params);
        let result = Some(GotoDefinitionResponse::Array(Vec::new()));
        let resp = Response::new_ok(id, result);
        connection.sender.send(Message::Response(resp))?;
    } else if let Some((id, params)) = cast::<Completion>(&mut req) {
        eprintln!("got completion request #{}: {:?}", id, params);
        let result = vec![
            CompletionItem::new_simple("Hello".to_string(), "Some detail".to_string()),
            CompletionItem::new_simple("Bye".to_string(), "More detail".to_string()),
        ];
        let resp = Response::new_ok(id, result);
        connection.sender.send(Message::Response(resp))?;
    };
    // ...
    Ok(())
}

fn cast<Kind: lsp_types::request::Request>(
    req: &mut Option<Request>,
) -> Option<(RequestId, Kind::Params)> {
    match req.take().unwrap().extract::<Kind::Params>(Kind::METHOD) {
        Ok(value) => Some(value),
        Err(owned) => {
            *req = Some(owned);
            None
        }
    }
}

fn handle_notification(
    connection: &Connection,
    req: lsp_server::Notification,
) -> Result<(), Error> {
    match &*req.method {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(req.params)?;
            reload_document(connection, params.text_document.text, params.text_document.uri)?;
        }
        DidChangeTextDocument::METHOD => {
            let mut params: DidChangeTextDocumentParams = serde_json::from_value(req.params)?;
            reload_document(
                connection,
                params.content_changes.pop().unwrap().text,
                params.text_document.uri,
            )?;
        }
        _ => (),
    }
    Ok(())
}

fn reload_document(
    connection: &Connection,
    content: String,
    uri: lsp_types::Url,
) -> Result<(), Error> {
    let (_node, diag) = sixtyfps_compilerlib::parser::parse(content, Some(Path::new(uri.as_str())));

    let diagnostics = diag
        .inner
        .iter()
        .map(|d| {
            lsp_types::Diagnostic::new(
                to_range(d.line_column(&diag)),
                Some(lsp_types::DiagnosticSeverity::Error),
                None,
                None,
                d.to_string(),
                None,
                None,
            )
        })
        .collect();

    connection.sender.send(Message::Notification(lsp_server::Notification::new(
        "textDocument/publishDiagnostics".into(),
        PublishDiagnosticsParams { uri, diagnostics, version: None },
    )))?;

    //sixtyfps_compilerlib::compile_syntax_node(doc_node, diagnostics, compiler_config);
    Ok(())
}

fn to_range(span: (usize, usize)) -> Range {
    let pos = Position::new((span.0 as u32).saturating_sub(1), (span.1 as u32).saturating_sub(1));
    Range::new(pos, pos)
}
