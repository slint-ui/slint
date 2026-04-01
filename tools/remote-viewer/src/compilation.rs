// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{path::PathBuf, rc::Weak};

use lsp_types::Url;

use crate::connection::Connection;

pub fn init_compiler(connection: Weak<Connection>) -> slint_interpreter::Compiler {
    let mut compiler = slint_interpreter::Compiler::new();

    compiler.set_file_loader(move |path: &std::path::Path| {
        // make path absolute in our virtual file system
        let path = PathBuf::from("/").join(path);
        let connection = connection.clone();
        Box::pin(async move {
            Some(if let Some(connection) = connection.upgrade() {
                connection
                    .request_file(Url::from_file_path(path).unwrap())
                    .await
                    .map(|file_content| String::from_utf8_lossy(&file_content.contents).to_string())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Connection is no longer available",
                ))
            })
        })
    });

    compiler
}
