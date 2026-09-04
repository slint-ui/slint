// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A layout constraint that reads back what the layout is computing from it
//! makes the generated code panic with "Recursion detected" at startup.
//!
//! The cycle is an ordinary binding loop, so `binding_analysis` finds it once
//! the dependencies it records match what lowering emits: a cell measured with
//! a constraint is read through `layoutinfo-<o>-with-constraint`, and the plain
//! `layoutinfo-<o>` reads the perpendicular size that the parametrized function
//! takes as a parameter.
//!
//! A loop closing through the window layout property is only a warning: the
//! runtime resolves it by solving one axis after the other. The loops below are
//! errors because they close without it, on the layout's own properties.
//!
//! The accepted cases matter as much as the rejected ones: they are the shapes
//! a check that decides this too early rejects.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

/// Every error reported for `source`, as `(message, has a source location)`.
/// The deprecation warnings are left out: they are what the compiler already
/// reported for these shapes.
fn errors(source: &str) -> Vec<(String, bool)> {
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diagnostics);
    let config = CompilerConfiguration::new(OutputFormat::Llr);
    let (_doc, diagnostics, _loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, config));
    diagnostics
        .iter()
        .filter(|d| d.level() == i_slint_compiler::diagnostics::DiagnosticLevel::Error)
        .map(|d| (d.message().to_string(), d.source_file().is_some()))
        .collect()
}

/// The loop a message names, as it is printed between the parentheses. Pinning
/// this is what catches a walk that reports a different cycle between runs:
/// asserting only that *some* error was produced cannot see that at all.
fn reported_chain(message: &str) -> &str {
    message
        .split_once('(')
        .and_then(|(_, rest)| rest.rsplit_once(')'))
        .map_or("", |(chain, _)| chain)
}

/// Returns the loop that was reported, so a caller can pin it.
fn assert_rejected(name: &str, source: &str) -> String {
    let errors = errors(source);
    for (message, _) in &errors {
        assert!(
            message.contains("is part of a binding loop"),
            "{name} was rejected for the wrong reason: {message}"
        );
    }
    assert!(!errors.is_empty(), "{name} should be rejected");
    // One per binding in the cycle, and they all have to name the same cycle:
    // two different chains mean the walk found the loop twice by two routes.
    let chain = reported_chain(&errors[0].0).to_string();
    for (message, _) in &errors {
        assert_eq!(reported_chain(message), chain, "{name} reported two different loops");
    }
    // A loop the reader cannot locate is barely a diagnostic.
    assert!(errors.iter().any(|(_, has_span)| *has_span), "{name} was reported without a location");
    let (first, last) = (
        chain.split(" -> ").next().unwrap_or_default(),
        chain.split(" -> ").last().unwrap_or_default(),
    );
    assert!(!chain.is_empty() && first == last, "{name} did not report a closed loop: {chain}");
    chain
}

/// Every error, not just the loops: a case that stops compiling for an
/// unrelated reason silently stops testing what it is here for.
fn assert_accepted(name: &str, source: &str) {
    let errors = errors(source);
    assert!(errors.is_empty(), "{name} should compile, but was rejected: {errors:?}");
}

/// Compiling the same source twice must name the same loop. The walk starts
/// from a hash map's keys if nobody keeps it in order, and then the message,
/// the span and the error count all change from run to run.
fn assert_same_loop_each_run(name: &str, source: &str) -> String {
    let first = assert_rejected(name, source);
    for _ in 0..4 {
        assert_eq!(assert_rejected(name, source), first, "{name} reported a different loop");
    }
    first
}

/// The enclosing layouts of every case below: a sidebar makes the row layout
/// solve a width, and the conditional makes the column layout solve rather than
/// hand its single cell the constraint unchanged.
fn wrap(inner: &str, extra_root: &str) -> String {
    format!(
        r#"
export component Main inherits Window {{
    in property <bool> horizontal: true;
    {extra_root}
    HorizontalLayout {{
        Rectangle {{ width: 40px; }}
        VerticalLayout {{
            if false: Rectangle {{ }}
            {inner}
        }}
    }}
}}"#
    )
}

const RUNTIME_FLEX: &str = "flex-direction: horizontal ? FlexboxLayoutDirection.row : FlexboxLayoutDirection.column; flex-wrap: wrap;";

