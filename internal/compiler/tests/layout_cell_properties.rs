// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The per-item properties of a `FlexboxLayout`, `HorizontalLayout` or `VerticalLayout`
//! cell (`layout-order`, `cross-axis-self-alignment`) are stable API. The syntax tests
//! can't verify that because they always enable the experimental features, so these
//! tests compile with them disabled.

use i_slint_compiler::diagnostics::{BuildDiagnostics, DiagnosticLevel};
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

const LAYOUT_CELL_PROPERTIES: &[&str] = &["layout-order: 3", "cross-axis-self-alignment: center"];

const CELL_LAYOUTS: &[&str] = &["FlexboxLayout", "HorizontalLayout", "VerticalLayout"];

const OUTSIDE_ERROR: &str = "used outside of a FlexboxLayout, HorizontalLayout, or VerticalLayout";

/// Compile `source` without experimental features and return its errors
/// (warnings are not of interest here).
fn errors(source: String) -> Vec<String> {
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source, None, &mut diagnostics);
    let mut config = CompilerConfiguration::new(OutputFormat::Interpreter);
    // The default follows SLINT_ENABLE_EXPERIMENTAL_FEATURES; pin it off.
    config.enable_experimental = false;
    let (_, diagnostics, _) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, config));
    diagnostics
        .iter()
        .filter(|d| d.level() == DiagnosticLevel::Error)
        .map(|d| d.message().to_owned())
        .collect()
}

fn in_layout(layout: &str, binding: &str) -> Vec<String> {
    errors(format!(
        r#"
export component TestCase inherits Window {{
    {layout} {{
        Rectangle {{ {binding}; }}
    }}
}}
"#
    ))
}

fn name_of(binding: &str) -> &str {
    binding.split(':').next().unwrap()
}

#[test]
fn layout_cell_properties_are_stable() {
    for layout in CELL_LAYOUTS {
        for binding in LAYOUT_CELL_PROPERTIES {
            assert_eq!(in_layout(layout, binding), Vec::<String>::new());
        }
    }
}

/// In a GridLayout (or outside of any layout) the properties are rejected.
#[test]
fn rejected_in_grid() {
    for binding in LAYOUT_CELL_PROPERTIES {
        assert_eq!(
            in_layout("GridLayout", binding),
            [format!("{} {OUTSIDE_ERROR}", name_of(binding))]
        );
    }
}

/// A repeated or conditional cell gets an element injected around it, onto which the
/// per-item bindings are linked. Check that this doesn't report the error a second time.
#[test]
fn repeated_and_conditional_cells_report_once() {
    for cell in ["for i in 3:", "if true:"] {
        let source = format!(
            r#"
export component TestCase inherits Window {{
    GridLayout {{
        {cell} Rectangle {{ layout-order: 1; }}
    }}
}}
"#
        );
        assert_eq!(errors(source), [format!("layout-order {OUTSIDE_ERROR}")]);
    }
}

/// The `CrossAxisSelfAlignment` enum is in the stable type register:
/// nameable as a type and as a qualified value without experimental features.
#[test]
fn cross_axis_self_alignment_enum_is_stable() {
    let source = r#"
export component TestCase inherits Window {
    in property <CrossAxisSelfAlignment> a: CrossAxisSelfAlignment.center;
    FlexboxLayout {
        Rectangle { cross-axis-self-alignment: root.a; }
    }
}
"#;
    assert_eq!(errors(source.into()), Vec::<String>::new());
}
