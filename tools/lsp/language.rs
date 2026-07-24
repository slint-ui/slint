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

use crate::common::LspToPreviews;
use crate::common::uri_to_file;
use crate::{common, util};

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;
use i_slint_compiler::object_tree::{ElementRc, QualifiedTypeName};
use i_slint_compiler::parser::{
    NodeOrToken, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize, syntax_nodes,
};
use i_slint_compiler::{diagnostics::BuildDiagnostics, langtype::Type};
#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
use i_slint_live_preview::protocol::PreviewComponent;
use i_slint_live_preview::{
    file_watcher::FileChangeKind,
    protocol::{LspToPreviewMessage, PreviewConfig, SourceFileVersion, VersionedUrl},
};

use itertools::Itertools;
use lsp_types::TextDocumentPositionParams;
use lsp_types::{
    ClientCapabilities, CodeActionOrCommand, CodeActionProviderCapability, CodeLens,
    CodeLensOptions, Color, ColorInformation, ColorPresentation, Command, CompletionOptions,
    DocumentSymbol, DocumentSymbolResponse, InitializeParams, InitializeResult, OneOf, Position,
    PrepareRenameResponse, RenameOptions, SemanticTokensFullOptions, SemanticTokensLegend,
    SemanticTokensOptions, ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextEdit,
    Url, WorkDoneProgressOptions,
    request::{
        CodeActionRequest, CodeLensRequest, ColorPresentationRequest, Completion, DocumentColor,
        DocumentHighlightRequest, DocumentSymbolRequest, ExecuteCommand, Formatting,
        GotoDefinition, HoverRequest, PrepareRenameRequest, Rename, SemanticTokensFullRequest,
        SignatureHelpRequest,
    },
};

use std::cell::Cell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;

const POPULATE_COMMAND: &str = "slint/populate";
pub const SHOW_PREVIEW_COMMAND: &str = "slint/showPreview";

fn command_list() -> Vec<String> {
    vec![
        POPULATE_COMMAND.into(),
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

fn create_populate_command(
    uri: lsp_types::Url,
    version: SourceFileVersion,
    title: String,
    text: String,
) -> Command {
    let text_document = lsp_types::OptionalVersionedTextDocumentIdentifier { uri, version };
    Command::new(
        title,
        POPULATE_COMMAND.into(),
        Some(vec![serde_json::to_value(text_document).unwrap(), text.into()]),
    )
}

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
pub fn send_state_to_preview(ctx: &Context) {
    let mut doc_count = 0;
    #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
    let mut fonts_sent = HashSet::<PathBuf>::new();
    for (url, node) in ctx.document_cache.all_url_documents() {
        if url.scheme() == "builtin" {
            continue;
        }
        let version = ctx.document_cache.document_version(&url);

        ctx.to_preview.send(&LspToPreviewMessage::SetContents {
            url: VersionedUrl::new(url.clone(), version),
            contents: node.text().to_string().into(),
        });
        #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
        send_referenced_fonts(ctx, &url, &mut fonts_sent);
        doc_count += 1;
    }

    ctx.to_preview
        .send(&LspToPreviewMessage::SetConfiguration { config: ctx.preview_config.clone() });

    if let Some(c) = ctx.to_show.clone() {
        tracing::debug!("Sending state to preview: {} documents, showing {}", doc_count, c.url);
        ctx.to_preview.send(&LspToPreviewMessage::ShowPreview(c));
    } else {
        tracing::debug!(
            "Sending state to preview: {} documents, showing default component",
            doc_count
        );
    }
}

// Callers live in the native LSP (main.rs / editor.rs); not used from WASM.
#[cfg(all(
    not(target_arch = "wasm32"),
    any(feature = "preview-external", feature = "preview-engine", feature = "preview-remote"),
))]
pub fn send_files_to_preview(ctx: &Context, files: &[lsp_types::Url]) {
    #[cfg(feature = "preview-remote")]
    let mut fonts_sent = HashSet::<PathBuf>::new();
    for url in files {
        if let Some(node) = ctx.document_cache.get_document(url).and_then(|doc| doc.node.as_ref()) {
            let version = ctx.document_cache.document_version_by_path(node.source_file.path());
            let contents = node.text().to_string().into();
            tracing::debug!("Sending cached file {} to preview", url);
            ctx.to_preview.send(&LspToPreviewMessage::SetContents {
                url: VersionedUrl::new(url.clone(), version),
                contents,
            });
            #[cfg(feature = "preview-remote")]
            send_referenced_fonts(ctx, url, &mut fonts_sent);
            continue;
        }
        let Some(path) = url.to_file_path().ok() else {
            tracing::warn!("Cannot convert URL to file path: {url}");
            continue;
        };
        match std::fs::read(&path) {
            Ok(contents) => {
                tracing::debug!("Sending file {} ({} bytes) to preview", url, contents.len());
                ctx.to_preview.send(&LspToPreviewMessage::SetContents {
                    url: VersionedUrl::new(url.clone(), None),
                    contents,
                });
            }
            Err(err) => {
                tracing::warn!("Failed to read file {}: {err}", path.display());
                ctx.to_preview.send(&LspToPreviewMessage::ForgetFile { url: url.clone() });
            }
        }
    }
}

/// Read each font file imported by the `.slint` at `doc_url` and push it
/// to the remote viewer via `SetContents`. Only the remote viewer needs
/// font bytes pushed: local previews read fonts from disk. Fonts in `sent`
/// are skipped: callers seed it with fonts that were already transferred
/// (e.g. referenced by an earlier document in the same batch, or sent
/// before the current edit).
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
fn send_referenced_fonts(ctx: &Context, doc_url: &Url, sent: &mut HashSet<PathBuf>) {
    let Some(remote) = ctx.to_preview.remote() else { return };
    let Some(doc) = ctx.document_cache.get_document(doc_url) else { return };
    // `custom_fonts` holds the resolved path of every font import that
    // passed the compiler's existence check, plus remote URLs.
    for (font_path, _) in &doc.custom_fonts {
        let font_path = PathBuf::from(font_path.as_str());
        if i_slint_compiler::pathutils::is_url(&font_path) {
            continue;
        }
        if !sent.insert(font_path.clone()) {
            continue;
        }
        let Ok(font_url) = Url::from_file_path(&font_path) else {
            tracing::warn!("Cannot convert font path to URL: {}", font_path.display());
            continue;
        };
        match std::fs::read(&font_path) {
            Ok(contents) => {
                tracing::debug!(
                    "Sending font {} ({} bytes) to remote viewer",
                    font_url,
                    contents.len()
                );
                remote.send(&LspToPreviewMessage::SetContents {
                    url: VersionedUrl::new(font_url, None),
                    contents,
                });
            }
            Err(err) => {
                tracing::warn!("Failed to read font {}: {err}", font_path.display());
            }
        }
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
        tracing::trace!(
            "Client supports dynamic file watcher registration, registering for all files"
        );
        let fs_watcher = lsp_types::DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![lsp_types::FileSystemWatcher {
                glob_pattern: lsp_types::GlobPattern::String("**/*".to_string()),
                kind: Some(
                    lsp_types::WatchKind::Change
                        | lsp_types::WatchKind::Delete
                        | lsp_types::WatchKind::Create,
                ),
            }],
        };
        let server_notifier = { ctx.server_notifier.clone() };
        server_notifier
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
    pub document_cache: common::DocumentCache,
    pub preview_config: PreviewConfig,
    pub server_notifier: crate::ServerNotifier,
    pub init_param: InitializeParams,
    /// The last component for which the user clicked "show preview"
    #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
    pub to_show: Option<PreviewComponent>,
    /// File currently open in the editor
    pub open_urls: HashSet<lsp_types::Url>,
    pub to_preview: Rc<LspToPreviews>,
    /// Files to recompile after all other operations are done
    /// (i.e. recompilations triggered by updates to unopened files)
    pub pending_recompile: HashSet<lsp_types::Url>,
    /// Disables the host-language rename prompt for the rest of the session.
    /// TODO(#12111): Persist this setting across sessions.
    pub host_language_rename_dont_ask_again: Rc<Cell<bool>>,
}

/// An error from a LSP request
#[derive(Debug, Clone)]
pub struct LspError {
    pub code: LspErrorCode,
    pub message: String,
}

impl std::fmt::Display for LspError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for LspError {}

/// The code of a LspError. Correspond to the lsp_server::ErrorCode
#[derive(Debug, Clone, Copy)]
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

impl std::fmt::Display for LspErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LspErrorCode::InvalidParameter => write!(f, "Invalid Parameter"),
            LspErrorCode::InternalError => write!(f, "Internal Error"),
            LspErrorCode::RequestFailed => write!(f, "Request Failed"),
            LspErrorCode::ContentModified => write!(f, "Content Modified"),
        }
    }
}

#[derive(Default)]
pub struct RequestHandler(
    pub  HashMap<
        &'static str,
        Box<dyn Fn(serde_json::Value, &mut Context) -> Result<serde_json::Value, LspError>>,
    >,
);

impl RequestHandler {
    pub fn register<R: lsp_types::request::Request>(
        &mut self,
        handler: impl Fn(R::Params, &mut Context) -> Result<R::Result, LspError> + 'static,
    ) where
        R::Params: 'static,
    {
        self.0.insert(
            R::METHOD,
            Box::new(move |value, ctx| {
                let params = serde_json::from_value(value).map_err(|e| LspError {
                    code: LspErrorCode::InvalidParameter,
                    message: format!("error when deserializing request: {e:?}"),
                })?;
                handler(params, ctx).map(|x| serde_json::to_value(x).unwrap())
            }),
        );
    }
}

