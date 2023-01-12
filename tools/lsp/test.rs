// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Code to help with writing tests for the language server

use lsp_types::{Diagnostic, Url};

use std::collections::HashMap;

use crate::server_loop::{reload_document_impl, DocumentCache};

/// Create an empty `DocumentCache`
pub fn empty_document_cache(style: &str) -> DocumentCache {
    let mut config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    config.style = Some(style.to_string());
    DocumentCache::new(config)
}

/// Create a `DocumentCache` with one document loaded into it.
pub fn loaded_document_cache(
    style: &str,
    content: String,
) -> (DocumentCache, Url, HashMap<Url, Vec<Diagnostic>>) {
    let mut dc = empty_document_cache(style);
    let dummy_absolute_path =
        if cfg!(target_family = "windows") { "c://foo/bar.slint" } else { "/foo/bar.slint" };
    let url = Url::from_file_path(dummy_absolute_path).unwrap();
    let diag = spin_on::spin_on(async {
        reload_document_impl(content, url.clone(), 42, &mut dc)
            .await
            .expect("reload_document_impl failed.")
    });
    (dc, url, diag)
}

/// Create a `DocumentCache` with one comparatively complex test document loaded into it.
pub fn complex_document_cache(style: &str) -> (DocumentCache, Url, HashMap<Url, Vec<Diagnostic>>) {
    loaded_document_cache(style,
            r#"import { LineEdit, Button, Slider, HorizontalBox, VerticalBox } from "std-widgets.slint";

component MainWindow inherits Window {
    property <duration> total-time: slider.value * 1s;
    property <duration> elapsed-time;

    callback tick(duration);
    tick(passed-time) => {
        elapsed-time += passed-time;
        elapsed-time = min(elapsed-time, total-time);
    }

    VerticalBox {
        HorizontalBox {
            padding-left: 0;
            Text { text: "Elapsed Time:"; }
            Rectangle {
                min-width: 200px;
                max-height: 30px;
                background: gray;
                Rectangle {
                    height: 100%;
                    width: parent.width * (elapsed-time/total-time);
                    background: lightblue;
                }
            }
        }
        Text{
            text: (total-time / 1s) + "s";
        }
        HorizontalBox {
            padding-left: 0;
            Text {
                text: "Duration:";
                vertical-alignment: center;
            }
            slider := Slider {
                maximum: 30s / 1s;
                value: 10s / 1s;
                changed(new-duration) => {
                    root.total-time = new-duration * 1s;
                    root.elapsed-time = min(root.elapsed-time, root.total-time);
                }
            }
        }
        Button {
            text: "Reset";
            clicked => {
                elapsed-time = 0
            }
        }
    }
}
            "#.to_string())
}
