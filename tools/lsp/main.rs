use std::collections::HashMap;
use std::path::Path;

use lsp_server::{Connection, Message, Request, RequestId, Response};
use lsp_types::notification::{DidChangeTextDocument, DidOpenTextDocument, Notification};
use lsp_types::request::GotoDefinition;
use lsp_types::request::{Completion, HoverRequest};
use lsp_types::{
    CompletionItem, CompletionOptions, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionResponse, Hover, HoverProviderCapability, InitializeParams, LocationLink,
    MarkedString, OneOf, Position, PublishDiagnosticsParams, Range, ServerCapabilities, Url,
    WorkDoneProgressOptions,
};
use sixtyfps_compilerlib::diagnostics::Spanned;
use sixtyfps_compilerlib::parser::{SyntaxKind, SyntaxNodeWithSourceFile};

type Error = Box<dyn std::error::Error>;

struct FileState {
    node: sixtyfps_compilerlib::parser::syntax_nodes::Document,
    newline_offsets: Vec<u32>,
    doc: sixtyfps_compilerlib::object_tree::Document,
}

#[derive(Default)]
/// FIXME: this should be merged with the TypeLoader cache
struct DocumentCache {
    files: HashMap<Url, FileState>,
}

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
    let mut document_cache = DocumentCache::default();
    for msg in &connection.receiver {
        eprintln!("got msg: {:?}", msg);
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                handle_request(connection, req, &mut document_cache)?;
            }
            Message::Response(_resp) => {}
            Message::Notification(notifi) => {
                handle_notification(connection, notifi, &mut document_cache)?
            }
        }
    }
    Ok(())
}

