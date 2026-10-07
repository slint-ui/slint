// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Merging a Flickable's repeated children into its own geometry (#407) makes
//! the repeater's model reachable from that geometry. Counting the model as a
//! static dependency then reports a binding loop for a model that reads an
//! enclosing size -- picking a variant by breakpoint, say -- which is a build
//! failure under `SLINT_COMPILER_DENY_WARNINGS`. It doesn't need to be one: the
//! merge tracks the repeater's instantiated row count rather than the model.
//!
//! A `.slint` behavior test can't see this: it only checks geometry, and the
//! loop is reported as a warning.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

/// Every diagnostic compiling `source` produces, at any level.
fn diagnostics(source: &str) -> Vec<String> {
    let mut diag = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diag);
    let config = CompilerConfiguration::new(OutputFormat::Llr);
    let (_doc, diag, _loader) = spin_on::spin_on(compile_syntax_node(syntax_node, diag, config));
    diag.to_string_vec()
}

fn assert_no_diagnostic(name: &str, source: &str) {
    let diagnostics = diagnostics(source);
    assert!(diagnostics.is_empty(), "{name}: {diagnostics:?}");
}

#[test]
fn model_reading_the_flickables_own_width() {
    assert_no_diagnostic(
        "model reads the Flickable's width",
        r#"
export component Main inherits Window {
    VerticalLayout {
        f := Flickable {
            for i in (f.width > 100px ? [1, 2] : [1]): HorizontalLayout {
                Rectangle { height: 20px; }
            }
        }
        Rectangle { }
    }
}"#,
    );
}

#[test]
fn model_reading_an_enclosing_size() {
    assert_no_diagnostic(
        "model reads the window width",
        r#"
export component Main inherits Window {
    VerticalLayout {
        Flickable {
            for i in (root.width > 100px ? [1, 2] : [1]): HorizontalLayout {
                Rectangle { height: 20px; }
            }
        }
        Rectangle { }
    }
}"#,
    );
}

#[test]
fn condition_reading_an_enclosing_size() {
    assert_no_diagnostic(
        "condition reads the window width",
        r#"
export component Main inherits Window {
    VerticalLayout {
        Flickable {
            if root.width > 100px: HorizontalLayout {
                Rectangle { height: 20px; }
            }
        }
        Rectangle { }
    }
}"#,
    );
}
