// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use crate::common::{
    ComponentInformation, LspToPreviewMessage, PreviewComponent, PreviewConfig, VersionedUrl,
};
use crate::lsp_ext::Health;
use i_slint_compiler::object_tree::{ElementRc, ElementWeak};
use i_slint_core::model::VecModel;
use i_slint_core::{component_factory::FactoryContext, lengths::LogicalRect};
use lsp_types::{notification::Notification, Url};
use slint_interpreter::highlight::{ComponentKind, ComponentPositions};
use slint_interpreter::{ComponentDefinition, ComponentHandle, ComponentInstance};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

mod debug;
mod element_selection;
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
    source_code: HashMap<Url, String>,
    dependency: HashSet<Url>,
    current: Option<PreviewComponent>,
    config: PreviewConfig,
    loading_state: PreviewFutureState,
    highlight: Option<(Url, u32)>,
    ui_is_visible: bool,
}

static CONTENT_CACHE: std::sync::OnceLock<Mutex<ContentCache>> = std::sync::OnceLock::new();

#[derive(Default)]
struct PreviewState {
    ui: Option<ui::PreviewUi>,
    handle: Rc<RefCell<Option<slint_interpreter::ComponentInstance>>>,
    selected_element: Option<ElementWeak>,
}
thread_local! {static PREVIEW_STATE: std::cell::RefCell<PreviewState> = Default::default();}

pub fn set_contents(url: &VersionedUrl, content: String) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let old = cache.source_code.insert(url.url.clone(), content.clone());
    if cache.dependency.contains(&url.url) {
        if let Some(old) = old {
            if content == old {
                return;
            }
        }
        let Some(current) = cache.current.clone() else {
            return;
        };
        let ui_is_visible = cache.ui_is_visible;

        drop(cache);

        if ui_is_visible {
            load_preview(current);
        }
    }
}

// triggered from the UI, running in UI thread
fn can_drop_component(component_name: slint::SharedString, x: f32, y: f32) -> bool {
    i_slint_core::debug_log!("can drop? {} at {x}x{y}", component_name.as_str());
    ((x.round() as i32) / 10) % 2 == 0 && ((y.round() as i32) / 10) % 2 == 0
}

// triggered from the UI, running in UI thread
fn drop_component(component_name: slint::SharedString, x: f32, y: f32) {
    i_slint_core::debug_log!("drop! {} at {x}x{y}", component_name.as_str());
}

fn change_style() {
    let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let ui_is_visible = cache.ui_is_visible;
    let Some(current) = cache.current.clone() else {
        return;
    };
    drop(cache);

    if ui_is_visible {
        load_preview(current);
    }
}

fn start_parsing() {
    set_status_text("Updating Preview...");
    set_diagnostics(&[]);
    send_status("Loading Preview…", Health::Ok);
}

fn finish_parsing(ok: bool) {
    set_status_text("");
    if ok {
        send_status("Preview Loaded", Health::Ok);
    } else {
        send_status("Preview not updated", Health::Error);
    }
}

pub fn config_changed(config: PreviewConfig) {
    if let Some(cache) = CONTENT_CACHE.get() {
        let mut cache = cache.lock().unwrap();
        if cache.config != config {
            cache.config = config;
            let current = cache.current.clone();
            let ui_is_visible = cache.ui_is_visible;
            let hide_ui = cache.config.hide_ui;

            drop(cache);

            if ui_is_visible {
                if let Some(hide_ui) = hide_ui {
                    set_show_preview_ui(!hide_ui);
                }
                if let Some(current) = current {
                    load_preview(current);
                }
            }
        }
    };
}

/// If the file is in the cache, returns it.
/// In any way, register it as a dependency
fn get_url_from_cache(url: &Url) -> Option<String> {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let r = cache.source_code.get(url).cloned();
    cache.dependency.insert(url.to_owned());
    r
}

fn get_path_from_cache(path: &Path) -> Option<String> {
    let url = Url::from_file_path(path).ok()?;
    get_url_from_cache(&url)
}

