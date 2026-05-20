// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::Path;

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::parser;

const INLINE_INPUT_PATH: &str = "<input>.slint";

pub(super) fn compiler_diagnostics_for_broken_input(
    source: &str,
    path: Option<&Path>,
) -> Option<String> {
    let diagnostics = parse_source(source, path);
    diagnostics.has_errors().then(|| diagnostics.diagnostics_as_string())
}

fn parse_source(source: &str, path: Option<&Path>) -> BuildDiagnostics {
    let mut diagnostics = BuildDiagnostics::default();
    let source_path = path.or_else(|| Some(Path::new(INLINE_INPUT_PATH)));
    parser::parse(source.to_owned(), source_path, &mut diagnostics);
    diagnostics
}
