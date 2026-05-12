// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{path::PathBuf, rc::Weak};

use i_slint_core::InternalToken;

use crate::connection::Connection;

pub fn init_compiler(connection: Weak<Connection>) -> slint_interpreter::Compiler {
    let mut compiler = slint_interpreter::Compiler::new();

    let file_loader_connection = connection.clone();
    compiler.set_file_loader(move |path: &std::path::Path| {
        // make path absolute in our virtual file system
        let path = PathBuf::from("/").join(path);
        let connection = file_loader_connection.clone();
        Box::pin(async move {
            Some(if let Some(connection) = connection.upgrade() {
                connection
                    .request_file(path.as_os_str().to_str().unwrap().to_owned())
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

    let mapper_connection = connection.clone();
    compiler.compiler_configuration(InternalToken).resource_url_mapper =
        Some(std::rc::Rc::new(move |url: &str| {
            let connection = mapper_connection.clone();
            let path = url.to_owned();
            Box::pin(async move {
                if path.starts_with("builtin:/") {
                    return None;
                }
                let connection = connection.upgrade()?;
                let file_content = connection.request_file(path.clone()).await.ok()?;

                let extension = std::path::Path::new(&path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png");

                let mime_type = match extension {
                    "svg" | "svgz" => "image/svg+xml",
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "gif" => "image/gif",
                    "bmp" => "image/bmp",
                    "webp" => "image/webp",
                    _ => "application/octet-stream",
                };

                use base64::Engine as _;
                let encoded =
                    base64::engine::general_purpose::STANDARD.encode(&*file_content.contents);
                Some(format!("data:{mime_type};base64,{encoded}"))
            })
        }));

    compiler
}
