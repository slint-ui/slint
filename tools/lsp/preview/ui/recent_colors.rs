// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use crate::preview::ui;

use slint::{ComponentHandle, Model};

const MAX_RECENT_COLORS: usize = 9;

pub fn setup(ui: &ui::PreviewUi) {
    let api = ui.global::<ui::Api>();

    api.set_recent_colors(Rc::new(slint::VecModel::default()).into());

    let weak_ui = ui.as_weak();

    api.on_add_recent_color(move |color| {
        if let Some(ui) = weak_ui.upgrade() {
            add_recent_color(ui, color);
        }
    });
}

fn add_recent_color(ui: ui::PreviewUi, value: slint::Color) {
    let api = ui.global::<ui::Api>();

    let model = api.get_recent_colors();
    let Some(model) = model.as_any().downcast_ref::<slint::VecModel<slint::Color>>() else {
        return;
    };

    if let Some(position) = model.iter().position(|c| c == value) {
        model.remove(position);
    } else {
        while model.row_count() >= MAX_RECENT_COLORS {
            model.remove(model.row_count() - 1);
        }
    }

    model.insert(0, value);
}
