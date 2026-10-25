// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[path = "project_file_macro/ancestor/nested/inline.rs"]
mod inline;

#[path = "project_file_macro/ancestor/nested/reexport.rs"]
mod reexport;

#[test]
fn inline_and_reexport_macros_ignore_project_files() {
    let _ = std::mem::size_of::<inline::Inline>();
    let _ = std::mem::size_of::<reexport::Reexported>();
}
