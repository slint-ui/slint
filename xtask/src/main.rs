// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore nodepackage
use anyhow::Context;
use clap::Parser;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::str::FromStr;

mod check_profiles;
mod generate_cppdocs_headers;
mod license;
mod license_headers_check;
mod nodepackage;
mod reuse_compliance_check;
#[derive(Debug, clap::Parser)]
#[command(author, version, about, long_about = None)]
pub enum TaskCommand {
    #[command(name = "check_license_headers")]
    CheckLicenseHeaders(license_headers_check::LicenseHeaderCheck),
    #[command(name = "check_profiles")]
    CheckProfiles(check_profiles::ProfilesCheck),
    #[command(name = "generate_cppdocs_headers")]
    GenerateCppDocsHeaders(GenerateCppDocsHeadersCommand),
    #[command(name = "license")]
    License(license::LicenseCommand),
    #[command(name = "node_package")]
    NodePackage(nodepackage::NodePackageOptions),
    #[command(name = "check_reuse_compliance")]
    ReuseComplianceCheck(reuse_compliance_check::ReuseComplianceCheck),
}

#[derive(Debug, clap::Parser)]
#[command(name = "xtask")]
pub struct ApplicationArguments {
    #[command(subcommand)]
    pub command: TaskCommand,
}

#[derive(Debug, clap::Parser)]
pub struct GenerateCppDocsHeadersCommand {
    /// Generate the headers for the experimental APIs as well.
    #[arg(long, action)]
    experimental: bool,
}

/// The root dir of the git repository
fn root_dir() -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop(); // $root/xtask -> $root
    root
}

/// Every file tracked by the repository's VCS, as an absolute path.
fn collect_files() -> anyhow::Result<Vec<PathBuf>> {
    let root = root_dir();

    let mut files = Vec::new();
    let (ls_files_output, split_char) = if root.join(".jj").exists() {
        (
            run_command(
                "jj",
                &["file", "list"],
                std::iter::empty::<(std::ffi::OsString, std::ffi::OsString)>(),
            )?
            .stdout,
            b'\n',
        )
    } else {
        (
            run_command(
                "git",
                &["ls-files", "-z"],
                std::iter::empty::<(std::ffi::OsString, std::ffi::OsString)>(),
            )?
            .stdout,
            b'\0',
        )
    };

    for path in ls_files_output.split(|ch| *ch == split_char) {
        if path.is_empty() {
            continue;
        }
        let path = PathBuf::from_str(
            std::str::from_utf8(path)
                .context("Error decoding file list command output from VCS as utf-8")?,
        )
        .context("Failed to decide path output in VCS file list")?;

        if !path.is_dir() {
            files.push(root.join(path));
        }
    }

    Ok(files)
}

/// `path` relative to the repository root, with `/` separators on every
/// platform so that it can be compared against the paths hardcoded in the tasks.
fn repo_relative(path: &Path) -> String {
    path.strip_prefix(root_dir()).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

struct CommandOutput {
    stdout: Vec<u8>,
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
        Ok(CommandOutput { stdout: output.stdout })
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    match ApplicationArguments::parse().command {
        TaskCommand::CheckLicenseHeaders(cmd) => cmd.check_license_headers()?,
        TaskCommand::CheckProfiles(cmd) => cmd.check_profiles()?,
        TaskCommand::GenerateCppDocsHeaders(cmd) => {
            generate_cppdocs_headers::generate(cmd.experimental)?
        }
        TaskCommand::License(cmd) => cmd.run()?,
        TaskCommand::NodePackage(cmd) => nodepackage::generate(cmd.sha1)?,
        TaskCommand::ReuseComplianceCheck(cmd) => cmd.check_reuse_compliance()?,
    };

    Ok(())
}
