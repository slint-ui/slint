// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `resolve_native_classes` picks the most minimal native class that still has every property the
//! element uses, and a binding that just repeats the default value from `builtins.slint` isn't a
//! use of it.
//!
//! The other half of the pass, keeping the default of a property that's only read through a
//! `NamedReference`, is covered by the flexbox cases in `tests/cases/layout`: their layout is
//! lowered away and then reads its `alignment` default from the element it left behind.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::object_tree::{ElementRc, recurse_elem};
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};
use smol_str::{SmolStr, ToSmolStr};

fn compile(source: &str) -> ElementRc {
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diagnostics);
    let compiler_config = CompilerConfiguration::new(OutputFormat::Interpreter);
    let (doc, diagnostics, _) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, compiler_config));
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());
    doc.last_exported_component().unwrap().root_element.clone()
}

/// The compiler appends a unique number to the id, so match on the name before it.
fn find_by_id(root: &ElementRc, id: &str) -> ElementRc {
    let mut result = None;
    recurse_elem(root, &(), &mut |element, _| {
        if element.borrow().id.rsplit_once('-').is_some_and(|(name, _)| name == id) {
            result = Some(element.clone());
        }
    });
    result.unwrap_or_else(|| panic!("no element with id {id}"))
}

fn class_of(root: &ElementRc, id: &str) -> SmolStr {
    find_by_id(root, id).borrow().base_type.to_smolstr()
}

fn has_line_height_factor(root: &ElementRc, id: &str) -> bool {
    find_by_id(root, id).borrow().is_binding_set("line-height-factor", false)
}

#[test]
fn text_without_complex_properties_is_a_simple_text() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    plain := Text { text: "hello"; }
}
"#,
    );
    assert_eq!(class_of(&root, "plain"), "SimpleText");
    assert!(!has_line_height_factor(&root, "plain"));
}

#[test]
fn complex_text_keeps_the_default_line_height_factor() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    fancy := Text { text: "hello"; font-family: "Arial"; }
}
"#,
    );
    assert_eq!(class_of(&root, "fancy"), "ComplexText");
    // Without the binding the item would keep its zero default and collapse the lines.
    assert!(has_line_height_factor(&root, "fancy"));
}

#[test]
fn setting_the_line_height_factor_to_its_default_is_still_a_simple_text() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    plain := Text { text: "hello"; line-height-factor: 1; }
}
"#,
    );
    assert_eq!(class_of(&root, "plain"), "SimpleText");
}

#[test]
fn a_computed_line_height_factor_selects_complex_text() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    in property <float> factor: 1;
    computed := Text { text: "hello"; line-height-factor: root.factor; }
}
"#,
    );
    assert_eq!(class_of(&root, "computed"), "ComplexText");
}

#[test]
fn setting_the_line_height_factor_selects_complex_text() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    spaced := Text { text: "hello"; line-height-factor: 1.5; }
}
"#,
    );
    assert_eq!(class_of(&root, "spaced"), "ComplexText");
}

#[test]
fn reading_the_line_height_factor_selects_complex_text() {
    let root = compile(
        r#"
export component TestCase inherits Window {
    source := Text { text: "hello"; }
    reader := Text { text: source.line-height-factor; }
}
"#,
    );
    assert_eq!(class_of(&root, "source"), "ComplexText");
    assert_eq!(class_of(&root, "reader"), "SimpleText");
}
