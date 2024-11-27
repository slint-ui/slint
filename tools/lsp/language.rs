// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore descr rfind unindented

pub mod completion;
mod formatting;
mod goto;
mod hover;
mod semantic_tokens;
mod signature_help;
#[cfg(test)]
pub mod test;
pub mod token_info;

use crate::common;
use crate::util;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;
use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::parser::{
    syntax_nodes, NodeOrToken, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize,
};
use i_slint_compiler::{diagnostics::BuildDiagnostics, langtype::Type};
use lsp_types::request::{
    CodeActionRequest, CodeLensRequest, ColorPresentationRequest, Completion, DocumentColor,
    DocumentHighlightRequest, DocumentSymbolRequest, ExecuteCommand, Formatting, GotoDefinition,
    HoverRequest, PrepareRenameRequest, Rename, SemanticTokensFullRequest, SignatureHelpRequest,
};
use lsp_types::{
    ClientCapabilities, CodeActionOrCommand, CodeActionProviderCapability, CodeLens,
    CodeLensOptions, Color, ColorInformation, ColorPresentation, Command, CompletionOptions,
    DocumentSymbol, DocumentSymbolResponse, InitializeParams, InitializeResult, OneOf, Position,
    PrepareRenameResponse, PublishDiagnosticsParams, RenameOptions, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextEdit, Url, WorkDoneProgressOptions,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;

pub const SHOW_PREVIEW_COMMAND: &str = "slint/showPreview";

fn command_list() -> Vec<String> {
    vec![
        #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
        SHOW_PREVIEW_COMMAND.into(),
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
    let document_cache = ctx.document_cache.borrow();

    for (url, node) in document_cache.all_url_documents() {
        if url.scheme() == "builtin" {
            continue;
        }
        let version = document_cache.document_version(&url);

        ctx.server_notifier.send_message_to_preview(common::LspToPreviewMessage::SetContents {
            url: common::VersionedUrl::new(url, version),
            contents: node.text().to_string(),
        })
    }
    ctx.server_notifier.send_message_to_preview(common::LspToPreviewMessage::SetConfiguration {
        config: ctx.preview_config.borrow().clone(),
    });
    if let Some(c) = ctx.to_show.borrow().clone() {
        ctx.server_notifier.send_message_to_preview(common::LspToPreviewMessage::ShowPreview(c))
    }
}

async fn register_file_watcher(ctx: &Context) -> common::Result<()> {
    use lsp_types::notification::Notification;

    if ctx
        .init_param
        .capabilities
        .workspace
        .as_ref()
        .and_then(|ws| ws.did_change_watched_files)
        .and_then(|wf| wf.dynamic_registration)
        .unwrap_or(false)
    {
        let fs_watcher = lsp_types::DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![lsp_types::FileSystemWatcher {
                glob_pattern: lsp_types::GlobPattern::String("**/*".to_string()),
                kind: Some(lsp_types::WatchKind::Change | lsp_types::WatchKind::Delete),
            }],
        };
        ctx.server_notifier
            .send_request::<lsp_types::request::RegisterCapability>(
                lsp_types::RegistrationParams {
                    registrations: vec![lsp_types::Registration {
                        id: "slint.file_watcher.registration".to_string(),
                        method: lsp_types::notification::DidChangeWatchedFiles::METHOD.to_string(),
                        register_options: Some(serde_json::to_value(fs_watcher).unwrap()),
                    }],
                },
            )?
            .await?;
    }

    Ok(())
}

pub struct Context {
    pub document_cache: RefCell<common::DocumentCache>,
    pub preview_config: RefCell<common::PreviewConfig>,
    pub server_notifier: crate::ServerNotifier,
    pub init_param: InitializeParams,
    /// The last component for which the user clicked "show preview"
    #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
    pub to_show: RefCell<Option<common::PreviewComponent>>,
    /// File currently open in the editor
    pub open_urls: RefCell<HashSet<lsp_types::Url>>,
}

/// An error from a LSP request
pub struct LspError {
    pub code: LspErrorCode,
    pub message: String,
}

/// The code of a LspError. Correspond to the lsp_server::ErrorCode
pub enum LspErrorCode {
    /// Invalid method parameter(s).
    InvalidParameter,
    /// Internal JSON-RPC error.
    #[allow(unused)]
    InternalError,

    /// A request failed but it was syntactically correct, e.g the
    /// method name was known and the parameters were valid. The error
    /// message should contain human readable information about why
    /// the request failed.
    RequestFailed,

    /// The server detected that the content of a document got
    /// modified outside normal conditions. A server should
    /// NOT send this error code if it detects a content change
    /// in it unprocessed messages. The result even computed
    /// on an older state might still be useful for the client.
    ///
    /// If a client decides that a result is not of any use anymore
    /// the client should cancel the request.
    #[allow(unused)]
    ContentModified = -32801,
}

#[derive(Default)]
pub struct RequestHandler(
    pub  HashMap<
        &'static str,
        Box<
            dyn Fn(
                serde_json::Value,
                Rc<Context>,
            )
                -> Pin<Box<dyn Future<Output = Result<serde_json::Value, LspError>>>>,
        >,
    >,
);

impl RequestHandler {
    pub fn register<
        R: lsp_types::request::Request,
        Fut: Future<Output = std::result::Result<R::Result, LspError>> + 'static,
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
                    let params = serde_json::from_value(value).map_err(|e| LspError {
                        code: LspErrorCode::InvalidParameter,
                        message: format!("error when deserializing request: {e:?}"),
                    })?;
                    handler(params, ctx).await.map(|x| serde_json::to_value(x).unwrap())
                })
            }),
        );
    }
}

