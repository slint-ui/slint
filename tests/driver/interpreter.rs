/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use std::error::Error;

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string(&testcase.absolute_path)?;

    let include_paths = &test_driver_lib::extract_include_paths(&source)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    let config = sixtyfps_compilerlib::CompilerConfiguration {
        include_paths: &include_paths,
        ..Default::default()
    };

    let (component, _warnings) =
        match sixtyfps_interpreter::load(source, &testcase.absolute_path, &config) {
            (Ok(c), diagnostics) => (c, diagnostics),
            (Err(()), errors) => {
                let vec = errors.to_string_vec();
                errors.print();
                return Err(vec.join("\n").into());
            }
        };

    component.create();

    Ok(())
}
