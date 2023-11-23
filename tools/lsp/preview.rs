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

#[derive(Default, Copy, Clone, PartialEq, Eq, Debug)]
enum PreviewFutureState {
    /// The preview future is currently no running
    #[default]
    Pending,
    /// The preview future has been started, but we haven't started compiling
    PreLoading,
    /// The preview future is currently loading the preview
    Loading,
    /// The preview future is currently loading an outdated preview, we should abort loading and restart loading again
    NeedsReload,
}

#[derive(Default)]
struct ContentCache {
    source_code: HashMap<PathBuf, String>,
    dependency: HashSet<PathBuf>,
    current: PreviewComponent,
    loading_state: PreviewFutureState,
    highlight: Option<(PathBuf, u32)>,
    ui_is_visible: bool,
    design_mode: bool,
    default_style: String,
    // Duplicate this information in case the UI is not up yet.
    show_preview_ui: bool,
}

static CONTENT_CACHE: std::sync::OnceLock<Mutex<ContentCache>> = std::sync::OnceLock::new();

pub fn set_contents(path: &Path, content: String) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let old = cache.source_code.insert(path.to_owned(), content.clone());
    if cache.dependency.contains(path) {
        if let Some(old) = old {
            if content == old {
                return;
            }
        }
        let current = cache.current.clone();
        let ui_is_visible = cache.ui_is_visible;
        drop(cache);

        if ui_is_visible && !current.path.as_os_str().is_empty() {
            load_preview(current);
        }
    }
}

fn set_design_mode(enable: bool) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    cache.design_mode = enable;

    configure_design_mode(enable);
}

fn change_style() {
    let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let ui_is_visible = cache.ui_is_visible;
    let current = cache.current.clone();
    drop(cache);

    if ui_is_visible && !current.path.as_os_str().is_empty() {
        load_preview(current);
    }
}

pub fn start_parsing() {
    set_status_text("Updating Preview...");
    set_diagnostics(&[]);
    send_status("Loading Preview…", Health::Ok);
}

pub fn finish_parsing(ok: bool) {
    set_status_text("");
    if ok {
        send_status("Preview Loaded", Health::Ok);
    } else {
        send_status("Preview not updated", Health::Error);
    }
}

pub fn config_changed(
    show_preview_ui: bool,
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
            || cache.show_preview_ui != show_preview_ui
        {
            cache.current.include_paths = include_paths.to_vec();
            cache.current.library_paths = library_paths.clone();
            cache.default_style = style;
            cache.show_preview_ui = show_preview_ui;
            let current = cache.current.clone();
            let ui_is_visible = cache.ui_is_visible;
            let show_preview_ui = cache.show_preview_ui;

            drop(cache);

            if ui_is_visible {
                set_show_preview_ui(show_preview_ui);
                if !current.path.as_os_str().is_empty() {
                    load_preview(current);
                }
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

pub fn load_preview(preview_component: PreviewComponent) {
    {
        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.current = preview_component.clone();
        if !cache.ui_is_visible {
            return;
        }
        match cache.loading_state {
            PreviewFutureState::Pending => (),
            PreviewFutureState::PreLoading => return,
            PreviewFutureState::Loading => {
                cache.loading_state = PreviewFutureState::NeedsReload;
                return;
            }
            PreviewFutureState::NeedsReload => return,
        }
        cache.loading_state = PreviewFutureState::PreLoading;
    };

    run_in_ui_thread(move || async move {
        loop {
            let (design_mode, preview_component) = {
                let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
                assert_eq!(cache.loading_state, PreviewFutureState::PreLoading);
                if !cache.ui_is_visible {
                    cache.loading_state = PreviewFutureState::Pending;
                    return;
                }
                cache.loading_state = PreviewFutureState::Loading;
                cache.dependency.clear();
                let preview_component = cache.current.clone();
                cache.current.style.clear();
                (cache.design_mode, preview_component)
            };
            let style = if preview_component.style.is_empty() {
                get_current_style()
            } else {
                set_current_style(preview_component.style.clone());
                preview_component.style.clone()
            };

            reload_preview_impl(preview_component, style, design_mode).await;

            let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
            match cache.loading_state {
                PreviewFutureState::Loading => {
                    cache.loading_state = PreviewFutureState::Pending;
                    return;
                }
                PreviewFutureState::Pending => unreachable!(),
                PreviewFutureState::PreLoading => unreachable!(),
                PreviewFutureState::NeedsReload => {
                    cache.loading_state = PreviewFutureState::PreLoading;
                    continue;
                }
            };
        }
    });
}

// Most be inside the thread running the slint event loop
async fn reload_preview_impl(
    preview_component: PreviewComponent,
    style: String,
    design_mode: bool,
) {
    let component = PreviewComponent { style: String::new(), ..preview_component };

    start_parsing();

    let mut builder = slint_interpreter::ComponentCompiler::default();

    #[cfg(target_arch = "wasm32")]
    {
        let cc = builder.compiler_configuration(i_slint_core::InternalToken);
        cc.resource_url_mapper = resource_url_mapper();
    }

    if !style.is_empty() {
        builder.set_style(style.clone());
    }
    builder.set_include_paths(component.include_paths);
    builder.set_library_paths(component.library_paths);

    builder.set_file_loader(|path| {
        let path = path.to_owned();
        Box::pin(async move { get_file_from_cache(path).map(Result::Ok) })
    });

    let compiled = if let Some(mut from_cache) = get_file_from_cache(component.path.clone()) {
        if let Some(component_name) = &component.component {
            from_cache = format!(
                "{from_cache}\nexport component _Preview inherits {component_name} {{ }}\n"
            );
        }
        builder.build_from_source(from_cache, component.path).await
    } else {
        builder.build_from_path(component.path).await
    };

    notify_diagnostics(builder.diagnostics());

    if let Some(compiled) = compiled {
        update_preview_area(compiled, design_mode);
        finish_parsing(true);
    } else {
        finish_parsing(false);
    };
}

fn configure_handle_for_design_mode(handle: &ComponentInstance, enabled: bool) {
    handle.set_design_mode(enabled);

    handle.on_element_selected(Box::new(
        move |file: &str, start_line: u32, start_column: u32, end_line: u32, end_column: u32| {
            ask_editor_to_show_document(
                file.to_string(),
                start_line,
                start_column,
                end_line,
                end_column,
            );
            // ignore errors
        },
    ));
}
/// This sets up the preview area to show the ComponentInstance
///
/// This must be run in the UI thread.
pub fn set_preview_factory(
    ui: &ui::PreviewUi,
    compiled: ComponentDefinition,
    callback: Box<dyn Fn(ComponentInstance)>,
    design_mode: bool,
) {
    let factory = slint::ComponentFactory::new(move |ctx: FactoryContext| {
        let instance = compiled.create_embedded(ctx).unwrap();
        configure_handle_for_design_mode(&instance, design_mode);

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
        if d.source_file().map_or(true, |f| !i_slint_compiler::pathutils::is_absolute(f)) {
            continue;
        }
        let uri = lsp_types::Url::from_file_path(d.source_file().unwrap())
            .ok()
            .unwrap_or_else(|| lsp_types::Url::parse("file:/unknown").unwrap());
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
