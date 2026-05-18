// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use clap::{Args, Parser, Subcommand, ValueEnum};
use i_slint_formatter::{
    Formatter, aggregate_repository_profile, collect_standalone_slint_files,
    compare_repository_profiles, format_repository_style_comparison_report,
    format_repository_style_report, profile_file, profile_source_with_path,
};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(about = "Profile and compare formatting decisions in standalone .slint files")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Profile the repository style in the selected corpus.
    Profile(CorpusArgs),
    /// Compare formatter output against the repository style in the selected corpus.
    Compare(CorpusArgs),
}

#[derive(Args, Debug)]
struct CorpusArgs {
    /// Use a built-in corpus preset instead of explicit paths.
    #[arg(long, value_enum)]
    corpus: Option<CorpusPreset>,

    /// Files or directories to analyze.
    paths: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CorpusPreset {
    /// Recursively traverse the current directory.
    Full,
    /// Traverse a faster subset: examples/ and tests/cases/.
    Fast,
}

#[derive(Debug)]
struct ResolvedCorpus {
    label: String,
    files: Vec<PathBuf>,
}

#[derive(Debug)]
struct ComparisonRun {
    compared_files: usize,
    skipped_files_with_parse_errors: usize,
    skipped_files_with_formatter_errors: usize,
    failed_formatter_files: Vec<PathBuf>,
    files_changed: usize,
    report: String,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Profile(args) => run_profile(&args),
        Command::Compare(args) => run_compare(&args),
    };

    match result {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(err) => report_error(&err),
    }
}

fn run_profile(args: &CorpusArgs) -> io::Result<String> {
    let corpus = resolve_corpus(args)?;
    let mut profiles = Vec::new();

    for file in &corpus.files {
        profiles.push(profile_file(file)?);
    }

    let repository = aggregate_repository_profile(&profiles);
    Ok(format!("Corpus: {}\n{}", corpus.label, format_repository_style_report(&repository)))
}

fn run_compare(args: &CorpusArgs) -> io::Result<String> {
    let corpus = resolve_corpus(args)?;
    let formatter = Formatter::new().map_err(formatter_error_to_io)?;
    let mut reference_profiles = Vec::new();
    let mut formatted_profiles = Vec::new();
    let mut skipped_files_with_parse_errors = 0;
    let mut skipped_files_with_formatter_errors = 0;
    let mut failed_formatter_files = Vec::new();
    let mut files_changed = 0;

    for file in &corpus.files {
        let reference_profile = profile_file(file)?;
        if reference_profile.has_parse_errors {
            skipped_files_with_parse_errors += 1;
            continue;
        }

        let source = fs::read_to_string(file)?;
        let formatted = match formatter.format_str(&source) {
            Ok(formatted) => formatted,
            Err(_) => {
                skipped_files_with_formatter_errors += 1;
                if failed_formatter_files.len() < 5 {
                    failed_formatter_files.push(file.clone());
                }
                continue;
            }
        };
        if formatted.changed {
            files_changed += 1;
        }

        reference_profiles.push(reference_profile);
        formatted_profiles.push(profile_source_with_path(&formatted.text, Some(file.as_path())));
    }

    let reference = aggregate_repository_profile(&reference_profiles);
    let candidate = aggregate_repository_profile(&formatted_profiles);
    let comparison = compare_repository_profiles(&reference, &candidate);
    let run = ComparisonRun {
        compared_files: reference_profiles.len(),
        skipped_files_with_parse_errors,
        skipped_files_with_formatter_errors,
        failed_formatter_files,
        files_changed,
        report: format_repository_style_comparison_report(&comparison),
    };

    Ok(format_comparison_report(&corpus, &run))
}

fn resolve_corpus(args: &CorpusArgs) -> io::Result<ResolvedCorpus> {
    if args.corpus.is_some() && !args.paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "use either --corpus or explicit paths, not both",
        ));
    }

    let (label, roots) = if !args.paths.is_empty() {
        (format!("explicit paths ({})", display_paths(&args.paths)), args.paths.clone())
    } else {
        match args.corpus.unwrap_or(CorpusPreset::Full) {
            CorpusPreset::Full => {
                ("full corpus (current directory traversal)".into(), vec![std::env::current_dir()?])
            }
            CorpusPreset::Fast => {
                let roots = ["examples", "tests/cases"]
                    .into_iter()
                    .map(PathBuf::from)
                    .filter(|path| path.exists())
                    .collect::<Vec<_>>();
                ("fast corpus (examples/ + tests/cases/)".into(), roots)
            }
        }
    };

    if roots.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no corpus roots found for the selected input",
        ));
    }

    let files = collect_standalone_slint_files(&roots)?;
    if files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no standalone .slint files found in {}", display_paths(&roots)),
        ));
    }

    Ok(ResolvedCorpus { label, files })
}

fn format_comparison_report(corpus: &ResolvedCorpus, run: &ComparisonRun) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Corpus: {}", corpus.label));
    lines.push(format!("Compared {} standalone .slint file(s)", run.compared_files));
    lines.push(format!("Files changed by formatter: {}", run.files_changed));
    if run.skipped_files_with_parse_errors > 0 {
        lines.push(format!(
            "Skipped files with parse errors: {}",
            run.skipped_files_with_parse_errors
        ));
    }
    if run.skipped_files_with_formatter_errors > 0 {
        lines.push(format!(
            "Skipped files with formatter failures: {}",
            run.skipped_files_with_formatter_errors
        ));
        if !run.failed_formatter_files.is_empty() {
            lines.push("Failed formatter files:".into());
            for file in &run.failed_formatter_files {
                lines.push(format!("  - {}", display_path(file)));
            }
            if run.skipped_files_with_formatter_errors > run.failed_formatter_files.len() {
                lines.push(format!(
                    "  - … and {} more",
                    run.skipped_files_with_formatter_errors - run.failed_formatter_files.len()
                ));
            }
        }
    }
    lines.push(String::new());
    lines.push(run.report.clone());
    lines.join("\n")
}

fn display_paths(paths: &[PathBuf]) -> String {
    paths.iter().map(|path| display_path(path)).collect::<Vec<_>>().join(", ")
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn report_error(err: &dyn std::fmt::Display) -> ExitCode {
    eprintln!("{err}");
    ExitCode::from(1)
}

fn formatter_error_to_io(err: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_report_lists_formatter_failures() {
        let corpus = ResolvedCorpus { label: "test corpus".into(), files: Vec::new() };
        let run = ComparisonRun {
            compared_files: 2,
            skipped_files_with_parse_errors: 1,
            skipped_files_with_formatter_errors: 2,
            failed_formatter_files: vec!["first.slint".into(), "second.slint".into()],
            files_changed: 1,
            report: "comparison details".into(),
        };

        let report = format_comparison_report(&corpus, &run);

        assert!(report.contains("Skipped files with formatter failures: 2"));
        assert!(report.contains("Failed formatter files:"));
        assert!(report.contains("  - first.slint"));
        assert!(report.contains("  - second.slint"));
    }
}
