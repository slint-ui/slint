// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::preview::ui;

pub fn setup(ui: &ui::PreviewUi) {
    let api = ui.global::<ui::Api>();
    api.on_filter_log_messages(filter_log_messages);
    api.on_clear_log_messages(clear_log_messages);

    clear_log_messages_impl(ui);
}

pub fn append_log_message(
    ui: &ui::PreviewUi,
    level: ui::LogMessageLevel,
    location: Option<(SharedString, usize, usize)>,
    message: &str,
) {
    let api = ui.global::<ui::Api>();

    let log_model = api.get_log_output();
    let Some(log_model) = log_model.as_any().downcast_ref::<VecModel<ui::LogMessage>>() else {
        return;
    };

    let location = location.unwrap_or_default();

    log_model.push(ui::LogMessage {
        file: location.0,
        line: location.1 as i32,
        column: location.2 as i32,
        message: message.into(),
        level,
    });
}

pub fn clear_log_messages() {
    crate::preview::PREVIEW_STATE.with_borrow(|preview_state| {
        if let Some(ui) = &preview_state.ui {
            clear_log_messages_impl(ui);
        }
    })
}

pub fn clear_log_messages_impl(ui: &ui::PreviewUi) {
    let api = ui.global::<ui::Api>();

    api.set_log_output(Rc::new(VecModel::default()).into());
}

pub fn filter_log_messages(
    messages: ModelRc<ui::LogMessage>,
    pattern: SharedString,
) -> ModelRc<ui::LogMessage> {
    let pattern = pattern.to_string();
    Rc::new(VecModel::from(filter_log_messages_iter(&mut messages.iter(), &pattern))).into()
}

fn filter_log_messages_iter(
    input: &mut impl Iterator<Item = ui::LogMessage>,
    needle: &str,
) -> Vec<ui::LogMessage> {
    use nucleo_matcher::{pattern, Config, Matcher};

    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let pattern = pattern::Pattern::parse(
        needle,
        pattern::CaseMatching::Ignore,
        pattern::Normalization::Smart,
    );

    let mut all_matches = input
        .filter_map(|lm| {
            let terms = [format!("{} %level:{:?} %file:{}", lm.message, lm.level, lm.file,)];
            pattern.match_list(terms.iter(), &mut matcher).pop().map(|(_, v)| (v, lm))
        })
        .collect::<Vec<_>>();

    // sort by value, highest first. Sort names with the same value alphabetically
    all_matches.sort_by(|r, l| match l.0.cmp(&r.0) {
        std::cmp::Ordering::Less => std::cmp::Ordering::Less,
        std::cmp::Ordering::Equal => r.1.message.cmp(&l.1.message),
        std::cmp::Ordering::Greater => std::cmp::Ordering::Greater,
    });

    let cut_off = {
        let lowest_value = all_matches.last().map(|(v, _)| *v).unwrap_or_default();
        let highest_value = all_matches.first().map(|(v, _)| *v).unwrap_or_default();

        if all_matches.len() < 10 {
            lowest_value
        } else {
            highest_value - (highest_value - lowest_value) / 2
        }
    };

    all_matches.drain(..).take_while(|(v, _)| *v >= cut_off).map(|(_, p)| p).collect::<Vec<_>>()
}