pub fn load_preview(preview_component: PreviewComponent) {
    {
        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.current = Some(preview_component.clone());
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
            let (preview_component, config) = {
                let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
                let Some(current) = &mut cache.current else { return };
                let preview_component = current.clone();
                current.style.clear();

                assert_eq!(cache.loading_state, PreviewFutureState::PreLoading);
                if !cache.ui_is_visible {
                    cache.loading_state = PreviewFutureState::Pending;
                    return;
                }
                cache.loading_state = PreviewFutureState::Loading;
                cache.dependency.clear();
                (preview_component, cache.config.clone())
            };
            let style = if preview_component.style.is_empty() {
                get_current_style()
            } else {
                set_current_style(preview_component.style.clone());
                preview_component.style.clone()
            };

            reload_preview_impl(preview_component, style, config).await;

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
    config: PreviewConfig,
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
    builder.set_include_paths(config.include_paths);
    builder.set_library_paths(config.library_paths);

    builder.set_file_loader(|path| {
        let path = path.to_owned();
        Box::pin(async move { get_path_from_cache(&path).map(Result::Ok) })
    });

    // to_file_path on a WASM Url just returns the URL as the path!
    let path = component.url.to_file_path().unwrap_or(PathBuf::from(&component.url.to_string()));

    let compiled = if let Some(mut from_cache) = get_url_from_cache(&component.url) {
        if let Some(component_name) = &component.component {
            from_cache = format!(
                "{from_cache}\nexport component _Preview inherits {component_name} {{ }}\n"
            );
        }
        builder.build_from_source(from_cache, path).await
    } else {
        builder.build_from_path(path).await
    };

    notify_diagnostics(builder.diagnostics());

    let success = compiled.is_some();
    update_preview_area(compiled);
    finish_parsing(success);
}

/// This sets up the preview area to show the ComponentInstance
///
/// This must be run in the UI thread.
fn set_preview_factory(
    ui: &ui::PreviewUi,
    compiled: ComponentDefinition,
    callback: Box<dyn Fn(ComponentInstance)>,
) {
    // Ensure that the popup is closed as it is related to the old factory
    i_slint_core::window::WindowInner::from_pub(ui.window()).close_popup();

    let factory = slint::ComponentFactory::new(move |ctx: FactoryContext| {
        let instance = compiled.create_embedded(ctx).unwrap();

        if let Some((url, offset)) =
            CONTENT_CACHE.get().and_then(|c| c.lock().unwrap().highlight.clone())
        {
            highlight(Some(url), offset);
        } else {
            highlight(None, 0);
        }

        callback(instance.clone_strong());

        Some(instance)
    });
    ui.set_preview_area(factory);
}

/// Highlight the element pointed at the offset in the path.
/// When path is None, remove the highlight.
pub fn highlight(url: Option<Url>, offset: u32) {
    let highlight = url.clone().map(|x| (x, offset));
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();

    if cache.highlight == highlight {
        return;
    }
    cache.highlight = highlight;

    if cache.highlight.as_ref().map_or(true, |(url, _)| cache.dependency.contains(url)) {
        run_in_ui_thread(move || async move {
            let handle = PREVIEW_STATE.with(|preview_state| {
                let preview_state = preview_state.borrow();
                let result = preview_state.handle.borrow().as_ref().map(|h| h.clone_strong());
                result
            });

            if let Some(handle) = handle {
                let element_positions =
                    if let Some(path) = url.as_ref().and_then(|u| u.to_file_path().ok()) {
                        handle.component_positions(path, offset)
                    } else {
                        ComponentPositions::default()
                    };

                set_selected_element(None, element_positions);
            }
        })
    }
}

/// Highlight the element pointed at the offset in the path.
/// When path is None, remove the highlight.
pub fn known_components(_url: &Option<VersionedUrl>, components: Vec<ComponentInformation>) {
    run_in_ui_thread(move || async move {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow();
            if let Some(ui) = &preview_state.ui {
                ui::ui_set_known_components(ui, &components)
            }
        })
    });
}

pub fn show_document_request_from_element_callback(
    file: &str,
    range: lsp_types::Range,
) -> Option<lsp_types::ShowDocumentParams> {
    use lsp_types::ShowDocumentParams;

    if file.is_empty() || range.start.character == 0 || range.end.character == 0 {
        return None;
    }

    Url::from_file_path(file).ok().map(|uri| ShowDocumentParams {
        uri,
        external: Some(false),
        take_focus: Some(true),
        selection: Some(range),
    })
}

