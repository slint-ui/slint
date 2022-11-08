// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore descr rfind

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;
use crate::{completion, goto, semantic_tokens, util, RequestHolder};
use i_slint_compiler::diagnostics::{BuildDiagnostics, Spanned};
use i_slint_compiler::langtype::Type;
use i_slint_compiler::object_tree::ElementRc;
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
    ColorInformation, ColorPresentation, Command, CompletionOptions, DocumentSymbol,
    DocumentSymbolResponse, Hover, InitializeParams, InitializeResult, OneOf, Position,
    PublishDiagnosticsParams, Range, SemanticTokensFullOptions, SemanticTokensLegend,
    SemanticTokensOptions, ServerCapabilities, ServerInfo, TextDocumentSyncCapability, Url,
    WorkDoneProgressOptions,
};
use std::collections::HashMap;

pub type Error = Box<dyn std::error::Error>;

const SHOW_PREVIEW_COMMAND: &str = "showPreview";
const QUERY_PROPERTIES_COMMAND: &str = "queryProperties";

fn command_list() -> Vec<String> {
    let mut result = vec![];

    #[cfg(any(feature = "preview", feature = "preview-lense"))]
    result.push(SHOW_PREVIEW_COMMAND.into());

    result.push(QUERY_PROPERTIES_COMMAND.into());

    result
}

fn create_show_preview_command(pretty: bool, file: &str, component_name: &str) -> Command {
    let title = format!("{}Show Preview", if pretty { &"▶ " } else { &"" });
    Command::new(title, SHOW_PREVIEW_COMMAND.into(), Some(vec![file.into(), component_name.into()]))
}

pub struct DocumentCache {
    pub(crate) documents: TypeLoader,
    newline_offsets: HashMap<Url, Vec<u32>>,
    versions: HashMap<Url, i32>,
}

