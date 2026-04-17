// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(not(target_os = "android"))]

mod headless;
mod mdx;
mod screenshots;

use clap::Parser;
use std::path::PathBuf;
use xshell::{Shell, cmd};

#[derive(Debug, clap::Parser)]
#[command(author, version, about = "Documentation generator for the Slint project")]
struct Cli {
    #[arg(long, action)]
    experimental: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    /// Generate .mdx and .md files for builtins, enums, structs, and keys.
    GenerateMdx,
    /// Generate screenshots from code snippets in documentation files.
    Screenshots(screenshots::ScreenshotsArgs),
    /// Build the Astro documentation site.
    BuildAstro,
}

/// Find the root of the git repository.
fn root_dir() -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop(); // docs/slint-doc-generator -> docs
    root.pop(); // docs -> root
    root
}

fn build_astro(include_experimental: bool) -> Result<(), Box<dyn std::error::Error>> {
    let docs_source_dir = root_dir().join("docs/astro");
    let sh = Shell::new()?;
    let _p = sh.push_dir(&docs_source_dir);
    cmd!(sh, "pnpm install --frozen-lockfile --ignore-scripts").run()?;
    let mut build_cmd = cmd!(sh, "pnpm run build");
    if include_experimental {
        build_cmd = build_cmd.env("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    }
    build_cmd.run()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let experimental = args.experimental;

    match args.command {
        Some(Command::GenerateMdx) => {
            mdx::generate(experimental)?;
        }
        Some(Command::Screenshots(args)) => {
            screenshots::run(args)?;
        }
        Some(Command::BuildAstro) => {
            build_astro(experimental)?;
        }
        None => {
            // Generate mdx first because screenshots reads them.
            mdx::generate(experimental)?;
            let docs_folder = root_dir().join("docs/astro/src/content");
            screenshots::run(screenshots::ScreenshotsArgs {
                include_paths: vec![],
                library_paths: vec![],
                docs_folder,
                style: None,
                overwrite_files: true,
                component: None,
            })?;
            build_astro(experimental)?;
        }
    }

    Ok(())
}
