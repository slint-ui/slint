// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Regression test for https://github.com/slint-ui/slint/issues/417.
//!
//! The sibling `.slint` test in `tests/syntax/analysis/` only inlines `SubCompWithAlias`,
//! which merges the alias and the override onto one `ElementRc` and hides the bug. Using
//! `OutputFormat::Llr` here instead keeps `SubCompWithAlias` as a separate component,
//! so the alias and the conflicting override end up on different `ElementRc`s, which is
//! the case `find_alias_targets` needs to handle.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

#[test]
fn alias_declared_in_non_inlined_base_component_is_detected_as_a_loop() {
    let source = r#"
component SubCompWithAlias inherits Rectangle {
    in-out property <string> bar: "Hello";
    in-out property <string> foo <=> bar;
}

export component Test inherits Window {
    a := SubCompWithAlias {
        foo: self.bar;
    }
    Text { text: a.bar; }
}
"#;

    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diagnostics);
    let config = CompilerConfiguration::new(OutputFormat::Llr);
    let (_doc, diagnostics, _loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, config));

    assert!(
        diagnostics.has_errors(),
        "expected a binding-loop diagnostic for `foo: self.bar;` (foo <=> bar), \
         but the compiler accepted the program with no errors: {:?}.",
        diagnostics.to_string_vec()
    );
}
