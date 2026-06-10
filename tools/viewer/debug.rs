// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::PathBuf;

pub fn debug_handler(
    location: Option<&i_slint_core::debug_log::DebugLogLocation>,
    arguments: core::fmt::Arguments,
) -> Option<(PathBuf, usize, usize)> {
    let location = location
        .map(|location| (PathBuf::from(location.path.as_str()), location.line, location.column));
    if let Some((file, line, column)) = &location {
        tracing::info!("DEBUG {file}:{line}:{column}> {arguments}", file = file.display());
    } else {
        tracing::info!("DEBUG> {arguments}");
    }

    location
}
