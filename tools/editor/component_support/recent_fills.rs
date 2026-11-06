// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use crate::ui;

use slint::Model;

const MAX_RECENT_FILLS: usize = 9;

pub fn setup(
    properties_api: &ui::Properties<'_>,
    properties_api_weak: slint::Weak<ui::Properties<'static>>,
) {
    properties_api.on_filter_recent_fills(|fills, allow_gradient| {
        Rc::new(slint::VecModel::from(
            fills
                .iter()
                .filter(|fill| allow_gradient || fill.kind == ui::BrushKind::Solid)
                .collect::<Vec<_>>(),
        ))
        .into()
    });
    properties_api.set_recent_fills(Rc::new(slint::VecModel::default()).into());

    properties_api.on_add_recent_fill(move |fill| {
        if let Some(properties_api) = properties_api_weak.upgrade() {
            add_recent_fill(&properties_api, fill);
        }
    });
}

fn add_recent_fill(properties_api: &ui::Properties<'_>, value: ui::FillData) {
    let brush = super::brushes::fill_brush(value.clone());
    let mut fills = properties_api
        .get_recent_fills()
        .iter()
        .filter(|fill| super::brushes::fill_brush(fill.clone()) != brush)
        .collect::<Vec<_>>();
    fills.insert(0, value);
    fills.truncate(MAX_RECENT_FILLS);
    properties_api.set_recent_fills(Rc::new(slint::VecModel::from(fills)).into());
}
