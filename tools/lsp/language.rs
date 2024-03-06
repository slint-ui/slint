// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore descr rfind unindented

pub mod completion;
mod component_catalog;
mod formatting;
mod goto;
pub mod properties;
mod semantic_tokens;
#[cfg(test)]
pub mod test;

use crate::common::{self, Result};
use crate::util;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;
use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::parser::{syntax_nodes, NodeOrToken, SyntaxKind, SyntaxNode, SyntaxToken};
use i_slint_compiler::pathutils::clean_path;
use i_slint_compiler::CompilerConfiguration;
use i_slint_compiler::{
    diagnostics::{BuildDiagnostics, SourceFileVersion},
    langtype::Type,
};
use i_slint_compiler::{typeloader::TypeLoader, typeregister::TypeRegister};
use lsp_types::request::{
    CodeActionRequest, CodeLensRequest, ColorPresentationRequest, Completion, DocumentColor,
    DocumentHighlightRequest, DocumentSymbolRequest, ExecuteCommand, Formatting, GotoDefinition,
    HoverRequest, PrepareRenameRequest, Rename, SemanticTokensFullRequest,
};
use lsp_types::{
    ClientCapabilities, CodeActionOrCommand, CodeActionProviderCapability, CodeLens,
    CodeLensOptions, Color, ColorInformation, ColorPresentation, Command, CompletionOptions,
    DocumentSymbol, DocumentSymbolResponse, Hover, InitializeParams, InitializeResult, OneOf,
    Position, PrepareRenameResponse, PublishDiagnosticsParams, RenameOptions,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextEdit, Url, WorkDoneProgressOptions,
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

pub fn uri_to_file(uri: &lsp_types::Url) -> Option<PathBuf> {
    let path = uri.to_file_path().ok()?;
    let cleaned_path = clean_path(&path);
    Some(cleaned_path)
}

fn command_list() -> Vec<String> {
    vec![
        QUERY_PROPERTIES_COMMAND.into(),
        REMOVE_BINDING_COMMAND.into(),
        #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
        SHOW_PREVIEW_COMMAND.into(),
        SET_BINDING_COMMAND.into(),
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

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
pub fn request_state(ctx: &std::rc::Rc<Context>) {
    let cache = ctx.document_cache.borrow();
    let documents = &cache.documents;

    for (p, d) in documents.all_file_documents() {
        if let Some(node) = &d.node {
            if p.starts_with("builtin:/") {
                continue; // The preview knows these, too.
            }
            let Ok(url) = Url::from_file_path(p) else {
                continue;
            };
            ctx.server_notifier.send_message_to_preview(common::LspToPreviewMessage::SetContents {
                url: common::VersionedUrl::new(url, node.source_file.version()),
                contents: node.text().to_string(),
            })
        }
    }
    ctx.server_notifier.send_message_to_preview(common::LspToPreviewMessage::SetConfiguration {
        config: cache.preview_config.clone(),
    });
    if let Some(c) = ctx.to_show.borrow().clone() {
        ctx.server_notifier.send_message_to_preview(common::LspToPreviewMessage::ShowPreview(c))
    }
}

/// A cache of loaded documents
pub struct DocumentCache {
    pub(crate) documents: TypeLoader,
    preview_config: common::PreviewConfig,
}

impl DocumentCache {
    pub fn new(config: CompilerConfiguration) -> Self {
        let documents =
            TypeLoader::new(TypeRegister::builtin(), config, &mut BuildDiagnostics::default());
        Self { documents, preview_config: Default::default() }
    }

    pub fn document_version(&self, target_uri: &lsp_types::Url) -> SourceFileVersion {
        self.documents
            .get_document(&uri_to_file(target_uri).unwrap_or_default())
            .and_then(|doc| doc.node.as_ref()?.source_file.version())
    }
}

pub struct Context {
    pub document_cache: RefCell<DocumentCache>,
    pub server_notifier: crate::ServerNotifier,
    pub init_param: InitializeParams,
    /// The last component for which the user clicked "show preview"
    pub to_show: RefCell<Option<common::PreviewComponent>>,
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
            document_formatting_provider: Some(OneOf::Left(true)),
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
            .and_then(|(token, _)| {
                get_code_actions(document_cache, token, &ctx.init_param.capabilities)
            });
        Ok(result)
    });
    rh.register::<ExecuteCommand, _>(|params, ctx| async move {
        if params.command.as_str() == SHOW_PREVIEW_COMMAND {
            #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
            show_preview_command(&params.arguments, &ctx)?;
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
        if let Some((tk, offset)) =
            token_descr(document_cache, &uri, &_params.text_document_position_params.position)
        {
            let p = tk.parent();
            if p.kind() == SyntaxKind::QualifiedName
                && p.parent().map_or(false, |n| n.kind() == SyntaxKind::Element)
            {
                if let Some(range) = util::map_node(&p) {
                    ctx.server_notifier.send_message_to_preview(
                        common::LspToPreviewMessage::HighlightFromEditor { url: Some(uri), offset },
                    );
                    return Ok(Some(vec![lsp_types::DocumentHighlight { range, kind: None }]));
                }
            }

            if let Some(value) = find_element_id_for_highlight(&tk, &p) {
                ctx.server_notifier.send_message_to_preview(
                    common::LspToPreviewMessage::HighlightFromEditor { url: None, offset: 0 },
                );
                return Ok(Some(
                    value
                        .into_iter()
                        .map(|r| lsp_types::DocumentHighlight {
                            range: util::map_range(&p.source_file, r),
                            kind: None,
                        })
                        .collect(),
                ));
            }
        }
        ctx.server_notifier.send_message_to_preview(
            common::LspToPreviewMessage::HighlightFromEditor { url: None, offset: 0 },
        );
        Ok(None)
    });
    rh.register::<Rename, _>(|params, ctx| async move {
        let mut document_cache = ctx.document_cache.borrow_mut();
        let uri = params.text_document_position.text_document.uri;
        if let Some((tk, _off)) =
            token_descr(&mut document_cache, &uri, &params.text_document_position.position)
        {
            let p = tk.parent();
            let version = p.source_file.version();
            if let Some(value) = find_element_id_for_highlight(&tk, &tk.parent()) {
                let edits: Vec<_> = value
                    .into_iter()
                    .map(|r| TextEdit {
                        range: util::map_range(&p.source_file, r),
                        new_text: params.new_name.clone(),
                    })
                    .collect();
                return Ok(Some(common::create_workspace_edit(uri, version, edits)));
            }
        };
        Err("This symbol cannot be renamed. (Only element id can be renamed at the moment)".into())
    });
    rh.register::<PrepareRenameRequest, _>(|params, ctx| async move {
        let mut document_cache = ctx.document_cache.borrow_mut();
        let uri = params.text_document.uri;
        if let Some((tk, _off)) = token_descr(&mut document_cache, &uri, &params.position) {
            if find_element_id_for_highlight(&tk, &tk.parent()).is_some() {
                return Ok(util::map_token(&tk).map(PrepareRenameResponse::Range));
            }
        };
        Ok(None)
    });
    rh.register::<Formatting, _>(|params, ctx| async move {
        let document_cache = ctx.document_cache.borrow_mut();
        Ok(formatting::format_document(params, &document_cache))
    });
}

#[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
pub fn show_preview_command(params: &[serde_json::Value], ctx: &Rc<Context>) -> Result<()> {
    let document_cache = &mut ctx.document_cache.borrow_mut();
    let config = &document_cache.documents.compiler_config;

    let e = || "InvalidParameter";

    let url: Url = serde_json::from_value(params.first().ok_or_else(e)?.clone())?;
    // Normalize the URL to make sure it is encoded the same way as what the preview expect from other URLs
    let url = Url::from_file_path(uri_to_file(&url).ok_or_else(e)?).map_err(|_| e())?;

    let component =
        params.get(1).and_then(|v| v.as_str()).filter(|v| !v.is_empty()).map(|v| v.to_string());

    let c = common::PreviewComponent {
        url,
        component,
        style: config.style.clone().unwrap_or_default(),
    };
    ctx.to_show.replace(Some(c.clone()));
    ctx.server_notifier.send_message_to_preview(common::LspToPreviewMessage::ShowPreview(c));

    // Update known Components
    report_known_components(document_cache, ctx);

    Ok(())
}

pub fn query_properties_command(
    params: &[serde_json::Value],
    ctx: &Rc<Context>,
) -> Result<serde_json::Value> {
    let document_cache = &mut ctx.document_cache.borrow_mut();

    let text_document_uri = serde_json::from_value::<lsp_types::TextDocumentIdentifier>(
        params.first().ok_or("No text document provided")?.clone(),
    )?
    .uri;
    let position = serde_json::from_value::<lsp_types::Position>(
        params.get(1).ok_or("No position provided")?.clone(),
    )?;

    let source_version = if let Some(v) = document_cache.document_version(&text_document_uri) {
        Some(v)
    } else {
        return Ok(serde_json::to_value(properties::QueryPropertyResponse::no_element_response(
            text_document_uri.to_string(),
            -1,
        ))
        .expect("Failed to serialize none-element property query result!"));
    };

    if let Some(element) =
        element_at_position(&document_cache.documents, &text_document_uri, &position)
    {
        properties::query_properties(&text_document_uri, source_version, &element)
            .map(|r| serde_json::to_value(r).expect("Failed to serialize property query result!"))
    } else {
        Ok(serde_json::to_value(properties::QueryPropertyResponse::no_element_response(
            text_document_uri.to_string(),
            source_version.unwrap_or(i32::MIN),
        ))
        .expect("Failed to serialize none-element property query result!"))
    }
}

pub async fn set_binding_command(
    params: &[serde_json::Value],
    ctx: &Rc<Context>,
) -> Result<serde_json::Value> {
    let text_document = serde_json::from_value::<lsp_types::OptionalVersionedTextDocumentIdentifier>(
        params.first().ok_or("No text document provided")?.clone(),
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
        let version = document_cache.document_version(&uri);
        if let Some(source_version) = text_document.version {
            if let Some(current_version) = version {
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

        let element = element_at_position(&document_cache.documents, &uri, &element_range.start)
            .ok_or_else(|| {
                format!("No element found at the given start position {:?}", &element_range.start)
            })?;

        let node_range =
            element.with_element_node(|node| util::map_node(node)).ok_or("Failed to map node")?;

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

        properties::set_binding(
            &document_cache,
            &uri,
            version,
            &element,
            &property_name,
            new_expression,
        )?
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
        params.first().ok_or("No text document provided")?.clone(),
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
        let version = document_cache.document_version(&uri);

        if let Some(source_version) = text_document.version {
            if let Some(current_version) = version {
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

        let element = element_at_position(&document_cache.documents, &uri, &element_range.start)
            .ok_or_else(|| {
                format!("No element found at the given start position {:?}", &element_range.start)
            })?;

        let node_range =
            element.with_element_node(|node| util::map_node(node)).ok_or("Failed to map node")?;

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

        properties::remove_binding(uri, version, &element, &property_name)?
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
    url: lsp_types::Url,
    version: Option<i32>,
    document_cache: &mut DocumentCache,
) -> HashMap<Url, Vec<lsp_types::Diagnostic>> {
    let Some(path) = uri_to_file(&url) else { return Default::default() };
    // Normalize the URL
    let Ok(url) = Url::from_file_path(path.clone()) else { return Default::default() };
    if path.extension().map_or(false, |e| e == "rs") {
        content = match i_slint_compiler::lexer::extract_rust_macro(content) {
            Some(content) => content,
            // A rust file without a rust macro, just ignore it
            None => return [(url, vec![])].into_iter().collect(),
        };
    }

    if let Some(ctx) = ctx {
        ctx.server_notifier.send_message_to_preview(common::LspToPreviewMessage::SetContents {
            url: common::VersionedUrl::new(url, version),
            contents: content.clone(),
        });
    }
    let mut diag = BuildDiagnostics::default();
    document_cache.documents.load_file(&path, version, &path, content, false, &mut diag).await;

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

    lsp_diags
}

fn report_known_components(document_cache: &mut DocumentCache, ctx: &Rc<Context>) {
    let mut components = Vec::new();
    component_catalog::builtin_components(document_cache, &mut components);
    component_catalog::all_exported_components(
        document_cache,
        &mut |ci| ci.is_global,
        &mut components,
    );

    components.sort_by(|a, b| a.name.cmp(&b.name));

    let url = ctx.to_show.borrow().as_ref().map(|pc| {
        let url = pc.url.clone();
        let file = PathBuf::from(url.to_string());
        let version = document_cache.document_version(&url);

        component_catalog::file_local_components(document_cache, &file, &mut components);

        common::VersionedUrl::new(url, version)
    });

    ctx.server_notifier
        .send_message_to_preview(common::LspToPreviewMessage::KnownComponents { url, components });
}

pub async fn reload_document(
    ctx: &Rc<Context>,
    content: String,
    url: lsp_types::Url,
    version: Option<i32>,
    document_cache: &mut DocumentCache,
) -> Result<()> {
    let lsp_diags =
        reload_document_impl(Some(ctx), content, url.clone(), version, document_cache).await;

    for (uri, diagnostics) in lsp_diags {
        ctx.server_notifier.send_notification(
            "textDocument/publishDiagnostics".into(),
            PublishDiagnosticsParams { uri, diagnostics, version: None },
        )?;
    }

    // Tell Preview about the Components:
    report_known_components(document_cache, ctx);

    Ok(())
}

fn get_document_and_offset<'a>(
    type_loader: &'a TypeLoader,
    text_document_uri: &'a Url,
    pos: &'a Position,
) -> Option<(&'a i_slint_compiler::object_tree::Document, u32)> {
    let path = uri_to_file(text_document_uri)?;
    let doc = type_loader.get_document(&path)?;
    let o = doc.node.as_ref()?.source_file.offset(pos.line as usize + 1, pos.character as usize + 1)
        as u32;
    doc.node.as_ref()?.text_range().contains_inclusive(o.into()).then_some((doc, o))
}

fn element_contains(
    element: &i_slint_compiler::object_tree::ElementRc,
    offset: u32,
) -> Option<usize> {
    element
        .borrow()
        .debug
        .iter()
        .position(|n| n.0.parent().map_or(false, |n| n.text_range().contains(offset.into())))
}

fn element_node_contains(element: &common::ElementRcNode, offset: u32) -> bool {
    element.with_element_node(|node| {
        node.parent().map_or(false, |n| n.text_range().contains(offset.into()))
    })
}

pub fn element_at_position(
    type_loader: &TypeLoader,
    text_document_uri: &Url,
    pos: &Position,
) -> Option<common::ElementRcNode> {
    let (doc, offset) = get_document_and_offset(&type_loader, text_document_uri, pos)?;

    for component in &doc.inner_components {
        let root_element = component.root_element.clone();
        let Some(root_debug_index) = element_contains(&root_element, offset) else {
            continue;
        };

        let mut element =
            common::ElementRcNode { element: root_element, debug_index: root_debug_index };
        while element_node_contains(&element, offset) {
            if let Some((c, i)) = element
                .element
                .clone()
                .borrow()
                .children
                .iter()
                .find_map(|c| element_contains(c, offset).map(|i| (c, i)))
            {
                element = common::ElementRcNode { element: c.clone(), debug_index: i };
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
    let (doc, o) = get_document_and_offset(&document_cache.documents, text_document_uri, pos)?;
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

fn has_experimental_client_capability(capabilities: &ClientCapabilities, name: &str) -> bool {
    capabilities
        .experimental
        .as_ref()
        .and_then(|o| o.get(name).and_then(|v| v.as_bool()))
        .unwrap_or(false)
}

fn get_code_actions(
    document_cache: &mut DocumentCache,
    token: SyntaxToken,
    client_capabilities: &ClientCapabilities,
) -> Option<Vec<CodeActionOrCommand>> {
    let node = token.parent();
    let uri = Url::from_file_path(token.source_file.path()).ok()?;
    let mut result = vec![];

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

    #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
    {
        if let Some(component) = &component {
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
        let r = util::map_range(&token.source_file, node.text_range());
        let edits = vec![
            TextEdit::new(lsp_types::Range::new(r.start, r.start), "@tr(".into()),
            TextEdit::new(lsp_types::Range::new(r.end, r.end), ")".into()),
        ];
        result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
            title: "Wrap in `@tr()`".into(),
            edit: common::create_workspace_edit_from_source_file(&token.source_file, edits),
            ..Default::default()
        }));
    } else if token.kind() == SyntaxKind::Identifier
        && node.kind() == SyntaxKind::QualifiedName
        && node.parent().map(|n| n.kind()) == Some(SyntaxKind::Element)
    {
        let is_lookup_error = {
            let global_tr = document_cache.documents.global_type_registry.borrow();
            let tr = document_cache
                .documents
                .get_document(token.source_file.path())
                .map(|doc| &doc.local_registry)
                .unwrap_or(&global_tr);
            util::lookup_current_element_type(node.clone(), tr).is_none()
        };
        if is_lookup_error {
            // Couldn't lookup the element, there is probably an error. Suggest an edit
            let text = token.text();
            completion::build_import_statements_edits(
                &token,
                document_cache,
                &mut |name| name == text,
                &mut |_name, file, edit| {
                    result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                        title: format!("Add import from \"{file}\""),
                        kind: Some(lsp_types::CodeActionKind::QUICKFIX),
                        edit: common::create_workspace_edit_from_source_file(
                            &token.source_file,
                            vec![edit],
                        ),
                        ..Default::default()
                    }))
                },
            );
        }

        if has_experimental_client_capability(client_capabilities, "snippetTextEdit") {
            let r = util::map_range(&token.source_file, node.parent().unwrap().text_range());
            let element = element_at_position(&document_cache.documents, &uri, &r.start);
            let element_indent = element.as_ref().and_then(util::find_element_indent);
            let indented_lines = node
                .parent()
                .unwrap()
                .text()
                .to_string()
                .lines()
                .map(
                    |line| if line.is_empty() { line.to_string() } else { format!("    {}", line) },
                )
                .collect::<Vec<String>>();
            let edits = vec![TextEdit::new(
                lsp_types::Range::new(r.start, r.end),
                format!(
                    "${{0:element}} {{\n{}{}\n}}",
                    element_indent.unwrap_or("".into()),
                    indented_lines.join("\n")
                ),
            )];
            result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                title: "Wrap in element".into(),
                kind: Some(lsp_types::CodeActionKind::REFACTOR),
                edit: common::create_workspace_edit_from_source_file(&token.source_file, edits),
                ..Default::default()
            }));

            // Collect all normal, repeated, and conditional sub-elements and any
            // whitespace in between for substituting the parent element with its
            // sub-elements, dropping its own properties, callbacks etc.
            fn is_sub_element(kind: SyntaxKind) -> bool {
                matches!(
                    kind,
                    SyntaxKind::SubElement
                        | SyntaxKind::RepeatedElement
                        | SyntaxKind::ConditionalElement
                )
            }
            let sub_elements = node
                .parent()
                .unwrap()
                .children_with_tokens()
                .skip_while(|n| !is_sub_element(n.kind()))
                .filter(|n| match n {
                    NodeOrToken::Node(_) => is_sub_element(n.kind()),
                    NodeOrToken::Token(t) => {
                        t.kind() == SyntaxKind::Whitespace
                            && t.next_sibling_or_token().map_or(false, |n| is_sub_element(n.kind()))
                    }
                })
                .collect::<Vec<_>>();

            if match component {
                // A top-level component element can only be removed if it contains
                // exactly one sub-element (without any condition or assignment)
                // that can substitute the component element.
                Some(_) => {
                    sub_elements.len() == 1
                        && sub_elements.first().and_then(|n| {
                            n.as_node().unwrap().first_child_or_token().map(|n| n.kind())
                        }) == Some(SyntaxKind::Element)
                }
                // Any other element can be removed in favor of one or more sub-elements.
                None => sub_elements.iter().any(|n| n.kind() == SyntaxKind::SubElement),
            } {
                let unindented_lines = sub_elements
                    .iter()
                    .map(|n| match n {
                        NodeOrToken::Node(n) => n
                            .text()
                            .to_string()
                            .lines()
                            .map(|line| line.strip_prefix("    ").unwrap_or(line).to_string())
                            .collect::<Vec<_>>()
                            .join("\n"),
                        NodeOrToken::Token(t) => {
                            t.text().strip_suffix("    ").unwrap_or(t.text()).to_string()
                        }
                    })
                    .collect::<Vec<String>>();
                let edits = vec![TextEdit::new(
                    lsp_types::Range::new(r.start, r.end),
                    unindented_lines.concat(),
                )];
                result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                    title: "Remove element".into(),
                    kind: Some(lsp_types::CodeActionKind::REFACTOR),
                    edit: common::create_workspace_edit_from_source_file(&token.source_file, edits),
                    ..Default::default()
                }));
            }

            // We have already checked that the node is a qualified name of an element.
            // Check whether the element is a direct sub-element of another element
            // meaning that it can be repeated or made conditional.
            if node // QualifiedName
                .parent() // Element
                .unwrap()
                .parent()
                .filter(|n| n.kind() == SyntaxKind::SubElement)
                .and_then(|p| p.parent())
                .is_some_and(|n| n.kind() == SyntaxKind::Element)
            {
                let edits = vec![TextEdit::new(
                    lsp_types::Range::new(r.start, r.start),
                    "for ${1:name}[index] in ${0:model} : ".to_string(),
                )];
                result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                    title: "Repeat element".into(),
                    kind: Some(lsp_types::CodeActionKind::REFACTOR),
                    edit: common::create_workspace_edit_from_source_file(&token.source_file, edits),
                    ..Default::default()
                }));

                let edits = vec![TextEdit::new(
                    lsp_types::Range::new(r.start, r.start),
                    "if ${0:condition} : ".to_string(),
                )];
                result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                    title: "Make conditional".into(),
                    kind: Some(lsp_types::CodeActionKind::REFACTOR),
                    edit: common::create_workspace_edit_from_source_file(&token.source_file, edits),
                    ..Default::default()
                }));
            }
        }
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
                let range = util::map_token(&token)?;
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

