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

    let drag_items = Rc::new(VecModel::default());

    let drag_item_width = app.global::<Sizes>().get_drag_item_width();
    let drag_item_height = app.global::<Sizes>().get_drag_item_height();

    for i in 0..10 {
        drag_items.push(DragItem {
            x: 0.,
            y: i as f32 * drag_item_height,
            width: drag_item_width,
            height: drag_item_height,
            text: format!("Node {i}").into(),
        });
    }

    app.set_drag_items(drag_items.into());
    app.set_node_items(Rc::new(VecModel::default()).into());

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
                        app.set_current_drag_item(drag_item);
                        app.invoke_start_drag(x, y);
                    }
                }
            })
            .expect("Cannot check drag.");
        }
    });

    app.on_drop({
        let app = app.as_weak();
        move |x, y| {
            app.upgrade_in_event_loop(move |app| {
                let drop_zone_x = app.get_drag_zone_x();
                let drop_zone_y = app.get_drag_zone_y();
                let drag_text = app.get_current_drag_item().text.clone();

                if let Some(model) =
                    app.get_node_items().as_any().downcast_ref::<VecModel<NodeItem>>()
                {
                    println!("x: {x}");
                    model.push(NodeItem {
                        text: drag_text,
                        x: x - drop_zone_x,
                        y: y - drop_zone_y,
                    });
                }
            })
            .expect("Cannot update.");
        }
    });

    app.run();
}
