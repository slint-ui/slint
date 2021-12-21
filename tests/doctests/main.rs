// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#[cfg(test)]
fn do_test(snippet: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut compiler = sixtyfps_interpreter::ComponentCompiler::default();
    let component =
        spin_on::spin_on(compiler.build_from_source(snippet.into(), Default::default()));

    #[cfg(feature = "display-diagnostics")]
    sixtyfps_interpreter::print_diagnostics(&compiler.diagnostics());

    for d in compiler.diagnostics() {
        if d.level() == sixtyfps_interpreter::DiagnosticLevel::Error {
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
