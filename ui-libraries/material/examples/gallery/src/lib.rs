// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use slint::{Color, Model, ModelExt, VecModel};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

fn ui() -> MainWindow {
    let ui = MainWindow::new().unwrap();
    navigation_view(&ui);
    date_picker::init(&ui);
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

mod date_picker {
    use super::{DatePickerAdapter, MainWindow};
    use chrono::Local;
    use chrono::{Datelike, NaiveDate};
    use slint::{Global, SharedString, VecModel};

    // initializes the DatePickerAdapter
    pub fn init(ui: &MainWindow) {
        let adapter = DatePickerAdapter::get(ui);

        adapter.on_month_day_count(|month, year| {
            month_day_count(month as u32, year).unwrap_or_default() as i32
        });
        adapter.on_month_offset(|month, year| month_offset(month as u32, year) as i32);
        adapter.on_format_date(|format, day, month, year| {
            format_date(format.as_str(), day as u32, month as u32, year)
        });
        adapter.on_parse_date(|date, format| {
            VecModel::from_slice(&parse_date(date.as_str(), format.as_str()).unwrap_or([0, 0, 0]))
        });
        adapter.on_valid_date(|date, format| valid_date(date.as_str(), format.as_str()));
        adapter.on_date_now(|| VecModel::from_slice(&date_now()));
    }

    // returns the number of days for the given month in the given year.
    fn month_day_count(month: u32, year: i32) -> Option<i32> {
        Some(
            NaiveDate::from_ymd_opt(
                match month {
                    12 => year + 1,
                    _ => year,
                },
                match month {
                    12 => 1,
                    _ => month + 1,
                },
                1,
            )?
            .signed_duration_since(NaiveDate::from_ymd_opt(year, month, 1)?)
            .num_days() as i32,
        )
    }

    // return the numbers of day to the first monday of the month.
    fn month_offset(month: u32, year: i32) -> i32 {
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, 1) {
            let offset = date.weekday().number_from_monday() as i32;

            // sunday
            if offset >= 7 {
                return 0;
            }

            return offset;
        }

        // The result is only None if month == 0, it should not happen because the function is only
        // used internal and not directly by the user. So it is ok to return 0 on a None result
        0
    }

    // used to format a date that is defined by day month and year.
    fn format_date(format: &str, day: u32, month: u32, year: i32) -> SharedString {
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
            return slint::format!("{}", date.format(format));
        }

        // Don't panic, this function is used only internal
        SharedString::default()
    }

    // parses the given date string and returns a list of day, month and year.
    fn parse_date(date: &str, format: &str) -> Option<[i32; 3]> {
        NaiveDate::parse_from_str(date, format)
            .ok()
            .map(|date| [date.day() as i32, date.month() as i32, date.year()])
    }

    // returns true if the given date is valid.
    fn valid_date(date: &str, format: &str) -> bool {
        return parse_date(date, format).is_some();
    }

    // returns the current date as list of day, month and year.
    fn date_now() -> [i32; 3] {
        let now = Local::now().date_naive();
        [now.day() as i32, now.month() as i32, now.year()]
    }
}
