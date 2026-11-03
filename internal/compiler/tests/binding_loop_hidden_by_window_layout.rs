// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A binding loop through the window layout is only a warning.
//! Before edition 1.19, an error loop whose bindings are all part of such a warning
//! loop stays unreported.
//! A `.slint` syntax test can't see that case: those always use the current version.

use i_slint_compiler::diagnostics::{BuildDiagnostics, DiagnosticLevel};
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::project_file::SlintEdition;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

/// The window layout's loop runs through the outer `Rectangle`; the inner loop doesn't.
const SOURCE: &str = r#"
export component TestCase inherits Window {
    HorizontalLayout {
        Rectangle {
            VerticalLayout {
                Rectangle { width: parent.width; }
            }
        }
    }
}
"#;

fn compile(edition: Option<SlintEdition>) -> BuildDiagnostics {
    let mut diag = BuildDiagnostics::default();
    let syntax_node = parse(SOURCE.into(), None, &mut diag);
    let mut config = CompilerConfiguration::new(OutputFormat::Llr);
    config.edition = edition;
    let (_doc, diag, _loader) = spin_on::spin_on(compile_syntax_node(syntax_node, diag, config));
    diag
}

fn binding_loops(diag: &BuildDiagnostics) -> Vec<String> {
    diag.to_string_vec().into_iter().filter(|d| d.contains("is part of a binding loop")).collect()
}

#[test]
fn hidden_loop_is_an_error_from_1_19() {
    let diag = compile(Some(SlintEdition::V1_19));
    let loops = binding_loops(&diag);
    assert_eq!(loops.len(), 2, "{loops:?}");
    let errors = diag
        .iter()
        .filter(|d| d.level() == DiagnosticLevel::Error)
        .map(|d| d.message())
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("(Rectangle.width -> VerticalLayout.width"), "{errors:?}");
}

#[test]
fn hidden_loop_is_not_reported_before_1_19() {
    for edition in [None, Some("1.18".parse().unwrap())] {
        let diag = compile(edition);
        assert!(!diag.has_errors(), "{edition:?}: {:?}", binding_loops(&diag));
        assert_eq!(binding_loops(&diag).len(), 1, "{edition:?}: {:?}", binding_loops(&diag));
    }
}
