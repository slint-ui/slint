/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

#[cfg(test)]
fn do_test(snippet: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config = sixtyfps_compilerlib::CompilerConfiguration::new(
        sixtyfps_compilerlib::generator::OutputFormat::Interpreter,
    );

    let (component, warnings) = match spin_on::spin_on(sixtyfps_interpreter::load(
        snippet.into(),
        Default::default(),
        config,
    )) {
        (Ok(c), diagnostics) => (c, diagnostics),
        (Err(()), errors) => {
            let vec = errors.to_string_vec();
            errors.print();
            return Err(vec.join("\n").into());
        }
    };

    warnings.print();

    component.create();

    Ok(())
}

include!(env!("TEST_FUNCTIONS"));

fn main() {
    println!("Nothing to see here, please run me through cargo test :)");
}
