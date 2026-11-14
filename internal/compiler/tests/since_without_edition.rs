// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A file without an edition warns about the builtins added after Slint 1.18.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::source_path::SourcePath;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

#[test]
fn builtins_newer_than_1_18_warn() {
    let source = r#"
        export component Test {
            property <CapitalizationMode> mode;
            property <ScrollBehavior> behavior;
        }
    "#;
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node =
        parse(source.into(), Some(SourcePath::new("since.slint")), None, &mut diagnostics);
    let compiler_config = CompilerConfiguration::new(OutputFormat::Interpreter);
    let (_, diagnostics, _) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, compiler_config));
    let messages = diagnostics.to_string_vec();
    assert!(
        messages.iter().any(|m| m.contains(
            "The type 'ScrollBehavior' is new in Slint 1.19, but no edition is set: set it to 1.19"
        )),
        "{messages:?}"
    );
    assert!(!messages.iter().any(|m| m.contains("CapitalizationMode")), "{messages:?}");
}
