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
    theme::init(&ui);
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
        action_button_icon: OutlinedIcons::get(&ui).get_share(),
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

#[allow(non_snake_case)]
mod theme {
    use crate::{MainViewAdapter, MainWindow, MaterialPalette, MenuItem};
    use serde::{Deserialize, Serialize};
    use slint::{Color, ComponentHandle, Global, VecModel};

    const SLINT_THEME: &str = include_str!("../ui/themes/material_slint_theme.json");
    const PURPLE_THEME: &str = include_str!("../ui/themes/material_purple_theme.json");
    const RED_THEME: &str = include_str!("../ui/themes/material_red_theme.json");
    const GREEN_THEME: &str = include_str!("../ui/themes/material_green_theme.json");

    #[derive(Serialize, Deserialize, Debug)]
    struct MaterialScheme {
        pub primary: String,
        pub surfaceTint: String,
        pub onPrimary: String,
        pub primaryContainer: String,
        pub onPrimaryContainer: String,
        pub secondary: String,
        pub onSecondary: String,
        pub secondaryContainer: String,
        pub onSecondaryContainer: String,
        pub tertiary: String,
        pub onTertiary: String,
        pub tertiaryContainer: String,
        pub onTertiaryContainer: String,
        pub error: String,
        pub onError: String,
        pub errorContainer: String,
        pub onErrorContainer: String,
        pub background: String,
        pub onBackground: String,
        pub surface: String,
        pub onSurface: String,
        pub surfaceVariant: String,
        pub onSurfaceVariant: String,
        pub outline: String,
        pub outlineVariant: String,
        pub shadow: String,
        pub scrim: String,
        pub inverseSurface: String,
        pub inverseOnSurface: String,
        pub inversePrimary: String,
        pub primaryFixed: String,
        pub onPrimaryFixed: String,
        pub primaryFixedDim: String,
        pub onPrimaryFixedVariant: String,
        pub secondaryFixed: String,
        pub onSecondaryFixed: String,
        pub secondaryFixedDim: String,
        pub onSecondaryFixedVariant: String,
        pub tertiaryFixed: String,
        pub onTertiaryFixed: String,
        pub tertiaryFixedDim: String,
        pub onTertiaryFixedVariant: String,
        pub surfaceDim: String,
        pub surfaceBright: String,
        pub surfaceContainerLowest: String,
        pub surfaceContainerLow: String,
        pub surfaceContainer: String,
        pub surfaceContainerHigh: String,
        pub surfaceContainerHighest: String,
    }

    impl Into<crate::MaterialScheme> for MaterialScheme {
        fn into(self) -> crate::MaterialScheme {
            crate::MaterialScheme {
                background: string_to_color(self.background),
                error: string_to_color(self.error),
                errorContainer: string_to_color(self.errorContainer),
                inverseOnSurface: string_to_color(self.inverseOnSurface),
                inversePrimary: string_to_color(self.inversePrimary),
                inverseSurface: string_to_color(self.inverseSurface),
                onBackground: string_to_color(self.onBackground),
                onError: string_to_color(self.onError),
                onErrorContainer: string_to_color(self.onErrorContainer),
                onPrimary: string_to_color(self.onPrimary),
                onPrimaryContainer: string_to_color(self.onPrimaryContainer),
                onPrimaryFixed: string_to_color(self.onPrimaryFixed),
                onPrimaryFixedVariant: string_to_color(self.onPrimaryFixedVariant),
                onSecondary: string_to_color(self.onSecondary),
                onSecondaryContainer: string_to_color(self.onSecondaryContainer),
                onSecondaryFixed: string_to_color(self.onSecondaryFixed),
                onSecondaryFixedVariant: string_to_color(self.onSecondaryFixedVariant),
                onSurface: string_to_color(self.onSurface),
                onSurfaceVariant: string_to_color(self.onSurfaceVariant),
                onTertiary: string_to_color(self.onTertiary),
                onTertiaryContainer: string_to_color(self.onTertiaryContainer),
                onTertiaryFixed: string_to_color(self.onTertiaryFixed),
                onTertiaryFixedVariant: string_to_color(self.onTertiaryFixedVariant),
                outline: string_to_color(self.outline),
                outlineVariant: string_to_color(self.outlineVariant),
                primary: string_to_color(self.primary),
                primaryContainer: string_to_color(self.primaryContainer),
                primaryFixed: string_to_color(self.primaryFixed),
                primaryFixedDim: string_to_color(self.primaryFixedDim),
                scrim: string_to_color(self.scrim),
                secondary: string_to_color(self.secondary),
                secondaryContainer: string_to_color(self.secondaryContainer),
                secondaryFixed: string_to_color(self.secondaryFixed),
                secondaryFixedDim: string_to_color(self.secondaryFixedDim),
                shadow: string_to_color(self.shadow),
                surface: string_to_color(self.surface),
                surfaceBright: string_to_color(self.surfaceBright),
                surfaceContainer: string_to_color(self.surfaceContainer),
                surfaceContainerHigh: string_to_color(self.surfaceContainerHigh),
                surfaceContainerHighest: string_to_color(self.surfaceContainerHighest),
                surfaceContainerLow: string_to_color(self.surfaceContainerLow),
                surfaceContainerLowest: string_to_color(self.surfaceContainerLowest),
                surfaceDim: string_to_color(self.surfaceDim),
                surfaceTint: string_to_color(self.surfaceTint),
                surfaceVariant: string_to_color(self.surfaceVariant),
                tertiary: string_to_color(self.tertiary),
                tertiaryContainer: string_to_color(self.tertiaryContainer),
                tertiaryFixed: string_to_color(self.tertiaryFixed),
                tertiaryFixedDim: string_to_color(self.tertiaryFixedDim),
            }
        }
    }

    fn string_to_color(color: String) -> Color {
        let c = color.parse::<css_color_parser2::Color>().unwrap();
        Color::from_argb_u8((c.a * 255.) as u8, c.r, c.g, c.b)
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct MaterialSchemes {
        pub dark: MaterialScheme,
        pub light: MaterialScheme,
    }

    impl Into<crate::MaterialSchemes> for MaterialSchemes {
        fn into(self) -> crate::MaterialSchemes {
            crate::MaterialSchemes {
                dark: self.dark.into(),
                light: self.light.into(),
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct MaterialTheme {
        pub schemes: MaterialSchemes,
    }

    pub fn init(ui: &MainWindow) {
        let adapter = MainViewAdapter::get(ui);
        adapter.set_palettes(VecModel::from_slice(&[
            MenuItem {
                text: "Slint".into(),
                enabled: true,
                ..Default::default()
            },
            MenuItem {
                text: "Purple".into(),
                enabled: true,
                ..Default::default()
            },
            MenuItem {
                text: "Red".into(),
                enabled: true,
                ..Default::default()
            },
            MenuItem {
                text: "Green".into(),
                enabled: true,
                ..Default::default()
            },
        ]));

        adapter.on_load_palette({
            let ui_weak = ui.as_weak();

            move |index| {
                let ui = ui_weak.unwrap();
                load_theme(index as usize, &ui);
            }
        })
    }

    fn load_theme(index: usize, ui: &MainWindow) {
        let theme: MaterialTheme = serde_json::from_str(match index {
            1 => PURPLE_THEME,
            2 => RED_THEME,
            3 => GREEN_THEME,
            _ => SLINT_THEME,
        })
        .unwrap();

        MaterialPalette::get(ui).set_schemes(theme.schemes.into());
    }
}
