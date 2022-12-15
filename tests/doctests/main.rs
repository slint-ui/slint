// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#[cfg(test)]
fn do_test(snippet: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut compiler = slint_interpreter::ComponentCompiler::default();
    let component =
        spin_on::spin_on(compiler.build_from_source(snippet.into(), Default::default()));

    slint_interpreter::print_diagnostics(&compiler.diagnostics());

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
