// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `is_property_set_in_source` answers "does the component decide this value itself?", so that a
//! tool like `slint-viewer` can set `Window.title` without stealing it. Every way the source has of
//! deciding a value has to say yes, and the `builtins.slint` default has to say no.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

/// `Window.title` has a default in `builtins.slint`, which is what makes it the interesting case.
fn title_set_in_source(source: &str) -> bool {
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diagnostics);
    let compiler_config = CompilerConfiguration::new(OutputFormat::Interpreter);
    let (doc, diagnostics, _) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, compiler_config));
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());
    let root = doc.last_exported_component().unwrap().root_element.clone();
    root.borrow().is_property_set_in_source("title")
}

#[test]
fn the_builtin_default_is_not_set_in_source() {
    assert!(!title_set_in_source(r#"export component T inherits Window { }"#));
}

#[test]
fn an_untouched_title_survives_a_component_with_widgets_and_callbacks() {
    assert!(!title_set_in_source(
        r#"import { Button, VerticalBox, LineEdit } from "std-widgets.slint";
           export component T inherits Window {
               in-out property <string> name: "x";
               callback go();
               VerticalBox {
                   LineEdit { text <=> root.name; }
                   Button { text: "Go"; clicked => { root.go(); root.name = "y"; } }
               }
           }"#
    ));
}

#[test]
fn a_binding_is_set_in_source() {
    assert!(title_set_in_source(r#"export component T inherits Window { title: "hello"; }"#));
    // An empty title is a choice too, not an absence.
    assert!(title_set_in_source(r#"export component T inherits Window { title: ""; }"#));
    assert!(title_set_in_source(
        r#"export component T inherits Window {
               in-out property <string> n: "x";
               title: "Editing " + root.n;
           }"#
    ));
}

#[test]
fn a_binding_in_a_base_is_set_in_source() {
    // An exported root's base is always inlined, so this lands on the root element itself.
    assert!(title_set_in_source(
        r#"component Base inherits Window { title: "hi"; }
           export component T inherits Base { }"#
    ));
}

#[test]
fn an_alias_to_a_global_is_set_in_source() {
    // The alias leaves `title` with a two-way stub and no analysis entry, but writing to it would
    // write into the global the component reads elsewhere.
    assert!(title_set_in_source(
        r#"global G { in-out property <string> t: "MyApp"; }
           export component T inherits Window { title <=> G.t; Text { text: G.t; } }"#
    ));
    assert!(title_set_in_source(
        r#"global G { in-out property <string> t: "MyApp"; }
           component Base inherits Window { in-out property <string> bt <=> G.t; title <=> root.bt; }
           export component T inherits Base { }"#
    ));
}

#[test]
fn a_state_is_set_in_source() {
    // The state replaces the `builtins.slint` default in place, so the binding it leaves behind
    // must not keep that default's priority.
    assert!(title_set_in_source(
        r#"export component T inherits Window {
               in-out property <bool> alt;
               states [ alternate when root.alt: { title: "alternate"; } ]
           }"#
    ));
}

#[test]
fn an_assignment_is_set_in_source() {
    assert!(title_set_in_source(
        r#"export component T inherits Window { init => { root.title = "from init"; } }"#
    ));
    assert!(title_set_in_source(
        r#"export component T inherits Window {
               callback go();
               go => { root.title = "from a callback"; }
           }"#
    ));
}

#[test]
fn a_two_way_binding_is_set_in_source() {
    assert!(title_set_in_source(
        r#"export component T inherits Window {
               in-out property <string> t: "x";
               title <=> root.t;
           }"#
    ));
    // The alias is declared the other way around: `title` itself carries no binding, but setting it
    // would overwrite the component's own property.
    assert!(title_set_in_source(
        r#"export component T inherits Window { in-out property <string> t <=> root.title; }"#
    ));
}
