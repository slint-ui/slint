// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Regression tests that drive a source file all the way through the LLR back-end,
//! the way the native (Rust/C++) code generators do. These run with `debug_info`
//! left at its default (false), which is what real `slint-build` / `slint!` users
//! get — the test drivers force `debug_info = true`, which can mask LLR lowering
//! crashes, so those cases need to be exercised here instead.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::generator::{self, OutputFormat};
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

/// Compile `source` and lower it through the LLR back-end. A panic in lowering
/// surfaces as a test failure.
fn lower_to_llr(source: &str) {
    let mut diagnostics = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), None, &mut diagnostics);
    let config = CompilerConfiguration::new(OutputFormat::Llr);
    let (doc, diagnostics, loader) =
        spin_on::spin_on(compile_syntax_node(syntax_node, diagnostics, config));
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());
    generator::generate(
        OutputFormat::Llr,
        &mut std::io::sink(),
        None,
        &doc,
        &loader.compiler_config,
    )
    .unwrap();
}

#[test]
fn custom_listview_without_viewport_height_does_not_crash() {
    // A component named `ListView` that contains a `for` repeater used to crash the LLR
    // lowering: the ListView magic was triggered purely by the base type being named
    // "ListView", so the compiler fabricated references to viewport-height/-width/-y +
    // visible-* without checking they exist. This custom ScrollView lacks `viewport-height`,
    // so it must not get the ListView treatment (which references that property), and must
    // lower cleanly.
    lower_to_llr(
        r#"
component ScrollView {
    out property <length> visible-width;
    out property <length> visible-height;
    in-out property <length> viewport-width;
    in-out property <length> viewport-x;
    in-out property <length> viewport-y;
    Rectangle { @children }
}
component ListView inherits ScrollView { }
export component TestCase inherits Window {
    width: 164px;
    height: 164px;
    ListView {
        width: 100%;
        height: 100%;
        for entry in 666: VerticalLayout {
            HorizontalLayout {
                height: 40px;
                Text { text: entry; }
            }
        }
    }
}
"#,
    );
}
