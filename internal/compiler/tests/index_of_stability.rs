// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `array.index-of` must stay usable without `enable_experimental` (unlike find-index/any/all).
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
fn index_of_does_not_require_experimental_features() {
    let src = r#"
        export component Test {
            in property <[string]> model: ["A", "B"];
            out property <int> idx: model.index-of("B");
        }
    "#;
    assert!(!compile(src, false).has_errors());
}

#[test]
fn find_index_still_requires_experimental_features() {
    let src = r#"
        export component Test {
            in property <[string]> model: ["A", "B"];
            out property <int> idx: model.find-index((x) => x == "B");
        }
    "#;
    assert!(compile(src, false).has_errors());
    assert!(!compile(src, true).has_errors());
}
