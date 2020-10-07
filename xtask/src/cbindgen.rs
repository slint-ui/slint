/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use std::{error::Error, path::Path};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct CbindgenCommand {
    #[structopt(long)]
    output_dir: String,
}

impl CbindgenCommand {
    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        Ok(test_driver_lib::cbindgen::gen_all(Path::new(&self.output_dir))?)
    }
}
