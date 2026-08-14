// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(not(target_os = "android"))]

mod coverage;
mod element_docs;
mod headless;
mod mdx;
mod screenshots;
mod test_results;
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

    /// Report the coverage from this `cargo llvm-cov report --json` export in
    /// the safety manual's Test Coverage chapter. Without it the chapter is a
    /// placeholder explaining how to build with coverage.
    #[arg(long, value_name = "FILE")]
    coverage_json: Option<PathBuf>,

    /// Also ship this `cargo llvm-cov report --html` report with the manual
    /// and link its per-line pages from the Test Coverage chapter.
    #[arg(long, value_name = "DIR", requires = "coverage_json")]
    coverage_html: Option<PathBuf>,

    /// Report the test outcomes collected in this directory by
    /// scripts/slint_sc_test_suite.sh in the safety manual's Test Results
    /// chapter. Without it the chapter is a placeholder.
    #[arg(long, value_name = "DIR")]
    test_results: Option<PathBuf>,

    /// Exit with [`GAPS_EXIT_CODE`] when the safety manual shows a gap: a
    /// runtime source file below 100% line, function, or region coverage, or a
    /// requirement paragraph that no test declares. The pages are written and
    /// the site is built either way. Requires a coverage export to check the
    /// coverage half against.
    #[arg(long, action, requires = "coverage_json")]
    fail_on_gaps: bool,

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
    /// `cargo llvm-cov report --json` export to report in the safety manual's
    /// Test Coverage chapter; without it the chapter is a placeholder.
    pub coverage_json: Option<PathBuf>,
    /// `cargo llvm-cov report --html` report to ship with the manual for
    /// per-line detail.
    pub coverage_html: Option<PathBuf>,
    /// Test outcomes collected by scripts/slint_sc_test_suite.sh, for the
    /// safety manual's Test Results chapter; without them the chapter is a
    /// placeholder.
    pub test_results: Option<PathBuf>,
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
            coverage_json: None,
            coverage_html: None,
            test_results: None,
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
            coverage_json: None,
            coverage_html: None,
            test_results: None,
        }
    }

    /// Generated pages of the API reference.
    pub fn reference_dir(&self) -> PathBuf {
        self.generated_dir.join("reference")
    }

    /// Generated pages of the qualification report (safety manual only).
    pub fn qualification_report_dir(&self) -> PathBuf {
        self.generated_dir.join("qualification-report")
    }

    /// Create a page of the qualification report, ready for writing.
    pub fn qualification_page(
        &self,
        file_name: &str,
    ) -> anyhow::Result<std::io::BufWriter<std::fs::File>> {
        use anyhow::Context;
        let dir = self.qualification_report_dir();
        std::fs::create_dir_all(&dir).with_context(|| format!("error creating {dir:?}"))?;
        let path = dir.join(file_name);
        Ok(std::io::BufWriter::new(
            std::fs::File::create(&path).with_context(|| format!("error creating {path:?}"))?,
        ))
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

/// Exit code of a run that found gaps, distinct from the `1` of a run that
/// failed. Everything is written and built by then, so a caller can publish
/// the documentation it produced and still fail its build afterwards.
pub const GAPS_EXIT_CODE: u8 = 2;

fn main() -> Result<std::process::ExitCode, Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let experimental = args.experimental;
    let mut cfg = if args.slint_sc {
        Config::safety_manual(experimental)
    } else {
        Config::slint_docs(experimental)
    };
    cfg.coverage_json = args.coverage_json;
    cfg.coverage_html = args.coverage_html;
    cfg.test_results = args.test_results;

    let mut gaps = Vec::new();
    match args.command {
        Some(Command::GenerateMdx) => {
            gaps = mdx::generate(&cfg)?;
        }
        Some(Command::Screenshots(args)) => {
            screenshots::run(args)?;
        }
        Some(Command::BuildAstro) => {
            build_astro(&cfg)?;
        }
        None => {
            // Generate mdx first because screenshots reads them.
            gaps = mdx::generate(&cfg)?;
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

    if args.fail_on_gaps && !gaps.is_empty() {
        eprintln!("error: the safety manual has {} gap(s):", gaps.len());
        for gap in &gaps {
            eprintln!("  {gap}");
        }
        eprintln!(
            "the runtime must stay completely covered and every requirement must be declared by a test"
        );
        return Ok(std::process::ExitCode::from(GAPS_EXIT_CODE));
    }

    Ok(std::process::ExitCode::SUCCESS)
}
