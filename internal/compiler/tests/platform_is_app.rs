// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `Platform.is-app` must be constant-folded at compile time: `false` only when
//! `CompilerConfiguration::is_preview` is set (as `slint-viewer` and the LSP/editor preview
//! do), `true` otherwise — including plain `OutputFormat::Interpreter` compiles such as the
//! ones the Node.js and Python APIs perform when dynamically loading a `.slint` file, which
//! have no other way to distinguish themselves from a preview tool.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

fn is_app_folds_to(config: CompilerConfiguration, expected: bool) {
    let source = r#"
        component TestCase {
            out property <bool> test: Platform.is-app;
        }
    "#;
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diagnostics);
    let (doc, diagnostics, _loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, config));
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());

    let component = doc.inner_components.last().unwrap();
    let root_element = component.root_element.borrow();
    let binding = root_element.binding("test").unwrap();
    let folded = binding.expression.ignore_debug_hooks();
    assert!(
        matches!(folded, Expression::BoolLiteral(b) if *b == expected),
        "Platform.is-app did not fold to a BoolLiteral({expected}), got {folded:?}"
    );
}

#[test]
fn is_app_true_for_plain_interpreter() {
    // No other signal distinguishes this from Node.js/Python dynamically loading a .slint
    // file as part of a real application, so it must default to `true`.
    is_app_folds_to(CompilerConfiguration::new(OutputFormat::Interpreter), true);
}

#[test]
fn is_app_false_when_marked_preview() {
    // What slint-viewer and the LSP/editor preview explicitly opt into.
    let mut config = CompilerConfiguration::new(OutputFormat::Interpreter);
    config.is_preview = true;
    is_app_folds_to(config, false);
}

#[test]
fn is_app_true_for_llr() {
    is_app_folds_to(CompilerConfiguration::new(OutputFormat::Llr), true);
}