fn convert_diagnostics(
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

fn reset_selections(ui: &ui::PreviewUi) {
    let model = Rc::new(slint::VecModel::from(Vec::new()));
    ui.set_selections(slint::ModelRc::from(model));
}

fn set_selections(
    ui: Option<&ui::PreviewUi>,
    element_position: Option<(&ElementRc, LogicalRect)>,
    positions: ComponentPositions,
) {
    let Some(ui) = ui else {
        return;
    };

    let values = {
        let mut tmp = Vec::with_capacity(
            positions.geometries.len() + if element_position.is_some() { 1 } else { 0 },
        );

        if let Some((e, primary_position)) = element_position.as_ref() {
            let border_color = if e.borrow().layout.is_some() {
                i_slint_core::Color::from_argb_encoded(0xffff0000)
            } else {
                i_slint_core::Color::from_argb_encoded(0xff0000ff)
            };

            tmp.push(ui::Selection {
                width: primary_position.size.width,
                height: primary_position.size.height,
                x: primary_position.origin.x,
                y: primary_position.origin.y,
                border_color,
            });
        }
        let secondary_border_color = match positions.kind {
            Some(ComponentKind::Layout) => i_slint_core::Color::from_argb_encoded(0x80ff0000),
            _ => i_slint_core::Color::from_argb_encoded(0x800000ff),
        };

        tmp.extend(positions.geometries.iter().map(|geometry| ui::Selection {
            width: geometry.size.width,
            height: geometry.size.height,
            x: geometry.origin.x,
            y: geometry.origin.y,
            border_color: secondary_border_color,
        }));
        tmp
    };
    let model = Rc::new(slint::VecModel::from(values));
    ui.set_selections(slint::ModelRc::from(model));
}

fn set_selected_element(
    element_position: Option<(&ElementRc, LogicalRect)>,
    secondary: slint_interpreter::highlight::ComponentPositions,
) {
    PREVIEW_STATE.with(move |preview_state| {
        let mut preview_state = preview_state.borrow_mut();
        if let Some((e, _)) = element_position {
            preview_state.selected_element = Some(Rc::downgrade(e));
        } else {
            preview_state.selected_element = None;
        }

        set_selections(preview_state.ui.as_ref(), element_position, secondary);
    })
}

fn selected_element() -> Option<ElementRc> {
    PREVIEW_STATE.with(move |preview_state| {
        let preview_state = preview_state.borrow();
        let Some(weak) = &preview_state.selected_element else {
            return None;
        };
        weak.upgrade()
    })
}

fn component_instance() -> Option<ComponentInstance> {
    PREVIEW_STATE.with(move |preview_state| {
        preview_state.borrow().handle.borrow().as_ref().map(|ci| ci.clone_strong())
    })
}

fn set_show_preview_ui(show_preview_ui: bool) {
    run_in_ui_thread(move || async move {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow();
            if let Some(ui) = &preview_state.ui {
                ui.set_show_preview_ui(show_preview_ui)
            }
        })
    });
}

fn set_current_style(style: String) {
    PREVIEW_STATE.with(move |preview_state| {
        let preview_state = preview_state.borrow_mut();
        if let Some(ui) = &preview_state.ui {
            ui.set_current_style(style.into())
        }
    });
}

fn get_current_style() -> String {
    PREVIEW_STATE.with(|preview_state| -> String {
        let preview_state = preview_state.borrow();
        if let Some(ui) = &preview_state.ui {
            ui.get_current_style().as_str().to_string()
        } else {
            String::new()
        }
    })
}

fn set_status_text(text: &str) {
    let text = text.to_string();

    i_slint_core::api::invoke_from_event_loop(move || {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow_mut();
            if let Some(ui) = &preview_state.ui {
                ui.set_status_text(text.into());
            }
        });
    })
    .unwrap();
}

fn set_diagnostics(diagnostics: &[slint_interpreter::Diagnostic]) {
    let data = crate::preview::ui::convert_diagnostics(diagnostics);

    i_slint_core::api::invoke_from_event_loop(move || {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow_mut();
            if let Some(ui) = &preview_state.ui {
                let model = VecModel::from(data);
                ui.set_diagnostics(Rc::new(model).into());
            }
        });
    })
    .unwrap();
}

/// This runs `set_preview_factory` in the UI thread
fn update_preview_area(compiled: Option<ComponentDefinition>) {
    PREVIEW_STATE.with(|preview_state| {
        #[allow(unused_mut)]
        let mut preview_state = preview_state.borrow_mut();

        #[cfg(not(target_arch = "wasm32"))]
        native::open_ui_impl(&mut preview_state);

        let ui = preview_state.ui.as_ref().unwrap();
        let shared_handle = preview_state.handle.clone();

        if let Some(compiled) = compiled {
            set_preview_factory(
                ui,
                compiled,
                Box::new(move |instance| {
                    shared_handle.replace(Some(instance));
                }),
            );
            reset_selections(ui);
        }

        ui.show().unwrap();
    });
}

pub fn lsp_to_preview_message(
    message: LspToPreviewMessage,
    #[cfg(not(target_arch = "wasm32"))] sender: &crate::ServerNotifier,
) {
    match message {
        LspToPreviewMessage::SetContents { url, contents } => {
            set_contents(&url, contents);
        }
        LspToPreviewMessage::SetConfiguration { config } => {
            config_changed(config);
        }
        LspToPreviewMessage::ShowPreview(pc) => {
            #[cfg(not(target_arch = "wasm32"))]
            native::open_ui(sender);
            load_preview(pc);
        }
        LspToPreviewMessage::HighlightFromEditor { url, offset } => {
            highlight(url, offset);
        }
        LspToPreviewMessage::KnownComponents { url, components } => {
            known_components(&url, components);
        }
    }
}
