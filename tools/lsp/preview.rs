// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Mutex,
};

use crate::{common::PreviewComponent, lsp_ext::Health};
use i_slint_core::component_factory::FactoryContext;
use slint_interpreter::{ComponentDefinition, ComponentHandle, ComponentInstance};

use lsp_types::notification::Notification;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

mod ui;
#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
mod wasm;
#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
pub use wasm::*;
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
pub use native::*;

#[derive(Default)]
struct ContentCache {
    source_code: HashMap<PathBuf, String>,
    dependency: HashSet<PathBuf>,
    current: PreviewComponent,
    highlight: Option<(PathBuf, u32)>,
    ui_is_visible: bool,
    design_mode: bool,
    default_style: String,
}

static CONTENT_CACHE: std::sync::OnceLock<Mutex<ContentCache>> = std::sync::OnceLock::new();

pub fn set_contents(path: &Path, content: String) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    cache.source_code.insert(path.to_owned(), content);
    if cache.dependency.contains(path) {
        let current = cache.current.clone();
        let ui_is_visible = cache.ui_is_visible;

        drop(cache);

        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.ui_is_visible = ui_is_visible;

        if ui_is_visible {
            load_preview(current);
        }
    }
}

fn set_design_mode(enable: bool) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    cache.design_mode = enable;

    configure_design_mode(enable);
}

fn change_style(style: String) {
    let component = {
        let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        PreviewComponent { style, ..cache.current.clone() }
    };
    load_preview(component);
}

#[cfg(target_arch = "wasm32")]
fn get_current_style() -> String {
    let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    cache.current.style.clone()
}

pub fn start_parsing() {
    set_busy(true);
    send_status("Loading Preview…", Health::Ok);
}

pub fn finish_parsing(ok: bool) {
    set_busy(false);
    if ok {
        send_status("Preview Loaded", Health::Ok);
    } else {
        send_status("Preview not updated", Health::Error);
    }
}

pub fn config_changed(
    style: &str,
    include_paths: &[PathBuf],
    library_paths: &HashMap<String, PathBuf>,
) {
    if let Some(cache) = CONTENT_CACHE.get() {
        let mut cache = cache.lock().unwrap();
        let style = style.to_string();
        if cache.current.style != style
            || cache.current.include_paths != include_paths
            || cache.current.library_paths != *library_paths
        {
            cache.current.include_paths = include_paths.to_vec();
            cache.current.library_paths = library_paths.clone();

            let current = cache.current.clone();
            let ui_is_visible = cache.ui_is_visible;

            drop(cache);

            let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
            cache.default_style = style;
            cache.ui_is_visible = ui_is_visible;

            if ui_is_visible {
                load_preview(current);
            }
        }
    };
}

/// If the file is in the cache, returns it.
/// In any way, register it as a dependency
fn get_file_from_cache(path: PathBuf) -> Option<String> {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let r = cache.source_code.get(&path).cloned();
    cache.dependency.insert(path);
    r
}

async fn reload_preview(preview_component: PreviewComponent) {
    let (preview_component, design_mode, ui_is_visible) = {
        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        let style = cache.current.style.clone();
        cache.dependency.clear();
        let component = if preview_component.style.is_empty() {
            PreviewComponent { style, ..preview_component }
        } else {
            preview_component
        };
        cache.current = component.clone();
        (component, cache.design_mode, cache.ui_is_visible)
    };
    if !ui_is_visible {
        return;
    };

    start_parsing();

    let mut builder = slint_interpreter::ComponentCompiler::default();

    #[cfg(target_arch = "wasm32")]
    {
        let cc = builder.compiler_configuration(i_slint_core::InternalToken);
        cc.resource_url_mapper = resource_url_mapper();
    }

    if !preview_component.style.is_empty() {
        builder.set_style(preview_component.style);
    }
    builder.set_include_paths(preview_component.include_paths);
    builder.set_library_paths(preview_component.library_paths);

    builder.set_file_loader(|path| {
        let path = path.to_owned();
        Box::pin(async move { get_file_from_cache(path).map(Result::Ok) })
    });

    let compiled = if let Some(mut from_cache) = get_file_from_cache(preview_component.path.clone())
    {
        if let Some(component) = &preview_component.component {
            from_cache =
                format!("{}\nexport component _Preview inherits {} {{ }}\n", from_cache, component);
        }
        builder.build_from_source(from_cache, preview_component.path).await
    } else {
        builder.build_from_path(preview_component.path).await
    };

    notify_diagnostics(builder.diagnostics());

    if let Some(compiled) = compiled {
        update_preview_area(compiled);
        finish_parsing(true);
    } else {
        finish_parsing(false);
    }

    configure_design_mode(design_mode);
}

