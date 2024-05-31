// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![allow(uncommon_codepoints)]

#[cfg(test)]
fn do_test(snippet: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut compiler = slint_interpreter::ComponentCompiler::default();
    let component = spin_on::spin_on(compiler.build_from_source(snippet.into(), path.into()));

    slint_interpreter::print_diagnostics(compiler.diagnostics());

    for d in compiler.diagnostics() {
        if d.level() == slint_interpreter::DiagnosticLevel::Error {
            return Err(d.message().to_owned().into());
        }
    }

    if component.is_none() {
        Err(String::from("Failure").into())
    } else {
        Ok(())
    }
}

include!(env!("TEST_FUNCTIONS"));

fn main() {
    println!("Nothing to see here, please run me through cargo test :)");
}
