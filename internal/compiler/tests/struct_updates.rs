// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::{
    CompilerConfiguration, EmbedResourcesKind, ExperimentalFeature, diagnostics::BuildDiagnostics,
};

fn compile(source: &str, enable_experimental: bool) -> BuildDiagnostics {
    let mut diag = BuildDiagnostics::default();
    let mut config = CompilerConfiguration::new(OutputFormat::Interpreter);
    config.embed_resources = EmbedResourcesKind::OnlyBuiltinResources;
    config.experimental_features = if enable_experimental {
        ExperimentalFeature::StructUpdates.into()
    } else {
        Default::default()
    };
    config.style = Some("fluent".into());
    config.edition = Some(i_slint_compiler::project_file::SlintEdition::current());
    let node = i_slint_compiler::parser::parse(source.into(), None, config.edition, &mut diag);
    let (_, diag, _) =
        i_slint_compiler::poll_ready(i_slint_compiler::compile_syntax_node(node, diag, config));
    diag
}

#[test]
fn font_and_border_example_compiles() {
    let diag = compile(include_str!("../../../examples/struct-updates/demo.slint"), true);
    assert!(!diag.has_errors(), "{:?}", diag.to_string_vec());
}

#[test]
fn struct_updates_require_experimental_features() {
    for (binding, feature, enabled_error) in [
        ("appearance: { border.width: 2px, ..root.preset };", "struct updates", None),
        ("appearance: { ..root.preset };", "struct updates", None),
        (
            "appearance: { border.width: 2px };",
            "struct updates",
            Some("Dotted fields require a '..base' expression"),
        ),
        ("appearance.border.width: 2px;", "struct field overrides", None),
        ("appearance.border { width: 2px };", "struct field overrides", None),
    ] {
        let source = format!(
            r#"
            struct Border {{ width: length }}
            struct Appearance {{ border: Border }}

            component Card inherits Rectangle {{
                in property <Appearance> appearance;
                border-width: root.appearance.border.width;
            }}

            export component Test inherits Window {{
                private property <Appearance> preset: {{ border: {{ width: 1px }} }};
                Card {{ {binding} }}
            }}
            "#
        );
        let disabled = compile(&source, false);
        assert_eq!(disabled.to_string_vec().len(), 1, "{binding}: {:?}", disabled.to_string_vec(),);
        assert!(
            disabled.to_string_vec()[0]
                .contains(&format!("'{feature}' is experimental. Add 'struct-updates'")),
            "{binding}: {:?}",
            disabled.to_string_vec(),
        );

        let enabled = compile(&source, true);
        let errors: Vec<_> = enabled
            .iter()
            .filter(|diag| diag.level() == i_slint_compiler::diagnostics::DiagnosticLevel::Error)
            .map(|diag| diag.message())
            .collect();
        assert_eq!(errors.as_slice(), enabled_error.as_slice(), "{binding}");
    }
}

#[test]
fn inherited_overrides_keep_cached_imports_unchanged() {
    use i_slint_compiler::{source_path::SourcePath, typeloader::TypeLoader};

    let mut config = CompilerConfiguration::new(OutputFormat::Interpreter);
    config.experimental_features = ExperimentalFeature::StructUpdates.into();
    config.edition = Some(i_slint_compiler::project_file::SlintEdition::current());
    let mut diag = BuildDiagnostics::default();
    let mut loader = TypeLoader::new(config, &mut diag);
    let base_path = SourcePath::new(std::env::temp_dir().join("struct-update-base.slint"));
    let app_path = SourcePath::new(std::env::temp_dir().join("struct-update-app.slint"));
    i_slint_compiler::poll_ready(
        loader.load_file(
            &base_path,
            r#"
        struct Value { number: int }
        struct Data { value: Value, other: int }
        export component Base inherits Rectangle {
            in property <int> input;
            private property <int> default-value: root.input;
            in property <Data> data: { value: { number: root.default-value }, other: child.value };
            child := Rectangle { in property <int> value: root.input + 1; }
        }
        "#
            .into(),
            false,
            &mut diag,
        ),
    );
    assert!(!diag.has_errors(), "{:?}", diag.to_string_vec());
    let base = loader.get_document(&base_path).unwrap().inner_components[0].clone();
    let declarations =
        base.root_element.borrow().property_declarations.keys().cloned().collect::<Vec<_>>();
    let bindings = base
        .root_element
        .borrow()
        .real_bindings()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();

    for binding in ["data.value.number: 42;", "data.value { number: 42 };", "data.other: 43;"] {
        let mut diag = BuildDiagnostics::default();
        i_slint_compiler::poll_ready(loader.load_file(
            &app_path,
            format!(
                r#"
                import {{ Base }} from "struct-update-base.slint";
                export component App inherits Window {{ Base {{ {binding} }} }}
            "#
            ),
            false,
            &mut diag,
        ));
        assert!(!diag.has_errors(), "{:?}", diag.to_string_vec());
        assert_eq!(
            base.root_element.borrow().property_declarations.keys().cloned().collect::<Vec<_>>(),
            declarations,
            "overriding document modified the imported component",
        );
        assert_eq!(
            base.root_element
                .borrow()
                .real_bindings()
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>(),
            bindings,
        );
    }
}

#[test]
fn inherited_override_diagnostics() {
    for (binding, expected) in [
        ("missing.inner.number: 2;", "Unknown property 'missing'"),
        ("missing.inner { number: 2 };", "Unknown property 'missing'"),
        ("width.number: 2;", "Dotted overrides require a struct property: 'width'"),
        (
            "style.inner { number: 2, ..root.preset.inner };",
            "Remove '..base' from a grouped override; it always starts from the inherited value",
        ),
    ] {
        let source = format!(
            r#"
            struct Data {{ number: int }}
            struct Outer {{ inner: Data }}
            component Styled inherits Rectangle {{ in property <Outer> style; }}
            export component Example inherits Window {{
                private property <Outer> preset;
                Styled {{ {binding} }}
            }}
        "#
        );
        let diag = compile(&source, true);
        let errors = diag
            .iter()
            .filter(|diag| diag.level() == i_slint_compiler::diagnostics::DiagnosticLevel::Error)
            .map(|diag| diag.message())
            .collect::<Vec<_>>();
        assert_eq!(errors, [expected], "{binding}");
    }
}