pub fn server_initialize_result(client_cap: &ClientCapabilities) -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
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
    }
}

pub fn register_request_handlers(rh: &mut RequestHandler) {
    rh.register::<GotoDefinition>(|params, ctx| {
        let result = token_descr(
            &ctx.document_cache,
            &params.text_document_position_params.text_document.uri,
            &params.text_document_position_params.position,
        )
        .and_then(|token| goto::goto_definition(&mut ctx.document_cache, token.0));
        Ok(result)
    });
    rh.register::<Completion>(|params, ctx| {
        let result = token_descr(
            &ctx.document_cache,
            &params.text_document_position.text_document.uri,
            &params.text_document_position.position,
        )
        .and_then(|token| {
            let client_caps = ctx
                .init_param
                .capabilities
                .text_document
                .as_ref()
                .and_then(|t| t.completion.clone());
            completion::completion_at(
                &mut ctx.document_cache,
                token.0,
                token.1,
                client_caps.as_ref(),
            )
            .map(Into::into)
        });
        Ok(result)
    });
    rh.register::<HoverRequest>(|params, ctx| {
        let Some((token, _text_size)) = token_descr(
            &ctx.document_cache,
            &params.text_document_position_params.text_document.uri,
            &params.text_document_position_params.position,
        ) else {
            return Ok(None);
        };

        let hover = hover::get_tooltip(&mut ctx.document_cache, token.clone());

        // we will show a tooltip in the editor, also update the highlight in the live preview
        if hover.is_some() {
            let (_document, preview) =
                get_highlights_for_position(ctx, &params.text_document_position_params);
            ctx.to_preview.send(&preview);
        }

        Ok(hover)
    });
    rh.register::<SignatureHelpRequest>(|params, ctx| {
        let result = token_descr(
            &ctx.document_cache,
            &params.text_document_position_params.text_document.uri,
            &params.text_document_position_params.position,
        )
        .and_then(|(token, _)| signature_help::get_signature_help(&mut ctx.document_cache, token));
        Ok(result)
    });
    rh.register::<CodeActionRequest>(|params, ctx| {
        let result =
            token_descr(&ctx.document_cache, &params.text_document.uri, &params.range.start)
                .and_then(|(token, _)| {
                    let capabilities = ctx.init_param.capabilities.clone();
                    get_code_actions(&mut ctx.document_cache, token, &capabilities)
                });
        Ok(result)
    });
    rh.register::<ExecuteCommand>(|params, ctx| {
        match params.command.as_str() {
            SHOW_PREVIEW_COMMAND => {
                #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
                {
                    show_preview_command(&params.arguments, ctx)?;
                }
                return Ok(None::<serde_json::Value>);
            }
            POPULATE_COMMAND => {
                let future = populate_command(&params.arguments, ctx)?;
                crate::common::spawn_local(async move {
                    if let Err(err) = future.await {
                        tracing::error!("Error executing populate command: {err}");
                    }
                });
                return Ok(None::<serde_json::Value>);
            }
            _ => {
                tracing::error!("Received unknown command {}", params.command.as_str());
            }
        }
        Ok(None::<serde_json::Value>)
    });
    rh.register::<DocumentColor>(|params, ctx| {
        Ok(get_document_color(&mut ctx.document_cache, &params.text_document).unwrap_or_default())
    });
    rh.register::<ColorPresentationRequest>(|params, _ctx| {
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
    rh.register::<DocumentSymbolRequest>(|params, ctx| {
        Ok(get_document_symbols(&mut ctx.document_cache, &params.text_document))
    });
    rh.register::<CodeLensRequest>(|params, ctx| {
        Ok(get_code_lenses(&mut ctx.document_cache, &params.text_document))
    });
    rh.register::<SemanticTokensFullRequest>(|params, ctx| {
        Ok(semantic_tokens::get_semantic_tokens(&mut ctx.document_cache, &params.text_document))
    });
    rh.register::<DocumentHighlightRequest>(|params, ctx| {
        tracing::trace!(
            "DocumentHighlightRequest for {} at {:?}",
            params.text_document_position_params.text_document.uri,
            params.text_document_position_params.position
        );

        let (document_highlights, preview) =
            get_highlights_for_position(ctx, &params.text_document_position_params);

        // Update the highlight in the live preview.
        // We do this even if there are no highlights to clear any previous highlights.
        ctx.to_preview.send(&preview);

        let not_empty = !document_highlights.is_empty();
        Ok(not_empty.then_some(document_highlights))
    });
    rh.register::<Rename>(|params, ctx| {
        let uri = params.text_document_position.text_document.uri;
        if let Some((tk, _off)) =
            token_descr(&ctx.document_cache, &uri, &params.text_document_position.position)
        {
            let p = tk.parent();
            let version = ctx.document_cache.document_version(&uri);
            if let Some(value) = common::rename_element_id::find_element_ids(&tk, &p) {
                let edits: Vec<_> = value
                    .into_iter()
                    .map(|r| TextEdit {
                        range: util::text_range_to_lsp_range(
                            &p.source_file,
                            r,
                            ctx.document_cache.format,
                        ),
                        new_text: params.new_name.clone(),
                    })
                    .collect();
                return Ok(Some(common::create_workspace_edit(uri, version, edits)));
            }
            if let Some(declaration_node) =
                common::rename_component::find_declaration_node(&ctx.document_cache, &tk)
            {
                let edit = declaration_node.rename(&ctx.document_cache, &params.new_name).map_err(
                    |e| LspError { code: LspErrorCode::RequestFailed, message: e.to_string() },
                )?;
                // After the synchronous slint-only rename, ask the user (once)
                // whether to also search and replace the generated Rust/C++
                // accessors. The dialog and follow-up edit have to
                // run asynchronously: the request handler is synchronous and
                // must return the slint edit immediately.
                #[cfg(not(target_arch = "wasm32"))]
                schedule_host_language_rename_followup(ctx, &declaration_node, &params.new_name);
                return Ok(Some(edit));
            }
        }

        Err(LspError {
            code: LspErrorCode::RequestFailed,
            message: "This symbol cannot be renamed.".into(),
        })
    });
    rh.register::<PrepareRenameRequest>(|params, ctx| {
        let uri = params.text_document.uri;
        if let Some((tk, _)) = token_descr(&ctx.document_cache, &uri, &params.position) {
            if common::rename_element_id::find_element_ids(&tk, &tk.parent()).is_some() {
                return Ok(Some(PrepareRenameResponse::Range(util::token_to_lsp_range(
                    &tk,
                    ctx.document_cache.format,
                ))));
            }
            if common::rename_component::find_declaration_node(&ctx.document_cache, &tk).is_some() {
                return Ok(Some(PrepareRenameResponse::Range(util::token_to_lsp_range(
                    &tk,
                    ctx.document_cache.format,
                ))));
            }
        }
        Ok(None)
    });
    rh.register::<Formatting>(|params, ctx| {
        Ok(formatting::format_document(params, &ctx.document_cache))
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
        message: format!("{name} parameter is missing"),
    })?;
    serde_json::from_value(p.clone()).map_err(|e| LspError {
        code: LspErrorCode::InvalidParameter,
        message: format!("{name} parameter is invalid: {e}"),
    })
}

#[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
pub fn show_preview_command(
    params: &[serde_json::Value],
    ctx: &mut Context,
) -> Result<(), LspError> {
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

    tracing::debug!("Show preview: url={}, component={:?}", url, component);
    let c = PreviewComponent { url, component };
    show_preview(c, ctx);

    Ok(())
}

#[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
pub fn show_preview(component: PreviewComponent, ctx: &mut Context) {
    ctx.pending_recompile.insert(component.url.clone());
    ctx.to_show = Some(component.clone());
    ctx.to_preview.send(&LspToPreviewMessage::ShowPreview(component));
}

fn populate_command_range(
    node: &SyntaxNode,
    format: common::ByteFormat,
) -> Option<lsp_types::Range> {
    let range = node.text_range();

    let start_offset = node
        .text()
        .find_char('\u{0002}')
        .and_then(|s| s.checked_add(1.into()))
        .unwrap_or(range.start());
    let end_offset = node.text().find_char('\u{0003}').unwrap_or(range.end());

    (start_offset <= end_offset).then_some(util::text_range_to_lsp_range(
        &node.source_file,
        TextRange::new(start_offset, end_offset),
        format,
    ))
}

pub fn populate_command(
    params: &[serde_json::Value],
    ctx: &mut Context,
) -> Result<impl Future<Output = Result<serde_json::Value, LspError>> + 'static, LspError> {
    let text_document =
        serde_json::from_value::<lsp_types::OptionalVersionedTextDocumentIdentifier>(
            params
                .first()
                .ok_or_else(|| LspError {
                    code: LspErrorCode::InvalidParameter,
                    message: "No textdocument provided".into(),
                })?
                .clone(),
        )
        .map_err(|_| LspError {
            code: LspErrorCode::InvalidParameter,
            message: "First parameter is not a OptionalVersionedTextDocumentIdentifier".into(),
        })?;
    let new_text = serde_json::from_value::<String>(
        params
            .get(1)
            .ok_or_else(|| LspError {
                code: LspErrorCode::InvalidParameter,
                message: "No code to insert".into(),
            })?
            .clone(),
    )
    .map_err(|_| LspError {
        code: LspErrorCode::InvalidParameter,
        message: "Invalid second parameter".into(),
    })?;

    let edit = {
        let document_cache = &mut ctx.document_cache;
        let uri = text_document.uri;
        let version = document_cache.document_version(&uri);

        if let Some(source_version) = text_document.version {
            if let Some(current_version) = version {
                if current_version != source_version {
                    return Err(LspError {
                        code: LspErrorCode::InvalidParameter,
                        message: "Document version mismatch".into(),
                    });
                }
            } else {
                return Err(LspError {
                    code: LspErrorCode::InvalidParameter,
                    message: format!("Document with uri {uri} not found in cache"),
                });
            }
        }

        let Some(doc) = document_cache.get_document(&uri) else {
            return Err(LspError {
                code: LspErrorCode::InvalidParameter,
                message: "Document not in cache".into(),
            });
        };
        let Some(node) = &doc.node else {
            return Err(LspError {
                code: LspErrorCode::InvalidParameter,
                message: "Document has no node".into(),
            });
        };

        let Some(range) = populate_command_range(node, document_cache.format) else {
            return Err(LspError {
                code: LspErrorCode::InvalidParameter,
                message: "No slint code range in document".into(),
            });
        };

        let edit = lsp_types::TextEdit { range, new_text };
        common::create_workspace_edit(uri, version, vec![edit])
    };

    let server_notifier = ctx.server_notifier.clone();

    Ok(async move {
        let response = server_notifier
            .send_request::<lsp_types::request::ApplyWorkspaceEdit>(
                lsp_types::ApplyWorkspaceEditParams {
                    label: Some("Populate empty file".into()),
                    edit,
                },
            )
            .map_err(|_| LspError {
                code: LspErrorCode::RequestFailed,
                message: "Failed to send populate edit".into(),
            })?
            .await
            .map_err(|_| LspError {
                code: LspErrorCode::RequestFailed,
                message: "Failed to send populate edit".into(),
            })?;

        if !response.applied {
            return Err(LspError {
                code: LspErrorCode::RequestFailed,
                message: "Failed to apply population edit".into(),
            });
        }

        Ok(serde_json::to_value(()).expect("Failed to serialize ()!"))
    })
}

/// Synchronous step in the host-language rename flow: classify the
/// declaration the user just renamed, and -- if it is exposed in the
/// generated Rust/C++ public API and the rename actually changes the
/// accessor name -- spawn the async dialog + scanner + applyEdit follow-up.
///
/// The handler that calls this has already returned the slint-only
/// `WorkspaceEdit` synchronously, so by the time the dialog appears the
/// `.slint` file has already been updated by the client. The host-language
/// scanner runs against the *old* accessor name on disk and produces a
/// second `workspace/applyEdit` from the spawned task; the editor's
/// undo/redo treats the two edits independently. That is intentional: if
/// the user rejects the host-language rewrite (or the scanner errors),
/// the slint rename still stands.
#[cfg(not(target_arch = "wasm32"))]
fn schedule_host_language_rename_followup(
    ctx: &Context,
    declaration_node: &common::rename_component::DeclarationNode,
    new_name: &str,
) {
    let Some(info) = declaration_node.host_language_classification(&ctx.document_cache) else {
        return;
    };
    // No-op when the slint rename normalizes to the same identifier
    // (e.g. `my-count` -> `my_count`): accessor names are unchanged so the
    // scanner would only produce identical-text TextEdits.
    if i_slint_compiler::parser::normalize_identifier(new_name) == info.old_name {
        return;
    }
    if ctx.host_language_rename_dont_ask_again.get() {
        return;
    }

    let server_notifier = ctx.server_notifier.clone();
    let dont_ask_again = ctx.host_language_rename_dont_ask_again.clone();
    let init_param = ctx.init_param.clone();
    let format = ctx.document_cache.format;
    let new_name = new_name.to_string();

    crate::common::spawn_local(async move {
        // Folders can change after initialization; query the client now
        // rather than reusing the InitializeParams snapshot.
        let workspace_folders =
            common::host_language_search::current_workspace_folders(&server_notifier, &init_param)
                .await;
        run_host_language_rename_followup(
            server_notifier,
            dont_ask_again,
            workspace_folders,
            format,
            info,
            new_name,
        )
        .await;
    });
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_host_language_rename_followup(
    server_notifier: crate::ServerNotifier,
    dont_ask_again: Rc<Cell<bool>>,
    workspace_folders: Vec<lsp_types::WorkspaceFolder>,
    format: common::ByteFormat,
    info: common::rename_component::HostLanguageRenameInfo,
    new_name: String,
) {
    use i_slint_compiler::generator::accessor_names::DeclarationKind;

    let kind_label = match info.kind {
        DeclarationKind::Property => "property",
        DeclarationKind::Callback => "callback",
        DeclarationKind::Function => "function",
    };
    let message = format!(
        "Slint {kind_label} '{}' is exposed to host language code. \
         Search & Replace the Rust/C++ code with the new accessors? \
         (The Slint rename has already been applied.)",
        info.old_name,
    );
    let action_replace = "🔍 Search & Replace Rust/C++ accessors";
    let action_skip = "⏩ Skip";
    let action_never = "⏭️ Skip and don't ask again";

    let action_items = vec![
        lsp_types::MessageActionItem {
            title: action_replace.into(),
            properties: Default::default(),
        },
        lsp_types::MessageActionItem { title: action_skip.into(), properties: Default::default() },
        lsp_types::MessageActionItem { title: action_never.into(), properties: Default::default() },
    ];

    let request = match server_notifier.send_request::<lsp_types::request::ShowMessageRequest>(
        lsp_types::ShowMessageRequestParams {
            typ: lsp_types::MessageType::INFO,
            message,
            actions: Some(action_items),
        },
    ) {
        Ok(fut) => fut,
        Err(e) => {
            tracing::warn!("Slint host-language rename: failed to send prompt: {e}");
            return;
        }
    };
    let chosen = match request.await {
        Ok(Some(item)) => Some(item.title),
        Ok(None) => None, // user dismissed
        Err(e) => {
            tracing::warn!("Slint host-language rename: prompt request failed: {e}");
            return;
        }
    };

    match chosen.as_deref() {
        Some(title) if title == action_replace => {
            let scan_result = common::host_language_search::search_replace_host_language_accessors(
                &workspace_folders,
                info.kind,
                &info.old_name,
                &new_name,
                format,
                common::host_language_search::ScanBounds::DEFAULT,
            );
            match scan_result {
                Ok(edits) if edits.is_empty() => {
                    show_info(
                        &server_notifier,
                        "Slint rename applied; no matching Rust/C++ accessor identifiers found in the workspace.",
                    );
                }
                Ok(edits) => {
                    let file_count = edits.iter().map(|e| &e.url).collect::<HashSet<_>>().len();
                    let edit_count = edits.len();
                    let workspace_edit =
                        common::create_workspace_edit_from_single_text_edits(edits);
                    apply_host_language_edits(
                        &server_notifier,
                        workspace_edit,
                        edit_count,
                        file_count,
                    )
                    .await;
                }
                Err(e) => {
                    show_warning(
                        &server_notifier,
                        format!(
                            "Slint rename applied; host-language scan failed: {e}. \
                             Rust/C++ accessor identifiers were not replaced."
                        ),
                    );
                }
            }
        }
        Some(title) if title == action_never => {
            dont_ask_again.set(true);
            show_info(
                &server_notifier,
                "Slint rename applied; skipping host-language rewrite (won't ask again this session).",
            );
        }
        _ => {
            show_info(
                &server_notifier,
                "Slint rename applied; host-language Rust/C++ accessors were not rewritten.",
            );
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn apply_host_language_edits(
    server_notifier: &crate::ServerNotifier,
    edit: lsp_types::WorkspaceEdit,
    edit_count: usize,
    file_count: usize,
) {
    let request = match server_notifier.send_request::<lsp_types::request::ApplyWorkspaceEdit>(
        lsp_types::ApplyWorkspaceEditParams {
            label: Some("Search & Replace Rust/C++ accessors for Slint rename".into()),
            edit,
        },
    ) {
        Ok(fut) => fut,
        Err(e) => {
            show_warning(
                server_notifier,
                format!("Failed to send host-language WorkspaceEdit: {e}."),
            );
            return;
        }
    };
    match request.await {
        Ok(response) if response.applied => {
            show_info(
                server_notifier,
                format!(
                    "Replaced {edit_count} Rust/C++ accessor occurrence{} across {file_count} file{}.",
                    if edit_count == 1 { "" } else { "s" },
                    if file_count == 1 { "" } else { "s" },
                ),
            );
        }
        Ok(_) => {
            show_warning(
                server_notifier,
                "Client refused to apply the host-language rename WorkspaceEdit.",
            );
        }
        Err(e) => {
            show_warning(
                server_notifier,
                format!("Failed to apply host-language WorkspaceEdit: {e}."),
            );
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn show_info(server_notifier: &crate::ServerNotifier, message: impl Into<String>) {
    let _ = server_notifier.send_notification::<lsp_types::notification::ShowMessage>(
        lsp_types::ShowMessageParams { typ: lsp_types::MessageType::INFO, message: message.into() },
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn show_warning(server_notifier: &crate::ServerNotifier, message: impl Into<String>) {
    let _ = server_notifier.send_notification::<lsp_types::notification::ShowMessage>(
        lsp_types::ShowMessageParams {
            typ: lsp_types::MessageType::WARNING,
            message: message.into(),
        },
    );
}

pub(crate) async fn load_document_impl(
    ctx: &mut Context,
    content: String,
    url: lsp_types::Url,
    version: Option<i32>,
) -> (HashSet<PathBuf>, BuildDiagnostics) {
    enum FileAction {
        ProcessContent(String),
        IgnoreFile,
        InvalidateFile,
    }

    tracing::trace!("Loading document: {url} (version: {version:?})");

    let Some(path) = common::uri_to_file(&url) else { return Default::default() };
    // Normalize the URL
    let Ok(url) = Url::from_file_path(path.clone()) else { return Default::default() };

    let action = if path.extension().is_some_and(|e| e == "rs") {
        match i_slint_compiler::lexer::extract_rust_macro(content) {
            Some(content) => FileAction::ProcessContent(content),
            // A rust file without a rust macro, just ignore it
            None => {
                if ctx.document_cache.get_document(&url).is_some() {
                    // This had contents before: Continue so we can invalidate it!
                    FileAction::InvalidateFile
                } else {
                    FileAction::IgnoreFile
                }
            }
        }
    } else {
        FileAction::ProcessContent(content)
    };

    let mut diag = BuildDiagnostics::default();

    let dependencies = match action {
        FileAction::ProcessContent(content) => {
            ctx.to_preview.send(&LspToPreviewMessage::SetContents {
                url: VersionedUrl::new(url.clone(), version),
                contents: content.clone().into(),
            });
            // Fonts imported before this edit were pushed to the remote viewer
            // already; seed the sent set with them so only fonts added by this
            // edit are transferred.
            #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
            let mut fonts_sent: HashSet<PathBuf> = ctx
                .document_cache
                .get_document(&url)
                .map(|doc| {
                    doc.custom_fonts.iter().map(|(p, _)| PathBuf::from(p.as_str())).collect()
                })
                .unwrap_or_default();
            let dependencies: HashSet<Url> = ctx.document_cache.invalidate_url(&url);
            let _ = ctx.document_cache.load_url(&url, version, content, &mut diag).await;
            #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
            send_referenced_fonts(ctx, &url, &mut fonts_sent);
            dependencies
        }
        FileAction::IgnoreFile => return Default::default(),
        FileAction::InvalidateFile => {
            ctx.to_preview.send(&LspToPreviewMessage::ForgetFile { url: url.clone() });
            ctx.document_cache.invalidate_url(&url)
        }
    };

    for dep in &dependencies {
        if ctx.open_urls.contains(dep) {
            ctx.document_cache.reload_cached_file(dep, &mut diag).await;
        }
    }

    let extra_files =
        dependencies.iter().filter_map(common::uri_to_file).chain(core::iter::once(path)).collect();

    (extra_files, diag)
}

pub async fn open_document(
    ctx: &mut Context,
    content: String,
    url: lsp_types::Url,
    version: Option<i32>,
) -> common::Result<()> {
    tracing::debug!("Opening document: {url}");
    ctx.open_urls.insert(url.clone());

    load_document(ctx, content, url, version).await
}

pub async fn close_document(ctx: &mut Context, url: lsp_types::Url) -> common::Result<()> {
    tracing::debug!("Closing document: {url}");
    ctx.open_urls.remove(&url);
    drop_document(ctx, url).await
}

pub async fn load_document(
    ctx: &mut Context,
    content: String,
    url: lsp_types::Url,
    version: Option<i32>,
) -> common::Result<()> {
    let (extra_files, diag) = load_document_impl(ctx, content, url.clone(), version).await;

    tracing::debug!("Loaded {url} with {} diagnostics", diag.iter().count());

    send_diagnostics(&ctx.server_notifier, &ctx.document_cache, &extra_files, diag);

    Ok(())
}

#[cfg_attr(target_arch = "wasm32", allow(unused))]
pub async fn reload_document(ctx: &mut Context, url: lsp_types::Url) -> common::Result<()> {
    tracing::debug!("Reloading document: {url}");

    // Check if document is in cache (can use reload_cached_file)
    let in_cache = ctx.document_cache.all_urls().contains(&url);

    if in_cache {
        tracing::trace!("Document is in cache, reloading: {url}");

        let mut diagnostics = BuildDiagnostics::default();

        ctx.document_cache.reload_cached_file(&url, &mut diagnostics).await;
        let mut extra_files = HashSet::new();
        extra_files.extend(uri_to_file(&url));

        send_diagnostics(&ctx.server_notifier, &ctx.document_cache, &extra_files, diagnostics);
    } else {
        tracing::trace!("Document not in cache, loading from disk: {url}");

        let Some(path) = common::uri_to_file(&url) else {
            // The file was likely deleted, log and move on
            tracing::debug!("Failed to locate file: {url}");
            return Ok(());
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => load_document(ctx, content, url, None).await?,
            // The file was likely deleted, log and move on
            Err(err) => tracing::debug!("Failed to read {} from disk: {err}", path.display()),
        };
    }

    Ok(())
}

pub fn convert_diagnostics(
    extra_files: &HashSet<PathBuf>,
    diag: BuildDiagnostics,
    format: common::ByteFormat,
) -> HashMap<Url, Vec<lsp_types::Diagnostic>> {
    // Always provide diagnostics for all files. Empty diagnostics clear any previous ones.
    let mut lsp_diags: HashMap<Url, Vec<lsp_types::Diagnostic>> = extra_files
        .iter()
        .chain(diag.all_loaded_files.iter())
        .filter_map(|p| Url::from_file_path(p).ok())
        .map(|uri| (uri, Default::default()))
        .collect();

    for d in diag.into_iter() {
        #[cfg(not(target_arch = "wasm32"))]
        if d.source_file().unwrap().is_relative() {
            continue;
        }
        let uri = Url::from_file_path(d.source_file().unwrap()).unwrap();
        lsp_diags
            .entry(uri)
            .or_default()
            .push(i_slint_live_preview::protocol::to_lsp_diagnostic(&d, format));
    }

    lsp_diags
}

fn send_diagnostics(
    _server_notifier: &crate::ServerNotifier,
    document_cache: &common::DocumentCache,
    extra_files: &HashSet<PathBuf>,
    diag: BuildDiagnostics,
) {
    let lsp_diags = convert_diagnostics(extra_files, diag, document_cache.format);
    tracing::trace!("Sending {} diagnostics to editor", lsp_diags.values().flatten().count());

    for (uri, _diagnostics) in lsp_diags {
        let _version = document_cache.document_version(&uri);

        #[cfg(feature = "preview-engine")]
        let _ = common::lsp_to_editor::notify_lsp_diagnostics(
            _server_notifier,
            uri,
            _version,
            _diagnostics,
        );
    }
}

fn drop_document_impl(ctx: &mut Context, url: lsp_types::Url) -> common::Result<()> {
    let dependencies = ctx.document_cache.drop_document(&url)?;

    let open_dependencies = ctx.open_urls.intersection(&dependencies).cloned();
    ctx.pending_recompile.extend(open_dependencies);

    #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
    if let Some(preview_url) = ctx.to_show.as_ref().map(|c| c.url.clone()) {
        // The external preview only has access to the files the LSP recompiled, so we need to
        // ensure the preview file is recompiled if anything it depends on changes, even if it's
        // not in the open_urls.
        if preview_url == url || dependencies.contains(&preview_url) {
            ctx.pending_recompile.insert(preview_url);
        }
    }

    Ok(())
}

pub async fn drop_document(ctx: &mut Context, url: lsp_types::Url) -> common::Result<()> {
    tracing::debug!("Dropping document: {url}");
    // The preview cares about resources and slint files, so forward everything
    ctx.to_preview.send(&LspToPreviewMessage::InvalidateContents { url: url.clone() });

    drop_document_impl(ctx, url)
}

pub async fn delete_document(ctx: &mut Context, url: lsp_types::Url) -> common::Result<()> {
    tracing::debug!("Deleting document: {url}");
    // The preview cares about resources and slint files, so forward everything
    ctx.to_preview.send(&LspToPreviewMessage::ForgetFile { url: url.clone() });

    #[cfg(feature = "preview-engine")]
    let version = ctx.document_cache.document_version(&url);

    let result = drop_document_impl(ctx, url.clone());

    // make sure to clear the diagnostics on this file.
    // This is especially important for deleted files, but also for renamed files to clear the diagnostics on the old file.
    // Otherwise they will stick around forever (e.g. in VS Code).
    #[cfg(feature = "preview-engine")]
    let _ =
        common::lsp_to_editor::notify_lsp_diagnostics(&ctx.server_notifier, url, version, vec![]);

    result
}

pub async fn trigger_file_watcher(
    ctx: &mut Context,
    url: lsp_types::Url,
    typ: FileChangeKind,
) -> common::Result<()> {
    if !ctx.open_urls.contains(&url) {
        tracing::debug!("File watcher triggered for {url} (type: {:?})", typ);
        match typ {
            FileChangeKind::Deleted => delete_document(ctx, url).await?,
            // If the file was newly created, we still need to drop it as another file may
            // already depend on it by trying to import it before it exists.
            // This is especially common on file renames.
            // See also #11304
            FileChangeKind::Changed | FileChangeKind::Created => drop_document(ctx, url).await?,
        }
    } else {
        tracing::trace!("Ignoring file watcher event for open document: {url}");
    }
    Ok(())
}

/// return the token, and the offset within the file
fn token_descr(
    document_cache: &common::DocumentCache,
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
    let mut result = Vec::new();

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
        })
        .or_else(|| syntax_nodes::ExportsList::new(node.clone()).and_then(|n| n.Component()))
        .filter(|c| c.child_text(SyntaxKind::Identifier).is_none_or(|t| t != "global"));

    #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
    {
        if let Some(component) = &component
            && let Some(component_name) =
                i_slint_compiler::parser::identifier_text(&component.DeclaredIdentifier())
        {
            result.push(CodeActionOrCommand::Command(create_show_preview_command(
                false,
                &uri,
                &component_name,
            )))
        }
    }

    if token.kind() == SyntaxKind::StringLiteral && node.kind() == SyntaxKind::Expression {
        let r = util::text_range_to_lsp_range(
            &token.source_file,
            node.text_range(),
            document_cache.format,
        );
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
            completion::build_component_import_statements_edits(
                &token,
                document_cache,
                &mut |ci| !ci.is_global && ci.is_exported && ci.name == text,
                &mut |name, file, edit| {
                    result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                        title: format!("import {{ {name} }} from \"{file}\""),
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
                document_cache.format,
            );
            let element = document_cache.element_at_position(&uri, &r.start);
            let element_indent = element.as_ref().and_then(util::find_element_indent);
            let indented_lines = node
                .parent()
                .unwrap()
                .text()
                .to_string()
                .lines()
                .map(|line| if line.is_empty() { line.to_string() } else { format!("    {line}") })
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
                            && t.next_sibling_or_token().is_some_and(|n| is_sub_element(n.kind()))
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
    } else if token.kind() == SyntaxKind::Identifier
        && node.kind() == SyntaxKind::QualifiedName
        && node.parent().map(|n| n.kind()) == Some(SyntaxKind::Type)
    {
        // Offer "Add import from ..." for unresolved type references in property/callback/
        // function type annotations (e.g. `property <MyStruct> foo`).
        let is_lookup_error =
            syntax_nodes::QualifiedName::new(node.clone()).is_some_and(|qualified_name| {
                let qual = QualifiedTypeName::from_node(qualified_name);
                let global_tr = document_cache.global_type_registry();
                let tr = document_cache
                    .get_document_for_source_file(&token.source_file)
                    .map(|doc| &doc.local_registry)
                    .unwrap_or(&global_tr);
                matches!(tr.lookup_qualified(&qual.members), Type::Invalid)
            });
        if is_lookup_error {
            let text = token.text();
            completion::build_type_import_statements_edits(
                &token,
                document_cache,
                &mut |type_info| type_info.name == text,
                &mut |_type_info, name, file, edit| {
                    result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                        title: format!("import {{ {name} }} from \"{file}\""),
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
    } else if token.kind() == SyntaxKind::Identifier
        && node.kind() == SyntaxKind::QualifiedName
        && node.parent().map(|n| n.kind()) == Some(SyntaxKind::Expression)
        && node.children_with_tokens().filter(|n| n.kind() == SyntaxKind::Identifier).count() == 1
    {
        // Qualify a bare identifier that doesn't resolve but is an enum value or named color
        // (`red` -> `Colors.red`). Like the import action above, re-derive the lookup error rather
        // than reading diagnostics, so nothing is offered when the identifier does resolve.
        use i_slint_compiler::lookup::LookupObject;
        let is_lookup_error = util::with_lookup_ctx(document_cache, node.clone(), None, |ctx| {
            let name = i_slint_compiler::parser::normalize_identifier(token.text());
            i_slint_compiler::lookup::global_lookup().lookup(ctx, &name).is_none()
        })
        .unwrap_or(true);
        if is_lookup_error {
            let suggestions = {
                let global_tr = document_cache.global_type_registry();
                let tr = document_cache
                    .get_document_for_source_file(&token.source_file)
                    .map(|doc| &doc.local_registry)
                    .unwrap_or(&global_tr);
                i_slint_compiler::lookup::enum_or_color_suggestions(tr, token.text())
            };
            let range = util::text_range_to_lsp_range(
                &token.source_file,
                token.text_range(),
                document_cache.format,
            );
            for suggestion in suggestions {
                result.push(CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                    title: format!("Qualify as '{suggestion}'"),
                    kind: Some(lsp_types::CodeActionKind::QUICKFIX),
                    edit: common::create_workspace_edit_from_path(
                        document_cache,
                        token.source_file.path(),
                        vec![TextEdit::new(range, suggestion.to_string())],
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
                let range = util::token_to_lsp_range(&token, document_cache.format);
                let col = i_slint_common::color_parsing::parse_color_literal(token.text())?;
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
            let selection_range = util::node_to_lsp_range(
                &component_node.DeclaredIdentifier(),
                document_cache.format,
            );
            if c.id.is_empty() {
                // Symbols with empty names are invalid
                return None;
            }

            Some(DocumentSymbol {
                range: util::node_to_lsp_range(&component_node, document_cache.format),
                selection_range,
                name: c.id.to_string(),
                kind: if c.is_global() {
                    lsp_types::SymbolKind::OBJECT
                } else {
                    lsp_types::SymbolKind::CLASS
                },
                children: gen_children(&c.root_element, &ds, document_cache.format),
                ..ds.clone()
            })
        })
        .collect::<Vec<_>>();

    r.extend(inner_types.iter().filter_map(|c| match c {
        Type::Struct(s) => s.node().and_then(|node| {
            Some(DocumentSymbol {
                range: util::node_to_lsp_range(node.parent().as_ref()?, document_cache.format),
                selection_range: util::node_to_lsp_range(
                    &node.parent()?.child_node(SyntaxKind::DeclaredIdentifier)?,
                    document_cache.format,
                ),
                name: s.name.slint_name().unwrap().to_string(),
                kind: lsp_types::SymbolKind::STRUCT,
                ..ds.clone()
            })
        }),
        Type::Enumeration(enumeration) => enumeration.node.as_ref().map(|node| DocumentSymbol {
            range: util::node_to_lsp_range(node, document_cache.format),
            selection_range: util::node_to_lsp_range(
                &node.DeclaredIdentifier(),
                document_cache.format,
            ),
            name: enumeration.name.to_string(),
            kind: lsp_types::SymbolKind::ENUM,
            ..ds.clone()
        }),
        _ => None,
    }));

    fn gen_children(
        elem: &ElementRc,
        ds: &DocumentSymbol,
        format: common::ByteFormat,
    ) -> Option<Vec<DocumentSymbol>> {
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
                    range: util::node_to_lsp_range(&sub_element_node, format),
                    selection_range: util::node_to_lsp_range(
                        element_node.QualifiedName().as_ref()?,
                        format,
                    ),
                    name: e.base_type.to_string(),
                    detail: (!e.id.is_empty()).then(|| e.id.to_string()),
                    kind: lsp_types::SymbolKind::VARIABLE,
                    children: gen_children(child, ds, format),
                    ..ds.clone()
                })
            })
            .collect::<Vec<_>>();
        (!r.is_empty()).then_some(r)
    }

    r.sort_by_key(|a| a.range.start);

    #[cfg(debug_assertions)]
    fn check_ranges(r: &[DocumentSymbol]) {
        // Make sure that the selection range is inside the range as this causes JS error in vscode
        for s in r {
            assert!(
                s.range.start <= s.selection_range.start && s.range.end >= s.selection_range.end,
                "Invalid range for {s:?}",
            );
            if let Some(children) = &s.children {
                check_ranges(children);
            }
        }
    }
    #[cfg(debug_assertions)]
    check_ranges(&r);

    Some(r.into())
}

fn get_code_lenses(
    document_cache: &mut common::DocumentCache,
    text_document: &lsp_types::TextDocumentIdentifier,
) -> Option<Vec<CodeLens>> {
    let doc = document_cache.get_document(&text_document.uri)?;
    let version = document_cache.document_version(&text_document.uri);

    let mut result = Vec::new();

    if cfg!(any(feature = "preview-builtin", feature = "preview-external")) {
        let inner_components = doc.inner_components.clone();

        // Handle preview lens
        result.extend(inner_components.iter().filter(|c| !c.is_global()).filter_map(|c| {
            let component_node = c.root_element.borrow().debug.first()?.node.parent()?;
            let range = match component_node.parent() {
                Some(parent) if parent.kind() == SyntaxKind::ExportsList => {
                    util::node_to_lsp_range(&parent, document_cache.format)
                }
                _ => util::node_to_lsp_range(&component_node, document_cache.format),
            };
            let command =
                Some(create_show_preview_command(true, &text_document.uri, c.id.as_str()));
            Some(CodeLens { range, command, data: None })
        }));
    }

    if let Some(node) = &doc.node {
        let has_non_ws_token = node
            .children_with_tokens()
            .any(|nt| nt.kind() != SyntaxKind::Whitespace && nt.kind() != SyntaxKind::Eof);
        if !has_non_ws_token
            && let Some(range) = populate_command_range(node, document_cache.format)
        {
            result.push(CodeLens {
                range,
                command: Some(create_populate_command(
                    text_document.uri.clone(),
                    version,
                    "Start with Hello World!".to_string(),
                    r#"import { AboutSlint, VerticalBox } from "std-widgets.slint";

export component MainWindow inherits Window {
    VerticalBox {
        Text {
            text: "Hello World!";
        }
        AboutSlint {
            preferred-height: 150px;
        }
    }
}
"#
                    .to_string(),
                )),
                data: None,
            });
        }
    }

    (!result.is_empty()).then_some(result)
}

/// Returns the list of DocumentHighlights, plus a LspToPreviewMessage::HighlightFromEditor  for the
/// given position
/// The list of DocumentHighlights will be empty if there is no highlight to show in the editor
/// The HighlightFromEditor will have url=None if there is no highlight to show in the preview
fn get_highlights_for_position(
    ctx: &Context,
    params: &TextDocumentPositionParams,
) -> (Vec<lsp_types::DocumentHighlight>, LspToPreviewMessage) {
    let uri = params.text_document.uri.clone();
    if let Some((token, _)) = token_descr(&ctx.document_cache, &uri, &params.position) {
        let parent = token.parent();
        let grand_parent = parent.parent();

        if parent.kind() == SyntaxKind::DeclaredIdentifier
            && grand_parent.as_ref().is_some_and(|n| n.kind() == SyntaxKind::Component)
        {
            let element = grand_parent.as_ref().unwrap().child_node(SyntaxKind::Element).unwrap();

            let preview_highlight = LspToPreviewMessage::HighlightFromEditor {
                url: Some(uri),
                offset: element.text_range().start().into(),
            };

            let range = util::node_to_lsp_range(&parent, ctx.document_cache.format);
            return (vec![lsp_types::DocumentHighlight { range, kind: None }], preview_highlight);
        }

        if parent.kind() == SyntaxKind::QualifiedName
            && grand_parent.as_ref().is_some_and(|n| n.kind() == SyntaxKind::Element)
        {
            let great_grand_parent = grand_parent.as_ref().unwrap().parent();
            let should_highlight_preview =
                great_grand_parent.as_ref().is_some_and(|n| n.kind() != SyntaxKind::Component);
            let preview_highlight = LspToPreviewMessage::HighlightFromEditor {
                url: should_highlight_preview.then_some(uri),
                offset: grand_parent.unwrap().text_range().start().into(),
            };
            let range = util::node_to_lsp_range(&parent, ctx.document_cache.format);

            return (vec![lsp_types::DocumentHighlight { range, kind: None }], preview_highlight);
        }

        if let Some(value) = common::rename_element_id::find_element_ids(&token, &parent) {
            let preview_highlight =
                LspToPreviewMessage::HighlightFromEditor { url: None, offset: 0 };
            let document_highlight = value
                .into_iter()
                .map(|r| lsp_types::DocumentHighlight {
                    range: util::text_range_to_lsp_range(
                        &parent.source_file,
                        r,
                        ctx.document_cache.format,
                    ),
                    kind: None,
                })
                .collect();
            return (document_highlight, preview_highlight);
        }
    }
    (vec![], LspToPreviewMessage::HighlightFromEditor { url: None, offset: 0 })
}

pub async fn startup_lsp(ctx: &mut Context) -> common::Result<()> {
    register_file_watcher(ctx).await?;
    load_configuration(ctx).await
}

#[derive(Debug)]
struct WorkspaceConfig {
    hide_ui: Option<bool>,
    include_paths: Option<Vec<PathBuf>>,
    library_paths: Option<HashMap<String, PathBuf>>,
    style: Option<String>,
    experimental: bool,
}

fn parse_configuration(workspace_config: Vec<serde_json::Value>) -> WorkspaceConfig {
    let mut hide_ui = None;
    let mut include_paths = None;
    let mut library_paths = None;
    let mut style = None;
    let mut experimental = false;

    for config_value in workspace_config {
        if let Some(config_object) = config_value.as_object() {
            if let Some(ip) = config_object.get("includePaths").and_then(|v| v.as_array())
                && !ip.is_empty()
            {
                include_paths = Some(
                    ip.iter().filter_map(serde_json::Value::as_str).map(PathBuf::from).collect(),
                );
            }
            if let Some(lp) = config_object.get("libraryPaths").and_then(|v| v.as_object())
                && !lp.is_empty()
            {
                library_paths = Some(
                    lp.iter()
                        .filter_map(|(key, value)| {
                            value.as_str().map(|v| (key.to_string(), PathBuf::from(v)))
                        })
                        .collect(),
                );
            }
            if let Some(s) =
                config_object.get("preview").and_then(|v| v.as_object()?.get("style")?.as_str())
                && !s.is_empty()
            {
                style = Some(s.to_string());
            }
            hide_ui =
                config_object.get("preview").and_then(|v| v.as_object()?.get("hide_ui")?.as_bool());
            if config_object.get("experimental").and_then(|v| v.as_bool()) == Some(true) {
                experimental = true;
            }
        }
    }
    WorkspaceConfig { hide_ui, include_paths, library_paths, style, experimental }
}

pub async fn load_configuration(ctx: &mut Context) -> common::Result<()> {
    tracing::debug!("Loading configuration from client");

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

    let workspace_config = ctx
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

    let workspace_config = parse_configuration(workspace_config);
    tracing::debug!("Loaded configuration: {workspace_config:?}");
    let WorkspaceConfig { hide_ui, include_paths, library_paths, style, experimental } =
        workspace_config;

    let mut diag = BuildDiagnostics::default();
    let (cc, all_files) = ctx
        .document_cache
        .reconfigure(style, include_paths, library_paths, experimental, &mut diag)
        .await;

    {
        send_diagnostics(
            &ctx.server_notifier,
            &ctx.document_cache,
            &all_files.iter().filter_map(common::uri_to_file).collect(),
            diag,
        );
    }

    let config = PreviewConfig {
        hide_ui,
        style: cc.style.clone().unwrap_or_default(),
        include_paths: cc.include_paths.clone(),
        library_paths: cc.library_paths.clone(),
        format_utf8: cc.format == common::ByteFormat::Utf8,
        enable_experimental: cc.enable_experimental,
    };
    {
        ctx.preview_config = config.clone();
        ctx.to_preview.send(&LspToPreviewMessage::SetConfiguration { config });
    }

    tracing::debug!("Loaded configuration from client");

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::test;
    use super::*;

    use crate::language::test::{
        complex_document_cache, loaded_document_cache, loaded_document_cache_with_file_name,
    };
    use lsp_server::{Message, Request, Response};
    use lsp_types::{
        ApplyWorkspaceEditResponse, MessageActionItem, WorkspaceEdit, WorkspaceFolder,
    };

    struct TestLspClient {
        receiver: crossbeam_channel::Receiver<Message>,
        queue: crate::OutgoingRequestQueue,
    }

    impl TestLspClient {
        fn next_request(&self, expected_method: &str) -> Request {
            let message = self
                .receiver
                .recv_timeout(std::time::Duration::from_secs(2))
                .expect("expected an LSP request");
            let Message::Request(request) = message else {
                panic!("expected request, got {message:?}");
            };
            assert_eq!(request.method, expected_method);
            request
        }

        fn respond(&self, request: Request, result: impl serde::Serialize) {
            let mut entry = loop {
                if let Some(entry) = self.queue.get_mut(&request.id) {
                    break entry;
                }
                std::thread::yield_now();
            };
            if let crate::OutgoingRequest::Pending(waker) = &*entry {
                waker.wake_by_ref();
            }
            *entry = crate::OutgoingRequest::Done(Response::new_ok(
                request.id,
                serde_json::to_value(result).unwrap(),
            ));
        }

        fn next_show_message(&self) -> lsp_types::ShowMessageParams {
            let message = self
                .receiver
                .recv_timeout(std::time::Duration::from_secs(2))
                .expect("expected a show-message notification");
            let Message::Notification(notification) = message else {
                panic!("expected notification, got {message:?}");
            };
            assert_eq!(notification.method, "window/showMessage");
            serde_json::from_value(notification.params).unwrap()
        }
    }

    fn test_lsp_client() -> (crate::ServerNotifier, TestLspClient) {
        let (sender, receiver) = crossbeam_channel::unbounded();
        let queue = crate::OutgoingRequestQueue::default();
        (crate::ServerNotifier { sender, queue: queue.clone() }, TestLspClient { receiver, queue })
    }

    fn poll_future<F: Future>(future: std::pin::Pin<&mut F>) -> std::task::Poll<F::Output> {
        let waker = std::task::Waker::noop();
        let mut context = std::task::Context::from_waker(waker);
        future.poll(&mut context)
    }

    #[test]
    fn host_language_followup_can_disable_prompts_for_the_session() {
        let (notifier, client) = test_lsp_client();
        let dont_ask_again = Rc::new(Cell::new(false));
        let info = common::rename_component::HostLanguageRenameInfo {
            kind: i_slint_compiler::generator::accessor_names::DeclarationKind::Property,
            old_name: "count".into(),
        };
        let mut future = Box::pin(run_host_language_rename_followup(
            notifier,
            dont_ask_again.clone(),
            Vec::new(),
            common::ByteFormat::Utf16,
            info,
            "total".into(),
        ));

        assert!(poll_future(future.as_mut()).is_pending());
        let request = client.next_request("window/showMessageRequest");
        let params: lsp_types::ShowMessageRequestParams =
            serde_json::from_value(request.params.clone()).unwrap();
        let actions: Vec<_> = params.actions.unwrap().into_iter().map(|a| a.title).collect();
        assert_eq!(
            actions,
            ["🔍 Search & Replace Rust/C++ accessors", "⏩ Skip", "⏭️ Skip and don't ask again"]
        );
        client.respond(
            request,
            Some(MessageActionItem {
                title: "⏭️ Skip and don't ask again".into(),
                properties: Default::default(),
            }),
        );

        assert!(poll_future(future.as_mut()).is_ready());
        assert!(dont_ask_again.get());
        assert!(client.next_show_message().message.contains("won't ask again this session"));
    }

    #[test]
    fn session_suppression_prevents_a_later_prompt() {
        let (document_cache, url, diagnostics) =
            loaded_document_cache("export component App { in property <int> count; }".into());
        assert!(diagnostics.get(&url).unwrap().is_empty());
        let document = document_cache.get_document(&url).unwrap().node.as_ref().unwrap();
        let offset = document.text().to_string().find("count").unwrap() as u32;
        let token = document
            .token_at_offset(offset.into())
            .find(|token| token.kind() == SyntaxKind::Identifier)
            .unwrap();
        let declaration =
            common::rename_component::find_declaration_node(&document_cache, &token).unwrap();
        let (notifier, client) = test_lsp_client();
        let mut context = test::mock_context();
        context.document_cache = document_cache;
        context.server_notifier = notifier;
        context.host_language_rename_dont_ask_again.set(true);

        schedule_host_language_rename_followup(&context, &declaration, "total");

        assert!(matches!(
            client.receiver.recv_timeout(std::time::Duration::from_millis(50)),
            Err(crossbeam_channel::RecvTimeoutError::Timeout)
        ));
    }

    #[test]
    fn host_language_followup_requests_and_reports_the_second_workspace_edit() {
        let path = std::env::temp_dir().join(format!(
            "slint-lsp-followup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("main.rs"), "obj.get_count();").unwrap();
        let folders =
            vec![WorkspaceFolder { uri: Url::from_file_path(&path).unwrap(), name: "test".into() }];
        let (notifier, client) = test_lsp_client();
        let info = common::rename_component::HostLanguageRenameInfo {
            kind: i_slint_compiler::generator::accessor_names::DeclarationKind::Property,
            old_name: "count".into(),
        };
        let mut future = Box::pin(run_host_language_rename_followup(
            notifier,
            Rc::new(Cell::new(false)),
            folders,
            common::ByteFormat::Utf16,
            info,
            "total".into(),
        ));

        assert!(poll_future(future.as_mut()).is_pending());
        let prompt = client.next_request("window/showMessageRequest");
        client.respond(
            prompt,
            Some(MessageActionItem {
                title: "🔍 Search & Replace Rust/C++ accessors".into(),
                properties: Default::default(),
            }),
        );
        assert!(poll_future(future.as_mut()).is_pending());
        let apply = client.next_request("workspace/applyEdit");
        let params: lsp_types::ApplyWorkspaceEditParams =
            serde_json::from_value(apply.params.clone()).unwrap();
        assert_eq!(
            params.label.as_deref(),
            Some("Search & Replace Rust/C++ accessors for Slint rename")
        );
        assert!(serde_json::to_string(&params.edit).unwrap().contains("get_total"));
        client.respond(
            apply,
            ApplyWorkspaceEditResponse { applied: true, failure_reason: None, failed_change: None },
        );

        assert!(poll_future(future.as_mut()).is_ready());
        assert_eq!(
            client.next_show_message().message,
            "Replaced 1 Rust/C++ accessor occurrence across 1 file."
        );
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn refused_host_language_workspace_edit_is_reported() {
        let (notifier, client) = test_lsp_client();
        let mut future =
            Box::pin(apply_host_language_edits(&notifier, WorkspaceEdit::default(), 1, 1));

        assert!(poll_future(future.as_mut()).is_pending());
        let apply = client.next_request("workspace/applyEdit");
        client.respond(
            apply,
            ApplyWorkspaceEditResponse {
                applied: false,
                failure_reason: Some("no".into()),
                failed_change: None,
            },
        );

        assert!(poll_future(future.as_mut()).is_ready());
        let message = client.next_show_message();
        assert_eq!(message.typ, lsp_types::MessageType::WARNING);
        assert!(message.message.contains("Client refused"));
    }

    #[test]
    fn current_workspace_folders_queries_capable_clients() {
        let (notifier, client) = test_lsp_client();
        let expected = WorkspaceFolder {
            uri: Url::parse("file:///current-workspace").unwrap(),
            name: "current".into(),
        };
        let mut init = InitializeParams::default();
        init.capabilities.workspace = Some(lsp_types::WorkspaceClientCapabilities {
            workspace_folders: Some(true),
            ..Default::default()
        });
        let mut future =
            Box::pin(common::host_language_search::current_workspace_folders(&notifier, &init));

        assert!(poll_future(future.as_mut()).is_pending());
        let request = client.next_request("workspace/workspaceFolders");
        client.respond(request, Some(vec![expected.clone()]));

        assert_eq!(poll_future(future.as_mut()), std::task::Poll::Ready(vec![expected]));
    }

    #[test]
    fn test_load_document_invalid_contents() {
        let (_, url, diag) = loaded_document_cache("This is not valid!".into());

        assert!(diag.len() == 1); // Only one URL is known

        let diagnostics = diag.get(&url).expect("URL not found in result");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(lsp_types::DiagnosticSeverity::ERROR));
    }

    #[test]
    fn test_load_document_valid_contents() {
        let (_, url, diag) =
            loaded_document_cache(r#"export component Main inherits Rectangle { }"#.into());

        assert_eq!(diag.len(), 1); // Only one URL is known
        let diagnostics = diag.get(&url).expect("URL not found in result");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_reload_invalid_url() {
        // An invalid URL may be reloaded if the file has been deleted on disk.
        //
        // In that case, make sure we do not return an error, as that would crash the LSP.
        // The reload_document function is a best-effort anyway.
        let mut ctx = test::mock_context();
        spin_on::spin_on(reload_document(
            &mut ctx,
            Url::parse("file:///non/existent/file.slint").unwrap(),
        ))
        .expect("reload_document failed");
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
    fn test_document_symbols_syntax_error() {
        let (mut dc, uri, _) =
            loaded_document_cache(r#"component foo { xxx := {} /*--*/ yyy := }"#.into());
        let result =
            get_document_symbols(&mut dc, &lsp_types::TextDocumentIdentifier { uri }).unwrap();
        let mk_range = |r: std::ops::Range<u32>| {
            lsp_types::Range::new(Position::new(0, r.start), Position::new(0, r.end))
        };

        if let DocumentSymbolResponse::Nested(result) = result {
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].name, "foo");
            assert_eq!(result[0].range, mk_range(0..41));
            assert_eq!(result[0].selection_range, mk_range(10..13));
            let children = result[0].children.as_ref().unwrap();
            assert_eq!(children.len(), 2);
            assert_eq!(children[0].name, "<error>");
            assert_eq!(children[0].range, mk_range(16..25));
            assert_eq!(children[0].detail, Some("xxx".into()));
            assert_eq!(children[0].selection_range, mk_range(23..23));
            assert_eq!(children[1].name, "<error>");
            assert_eq!(children[1].range, mk_range(33..40));
            assert_eq!(children[1].detail, Some("yyy".into()));
            assert_eq!(children[1].selection_range, mk_range(40..40));
        } else {
            unreachable!();
        }
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
}
export struct NoPreviewForStruct { x: int }
export global NoPreviewForGlobal {}
"#
            .into(),
        );
        let mut capabilities = ClientCapabilities::default();

        let text_literal = lsp_types::Range::new(Position::new(7, 18), Position::new(7, 32));
        assert_eq!(
            token_descr(&dc, &url, &text_literal.start).and_then(|(token, _)| get_code_actions(
                &mut dc,
                token,
                &capabilities
            )),
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
                token_descr(&dc, &url, &pos).and_then(|(token, _)| get_code_actions(
                    &mut dc,
                    token,
                    &capabilities
                )),
                None
            );

            capabilities.experimental = Some(serde_json::json!({"snippetTextEdit": true}));
            assert_eq!(
                token_descr(&dc, &url, &pos).and_then(|(token, _)| get_code_actions(
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
            token_descr(&dc, &url, &horizontal_box.start).and_then(|(token, _)| get_code_actions(
                &mut dc,
                token,
                &capabilities
            )),
            None
        );

        capabilities.experimental = Some(serde_json::json!({"snippetTextEdit": true}));
        assert_eq!(
            token_descr(&dc, &url, &horizontal_box.start).and_then(|(token, _)| get_code_actions(
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
            token_descr(&dc, &url, &line_edit).and_then(|(token, _)| get_code_actions(
                &mut dc,
                token,
                &capabilities
            )),
            Some(vec![CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                title: "import { LineEdit } from \"std-widgets.slint\"".into(),
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

        #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
        for col in [
            0,  // "export"
            8,  // "component"
            22, // "TestWindow"
            42, // "Window"
        ] {
            let pos = Position::new(2, col);
            assert_eq!(
                token_descr(&dc, &url, &pos).and_then(|(token, _)| get_code_actions(
                    &mut dc,
                    token,
                    &capabilities
                )),
                Some(vec![CodeActionOrCommand::Command(Command::new(
                    "Show Preview".into(),
                    SHOW_PREVIEW_COMMAND.into(),
                    Some(vec![url.as_str().into(), "TestWindow".into()]),
                ))]),
                "show preview missing {pos:?}"
            );
        }

        // Test that we don't get a show preview action for struct and globals
        for line in [27, 28] {
            let pos = Position::new(line, 15);
            let token = token_descr(&dc, &url, &pos).unwrap().0;
            assert!(token.text().starts_with("NoPreviewFor"));
            assert_eq!(get_code_actions(&mut dc, token, &capabilities), None);
        }
    }

    #[test]
    fn test_add_import_for_type_annotation() {
        // Document that uses a user-defined exported struct from a separate file as a
        // property type, but doesn't import it yet.  The code action should be offered
        // on the unresolved type identifier, and should NOT be offered once imported.
        //
        // We load the "types" file first so the DocumentCache knows about it, then
        // load the main file that references the struct without importing it.
        let types_content = r#"export struct MyPoint { x: int, y: int }
export enum MyDirection { Up, Down, Left, Right }
"#;
        let main_content_without_import = r#"export component TestWindow inherits Window {
    property <MyPoint> position;
}
"#;
        let main_content_with_import = r#"import { MyPoint } from "types.slint";

export component TestWindow inherits Window {
    property <MyPoint> position;
}
"#;

        let types_url = Url::from_file_path(common::test::test_file_name("types.slint")).unwrap();
        let main_url = Url::from_file_path(common::test::test_file_name("main.slint")).unwrap();

        // Load the types file first so the cache knows about it
        let mut dc = test::empty_document_cache();
        spin_on::spin_on(dc.preload_builtins());
        let mut diagnostics = BuildDiagnostics::default();
        let _ = spin_on::spin_on(dc.load_url(
            &types_url,
            Some(1),
            types_content.to_string(),
            &mut diagnostics,
        ));

        // Load the main file (no import for MyPoint yet) with version 42 for test assertions
        let _ = spin_on::spin_on(dc.load_url(
            &main_url,
            Some(42),
            main_content_without_import.to_string(),
            &mut diagnostics,
        ));

        let capabilities = ClientCapabilities::default();

        // Cursor on "MyPoint" in `property <MyPoint> position;` (line 1, col 14)
        // Line 1: `    property <MyPoint> position;`
        // Col 13: `<`, col 14: `M`
        let type_pos = Position::new(1, 14);
        let action = token_descr(&dc, &main_url, &type_pos)
            .and_then(|(token, _)| get_code_actions(&mut dc, token, &capabilities));

        assert_eq!(
            action,
            Some(vec![CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                title: "import { MyPoint } from \"types.slint\"".into(),
                kind: Some(lsp_types::CodeActionKind::QUICKFIX),
                edit: Some(WorkspaceEdit {
                    document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
                        lsp_types::TextDocumentEdit {
                            text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                                version: Some(42),
                                uri: main_url.clone(),
                            },
                            // New import line at the top (no existing imports to merge into)
                            edits: vec![lsp_types::OneOf::Left(TextEdit::new(
                                lsp_types::Range::new(Position::new(0, 0), Position::new(0, 0)),
                                "import { MyPoint } from \"types.slint\";\n".into()
                            ))]
                        }
                    ])),
                    ..Default::default()
                }),
                ..Default::default()
            })])
        );

        // Now load the version WITH the import — action should no longer appear
        let mut document_cache_with_import = test::empty_document_cache();
        spin_on::spin_on(document_cache_with_import.preload_builtins());
        let mut diagnostics = BuildDiagnostics::default();
        let _ = spin_on::spin_on(document_cache_with_import.load_url(
            &types_url,
            Some(1),
            types_content.to_string(),
            &mut diagnostics,
        ));
        let _ = spin_on::spin_on(document_cache_with_import.load_url(
            &main_url,
            Some(42),
            main_content_with_import.to_string(),
            &mut diagnostics,
        ));

        // Cursor on "MyPoint" — now on line 3 col 14 (because the import line is added)
        let action2 = token_descr(&document_cache_with_import, &main_url, &Position::new(3, 14))
            .and_then(|(token, _)| {
                get_code_actions(&mut document_cache_with_import, token, &capabilities)
            });
        assert_eq!(action2, None, "import action should not appear when type is already imported");
    }

    #[test]
    fn test_qualify_enum_or_color() {
        // Build the expected "Qualify as ..." quick-fix that replaces `range` with `new_text`.
        let expect_qualify = |new_text: &str, range: lsp_types::Range, uri: &Url| {
            CodeActionOrCommand::CodeAction(lsp_types::CodeAction {
                title: format!("Qualify as '{new_text}'"),
                kind: Some(lsp_types::CodeActionKind::QUICKFIX),
                edit: Some(WorkspaceEdit {
                    document_changes: Some(lsp_types::DocumentChanges::Edits(vec![
                        lsp_types::TextDocumentEdit {
                            text_document: lsp_types::OptionalVersionedTextDocumentIdentifier {
                                version: Some(42),
                                uri: uri.clone(),
                            },
                            edits: vec![lsp_types::OneOf::Left(TextEdit::new(
                                range,
                                new_text.into(),
                            ))],
                        },
                    ])),
                    ..Default::default()
                }),
                ..Default::default()
            })
        };

        let capabilities = ClientCapabilities::default();

        // A named color used where no color type is expected does not resolve, so the code action
        // offers to qualify `red` as `Colors.red`.
        let (mut dc, url, _) = test::loaded_document_cache(
            r#"export component TestCase {
    property <string> s: red;
}
"#
            .into(),
        );

        // Cursor on `red` in `    property <string> s: red;` (line 1, col 25)
        let pos = Position::new(1, 25);
        let range = lsp_types::Range::new(Position::new(1, 25), Position::new(1, 28));

        let action = token_descr(&dc, &url, &pos)
            .and_then(|(token, _)| get_code_actions(&mut dc, token, &capabilities));
        assert_eq!(action, Some(vec![expect_qualify("Colors.red", range, &url)]));

        // The same `red` bound to a `color` property resolves, so no action is offered.
        let (mut dc, url, _) = test::loaded_document_cache(
            r#"export component TestCase {
    property <color> c: red;
}
"#
            .into(),
        );
        // Cursor on `red` in `    property <color> c: red;` (line 1, col 24)
        let action = token_descr(&dc, &url, &Position::new(1, 24))
            .and_then(|(token, _)| get_code_actions(&mut dc, token, &capabilities));
        assert_eq!(action, None);

        // A value shared by several enums: one quick-fix per match (the menu is not capped),
        // sorted by the qualified name.
        let (mut dc, url, _) = test::loaded_document_cache(
            r#"enum EA { shared_value }
enum EB { shared_value }
export component TestCase {
    property <int> foo: shared_value;
}
"#
            .into(),
        );

        // Cursor on `shared_value` in `    property <int> foo: shared_value;` (line 3, col 24)
        let pos = Position::new(3, 24);
        let range = lsp_types::Range::new(Position::new(3, 24), Position::new(3, 36));

        let action = token_descr(&dc, &url, &pos)
            .and_then(|(token, _)| get_code_actions(&mut dc, token, &capabilities));
        assert_eq!(
            action,
            Some(vec![
                expect_qualify("EA.shared-value", range, &url),
                expect_qualify("EB.shared-value", range, &url),
            ])
        );
    }

    #[test]
    fn test_hello_world_code_lens_slint_file() {
        // Empty slint document:
        let (mut dc, url, _) = loaded_document_cache(
            "
  \t
\t     \t

"
            .into(),
        );

        assert_eq!(
            get_code_lenses(&mut dc, &lsp_types::TextDocumentIdentifier { uri: url.clone() }),
            Some(vec![lsp_types::CodeLens {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(0, 0),
                    lsp_types::Position::new(4, 0)
                ),
                command: Some(lsp_types::Command {
                    title: "Start with Hello World!".to_string(),
                    command: POPULATE_COMMAND.to_string(),
                    arguments: Some(vec![
                        serde_json::to_value(lsp_types::OptionalVersionedTextDocumentIdentifier {
                            uri: url,
                            version: Some(42)
                        })
                        .unwrap(),
                        r#"import { AboutSlint, VerticalBox } from "std-widgets.slint";

export component MainWindow inherits Window {
    VerticalBox {
        Text {
            text: "Hello World!";
        }
        AboutSlint {
            preferred-height: 150px;
        }
    }
}
"#
                        .into()
                    ]),
                }),
                data: None,
            }])
        );
    }

    #[test]
    fn test_hello_world_code_lens_rust_file() {
        // Empty slint document in rust macro:
        let (mut dc, url, _) = loaded_document_cache_with_file_name(
            "
use slint::Model;

slint!(\t
  \t
\t       \t

)

fn main() {{
    println!(\"Hello World\");
}}
"
            .into(),
            "bar.rs",
        );

        assert_eq!(
            get_code_lenses(&mut dc, &lsp_types::TextDocumentIdentifier { uri: url.clone() }),
            Some(vec![lsp_types::CodeLens {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(3, 7),
                    lsp_types::Position::new(7, 0)
                ),
                command: Some(lsp_types::Command {
                    title: "Start with Hello World!".to_string(),
                    command: POPULATE_COMMAND.to_string(),
                    arguments: Some(vec![
                        serde_json::to_value(lsp_types::OptionalVersionedTextDocumentIdentifier {
                            uri: url,
                            version: Some(42)
                        })
                        .unwrap(),
                        r#"import { AboutSlint, VerticalBox } from "std-widgets.slint";

export component MainWindow inherits Window {
    VerticalBox {
        Text {
            text: "Hello World!";
        }
        AboutSlint {
            preferred-height: 150px;
        }
    }
}
"#
                        .into()
                    ]),
                }),
                data: None,
            }])
        );
    }

    #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
    #[test]
    fn test_show_preview_code_lens() {
        // Empty slint document:
        let (mut dc, url, _) = loaded_document_cache(
            r#"
component Internal { }

export component Test {
   FooBar := Rectangle {}
}

global Xyz {}
export { Global }
"#
            .into(),
        );

        assert_eq!(
            get_code_lenses(&mut dc, &lsp_types::TextDocumentIdentifier { uri: url.clone() }),
            Some(vec![
                lsp_types::CodeLens {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(1, 0),
                        lsp_types::Position::new(1, 22)
                    ),
                    command: Some(lsp_types::Command {
                        title: "▶ Show Preview".to_string(),
                        command: SHOW_PREVIEW_COMMAND.to_string(),
                        arguments: Some(vec![
                            serde_json::to_value(url.clone()).unwrap(),
                            "Internal".into()
                        ]),
                    }),
                    data: None,
                },
                lsp_types::CodeLens {
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(3, 0),
                        lsp_types::Position::new(5, 1)
                    ),
                    command: Some(lsp_types::Command {
                        title: "▶ Show Preview".to_string(),
                        command: SHOW_PREVIEW_COMMAND.to_string(),
                        arguments: Some(vec![
                            serde_json::to_value(url.clone()).unwrap(),
                            "Test".into()
                        ])
                    }),
                    data: None,
                }
            ])
        );
    }
}
