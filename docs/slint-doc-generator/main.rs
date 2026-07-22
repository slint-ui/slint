// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(not(target_os = "android"))]

mod element_docs;
mod headless;
mod mdx;
mod screenshots;
mod traceability;

use clap::Parser;
use std::path::PathBuf;
use xshell::{Shell, cmd};

#[derive(Debug, clap::Parser)]
#[command(author, version, about = "Documentation generator for the Slint project")]
struct Cli {
    #[arg(long, action)]
    experimental: bool,

    /// Generate the SC-filtered reference into docs/safety instead of docs/astro.
    /// Only items annotated with `\sc` are included, and screenshot code-fence
    /// attributes are stripped.
    #[arg(long, action)]
    slint_sc: bool,

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
pub(crate) fn root_dir() -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop(); // docs/slint-doc-generator -> docs
    root.pop(); // docs -> root
    root
}

/// Configuration for a documentation generation run.
#[derive(Clone)]
pub struct Config {
    /// Absolute path to the Astro project root (containing `package.json`).
    pub astro_dir: PathBuf,
    /// Absolute path to the root of the generated content. Everything below
    /// it is written by this tool and gitignored; one subdirectory per section
    /// of the site the pages belong to. Pages carry an explicit `slug`, so
    /// this location doesn't determine their URL.
    pub generated_dir: PathBuf,
    /// Skip items that don't carry a `\sc` marker in their doc comment.
    pub sc_only: bool,
    /// Strip screenshot code-fence attributes instead of wrapping with
    /// `<CodeSnippetMD>`.
    pub skip_screenshots: bool,
    pub include_experimental: bool,
}

/// Path of the generated content root, relative to the site's `src` directory.
/// Also the prefix of the `import` paths the generated pages use, and the sole
/// entry each site's `.gitignore` needs for generated content.
pub const GENERATED_DIR: &str = "content/docs/generated";

impl Config {
    pub fn slint_docs(include_experimental: bool) -> Self {
        let astro_dir = root_dir().join("docs/astro");
        Self {
            generated_dir: astro_dir.join("src").join(GENERATED_DIR),
            astro_dir,
            sc_only: false,
            skip_screenshots: false,
            include_experimental,
        }
    }
    pub fn safety_manual(include_experimental: bool) -> Self {
        let astro_dir = root_dir().join("docs/safety");
        Self {
            generated_dir: astro_dir.join("src").join(GENERATED_DIR),
            astro_dir,
            sc_only: true,
            skip_screenshots: true,
            include_experimental,
        }
    }

    /// Generated pages of the API reference.
    pub fn reference_dir(&self) -> PathBuf {
        self.generated_dir.join("reference")
    }

    /// Generated pages of the qualification plan (safety manual only).
    pub fn qualification_plan_dir(&self) -> PathBuf {
        self.generated_dir.join("qualification-plan")
    }
}

fn build_astro(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let sh = Shell::new()?;
    let _p = sh.push_dir(&cfg.astro_dir);
    cmd!(sh, "pnpm install --frozen-lockfile --ignore-scripts").run()?;
    let mut build_cmd = cmd!(sh, "pnpm run build");
    if cfg.include_experimental {
        build_cmd = build_cmd.env("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    }
    build_cmd.run()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let experimental = args.experimental;
    let cfg = if args.slint_sc {
        Config::safety_manual(experimental)
    } else {
        Config::slint_docs(experimental)
    };

    match args.command {
        Some(Command::GenerateMdx) => {
            mdx::generate(&cfg)?;
        }
        Some(Command::Screenshots(args)) => {
            screenshots::run(args)?;
        }
        Some(Command::BuildAstro) => {
            build_astro(&cfg)?;
        }
        None => {
            // Generate mdx first because screenshots reads them.
            mdx::generate(&cfg)?;
            if !cfg.skip_screenshots {
                let docs_folder = cfg.astro_dir.join("src/content");
                let reference_elements = cfg.astro_dir.join("src/content/docs/reference/elements");
                screenshots::run(screenshots::ScreenshotsArgs {
                    include_paths: vec![reference_elements],
                    library_paths: vec![],
                    docs_folder,
                    style: None,
                    overwrite_files: true,
                    component: None,
                })?;
            }
            build_astro(&cfg)?;
        }
    }

    Ok(())
}
