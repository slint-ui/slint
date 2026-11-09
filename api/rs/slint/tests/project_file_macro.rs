// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[path = "project_file_macro/ancestor/nested/inline.rs"]
mod inline;

#[path = "project_file_macro/ancestor/nested/reexport.rs"]
mod reexport;

#[path = "project_file_macro/ancestor/nested/project.rs"]
mod relative_project;

mod manifest_project {
    slint::compile_project!();
}

#[test]
fn inline_and_reexport_macros_ignore_project_files() {
    let _ = std::mem::size_of::<inline::Inline>();
    let _ = std::mem::size_of::<reexport::Reexported>();
}

#[test]
fn compile_project_uses_manifest_directory() {
    let _ = std::mem::size_of::<manifest_project::Reexported>();
}

#[test]
fn compile_project_resolves_paths_from_source_file() {
    let _ = std::mem::size_of::<relative_project::directory::Reexported>();
    let _ = std::mem::size_of::<relative_project::file::Reexported>();
}
