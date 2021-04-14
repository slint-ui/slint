/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

mod preview;

use std::collections::HashMap;
use std::path::Path;

use lsp_server::{Connection, Message, Request, RequestId, Response};
use lsp_types::notification::{DidChangeTextDocument, DidOpenTextDocument, Notification};
use lsp_types::request::GotoDefinition;
use lsp_types::request::{Completion, HoverRequest};
use lsp_types::{
    CompletionItem, CompletionOptions, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionResponse, Hover, HoverProviderCapability, InitializeParams, LocationLink, OneOf,
    Position, PublishDiagnosticsParams, Range, ServerCapabilities, TextDocumentSyncCapability, Url,
    WorkDoneProgressOptions,
};
use sixtyfps_compilerlib::diagnostics::{BuildDiagnostics, Spanned};
use sixtyfps_compilerlib::langtype::Type;
use sixtyfps_compilerlib::parser::{syntax_nodes, SyntaxKind, SyntaxNode};
use sixtyfps_compilerlib::typeloader::TypeLoader;
use sixtyfps_compilerlib::typeregister::TypeRegister;
use sixtyfps_compilerlib::{object_tree, CompilerConfiguration};

type Error = Box<dyn std::error::Error>;

struct DocumentCache<'a> {
    documents: TypeLoader<'a>,
    newline_offsets: HashMap<Url, Vec<u32>>,
}

impl<'a> DocumentCache<'a> {
    fn new(config: &'a CompilerConfiguration) -> Self {
        let documents =
            TypeLoader::new(TypeRegister::builtin(), config, &mut BuildDiagnostics::default());
        Self { documents, newline_offsets: Default::default() }
    }
}

fn main() {
    std::thread::spawn(|| {
        match run_lsp_server() {
            Ok(_) => {}
            Err(error) => {
                eprintln!("Error running LSP server: {}", error);
            }
        }
        preview::quit_ui_event_loop();
    });

    preview::start_ui_event_loop();
}

fn run_lsp_server() -> Result<(), Error> {
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
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            lsp_types::TextDocumentSyncKind::Full,
        )),

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
    let mut compiler_config = sixtyfps_compilerlib::CompilerConfiguration::new(
        sixtyfps_compilerlib::generator::OutputFormat::Interpreter,
    );
    compiler_config.style = Some("ugly".into());

    let mut document_cache = DocumentCache::new(&compiler_config);
    for msg in &connection.receiver {
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
        let result = token_descr(document_cache, params.text_document_position)
            .and_then(|token| completion_at(document_cache, token.parent()));
        let resp = Response::new_ok(id, result);
        connection.sender.send(Message::Response(resp))?;
    } else if let Some((id, _params)) = cast::<HoverRequest>(&mut req) {
        /*let result =
            token_descr(document_cache, params.text_document_position_params).map(|x| Hover {
                contents: lsp_types::HoverContents::Scalar(MarkedString::from_language_code(
                    "text".into(),
                    format!("{:?}", x.token),
                )),
                range: None,
            });
        let resp = Response::new_ok(id, result);
        connection.sender.send(Message::Response(resp))?;*/
        connection.sender.send(Message::Response(Response::new_ok(id, None::<Hover>)))?;
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
        "sixtyfps/showPreview" => {
            let e = || -> Error { "InvalidParameter".into() };
            let path = if let serde_json::Value::String(s) = req.params.get(0).ok_or_else(e)? {
                std::path::PathBuf::from(s)
            } else {
                return Err(e());
            };
            let path_canon = path.canonicalize().unwrap_or_else(|_| path.to_owned());
            preview::load_preview(path_canon.into(), preview::PostLoadBehavior::ShowAfterLoad);
        }
        _ => (),
    }
    Ok(())
}

fn newline_offsets_from_content(content: &str) -> Vec<u32> {
    let mut ln_offs = 0;
    content
        .split('\n')
        .map(|line| {
            let r = ln_offs;
            ln_offs += line.len() as u32 + 1;
            r
        })
        .collect()
}

fn reload_document(
    connection: &Connection,
    content: String,
    uri: lsp_types::Url,
    document_cache: &mut DocumentCache,
) -> Result<(), Error> {
    let newline_offsets = newline_offsets_from_content(&content);
    document_cache.newline_offsets.insert(uri.clone(), newline_offsets);

    let path = uri.to_file_path().unwrap();
    let path_canon = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    preview::set_contents(&path_canon, content.clone());
    let mut diag = BuildDiagnostics::default();
    spin_on::spin_on(document_cache.documents.load_file(&path_canon, &path, content, &mut diag));

    // Always provide diagnostics for all files. Empty diagnostics clear any previous ones.
    let mut lsp_diags: HashMap<Url, Vec<lsp_types::Diagnostic>> = core::iter::once(&path)
        .chain(diag.all_loaded_files.iter())
        .map(|path| {
            let uri = Url::from_file_path(path).unwrap();
            (uri, Default::default())
        })
        .collect();

    for d in diag.into_iter() {
        if d.source_file().unwrap().is_relative() {
            continue;
        }
        let uri = Url::from_file_path(d.source_file().unwrap()).unwrap();
        lsp_diags.entry(uri).or_default().push(to_lsp_diag(&d));
    }

    for (uri, diagnostics) in lsp_diags {
        connection.sender.send(Message::Notification(lsp_server::Notification::new(
            "textDocument/publishDiagnostics".into(),
            PublishDiagnosticsParams { uri, diagnostics, version: None },
        )))?;
    }

    Ok(())
}

