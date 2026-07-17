// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Check whether indentation-noisy multiline blocks converge to the same
//! formatted output, ported from the Topiary-based formatter's
//! `format-canonicalization` harness.

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::parser::{self, SyntaxKind, SyntaxNode, SyntaxToken, TextSize};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::PathBuf;

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

struct DivergentCase {
    file: PathBuf,
    block_kind: &'static str,
    block_start_line: usize,
    block_end_line: usize,
    perturbation: &'static str,
}

#[derive(Default)]
struct CanonicalizationRun {
    files_checked: usize,
    files_skipped_with_parse_errors: usize,
    eligible_blocks: usize,
    perturbations_tested: usize,
    perturbations_skipped_with_parse_errors: usize,
    perturbations_converged: usize,
    perturbations_diverged: usize,
    divergent_cases: Vec<DivergentCase>,
}

pub fn run(corpus_label: &str, files: &[PathBuf]) -> io::Result<String> {
    let mut run = CanonicalizationRun::default();

    for file in files {
        run.files_checked += 1;
        let source = fs::read_to_string(file)?;

        let Some(baseline) = super::format_source(&source) else {
            run.files_skipped_with_parse_errors += 1;
            continue;
        };

        let (eligible_blocks, perturbations) = generate_perturbations(&source);
        run.eligible_blocks += eligible_blocks;

        for (region, perturbation, perturbed_source) in perturbations {
            run.perturbations_tested += 1;
            let Some(formatted) = super::format_source(&perturbed_source) else {
                run.perturbations_skipped_with_parse_errors += 1;
                continue;
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

    Ok(format_report(corpus_label, &run))
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
            let (leading_whitespace, rest) = split_leading_whitespace(without_ending);
            if rest.is_empty() {
                output.push_str(line);
                continue;
            }

            match kind {
                PerturbationKind::AddSpaces => {
                    output.push_str("  ");
                    output.push_str(leading_whitespace);
                    output.push_str(rest);
                    changed = true;
                }
                PerturbationKind::StripIndentation => {
                    if !leading_whitespace.is_empty() {
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
    let first_non_whitespace = line
        .char_indices()
        .find_map(|(index, character)| (!character.is_whitespace()).then_some(index))
        .unwrap_or(line.len());
    line.split_at(first_non_whitespace)
}

fn first_token_of_kind(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens_in_node(node).find(|token| token.kind() == kind)
}

fn last_token_of_kind(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens_in_node(node).filter(|token| token.kind() == kind).last()
}

fn tokens_in_node(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    let end = node.text_range().end();
    std::iter::successors(node.first_token(), |token| token.next_token())
        .take_while(move |token| token.text_range().start() < end)
}

fn line_number(source: &str, offset: TextSize) -> usize {
    let offset = byte_offset(offset);
    source.as_bytes()[..offset].iter().filter(|&&byte| byte == b'\n').count() + 1
}

fn byte_offset(text_size: TextSize) -> usize {
    u32::from(text_size) as usize
}

fn format_report(corpus_label: &str, run: &CanonicalizationRun) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Corpus: {corpus_label}"));
    lines.push(format!("Files checked: {}", run.files_checked));
    if run.files_skipped_with_parse_errors > 0 {
        lines.push(format!(
            "Skipped files with parse errors: {}",
            run.files_skipped_with_parse_errors
        ));
    }
    if run.perturbations_skipped_with_parse_errors > 0 {
        lines.push(format!(
            "Skipped perturbations with parse errors: {}",
            run.perturbations_skipped_with_parse_errors
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
                case.file.display(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_indentation_perturbations_for_multiline_blocks() {
        let source = include_str!("../../../tests/cases/examples/color.slint");

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
        let source = include_str!("../../../tests/cases/examples/color.slint");
        let (_, perturbations) = generate_perturbations(source);

        for (_, _, perturbed) in perturbations {
            let normalized_original = source.split_whitespace().collect::<String>();
            let normalized_perturbed = perturbed.split_whitespace().collect::<String>();
            assert_eq!(normalized_perturbed, normalized_original);
        }
    }
}
