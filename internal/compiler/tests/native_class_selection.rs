// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The LLR lowering picks the most minimal native class that still has every property the
//! element uses, and a binding that just repeats the default value of the builtin element isn't a
//! use of it.
//!
//! The other half, keeping the default of a property that's only read through a
//! `NamedReference`, is covered by the flexbox cases in `tests/cases/layout`: their layout is
//! lowered away and then reads its `alignment` default from the element it left behind.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::llr::{CompilationUnit, LocalMemberIndex, MemberReference};
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};
use smol_str::SmolStr;

fn compile(source: &str) -> CompilationUnit {
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diagnostics);
    let compiler_config = CompilerConfiguration::new(OutputFormat::Interpreter);
    let (doc, diagnostics, _) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, compiler_config.clone()));
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());
    i_slint_compiler::llr::lower_to_item_tree::lower_to_item_tree(&doc, &compiler_config)
}

/// The compiler appends a unique number to the id, so match on the name before it.
fn is_named(name: &str, id: &str) -> bool {
    name.rsplit_once('-').is_some_and(|(name, _)| name == id)
}

fn class_of(unit: &CompilationUnit, id: &str) -> SmolStr {
    unit.sub_components
        .iter()
        .flat_map(|sc| sc.items.iter())
        .find(|item| is_named(&item.name, id))
        .unwrap_or_else(|| panic!("no item with id {id}"))
        .ty
        .class_name
        .clone()
}

fn has_line_height_factor(unit: &CompilationUnit, id: &str) -> bool {
    unit.sub_components.iter().any(|sc| {
        sc.property_init.iter().any(|(prop, _)| {
            let MemberReference::Relative { local_reference, .. } = prop else { return false };
            let LocalMemberIndex::Native { item_index, prop_name, .. } = &local_reference.reference
            else {
                return false;
            };
            local_reference.sub_component_path.is_empty()
                && prop_name == "line-height-factor"
                && is_named(&sc.items[*item_index].name, id)
        })
    })
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
