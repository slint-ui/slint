// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![allow(uncommon_codepoints)]

#[cfg(test)]
fn do_test(snippet: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let compiler = slint_interpreter::Compiler::default();
    let result = spin_on::spin_on(compiler.build_from_source(snippet.into(), path.into()));

    let diagnostics = result.diagnostics().collect::<Vec<_>>();
    slint_interpreter::print_diagnostics(&diagnostics);

    if result.has_error() {
        return Err(format!("Error when loading {snippet:?} in {path:?}: {diagnostics:?}").into());
    }
    Ok(())
}

include!(env!("TEST_FUNCTIONS"));

fn main() {
    println!("Nothing to see here, please run me through cargo test :)");
}
