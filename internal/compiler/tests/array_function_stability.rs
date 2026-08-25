// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The array functions `index-of`, `find-index`, `any`, and `all` require
//! `enable_experimental`, but the builtin widget library may still use them internally.
//! The syntax-test corpus and every runtime test driver force that flag on uniformly for their
//! whole corpus, so a regression here wouldn't be caught there — this test controls the flag
//! directly instead.

fn compile(
    source: &str,
    enable_experimental: bool,
) -> i_slint_compiler::diagnostics::BuildDiagnostics {
    let mut diag = i_slint_compiler::diagnostics::BuildDiagnostics::default();
    let syntax_node = i_slint_compiler::parser::parse(source.into(), None, &mut diag);
    let mut compiler_config = i_slint_compiler::CompilerConfiguration::new(
        i_slint_compiler::generator::OutputFormat::Interpreter,
    );
    compiler_config.embed_resources = i_slint_compiler::EmbedResourcesKind::OnlyBuiltinResources;
    compiler_config.enable_experimental = enable_experimental;
    compiler_config.style = Some("fluent".into());
    let (_, build_diags, _) =
        spin_on::spin_on(i_slint_compiler::compile_syntax_node(syntax_node, diag, compiler_config));
    build_diags
}

#[test]
fn array_search_functions_require_experimental_features() {
    for expression in ["model.index-of(\"B\")", "model.find-index((x) => x == \"B\")"] {
        let src = format!(
            r#"
            export component Test {{
                in property <[string]> model: ["A", "B"];
                out property <int> idx: {expression};
            }}
        "#
        );
        assert!(compile(&src, false).has_errors());
        assert!(!compile(&src, true).has_errors());
    }
}

/// The ComboBox implementation uses `index-of` internally: the widget library
/// passes the `expose_internal_types` gate, so it keeps working without
/// experimental features.
#[test]
fn combobox_works_without_experimental_features() {
    let src = r#"
        import { ComboBox } from "std-widgets.slint";
        export component Test {
            ComboBox { model: ["A", "B"]; current-value: "B"; }
        }
    "#;
    let diag = compile(src, false);
    assert!(!diag.has_errors(), "{:?}", diag.to_string_vec());
}