/// Retrieve the document outline
fn get_document_symbols(
    document_cache: &mut DocumentCache,
    text_document: &lsp_types::TextDocumentIdentifier,
) -> Option<DocumentSymbolResponse> {
    let uri_path = uri_to_file(&text_document.uri)?;
    let doc = document_cache.documents.get_document(&uri_path)?;

    // DocumentSymbol doesn't implement default and some field depends on features or are deprecated
    let ds: DocumentSymbol = serde_json::from_value(
        serde_json::json!({ "name" : "", "kind": 255, "range" : lsp_types::Range::default(), "selectionRange" : lsp_types::Range::default() })
    )
    .unwrap();

    let inner_components = doc.inner_components.clone();
    let inner_types = doc.inner_types.clone();

    let mut r = inner_components
        .iter()
        .filter_map(|c| {
            let root_element = c.root_element.borrow();
            let element_node = &root_element.debug.first()?.0;
            let component_node = syntax_nodes::Component::new(element_node.parent()?)?;
            let selection_range = util::map_node(&component_node.DeclaredIdentifier())?;
            if c.id.is_empty() {
                // Symbols with empty names are invalid
                return None;
            }

            Some(DocumentSymbol {
                range: util::map_node(&component_node)?,
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
            range: util::map_node(node.parent().as_ref()?)?,
            selection_range: util::map_node(
                &node.parent()?.child_node(SyntaxKind::DeclaredIdentifier)?,
            )?,
            name: name.clone(),
            kind: lsp_types::SymbolKind::STRUCT,
            ..ds.clone()
        }),
        Type::Enumeration(enumeration) => enumeration.node.as_ref().and_then(|node| {
            Some(DocumentSymbol {
                range: util::map_node(node)?,
                selection_range: util::map_node(&node.DeclaredIdentifier())?,
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
                let element_node = &e.debug.first()?.0;
                let sub_element_node = element_node.parent()?;
                debug_assert_eq!(sub_element_node.kind(), SyntaxKind::SubElement);
                Some(DocumentSymbol {
                    range: util::map_node(&sub_element_node)?,
                    selection_range: util::map_node(element_node.QualifiedName().as_ref()?)?,
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

    r.sort_by(|a, b| a.range.start.cmp(&b.range.start));

    Some(r.into())
}

fn get_code_lenses(
    document_cache: &mut DocumentCache,
    text_document: &lsp_types::TextDocumentIdentifier,
) -> Option<Vec<CodeLens>> {
    if cfg!(any(feature = "preview-builtin", feature = "preview-external")) {
        let filepath = uri_to_file(&text_document.uri)?;
        let doc = document_cache.documents.get_document(&filepath)?;

        let inner_components = doc.inner_components.clone();

        let mut r = vec![];

        // Handle preview lens
        r.extend(inner_components.iter().filter(|c| !c.is_global()).filter_map(|c| {
            Some(CodeLens {
                range: util::map_node(&c.root_element.borrow().debug.first()?.0)?,
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
    let mut hide_ui = None;
    for v in r {
        if let Some(o) = v.as_object() {
            if let Some(ip) = o.get("includePaths").and_then(|v| v.as_array()) {
                if !ip.is_empty() {
                    document_cache.documents.compiler_config.include_paths =
                        ip.iter().filter_map(|x| x.as_str()).map(PathBuf::from).collect();
                }
            }
            if let Some(lp) = o.get("libraryPaths").and_then(|v| v.as_object()) {
                if !lp.is_empty() {
                    document_cache.documents.compiler_config.library_paths = lp
                        .iter()
                        .filter_map(|(k, v)| v.as_str().map(|v| (k.to_string(), PathBuf::from(v))))
                        .collect();
                }
            }
            if let Some(style) =
                o.get("preview").and_then(|v| v.as_object()?.get("style")?.as_str())
            {
                if !style.is_empty() {
                    document_cache.documents.compiler_config.style = Some(style.into());
                }
            }
            hide_ui = o.get("preview").and_then(|v| v.as_object()?.get("hide_ui")?.as_bool());
        }
    }

    // Always load the widgets so we can auto-complete them
    let mut diag = BuildDiagnostics::default();
    document_cache.documents.import_component("std-widgets.slint", "StyleMetrics", &mut diag).await;

    let cc = &document_cache.documents.compiler_config;
    let config = common::PreviewConfig {
        hide_ui,
        style: cc.style.clone().unwrap_or_default(),
        include_paths: cc.include_paths.clone(),
        library_paths: cc.library_paths.clone(),
    };
    document_cache.preview_config = config.clone();
    ctx.server_notifier
        .send_message_to_preview(common::LspToPreviewMessage::SetConfiguration { config });
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use lsp_types::WorkspaceEdit;

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
        let (mut dc, uri, _) = loaded_document_cache(
            r#"
            component Main inherits Rectangle { }
            "#
            .into(),
        );

        let result = get_document_color(&mut dc, &lsp_types::TextDocumentIdentifier { uri })
            .expect("Color Vec was returned");
        assert!(result.is_empty());
    }

    #[test]
    fn test_text_document_color_rgba_color() {
        let (mut dc, uri, _) = loaded_document_cache(
            r#"
            component Main inherits Rectangle {
                background: #1200FF80;
            }
            "#
            .into(),
        );

        let result = get_document_color(&mut dc, &lsp_types::TextDocumentIdentifier { uri })
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
        let result = element_at_position(&dc.documents, url, &Position { line, character })?;
        let element = result.element.borrow();
        Some(element.id.clone())
    }

    fn base_type_at_position(
        dc: &mut DocumentCache,
        url: &Url,
        line: u32,
        character: u32,
    ) -> Option<String> {
        let result = element_at_position(&dc.documents, url, &Position { line, character })?;
        let element = result.element.borrow();
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

            let first = result.first().unwrap();
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

            let first = result.first().unwrap();
            assert_eq!(&first.name, "Demo");
        } else {
            unreachable!();
        }
    }

    #[test]
    fn test_document_symbols_no_empty_names() {
        // issue #3979
        let (mut dc, uri, _) = loaded_document_cache(
            r#"import { Button, VerticalBox } from "std-widgets.slint";
struct Foo {}
enum Bar {}
export component Yo { Rectangle {} }
export component {}
struct {}
enum {}
            "#
            .into(),
        );
        let result =
            get_document_symbols(&mut dc, &lsp_types::TextDocumentIdentifier { uri }).unwrap();

        if let DocumentSymbolResponse::Nested(result) = result {
            assert_eq!(result.len(), 3);
            assert_eq!(result[0].name, "Foo");
            assert_eq!(result[1].name, "Bar");
            assert_eq!(result[2].name, "Yo");
        } else {
            unreachable!();
        }
    }

    #[test]
    fn test_document_symbols_positions() {
        let source = r#"import { Button } from "std-widgets.slint";

        enum TheEnum {
            Abc, Def
        }/*TheEnum*/

        component FooBar {
            in property <TheEnum> the-enum;
            HorizontalLayout {
                btn := Button {}
                Rectangle {
                    ta := TouchArea {}
                    Image {
                    }
                }
            }/*HorizontalLayout*/
        }/*FooBar*/

        struct Str { abc: string }

        export global SomeGlobal {
            in-out property<Str> prop;
        }/*SomeGlobal*/

        export component TestWindow inherits Window {
            FooBar {}
        }/*TestWindow*/
        "#;

        let (mut dc, uri, _) = test::loaded_document_cache(source.into());

        let result =
            get_document_symbols(&mut dc, &lsp_types::TextDocumentIdentifier { uri: uri.clone() })
                .unwrap();

        let check_start_with = |pos, str: &str| {
            let (_, offset) = get_document_and_offset(&dc.documents, &uri, &pos).unwrap();
            assert_eq!(&source[offset as usize..][..str.len()], str);
        };

        let DocumentSymbolResponse::Nested(result) = result else {
            panic!("not nested {result:?}")
        };

        assert_eq!(result.len(), 5);
        assert_eq!(result[0].name, "TheEnum");
        check_start_with(result[0].range.start, "enum TheEnum {");
        check_start_with(result[0].range.end, "/*TheEnum*/");
        check_start_with(result[0].selection_range.start, "TheEnum {");
        check_start_with(result[0].selection_range.end, " {");
        assert_eq!(result[1].name, "FooBar");
        check_start_with(result[1].range.start, "component FooBar {");
        check_start_with(result[1].range.end, "/*FooBar*/");
        check_start_with(result[1].selection_range.start, "FooBar {");
        check_start_with(result[1].selection_range.end, " {");
        assert_eq!(result[2].name, "Str");
        check_start_with(result[2].range.start, "struct Str {");
        check_start_with(result[2].range.end, "\n");
        check_start_with(result[2].selection_range.start, "Str {");
        check_start_with(result[2].selection_range.end, " {");
        assert_eq!(result[3].name, "SomeGlobal");
        check_start_with(result[3].range.start, "global SomeGlobal");
        check_start_with(result[3].range.end, "/*SomeGlobal*/");
        check_start_with(result[3].selection_range.start, "SomeGlobal {");
        check_start_with(result[3].selection_range.end, " {");
        assert_eq!(result[4].name, "TestWindow");
        check_start_with(result[4].range.start, "component TestWindow inherits Window {");
        check_start_with(result[4].range.end, "/*TestWindow*/");
        check_start_with(result[4].selection_range.start, "TestWindow inherits");
        check_start_with(result[4].selection_range.end, " inherits");

        macro_rules! tree {
                ($root:literal $($more:literal)*) => {
                    result[$root] $(.children.as_ref().unwrap()[$more])*
                };
            }
        assert_eq!(tree!(1 0).name, "HorizontalLayout");
        check_start_with(tree!(1 0).range.start, "HorizontalLayout {");
        check_start_with(tree!(1 0).range.end, "/*HorizontalLayout*/");
        assert_eq!(tree!(1 0 0).name, "Button");
        assert_eq!(tree!(1 0 0).detail, Some("btn".into()));
        check_start_with(tree!(1 0 0).range.start, "btn := Button");

        assert_eq!(tree!(1 0 1 0).name, "TouchArea");
        assert_eq!(tree!(1 0 1 0).detail, Some("ta".into()));
        check_start_with(tree!(1 0 1 0).range.start, "ta := TouchArea");
    }

    #[test]
    fn test_code_actions() {
        let (mut dc, url, _) = loaded_document_cache(
            r#"import { Button, VerticalBox, HorizontalBox} from "std-widgets.slint";

export component TestWindow inherits Window {
    VerticalBox {
        alignment: start;

        Text {
            text: "Hello World!";
            font-size: 20px;
        }

        input := LineEdit {
            placeholder-text: "Enter your name";
        }

        if (true): HorizontalBox {
            alignment: end;

            Button { text: "Cancel"; }

            Button {
                text: "OK";
                primary: true;
            }
        }
    }
}"#
            .into(),
        );
        let mut capabilities = ClientCapabilities::default();

        let text_literal = lsp_types::Range::new(Position::new(7, 18), Position::new(7, 32));
        assert_eq!(
            token_descr(&mut dc, &url, &text_literal.start)
                .and_then(|(token, _)| get_code_actions(&mut dc, token, &capabilities)),
            Some(vec![CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                title: "Wrap in `@tr()`".into(),
                edit: Some(WorkspaceEdit {
                    document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
                        lsp_types::TextDocumentEdit {
                            text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                                version: Some(42),
                                uri: url.clone(),
                            },
                            edits: vec![
                                lsp_types::OneOf::Left(TextEdit::new(
                                    lsp_types::Range::new(text_literal.start, text_literal.start),
                                    "@tr(".into()
                                )),
                                lsp_types::OneOf::Left(TextEdit::new(
                                    lsp_types::Range::new(text_literal.end, text_literal.end),
                                    ")".into()
                                )),
                            ],
                        }
                    ])),
                    ..Default::default()
                }),
                ..Default::default()
            })]),
        );

        let text_element = lsp_types::Range::new(Position::new(6, 8), Position::new(9, 9));
        for offset in 0..=4 {
            let pos = Position::new(text_element.start.line, text_element.start.character + offset);

            capabilities.experimental = None;
            assert_eq!(
                token_descr(&mut dc, &url, &pos).and_then(|(token, _)| get_code_actions(
                    &mut dc,
                    token,
                    &capabilities
                )),
                None
            );

            capabilities.experimental = Some(serde_json::json!({"snippetTextEdit": true}));
            assert_eq!(
                token_descr(&mut dc, &url, &pos).and_then(|(token, _)| get_code_actions(
                    &mut dc,
                    token,
                    &capabilities
                )),
                Some(vec![
                    CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                        title: "Wrap in element".into(),
                        kind: Some(lsp_types::CodeActionKind::REFACTOR),
                        edit: Some(WorkspaceEdit {
                            document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
                                lsp_types::TextDocumentEdit {
                                    text_document:
                                        lsp_types::OptionalVersionedTextDocumentIdentifier {
                                            version: Some(42),
                                            uri: url.clone(),
                                        },
                                    edits: vec![lsp_types::OneOf::Left(TextEdit::new(
                                        text_element,
                                        r#"${0:element} {
            Text {
                text: "Hello World!";
                font-size: 20px;
            }
}"#
                                        .into()
                                    ))],
                                },
                            ])),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                        title: "Repeat element".into(),
                        kind: Some(lsp_types::CodeActionKind::REFACTOR),
                        edit: Some(WorkspaceEdit {
                            document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
                                lsp_types::TextDocumentEdit {
                                    text_document:
                                        lsp_types::OptionalVersionedTextDocumentIdentifier {
                                            version: Some(42),
                                            uri: url.clone(),
                                        },
                                    edits: vec![lsp_types::OneOf::Left(TextEdit::new(
                                        lsp_types::Range::new(
                                            text_element.start,
                                            text_element.start
                                        ),
                                        r#"for ${1:name}[index] in ${0:model} : "#.into()
                                    ))],
                                }
                            ])),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                        title: "Make conditional".into(),
                        kind: Some(lsp_types::CodeActionKind::REFACTOR),
                        edit: Some(WorkspaceEdit {
                            document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
                                lsp_types::TextDocumentEdit {
                                    text_document:
                                        lsp_types::OptionalVersionedTextDocumentIdentifier {
                                            version: Some(42),
                                            uri: url.clone(),
                                        },
                                    edits: vec![lsp_types::OneOf::Left(TextEdit::new(
                                        lsp_types::Range::new(
                                            text_element.start,
                                            text_element.start
                                        ),
                                        r#"if ${0:condition} : "#.into()
                                    ))],
                                }
                            ])),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                ])
            );
        }

        let horizontal_box = lsp_types::Range::new(Position::new(15, 19), Position::new(24, 9));

        capabilities.experimental = None;
        assert_eq!(
            token_descr(&mut dc, &url, &horizontal_box.start)
                .and_then(|(token, _)| get_code_actions(&mut dc, token, &capabilities)),
            None
        );

        capabilities.experimental = Some(serde_json::json!({"snippetTextEdit": true}));
        assert_eq!(
            token_descr(&mut dc, &url, &horizontal_box.start)
                .and_then(|(token, _)| get_code_actions(&mut dc, token, &capabilities)),
            Some(vec![
                CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                    title: "Wrap in element".into(),
                    kind: Some(lsp_types::CodeActionKind::REFACTOR),
                    edit: Some(WorkspaceEdit {
                        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
                            lsp_types::TextDocumentEdit {
                                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                                    version: Some(42),
                                    uri: url.clone(),
                                },
                                edits: vec![lsp_types::OneOf::Left(TextEdit::new(
                                    horizontal_box,
                                    r#"${0:element} {
            HorizontalBox {
                alignment: end;

                Button { text: "Cancel"; }

                Button {
                    text: "OK";
                    primary: true;
                }
            }
}"#
                                    .into()
                                ))]
                            }
                        ])),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                    title: "Remove element".into(),
                    kind: Some(lsp_types::CodeActionKind::REFACTOR),
                    edit: Some(WorkspaceEdit {
                        document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
                            lsp_types::TextDocumentEdit {
                                text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                                    version: Some(42),
                                    uri: url.clone(),
                                },
                                edits: vec![lsp_types::OneOf::Left(TextEdit::new(
                                    horizontal_box,
                                    r#"Button { text: "Cancel"; }

        Button {
            text: "OK";
            primary: true;
        }"#
                                    .into()
                                ))]
                            }
                        ])),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            ])
        );

        let line_edit = Position::new(11, 20);
        let import_pos = lsp_types::Position::new(0, 43);
        capabilities.experimental = None;
        assert_eq!(
            token_descr(&mut dc, &url, &line_edit).and_then(|(token, _)| get_code_actions(
                &mut dc,
                token,
                &capabilities
            )),
            Some(vec![CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                title: "Add import from \"std-widgets.slint\"".into(),
                kind: Some(lsp_types::CodeActionKind::QUICKFIX),
                edit: Some(WorkspaceEdit {
                    document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
                        lsp_types::TextDocumentEdit {
                            text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                                version: Some(42),
                                uri: url.clone(),
                            },
                            edits: vec![lsp_types::OneOf::Left(TextEdit::new(
                                lsp_types::Range::new(import_pos, import_pos),
                                ", LineEdit".into()
                            ))]
                        }
                    ])),
                    ..Default::default()
                }),
                ..Default::default()
            }),])
        );
    }
}
