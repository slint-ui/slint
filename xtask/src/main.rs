// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use anyhow::Context;
use clap::Parser;
use std::error::Error;
use std::path::PathBuf;

mod cppdocs;
mod enumdocs;
mod license_headers_check;
mod nodepackage;
mod reuse_compliance_check;

#[derive(Debug, clap::Parser)]
#[command(author, version, about, long_about = None)]
pub enum TaskCommand {
    #[command(name = "check_license_headers")]
    CheckLicenseHeaders(license_headers_check::LicenseHeaderCheck),
    #[command(name = "cppdocs")]
    CppDocs(CppDocsCommand),
    #[command(name = "node_package")]
    NodePackage,
    #[command(name = "check_reuse_compliance")]
    ReuseComplianceCheck(reuse_compliance_check::ReuseComplianceCheck),
    #[command(name = "enumdocs")]
    EnumDocs,
}

#[derive(Debug, clap::Parser)]
#[command(name = "xtask")]
pub struct ApplicationArguments {
    #[command(subcommand)]
    pub command: TaskCommand,
}

#[derive(Debug, clap::Parser)]
pub struct CppDocsCommand {
    #[arg(long, action)]
    show_warnings: bool,
}

/// The root dir of the git repository
fn root_dir() -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop(); // $root/xtask -> $root
    root
}

struct CommandOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_command<I, K, V>(program: &str, args: &[&str], env: I) -> anyhow::Result<CommandOutput>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<std::ffi::OsStr>,
    V: AsRef<std::ffi::OsStr>,
{
    let cmdline = || format!("{} {}", program, args.join(" "));
    let output = std::process::Command::new(program)
        .args(args)
        .current_dir(root_dir())
        .envs(env)
        .output()
        .with_context(|| format!("Error launching {}", cmdline()))?;
    let code = output
        .status
        .code()
        .with_context(|| format!("Command received callback: {}", cmdline()))?;
    if code != 0 {
        Err(anyhow::anyhow!(
            "Command {} exited with non-zero status: {}\nstdout: {}\nstderr: {}",
            cmdline(),
            code,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    } else {
        Ok(CommandOutput { stderr: output.stderr, stdout: output.stdout })
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    match ApplicationArguments::parse().command {
        TaskCommand::CheckLicenseHeaders(cmd) => cmd.check_license_headers()?,
        TaskCommand::CppDocs(cmd) => cppdocs::generate(cmd.show_warnings)?,
        TaskCommand::NodePackage => nodepackage::generate()?,
        TaskCommand::ReuseComplianceCheck(cmd) => cmd.check_reuse_compliance()?,
        TaskCommand::EnumDocs => enumdocs::generate()?,
    };

    Ok(())
}
