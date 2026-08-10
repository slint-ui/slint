// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::PathBuf;

pub fn log_message_handler(
    message: &i_slint_core::debug_log::LogMessage<'_>,
) -> Option<(PathBuf, usize, usize)> {
    let arguments = message.message_arguments();
    let location = message
        .location()
        .map(|location| (PathBuf::from(location.path), location.line, location.column));
    if let Some((file, line, column)) = &location {
        tracing::info!("DEBUG {file}:{line}:{column}> {arguments}", file = file.display());
    } else {
        tracing::info!("DEBUG> {arguments}");
    }

    location
}
