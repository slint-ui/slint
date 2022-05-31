// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;
use crate::{completion, goto, preview, semantic_tokens, util, RequestHolder};
use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::langtype::Type;
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode, SyntaxToken};
use i_slint_compiler::typeloader::TypeLoader;
use i_slint_compiler::typeregister::TypeRegister;
use i_slint_compiler::CompilerConfiguration;
use lsp_types::request::{
    CodeActionRequest, CodeLensRequest, ColorPresentationRequest, Completion, DocumentColor,
    DocumentSymbolRequest, ExecuteCommand, GotoDefinition, HoverRequest, SemanticTokensFullRequest,
};
use lsp_types::{
    CodeActionOrCommand, CodeActionProviderCapability, CodeLens, CodeLensOptions, Color,
    ColorInformation, ColorPresentation, Command, CompletionOptions, DocumentSymbolResponse,
    ExecuteCommandOptions, Hover, InitializeParams, Location, OneOf, Position,
    PublishDiagnosticsParams, Range, SemanticTokensFullOptions, SemanticTokensLegend,
    SemanticTokensOptions, ServerCapabilities, SymbolInformation, TextDocumentSyncCapability, Url,
    WorkDoneProgressOptions,
};
use std::collections::HashMap;

pub type Error = Box<dyn std::error::Error>;

const SHOW_PREVIEW_COMMAND: &str = "showPreview";

pub struct DocumentCache<'a> {
    pub(crate) documents: TypeLoader<'a>,
    newline_offsets: HashMap<Url, Vec<u32>>,
}

impl<'a> DocumentCache<'a> {
    pub fn new(config: &'a CompilerConfiguration) -> Self {
        let documents =
            TypeLoader::new(TypeRegister::builtin(), config, &mut BuildDiagnostics::default());
        Self { documents, newline_offsets: Default::default() }
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

    pub fn byte_offset_to_position(
        &mut self,
        offset: u32,
        target_uri: &lsp_types::Url,
    ) -> Option<lsp_types::Position> {
        let newline_offsets = match self.newline_offsets.entry(target_uri.clone()) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(Self::newline_offsets_from_content(
                    &std::fs::read_to_string(target_uri.to_file_path().ok()?).ok()?,
                ))
            }
        };
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
        Some(pos)
    }
}

pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        completion_provider: Some(CompletionOptions {
            resolve_provider: None,
            trigger_characters: Some(vec![".".to_owned()]),
            work_done_progress_options: WorkDoneProgressOptions::default(),
            all_commit_characters: None,
        }),
        definition_provider: Some(OneOf::Left(true)),
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            lsp_types::TextDocumentSyncKind::FULL,
        )),
        code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
        execute_command_provider: Some(ExecuteCommandOptions {
            commands: vec![SHOW_PREVIEW_COMMAND.into()],
            ..Default::default()
        }),
        document_symbol_provider: Some(OneOf::Left(true)),
        color_provider: Some(true.into()),
        code_lens_provider: Some(CodeLensOptions { resolve_provider: Some(true) }),
        semantic_tokens_provider: Some(
            SemanticTokensOptions {
                legend: SemanticTokensLegend {
                    token_types: semantic_tokens::LEGEND_TYPES.to_vec(),
                    token_modifiers: semantic_tokens::LEGEND_MODS.to_vec(),
                },
                full: Some(SemanticTokensFullOptions::Bool(true)),
                ..Default::default()
            }
            .into(),
        ),
        ..ServerCapabilities::default()
    }
}

