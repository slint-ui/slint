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

mod ui;
#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[derive(Default)]
struct ContentCache {
    source_code: HashMap<PathBuf, String>,
    dependency: HashSet<PathBuf>,
    current: PreviewComponent,
    highlight: Option<(PathBuf, u32)>,
    ui_is_visible: bool,
    design_mode: bool,
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
    send_status(if enable { "Design mode enabled." } else { "Design mode disabled." }, Health::Ok);
}

pub fn config_changed(style: &str, include_paths: &[PathBuf]) {
    if let Some(cache) = CONTENT_CACHE.get() {
        let mut cache = cache.lock().unwrap();
        let style = style.to_string();
        if cache.current.style != style || cache.current.include_paths != include_paths {
            cache.current.style = style;
            cache.current.include_paths = include_paths.to_vec();
            let current = cache.current.clone();
            let ui_is_visible = cache.ui_is_visible;

            drop(cache);

            let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
            cache.ui_is_visible = ui_is_visible;

            if ui_is_visible {
                load_preview(current);
            }
        }
    };
}

/// If the file is in the cache, returns it.
/// In any was, register it as a dependency
fn get_file_from_cache(path: PathBuf) -> Option<String> {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let r = cache.source_code.get(&path).cloned();
    cache.dependency.insert(path);
    r
}

async fn reload_preview(preview_component: PreviewComponent) {
    let (design_mode, ui_is_visible) = {
        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.dependency.clear();
        cache.current = preview_component.clone();
        (cache.design_mode, cache.ui_is_visible)
    };
    if !ui_is_visible {
        return;
    };

    send_status("Loading Preview…", Health::Ok);

    let mut builder = slint_interpreter::ComponentCompiler::default();

    if !preview_component.style.is_empty() {
        builder.set_style(preview_component.style);
    }
    builder.set_include_paths(preview_component.include_paths);

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
        send_status("Preview Loaded", Health::Ok);
    } else {
        send_status("Preview not updated", Health::Error);
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
        let instance =
            compiled.create_embedded(ctx).unwrap();

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
pub fn highlight(path: Option<PathBuf>, offset: u32) {
    let highlight = path.clone().map(|x| (x, offset));
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();

    if cache.highlight == highlight {
        return;
    }
    cache.highlight = highlight;

    if cache.highlight.as_ref().map_or(true, |(path, _)| cache.dependency.contains(path)) {
        let path = path.unwrap_or_default();
        update_highlight(path, offset);
    }
}