pub fn server_initialize_result(client_cap: &ClientCapabilities) -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
            // Note: we only support UTF8 at the moment (which is a bug, as the spec says that support for utf-16 is mandatory)
            position_encoding: client_cap
                .general
                .as_ref()
                .and_then(|x| x.position_encodings.as_ref())
                .and_then(|x| x.iter().find(|x| *x == &lsp_types::PositionEncodingKind::UTF8))
                .cloned(),
            hover_provider: Some(true.into()),
            signature_help_provider: Some(lsp_types::SignatureHelpOptions {
                trigger_characters: Some(vec!["(".to_owned(), ",".to_owned()]),
                retrigger_characters: None,
                work_done_progress_options: WorkDoneProgressOptions::default(),
            }),
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
    rh.register::<HoverRequest, _>(|params, ctx| async move {
        let document_cache = &mut ctx.document_cache.borrow_mut();
        let result = token_descr(
            document_cache,
            &params.text_document_position_params.text_document.uri,
            &params.text_document_position_params.position,
        )
        .and_then(|(token, _)| hover::get_tooltip(document_cache, token));

        Ok(result)
    });
    rh.register::<SignatureHelpRequest, _>(|params, ctx| async move {
        let document_cache = &mut ctx.document_cache.borrow_mut();
        let result = token_descr(
            document_cache,
            &params.text_document_position_params.text_document.uri,
            &params.text_document_position_params.position,
        )
        .and_then(|(token, _)| signature_help::get_signature_help(document_cache, token));
        Ok(result)
    });
    rh.register::<CodeActionRequest, _>(|params, ctx| async move {
        let document_cache = &mut ctx.document_cache.borrow_mut();

        let result = token_descr(document_cache, &params.text_document.uri, &params.range.start)
            .and_then(|(token, _)| {
                get_code_actions(document_cache, token, &ctx.init_param.capabilities)
            });
        Ok(result)
    });
    rh.register::<ExecuteCommand, _>(|params, _ctx| async move {
        if params.command.as_str() == SHOW_PREVIEW_COMMAND {
            #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
            show_preview_command(&params.arguments, &_ctx)?;
            return Ok(None::<serde_json::Value>);
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
    rh.register::<DocumentHighlightRequest, _>(|params, ctx| async move {
        let document_cache = &mut ctx.document_cache.borrow_mut();
        let uri = params.text_document_position_params.text_document.uri;
        if let Some((tk, _)) =
            token_descr(document_cache, &uri, &params.text_document_position_params.position)
        {
            let p = tk.parent();
            let gp = p.parent();

            if p.kind() == SyntaxKind::DeclaredIdentifier
                && gp.as_ref().map_or(false, |n| n.kind() == SyntaxKind::Component)
            {
                let element = gp.as_ref().unwrap().child_node(SyntaxKind::Element).unwrap();

                ctx.server_notifier.send_message_to_preview(
                    common::LspToPreviewMessage::HighlightFromEditor {
                        url: Some(uri),
                        offset: element.text_range().start().into(),
                    },
                );

                let range = util::node_to_lsp_range(&p);
                return Ok(Some(vec![lsp_types::DocumentHighlight { range, kind: None }]));
            }

            if p.kind() == SyntaxKind::QualifiedName
                && gp.as_ref().map_or(false, |n| n.kind() == SyntaxKind::Element)
            {
                let range = util::node_to_lsp_range(&p);

                if gp
                    .as_ref()
                    .unwrap()
                    .parent()
                    .as_ref()
                    .map_or(false, |n| n.kind() != SyntaxKind::Component)
                {
                    ctx.server_notifier.send_message_to_preview(
                        common::LspToPreviewMessage::HighlightFromEditor {
                            url: Some(uri),
                            offset: gp.unwrap().text_range().start().into(),
                        },
                    );
                }
                return Ok(Some(vec![lsp_types::DocumentHighlight { range, kind: None }]));
            }

            if let Some(value) = find_element_id_for_highlight(&tk, &p) {
                ctx.server_notifier.send_message_to_preview(
                    common::LspToPreviewMessage::HighlightFromEditor { url: None, offset: 0 },
                );
                return Ok(Some(
                    value
                        .into_iter()
                        .map(|r| lsp_types::DocumentHighlight {
                            range: util::text_range_to_lsp_range(&p.source_file, r),
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
            let version = document_cache.document_version(&uri);
            if let Some(value) = find_element_id_for_highlight(&tk, &p) {
                let edits: Vec<_> = value
                    .into_iter()
                    .map(|r| TextEdit {
                        range: util::text_range_to_lsp_range(&p.source_file, r),
                        new_text: params.new_name.clone(),
                    })
                    .collect();
                return Ok(Some(common::create_workspace_edit(uri, version, edits)));
            }
            match p.kind() {
                SyntaxKind::DeclaredIdentifier => {
                    common::rename_component::rename_component_from_definition(
                        &document_cache,
                        &p.into(),
                        &params.new_name,
                    )
                    .map(Some)
                    .map_err(|e| LspError {
                        code: LspErrorCode::RequestFailed,
                        message: e.to_string(),
                    })
                }
                _ => Err(LspError {
                    code: LspErrorCode::RequestFailed,
                    message: "This symbol cannot be renamed.".into(),
                }),
            }
        } else {
            Err(LspError {
                code: LspErrorCode::RequestFailed,
                message: "This symbol cannot be renamed.".into(),
            })
        }
    });
    rh.register::<PrepareRenameRequest, _>(|params, ctx| async move {
        let mut document_cache = ctx.document_cache.borrow_mut();
        let uri = params.text_document.uri;
        if let Some((tk, _off)) = token_descr(&mut document_cache, &uri, &params.position) {
            if find_element_id_for_highlight(&tk, &tk.parent()).is_some() {
                return Ok(Some(PrepareRenameResponse::Range(util::token_to_lsp_range(&tk))));
            }
            let p = tk.parent();
            if matches!(p.kind(), SyntaxKind::DeclaredIdentifier) {
                if let Some(gp) = p.parent() {
                    if gp.kind() == SyntaxKind::Component {
                        return Ok(Some(PrepareRenameResponse::Range(util::node_to_lsp_range(&p))));
                    }
                }
            }
        }
        Ok(None)
    });
    rh.register::<Formatting, _>(|params, ctx| async move {
        let document_cache = ctx.document_cache.borrow_mut();
        Ok(formatting::format_document(params, &document_cache))
    });
}

/// extract the parameter at given index. name is used in the error
#[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
fn extract_param<T: serde::de::DeserializeOwned>(
    params: &[serde_json::Value],
    index: usize,
    name: &str,
) -> Result<T, LspError> {
    let p = params.get(index).ok_or_else(|| LspError {
        code: LspErrorCode::InvalidParameter,
        message: format!("{} parameter is missing", name),
    })?;
    serde_json::from_value(p.clone()).map_err(|e| LspError {
        code: LspErrorCode::InvalidParameter,
        message: format!("{} parameter is invalid: {}", name, e),
    })
}

#[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
pub fn show_preview_command(
    params: &[serde_json::Value],
    ctx: &Rc<Context>,
) -> Result<(), LspError> {
    let document_cache = &mut ctx.document_cache.borrow_mut();
    let config = document_cache.compiler_configuration();

    let url: Url = extract_param(params, 0, "url")?;

    // Normalize the URL to make sure it is encoded the same way as what the preview expect from other URLs
    let url =
        common::uri_to_file(&url).and_then(|u| Url::from_file_path(u).ok()).ok_or_else(|| {
            LspError {
                code: LspErrorCode::InvalidParameter,
                message: "invalid document url".into(),
            }
        })?;

    let component =
        params.get(1).and_then(|v| v.as_str()).filter(|v| !v.is_empty()).map(|v| v.to_string());

    let c = common::PreviewComponent {
        url,
        component,
        style: config.style.clone().unwrap_or_default(),
    };
    ctx.to_show.replace(Some(c.clone()));
    ctx.server_notifier.send_message_to_preview(common::LspToPreviewMessage::ShowPreview(c));

    Ok(())
}

pub(crate) async fn reload_document_impl(
    ctx: Option<&Rc<Context>>,
    mut content: String,
    url: lsp_types::Url,
    version: Option<i32>,
    document_cache: &mut common::DocumentCache,
) -> HashMap<Url, Vec<lsp_types::Diagnostic>> {
    let Some(path) = common::uri_to_file(&url) else { return Default::default() };
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
            url: common::VersionedUrl::new(url.clone(), version),
            contents: content.clone(),
        });
    }
    let dependencies = document_cache.invalidate_url(&url);
    let mut diag = BuildDiagnostics::default();
    let _ = document_cache.load_url(&url, version, content, &mut diag).await; // ignore url conversion errors

    for dep in &dependencies {
        if ctx.is_some_and(|ctx| ctx.open_urls.borrow().contains(dep)) {
            document_cache.reload_cached_file(dep, &mut diag).await;
        }
    }

    // Always provide diagnostics for all files. Empty diagnostics clear any previous ones.
    let mut lsp_diags: HashMap<Url, Vec<lsp_types::Diagnostic>> =
        core::iter::once(Url::from_file_path(&path).unwrap())
            .chain(dependencies.iter().cloned())
            .chain(diag.all_loaded_files.iter().filter_map(|p| Url::from_file_path(&p).ok()))
            .map(|uri| (uri, Default::default()))
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

pub async fn open_document(
    ctx: &Rc<Context>,
    content: String,
    url: lsp_types::Url,
    version: Option<i32>,
    document_cache: &mut common::DocumentCache,
) -> common::Result<()> {
    ctx.open_urls.borrow_mut().insert(url.clone());

    reload_document(ctx, content, url, version, document_cache).await
}

pub async fn close_document(ctx: &Rc<Context>, url: lsp_types::Url) -> common::Result<()> {
    ctx.open_urls.borrow_mut().remove(&url);
    invalidate_document(ctx, url).await
}

pub async fn reload_document(
    ctx: &Rc<Context>,
    content: String,
    url: lsp_types::Url,
    version: Option<i32>,
    document_cache: &mut common::DocumentCache,
) -> common::Result<()> {
    let lsp_diags =
        reload_document_impl(Some(ctx), content, url.clone(), version, document_cache).await;

    for (uri, diagnostics) in lsp_diags {
        let version = document_cache.document_version(&uri);

        ctx.server_notifier.send_notification::<lsp_types::notification::PublishDiagnostics>(
            PublishDiagnosticsParams { uri, diagnostics, version },
        )?;
    }

    Ok(())
}

pub async fn invalidate_document(ctx: &Rc<Context>, url: lsp_types::Url) -> common::Result<()> {
    // The preview cares about resources and slint files, so forward everything
    ctx.server_notifier.send_message_to_preview(common::LspToPreviewMessage::InvalidateContents {
        url: url.clone(),
    });

    ctx.document_cache.borrow_mut().drop_document(&url)
}

pub async fn delete_document(ctx: &Rc<Context>, url: lsp_types::Url) -> common::Result<()> {
    // The preview cares about resources and slint files, so forward everything
    ctx.server_notifier
        .send_message_to_preview(common::LspToPreviewMessage::FileLost { url: url.clone() });

    ctx.document_cache.borrow_mut().drop_document(&url)
}

pub async fn trigger_file_watcher(
    ctx: &Rc<Context>,
    url: lsp_types::Url,
    typ: lsp_types::FileChangeType,
) -> common::Result<()> {
    if !ctx.open_urls.borrow().contains(&url) {
        if typ == lsp_types::FileChangeType::DELETED {
            delete_document(ctx, url).await?;
        } else {
            invalidate_document(ctx, url).await?;
        }
    }
    Ok(())
}

/// return the token, and the offset within the file
fn token_descr(
    document_cache: &mut common::DocumentCache,
    text_document_uri: &Url,
    pos: &Position,
) -> Option<(SyntaxToken, TextSize)> {
    let (doc, o) = document_cache.get_document_and_offset(text_document_uri, pos)?;
    let node = doc.node.as_ref()?;

    let token = token_at_offset(node, o)?;
    Some((token, o))
}

/// Return the token that matches best the token at cursor position
pub fn token_at_offset(doc: &syntax_nodes::Document, offset: TextSize) -> Option<SyntaxToken> {
    let mut taf = doc.token_at_offset(offset);
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
    Some(token)
}

fn has_experimental_client_capability(capabilities: &ClientCapabilities, name: &str) -> bool {
    capabilities
        .experimental
        .as_ref()
        .and_then(|o| o.get(name).and_then(|v| v.as_bool()))
        .unwrap_or(false)
}

fn get_code_actions(
    document_cache: &mut common::DocumentCache,
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
        let r = util::text_range_to_lsp_range(&token.source_file, node.text_range());
        let edits = vec![
            TextEdit::new(lsp_types::Range::new(r.start, r.start), "@tr(".into()),
            TextEdit::new(lsp_types::Range::new(r.end, r.end), ")".into()),
        ];
        result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
            title: "Wrap in `@tr()`".into(),
            edit: common::create_workspace_edit_from_path(
                document_cache,
                token.source_file.path(),
                edits,
            ),
            ..Default::default()
        }));
    } else if token.kind() == SyntaxKind::Identifier
        && node.kind() == SyntaxKind::QualifiedName
        && node.parent().map(|n| n.kind()) == Some(SyntaxKind::Element)
    {
        let is_lookup_error = {
            let global_tr = document_cache.global_type_registry();
            let tr = document_cache
                .get_document_for_source_file(&token.source_file)
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
                &mut |ci| !ci.is_global && ci.is_exported && ci.name == text,
                &mut |_name, file, edit| {
                    result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                        title: format!("Add import from \"{file}\""),
                        kind: Some(lsp_types::CodeActionKind::QUICKFIX),
                        edit: common::create_workspace_edit_from_path(
                            document_cache,
                            token.source_file.path(),
                            vec![edit],
                        ),
                        ..Default::default()
                    }))
                },
            );
        }

        if has_experimental_client_capability(client_capabilities, "snippetTextEdit") {
            let r = util::text_range_to_lsp_range(
                &token.source_file,
                node.parent().unwrap().text_range(),
            );
            let element = document_cache.element_at_position(&uri, &r.start);
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
                edit: common::create_workspace_edit_from_path(
                    document_cache,
                    token.source_file.path(),
                    edits,
                ),
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
                    edit: common::create_workspace_edit_from_path(
                        document_cache,
                        token.source_file.path(),
                        edits,
                    ),
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
                    edit: common::create_workspace_edit_from_path(
                        document_cache,
                        token.source_file.path(),
                        edits,
                    ),
                    ..Default::default()
                }));

                let edits = vec![TextEdit::new(
                    lsp_types::Range::new(r.start, r.start),
                    "if ${0:condition} : ".to_string(),
                )];
                result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                    title: "Make conditional".into(),
                    kind: Some(lsp_types::CodeActionKind::REFACTOR),
                    edit: common::create_workspace_edit_from_path(
                        document_cache,
                        token.source_file.path(),
                        edits,
                    ),
                    ..Default::default()
                }));
            }
        }
    }

    (!result.is_empty()).then_some(result)
}

