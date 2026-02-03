// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![deny(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn load_font_from_bytes(font_data: js_sys::Uint8Array, locale: &str) -> Result<(), JsValue> {
    use slint::fontique_07::fontique;

    let font_data = font_data.to_vec();
    let blob = fontique::Blob::new(std::sync::Arc::new(font_data));
    let mut collection = slint::fontique_07::shared_collection();
    let fonts = collection.register_fonts(blob, None);

    scripts_for_locale(locale, |script| {
        collection
            .append_fallbacks(fontique::FallbackKey::new(*script, None), fonts.iter().map(|x| x.0));
    });

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn scripts_for_locale(
    locale: &str,
    mut callback: impl FnMut(&slint::fontique_07::fontique::Script),
) {
    use slint::fontique_07::fontique;

    let Ok(locale) = icu_locale_core::Locale::try_from_str(locale) else {
        return;
    };

    let scripts: &[fontique::Script] = match locale.id.language.as_str() {
        "ja" => &[
            fontique::Script::from("Hira"),
            fontique::Script::from("Kana"),
            fontique::Script::from("Hani"),
        ],
        "ko" => &[fontique::Script::from("Hang"), fontique::Script::from("Hani")],
        "zh" => &[fontique::Script::from("Hani")],
        _ => {
            if let Some(script) = locale.id.script {
                &[fontique::Script::from(script.into_raw())]
            } else {
                &[]
            }
        }
    };

    for script in scripts {
        callback(script);
    }
}

use std::rc::Rc;

use slint::{Model, ModelExt, ModelRc, SharedString, StandardListViewItem, VecModel};

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    // For native builds, initialize gettext translations
    #[cfg(not(target_arch = "wasm32"))]
    slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/"));

    let app = App::new().unwrap();

    let row_data: Rc<VecModel<slint::ModelRc<StandardListViewItem>>> = Rc::new(VecModel::default());

    for r in 1..101 {
        let items = Rc::new(VecModel::default());

        for c in 1..5 {
            items.push(slint::format!("Item {r}.{c}").into());
        }

        row_data.push(items.into());
    }

    app.global::<TableViewPageAdapter>().set_row_data(row_data.clone().into());
    app.global::<TableViewPageAdapter>().on_filter_sort_model(filter_sort_model);

    app.run().unwrap();
}

fn filter_sort_model(
    source_model: ModelRc<ModelRc<StandardListViewItem>>,
    filter: SharedString,
    sort_index: i32,
    sort_ascending: bool,
) -> ModelRc<ModelRc<StandardListViewItem>> {
    let mut model = source_model.clone();

    if !filter.is_empty() {
        let filter = filter.to_lowercase();

        // filter by first row
        model =
            Rc::new(source_model.clone().filter(move |e| {
                e.row_data(0).unwrap().text.to_lowercase().contains(filter.as_str())
            }))
            .into();
    }

    if sort_index >= 0 {
        model = Rc::new(model.clone().sort_by(move |r_a, r_b| {
            let c_a = r_a.row_data(sort_index as usize).unwrap();
            let c_b = r_b.row_data(sort_index as usize).unwrap();

            if sort_ascending { c_a.text.cmp(&c_b.text) } else { c_b.text.cmp(&c_a.text) }
        }))
        .into();
    }

    model
}
