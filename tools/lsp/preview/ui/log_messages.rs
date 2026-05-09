// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use slint::{Model, ModelRc, SharedString, VecModel};

use crate::{common, preview::ui};

pub fn setup(api: &ui::Api<'_>) {
    api.on_filter_log_messages(filter_log_messages);
    api.on_clear_log_messages(clear_log_messages);

    clear_log_messages_impl(api);
}

pub fn append_log_message(
    api: &ui::Api<'_>,
    level: ui::LogMessageLevel,
    location: Option<(SharedString, usize, usize)>,
    message: &str,
) {
    let log_model = api.get_log_output();
    let Some(model) = log_model.as_any().downcast_ref::<VecModel<ui::LogMessage>>() else {
        return;
    };

    let location = location.unwrap_or_default();

    model.push(ui::LogMessage {
        file: location.0,
        line: location.1 as i32,
        column: location.2 as i32,
        message: message.into(),
        level,
    });
}

pub fn clear_log_messages() {
    crate::preview::PREVIEW_STATE.with_borrow(|preview_state| {
        if let Some(api) = preview_state.api.upgrade() {
            clear_log_messages_impl(&api);
        }
    })
}

pub fn clear_log_messages_impl(api: &ui::Api<'_>) {
    api.set_log_output(Rc::new(VecModel::default()).into());
}

pub fn filter_log_messages(
    messages: ModelRc<ui::LogMessage>,
    pattern: SharedString,
) -> ModelRc<ui::LogMessage> {
    let pattern = pattern.to_string();
    tracing::debug!("messages: row_count: {}", messages.row_count());
    Rc::new(VecModel::from(common::fuzzy_filter_iter(
        &mut messages.iter(),
        |lm| format!("{} %level:{:?} %file:{}", lm.message, lm.level, lm.file,),
        &pattern,
    )))
    .into()
}
