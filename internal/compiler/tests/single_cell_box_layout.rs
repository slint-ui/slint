// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The `.slint` behavior tests (`box_single_cell.slint`, `box_cross_stretch_constraints.slint`)
//! pass whether the geometry comes from the compile-time lowering or from the runtime solver, so
//! they can't tell whether the optimization actually fired. These tests inspect the lowered LLR
//! instead: a compile-time-solvable single-cell box layout must not emit a `solve_box_layout`
//! runtime call, while the cases that keep the solver must still emit it.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::{self, OutputFormat};
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

/// Compile `source`, lower it through the LLR back-end (the way the native code generators do) and
/// return the pretty-printed LLR. `debug_info` is left at its default (false), matching real
/// `slint-build` / `slint!` users.
fn lower_to_llr_text(source: &str) -> String {
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diagnostics);
    let config = CompilerConfiguration::new(OutputFormat::Llr);
    let (doc, diagnostics, loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, config));
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());
    let mut output = Vec::new();
    generator::generate(OutputFormat::Llr, &mut output, None, &doc, &loader.compiler_config)
        .unwrap();
    String::from_utf8(output).unwrap()
}

/// The runtime box layout solver appears as a `solve_box_layout` / `solve_box_layout_ortho` call in
/// the lowered expressions.
fn uses_runtime_solver(llr: &str) -> bool {
    llr.contains("solve_box_layout")
}

#[test]
fn single_cell_stretch_solved_at_compile_time() {
    let llr = lower_to_llr_text(
        r#"
export component TestCase inherits Window {
    width: 300px;
    height: 200px;
    VerticalLayout {
        padding: 0px;
        Rectangle { max-height: 150px; }
    }
}
"#,
    );
    assert!(!uses_runtime_solver(&llr), "the solver should have been elided:\n{llr}");
}

#[test]
fn single_cell_center_solved_at_compile_time() {
    let llr = lower_to_llr_text(
        r#"
export component TestCase inherits Window {
    width: 300px;
    height: 200px;
    VerticalLayout {
        padding: 0px;
        alignment: center;
        Rectangle { preferred-height: 50px; }
    }
}
"#,
    );
    assert!(!uses_runtime_solver(&llr), "the solver should have been elided:\n{llr}");
}

#[test]
fn percent_size_keeps_runtime_solver() {
    // A percent size can't be resolved at compile time, so the solver stays. This is the control
    // that proves the checks above are meaningful.
    let llr = lower_to_llr_text(
        r#"
export component TestCase inherits Window {
    width: 300px;
    height: 200px;
    VerticalLayout {
        padding: 0px;
        Rectangle { height: 25%; }
    }
}
"#,
    );
    assert!(uses_runtime_solver(&llr), "a percent constraint needs the solver:\n{llr}");
}

#[test]
fn repeated_cell_keeps_runtime_solver() {
    let llr = lower_to_llr_text(
        r#"
export component TestCase inherits Window {
    width: 300px;
    height: 200px;
    VerticalLayout {
        padding: 0px;
        for i in [0]: Rectangle { min-height: 200px; }
    }
}
"#,
    );
    assert!(uses_runtime_solver(&llr), "a repeated cell needs the solver:\n{llr}");
}
