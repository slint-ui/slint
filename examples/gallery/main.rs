// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![deny(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

use std::rc::Rc;

use slint::{Model, StandardListViewItem, VecModel};

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

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

    app.global::<TableViewPageAdapter>().on_sort_ascending({
        let app_weak = app.as_weak();
        let row_data = row_data.clone();
        move |index| {
            let row_data = row_data.clone();

            let sort_model = Rc::new(row_data.sort_by(move |r_a, r_b| {
                let c_a = r_a.row_data(index as usize).unwrap();
                let c_b = r_b.row_data(index as usize).unwrap();

                c_a.text.cmp(&c_b.text)
            }));

            app_weak.unwrap().global::<TableViewPageAdapter>().set_row_data(sort_model.into());
        }
    });

    app.global::<TableViewPageAdapter>().on_sort_descending({
        let app_weak = app.as_weak();
        move |index| {
            let row_data = row_data.clone();

            let sort_model = Rc::new(row_data.sort_by(move |r_a, r_b| {
                let c_a = r_a.row_data(index as usize).unwrap();
                let c_b = r_b.row_data(index as usize).unwrap();

                c_b.text.cmp(&c_a.text)
            }));

            app_weak.unwrap().global::<TableViewPageAdapter>().set_row_data(sort_model.into());
        }
    });

    app.run().unwrap();
}
