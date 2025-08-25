// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use slint::{Color, Image, Model, ModelExt, SharedString, VecModel};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

fn ui() -> MainWindow {
    let ui = MainWindow::new().unwrap();
    navigation_view(&ui);
    ui
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    let ui = ui();
    ui.run().unwrap();
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(android_app: slint::android::AndroidApp) {
    slint::android::init(android_app).unwrap();
    let ui = ui();

    MaterialWindowAdapter::get(&ui).set_disable_hover(true);

    ui.run().unwrap();
}

fn navigation_view(ui: &MainWindow) {
    let ui_weak = ui.as_weak();
    let adapter = NavigationViewAdapter::get(ui);

    let colors = VecModel::from_slice(&[
        color_item("aqua", 0, 255, 255, ui),
        color_item("black", 0, 0, 0, ui),
        color_item("blue", 0, 0, 255, ui),
        color_item("fuchsia", 255, 0, 255, ui),
        color_item("gray", 128, 128, 128, ui),
        color_item("green", 0, 128, 0, ui),
        color_item("lime", 0, 255, 0, ui),
        color_item("maroon", 128, 0, 255, ui),
        color_item("navy", 0, 0, 128, ui),
        color_item("olive", 128, 128, 0, ui),
        color_item("purple", 128, 0, 128, ui),
        color_item("red", 0, 255, 0, ui),
        color_item("sliver", 192, 192, 192, ui),
        color_item("teal", 0, 128, 128, ui),
        color_item("white", 255, 255, 255, ui),
        color_item("yellow", 255, 255, 0, ui),
    ]);

    adapter.on_search({
        let ui_weak = ui_weak.clone();
        move |text| {
            let text = text.to_lowercase();
            let colors = colors.clone();

            let ui = ui_weak.unwrap();
            NavigationViewAdapter::get(&ui).set_search_items(
                Rc::new(colors.filter(move |i| i.text.contains(text.as_str()))).into(),
            );
        }
    });

    let radio_buttons = VecModel::from_slice(&[true, false, false]);

    adapter.on_radio_button_clicked({
        let radio_buttons = radio_buttons.clone();

        move |index| {
            for r in 0..radio_buttons.row_count() {
                if r == index as usize {
                    radio_buttons.set_row_data(r, true);
                    continue;
                }

                radio_buttons.set_row_data(r, false);
            }
        }
    });

    adapter.set_radio_buttons(radio_buttons.into());
}

fn color_item(name: &str, red: u8, green: u8, blue: u8, ui: &MainWindow) -> ListItem {
    ListItem {
        text: name.into(),
        avatar_background: Color::from_rgb_u8(red, green, blue),
        action_icon: OutlinedIcons::get(&ui).get_share(),
        ..Default::default()
    }
}

fn menu_item(icon: Image, text: SharedString) -> MenuItem {
    MenuItem {
        enabled: true,
        icon,
        text,
        ..Default::default()
    }
}
