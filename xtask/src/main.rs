/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use std::error::Error;
use std::path::PathBuf;
use structopt::StructOpt;

mod cmake;
mod license_headers_check;

#[derive(Debug, StructOpt)]
pub enum TaskCommand {
    #[structopt(name = "cmake")]
    CMake(cmake::CMakeCommand),
    #[structopt(name = "check_license_headers")]
    CheckLicenseHeaders(license_headers_check::LicenseHeaderCheck),
}

#[derive(Debug, StructOpt)]
#[structopt(name = "xtask")]
pub struct ApplicationArguments {
    #[structopt(subcommand)]
    pub command: TaskCommand,
}

pub fn root_dir() -> anyhow::Result<PathBuf> {
    let mut root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| anyhow::anyhow!("Cannot determine root directory - CARGO_MANIFEST_DIR is not set -- you can only run xtask via cargo"))?);
    root.pop(); // $root/xtask -> $root
    Ok(root)
}

fn main() -> Result<(), Box<dyn Error>> {
    match ApplicationArguments::from_args().command {
        TaskCommand::CMake(cmd) => cmd.build_cmake()?,
        TaskCommand::CheckLicenseHeaders(cmd) => cmd.check_license_headers()?,
    };

    Ok(())
}