pub fn handle_request(
    req: RequestHolder,
    init_param: &InitializeParams,
    document_cache: &mut DocumentCache,
) -> Result<(), Error> {
    if req.handle_request::<GotoDefinition, _>(|params| {
        let result = token_descr(
            document_cache,
            params.text_document_position_params.text_document,
            params.text_document_position_params.position,
        )
        .and_then(|token| {
            if token.0.kind() == SyntaxKind::Comment {
                maybe_goto_preview(token.0, token.1, req.server_notifier());
                return None;
            }
            goto::goto_definition(document_cache, token.0)
        });
        Ok(result)
    })? {
    } else if req.handle_request::<Completion, _>(|params| {
        let result = token_descr(
            document_cache,
            params.text_document_position.text_document,
            params.text_document_position.position,
        )
        .and_then(|token| {
            completion::completion_at(
                document_cache,
                token.0,
                token.1,
                init_param.capabilities.text_document.as_ref().and_then(|t| t.completion.as_ref()),
            )
        });
        Ok(result)
    })? {
    } else if req.handle_request::<HoverRequest, _>(|_| {
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
        Ok(None::<Hover>)
    })? {
    } else if req.handle_request::<CodeActionRequest, _>(|params| {
        let result = token_descr(document_cache, params.text_document, params.range.start)
            .and_then(|token| get_code_actions(document_cache, token.0.parent()));
        Ok(result)
    })? {
    } else if req.handle_request::<ExecuteCommand, _>(|params| {
        if params.command.as_str() == SHOW_PREVIEW_COMMAND {
            show_preview_command(&params.arguments, &req.server_notifier(), document_cache)?
        }
        Ok(None::<serde_json::Value>)
    })? {
    } else if req.handle_request::<DocumentColor, _>(|params| {
        Ok(get_document_color(document_cache, &params.text_document).unwrap_or_default())
    })? {
    } else if req.handle_request::<ColorPresentationRequest, _>(|params| {
        // Convert the color from the color picker to a string representation. This could try to produce a minimal
        // representation.
        let requested_color = params.color;

        let color_literal = if requested_color.alpha < 1. {
            format!(
                "#{:0>2x}{:0>2x}{:0>2x}{:0>2x}",
                (requested_color.red * 255.) as u8,
                (requested_color.green * 255.) as u8,
                (requested_color.blue * 255.) as u8,
                (requested_color.alpha * 255.) as u8
            )
        } else {
            format!(
                "#{:0>2x}{:0>2x}{:0>2x}",
                (requested_color.red * 255.) as u8,
                (requested_color.green * 255.) as u8,
                (requested_color.blue * 255.) as u8,
            )
        };

        Ok(vec![ColorPresentation { label: color_literal, ..Default::default() }])
    })? {
    } else if req.handle_request::<DocumentSymbolRequest, _>(|params| {
        Ok(get_document_symbols(document_cache, &params.text_document))
    })? {
    } else if req.handle_request::<CodeLensRequest, _>(|params| {
        Ok(get_code_lenses(document_cache, &params.text_document))
    })? {
    } else if req.handle_request::<SemanticTokensFullRequest, _>(|params| {
        Ok(semantic_tokens::get_semantic_tokens(document_cache, &params.text_document))
    })? {
    };
    Ok(())
}

pub fn show_preview_command(
    params: &[serde_json::Value],
    connection: &crate::ServerNotifier,
    _document_cache: &DocumentCache,
) -> Result<(), Error> {
    let e = || -> Error { "InvalidParameter".into() };
    let path = if let serde_json::Value::String(s) = params.get(0).ok_or_else(e)? {
        std::path::PathBuf::from(s)
    } else {
        return Err(e());
    };
    let path_canon = dunce::canonicalize(&path).unwrap_or_else(|_| path.to_owned());
    let component = params.get(1).and_then(|v| v.as_str()).map(|v| v.to_string());
    preview::load_preview(
        connection.clone(),
        preview::PreviewComponent { path: path_canon, component },
        preview::PostLoadBehavior::ShowAfterLoad,
    );
    Ok(())
}

/// Workaround for editor that do not support code action: using the goto definition on a comment
/// that says "preview" will show the preview.
fn maybe_goto_preview(
    token: SyntaxToken,
    offset: u32,
    sender: crate::ServerNotifier,
) -> Option<()> {
    let text = token.text();
    let offset = offset.checked_sub(token.text_range().start().into())? as usize;
    if offset > text.len() || offset == 0 {
        return None;
    }
    let begin = text[..offset].rfind(|x: char| !x.is_ascii_alphanumeric())? + 1;
    let text = &text.as_bytes()[begin..];
    let rest = text.strip_prefix(b"preview").or_else(|| text.strip_prefix(b"PREVIEW"))?;
    if rest.get(0).map_or(true, |x| x.is_ascii_alphanumeric()) {
        return None;
    }

    // Ok, we were hovering on PREVIEW
    let mut node = token.parent();
    loop {
        if let Some(component) = syntax_nodes::Component::new(node.clone()) {
            let component_name =
                i_slint_compiler::parser::identifier_text(&component.DeclaredIdentifier())?;
            preview::load_preview(
                sender,
                preview::PreviewComponent {
                    path: token.source_file.path().into(),
                    component: Some(component_name),
                },
                preview::PostLoadBehavior::ShowAfterLoad,
            );
            return Some(());
        }
        node = node.parent()?;
    }
}

