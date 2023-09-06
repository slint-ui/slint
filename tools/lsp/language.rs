// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore descr rfind

mod completion;
mod goto;
mod properties;
mod semantic_tokens;
#[cfg(test)]
mod test;

use crate::common::{PreviewApi, Result};
use crate::util::{map_node, map_range, map_token, to_lsp_diag};

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::parser::{syntax_nodes, NodeOrToken, SyntaxKind, SyntaxNode, SyntaxToken};
use i_slint_compiler::CompilerConfiguration;
use i_slint_compiler::{diagnostics::BuildDiagnostics, langtype::Type};
use i_slint_compiler::{typeloader::TypeLoader, typeregister::TypeRegister};
use lsp_types::request::{
    CodeActionRequest, CodeLensRequest, ColorPresentationRequest, Completion, DocumentColor,
    DocumentHighlightRequest, DocumentSymbolRequest, ExecuteCommand, GotoDefinition, HoverRequest,
    PrepareRenameRequest, Rename, SemanticTokensFullRequest,
};
use lsp_types::{
    ClientCapabilities, CodeActionOrCommand, CodeActionProviderCapability, CodeLens,
    CodeLensOptions, Color, ColorInformation, ColorPresentation, Command, CompletionOptions,
    DocumentSymbol, DocumentSymbolResponse, Hover, InitializeParams, InitializeResult, OneOf,
    Position, PrepareRenameResponse, PublishDiagnosticsParams, RenameOptions,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextEdit, Url, WorkDoneProgressOptions, WorkspaceEdit,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;

const QUERY_PROPERTIES_COMMAND: &str = "slint/queryProperties";
const REMOVE_BINDING_COMMAND: &str = "slint/removeBinding";
const SHOW_PREVIEW_COMMAND: &str = "slint/showPreview";
const SET_BINDING_COMMAND: &str = "slint/setBinding";
const SET_DESIGN_MODE_COMMAND: &str = "slint/setDesignMode";
const TOGGLE_DESIGN_MODE_COMMAND: &str = "slint/toggleDesignMode";

pub fn uri_to_file(uri: &lsp_types::Url) -> Option<PathBuf> {
    let Ok(path) = uri.to_file_path() else { return None };
    let path_canon = dunce::canonicalize(&path).unwrap_or_else(|_| path.to_owned());
    Some(path_canon)
}

fn command_list() -> Vec<String> {
    vec![
        QUERY_PROPERTIES_COMMAND.into(),
        REMOVE_BINDING_COMMAND.into(),
        #[cfg(any(feature = "preview", feature = "preview-lense"))]
        SHOW_PREVIEW_COMMAND.into(),
        #[cfg(any(feature = "preview", feature = "preview-lense"))]
        SET_DESIGN_MODE_COMMAND.into(),
        SET_BINDING_COMMAND.into(),
        #[cfg(any(feature = "preview", feature = "preview-lense"))]
        TOGGLE_DESIGN_MODE_COMMAND.into(),
    ]
}

fn create_show_preview_command(
    pretty: bool,
    file: &lsp_types::Url,
    component_name: &str,
) -> Command {
    let title = format!("{}Show Preview", if pretty { &"▶ " } else { &"" });
    Command::new(
        title,
        SHOW_PREVIEW_COMMAND.into(),
        Some(vec![file.as_str().into(), component_name.into()]),
    )
}

/// A cache of loaded documents
pub struct DocumentCache {
    pub(crate) documents: TypeLoader,
    versions: HashMap<Url, i32>,
}

impl DocumentCache {
    pub fn new(config: CompilerConfiguration) -> Self {
        let documents =
            TypeLoader::new(TypeRegister::builtin(), config, &mut BuildDiagnostics::default());
        Self { documents, versions: Default::default() }
    }

    pub fn document_version(&self, target_uri: &lsp_types::Url) -> Option<i32> {
        self.versions.get(target_uri).cloned()
    }
}

pub struct Context {
    pub document_cache: RefCell<DocumentCache>,
    pub server_notifier: crate::ServerNotifier,
    pub init_param: InitializeParams,
    pub preview: Box<dyn PreviewApi>,
}

#[derive(Default)]
pub struct RequestHandler(
    pub  HashMap<
        &'static str,
        Box<
            dyn Fn(
                serde_json::Value,
                Rc<Context>,
            ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value>>>>,
        >,
    >,
);

impl RequestHandler {
    pub fn register<
        R: lsp_types::request::Request,
        Fut: Future<Output = Result<R::Result>> + 'static,
    >(
        &mut self,
        handler: fn(R::Params, Rc<Context>) -> Fut,
    ) where
        R::Params: 'static,
    {
        self.0.insert(
            R::METHOD,
            Box::new(move |value, ctx| {
                Box::pin(async move {
                    let params = serde_json::from_value(value)
                        .map_err(|e| format!("error when deserializing request: {e:?}"))?;
                    handler(params, ctx).await.map(|x| serde_json::to_value(x).unwrap())
                })
            }),
        );
    }
}

pub fn server_initialize_result(client_cap: &ClientCapabilities) -> InitializeResult {
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
            document_highlight_provider: Some(OneOf::Left(true)),
            rename_provider: Some(
                if client_cap
                    .text_document
                    .as_ref()
                    .and_then(|td| td.rename.as_ref())
                    .and_then(|r| r.prepare_support)
                    .unwrap_or(false)
                {
                    OneOf::Right(RenameOptions {
                        prepare_provider: Some(true),
                        work_done_progress_options: WorkDoneProgressOptions::default(),
                    })
                } else {
                    OneOf::Left(true)
                },
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

pub fn register_request_handlers(rh: &mut RequestHandler) {
    rh.register::<GotoDefinition, _>(|params, ctx| async move {
        let document_cache = &mut ctx.document_cache.borrow_mut();
        let result = token_descr(
            document_cache,
            &params.text_document_position_params.text_document.uri,
            &params.text_document_position_params.position,
        )
        .and_then(|token| goto::goto_definition(document_cache, token.0));
        Ok(result)
    });
    rh.register::<Completion, _>(|params, ctx| async move {
        let document_cache = &mut ctx.document_cache.borrow_mut();

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
                ctx.init_param
                    .capabilities
                    .text_document
                    .as_ref()
                    .and_then(|t| t.completion.as_ref()),
            )
            .map(Into::into)
        });
        Ok(result)
    });
    rh.register::<HoverRequest, _>(|_params, _ctx| async move {
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
    });
    rh.register::<CodeActionRequest, _>(|params, ctx| async move {
        let document_cache = &mut ctx.document_cache.borrow_mut();

        let result = token_descr(document_cache, &params.text_document.uri, &params.range.start)
            .and_then(|(token, _)| get_code_actions(document_cache, token));
        Ok(result)
    });
    rh.register::<ExecuteCommand, _>(|params, ctx| async move {
        if params.command.as_str() == SHOW_PREVIEW_COMMAND {
            #[cfg(feature = "preview")]
            show_preview_command(&params.arguments, &ctx)?;
            return Ok(None::<serde_json::Value>);
        }
        if params.command.as_str() == SET_DESIGN_MODE_COMMAND {
            #[cfg(feature = "preview")]
            set_design_mode(&params.arguments, &ctx)?;
            return Ok(None::<serde_json::Value>);
        }
        if params.command.as_str() == TOGGLE_DESIGN_MODE_COMMAND {
            #[cfg(feature = "preview")]
            toggle_design_mode(&params.arguments, &ctx)?;
            return Ok(None::<serde_json::Value>);
        }
        if params.command.as_str() == QUERY_PROPERTIES_COMMAND {
            return Ok(Some(query_properties_command(&params.arguments, &ctx)?));
        }
        if params.command.as_str() == SET_BINDING_COMMAND {
            return Ok(Some(set_binding_command(&params.arguments, &ctx).await?));
        }
        if params.command.as_str() == REMOVE_BINDING_COMMAND {
            return Ok(Some(remove_binding_command(&params.arguments, &ctx).await?));
        }
        Ok(None::<serde_json::Value>)
    });
    rh.register::<DocumentColor, _>(|params, ctx| async move {
        let document_cache = &mut ctx.document_cache.borrow_mut();
        Ok(get_document_color(document_cache, &params.text_document).unwrap_or_default())
    });
    rh.register::<ColorPresentationRequest, _>(|params, _ctx| async move {
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
    });
    rh.register::<DocumentSymbolRequest, _>(|params, ctx| async move {
        let document_cache = &mut ctx.document_cache.borrow_mut();
        Ok(get_document_symbols(document_cache, &params.text_document))
    });
    rh.register::<CodeLensRequest, _>(|params, ctx| async move {
        let document_cache = &mut ctx.document_cache.borrow_mut();
        Ok(get_code_lenses(document_cache, &params.text_document))
    });
    rh.register::<SemanticTokensFullRequest, _>(|params, ctx| async move {
        let document_cache = &mut ctx.document_cache.borrow_mut();
        Ok(semantic_tokens::get_semantic_tokens(document_cache, &params.text_document))
    });
    rh.register::<DocumentHighlightRequest, _>(|_params, ctx| async move {
        let document_cache = &mut ctx.document_cache.borrow_mut();
        let uri = _params.text_document_position_params.text_document.uri;
        if let Some((tk, _off)) =
            token_descr(document_cache, &uri, &_params.text_document_position_params.position)
        {
            let p = tk.parent();
            if p.kind() == SyntaxKind::QualifiedName
                && p.parent().map_or(false, |n| n.kind() == SyntaxKind::Element)
            {
                if let Some(range) = map_node(&p) {
                    ctx.preview.highlight(uri_to_file(&uri), _off)?;
                    return Ok(Some(vec![lsp_types::DocumentHighlight { range, kind: None }]));
                }
            }

            if let Some(value) = find_element_id_for_highlight(&tk, &p) {
                ctx.preview.highlight(None, 0)?;
                return Ok(Some(
                    value
                        .into_iter()
                        .map(|r| lsp_types::DocumentHighlight {
                            range: map_range(&p.source_file, r),
                            kind: None,
                        })
                        .collect(),
                ));
            }
        }
        ctx.preview.highlight(None, 0)?;
        Ok(None)
    });
    rh.register::<Rename, _>(|params, ctx| async move {
        let mut document_cache = ctx.document_cache.borrow_mut();
        let uri = params.text_document_position.text_document.uri;
        if let Some((tk, _off)) =
            token_descr(&mut document_cache, &uri, &params.text_document_position.position)
        {
            let p = tk.parent();
            if let Some(value) = find_element_id_for_highlight(&tk, &tk.parent()) {
                let edits = value
                    .into_iter()
                    .map(|r| TextEdit {
                        range: map_range(&p.source_file, r),
                        new_text: params.new_name.clone(),
                    })
                    .collect();
                return Ok(Some(WorkspaceEdit {
                    changes: Some(std::iter::once((uri, edits)).collect()),
                    ..Default::default()
                }));
            }
        };
        Err("This symbol cannot be renamed. (Only element id can be renamed at the moment)".into())
    });
    rh.register::<PrepareRenameRequest, _>(|params, ctx| async move {
        let mut document_cache = ctx.document_cache.borrow_mut();
        let uri = params.text_document.uri;
        if let Some((tk, _off)) = token_descr(&mut document_cache, &uri, &params.position) {
            if find_element_id_for_highlight(&tk, &tk.parent()).is_some() {
                return Ok(map_token(&tk).map(PrepareRenameResponse::Range));
            }
        };
        Ok(None)
    });
}

#[cfg(feature = "preview")]
pub fn show_preview_command(params: &[serde_json::Value], ctx: &Rc<Context>) -> Result<()> {
    let document_cache = &mut ctx.document_cache.borrow_mut();
    let config = &document_cache.documents.compiler_config;

    let e = || "InvalidParameter";

    let url = if let serde_json::Value::String(s) = params.get(0).ok_or_else(e)? {
        Url::parse(s)?
    } else {
        return Err(e().into());
    };
    let component =
        params.get(1).and_then(|v| v.as_str()).filter(|v| !v.is_empty()).map(|v| v.to_string());
    let path = uri_to_file(&url).unwrap_or_default();

    ctx.preview.load_preview(
        crate::common::PreviewComponent {
            path,
            component,
            include_paths: config.include_paths.clone(),
            style: config.style.clone().unwrap_or_default(),
        },
        crate::common::PostLoadBehavior::ShowAfterLoad,
    );
    Ok(())
}

#[cfg(feature = "preview")]
pub fn set_design_mode(params: &[serde_json::Value], ctx: &Rc<Context>) -> Result<()> {
    let e = || "InvalidParameter";
    let enable = if let serde_json::Value::Bool(b) = params.get(0).ok_or_else(e)? {
        b
    } else {
        return Err(e().into());
    };

    ctx.preview.set_design_mode(*enable);
    Ok(())
}

#[cfg(feature = "preview")]
pub fn toggle_design_mode(_params: &[serde_json::Value], ctx: &Rc<Context>) -> Result<()> {
    ctx.preview.set_design_mode(!ctx.preview.design_mode());
    Ok(())
}

pub fn query_properties_command(
    params: &[serde_json::Value],
    ctx: &Rc<Context>,
) -> Result<serde_json::Value> {
    let document_cache = &mut ctx.document_cache.borrow_mut();

    let text_document_uri = serde_json::from_value::<lsp_types::TextDocumentIdentifier>(
        params.get(0).ok_or("No text document provided")?.clone(),
    )?
    .uri;
    let position = serde_json::from_value::<lsp_types::Position>(
        params.get(1).ok_or("No position provided")?.clone(),
    )?;

    let source_version = if let Some(v) = document_cache.document_version(&text_document_uri) {
        v
    } else {
        return Ok(serde_json::to_value(properties::QueryPropertyResponse::no_element_response(
            text_document_uri.to_string(),
            -1,
        ))
        .expect("Failed to serialize none-element property query result!"));
    };

    if let Some(element) = element_at_position(document_cache, &text_document_uri, &position) {
        properties::query_properties(&text_document_uri, source_version, &element)
            .map(|r| serde_json::to_value(r).expect("Failed to serialize property query result!"))
    } else {
        Ok(serde_json::to_value(properties::QueryPropertyResponse::no_element_response(
            text_document_uri.to_string(),
            source_version,
        ))
        .expect("Failed to serialize none-element property query result!"))
    }
}

pub async fn set_binding_command(
    params: &[serde_json::Value],
    ctx: &Rc<Context>,
) -> Result<serde_json::Value> {
    let text_document = serde_json::from_value::<lsp_types::OptionalVersionedTextDocumentIdentifier>(
        params.get(0).ok_or("No text document provided")?.clone(),
    )?;
    let element_range = serde_json::from_value::<lsp_types::Range>(
        params.get(1).ok_or("No element range provided")?.clone(),
    )?;
    let property_name = serde_json::from_value::<String>(
        params.get(2).ok_or("No property name provided")?.clone(),
    )?;
    let new_expression =
        serde_json::from_value::<String>(params.get(3).ok_or("No expression provided")?.clone())?;
    let dry_run = {
        if let Some(p) = params.get(4) {
            serde_json::from_value::<bool>(p.clone())
        } else {
            Ok(true)
        }
    }?;

    let (result, edit) = {
        let document_cache = &mut ctx.document_cache.borrow_mut();
        let uri = text_document.uri;
        if let Some(source_version) = text_document.version {
            if let Some(current_version) = document_cache.document_version(&uri) {
                if current_version != source_version {
                    return Err(
                        "Document version mismatch. Please refresh your property information"
                            .into(),
                    );
                }
            } else {
                return Err(format!("Document with uri {uri} not found in cache").into());
            }
        }

        let element =
            element_at_position(document_cache, &uri, &element_range.start).ok_or_else(|| {
                format!("No element found at the given start position {:?}", &element_range.start)
            })?;

        let node_range = map_node(
            element
                .borrow()
                .node
                .as_ref()
                .ok_or("The element was found, but had no range defined!")?,
        )
        .ok_or("Failed to map node")?;

        if node_range.start != element_range.start {
            return Err(format!(
                "Element found, but does not start at the expected place (){:?} != {:?}).",
                node_range.start, element_range.start
            )
            .into());
        }
        if node_range.end != element_range.end {
            return Err(format!(
                "Element found, but does not end at the expected place (){:?} != {:?}).",
                node_range.end, element_range.end
            )
            .into());
        }

        properties::set_binding(document_cache, &uri, &element, &property_name, new_expression)?
    };

    if !dry_run {
        if let Some(edit) = edit {
            let response = ctx
                .server_notifier
                .send_request::<lsp_types::request::ApplyWorkspaceEdit>(
                    lsp_types::ApplyWorkspaceEditParams { label: Some("set binding".into()), edit },
                )?
                .await?;
            if !response.applied {
                return Err(response
                    .failure_reason
                    .unwrap_or("Operation failed, no specific reason given".into())
                    .into());
            }
        }
    }

    Ok(serde_json::to_value(result).expect("Failed to serialize set_binding result!"))
}

pub async fn remove_binding_command(
    params: &[serde_json::Value],
    ctx: &Rc<Context>,
) -> Result<serde_json::Value> {
    let text_document = serde_json::from_value::<lsp_types::OptionalVersionedTextDocumentIdentifier>(
        params.get(0).ok_or("No text document provided")?.clone(),
    )?;
    let element_range = serde_json::from_value::<lsp_types::Range>(
        params.get(1).ok_or("No element range provided")?.clone(),
    )?;
    let property_name = serde_json::from_value::<String>(
        params.get(2).ok_or("No property name provided")?.clone(),
    )?;

    let edit = {
        let document_cache = &mut ctx.document_cache.borrow_mut();
        let uri = text_document.uri;

        if let Some(source_version) = text_document.version {
            if let Some(current_version) = document_cache.document_version(&uri) {
                if current_version != source_version {
                    return Err(
                        "Document version mismatch. Please refresh your property information"
                            .into(),
                    );
                }
            } else {
                return Err(format!("Document with uri {uri} not found in cache").into());
            }
        }

        let element =
            element_at_position(document_cache, &uri, &element_range.start).ok_or_else(|| {
                format!("No element found at the given start position {:?}", &element_range.start)
            })?;

        let node_range = map_node(
            element
                .borrow()
                .node
                .as_ref()
                .ok_or("The element was found, but had no range defined!")?,
        )
        .ok_or("Failed to map node")?;

        if node_range.start != element_range.start {
            return Err(format!(
                "Element found, but does not start at the expected place (){:?} != {:?}).",
                node_range.start, element_range.start
            )
            .into());
        }
        if node_range.end != element_range.end {
            return Err(format!(
                "Element found, but does not end at the expected place (){:?} != {:?}).",
                node_range.end, element_range.end
            )
            .into());
        }

        properties::remove_binding(document_cache, &uri, &element, &property_name)?
    };

    let response = ctx
        .server_notifier
        .send_request::<lsp_types::request::ApplyWorkspaceEdit>(
            lsp_types::ApplyWorkspaceEditParams { label: Some("set binding".into()), edit },
        )?
        .await?;

    if !response.applied {
        return Err(response
            .failure_reason
            .unwrap_or("Operation failed, no specific reason given".into())
            .into());
    }

    Ok(serde_json::to_value(()).expect("Failed to serialize ()!"))
}

pub(crate) async fn reload_document_impl(
    ctx: Option<&Rc<Context>>,
    mut content: String,
    uri: lsp_types::Url,
    version: i32,
    document_cache: &mut DocumentCache,
) -> HashMap<Url, Vec<lsp_types::Diagnostic>> {
    let Some(path) = uri_to_file(&uri) else { return Default::default() };
    if path.extension().map_or(false, |e| e == "rs") {
        content = match i_slint_compiler::lexer::extract_rust_macro(content) {
            Some(content) => content,
            // A rust file without a rust macro, just ignore it
            None => return [(uri, vec![])].into_iter().collect(),
        };
    }

    document_cache.versions.insert(uri.clone(), version);

    if let Some(ctx) = ctx {
        ctx.preview.set_contents(&path, &content);
    }
    let mut diag = BuildDiagnostics::default();
    document_cache.documents.load_file(&path, &path, content, false, &mut diag).await;

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
        lsp_diags.entry(uri).or_default().push(to_lsp_diag(&d));
    }

    lsp_diags
}

