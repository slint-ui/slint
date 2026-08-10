// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![no_main]

use i_slint_compiler::ComponentSelection;
use i_slint_compiler::diagnostics::BuildDiagnostics;

pub fn process_file_source(path: &std::path::Path, source: String) {
    let mut parse_diagnostics = BuildDiagnostics::default();
    let syntax_node =
        i_slint_compiler::parser::parse(source.clone(), Some(path), &mut parse_diagnostics);
    // Fuzz the native lowering pipeline. The interpreter is not usable here: it has
    // its own item-tree builder and is rejected by generator::generate(), and lowering
    // a document compiled for the interpreter would exercise a configuration (full
    // inlining + LLR back-end) that never occurs in a real compilation.
    let output_format = i_slint_compiler::generator::OutputFormat::Llr;
    let mut compiler_config = i_slint_compiler::CompilerConfiguration::new(output_format.clone());
    compiler_config.library_paths = [(
        "test-lib".into(),
        // CARGO_MANIFEST_DIR is this fuzz crate; the test library lives in the compiler crate.
        concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/typeloader/library").into(),
    )]
    .into_iter()
    .collect();
    compiler_config.embed_resources = i_slint_compiler::EmbedResourcesKind::OnlyBuiltinResources;
    compiler_config.enable_experimental = true;
    compiler_config.style = Some("fluent".into());
    compiler_config.components_to_generate =
        if source.contains("config:generate_all_exported_windows") {
            ComponentSelection::ExportedWindows
        } else {
            // Otherwise we'd have lots of warnings about not inheriting Window
            ComponentSelection::LastExported
        };
    let (doc, diagnostics, loader) = spin_on::spin_on(i_slint_compiler::compile_syntax_node(
        syntax_node,
        parse_diagnostics,
        compiler_config,
    ));

    // On a clean compile, run the back-end generator too, so the whole compiler
    // pipeline is fuzzed: parsing, the middle-end passes (run by compile_syntax_node
    // above) and the back-end lowering + optimization passes + code generation. Letting
    // generate() drive the lowering — instead of calling llr::lower_to_item_tree
    // directly — ensures we only ever exercise document shapes that a real compilation
    // for this output format actually produces. Errors are guarded out because the
    // document is then incomplete, matching what the real code generators require.
    if !diagnostics.has_errors() {
        let _ = i_slint_compiler::generator::generate(
            output_format,
            &mut std::io::sink(),
            None,
            &doc,
            &loader.compiler_config,
        );
    }
}

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let path = std::path::PathBuf::from("fuzz input");
    let source = String::from_utf8_lossy(data);
    process_file_source(&path, source.to_string());
});
