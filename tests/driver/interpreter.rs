/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
use std::error::Error;

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string(&testcase.absolute_path)?;

    let include_paths = &test_driver_lib::extract_include_paths(&source)
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();

    let component = match sixtyfps_interpreter::load(source, &testcase.absolute_path, include_paths)
    {
        Ok(c) => c,
        Err(diag) => {
            let vec = diag.to_string_vec();
            diag.print();
            return Err(vec.join("\n").into());
        }
    };

    component.create();

    Ok(())
}