pub fn reload_document(
    connection: &crate::ServerNotifier,
    content: String,
    uri: lsp_types::Url,
    document_cache: &mut DocumentCache,
) -> Result<(), Error> {
    let newline_offsets = DocumentCache::newline_offsets_from_content(&content);
    document_cache.newline_offsets.insert(uri.clone(), newline_offsets);

    let path = uri.to_file_path().unwrap();
    let path_canon = dunce::canonicalize(&path).unwrap_or_else(|_| path.to_owned());
    preview::set_contents(&path_canon, content.clone());
    let mut diag = BuildDiagnostics::default();
    spin_on::spin_on(document_cache.documents.load_file(
        &path_canon,
        &path,
        content,
        false,
        &mut diag,
    ));

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
        lsp_diags.entry(uri).or_default().push(util::to_lsp_diag(&d));
    }

    for (uri, diagnostics) in lsp_diags {
        connection.send_notification(
            "textDocument/publishDiagnostics".into(),
            PublishDiagnosticsParams { uri, diagnostics, version: None },
        )?;
    }

    Ok(())
}

/// return the token, and the offset within the file
fn token_descr(
    document_cache: &mut DocumentCache,
    text_document: lsp_types::TextDocumentIdentifier,
    pos: Position,
) -> Option<(SyntaxToken, u32)> {
    let o = document_cache.newline_offsets.get(&text_document.uri)?.get(pos.line as usize)?
        + pos.character as u32;

    let doc = document_cache.documents.get_document(&text_document.uri.to_file_path().ok()?)?;
    let node = doc.node.as_ref()?;
    if !node.text_range().contains(o.into()) {
        return None;
    }
    let mut taf = node.token_at_offset(o.into());
    let token = match (taf.next(), taf.next()) {
        (None, _) => return None,
        (Some(t), None) => t,
        (Some(l), Some(r)) => match (l.kind(), r.kind()) {
            // Prioritize identifier
            (SyntaxKind::Identifier, _) => l,
            (_, SyntaxKind::Identifier) => r,
            // then the dot
            (SyntaxKind::Dot, _) => l,
            (_, SyntaxKind::Dot) => r,
            // de-prioritize the white spaces
            (SyntaxKind::Whitespace, _) => r,
            (SyntaxKind::Comment, _) => r,
            (_, SyntaxKind::Whitespace) => l,
            (_, SyntaxKind::Comment) => l,
            _ => l,
        },
    };
    Some((SyntaxToken { token, source_file: node.source_file.clone() }, o))
}

fn get_code_actions(
    _document_cache: &mut DocumentCache,
    node: SyntaxNode,
) -> Option<Vec<CodeActionOrCommand>> {
    let component = syntax_nodes::Component::new(node.clone())
        .or_else(|| {
            syntax_nodes::DeclaredIdentifier::new(node.clone())
                .and_then(|n| n.parent())
                .and_then(syntax_nodes::Component::new)
        })
        .or_else(|| {
            syntax_nodes::QualifiedName::new(node.clone())
                .and_then(|n| n.parent())
                .and_then(syntax_nodes::Element::new)
                .and_then(|n| n.parent())
                .and_then(syntax_nodes::Component::new)
        })?;

    let component_name =
        i_slint_compiler::parser::identifier_text(&component.DeclaredIdentifier())?;

    Some(vec![CodeActionOrCommand::Command(Command::new(
        "Show preview".into(),
        SHOW_PREVIEW_COMMAND.into(),
        Some(vec![component.source_file.path().to_string_lossy().into(), component_name.into()]),
    ))])
}