/// This sets up the preview area to show the ComponentInstance
///
/// This must be run in the UI thread.
pub fn set_preview_factory(
    ui: &ui::PreviewUi,
    compiled: ComponentDefinition,
    callback: Box<dyn Fn(ComponentInstance)>,
) {
    let factory = slint::ComponentFactory::new(move |ctx: FactoryContext| {
        let instance = compiled.create_embedded(ctx).unwrap();

        if let Some((path, offset)) =
            CONTENT_CACHE.get().and_then(|c| c.lock().unwrap().highlight.clone())
        {
            instance.highlight(path, offset);
        }

        callback(instance.clone_strong());

        Some(instance)
    });
    ui.set_preview_area(factory);
}

/// Highlight the element pointed at the offset in the path.
/// When path is None, remove the highlight.
pub fn highlight(path: &Option<PathBuf>, offset: u32) {
    let highlight = path.clone().map(|x| (x, offset));
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();

    if cache.highlight == highlight {
        return;
    }
    cache.highlight = highlight;

    if cache.highlight.as_ref().map_or(true, |(path, _)| cache.dependency.contains(path)) {
        let path = path.clone().unwrap_or_default();
        update_highlight(path, offset);
    }
}

pub fn show_document_request_from_element_callback(
    file: &str,
    start_line: u32,
    start_column: u32,
    _end_line: u32,
    end_column: u32,
) -> Option<lsp_types::ShowDocumentParams> {
    use lsp_types::{Position, Range, ShowDocumentParams, Url};

    if file.is_empty() || start_column == 0 || end_column == 0 {
        return None;
    }

    let start_pos = Position::new(start_line.saturating_sub(1), start_column.saturating_sub(1));
    // let end_pos = Position::new(end_line.saturating_sub(1), end_column.saturating_sub(1));
    // Place the cursor at the start of the range and do not mark up the entire range!
    let selection = Some(Range::new(start_pos, start_pos));

    Url::from_file_path(file).ok().map(|uri| ShowDocumentParams {
        uri,
        external: Some(false),
        take_focus: Some(true),
        selection,
    })
}

pub fn convert_diagnostics(
    diagnostics: &[slint_interpreter::Diagnostic],
) -> HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>> {
    let mut result: HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>> = Default::default();
    for d in diagnostics {
        if d.source_file().map_or(true, |f| f.is_relative()) {
            continue;
        }
        let uri = lsp_types::Url::from_file_path(d.source_file().unwrap()).unwrap();
        result.entry(uri).or_default().push(crate::util::to_lsp_diag(d));
    }
    result
}

pub fn notify_lsp_diagnostics(
    sender: &crate::ServerNotifier,
    uri: lsp_types::Url,
    diagnostics: Vec<lsp_types::Diagnostic>,
) -> Option<()> {
    sender
        .send_notification(
            "textDocument/publishDiagnostics".into(),
            lsp_types::PublishDiagnosticsParams { uri, diagnostics, version: None },
        )
        .ok()
}

pub fn send_status_notification(sender: &crate::ServerNotifier, message: &str, health: Health) {
    sender
        .send_notification(
            crate::lsp_ext::ServerStatusNotification::METHOD.into(),
            crate::lsp_ext::ServerStatusParams {
                health,
                quiescent: false,
                message: Some(message.into()),
            },
        )
        .unwrap_or_else(|e| eprintln!("Error sending notification: {:?}", e));
}

#[cfg(feature = "preview-external")]
pub fn ask_editor_to_show_document(
    sender: &crate::ServerNotifier,
    file: &str,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
) {
    let Some(params) = crate::preview::show_document_request_from_element_callback(
        file,
        start_line,
        start_column,
        end_line,
        end_column,
    ) else {
        return;
    };
    let Ok(fut) = sender.send_request::<lsp_types::request::ShowDocument>(params) else {
        return;
    };
    i_slint_core::future::spawn_local(fut).unwrap();
}
