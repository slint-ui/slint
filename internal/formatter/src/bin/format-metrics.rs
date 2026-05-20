// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use clap::{Args, Parser, Subcommand, ValueEnum};
use i_slint_formatter::profiling::{
    aggregate_repository_profile, collect_standalone_slint_files, compare_repository_profiles,
    format_repository_style_comparison_report, format_repository_style_report, profile_file,
    profile_source_with_path,
};
use i_slint_formatter::{Formatter, slint_language};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use topiary_core::check_query_coverage;

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
    /// Report whole-file line churn between original and formatted output.
    Diff(CorpusArgs),
    /// Report Topiary query coverage across the selected corpus.
    Coverage(CorpusArgs),
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

#[derive(Debug)]
struct DiffRun {
    compared_files: usize,
    skipped_files_with_parse_errors: usize,
    skipped_files_with_formatter_errors: usize,
    failed_formatter_files: Vec<PathBuf>,
    files_changed: usize,
    inserted_lines: usize,
    removed_lines: usize,
    blank_inserted_lines: usize,
    blank_removed_lines: usize,
    files: Vec<FileDiffSummary>,
}

#[derive(Debug)]
struct FileDiffSummary {
    path: PathBuf,
    inserted_lines: usize,
    removed_lines: usize,
    blank_inserted_lines: usize,
    blank_removed_lines: usize,
    sample: Vec<LineDiffSample>,
}

