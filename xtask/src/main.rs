/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use anyhow::Context;
use std::error::Error;
use std::path::PathBuf;
use structopt::StructOpt;

mod cbindgen;
mod cppdocs;
mod license_headers_check;

#[derive(Debug, StructOpt)]
pub enum TaskCommand {
    #[structopt(name = "check_license_headers")]
    CheckLicenseHeaders(license_headers_check::LicenseHeaderCheck),
    #[structopt(name = "cppdocs")]
    CppDocs,
    #[structopt(name = "cbindgen")]
    Cbindgen(cbindgen::CbindgenCommand),
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

fn run_command<I, K, V>(program: &str, args: &[&str], env: I) -> anyhow::Result<Vec<u8>>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<std::ffi::OsStr>,
    V: AsRef<std::ffi::OsStr>,
{
    let cmdline = || format!("{} {}", program, args.join(" "));
    let output = std::process::Command::new(program)
        .args(args)
        .current_dir(root_dir()?)
        .envs(env)
        .output()
        .with_context(|| format!("Error launching {}", cmdline()))?;
    let code =
        output.status.code().with_context(|| format!("Command received signal: {}", cmdline()))?;
    if code != 0 {
        Err(anyhow::anyhow!(
            "Command {} exited with non-zero status: {}\nstdout: {}\nstderr: {}",
            cmdline(),
            code,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    } else {
        Ok(output.stdout)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    match ApplicationArguments::from_args().command {
        TaskCommand::CheckLicenseHeaders(cmd) => cmd.check_license_headers()?,
        TaskCommand::CppDocs => cppdocs::generate()?,
        TaskCommand::Cbindgen(cmd) => cmd.run()?,
    };

    Ok(())
}
