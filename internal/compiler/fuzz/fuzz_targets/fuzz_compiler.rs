// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![no_main]
use libfuzzer_sys::fuzz_target;

use i_slint_compiler::diagnostics::{BuildDiagnostics, SourceFile};
use i_slint_compiler::parser::{parse_tokens, Token};

fuzz_target!(|tokens: Vec<Token>| {
    let source_file = SourceFile::default();
    let mut diags = BuildDiagnostics::default();

    let doc_node = parse_tokens(tokens, source_file, &mut diags);

    let (_, _) = spin_on::spin_on(i_slint_compiler::compile_syntax_node(
        doc_node,
        diags,
        i_slint_compiler::CompilerConfiguration::new(
            i_slint_compiler::generator::OutputFormat::Llr,
        ),
    ));
});
