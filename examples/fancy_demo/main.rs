// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![deny(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

use std::rc::Rc;
use slint::{VecModel, Model};

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let app = Demo::new();

    let model = Rc::new(VecModel::default());

    app.global::<NodeController>().set_model(model.clone().into());


    app.global::<NodeController>().on_push({
        let app = app.as_weak();

      

        move |x, y, title| {
            app.upgrade_in_event_loop(move |app| {
                if let Some(model) = app.global::<NodeController>().get_model().as_any()
                .downcast_ref::<VecModel<NodeModel>>() {
                    model.push(NodeModel {
                        title,
                        x,
                        y
                    });
                }
                // model.push()
            }).expect("Cannot add node.");
        }
    });

    app.run();
}