#[test]
fn constraint_reading_own_preferred_size() {
    // slint-ui/slint#13059 as reported.
    let chain = assert_same_loop_each_run(
        "self constraint",
        &wrap(
            &format!("fl := FlexboxLayout {{ min-height: fl.preferred-height; {RUNTIME_FLEX} }}"),
            "",
        ),
    );
    // Pinned: a walk that reports some *other* loop for this source, stably,
    // passes every assertion above.
    assert_eq!(
        chain,
        "root.layoutinfo-h -> width -> layout-cache -> width -> fl.width -> fl.layoutinfo-v \
         -> fl.preferred-height -> fl.min-height -> layoutinfo-h-with-constraint \
         -> layoutinfo-h -> root.layoutinfo-h"
    );
}

#[test]
fn constraint_on_a_component_that_is_a_flexbox() {
    // The constraint is set on the instance, so the layout info it reads back
    // lives in the sub-component.
    assert_same_loop_each_run(
        "component instance",
        &format!(
            r#"
component Inner inherits FlexboxLayout {{
    in property <bool> horizontal: true;
    {RUNTIME_FLEX}
    Rectangle {{ min-width: 60px; min-height: 20px; }}
}}
{}"#,
            wrap("i := Inner { min-height: i.preferred-height; }", "")
        ),
    );
}

#[test]
fn constraint_reading_own_width() {
    assert_same_loop_each_run(
        "own width",
        &wrap(&format!("fl := FlexboxLayout {{ min-height: self.width / 4; {RUNTIME_FLEX} }}"), ""),
    );
}

#[test]
fn constraint_reading_a_sibling() {
    assert_same_loop_each_run(
        "sibling constraint",
        &wrap(
            &format!(
                "fl := FlexboxLayout {{ {RUNTIME_FLEX} Rectangle {{ min-width: 60px; min-height: 20px; }} }}
             fl2 := FlexboxLayout {{ min-height: fl.preferred-height; {RUNTIME_FLEX} Rectangle {{ min-width: 60px; min-height: 20px; }} }}"
            ),
            "",
        ),
    );
}

#[test]
fn constraint_reading_through_a_property() {
    assert_same_loop_each_run(
        "computed property",
        &wrap(
            &format!("fl := FlexboxLayout {{ min-height: root.extra; {RUNTIME_FLEX} }}"),
            "property <length> extra: fl.preferred-height + 1px;",
        ),
    );
}

#[test]
fn constraint_reading_through_a_function() {
    // Nothing here matches on the shape of the expression, so an indirection
    // the reader invents makes no difference.
    assert_same_loop_each_run(
        "function call",
        &wrap(
            &format!("fl := FlexboxLayout {{ min-height: root.f(); {RUNTIME_FLEX} }}"),
            "pure function f() -> length { return fl.preferred-height; }",
        ),
    );
}

#[test]
fn row_direction_is_not_measured_from_its_height() {
    assert_accepted(
        "static row",
        &wrap(
            "fl := FlexboxLayout { min-height: self.width / 4; flex-direction: FlexboxLayoutDirection.row; flex-wrap: wrap; }",
            "",
        ),
    );
}

#[test]
fn runtime_direction_without_a_self_constraint() {
    assert_accepted(
        "no self constraint",
        &wrap(
            &format!(
                "fl := FlexboxLayout {{ {RUNTIME_FLEX} Text {{ text: \"wraps\"; wrap: word-wrap; }} }}"
            ),
            "",
        ),
    );
}

#[test]
fn component_measured_without_a_constraint() {
    // A component carrying `layoutinfo-v-with-constraint` (it wraps a
    // height-for-width Text) as a plain cell of a box layout: the cell is not
    // measured there, so the layout reads its plain `layoutinfo-v`.
    assert_accepted(
        "unmeasured component cell",
        r#"
component Card inherits Rectangle {
    VerticalLayout {
        Text { text: "a text that wraps"; wrap: word-wrap; }
    }
}
export component Main inherits Window {
    VerticalLayout {
        Card { }
        Rectangle { }
    }
}"#,
    );
}

#[test]
fn aspect_ratio_on_a_plain_element() {
    assert_accepted(
        "aspect ratio",
        r#"
export component Main inherits Window {
    HorizontalLayout {
        Rectangle {
            min-width: self.height / 2;
            Text { text: "wraps"; wrap: word-wrap; }
        }
    }
}"#,
    );
}

#[test]
fn aspect_ratio_around_a_runtime_direction_flexbox() {
    assert_accepted(
        "aspect ratio around a flex",
        r#"
export component Main inherits Window {
    in property <bool> horizontal: true;
    VerticalLayout {
        Rectangle {
            preferred-height: self.width / 2;
            FlexboxLayout {
                flex-direction: horizontal ? FlexboxLayoutDirection.row : FlexboxLayoutDirection.column;
                Rectangle { min-width: 10px; min-height: 10px; }
            }
        }
    }
}"#,
    );
}