fn get_document_color(
    document_cache: &mut common::DocumentCache,
    text_document: &lsp_types::TextDocumentIdentifier,
) -> Option<Vec<ColorInformation>> {
    let mut result = Vec::new();
    let doc = document_cache.get_document(&text_document.uri)?;
    let root_node = doc.node.as_ref()?;
    let mut token = root_node.first_token()?;
    loop {
        if token.kind() == SyntaxKind::ColorLiteral {
            (|| -> Option<()> {
                let range = util::token_to_lsp_range(&token);
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
    document_cache: &mut common::DocumentCache,
    text_document: &lsp_types::TextDocumentIdentifier,
) -> Option<DocumentSymbolResponse> {
    let doc = document_cache.get_document(&text_document.uri)?;

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
            let element_node = &root_element.debug.first()?.node;
            let component_node = syntax_nodes::Component::new(element_node.parent()?)?;
            let selection_range = util::node_to_lsp_range(&component_node.DeclaredIdentifier());
            if c.id.is_empty() {
                // Symbols with empty names are invalid
                return None;
            }

            Some(DocumentSymbol {
                range: util::node_to_lsp_range(&component_node),
                selection_range,
                name: c.id.to_string(),
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
        Type::Struct(s) if s.name.is_some() && s.node.is_some() => Some(DocumentSymbol {
            range: util::node_to_lsp_range(s.node.as_ref().unwrap().parent().as_ref()?),
            selection_range: util::node_to_lsp_range(
                &s.node.as_ref().unwrap().parent()?.child_node(SyntaxKind::DeclaredIdentifier)?,
            ),
            name: s.name.as_ref().unwrap().to_string(),
            kind: lsp_types::SymbolKind::STRUCT,
            ..ds.clone()
        }),
        Type::Enumeration(enumeration) => enumeration.node.as_ref().map(|node| DocumentSymbol {
            range: util::node_to_lsp_range(node),
            selection_range: util::node_to_lsp_range(&node.DeclaredIdentifier()),
            name: enumeration.name.to_string(),
            kind: lsp_types::SymbolKind::ENUM,
            ..ds.clone()
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
                let element_node = &e.debug.first()?.node;
                let sub_element_node = element_node.parent()?;
                debug_assert_eq!(sub_element_node.kind(), SyntaxKind::SubElement);
                Some(DocumentSymbol {
                    range: util::node_to_lsp_range(&sub_element_node),
                    selection_range: util::node_to_lsp_range(
                        element_node.QualifiedName().as_ref()?,
                    ),
                    name: e.base_type.to_string(),
                    detail: (!e.id.is_empty()).then(|| e.id.to_string()),
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
    document_cache: &mut common::DocumentCache,
    text_document: &lsp_types::TextDocumentIdentifier,
) -> Option<Vec<CodeLens>> {
    if cfg!(any(feature = "preview-builtin", feature = "preview-external")) {
        let doc = document_cache.get_document(&text_document.uri)?;

        let inner_components = doc.inner_components.clone();

        let mut r = vec![];

        // Handle preview lens
        r.extend(inner_components.iter().filter(|c| !c.is_global()).filter_map(|c| {
            Some(CodeLens {
                range: util::node_to_lsp_range(&c.root_element.borrow().debug.first()?.node),
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
) -> Option<Vec<TextRange>> {
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
                    ranges: &mut Vec<TextRange>,
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

pub async fn startup_lsp(ctx: &Context) -> common::Result<()> {
    register_file_watcher(ctx).await?;
    load_configuration(ctx).await
}

pub async fn load_configuration(ctx: &Context) -> common::Result<()> {
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

    let (hide_ui, include_paths, library_paths, style) = {
        let mut hide_ui = None;
        let mut include_paths = None;
        let mut library_paths = None;
        let mut style = None;

        for v in r {
            if let Some(o) = v.as_object() {
                if let Some(ip) = o.get("includePaths").and_then(|v| v.as_array()) {
                    if !ip.is_empty() {
                        include_paths =
                            Some(ip.iter().filter_map(|x| x.as_str()).map(PathBuf::from).collect());
                    }
                }
                if let Some(lp) = o.get("libraryPaths").and_then(|v| v.as_object()) {
                    if !lp.is_empty() {
                        library_paths = Some(
                            lp.iter()
                                .filter_map(|(k, v)| {
                                    v.as_str().map(|v| (k.to_string(), PathBuf::from(v)))
                                })
                                .collect(),
                        );
                    }
                }
                if let Some(s) =
                    o.get("preview").and_then(|v| v.as_object()?.get("style")?.as_str())
                {
                    if !s.is_empty() {
                        style = Some(s.to_string());
                    }
                }
                hide_ui = o.get("preview").and_then(|v| v.as_object()?.get("hide_ui")?.as_bool());
            }
        }
        (hide_ui, include_paths, library_paths, style)
    };

    let document_cache = &mut ctx.document_cache.borrow_mut();
    let cc = document_cache.reconfigure(style, include_paths, library_paths).await?;

    let config = common::PreviewConfig {
        hide_ui,
        style: cc.style.clone().unwrap_or_default(),
        include_paths: cc.include_paths.clone(),
        library_paths: cc.library_paths.clone(),
    };
    *ctx.preview_config.borrow_mut() = config.clone();
    ctx.server_notifier
        .send_message_to_preview(common::LspToPreviewMessage::SetConfiguration { config });
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use lsp_types::WorkspaceEdit;

    use crate::language::test::{complex_document_cache, loaded_document_cache};

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
            let (_, offset) = dc.get_document_and_offset(&uri, &pos).unwrap();
            assert_eq!(&source[usize::from(offset)..][..str.len()], str);
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
