// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `@testable` is an experimental feature (see `docs/astro/.../experimental/testable.mdx`).
//! The syntax tests can't verify the gate because they always enable experimental features,
//! so this test compiles with them disabled. Mirrors `layout_cell_properties.rs`.

use i_slint_compiler::diagnostics::{BuildDiagnostics, DiagnosticLevel};
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

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

#[test]
fn testable_is_rejected_without_experimental_features() {
    let source = r#"
export component TestCase inherits Window {
    @testable in-out property <int> a;
}
"#;
    assert_eq!(errors(source.into()), ["'@testable' is an experimental feature"]);
}