fn handle_request(
    connection: &Connection,
    req: Request,
    document_cache: &mut DocumentCache,
) -> Result<(), Error> {
    let mut req = Some(req);
    if let Some((id, params)) = cast::<GotoDefinition>(&mut req) {
        let result = token_descr(document_cache, params.text_document_position_params)
            .and_then(|token| goto_definition(document_cache, token.parent()));
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
    } else if let Some((id, params)) = cast::<HoverRequest>(&mut req) {
        let result =
            token_descr(document_cache, params.text_document_position_params).map(|x| Hover {
                contents: lsp_types::HoverContents::Scalar(MarkedString::from_language_code(
                    "text".into(),
                    format!("{:?}", x.token),
                )),
                range: None,
            });
        let resp = Response::new_ok(id, result);
        connection.sender.send(Message::Response(resp))?;
    };
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
    document_cache: &mut DocumentCache,
) -> Result<(), Error> {
    match &*req.method {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(req.params)?;
            reload_document(
                connection,
                params.text_document.text,
                params.text_document.uri,
                document_cache,
            )?;
        }
        DidChangeTextDocument::METHOD => {
            let mut params: DidChangeTextDocumentParams = serde_json::from_value(req.params)?;
            reload_document(
                connection,
                params.content_changes.pop().unwrap().text,
                params.text_document.uri,
                document_cache,
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
    document_cache: &mut DocumentCache,
) -> Result<(), Error> {
    let mut ln_offs = 0;
    let newline_offsets = content
        .split('\n')
        .map(|line| {
            let r = ln_offs;
            ln_offs += line.len() as u32 + 1;
            r
        })
        .collect();

    let (doc_node, diag) =
        sixtyfps_compilerlib::parser::parse(content, Some(Path::new(uri.path())));

    document_cache.files.insert(
        uri.clone(),
        FileState { node: doc_node.clone().into(), newline_offsets, doc: Default::default() },
    );

    if diag.has_error() {
        let diagnostics = diag.inner.iter().map(|d| to_lsp_diag(d, &diag)).collect();
        connection.sender.send(Message::Notification(lsp_server::Notification::new(
            "textDocument/publishDiagnostics".into(),
            PublishDiagnosticsParams { uri, diagnostics, version: None },
        )))?;
        return Ok(());
    }

    let mut compiler_config = sixtyfps_compilerlib::CompilerConfiguration::new(
        sixtyfps_compilerlib::generator::OutputFormat::Interpreter,
    );
    compiler_config.style = Some("ugly".into());
    let (doc, diag) = spin_on::spin_on(sixtyfps_compilerlib::compile_syntax_node(
        doc_node,
        diag,
        compiler_config,
    ));
    document_cache.files.get_mut(&uri).unwrap().doc = doc;

    for file_diag in diag.into_iter() {
        if file_diag.current_path.is_relative() {
            continue;
        }
        let diagnostics = file_diag.inner.iter().map(|d| to_lsp_diag(d, &file_diag)).collect();
        connection.sender.send(Message::Notification(lsp_server::Notification::new(
            "textDocument/publishDiagnostics".into(),
            PublishDiagnosticsParams {
                uri: Url::from_file_path(file_diag.current_path.as_path()).unwrap(),
                diagnostics,
                version: None,
            },
        )))?;
    }

    Ok(())
}

fn to_lsp_diag(
    d: &sixtyfps_compilerlib::diagnostics::Diagnostic,
    file_diag: &sixtyfps_compilerlib::diagnostics::FileDiagnostics,
) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic::new(
        to_range(d.line_column(file_diag)),
        Some(lsp_types::DiagnosticSeverity::Error),
        None,
        None,
        d.to_string(),
        None,
        None,
    )
}

fn to_range(span: (usize, usize)) -> Range {
    let pos = Position::new((span.0 as u32).saturating_sub(1), (span.1 as u32).saturating_sub(1));
    Range::new(pos, pos)
}

fn token_descr(
    document_cache: &DocumentCache,
    lsp_position: lsp_types::TextDocumentPositionParams,
) -> Option<sixtyfps_compilerlib::parser::SyntaxTokenWithSourceFile> {
    let file_state = document_cache.files.get(&lsp_position.text_document.uri)?;
    let o = file_state.newline_offsets.get(lsp_position.position.line as usize)?
        + lsp_position.position.character as u32;
    let token = file_state.node.0.node.token_at_offset(o.into()).last()?;
    Some(sixtyfps_compilerlib::parser::SyntaxTokenWithSourceFile {
        token,
        source_file: file_state.node.0.source_file.clone(),
    })
    //Some(format!("{:?}", token))
}

fn goto_definition(
    document_cache: &mut DocumentCache,
    token: sixtyfps_compilerlib::parser::SyntaxNodeWithSourceFile,
) -> Option<GotoDefinitionResponse> {
    match token.kind() {
        SyntaxKind::QualifiedName => {
            let source_file = token.source_file.clone()?;
            let parent = token.node.parent()?;
            let qual =
                sixtyfps_compilerlib::object_tree::QualifiedTypeName::from_node(token.into());
            match parent.kind() {
                SyntaxKind::Element => {
                    let file_state = document_cache
                        .files
                        .get(&Url::from_file_path(source_file.as_path()).ok()?)?;
                    match file_state.doc.local_registry.lookup_qualified(&qual.members) {
                        sixtyfps_compilerlib::langtype::Type::Component(c) => {
                            goto_node(document_cache, &c.root_element.borrow().node.as_ref()?.0)
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn goto_node(
    document_cache: &mut DocumentCache,
    node: &SyntaxNodeWithSourceFile,
) -> Option<GotoDefinitionResponse> {
    let target_uri = Url::from_file_path(node.source_file.as_ref()?.as_path()).ok()?;
    let file_state = document_cache.files.get(&target_uri)?; // FIXME! the file might not be in the cache if it is not open currently
    let offset = node.span().offset as u32;
    let pos = file_state.newline_offsets.binary_search(&offset).map_or_else(
        |line| {
            if line == 0 {
                Position::new(0, offset)
            } else {
                Position::new(
                    line as u32 - 1,
                    file_state.newline_offsets.get(line - 1).map_or(0, |x| offset - *x),
                )
            }
        },
        |line| Position::new(line as u32, 0),
    );
    let range = Range::new(pos, pos);
    Some(GotoDefinitionResponse::Link(vec![LocationLink {
        origin_selection_range: None,
        target_uri,
        target_range: range,
        target_selection_range: range,
    }]))
}
