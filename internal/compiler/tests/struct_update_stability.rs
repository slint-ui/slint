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
    for (binding, feature) in [
        ("appearance: { border.width: 2px, ..root.preset };", "struct updates"),
        ("appearance: { ..root.preset };", "struct updates"),
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
        assert!(!enabled.has_errors(), "{binding}: {:?}", enabled.to_string_vec());
    }
}