#[derive(Debug)]
struct LineDiffSample {
    prefix: char,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineDiffOp<'a> {
    Equal(&'a str),
    Delete(&'a str),
    Insert(&'a str),
}

#[derive(Debug)]
struct CoverageRun {
    scanned_files: usize,
    covered_files: usize,
    skipped_files_with_parse_errors: usize,
    skipped_files_with_coverage_errors: usize,
    failed_coverage_files: Vec<PathBuf>,
    pattern_count: usize,
    average_file_coverage: f32,
    patterns_used_in_any_file: usize,
    pattern_usages: Vec<PatternUsage>,
}

#[derive(Debug)]
struct PatternUsage {
    pattern_index: usize,
    file_matches: usize,
    line: usize,
    column: usize,
    snippet: String,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Profile(args) => run_profile(&args),
        Command::Compare(args) => run_compare(&args),
        Command::Diff(args) => run_diff(&args),
        Command::Coverage(args) => run_coverage(&args),
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

fn run_diff(args: &CorpusArgs) -> io::Result<String> {
    let corpus = resolve_corpus(args)?;
    let formatter = Formatter::new().map_err(formatter_error_to_io)?;
    let mut run = DiffRun {
        compared_files: 0,
        skipped_files_with_parse_errors: 0,
        skipped_files_with_formatter_errors: 0,
        failed_formatter_files: Vec::new(),
        files_changed: 0,
        inserted_lines: 0,
        removed_lines: 0,
        blank_inserted_lines: 0,
        blank_removed_lines: 0,
        files: Vec::new(),
    };

    for file in &corpus.files {
        let reference_profile = profile_file(file)?;
        if reference_profile.has_parse_errors {
            run.skipped_files_with_parse_errors += 1;
            continue;
        }

        let source = fs::read_to_string(file)?;
        let formatted = match formatter.format_str(&source) {
            Ok(formatted) => formatted,
            Err(_) => {
                run.skipped_files_with_formatter_errors += 1;
                if run.failed_formatter_files.len() < 5 {
                    run.failed_formatter_files.push(file.clone());
                }
                continue;
            }
        };

        run.compared_files += 1;
        let summary = summarize_file_diff(file, &source, &formatted.text);
        if summary.inserted_lines + summary.removed_lines > 0 {
            run.files_changed += 1;
            run.inserted_lines += summary.inserted_lines;
            run.removed_lines += summary.removed_lines;
            run.blank_inserted_lines += summary.blank_inserted_lines;
            run.blank_removed_lines += summary.blank_removed_lines;
            run.files.push(summary);
        }
    }

    Ok(format_diff_report(&corpus, &run))
}

fn run_coverage(args: &CorpusArgs) -> io::Result<String> {
    let corpus = resolve_corpus(args)?;
    let language = slint_language().map_err(formatter_error_to_io)?;
    let pattern_count = language.query.query.pattern_count();
    let mut pattern_missing_counts = vec![0usize; pattern_count];
    let mut covered_files = 0usize;
    let mut skipped_files_with_parse_errors = 0;
    let mut skipped_files_with_coverage_errors = 0;
    let mut failed_coverage_files = Vec::new();
    let mut total_coverage = 0.0f32;

    for file in &corpus.files {
        let reference_profile = profile_file(file)?;
        if reference_profile.has_parse_errors {
            skipped_files_with_parse_errors += 1;
            continue;
        }

        let source = fs::read_to_string(file)?;
        match check_query_coverage(&source, &language.query, &language.grammar) {
            Ok(coverage) => {
                covered_files += 1;
                total_coverage += coverage.cover_percentage;
                for missing in &coverage.missing_patterns {
                    let start = missing.offset();
                    if let Some(pattern_index) = (0..pattern_count).find(|&index| {
                        let pattern_start = language.query.query.start_byte_for_pattern(index);
                        let pattern_end = language.query.query.end_byte_for_pattern(index);
                        pattern_start <= start && start < pattern_end
                    }) {
                        if let Some(count) = pattern_missing_counts.get_mut(pattern_index) {
                            *count += 1;
                        }
                    }
                }
            }
            Err(_) => {
                skipped_files_with_coverage_errors += 1;
                if failed_coverage_files.len() < 5 {
                    failed_coverage_files.push(file.clone());
                }
            }
        }
    }

    let average_file_coverage =
        if covered_files == 0 { 0.0 } else { total_coverage / covered_files as f32 };

    let mut pattern_usages = Vec::new();
    for (pattern_index, missing_files) in pattern_missing_counts.into_iter().enumerate() {
        pattern_usages.push(PatternUsage {
            pattern_index,
            file_matches: covered_files.saturating_sub(missing_files),
            line: 0,
            column: 0,
            snippet: pattern_snippet(&language, pattern_index),
        });
    }
    for usage in &mut pattern_usages {
        let position = language.query.pattern_position(usage.pattern_index);
        usage.line = position.row as usize;
        usage.column = position.column as usize;
    }
    pattern_usages.sort_by_key(|usage| (usage.file_matches, usage.line, usage.column));

    let run = CoverageRun {
        scanned_files: corpus.files.len(),
        covered_files,
        skipped_files_with_parse_errors,
        skipped_files_with_coverage_errors,
        failed_coverage_files,
        pattern_count,
        average_file_coverage,
        patterns_used_in_any_file: pattern_usages
            .iter()
            .filter(|usage| usage.file_matches > 0)
            .count(),
        pattern_usages,
    };

    Ok(format_coverage_report(&corpus, &run))
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

fn format_diff_report(corpus: &ResolvedCorpus, run: &DiffRun) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Corpus: {}", corpus.label));
    lines.push(format!("Compared {} standalone .slint file(s)", run.compared_files));
    lines.push(format!("Changed files: {}", run.files_changed));
    lines.push(format!(
        "Whole-file line churn: +{} / -{} ({} total)",
        run.inserted_lines,
        run.removed_lines,
        run.inserted_lines + run.removed_lines
    ));
    lines.push(format!(
        "Blank-line churn: +{} / -{}",
        run.blank_inserted_lines, run.blank_removed_lines
    ));
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

    if run.files.is_empty() {
        lines.push("No formatter churn detected.".into());
        return lines.join("\n");
    }

    let mut files = run.files.iter().collect::<Vec<_>>();
    files.sort_by(|left, right| {
        let left_churn = left.inserted_lines + left.removed_lines;
        let right_churn = right.inserted_lines + right.removed_lines;
        right_churn
            .cmp(&left_churn)
            .then_with(|| right.inserted_lines.cmp(&left.inserted_lines))
            .then_with(|| left.path.cmp(&right.path))
    });

    lines.push(String::new());
    lines.push("Most diff-heavy files:".into());
    for file in files.iter().take(10) {
        lines.push(format!(
            "  - {} (+{} / -{}, blank +{} / -{})",
            display_path(&file.path),
            file.inserted_lines,
            file.removed_lines,
            file.blank_inserted_lines,
            file.blank_removed_lines
        ));
    }
    if files.len() > 10 {
        lines.push(format!("  - … and {} more", files.len() - 10));
    }

    lines.push(String::new());
    lines.push("Sample diff lines:".into());
    for file in files.iter().take(3) {
        lines.push(format!("  - {}", display_path(&file.path)));
        for sample in &file.sample {
            lines.push(format!("    {}{}", sample.prefix, sample.text));
        }
    }

    lines.join("\n")
}

fn summarize_file_diff(path: &Path, original: &str, formatted: &str) -> FileDiffSummary {
    let original_lines = split_logical_lines(original);
    let formatted_lines = split_logical_lines(formatted);
    let diff = myers_line_diff(&original_lines, &formatted_lines);

    let mut inserted_lines = 0;
    let mut removed_lines = 0;
    let mut blank_inserted_lines = 0;
    let mut blank_removed_lines = 0;
    let mut sample = Vec::new();

    for op in diff {
        match op {
            LineDiffOp::Equal(_) => {}
            LineDiffOp::Delete(line) => {
                removed_lines += 1;
                if line.trim().is_empty() {
                    blank_removed_lines += 1;
                }
                if sample.len() < 12 {
                    sample.push(LineDiffSample { prefix: '-', text: line.to_string() });
                }
            }
            LineDiffOp::Insert(line) => {
                inserted_lines += 1;
                if line.trim().is_empty() {
                    blank_inserted_lines += 1;
                }
                if sample.len() < 12 {
                    sample.push(LineDiffSample { prefix: '+', text: line.to_string() });
                }
            }
        }
    }

    FileDiffSummary {
        path: path.to_owned(),
        inserted_lines,
        removed_lines,
        blank_inserted_lines,
        blank_removed_lines,
        sample,
    }
}

fn myers_line_diff<'a>(original: &'a [&'a str], formatted: &'a [&'a str]) -> Vec<LineDiffOp<'a>> {
    if original == formatted {
        return original.iter().copied().map(LineDiffOp::Equal).collect();
    }

    let n = original.len();
    let m = formatted.len();
    let max = n + m;
    let offset = max as isize;
    let mut v = vec![0usize; 2 * max + 1];
    let mut trace = Vec::with_capacity(max + 1);

    for d in 0..=max {
        let d_isize = d as isize;
        for k in (-d_isize..=d_isize).step_by(2) {
            let idx = (k + offset) as usize;
            let x = if k == -d_isize || (k != d_isize && v[idx - 1] < v[idx + 1]) {
                v[idx + 1]
            } else {
                v[idx - 1] + 1
            };

            let mut x = x;
            let mut y = (x as isize - k) as usize;
            while x < n && y < m && original[x] == formatted[y] {
                x += 1;
                y += 1;
            }
            v[idx] = x;
            if x >= n && y >= m {
                trace.push(v.clone());
                return backtrack_line_diff(&trace, original, formatted);
            }
        }
        trace.push(v.clone());
    }

    unreachable!("myers diff should always converge");
}

fn backtrack_line_diff<'a>(
    trace: &[Vec<usize>],
    original: &'a [&'a str],
    formatted: &'a [&'a str],
) -> Vec<LineDiffOp<'a>> {
    let mut x = original.len();
    let mut y = formatted.len();
    let max = original.len() + formatted.len();
    let mut ops = Vec::new();

    for d in (1..trace.len()).rev() {
        let v = &trace[d - 1];
        let k = x as isize - y as isize;
        let prev_k = if k == -(d as isize)
            || (k != d as isize
                && v[(k + max as isize - 1) as usize] < v[(k + max as isize + 1) as usize])
        {
            k + 1
        } else {
            k - 1
        };
        let prev_x = v[(prev_k + max as isize) as usize];
        let prev_y = (prev_x as isize - prev_k) as usize;

        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            ops.push(LineDiffOp::Equal(original[x]));
        }

        if x == prev_x {
            y -= 1;
            ops.push(LineDiffOp::Insert(formatted[y]));
        } else {
            x -= 1;
            ops.push(LineDiffOp::Delete(original[x]));
        }
    }

    while x > 0 && y > 0 {
        x -= 1;
        y -= 1;
        ops.push(LineDiffOp::Equal(original[x]));
    }

    ops.reverse();
    ops
}

fn split_logical_lines(source: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;

    for (index, ch) in source.char_indices() {
        if ch == '\n' {
            let mut end = index;
            if end > start && source.as_bytes()[end - 1] == b'\r' {
                end -= 1;
            }
            lines.push(&source[start..end]);
            start = index + 1;
        }
    }

    if start < source.len() {
        let mut end = source.len();
        if end > start && source.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        lines.push(&source[start..end]);
    }

    lines
}

fn format_coverage_report(corpus: &ResolvedCorpus, run: &CoverageRun) -> String {
    const LOW_USAGE_THRESHOLD: usize = 3;
    const LOWEST_USAGE_PATTERN_LIMIT: usize = 5;

    let mut lines = Vec::new();
    lines.push(format!("Corpus: {}", corpus.label));
    lines.push(format!("Scanned {} standalone .slint file(s)", run.scanned_files));
    lines.push(format!("Query patterns: {}", run.pattern_count));
    lines.push(format!("Average file coverage: {:.2}%", run.average_file_coverage * 100.0));
    lines.push(format!(
        "Patterns used in at least one file: {}/{}",
        run.patterns_used_in_any_file, run.pattern_count
    ));

    let never_used: Vec<_> =
        run.pattern_usages.iter().filter(|usage| usage.file_matches == 0).collect();
    let low_usage: Vec<_> = run
        .pattern_usages
        .iter()
        .filter(|usage| usage.file_matches > 0 && usage.file_matches <= LOW_USAGE_THRESHOLD)
        .collect();

    lines.push(format!("Patterns never matched: {}", never_used.len()));
    lines.push(format!(
        "Patterns matched in <= {} file(s): {}",
        LOW_USAGE_THRESHOLD,
        low_usage.len()
    ));

    if run.skipped_files_with_parse_errors > 0 {
        lines.push(format!(
            "Skipped files with parse errors: {}",
            run.skipped_files_with_parse_errors
        ));
    }
    if run.skipped_files_with_coverage_errors > 0 {
        lines.push(format!(
            "Skipped files with coverage errors: {}",
            run.skipped_files_with_coverage_errors
        ));
        if !run.failed_coverage_files.is_empty() {
            lines.push("Failed coverage files:".into());
            for file in &run.failed_coverage_files {
                lines.push(format!("  - {}", display_path(file)));
            }
            if run.skipped_files_with_coverage_errors > run.failed_coverage_files.len() {
                lines.push(format!(
                    "  - … and {} more",
                    run.skipped_files_with_coverage_errors - run.failed_coverage_files.len()
                ));
            }
        }
    }

    lines.push(String::new());
    lines.push("Patterns never matched:".into());
    if never_used.is_empty() {
        lines.push("  - none".into());
    } else {
        for usage in never_used {
            lines.push(format_pattern_usage(usage, run.covered_files));
        }
    }

    lines.push(String::new());
    lines.push(format!("Patterns matched in <= {} file(s):", LOW_USAGE_THRESHOLD));
    if low_usage.is_empty() {
        lines.push("  - none".into());
    } else {
        for usage in low_usage {
            lines.push(format_pattern_usage(usage, run.covered_files));
        }
    }

    let lowest_usage_patterns =
        run.pattern_usages.iter().take(LOWEST_USAGE_PATTERN_LIMIT).collect::<Vec<_>>();

    lines.push(String::new());
    lines.push("Lowest-usage patterns:".into());
    if lowest_usage_patterns.is_empty() {
        lines.push("  - none".into());
    } else {
        for usage in lowest_usage_patterns {
            lines.push(format_pattern_usage(usage, run.covered_files));
        }
    }

    lines.join("\n")
}

fn format_pattern_usage(usage: &PatternUsage, covered_files: usize) -> String {
    format!(
        "  - pattern #{:03} at {}:{}: {} (used in {}/{} file(s))",
        usage.pattern_index + 1,
        usage.line,
        usage.column,
        usage.snippet,
        usage.file_matches,
        covered_files
    )
}

fn pattern_snippet(language: &topiary_core::Language, pattern_index: usize) -> String {
    let query_content = &language.query.query_content;
    let start = language.query.query.start_byte_for_pattern(pattern_index);
    let end = language.query.query.end_byte_for_pattern(pattern_index);
    let pattern = &query_content[start..end];
    let snippet = pattern
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with(';'))
        .unwrap_or("")
        .trim();
    if snippet.is_empty() { "<empty pattern>".into() } else { truncate(snippet, 100) }
}

