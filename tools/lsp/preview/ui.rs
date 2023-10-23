// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use slint_interpreter::PlatformError;

slint::include_modules!();

pub fn create_ui() -> Result<PreviewUi, PlatformError> {
    let ui = PreviewUi::new()?;
    ui.on_design_mode_changed(super::set_design_mode);
    Ok(ui)
}
