// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! With edition 1.19, a repeated (`if`/`for`) child of a non-layout
//! parent contributes its constraints to that parent (#407), which is what these
//! diagnostics are about. A `.slint` behavior test can't see either case: one is
//! the absence of a warning, the other a warning.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::project_file::SlintVersion;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

/// Every diagnostic compiling `source` produces, at any level.
fn diagnostics(source: &str, merge_repeated: bool) -> Vec<String> {
    let mut diag = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diag);
    let mut config = CompilerConfiguration::new(OutputFormat::Llr);
    config.edition = merge_repeated.then_some(SlintVersion::V1_19);
    let (_doc, diag, _loader) = spin_on::spin_on(compile_syntax_node(syntax_node, diag, config));
    diag.to_string_vec()
}

fn assert_binding_loop(name: &str, source: &str) {
    let diagnostics = diagnostics(source, true);
    assert!(
        diagnostics.iter().any(|d| d.contains("binding loop")),
        "{name}: expected a binding loop, got {diagnostics:?}"
    );
}

/// A model that reads the parent's own size is circular in principle: the parent is
/// measured against however many rows the model produces, and that count comes from
/// the parent's size. Nothing reports it, because the merge tracks the repeater's
/// instantiated row count rather than the model property. Reporting it would fail
/// every build under `SLINT_COMPILER_DENY_WARNINGS` that picks a UI variant by
/// breakpoint.
#[test]
fn model_reading_an_enclosing_size() {
    for (name, repeated) in [
        ("model", "for i in (root.width > 100px ? [1, 2] : [1]): Rectangle { height: 20px; }"),
        ("condition", "if root.width > 100px: Rectangle { height: 20px; }"),
    ] {
        let source = format!(
            r#"
export component Main inherits Window {{
    VerticalLayout {{
        Rectangle {{
            {repeated}
        }}
        Rectangle {{ }}
    }}
}}"#
        );
        let diagnostics = diagnostics(&source, true);
        assert!(diagnostics.is_empty(), "{name} reads the window width: {diagnostics:?}");
    }
}

fn child_reading_an_enclosing_size(child: &str) -> String {
    format!(
        r#"
export component Main inherits Window {{
    out property <length> win-height: root.height;
    Rectangle {{
        {child}
    }}
}}"#
    )
}

const STATIC_CHILD: &str = r#"Text { text: root.win-height / 1px; }"#;
const REPEATED_CHILD: &str = r#"if true: Text { text: root.win-height / 1px; }"#;

/// A child whose own layout info depends on the size it is laid out in closes a
/// loop through its parent. That is reported for a static child, and with the
/// merge on, a repeated child is no different.
#[test]
fn child_reading_an_enclosing_size_loops_either_way() {
    assert_binding_loop("static child", &child_reading_an_enclosing_size(STATIC_CHILD));
    assert_binding_loop("repeated child", &child_reading_an_enclosing_size(REPEATED_CHILD));
}

/// The default, which the merge breaks: a repeated child is left out, so the same
/// source compiles. A static child loops either way (#13972).
#[test]
fn repeated_child_is_left_out_by_default() {
    let repeated = diagnostics(&child_reading_an_enclosing_size(REPEATED_CHILD), false);
    assert!(repeated.is_empty(), "repeated child: {repeated:?}");
    let static_child = diagnostics(&child_reading_an_enclosing_size(STATIC_CHILD), false);
    assert!(
        static_child.iter().any(|d| d.contains("binding loop")),
        "static child: expected a binding loop, got {static_child:?}"
    );
}
