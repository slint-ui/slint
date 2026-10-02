// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::langtype::ElementType;
use i_slint_compiler::object_tree::recurse_elem_including_sub_components;
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

/// Returns how many `Something` elements and conditional elements are left,
/// the same with and without inlining.
fn compile(const_operating_system: Option<&str>) -> (usize, usize) {
    let inlined = compile_with(const_operating_system, true);
    assert_eq!(inlined, compile_with(const_operating_system, false));
    inlined
}

fn compile_with(const_operating_system: Option<&str>, inline_all_elements: bool) -> (usize, usize) {
    let source = r#"
        global Global {
            out property <bool> is-mobile: Platform.os == OperatingSystemType.android
                || Platform.os == OperatingSystemType.ios;
        }
        component Something inherits Rectangle {
            background: red;
            Text { text: "mobile"; }
        }
        export component TestCase {
            if Global.is-mobile: Something {}
        }
    "#;
    let mut config = CompilerConfiguration::new(OutputFormat::Interpreter);
    config.const_operating_system = const_operating_system.map(Into::into);
    config.inline_all_elements = inline_all_elements;
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diagnostics);
    let (doc, diagnostics, _loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, config));
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());

    let component = doc.inner_components.last().unwrap();
    let (mut something, mut conditionals) = (0, 0);
    recurse_elem_including_sub_components(component, &(), &mut |e, _| {
        let e = e.borrow();
        conditionals += e.repeated.is_some() as usize;
        // Without inlining, `Something` stays a sub-component.
        let is_something = matches!(&e.base_type, ElementType::Component(c) if c.id == "Something");
        let is_inlined_text = e.binding("text").is_some_and(|b| {
            matches!(b.expression.ignore_debug_hooks(), Expression::StringLiteral(s) if s == "mobile")
        });
        something += (is_something || is_inlined_text) as usize;
    });
    (something, conditionals)
}

#[test]
fn platform_os_mobile() {
    // `None` detects the operating system at run time, so `Something` is kept.
    for os in [Some("android"), Some("ios"), None] {
        let (something, conditionals) = compile(os);
        assert!(something > 0, "{os:?}: Something is missing");
        assert_eq!(conditionals, 1, "{os:?}");
    }
}

#[test]
fn platform_os_not_mobile() {
    for os in ["linux", "macos", "windows", "other"] {
        assert_eq!(compile(Some(os)), (0, 0), "{os}");
    }
}
