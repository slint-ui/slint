// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// TODO: Remove
#![allow(dead_code)]
#![cfg(not(test))]

mod common;
#[cfg(feature = "preview")]
mod editor;
mod fmt;
mod language;
#[cfg(feature = "preview-engine")]
mod preview;
mod util;

use crate::common::Result;
use editor::ServerNotifier;
use lsp_types::Url;

fn main() {
    #[cfg(not(feature = "preview"))]
    panic!(
        "The visual editor was compiled without live-preview compiled in. Please compile with --features=preview to use it."
    );

    #[cfg(feature = "preview")]
    editor::editor_main()
}
