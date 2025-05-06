// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::preview::ui;

fn collect_colors_palette() -> Vec<ui::PaletteEntry> {
    let colors = i_slint_compiler::lookup::named_colors();
    colors
        .iter()
        .map(|(k, v)| {
            let color_code: slint::SharedString = format!("Colors.{k}").into();
            ui::PaletteEntry {
                name: k.to_string().into(),
                value: ui::PropertyValue {
                    accessor_path: color_code.clone(),
                    code: color_code.clone(),
                    display_string: color_code.clone(),
                    kind: ui::PropertyValueKind::Color,
                    value_string: color_code,
                    value_brush: slint::Color::from_argb_encoded(*v).into(),
                    ..Default::default()
                },
            }
        })
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_colors_palette() {
        let colors = collect_colors_palette();
        let input_colors = i_slint_compiler::lookup::named_colors();

        assert_eq!(colors.len(), input_colors.len());
        colors.iter().zip(input_colors).for_each(|(c, (ki, vi))| {
            assert_eq!(c.name, ki);
            let slint::Brush::SolidColor(color_value) = c.value.value_brush else {
                panic!("Not a solid color");
            };
            assert_eq!(color_value, slint::Color::from_argb_encoded(*vi));
        });
    }
}

pub fn collect_palettes() -> Vec<ui::PaletteEntry> {
    collect_colors_palette()
}
