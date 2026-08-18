// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::parse;
use i_slint_compiler::rust_interface::{RustInterfaceDescriptor, RustInterfaceEntry};
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

fn descriptor(source: &str) -> RustInterfaceDescriptor {
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diagnostics);
    let config = CompilerConfiguration::new(OutputFormat::Rust);
    let (document, diagnostics, loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, config));
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());
    let unit = i_slint_compiler::llr::lower_to_item_tree::lower_to_item_tree(
        &document,
        &loader.compiler_config,
    );
    RustInterfaceDescriptor::from_document(&document, &unit)
}

fn base_source(body: &str) -> String {
    format!(
        r#"
struct Child {{ value: int }}
struct Payload {{ child: Child, label: string }}
export enum Mode {{ first, second }}
export global Settings {{
    in-out property <int> count;
    callback accepted(Payload) -> bool;
}}
component App inherits Window {{
    in property <string> caption;
    out property <int> result;
    callback submit(Payload) -> Mode;
    public function reset(value: int) -> bool {{ return value > 0; }}
    {body}
}}
export {{ App as RenamedApp, Payload as Data }}
"#
    )
}

#[test]
fn descriptor_covers_the_generated_rust_interface() {
    let descriptor = descriptor(&base_source("Rectangle {}"));
    let entries = descriptor
        .entries()
        .iter()
        .map(|entry| (entry.path(), entry.signature()))
        .collect::<std::collections::BTreeMap<_, _>>();

    assert_eq!(entries["component App"], "window");
    assert_eq!(entries["component App/property caption"], "in string");
    assert_eq!(entries["component App/property result"], "out int");
    assert_eq!(entries["component App/callback submit"], "(struct Payload) -> enum Mode");
    assert_eq!(entries["component App/function reset"], "(int) -> bool");
    assert_eq!(entries["global Settings/property count"], "in-out int");
    assert_eq!(entries["struct Payload/field child"], "struct Child");
    assert_eq!(entries["enum Mode/variant first"], "index 0, default");
    assert_eq!(entries["named export RenamedApp"], "component App");
    assert_eq!(entries["named export Data"], "struct Payload");
}

#[test]
fn canonical_entries_are_order_independent() {
    let first = RustInterfaceDescriptor::from_entries([
        RustInterfaceEntry::new("struct Payload/field label", "string"),
        RustInterfaceEntry::new("struct Payload", "struct"),
    ]);
    let second = RustInterfaceDescriptor::from_entries([
        RustInterfaceEntry::new("struct Payload", "struct"),
        RustInterfaceEntry::new("struct Payload/field label", "string"),
    ]);

    assert_eq!(first, second);
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(first.entries()[0].path(), "struct Payload");
}

#[test]
fn implementation_edits_keep_the_same_fingerprint() {
    let before = descriptor(&base_source("Rectangle { background: red; }"));
    let after =
        descriptor(&base_source("VerticalLayout { spacing: 12px; Text { text: root.caption; } }"));

    assert_eq!(before.fingerprint(), after.fingerprint());
    assert!(before.diff(&after).is_empty());
}

#[test]
fn reports_renames_type_changes_and_direction_changes() {
    let before = descriptor(&base_source("Rectangle {}"));
    let after = descriptor(
        &base_source("Rectangle {}")
            .replace("property <string> caption", "property <int> heading")
            .replace("out property <int> result", "in-out property <int> result"),
    );
    let difference = before.diff(&after).to_string();

    assert_ne!(before.fingerprint(), after.fingerprint());
    assert!(difference.contains("- component App/property caption: in string"));
    assert!(difference.contains("+ component App/property heading: in int"));
    assert!(difference.contains("~ component App/property result: out int -> in-out int"));
}

#[test]
fn reports_nested_exported_type_changes() {
    let before = descriptor(&base_source("Rectangle {}"));
    let after = descriptor(
        &base_source("Rectangle {}")
            .replace("struct Child { value: int }", "struct Child { value: string }"),
    );
    let difference = before.diff(&after).to_string();

    assert_ne!(before.fingerprint(), after.fingerprint());
    assert!(difference.contains("~ struct Child/field value: int -> string"));
}
