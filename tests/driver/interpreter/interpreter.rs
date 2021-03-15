/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use itertools::Itertools;
use std::error::Error;

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string(&testcase.absolute_path)?;
    let include_paths = test_driver_lib::extract_include_paths(&source)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    let mut config =
        sixtyfps_interpreter::CompilerConfiguration::new().with_include_paths(include_paths);

    let (component, diags) =
        spin_on::spin_on(sixtyfps_interpreter::ComponentDefinition::from_source(
            source,
            testcase.absolute_path,
            config,
        ));

    let component = match component {
        None => {
            sixtyfps_interpreter::print_diagnostics(&diags);
            return Err(diags.into_iter().map(|d| d.to_string()).join("\n").into());
        }
        Some(c) => c,
    };

    component.create();

    Ok(())
}
