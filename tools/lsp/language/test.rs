// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Code to help with writing tests for the language server

use lsp_types::{Diagnostic, Url};

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use crate::common;
use crate::language::convert_diagnostics;
use crate::language::reload_document_impl;

use super::Context;

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
    let (extra_files, diag) =
        spin_on::spin_on(reload_document_impl(None, content, url.clone(), Some(42), &mut dc));

    let diag = convert_diagnostics(&extra_files, diag);
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

pub fn load(
    ctx: Option<&Rc<Context>>,
    document_cache: &mut common::DocumentCache,
    path: &Path,
    content: &str,
) -> (Url, HashMap<Url, Vec<lsp_types::Diagnostic>>) {
    let url = Url::from_file_path(path).unwrap();

    let (main_file, diag) = spin_on::spin_on(reload_document_impl(
        ctx,
        content.into(),
        url.clone(),
        Some(1),
        document_cache,
    ));

    (url, convert_diagnostics(&main_file, diag))
}

#[test]
fn accurate_diagnostics_in_dependencies() {
    // Test for issue 5797
    let mut dc = empty_document_cache();

    let (bar_url, diag) = load(
        None,
        &mut dc,
        &std::env::current_dir().unwrap().join("xxx/bar.slint"),
        r#" export component Bar { property <int> hi; } "#,
    );
    assert_eq!(diag, HashMap::from_iter([(bar_url.clone(), vec![])]));

    let (reexport_url, diag) = load(
        None,
        &mut dc,
        &std::env::current_dir().unwrap().join("xxx/reexport.slint"),
        r#"import { Bar } from "bar.slint"; export component Foo inherits Bar { in property <string> reexport; }"#,
    );
    assert_eq!(diag, HashMap::from_iter([(reexport_url.clone(), vec![])]));

    let (foo_url, diag) = load(
        None,
        &mut dc,
        &std::env::current_dir().unwrap().join("xxx/foo.slint"),
        r#"import { Foo } from "reexport.slint"; export component MainWindow inherits Window { Foo { hello: 45; } }"#,
    );

    assert!(diag[&foo_url][0].message.contains("hello"));
    assert_eq!(diag.len(), 1);

    let ctx = Some(std::rc::Rc::new(crate::language::Context {
        document_cache: empty_document_cache().into(),
        preview_config: Default::default(),
        server_notifier: crate::ServerNotifier::dummy(),
        init_param: Default::default(),
        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        to_show: Default::default(),
        open_urls: RefCell::new(HashSet::from_iter([foo_url.clone(), bar_url.clone()])),
    }));

    let (bar_url, diag) = load(
        ctx.as_ref(),
        &mut dc,
        &std::env::current_dir().unwrap().join("xxx/bar.slint"),
        r#" export component Bar { in property <int> hello; } "#,
    );
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
    assert!(matches!(sym, lsp_types::DocumentSymbolResponse::Nested(result) if !result.is_empty()));

    let (foo_url, diag) = load(
        ctx.as_ref(),
        &mut dc,
        &std::env::current_dir().unwrap().join("xxx/foo.slint"),
        r#"import { Foo } from "reexport.slint"; export component MainWindow inherits Window { Foo { hi: 45; } }"#,
    );
    assert!(diag[&foo_url][0].message.contains("hi"));

    let (foo_url, diag) = load(
        ctx.as_ref(),
        &mut dc,
        &std::env::current_dir().unwrap().join("xxx/foo.slint"),
        r#"import { Foo } from "reexport.slint"; export component MainWindow inherits Window { Foo { hello: 12; } }"#,
    );
    assert_eq!(diag[&foo_url], vec![]);
}

#[test]
fn accurate_diagnostics_in_dependencies_with_parse_errors() {
    // Test for issue 8064
    let ctx = std::rc::Rc::new(crate::language::Context {
        document_cache: empty_document_cache().into(),
        preview_config: Default::default(),
        server_notifier: crate::ServerNotifier::dummy(),
        init_param: Default::default(),
        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        to_show: Default::default(),
        open_urls: Default::default(),
    });

    let (bar_url, diag) = load(
        Some(&ctx),
        &mut ctx.document_cache.borrow_mut(),
        &std::env::current_dir().unwrap().join("xxx/bar.slint"),
        r#" export component Bar { in property <int> hello; } "#,
    );
    assert_eq!(diag, HashMap::from_iter([(bar_url.clone(), vec![])]));

    ctx.open_urls.borrow_mut().insert(bar_url.clone());

    let (reexport_url, diag) = load(
        Some(&ctx),
        &mut ctx.document_cache.borrow_mut(),
        &std::env::current_dir().unwrap().join("xxx/reexport.slint"),
        r#"import { Bar } from "bar.slint"; export component Foo inherits Bar { in property <string> reexport; if true error }"#,
    );
    assert!(diag[&reexport_url].iter().any(|d| d.message.contains("Syntax error:")));
    assert_eq!(diag.len(), 1);

    ctx.open_urls.borrow_mut().insert(reexport_url.clone());

    let (foo_url, diag) = load(
        Some(&ctx),
        &mut ctx.document_cache.borrow_mut(),
        &std::env::current_dir().unwrap().join("xxx/foo.slint"),
        r#"import { Foo } from "reexport.slint"; export component MainWindow inherits Window { Foo { hello: 45; world: 12; } }"#,
    );
    assert!(diag[&foo_url][0].message.contains("world"));
    assert_eq!(diag[&foo_url].len(), 1);
    // Don't clear further error (so the client still has the parse error in reexport_url)
    assert_eq!(diag.len(), 1);

    ctx.open_urls.borrow_mut().insert(foo_url.clone());

    let (bar_url, diag) = load(
        Some(&ctx),
        &mut ctx.document_cache.borrow_mut(),
        &std::env::current_dir().unwrap().join("xxx/bar.slint"),
        r#" export component Bar { private property <int> hello; in property <int> world; } "#,
    );

    // bar still don't have error
    assert_eq!(diag[&bar_url], vec![]);
    // But reexport_url still have the same syntax error as before
    assert!(diag[&reexport_url].iter().any(|d| d.message.contains("Syntax error:")));
}
