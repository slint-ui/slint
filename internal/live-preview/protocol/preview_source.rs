// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::project_file::{
    FILE_NAME, normalize_project_file_path, resolve_input_with_file_loader,
};
use i_slint_compiler::source_path::SourcePath;
use std::rc::Rc;

pub async fn preview_source_path(input: &SourcePath, contents: &str) -> Option<SourcePath> {
    if input.file_name() != Some(FILE_NAME) {
        return Some(input.clone());
    }
    let root = normalize_project_file_path(input.clone());
    let contents = contents.to_owned();
    let loader: i_slint_compiler::OpenImportCallback = Rc::new(move |path| {
        let source = if path == root {
            Ok(contents.clone())
        } else {
            Err(std::io::ErrorKind::NotFound.into())
        };
        Box::pin(async move { Some(source) })
    });
    resolve_input_with_file_loader(input.clone(), Some(&loader))
        .await
        .ok()
        .flatten()
        .map(|resolved| resolved.source)
}