pub async fn reload_document(
    ctx: &Rc<Context>,
    content: String,
    uri: lsp_types::Url,
    version: i32,
    document_cache: &mut DocumentCache,
) -> Result<()> {
    let lsp_diags = reload_document_impl(Some(ctx), content, uri, version, document_cache).await;

    for (uri, diagnostics) in lsp_diags {
        ctx.server_notifier.send_notification(
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
    let path = uri_to_file(&text_document_uri)?;
    let doc = document_cache.documents.get_document(&path)?;
    let o = doc.node.as_ref()?.source_file.offset(pos.line as usize + 1, pos.character as usize + 1)
        as u32;
    doc.node.as_ref()?.text_range().contains_inclusive(o.into()).then_some((doc, o))
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

    let token = token_at_offset(node, o)?;
    Some((token, o))
}

/// Return the token that matches best the token at cursor position
pub fn token_at_offset(doc: &syntax_nodes::Document, offset: u32) -> Option<SyntaxToken> {
    let mut taf = doc.token_at_offset(offset.into());
    let token = match (taf.next(), taf.next()) {
        (None, _) => doc.last_token()?,
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
    Some(SyntaxToken { token, source_file: doc.source_file.clone() })
}

fn get_code_actions(
    _document_cache: &mut DocumentCache,
    token: SyntaxToken,
) -> Option<Vec<CodeActionOrCommand>> {
    let node = token.parent();
    let uri = Url::from_file_path(token.source_file.path()).ok()?;
    let mut result = vec![];
    #[cfg(feature = "preview-lense")]
    {
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
            });
        if let Some(component) = component {
            if let Some(component_name) =
                i_slint_compiler::parser::identifier_text(&component.DeclaredIdentifier())
            {
                result.push(CodeActionOrCommand::Command(create_show_preview_command(
                    false,
                    &uri,
                    &component_name,
                )))
            }
        }
    }

    if token.kind() == SyntaxKind::StringLiteral && node.kind() == SyntaxKind::Expression {
        let r = map_range(&token.source_file, node.text_range());
        let edits = vec![
            TextEdit::new(lsp_types::Range::new(r.start, r.start), "@tr(".into()),
            TextEdit::new(lsp_types::Range::new(r.end, r.end), ")".into()),
        ];
        result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
            title: "Wrap in `@tr()`".into(),
            edit: Some(WorkspaceEdit {
                changes: Some(std::iter::once((uri, edits)).collect()),
                ..Default::default()
            }),
            ..Default::default()
        }));
    }

    (!result.is_empty()).then_some(result)
}

