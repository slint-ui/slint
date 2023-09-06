// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Mutex,
};

use crate::{common::PreviewComponent, lsp_ext::Health};

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

static CONTENT_CACHE: once_cell::sync::OnceCell<Mutex<ContentCache>> =
    once_cell::sync::OnceCell::new();

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