fn truncate(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (count, ch) in text.chars().enumerate() {
        if count >= max_chars {
            out.push('…');
            return out;
        }
        out.push(ch);
    }
    out
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

    #[test]
    fn coverage_report_lists_unused_and_low_usage_patterns() {
        let corpus = ResolvedCorpus { label: "test corpus".into(), files: Vec::new() };
        let run = CoverageRun {
            scanned_files: 4,
            covered_files: 3,
            skipped_files_with_parse_errors: 1,
            skipped_files_with_coverage_errors: 0,
            failed_coverage_files: Vec::new(),
            pattern_count: 4,
            average_file_coverage: 0.5,
            patterns_used_in_any_file: 3,
            pattern_usages: vec![
                PatternUsage {
                    pattern_index: 0,
                    file_matches: 0,
                    line: 1,
                    column: 1,
                    snippet: "(foo)".into(),
                },
                PatternUsage {
                    pattern_index: 1,
                    file_matches: 1,
                    line: 2,
                    column: 1,
                    snippet: "(bar)".into(),
                },
                PatternUsage {
                    pattern_index: 2,
                    file_matches: 2,
                    line: 3,
                    column: 1,
                    snippet: "(baz)".into(),
                },
                PatternUsage {
                    pattern_index: 3,
                    file_matches: 3,
                    line: 4,
                    column: 1,
                    snippet: "(qux)".into(),
                },
            ],
        };

        let report = format_coverage_report(&corpus, &run);

        assert!(report.contains("Patterns never matched: 1"));
        assert!(report.contains("Patterns matched in <= 3 file(s): 3"));
        assert!(report.contains("Patterns never matched:"));
        assert!(report.contains("pattern #001"));
        assert!(report.contains("Lowest-usage patterns:"));
        assert!(report.contains("used in 0/3 file(s)"));
        assert!(!report.contains("Lowest-coverage files:"));
    }

    #[test]
    fn diff_summary_counts_blank_line_drift() {
        let summary = summarize_file_diff(Path::new("sample.slint"), "A\n\nB\n", "A\nB\n\n");

        assert_eq!(summary.inserted_lines, 1);
        assert_eq!(summary.removed_lines, 1);
        assert_eq!(summary.blank_inserted_lines, 1);
        assert_eq!(summary.blank_removed_lines, 1);
    }
}
