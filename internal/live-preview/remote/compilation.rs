// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Weak;

use i_slint_core::InternalToken;
use lsp_types::Url;

use super::connection::Connection;

pub fn init_compiler(connection: Weak<Connection>) -> slint_interpreter::Compiler {
    let mut compiler = slint_interpreter::Compiler::new();

    let file_loader_connection = connection.clone();
    compiler.set_file_loader(move |path: &std::path::Path| {
        let url = Url::from_file_path(path);
        let path_display = path.display().to_string();
        let connection = file_loader_connection.clone();
        Box::pin(async move {
            let Some(connection) = connection.upgrade() else {
                return Some(Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Connection is no longer available",
                )));
            };
            let Ok(url) = url else {
                return Some(Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Not an absolute file path: {path_display}"),
                )));
            };
            Some(
                connection.request_file(url).await.map(|file_content| {
                    String::from_utf8_lossy(&file_content.contents).to_string()
                }),
            )
        })
    });

    let mapper_connection = connection.clone();
    compiler.compiler_configuration(InternalToken).resource_url_mapper =
        Some(std::rc::Rc::new(move |url: &Url| {
            let connection = mapper_connection.clone();
            let url = url.clone();
            Box::pin(async move {
                // Only files on the editor's machine need fetching over the
                // connection; `builtin:/`, `data:`, `http(s):`, ... are loaded
                // directly by the renderer.
                if url.scheme() != "file" {
                    return None;
                }
                let connection = connection.upgrade()?;
                let extension = std::path::Path::new(url.path())
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png");
                let mime_type = i_slint_core::graphics::image_mime_type_from_extension(extension)
                    .unwrap_or("application/octet-stream");
                let file_content = connection.request_file(url).await.ok()?;

                use base64::Engine as _;
                let encoded =
                    base64::engine::general_purpose::STANDARD.encode(&*file_content.contents);
                Url::parse(&format!("data:{mime_type};base64,{encoded}")).ok()
            })
        }));

    compiler
}
