// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_core::{
    model::{Model, VecModel},
    SharedString,
};
use slint_interpreter::PlatformError;

slint::include_modules!();

pub fn create_ui(style: String) -> Result<(PreviewUi, String), PlatformError> {
    let default_style =
        i_slint_common::get_native_style(false, &std::env::var("TARGET").unwrap_or_default());

    let ui = PreviewUi::new()?;

    // design mode:
    ui.on_design_mode_changed(super::set_design_mode);

    // styles:
    let known_styles = i_slint_compiler::fileaccess::styles()
        .iter()
        .filter(|s| s != &&"qt" || i_slint_backend_selector::HAS_NATIVE_STYLE)
        .cloned()
        .collect::<Vec<_>>();
    let style = if known_styles.contains(&style.as_str()) {
        style
    } else if known_styles.contains(&default_style) {
        default_style.to_string()
    } else {
        known_styles.first().map(|s| s.to_string()).unwrap_or_default()
    };

    let style_model = std::rc::Rc::new({
        let model = VecModel::default();
        model.extend(known_styles.iter().map(|s| SharedString::from(*s)));
        assert!(model.row_count() > 0);
        model
    });

    ui.on_style_changed(|style| super::change_style(style.into()));
    ui.set_known_styles(style_model.into());
    ui.set_current_style(style.clone().into());

    Ok((ui, style))
}