impl DocumentCache {
    pub fn new(config: CompilerConfiguration) -> Self {
        let documents =
            TypeLoader::new(TypeRegister::builtin(), config, &mut BuildDiagnostics::default());
        Self { documents, newline_offsets: Default::default(), versions: Default::default() }
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

    pub fn document_version(&self, target_uri: &lsp_types::Url) -> Option<i32> {
        self.versions.get(target_uri).cloned()
    }

    pub fn byte_offset_to_position(
        &mut self,
        offset: u32,
        target_uri: &lsp_types::Url,
    ) -> Option<lsp_types::Position> {
        let newline_offsets = match self.newline_offsets.entry(target_uri.clone()) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                let path = target_uri.to_file_path().ok()?;
                let content =
                    self.documents.get_document(&path)?.node.as_ref()?.source_file()?.source()?;
                e.insert(Self::newline_offsets_from_content(content))
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

pub fn server_initialize_result() -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
            completion_provider: Some(CompletionOptions {
                resolve_provider: None,
                trigger_characters: Some(vec![".".to_owned()]),
                work_done_progress_options: WorkDoneProgressOptions::default(),
                all_commit_characters: None,
                completion_item: None,
            }),
            definition_provider: Some(OneOf::Left(true)),
            text_document_sync: Some(TextDocumentSyncCapability::Kind(
                lsp_types::TextDocumentSyncKind::FULL,
            )),
            code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
            execute_command_provider: Some(lsp_types::ExecuteCommandOptions {
                commands: command_list(),
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
        },
        server_info: Some(ServerInfo {
            name: env!("CARGO_PKG_NAME").to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
        offset_encoding: Some("utf-8".to_string()),
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
            &params.text_document_position_params.text_document.uri,
            &params.text_document_position_params.position,
        )
        .and_then(|token| {
            #[cfg(feature = "preview")]
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
            &params.text_document_position.text_document.uri,
            &params.text_document_position.position,
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
        let result = token_descr(document_cache, &params.text_document.uri, &params.range.start)
            .and_then(|token| get_code_actions(document_cache, token.0.parent()));
        Ok(result)
    })? {
    } else if req.handle_request::<ExecuteCommand, _>(|params| {
        #[cfg(any(feature = "preview", feature = "preview-lense"))]
        if params.command.as_str() == SHOW_PREVIEW_COMMAND {
            #[cfg(feature = "preview")]
            show_preview_command(&params.arguments, &req.server_notifier())?;
            return Ok(None::<serde_json::Value>);
        }
        if params.command.as_str() == QUERY_PROPERTIES_COMMAND {
            return Ok(Some(query_properties_command(
                &params.arguments,
                &req.server_notifier(),
                document_cache,
            )?));
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

#[cfg(feature = "preview")]
pub fn show_preview_command(
    params: &[serde_json::Value],
    connection: &crate::ServerNotifier,
) -> Result<(), Error> {
    use crate::preview;
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

pub fn query_properties_command(
    params: &[serde_json::Value],
    _connection: &crate::ServerNotifier,
    document_cache: &mut DocumentCache,
) -> Result<serde_json::Value, Error> {
    use crate::properties;

    let text_document_uri = Url::parse(
        params
            .get(0)
            .ok_or_else(|| -> Error { "No first parameter".into() })?
            .as_str()
            .ok_or_else(|| -> Error { "Failed to convert first parameter to string".into() })?,
    )?;
    let line = u32::try_from(
        params
            .get(1)
            .ok_or_else(|| -> Error { "No second parameter".into() })?
            .as_u64()
            .ok_or_else(|| -> Error { "Failed to convert second parameter to int".into() })?,
    )?;
    let character = u32::try_from(
        params
            .get(2)
            .ok_or_else(|| -> Error { "No third parameter.".into() })?
            .as_u64()
            .ok_or_else(|| -> Error { "Failed to convert third parameter to int".into() })?,
    )?;
    if let Some(element) =
        element_at_position(document_cache, &text_document_uri, &Position { line, character })
    {
        properties::query_properties(&element, &mut |offset| {
            document_cache
                .byte_offset_to_position(offset, &text_document_uri)
                .expect("invalid node offset")
        })
        .map(|r| serde_json::to_value(r).expect("Failed to serialize property query result!"))
    } else {
        Ok(serde_json::to_value(properties::QueryPropertyResponse::no_element_response(
            text_document_uri.to_string(),
        ))
        .expect("Failed to serialize none-element property query result!"))
    }
}

#[cfg(feature = "preview")]
/// Workaround for editor that do not support code action: using the goto definition on a comment
/// that says "preview" will show the preview.
fn maybe_goto_preview(
    token: SyntaxToken,
    offset: u32,
    sender: crate::ServerNotifier,
) -> Option<()> {
    use crate::preview;
    let text = token.text();
    let offset = offset.checked_sub(token.text_range().start().into())? as usize;
    if offset > text.len() || offset == 0 {
        return None;
    }
    let begin = text[..offset].rfind(|x: char| !x.is_ascii_alphanumeric())? + 1;
    let text = &text.as_bytes()[begin..];
    let rest = text.strip_prefix(b"preview").or_else(|| text.strip_prefix(b"PREVIEW"))?;
    if rest.first().map_or(true, |x| x.is_ascii_alphanumeric()) {
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

pub async fn reload_document_impl(
    content: String,
    uri: lsp_types::Url,
    version: i32,
    document_cache: &mut DocumentCache,
) -> Result<HashMap<Url, Vec<lsp_types::Diagnostic>>, Error> {
    let newline_offsets = DocumentCache::newline_offsets_from_content(&content);
    document_cache.newline_offsets.insert(uri.clone(), newline_offsets);
    document_cache.versions.insert(uri.clone(), version);

    let path = uri.to_file_path().unwrap();
    let path_canon = dunce::canonicalize(&path).unwrap_or_else(|_| path.to_owned());
    #[cfg(feature = "preview")]
    crate::preview::set_contents(&path_canon, content.clone());
    let mut diag = BuildDiagnostics::default();
    document_cache.documents.load_file(&path_canon, &path, content, false, &mut diag).await;

    // Always provide diagnostics for all files. Empty diagnostics clear any previous ones.
    let mut lsp_diags: HashMap<Url, Vec<lsp_types::Diagnostic>> = core::iter::once(&path)
        .chain(diag.all_loaded_files.iter())
        .map(|path| {
            let uri = Url::from_file_path(path).unwrap();
            (uri, Default::default())
        })
        .collect();

    for d in diag.into_iter() {
        #[cfg(not(target_arch = "wasm32"))]
        if d.source_file().unwrap().is_relative() {
            continue;
        }
        let uri = Url::from_file_path(d.source_file().unwrap()).unwrap();
        lsp_diags.entry(uri).or_default().push(util::to_lsp_diag(&d));
    }

    Ok(lsp_diags)
}

pub async fn reload_document(
    connection: &crate::ServerNotifier,
    content: String,
    uri: lsp_types::Url,
    version: i32,
    document_cache: &mut DocumentCache,
) -> Result<(), Error> {
    let lsp_diags = reload_document_impl(content, uri, version, document_cache).await?;

    for (uri, diagnostics) in lsp_diags {
        connection.send_notification(
            "textDocument/publishDiagnostics".into(),
            PublishDiagnosticsParams { uri, diagnostics, version: None },
        )?;
    }
    Ok(())
}

fn get_document_and_offset<'a>(
    document_cache: &'a mut DocumentCache,
    text_document_uri: &'a Url,
    pos: &'a Position,
) -> Option<(&'a i_slint_compiler::object_tree::Document, u32)> {
    let o = document_cache.newline_offsets.get(&text_document_uri)?.get(pos.line as usize)?
        + pos.character as u32;

    let doc = document_cache.documents.get_document(&text_document_uri.to_file_path().ok()?)?;
    doc.node.as_ref()?.text_range().contains(o.into()).then(|| (doc, o))
}

fn element_contains(element: &i_slint_compiler::object_tree::ElementRc, offset: u32) -> bool {
    element.borrow().node.as_ref().map_or(false, |n| n.text_range().contains(offset.into()))
}

pub fn element_at_position(
    document_cache: &mut DocumentCache,
    text_document_uri: &Url,
    pos: &Position,
) -> Option<i_slint_compiler::object_tree::ElementRc> {
    let (doc, offset) = get_document_and_offset(document_cache, text_document_uri, pos)?;

    for component in &doc.inner_components {
        let mut element = component.root_element.clone();
        while element_contains(&element, offset) {
            if let Some(c) =
                element.clone().borrow().children.iter().find(|c| element_contains(c, offset))
            {
                element = c.clone();
            } else {
                return Some(element);
            }
        }
    }
    None
}

/// return the token, and the offset within the file
fn token_descr(
    document_cache: &mut DocumentCache,
    text_document_uri: &Url,
    pos: &Position,
) -> Option<(SyntaxToken, u32)> {
    let (doc, o) = get_document_and_offset(document_cache, text_document_uri, pos)?;
    let node = doc.node.as_ref()?;

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
    if cfg!(feature = "preview-lense") {
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

        let mut r = vec![];

        // handle preview lense:
        r.push(CodeActionOrCommand::Command(create_show_preview_command(
            false,
            &component.source_file.path().to_string_lossy(),
            &component_name,
        )));

        Some(r)
    } else {
        None
    }
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

    // DocumentSymbol doesn't implement default and some field depends on features or are deprecated
    let ds: DocumentSymbol = serde_json::from_value(
        serde_json::json!({ "name" : "", "kind": 255, "range" : Range::default(), "selectionRange": Range::default() })
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
            Some(DocumentSymbol {
                range: make_range(c.root_element.borrow().node.as_ref()?)?,
                selection_range: make_range(
                    c.root_element.borrow().node.as_ref()?.QualifiedName().as_ref()?,
                )?,
                name: c.id.clone(),
                kind: if c.is_global() {
                    lsp_types::SymbolKind::OBJECT
                } else {
                    lsp_types::SymbolKind::CLASS
                },
                children: gen_children(&c.root_element, &ds, &mut make_range),
                ..ds.clone()
            })
        })
        .collect::<Vec<_>>();

    r.extend(inner_structs.iter().filter_map(|c| match c {
        Type::Struct { name: Some(name), node: Some(node), .. } => Some(DocumentSymbol {
            range: make_range(node.parent().as_ref()?)?,
            selection_range: make_range(node)?,
            name: name.clone(),
            kind: lsp_types::SymbolKind::STRUCT,
            ..ds.clone()
        }),
        _ => None,
    }));

    fn gen_children(
        elem: &ElementRc,
        ds: &DocumentSymbol,
        make_range: &mut dyn FnMut(&SyntaxNode) -> Option<Range>,
    ) -> Option<Vec<DocumentSymbol>> {
        let r = elem
            .borrow()
            .children
            .iter()
            .filter_map(|child| {
                let e = child.borrow();
                Some(DocumentSymbol {
                    range: make_range(e.node.as_ref()?)?,
                    selection_range: make_range(e.node.as_ref()?.QualifiedName().as_ref()?)?,
                    name: e.base_type.to_string(),
                    detail: (!e.id.is_empty()).then(|| e.id.clone()),
                    kind: lsp_types::SymbolKind::VARIABLE,
                    children: gen_children(child, ds, make_range),
                    ..ds.clone()
                })
            })
            .collect::<Vec<_>>();
        (!r.is_empty()).then(|| r)
    }

    Some(r.into())
}

fn get_code_lenses(
    document_cache: &mut DocumentCache,
    text_document: &lsp_types::TextDocumentIdentifier,
) -> Option<Vec<CodeLens>> {
    if cfg!(feature = "preview-lense") {
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

        let mut r = vec![];

        // Handle preview lense
        r.extend(inner_components.iter().filter(|c| !c.is_global()).filter_map(|c| {
            Some(CodeLens {
                range: make_range(c.root_element.borrow().node.as_ref()?)?,
                command: Some(create_show_preview_command(true, filepath.to_str()?, c.id.as_str())),
                data: None,
            })
        }));

        Some(r)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test::{complex_document_cache, loaded_document_cache};

    #[test]
    fn test_reload_document_invalid_contents() {
        let (_, url, diag) = loaded_document_cache("fluent", "This is not valid!".into());

        assert!(diag.len() == 1); // Only one URL is known

        let diagnostics = diag.get(&url).expect("URL not found in result");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(lsp_types::DiagnosticSeverity::ERROR));
    }

    #[test]
    fn test_reload_document_text_positions() {
        let (mut dc, url, _) = loaded_document_cache(
            "fluent",
            // cspell:disable-next-line
            "Thiß is not valid!\n and more...".into(),
        );

        assert_eq!(
            dc.byte_offset_to_position(0, &url),
            Some(lsp_types::Position { line: 0, character: 0 })
        );
        assert_eq!(
            dc.byte_offset_to_position(4, &url),
            Some(lsp_types::Position { line: 0, character: 4 })
        );
        assert_eq!(
            dc.byte_offset_to_position(5, &url),
            Some(lsp_types::Position { line: 0, character: 5 })
        ); // TODO: Figure out whether this is actually correct...
        assert_eq!(
            dc.byte_offset_to_position(1024, &url),
            Some(lsp_types::Position { line: 1, character: 1004 })
        ); // TODO: This is nonsense!
    }

    #[test]
    fn test_reload_document_valid_contents() {
        let (_, url, diag) = loaded_document_cache("fluent", r#"Main := Rectangle { }"#.into());

        assert!(diag.len() == 1); // Only one URL is known
        let diagnostics = diag.get(&url).expect("URL not found in result");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_text_document_color_no_color_set() {
        let (mut dc, url, _) = loaded_document_cache(
            "fluent",
            r#"
            Main := Rectangle { }
            "#
            .into(),
        );

        let result =
            get_document_color(&mut dc, &lsp_types::TextDocumentIdentifier { uri: url.clone() })
                .expect("Color Vec was returned");
        assert!(result.is_empty());
    }

    #[test]
    fn test_text_document_color_rgba_color() {
        let (mut dc, url, _) = loaded_document_cache(
            "fluent",
            r#"
            Main := Rectangle {
                background: #1200FF80;
            }
            "#
            .into(),
        );

        let result =
            get_document_color(&mut dc, &lsp_types::TextDocumentIdentifier { uri: url.clone() })
                .expect("Color Vec was returned");

        assert_eq!(result.len(), 1);

        let start = &result[0].range.start;
        assert_eq!(start.line, 2);
        assert_eq!(start.character, 28); // TODO: Why is this not 30?

        let end = &result[0].range.end;
        assert_eq!(end.line, 2);
        assert_eq!(end.character, 37); // TODO: Why is this not 39?

        let color = &result[0].color;
        assert_eq!(f64::trunc(color.red as f64 * 255.0), 18.0);
        assert_eq!(f64::trunc(color.green as f64 * 255.0), 0.0);
        assert_eq!(f64::trunc(color.blue as f64 * 255.0), 255.0);
        assert_eq!(f64::trunc(color.alpha as f64 * 255.0), 128.0);
    }

    fn id_at_position(
        dc: &mut DocumentCache,
        url: &Url,
        line: u32,
        character: u32,
    ) -> Option<String> {
        let result = element_at_position(dc, &url, &Position { line, character })?;
        let element = result.borrow();
        Some(element.id.clone())
    }

    fn base_type_at_position(
        dc: &mut DocumentCache,
        url: &Url,
        line: u32,
        character: u32,
    ) -> Option<String> {
        let result = element_at_position(dc, &url, &Position { line, character })?;
        let element = result.borrow();
        Some(format!("{}", &element.base_type))
    }

    #[test]
    fn test_element_at_position_no_element() {
        let (mut dc, url, _) = complex_document_cache("fluent");
        assert_eq!(id_at_position(&mut dc, &url, 0, 10), None);
        // TODO: This is past the end of the line and should thus return None
        assert_eq!(id_at_position(&mut dc, &url, 42, 90), Some(String::new()));
        assert_eq!(id_at_position(&mut dc, &url, 1, 0), None);
        assert_eq!(id_at_position(&mut dc, &url, 55, 1), None);
        assert_eq!(id_at_position(&mut dc, &url, 56, 5), None);
    }

    #[test]
    fn test_element_at_position_no_such_document() {
        let (mut dc, _, _) = complex_document_cache("fluent");
        assert_eq!(
            id_at_position(&mut dc, &Url::parse("https://foo.bar/baz").unwrap(), 5, 0),
            None
        );
    }

    #[test]
    fn test_element_at_position_root() {
        let (mut dc, url, _) = complex_document_cache("fluent");

        assert_eq!(id_at_position(&mut dc, &url, 2, 13), Some("root".to_string()));
        assert_eq!(id_at_position(&mut dc, &url, 2, 14), Some("root".to_string()));
        assert_eq!(id_at_position(&mut dc, &url, 2, 22), Some("root".to_string()));
        assert_eq!(id_at_position(&mut dc, &url, 3, 0), Some("root".to_string()));
        assert_eq!(id_at_position(&mut dc, &url, 3, 53), Some("root".to_string()));
        assert_eq!(id_at_position(&mut dc, &url, 4, 19), Some("root".to_string()));
        assert_eq!(id_at_position(&mut dc, &url, 5, 0), Some("root".to_string()));
        assert_eq!(id_at_position(&mut dc, &url, 6, 8), Some("root".to_string()));
        assert_eq!(id_at_position(&mut dc, &url, 6, 15), Some("root".to_string()));
        assert_eq!(id_at_position(&mut dc, &url, 6, 23), Some("root".to_string()));
        assert_eq!(id_at_position(&mut dc, &url, 8, 15), Some("root".to_string()));
        assert_eq!(id_at_position(&mut dc, &url, 12, 3), Some("root".to_string())); // right before child // TODO: Seems wrong!
        assert_eq!(id_at_position(&mut dc, &url, 51, 5), Some("root".to_string())); // right after child // TODO: Why does this not work?
        assert_eq!(id_at_position(&mut dc, &url, 52, 0), Some("root".to_string()));
    }

    #[test]
    fn test_element_at_position_child() {
        let (mut dc, url, _) = complex_document_cache("fluent");

        assert_eq!(base_type_at_position(&mut dc, &url, 12, 4), Some("VerticalBox".to_string()));
        assert_eq!(base_type_at_position(&mut dc, &url, 14, 22), Some("HorizontalBox".to_string()));
        assert_eq!(base_type_at_position(&mut dc, &url, 15, 33), Some("Text".to_string()));
        assert_eq!(base_type_at_position(&mut dc, &url, 27, 4), Some("VerticalBox".to_string()));
        assert_eq!(base_type_at_position(&mut dc, &url, 28, 8), Some("Text".to_string()));
        assert_eq!(base_type_at_position(&mut dc, &url, 51, 4), Some("VerticalBox".to_string()));
    }
}
