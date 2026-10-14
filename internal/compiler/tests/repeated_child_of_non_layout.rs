// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A repeated (`if`/`for`) child of a non-layout parent contributes its
//! constraints to that parent (#407), which is what these diagnostics are about.
//! A `.slint` behavior test can't see either case: one is the absence of a
//! warning, the other a warning.

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

fn assert_binding_loop(name: &str, source: &str) {
    let diagnostics = diagnostics(source);
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
        let diagnostics = diagnostics(&source);
        assert!(diagnostics.is_empty(), "{name} reads the window width: {diagnostics:?}");
    }
}

/// A child whose own layout info depends on the size it is laid out in would make its
/// parent's size depend on itself. Its parent leaves it out instead, static or repeated.
/// Through the window's layout, a static child stays the deprecation warning it was.
#[test]
fn child_reading_an_enclosing_size() {
    let static_child = r#"Text { text: root.h / 1px; }"#;
    let repeated_child = r#"if true: Text { text: root.h / 1px; }"#;
    for child in [static_child, repeated_child] {
        let source = format!(
            r#"
component Inner {{
    out property <length> h: root.height;
    Rectangle {{
        {child}
    }}
}}
export component Main inherits Window {{
    Rectangle {{ x: 0; VerticalLayout {{ Inner {{ }} }} }}
}}"#
        );
        let diagnostics = diagnostics(&source);
        assert!(diagnostics.is_empty(), "{child}: {diagnostics:?}");
    }

    let in_window = |child: &str| {
        format!(
            r#"
export component Main inherits Window {{
    out property <length> h: root.height;
    Rectangle {{
        {child}
    }}
}}"#
        )
    };
    assert_binding_loop("static child", &in_window(static_child));
    let diagnostics = diagnostics(&in_window(repeated_child));
    assert!(diagnostics.is_empty(), "repeated child: {diagnostics:?}");
}
