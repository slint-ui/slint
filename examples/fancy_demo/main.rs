// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![deny(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

use slint::{Model, VecModel};
use std::rc::Rc;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let app = Demo::new();

    let model = Rc::new(VecModel::default());

    let drag_item_width = app.global::<Sizes>().get_drag_item_width();
    let drag_item_height = app.global::<Sizes>().get_drag_item_height();

    for i in 0..10 {
        model.push(DragItem {
            x: 0.,
            y: i as f32 * drag_item_height,
            width: drag_item_width,
            height: drag_item_height,
            text: format!("Node {i}").into(),
        });
    }

    app.set_drag_items(model.into());

    app.on_check_drag({
        let app = app.as_weak();
        move |x, y| {
            app.upgrade_in_event_loop(move |app| {
                for drag_item in app.get_drag_items().iter() {
                    if x >= drag_item.x
                        && x <= drag_item.x + drag_item.width
                        && y >= drag_item.y
                        && y <= drag_item.y + drag_item.height
                    {
                        println!("yes");
                        app.invoke_start_drag(x, y, drag_item.text);
                    }
                }
            })
            .expect("Cannot check drag.");
        }
    });

    // app.global::<NodeController>().set_model(model.clone().into());

    // app.global::<NodeController>().on_push({
    //     let app = app.as_weak();

    //     move |x, y, title| {
    //         app.upgrade_in_event_loop(move |app| {
    //             if let Some(model) = app.global::<NodeController>().get_model().as_any()
    //             .downcast_ref::<VecModel<NodeModel>>() {
    //                 model.push(NodeModel {
    //                     title,
    //                     x,
    //                     y
    //                 });
    //             }
    //             // model.push()
    //         }).expect("Cannot add node.");
    //     }
    // });

    app.run();
}