fn get_document_color(
    document_cache: &mut DocumentCache,
    text_document: &lsp_types::TextDocumentIdentifier,
) -> Option<Vec<ColorInformation>> {
    let mut result = Vec::new();
    let uri_path = uri_to_file(&text_document.uri)?;
    let doc = document_cache.documents.get_document(&uri_path)?;
    let root_node = doc.node.as_ref()?;
    let mut token = root_node.first_token()?;
    loop {
        if token.kind() == SyntaxKind::ColorLiteral {
            (|| -> Option<()> {
                let range = map_token(&token)?;
                let col = i_slint_compiler::literals::parse_color_literal(token.text())?;
                let shift = |s: u32| -> f32 { ((col >> s) & 0xff) as f32 / 255. };
                result.push(ColorInformation {
                    range,
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
    let uri_path = uri_to_file(&text_document.uri)?;
    let doc = document_cache.documents.get_document(&uri_path)?;

    // DocumentSymbol doesn't implement default and some field depends on features or are deprecated
    let ds: DocumentSymbol = serde_json::from_value(
        serde_json::json!({ "name" : "", "kind": 255, "range" : lsp_types::Range::default(), "selectionRange": lsp_types::Range::default() })
    )
    .unwrap();

    let inner_components = doc.inner_components.clone();
    let inner_types = doc.inner_types.clone();

    let mut r = inner_components
        .iter()
        .filter_map(|c| {
            let root_element = c.root_element.borrow();
            let element_node = root_element.node.as_ref()?;
            let component_node = syntax_nodes::Component::new(element_node.parent()?)?;
            let selection_range = map_node(&component_node.DeclaredIdentifier())?;

            Some(DocumentSymbol {
                range: map_node(&component_node)?,
                selection_range,
                name: c.id.clone(),
                kind: if c.is_global() {
                    lsp_types::SymbolKind::OBJECT
                } else {
                    lsp_types::SymbolKind::CLASS
                },
                children: gen_children(&c.root_element, &ds),
                ..ds.clone()
            })
        })
        .collect::<Vec<_>>();

    r.extend(inner_types.iter().filter_map(|c| match c {
        Type::Struct { name: Some(name), node: Some(node), .. } => Some(DocumentSymbol {
            range: map_node(node.parent().as_ref()?)?,
            selection_range: map_node(node)?,
            name: name.clone(),
            kind: lsp_types::SymbolKind::STRUCT,
            ..ds.clone()
        }),
        Type::Enumeration(enumeration) => enumeration.node.as_ref().and_then(|node| {
            Some(DocumentSymbol {
                range: map_node(node.parent().as_ref()?)?,
                selection_range: map_node(node)?,
                name: enumeration.name.clone(),
                kind: lsp_types::SymbolKind::ENUM,
                ..ds.clone()
            })
        }),
        _ => None,
    }));

    fn gen_children(elem: &ElementRc, ds: &DocumentSymbol) -> Option<Vec<DocumentSymbol>> {
        let r = elem
            .borrow()
            .children
            .iter()
            .filter_map(|child| {
                let e = child.borrow();
                Some(DocumentSymbol {
                    range: map_node(e.node.as_ref()?)?,
                    selection_range: map_node(e.node.as_ref()?.QualifiedName().as_ref()?)?,
                    name: e.base_type.to_string(),
                    detail: (!e.id.is_empty()).then(|| e.id.clone()),
                    kind: lsp_types::SymbolKind::VARIABLE,
                    children: gen_children(child, ds),
                    ..ds.clone()
                })
            })
            .collect::<Vec<_>>();
        (!r.is_empty()).then_some(r)
    }

    r.sort_by(|a, b| {
        if a.range.start.line.cmp(&b.range.start.line) == std::cmp::Ordering::Less {
            std::cmp::Ordering::Less
        } else if a.range.start.line.cmp(&b.range.start.line) == std::cmp::Ordering::Equal {
            a.range.start.character.cmp(&b.range.start.character)
        } else {
            std::cmp::Ordering::Greater
        }
    });

    Some(r.into())
}

fn get_code_lenses(
    document_cache: &mut DocumentCache,
    text_document: &lsp_types::TextDocumentIdentifier,
) -> Option<Vec<CodeLens>> {
    if cfg!(feature = "preview-lense") {
        let filepath = uri_to_file(&text_document.uri)?;
        let doc = document_cache.documents.get_document(&filepath)?;

        let inner_components = doc.inner_components.clone();

        let mut r = vec![];

        // Handle preview lens
        r.extend(inner_components.iter().filter(|c| !c.is_global()).filter_map(|c| {
            Some(CodeLens {
                range: map_node(c.root_element.borrow().node.as_ref()?)?,
                command: Some(create_show_preview_command(true, &text_document.uri, c.id.as_str())),
                data: None,
            })
        }));

        Some(r)
    } else {
        None
    }
}

/// If the token is matching a Element ID, return the list of all element id in the same component
fn find_element_id_for_highlight(
    token: &SyntaxToken,
    parent: &SyntaxNode,
) -> Option<Vec<rowan::TextRange>> {
    fn is_element_id(tk: &SyntaxToken, parent: &SyntaxNode) -> bool {
        if tk.kind() != SyntaxKind::Identifier {
            return false;
        }
        if parent.kind() == SyntaxKind::SubElement {
            return true;
        };
        if parent.kind() == SyntaxKind::QualifiedName
            && matches!(
                parent.parent().map(|n| n.kind()),
                Some(SyntaxKind::Expression | SyntaxKind::StatePropertyChange)
            )
        {
            let mut c = parent.children_with_tokens();
            if let Some(NodeOrToken::Token(first)) = c.next() {
                return first.text_range() == tk.text_range()
                    && matches!(c.next(), Some(NodeOrToken::Token(second)) if second.kind() == SyntaxKind::Dot);
            }
        }

        false
    }
    if is_element_id(token, parent) {
        // An id: search all use of the id in this Component
        let mut candidate = parent.parent();
        while let Some(c) = candidate {
            if c.kind() == SyntaxKind::Component {
                let mut ranges = Vec::new();
                let mut found_definition = false;
                recurse(&mut ranges, &mut found_definition, c, token.text());
                fn recurse(
                    ranges: &mut Vec<rowan::TextRange>,
                    found_definition: &mut bool,
                    c: SyntaxNode,
                    text: &str,
                ) {
                    for x in c.children_with_tokens() {
                        match x {
                            NodeOrToken::Node(n) => recurse(ranges, found_definition, n, text),
                            NodeOrToken::Token(tk) => {
                                if is_element_id(&tk, &c) && tk.text() == text {
                                    ranges.push(tk.text_range());
                                    if c.kind() == SyntaxKind::SubElement {
                                        *found_definition = true;
                                    }
                                }
                            }
                        }
                    }
                }
                if !found_definition {
                    return None;
                }
                return Some(ranges);
            }
            candidate = c.parent()
        }
    }
    None
}

pub async fn load_configuration(ctx: &Context) -> Result<()> {
    if !ctx
        .init_param
        .capabilities
        .workspace
        .as_ref()
        .and_then(|w| w.configuration)
        .unwrap_or(false)
    {
        return Ok(());
    }

    let r = ctx
        .server_notifier
        .send_request::<lsp_types::request::WorkspaceConfiguration>(
            lsp_types::ConfigurationParams {
                items: vec![lsp_types::ConfigurationItem {
                    scope_uri: None,
                    section: Some("slint".into()),
                }],
            },
        )?
        .await?;

    let document_cache = &mut ctx.document_cache.borrow_mut();
    for v in r {
        if let Some(o) = v.as_object() {
            if let Some(ip) = o.get("includePath").and_then(|v| v.as_array()) {
                if !ip.is_empty() {
                    document_cache.documents.compiler_config.include_paths =
                        ip.iter().filter_map(|x| x.as_str()).map(PathBuf::from).collect();
                }
            }
            if let Some(style) =
                o.get("preview").and_then(|v| v.as_object()?.get("style")?.as_str())
            {
                if !style.is_empty() {
                    document_cache.documents.compiler_config.style = Some(style.into());
                }
            }
        }
    }

    // Always load the widgets so we can auto-complete them
    let mut diag = BuildDiagnostics::default();
    document_cache.documents.import_component("std-widgets.slint", "StyleMetrics", &mut diag).await;

    let cc = &document_cache.documents.compiler_config;
    let empty_string = String::new();
    ctx.preview.config_changed(&cc.style.as_ref().unwrap_or(&empty_string), &cc.include_paths);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use test::{complex_document_cache, loaded_document_cache};

    #[test]
    fn test_reload_document_invalid_contents() {
        let (_, url, diag) = loaded_document_cache("This is not valid!".into());

        assert!(diag.len() == 1); // Only one URL is known

        let diagnostics = diag.get(&url).expect("URL not found in result");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(lsp_types::DiagnosticSeverity::ERROR));
    }

    #[test]
    fn test_reload_document_valid_contents() {
        let (_, url, diag) =
            loaded_document_cache(r#"export component Main inherits Rectangle { }"#.into());

        assert!(diag.len() == 1); // Only one URL is known
        let diagnostics = diag.get(&url).expect("URL not found in result");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_text_document_color_no_color_set() {
        let (mut dc, url, _) = loaded_document_cache(
            r#"
            component Main inherits Rectangle { }
            "#
            .into(),
        );

        let result = get_document_color(&mut dc, &lsp_types::TextDocumentIdentifier { uri: url })
            .expect("Color Vec was returned");
        assert!(result.is_empty());
    }

    #[test]
    fn test_text_document_color_rgba_color() {
        let (mut dc, url, _) = loaded_document_cache(
            r#"
            component Main inherits Rectangle {
                background: #1200FF80;
            }
            "#
            .into(),
        );

        let result = get_document_color(&mut dc, &lsp_types::TextDocumentIdentifier { uri: url })
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
        let result = element_at_position(dc, url, &Position { line, character })?;
        let element = result.borrow();
        Some(element.id.clone())
    }

    fn base_type_at_position(
        dc: &mut DocumentCache,
        url: &Url,
        line: u32,
        character: u32,
    ) -> Option<String> {
        let result = element_at_position(dc, url, &Position { line, character })?;
        let element = result.borrow();
        Some(format!("{}", &element.base_type))
    }

    #[test]
    fn test_element_at_position_no_element() {
        let (mut dc, url, _) = complex_document_cache();
        assert_eq!(id_at_position(&mut dc, &url, 0, 10), None);
        // TODO: This is past the end of the line and should thus return None
        assert_eq!(id_at_position(&mut dc, &url, 42, 90), Some(String::new()));
        assert_eq!(id_at_position(&mut dc, &url, 1, 0), None);
        assert_eq!(id_at_position(&mut dc, &url, 55, 1), None);
        assert_eq!(id_at_position(&mut dc, &url, 56, 5), None);
    }

    #[test]
    fn test_element_at_position_no_such_document() {
        let (mut dc, _, _) = complex_document_cache();
        assert_eq!(
            id_at_position(&mut dc, &Url::parse("https://foo.bar/baz").unwrap(), 5, 0),
            None
        );
    }

    #[test]
    fn test_element_at_position_root() {
        let (mut dc, url, _) = complex_document_cache();

        assert_eq!(id_at_position(&mut dc, &url, 2, 30), Some("root".to_string()));
        assert_eq!(id_at_position(&mut dc, &url, 2, 32), Some("root".to_string()));
        assert_eq!(id_at_position(&mut dc, &url, 2, 42), Some("root".to_string()));
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
        let (mut dc, url, _) = complex_document_cache();

        assert_eq!(base_type_at_position(&mut dc, &url, 12, 4), Some("VerticalBox".to_string()));
        assert_eq!(base_type_at_position(&mut dc, &url, 14, 22), Some("HorizontalBox".to_string()));
        assert_eq!(base_type_at_position(&mut dc, &url, 15, 33), Some("Text".to_string()));
        assert_eq!(base_type_at_position(&mut dc, &url, 27, 4), Some("VerticalBox".to_string()));
        assert_eq!(base_type_at_position(&mut dc, &url, 28, 8), Some("Text".to_string()));
        assert_eq!(base_type_at_position(&mut dc, &url, 51, 4), Some("VerticalBox".to_string()));
    }

    #[test]
    fn test_document_symbols() {
        let (mut dc, uri, _) = complex_document_cache();

        let result =
            get_document_symbols(&mut dc, &lsp_types::TextDocumentIdentifier { uri }).unwrap();

        if let DocumentSymbolResponse::Nested(result) = result {
            assert_eq!(result.len(), 1);

            let first = result.get(0).unwrap();
            assert_eq!(&first.name, "MainWindow");
        } else {
            unreachable!();
        }
    }

    #[test]
    fn test_document_symbols_hello_world() {
        let (mut dc, uri, _) = loaded_document_cache(
            r#"import { Button, VerticalBox } from "std-widgets.slint";
component Demo {
    VerticalBox {
        alignment: start;
        Text {
            text: "Hello World!";
            font-size: 24px;
            horizontal-alignment: center;
        }
        Image {
            source: @image-url("https://slint.dev/logo/slint-logo-full-light.svg");
            height: 100px;
        }
        HorizontalLayout { alignment: center; Button { text: "OK!"; } }
    }
}
            "#
            .into(),
        );
        let result =
            get_document_symbols(&mut dc, &lsp_types::TextDocumentIdentifier { uri }).unwrap();

        if let DocumentSymbolResponse::Nested(result) = result {
            assert_eq!(result.len(), 1);

            let first = result.get(0).unwrap();
            assert_eq!(&first.name, "Demo");
        } else {
            unreachable!();
        }
    }
}
