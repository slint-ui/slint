// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::iter::once;

use i_slint_core::{
    model::{Model, VecModel},
    SharedString,
};
use slint_interpreter::PlatformError;

slint::include_modules!();

pub fn create_ui(style: String) -> Result<PreviewUi, PlatformError> {
    let ui = PreviewUi::new()?;

    // design mode:
    ui.on_design_mode_changed(super::set_design_mode);

    // styles:
    let known_styles = once(&"native")
        .chain(i_slint_compiler::fileaccess::styles().iter())
        .filter(|s| s != &&"qt" || i_slint_backend_selector::HAS_NATIVE_STYLE)
        .cloned()
        .collect::<Vec<_>>();
    let style = if known_styles.contains(&style.as_str()) {
        style
    } else {
        known_styles.first().map(|s| s.to_string()).unwrap_or_default()
    };

    let style_model = std::rc::Rc::new({
        let model = VecModel::default();
        model.extend(known_styles.iter().map(|s| SharedString::from(*s)));
        assert!(model.row_count() > 1);
        model
    });

    ui.on_style_changed(|| super::change_style());
    ui.set_known_styles(style_model.into());
    ui.set_current_style(style.clone().into());

    Ok(ui)
}
