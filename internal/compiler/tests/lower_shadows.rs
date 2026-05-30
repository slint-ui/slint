// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::expression_tree::Expression;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::object_tree::{ElementRc, recurse_elem};
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};
use smol_str::ToSmolStr;

fn compile(source: &str) -> i_slint_compiler::object_tree::Document {
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diagnostics);
    let compiler_config = CompilerConfiguration::new(OutputFormat::Interpreter);
    let (doc, diagnostics, _) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, compiler_config));
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());
    doc
}

fn find_box_shadow(root: &ElementRc) -> ElementRc {
    let mut result = None;
    recurse_elem(root, &(), &mut |element, _| {
        if element.borrow().base_type.to_smolstr() == "BoxShadow" {
            result = Some(element.clone());
        }
    });
    result.expect("BoxShadow element should be generated")
}

#[test]
fn box_shadow_keeps_per_corner_border_radius_bindings() {
    let doc = compile(
        r#"
export component TestCase inherits Window {
    in-out property <length> top-left-radius: 12px;
    in-out property <length> top-right-radius: 24px;
    in-out property <length> bottom-right-radius: 36px;
    in-out property <length> bottom-left-radius: 48px;
    width: 160px;
    height: 120px;

    Rectangle {
        width: 100px;
        height: 80px;
        border-radius: 0px;
        border-top-left-radius: root.top-left-radius;
        border-top-right-radius: root.top-right-radius;
        border-bottom-right-radius: root.bottom-right-radius;
        border-bottom-left-radius: root.bottom-left-radius;
        drop-shadow-blur: 8px;
        drop-shadow-color: red;
    }
}
"#,
    );

    let root = doc.exports.iter().next().unwrap().1.as_ref().left().unwrap().root_element.clone();
    let box_shadow = find_box_shadow(&root);
    let bindings = &box_shadow.borrow().bindings;

    for property_name in [
        "border-top-left-radius",
        "border-top-right-radius",
        "border-bottom-right-radius",
        "border-bottom-left-radius",
    ] {
        assert!(bindings.contains_key(property_name), "{property_name} binding missing");
        let binding = bindings.get(property_name).unwrap().borrow();
        assert!(
            matches!(&binding.expression, Expression::PropertyReference(_)),
            "{property_name} should reference the source rectangle"
        );
    }
}

#[test]
fn box_shadow_expands_uniform_border_radius_to_corner_bindings() {
    let doc = compile(
        r#"
export component TestCase inherits Window {
    in-out property <length> radius: 24px;
    width: 160px;
    height: 120px;

    Rectangle {
        width: 100px;
        height: 80px;
        border-radius: root.radius;
        drop-shadow-blur: 8px;
        drop-shadow-color: red;
    }
}
"#,
    );

    let root = doc.exports.iter().next().unwrap().1.as_ref().left().unwrap().root_element.clone();
    let box_shadow = find_box_shadow(&root);
    let bindings = &box_shadow.borrow().bindings;

    for property_name in [
        "border-top-left-radius",
        "border-top-right-radius",
        "border-bottom-right-radius",
        "border-bottom-left-radius",
    ] {
        assert!(bindings.contains_key(property_name), "{property_name} binding missing");
        let binding = bindings.get(property_name).unwrap().borrow();
        assert!(
            matches!(&binding.expression, Expression::PropertyReference(_)),
            "{property_name} should reference the source rectangle"
        );
    }
}
