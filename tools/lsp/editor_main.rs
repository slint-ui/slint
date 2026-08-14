// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![allow(dead_code)]
// Keep Windows from opening a console behind the editor. Debug builds keep
// theirs, so that printing something still reaches somewhere visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod common;
#[cfg(feature = "preview")]
mod editor;
mod fmt;
mod language;
#[cfg(feature = "preview-engine")]
mod preview;
#[cfg(all(
    not(target_arch = "wasm32"),
    any(feature = "preview-external", feature = "preview-engine")
))]
mod settings_store;
mod server_notifier;
mod util;

pub use server_notifier::ServerNotifier;

fn main() -> std::result::Result<(), slint::PlatformError> {
    #[cfg(not(feature = "preview"))]
    panic!(
        "The visual editor was compiled without live-preview compiled in. Please compile with --features=preview to use it."
    );

    #[cfg(feature = "preview")]
    editor::editor_main()
}
