// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use clap::{Parser, ValueEnum};
use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::parser::{self, SyntaxKind, SyntaxNode, SyntaxToken, TextSize};
use i_slint_formatter::Formatter;
use i_slint_formatter::profiling::{collect_standalone_slint_files, profile_file};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    about = "Check whether indentation-noisy multiline blocks converge to the same formatted output"
)]
struct Cli {
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

#[derive(Debug, Default)]
struct HarnessRun {
    files_checked: usize,
    files_skipped_with_parse_errors: usize,
    files_skipped_with_formatter_errors: usize,
    perturbations_skipped_with_formatter_errors: usize,
    eligible_blocks: usize,
    perturbations_tested: usize,
    perturbations_converged: usize,
    perturbations_diverged: usize,
    divergent_cases: Vec<DivergentCase>,
}

#[derive(Debug)]
struct DivergentCase {
    file: PathBuf,
    block_kind: &'static str,
    block_start_line: usize,
    block_end_line: usize,
    perturbation: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct BlockRegion {
    kind: &'static str,
    start: usize,
    end: usize,
    start_line: usize,
    end_line: usize,
}

#[derive(Debug, Clone, Copy)]
enum PerturbationKind {
    AddSpaces,
    StripIndentation,
}

impl PerturbationKind {
    fn label(self) -> &'static str {
        match self {
            Self::AddSpaces => "add 2 spaces to interior lines",
            Self::StripIndentation => "strip interior indentation",
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(err) => report_error(&err),
    }
}

fn run(cli: &Cli) -> io::Result<String> {
    let corpus = resolve_corpus(cli)?;
    let formatter = Formatter::new().map_err(formatter_error_to_io)?;
    let mut run = HarnessRun::default();

    for file in &corpus.files {
        run.files_checked += 1;

        if profile_file(file)?.has_parse_errors {
            run.files_skipped_with_parse_errors += 1;
            continue;
        }

        let source = fs::read_to_string(file)?;
        let baseline = match formatter.format_str(&source) {
            Ok(result) => result.text,
            Err(_) => {
                run.files_skipped_with_formatter_errors += 1;
                continue;
            }
        };

        let (eligible_blocks, perturbations) = generate_perturbations(&source);
        run.eligible_blocks += eligible_blocks;

        for (region, perturbation, perturbed_source) in perturbations {
            run.perturbations_tested += 1;
            let formatted = match formatter.format_str(&perturbed_source) {
                Ok(result) => result.text,
                Err(_) => {
                    run.perturbations_skipped_with_formatter_errors += 1;
                    continue;
                }
            };

            if formatted == baseline {
                run.perturbations_converged += 1;
            } else {
                run.perturbations_diverged += 1;
                if run.divergent_cases.len() < 10 {
                    run.divergent_cases.push(DivergentCase {
                        file: file.clone(),
                        block_kind: region.kind,
                        block_start_line: region.start_line,
                        block_end_line: region.end_line,
                        perturbation: perturbation.label(),
                    });
                }
            }
        }
    }

    Ok(format_report(&corpus, &run))
}

fn resolve_corpus(cli: &Cli) -> io::Result<ResolvedCorpus> {
    if cli.corpus.is_some() && !cli.paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "use either --corpus or explicit paths, not both",
        ));
    }

    let (label, roots) = if !cli.paths.is_empty() {
        (format!("explicit paths ({})", display_paths(&cli.paths)), cli.paths.clone())
    } else {
        match cli.corpus.unwrap_or(CorpusPreset::Full) {
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

fn generate_perturbations(source: &str) -> (usize, Vec<(BlockRegion, PerturbationKind, String)>) {
    let mut diagnostics = BuildDiagnostics::default();
    let document = parser::parse(source.to_owned(), None, &mut diagnostics);
    if diagnostics.has_errors() {
        return (0, Vec::new());
    }

    let mut seen = BTreeSet::new();
    let mut perturbations = Vec::new();

    let regions = multiline_block_regions(source, &document);
    for region in &regions {
        for kind in [PerturbationKind::AddSpaces, PerturbationKind::StripIndentation] {
            if let Some(variant) = apply_indentation_perturbation(source, region, kind) {
                if seen.insert(variant.clone()) {
                    perturbations.push((*region, kind, variant));
                }
            }
        }
    }

    (regions.len(), perturbations)
}

fn multiline_block_regions(source: &str, document: &SyntaxNode) -> Vec<BlockRegion> {
    let mut regions = Vec::new();

    for node in document.descendants() {
        let kind = match node.kind() {
            SyntaxKind::CodeBlock => "code block",
            SyntaxKind::ObjectLiteral => "object literal",
            SyntaxKind::Element => "element",
            _ => continue,
        };

        let Some(opening_brace) = first_token_of_kind(&node, SyntaxKind::LBrace) else {
            continue;
        };
        let Some(closing_brace) = last_token_of_kind(&node, SyntaxKind::RBrace) else {
            continue;
        };

        let start_line = line_number(source, opening_brace.text_range().end());
        let end_line = line_number(source, closing_brace.text_range().start());
        if start_line == end_line {
            continue;
        }

        regions.push(BlockRegion {
            kind,
            start: byte_offset(opening_brace.text_range().end()),
            end: byte_offset(closing_brace.text_range().start()),
            start_line,
            end_line,
        });
    }

    regions
}

fn apply_indentation_perturbation(
    source: &str,
    region: &BlockRegion,
    kind: PerturbationKind,
) -> Option<String> {
    let mut changed = false;
    let mut output = String::with_capacity(source.len() + 64);
    let mut offset = 0;

    for line in source.split_inclusive('\n') {
        let line_start = offset;
        let line_end = offset + line.len();
        offset = line_end;

        if line_start >= region.start && line_start < region.end {
            let (without_ending, ending) = split_line_ending(line);
            let (leading_ws, rest) = split_leading_whitespace(without_ending);
            if rest.is_empty() {
                output.push_str(line);
                continue;
            }

            match kind {
                PerturbationKind::AddSpaces => {
                    output.push_str("  ");
                    output.push_str(leading_ws);
                    output.push_str(rest);
                    changed = true;
                }
                PerturbationKind::StripIndentation => {
                    if !leading_ws.is_empty() {
                        output.push_str(rest);
                        changed = true;
                    } else {
                        output.push_str(without_ending);
                    }
                }
            }

            output.push_str(ending);
        } else {
            output.push_str(line);
        }
    }

    if changed { Some(output) } else { None }
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(stripped) = line.strip_suffix('\n') {
        if let Some(stripped) = stripped.strip_suffix('\r') {
            (stripped, "\r\n")
        } else {
            (stripped, "\n")
        }
    } else {
        (line, "")
    }
}

fn split_leading_whitespace(line: &str) -> (&str, &str) {
    let idx = line
        .char_indices()
        .find_map(|(index, ch)| (!ch.is_whitespace()).then_some(index))
        .unwrap_or(line.len());
    line.split_at(idx)
}

fn first_token_of_kind(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens_in_node(node).into_iter().find(|token| token.kind() == kind)
}

fn last_token_of_kind(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens_in_node(node).into_iter().rev().find(|token| token.kind() == kind)
}

fn tokens_in_node(node: &SyntaxNode) -> Vec<SyntaxToken> {
    let Some(mut token) = node.first_token() else {
        return Vec::new();
    };

    let end = byte_offset(node.text_range().end());
    let mut tokens = Vec::new();

    loop {
        if byte_offset(token.text_range().start()) >= end {
            break;
        }

        tokens.push(token.clone());

        let Some(next) = token.next_token() else {
            break;
        };
        token = next;
    }

    tokens
}

fn line_number(source: &str, offset: TextSize) -> usize {
    let offset = byte_offset(offset);
    source.as_bytes()[..offset].iter().filter(|&&byte| byte == b'\n').count() + 1
}

fn byte_offset(text_size: TextSize) -> usize {
    u32::from(text_size) as usize
}

fn format_report(corpus: &ResolvedCorpus, run: &HarnessRun) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Corpus: {}", corpus.label));
    lines.push(format!("Files checked: {}", run.files_checked));
    if run.files_skipped_with_parse_errors > 0 {
        lines.push(format!(
            "Skipped files with parse errors: {}",
            run.files_skipped_with_parse_errors
        ));
    }
    if run.files_skipped_with_formatter_errors > 0 {
        lines.push(format!(
            "Skipped files with formatter errors: {}",
            run.files_skipped_with_formatter_errors
        ));
    }
    if run.perturbations_skipped_with_formatter_errors > 0 {
        lines.push(format!(
            "Skipped perturbations with formatter errors: {}",
            run.perturbations_skipped_with_formatter_errors
        ));
    }
    lines.push(format!("Eligible multiline blocks: {}", run.eligible_blocks));
    lines.push(format!("Perturbations tested: {}", run.perturbations_tested));
    lines.push(format!("Converged: {}", run.perturbations_converged));
    lines.push(format!("Diverged: {}", run.perturbations_diverged));

    if run.divergent_cases.is_empty() {
        lines.push("All tested perturbations converged to the same formatted output.".into());
    } else {
        lines.push(String::new());
        lines.push("Divergent cases:".into());
        for case in &run.divergent_cases {
            lines.push(format!(
                "  - {}: {} lines {}-{} ({})",
                display_path(&case.file),
                case.block_kind,
                case.block_start_line,
                case.block_end_line,
                case.perturbation
            ));
        }
        if run.perturbations_diverged > run.divergent_cases.len() {
            lines.push(format!(
                "  - … and {} more",
                run.perturbations_diverged - run.divergent_cases.len()
            ));
        }
    }

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
    fn generates_indentation_perturbations_for_multiline_blocks() {
        let source = include_str!("../../../../tests/cases/examples/color.slint");

        let (_, perturbations) = generate_perturbations(source);

        assert!(!perturbations.is_empty());
        assert!(
            perturbations.iter().any(|(_, kind, _)| matches!(kind, PerturbationKind::AddSpaces))
        );
        assert!(
            perturbations
                .iter()
                .any(|(_, kind, _)| matches!(kind, PerturbationKind::StripIndentation))
        );
    }

    #[test]
    fn perturbations_keep_brace_structure_intact() {
        let source = include_str!("../../../../tests/cases/examples/color.slint");
        let (_, perturbations) = generate_perturbations(source);

        for (_, _, perturbed) in perturbations {
            let normalized_original = source.split_whitespace().collect::<String>();
            let normalized_perturbed = perturbed.split_whitespace().collect::<String>();
            assert_eq!(normalized_perturbed, normalized_original);
        }
    }
}
