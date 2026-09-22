// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A binding loop running through a widget the user only instantiates is reported on the
//! binding they wrote, and the widget's own bindings become notes.
//!
//! The syntax tests can't pin this: a widget's bindings live in a `builtin:` file, which they
//! have no way to reference, and here two of them come before the user's in the cycle.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

/// `IconButton` takes its width from its own height, which the TabWidget decides from the
/// window's height, which the wrapping text makes depend on the window's width, which the
/// TabWidget's minimum width — from that same `IconButton` — decides.
const SOURCE: &str = r#"
import { TabWidget, HorizontalBox } from "std-widgets.slint";

component IconButton {
    width: self.height;
    min-height: 32px;
}

export component Main inherits Window {
    VerticalLayout {
        Text { text: "a long header that wraps"; wrap: word-wrap; }
        HorizontalLayout {
            TabWidget {
                Tab {
                    title: "a";
                    HorizontalBox { IconButton { } }
                }
            }
        }
    }
}
"#;

fn compile(source: &str) -> Vec<String> {
    let mut diag = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), Some(std::path::Path::new("main.slint")), &mut diag);
    let mut config = CompilerConfiguration::new(OutputFormat::Llr);
    config.style = Some("fluent".into());
    let (_doc, diag, _loader) = spin_on::spin_on(compile_syntax_node(syntax_node, diag, config));
    diag.to_string_vec()
}

#[test]
fn reported_on_the_binding_the_user_wrote() {
    let diagnostics = compile(SOURCE);

    let reports: Vec<_> =
        diagnostics.iter().filter(|d| d.contains("is part of a binding loop")).collect();
    assert_eq!(reports.len(), 1, "{diagnostics:?}");
    assert!(reports[0].contains("property 'width'"), "{reports:?}");
    assert!(
        !reports[0].starts_with("builtin:"),
        "reported on a widget, not on our own: {reports:?}"
    );

    let notes: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.contains("is part of the binding loop reported for"))
        .collect();
    assert!(!notes.is_empty(), "{diagnostics:?}");
    assert!(notes.iter().all(|n| n.starts_with("builtin:")), "{notes:?}");
}

/// The ScrollView's content height comes from the Rectangle it holds, which fills that content
/// (#12333). The `content-height` binding the source wrote sits behind the ScrollView's alias,
/// so no binding of the cycle is one the source wrote. "main.slint" sorts after "builtin:".
const SCROLL_VIEW_SOURCE: &str = r#"
import { ScrollView } from "std-widgets.slint";

export component Main inherits Window {
    ScrollView {
        content-height: cell.height;
        cell := Rectangle { }
    }
}
"#;

#[test]
fn of_equal_blame_reported_in_the_user_file() {
    let diagnostics = compile(SCROLL_VIEW_SOURCE);

    let reports: Vec<_> =
        diagnostics.iter().filter(|d| d.contains("is part of a binding loop")).collect();
    assert_eq!(reports.len(), 1, "{diagnostics:?}");
    assert!(reports[0].starts_with("main.slint:"), "{reports:?}");
    assert!(reports[0].contains("property 'height'"), "{reports:?}");
}

/// A GroupBox holds its content in a layout of its own, so the child measured inside the cycle
/// is an element of the widget's source. The escape names `outer` instead, since a `builtin:`
/// file is not one the user can put an `x` in.
const GROUP_BOX_SOURCE: &str = r#"
import { GroupBox } from "std-widgets.slint";

export component Main inherits Window {
    outer := VerticalLayout {
        GroupBox {
            VerticalLayout { Text { text: "a"; font-size: root.width / 10; } }
        }
    }
}
"#;

#[test]
fn escape_never_names_an_element_of_a_widget() {
    let diagnostics = compile(GROUP_BOX_SOURCE);

    let advice: Vec<_> =
        diagnostics.iter().filter(|d| d.contains("set an 'x' or a 'y' on")).collect();
    assert_eq!(advice.len(), 1, "{diagnostics:?}");
    assert!(advice[0].contains("set an 'x' or a 'y' on 'outer'"), "{advice:?}");

    let notes: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.contains("takes 'outer' out of its parent's size"))
        .collect();
    assert_eq!(notes.len(), 1, "{diagnostics:?}");
    assert!(notes[0].starts_with("main.slint:"), "{notes:?}");
}
