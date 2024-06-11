// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Code to help with writing tests for the language server

use lsp_types::{Diagnostic, Url};

use std::collections::HashMap;

use crate::language::{reload_document_impl, DocumentCache};

/// Create an empty `DocumentCache`
pub fn empty_document_cache() -> DocumentCache {
    let mut config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    config.style = Some("fluent".to_string());
    DocumentCache::new(config)
}

/// Create a `DocumentCache` with one document loaded into it.
pub fn loaded_document_cache(
    content: String,
) -> (DocumentCache, Url, HashMap<Url, Vec<Diagnostic>>) {
    let mut dc = empty_document_cache();

    // Pre-load std-widgets.slint:
    spin_on::spin_on(dc.preload_builtins());

    let dummy_absolute_path =
        if cfg!(target_family = "windows") { "c://foo/bar.slint" } else { "/foo/bar.slint" };
    let url = Url::from_file_path(dummy_absolute_path).unwrap();
    let diag =
        spin_on::spin_on(reload_document_impl(None, content, url.clone(), Some(42), &mut dc));
    (dc, url, diag)
}

/// Create a `DocumentCache` with one comparatively complex test document loaded into it.
pub fn complex_document_cache() -> (DocumentCache, Url, HashMap<Url, Vec<Diagnostic>>) {
    loaded_document_cache(
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
