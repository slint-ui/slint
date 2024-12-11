// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Code to help with writing tests for the language server

use lsp_types::{Diagnostic, Url};

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use crate::common;
use crate::language::reload_document_impl;

/// Create an empty `DocumentCache`
pub fn empty_document_cache() -> common::DocumentCache {
    let mut config = crate::common::document_cache::CompilerConfiguration::default();
    config.style = Some("fluent".to_string());
    common::DocumentCache::new(config)
}

/// Create a `DocumentCache` with one document loaded into it.
pub fn loaded_document_cache(
    content: String,
) -> (common::DocumentCache, Url, HashMap<Url, Vec<Diagnostic>>) {
    loaded_document_cache_with_file_name(content, "bar.slint")
}

pub fn loaded_document_cache_with_file_name(
    content: String,
    file_name: &str,
) -> (common::DocumentCache, Url, HashMap<Url, Vec<Diagnostic>>) {
    let mut dc = empty_document_cache();

    // Pre-load std-widgets.slint:
    spin_on::spin_on(dc.preload_builtins());

    let dummy_absolute_path = if cfg!(target_family = "windows") {
        format!("c://foo/{file_name}")
    } else {
        format!("/foo/{file_name}")
    };
    let url = Url::from_file_path(dummy_absolute_path).unwrap();
    let diag =
        spin_on::spin_on(reload_document_impl(None, content, url.clone(), Some(42), &mut dc));
    (dc, url, diag)
}

/// Create a `DocumentCache` with one comparatively complex test document loaded into it.
pub fn complex_document_cache() -> (common::DocumentCache, Url, HashMap<Url, Vec<Diagnostic>>) {
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

#[test]
fn accurate_diagnostics_in_dependencies() {
    // Test for issue 5797
    let mut dc = empty_document_cache();

    let bar_ctn = r#" export component Bar { property <int> hi; } "#;
    let bar_url =
        Url::from_file_path(std::env::current_dir().unwrap().join("xxx/bar.slint")).unwrap();
    let diag = spin_on::spin_on(reload_document_impl(
        None,
        bar_ctn.into(),
        bar_url.clone(),
        Some(1),
        &mut dc,
    ));
    assert_eq!(diag, HashMap::from_iter([(bar_url.clone(), vec![])]));

    let reexport_ctn = r#"import { Bar } from "bar.slint"; export component Foo inherits Bar { in property <string> reexport; }"#;
    let reexport_url =
        Url::from_file_path(std::env::current_dir().unwrap().join("xxx/reexport.slint")).unwrap();
    let diag = spin_on::spin_on(reload_document_impl(
        None,
        reexport_ctn.into(),
        reexport_url.clone(),
        Some(1),
        &mut dc,
    ));
    assert_eq!(
        diag,
        HashMap::from_iter([(reexport_url.clone(), vec![]), (bar_url.clone(), vec![])])
    );

    let foo_ctn = r#"import { Foo } from "reexport.slint"; export component MainWindow inherits Window { Foo { hello: 45; } }"#;
    let foo_url =
        Url::from_file_path(std::env::current_dir().unwrap().join("xxx/foo.slint")).unwrap();
    let diag = spin_on::spin_on(reload_document_impl(
        None,
        foo_ctn.into(),
        foo_url.clone(),
        Some(1),
        &mut dc,
    ));
    //assert_eq!(diag.len(), 3);
    assert_eq!(diag[&reexport_url], vec![]);
    //assert_eq!(diag[&bar_url], vec![]);
    assert!(diag[&foo_url][0].message.contains("hello"));

    let ctx = Some(std::rc::Rc::new(crate::language::Context {
        document_cache: empty_document_cache().into(),
        preview_config: Default::default(),
        server_notifier: crate::ServerNotifier::dummy(),
        init_param: Default::default(),
        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        to_show: Default::default(),
        open_urls: RefCell::new(HashSet::from_iter([foo_url.clone(), bar_url.clone()])),
    }));

    let bar_ctn = r#" export component Bar { in property <int> hello; } "#;
    let diag = spin_on::spin_on(reload_document_impl(
        ctx.as_ref(),
        bar_ctn.into(),
        bar_url.clone(),
        Some(1),
        &mut dc,
    ));
    assert_eq!(diag.len(), 3);
    assert_eq!(
        diag,
        HashMap::from_iter([
            (reexport_url.clone(), vec![]),
            (bar_url.clone(), vec![]),
            (foo_url.clone(), vec![])
        ])
    );

    let sym = crate::language::get_document_symbols(
        &mut dc,
        &lsp_types::TextDocumentIdentifier { uri: foo_url.clone() },
    )
    .expect("foo.slint should still be loaded");
    assert!(matches!(sym, lsp_types::DocumentSymbolResponse::Nested(result) if result.len() >= 1));

    let foo_ctn = r#"import { Foo } from "reexport.slint"; export component MainWindow inherits Window { Foo { hi: 45; } }"#;
    let diag = spin_on::spin_on(reload_document_impl(
        ctx.as_ref(),
        foo_ctn.into(),
        foo_url.clone(),
        Some(1),
        &mut dc,
    ));
    assert!(diag[&foo_url][0].message.contains("hi"));

    let foo_ctn = r#"import { Foo } from "reexport.slint"; export component MainWindow inherits Window { Foo { hello: 12; } }"#;
    let diag = spin_on::spin_on(reload_document_impl(
        ctx.as_ref(),
        foo_ctn.into(),
        foo_url.clone(),
        Some(1),
        &mut dc,
    ));
    assert_eq!(diag[&foo_url], vec![]);
}
