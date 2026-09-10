// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use crate::preview::ui;

use slint::Model;

const MAX_RECENT_FILLS: usize = 9;

pub fn setup(api: &ui::Api<'_>, api_weak: slint::Weak<ui::Api<'static>>) {
    api.on_filter_recent_fills(|fills, allow_gradient| {
        Rc::new(slint::VecModel::from(fills.iter()
            .filter(|fill| allow_gradient || fill.kind == ui::BrushKind::Solid)
            .collect::<Vec<_>>())).into()
    });
    api.set_recent_fills(Rc::new(slint::VecModel::default()).into());

    api.on_add_recent_fill(move |color| {
        if let Some(api) = api_weak.upgrade() {
            add_recent_fill(&api, color);
        }
    });
}

fn add_recent_fill(api: &ui::Api<'_>, value: ui::FillData) {
    let model = api.get_recent_fills();
    let Some(model) = model.as_any().downcast_ref::<slint::VecModel<ui::FillData>>() else {
        return;
    };

    if let Some(position) =
        model.iter().position(|c| ui::fill_brush(c) == ui::fill_brush(value.clone()))
    {
        model.remove(position);
    } else {
        while model.row_count() >= MAX_RECENT_FILLS {
            model.remove(model.row_count() - 1);
        }
    }

    model.insert(0, value);
}