fn get_document_color(
    document_cache: &mut DocumentCache,
    text_document: &lsp_types::TextDocumentIdentifier,
) -> Option<Vec<ColorInformation>> {
    let mut result = Vec::new();
    let uri = &text_document.uri;
    let doc = document_cache.documents.get_document(&uri.to_file_path().ok()?)?;
    let root_node = &doc.node.as_ref()?.node;
    let mut token = root_node.first_token()?;
    loop {
        if token.kind() == SyntaxKind::ColorLiteral {
            (|| -> Option<()> {
                let range = token.text_range();
                let col = i_slint_compiler::literals::parse_color_literal(token.text())?;
                let shift = |s: u32| -> f32 { ((col >> s) & 0xff) as f32 / 255. };
                result.push(ColorInformation {
                    range: Range::new(
                        document_cache.byte_offset_to_position(range.start().into(), uri)?,
                        document_cache.byte_offset_to_position(range.end().into(), uri)?,
                    ),
                    color: Color {
                        alpha: shift(24),
                        red: shift(16),
                        green: shift(8),
                        blue: shift(0),
                    },
                });
                Some(())
            })();
        }
        token = match token.next_token() {
            Some(token) => token,
            None => break Some(result),
        }
    }
}

fn get_document_symbols(
    document_cache: &mut DocumentCache,
    text_document: &lsp_types::TextDocumentIdentifier,
) -> Option<DocumentSymbolResponse> {
    let uri = &text_document.uri;
    let doc = document_cache.documents.get_document(&uri.to_file_path().ok()?)?;

    // SymbolInformation doesn't implement default and some field depends on features or are deprecated
    let si: SymbolInformation = serde_json::from_value(
        serde_json::json!({ "name" : "", "kind": 255, "location" : Location::new(uri.clone(), Range::default()) })
    )
    .unwrap();

    let inner_components = doc.inner_components.clone();
    let inner_structs = doc.inner_structs.clone();
    let mut make_range = |node: &SyntaxNode| {
        let r = node.text_range();
        Some(Range::new(
            document_cache.byte_offset_to_position(r.start().into(), uri)?,
            document_cache.byte_offset_to_position(r.end().into(), uri)?,
        ))
    };

    let mut r = inner_components
        .iter()
        .filter_map(|c| {
            Some(SymbolInformation {
                location: Location::new(
                    uri.clone(),
                    make_range(c.root_element.borrow().node.as_ref()?)?,
                ),
                name: c.id.clone(),
                kind: lsp_types::SymbolKind::OBJECT,
                ..si.clone()
            })
        })
        .collect::<Vec<_>>();

    r.extend(inner_structs.iter().filter_map(|c| match c {
        Type::Struct { name: Some(name), node: Some(node), .. } => Some(SymbolInformation {
            location: Location::new(uri.clone(), make_range(node.parent().as_ref()?)?),
            name: name.clone(),
            kind: lsp_types::SymbolKind::STRUCT,
            ..si.clone()
        }),
        _ => None,
    }));

    Some(r.into())

    // TODO: add the structs
}

fn get_code_lenses(
    document_cache: &mut DocumentCache,
    text_document: &lsp_types::TextDocumentIdentifier,
) -> Option<Vec<CodeLens>> {
    let uri = &text_document.uri;
    let filepath = uri.to_file_path().ok()?;
    let doc = document_cache.documents.get_document(&filepath)?;

    let inner_components = doc.inner_components.clone();
    let mut make_range = |node: &SyntaxNode| {
        let r = node.text_range();
        Some(Range::new(
            document_cache.byte_offset_to_position(r.start().into(), uri)?,
            document_cache.byte_offset_to_position(r.end().into(), uri)?,
        ))
    };

    let r = inner_components
        .iter()
        .filter(|c| !c.is_global())
        .filter_map(|c| {
            Some(CodeLens {
                range: make_range(c.root_element.borrow().node.as_ref()?)?,
                command: Some(Command::new(
                    "▶ Show preview".into(),
                    SHOW_PREVIEW_COMMAND.into(),
                    Some(vec![filepath.to_str()?.into(), c.id.as_str().into()]),
                )),
                data: None,
            })
        })
        .collect::<Vec<_>>();
    Some(r)
}
