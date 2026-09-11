// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use crate::preview::ui;

use slint::Model;

const MAX_RECENT_FILLS: usize = 9;

pub fn setup(api: &ui::Api<'_>, api_weak: slint::Weak<ui::Api<'static>>) {
    api.on_filter_recent_fills(|fills, allow_gradient| {
        Rc::new(slint::VecModel::from(
            fills
                .iter()
                .filter(|fill| allow_gradient || fill.kind == ui::BrushKind::Solid)
                .collect::<Vec<_>>(),
        ))
        .into()
    });
    api.set_recent_fills(Rc::new(slint::VecModel::default()).into());

    api.on_add_recent_fill(move |fill| {
        if let Some(api) = api_weak.upgrade() {
            add_recent_fill(&api, fill);
        }
    });
}

fn add_recent_fill(api: &ui::Api<'_>, value: ui::FillData) {
    let brush = ui::fill_brush(value.clone());
    let mut fills = api
        .get_recent_fills()
        .iter()
        .filter(|fill| ui::fill_brush(fill.clone()) != brush)
        .collect::<Vec<_>>();
    fills.insert(0, value);
    fills.truncate(MAX_RECENT_FILLS);
    api.set_recent_fills(Rc::new(slint::VecModel::from(fills)).into());
}