fn to_lsp_diag_level(
    level: sixtyfps_compilerlib::diagnostics::DiagnosticLevel,
) -> lsp_types::DiagnosticSeverity {
    match level {
        sixtyfps_interpreter::DiagnosticLevel::Error => lsp_types::DiagnosticSeverity::Error,
        sixtyfps_interpreter::DiagnosticLevel::Warning => lsp_types::DiagnosticSeverity::Warning,
    }
}

fn to_lsp_diag(d: &sixtyfps_compilerlib::diagnostics::Diagnostic) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic::new(
        to_range(d.line_column()),
        Some(to_lsp_diag_level(d.level())),
        None,
        None,
        d.message().to_owned(),
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
) -> Option<sixtyfps_compilerlib::parser::SyntaxToken> {
    let o = document_cache
        .newline_offsets
        .get(&lsp_position.text_document.uri)?
        .get(lsp_position.position.line as usize)?
        + lsp_position.position.character as u32;

    let doc =
        document_cache.documents.get_document(Path::new(lsp_position.text_document.uri.path()))?;
    let node = doc.node.as_ref()?;
    if !node.text_range().contains(o.into()) {
        return None;
    }
    let token = node.token_at_offset(o.into()).last()?;
    Some(sixtyfps_compilerlib::parser::SyntaxToken { token, source_file: node.source_file.clone() })
}

fn goto_definition(
    document_cache: &mut DocumentCache,
    token: sixtyfps_compilerlib::parser::SyntaxNode,
) -> Option<GotoDefinitionResponse> {
    match token.kind() {
        SyntaxKind::QualifiedName => {
            let source_file = token.source_file.clone();
            let parent = token.node.parent()?;
            let qual =
                sixtyfps_compilerlib::object_tree::QualifiedTypeName::from_node(token.into());
            match parent.kind() {
                SyntaxKind::Element => {
                    let doc = document_cache.documents.get_document(source_file.path())?;
                    match doc.local_registry.lookup_qualified(&qual.members) {
                        sixtyfps_compilerlib::langtype::Type::Component(c) => {
                            goto_node(document_cache, &*c.root_element.borrow().node.as_ref()?)
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
    node: &SyntaxNode,
) -> Option<GotoDefinitionResponse> {
    let path = node.source_file.path();
    let target_uri = Url::from_file_path(path).ok()?;
    let newline_offsets = match document_cache.newline_offsets.entry(target_uri.clone()) {
        std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
        std::collections::hash_map::Entry::Vacant(e) => {
            e.insert(newline_offsets_from_content(&std::fs::read_to_string(path).ok()?))
        }
    };
    let offset = node.span().offset as u32;
    let pos = newline_offsets.binary_search(&offset).map_or_else(
        |line| {
            if line == 0 {
                Position::new(0, offset)
            } else {
                Position::new(
                    line as u32 - 1,
                    newline_offsets.get(line - 1).map_or(0, |x| offset - *x),
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

fn completion_at(document_cache: &DocumentCache, token: SyntaxNode) -> Option<Vec<CompletionItem>> {
    match token.kind() {
        SyntaxKind::Element => {
            let element = syntax_nodes::Element::from(token.clone());
            let global_tr = document_cache.documents.global_type_registry.borrow();
            let tr = token
                .source_file()
                .and_then(|sf| document_cache.documents.get_document(sf.path()))
                .map(|doc| &doc.local_registry)
                .unwrap_or(&global_tr);
            let element_type = lookup_current_element_type(token, tr).unwrap_or_default();
            return Some(
                element_type
                    .property_list()
                    .into_iter()
                    .map(|(k, t)| CompletionItem::new_simple(k, t.to_string()))
                    .chain(element.PropertyDeclaration().map(|pr| {
                        CompletionItem::new_simple(
                            sixtyfps_compilerlib::parser::identifier_text(&pr.DeclaredIdentifier())
                                .unwrap_or_default(),
                            pr.Type().text().into(),
                        )
                    }))
                    .chain(element.CallbackDeclaration().map(|cd| {
                        CompletionItem::new_simple(
                            sixtyfps_compilerlib::parser::identifier_text(&cd.DeclaredIdentifier())
                                .unwrap_or_default(),
                            "callback".into(),
                        )
                    }))
                    .collect(),
            );
        }
        _ => return None,
    }
}

fn lookup_current_element_type(mut node: SyntaxNode, tr: &TypeRegister) -> Option<Type> {
    while node.kind() != SyntaxKind::Element {
        if let Some(parent) = node.parent() {
            node = parent
        } else {
            return None;
        }
    }
    let parent = node
        .parent()
        .and_then(|parent| lookup_current_element_type(parent, tr))
        .unwrap_or_default();
    let qualname = object_tree::QualifiedTypeName::from_node(
        syntax_nodes::Element::from(node).QualifiedName()?,
    );
    parent.lookup_type_for_child_element(&qualname.to_string(), tr).ok()
}
