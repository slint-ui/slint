// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::parser::{self, SyntaxKind, SyntaxNode, SyntaxToken};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StyleDecisionKind {
    BindingColonSpacing,
    TypeAnnotationColonSpacing,
    ConditionalColonSpacing,
    DeclarationColonEqualSpacing,
    TwoWayBindingSpacing,
    CallbackArrowSpacing,
    ElementBracePlacement,
    SimpleCodeBlockLayout,
    SimpleObjectLiteralLayout,
    TopLevelBlankLines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IndentationDecisionKind {
    MultilineBlockChildIndentation,
    MultilineBlockClosingBraceAlignment,
    MultilineBlockSiblingIndentSpread,
}

impl StyleDecisionKind {
    pub const ALL: [Self; 10] = [
        Self::BindingColonSpacing,
        Self::TypeAnnotationColonSpacing,
        Self::ConditionalColonSpacing,
        Self::DeclarationColonEqualSpacing,
        Self::TwoWayBindingSpacing,
        Self::CallbackArrowSpacing,
        Self::ElementBracePlacement,
        Self::SimpleCodeBlockLayout,
        Self::SimpleObjectLiteralLayout,
        Self::TopLevelBlankLines,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::BindingColonSpacing => "binding ':' spacing",
            Self::TypeAnnotationColonSpacing => "type annotation ':' spacing",
            Self::ConditionalColonSpacing => "conditional ':' spacing",
            Self::DeclarationColonEqualSpacing => "declaration ':=' spacing",
            Self::TwoWayBindingSpacing => "two-way binding '<=>' spacing",
            Self::CallbackArrowSpacing => "callback '=>' spacing",
            Self::ElementBracePlacement => "element opening brace placement",
            Self::SimpleCodeBlockLayout => "simple code block layout",
            Self::SimpleObjectLiteralLayout => "simple object literal layout",
            Self::TopLevelBlankLines => "blank lines between top-level definitions",
        }
    }

    pub fn choice_label(self, choice: StyleChoice) -> &'static str {
        match self {
            Self::BindingColonSpacing
            | Self::TypeAnnotationColonSpacing
            | Self::ConditionalColonSpacing
            | Self::DeclarationColonEqualSpacing
            | Self::TwoWayBindingSpacing
            | Self::CallbackArrowSpacing => choice.operator_spacing_label(),
            Self::ElementBracePlacement => match choice {
                StyleChoice::SameLine => "same line",
                StyleChoice::NextLine => "next line",
                _ => "n/a",
            },
            Self::SimpleCodeBlockLayout | Self::SimpleObjectLiteralLayout => match choice {
                StyleChoice::Inline => "inline",
                StyleChoice::Multiline => "multiline",
                _ => "n/a",
            },
            Self::TopLevelBlankLines => match choice {
                StyleChoice::NoBlankLines => "no blank lines",
                StyleChoice::OneBlankLine => "one blank line",
                StyleChoice::MultipleBlankLines => "multiple blank lines",
                _ => "n/a",
            },
        }
    }
}

impl IndentationDecisionKind {
    pub const ALL: [Self; 3] = [
        Self::MultilineBlockChildIndentation,
        Self::MultilineBlockClosingBraceAlignment,
        Self::MultilineBlockSiblingIndentSpread,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::MultilineBlockChildIndentation => "multiline block direct-child indentation",
            Self::MultilineBlockClosingBraceAlignment => "multiline block closing-brace alignment",
            Self::MultilineBlockSiblingIndentSpread => "multiline block sibling indent spread",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StyleChoice {
    NoSpaces,
    SpaceBefore,
    SpaceAfter,
    SpacesAround,
    SameLine,
    NextLine,
    Inline,
    Multiline,
    NoBlankLines,
    OneBlankLine,
    MultipleBlankLines,
}

impl StyleChoice {
    fn operator_spacing_label(self) -> &'static str {
        match self {
            Self::NoSpaces => "no spaces",
            Self::SpaceBefore => "space before only",
            Self::SpaceAfter => "space after only",
            Self::SpacesAround => "spaces around",
            _ => "n/a",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StyleDecision {
    pub kind: StyleDecisionKind,
    pub choice: StyleChoice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndentationObservation {
    pub kind: IndentationDecisionKind,
    pub delta: isize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStyleProfile {
    pub path: Option<PathBuf>,
    pub decisions: Vec<StyleDecision>,
    pub indentation_observations: Vec<IndentationObservation>,
    pub diagnostics: Vec<String>,
    pub has_parse_errors: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleDecisionSummary {
    pub kind: StyleDecisionKind,
    pub total_observations: usize,
    pub dominant_choice: StyleChoice,
    pub dominant_count: usize,
    pub counts: Vec<(StyleChoice, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleDecisionComparison {
    pub kind: StyleDecisionKind,
    pub reference: StyleDecisionSummary,
    pub candidate: Option<StyleDecisionSummary>,
    pub candidate_matches_reference_dominant: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentationObservationSummary {
    pub kind: IndentationDecisionKind,
    pub total_observations: usize,
    pub dominant_delta: isize,
    pub dominant_count: usize,
    pub counts: Vec<(isize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentationObservationComparison {
    pub kind: IndentationDecisionKind,
    pub reference: IndentationObservationSummary,
    pub candidate: Option<IndentationObservationSummary>,
    pub candidate_matches_reference_dominant: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryStyleComparison {
    pub reference_file_count: usize,
    pub candidate_file_count: usize,
    pub reference_files_with_parse_errors: usize,
    pub candidate_files_with_parse_errors: usize,
    pub decisions: Vec<StyleDecisionComparison>,
    pub indentation_observations: Vec<IndentationObservationComparison>,
    pub total_candidate_observations: usize,
    pub total_candidate_matches_reference_dominant: usize,
    pub total_candidate_indentation_observations: usize,
    pub total_candidate_matches_reference_indentation_dominant: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryStyleProfile {
    pub file_count: usize,
    pub files_with_parse_errors: usize,
    pub decision_counts: BTreeMap<StyleDecisionKind, BTreeMap<StyleChoice, usize>>,
    pub indentation_counts: BTreeMap<IndentationDecisionKind, BTreeMap<isize, usize>>,
}

impl RepositoryStyleProfile {
    pub fn summaries(&self) -> Vec<StyleDecisionSummary> {
        StyleDecisionKind::ALL
            .into_iter()
            .filter_map(|kind| {
                let counts = self.decision_counts.get(&kind)?;
                let total_observations = counts.values().sum();
                let mut dominant_choice = None;
                let mut dominant_count = 0;

                for (&choice, &count) in counts {
                    if count > dominant_count {
                        dominant_choice = Some(choice);
                        dominant_count = count;
                    }
                }

                Some(StyleDecisionSummary {
                    kind,
                    total_observations,
                    dominant_choice: dominant_choice.expect("counts should not be empty"),
                    dominant_count,
                    counts: counts.iter().map(|(&choice, &count)| (choice, count)).collect(),
                })
            })
            .collect()
    }

    pub fn indentation_summaries(&self) -> Vec<IndentationObservationSummary> {
        IndentationDecisionKind::ALL
            .into_iter()
            .filter_map(|kind| {
                let counts = self.indentation_counts.get(&kind)?;
                let total_observations = counts.values().sum();
                let mut dominant_delta = None;
                let mut dominant_count = 0;

                for (&delta, &count) in counts {
                    if count > dominant_count {
                        dominant_delta = Some(delta);
                        dominant_count = count;
                    }
                }

                Some(IndentationObservationSummary {
                    kind,
                    total_observations,
                    dominant_delta: dominant_delta.expect("counts should not be empty"),
                    dominant_count,
                    counts: counts.iter().map(|(&delta, &count)| (delta, count)).collect(),
                })
            })
            .collect()
    }
}

pub fn profile_source(source: &str) -> FileStyleProfile {
    profile_source_with_path(source, None)
}

pub fn profile_source_with_path(source: &str, path: Option<&Path>) -> FileStyleProfile {
    let mut diagnostics = BuildDiagnostics::default();
    let document = parser::parse(source.to_owned(), path, &mut diagnostics);
    let mut decisions = Vec::new();

    decisions.extend(extract_operator_spacing(source, &document));
    decisions.extend(extract_element_brace_placement(source, &document));
    decisions.extend(extract_simple_block_layouts(source, &document));
    decisions.extend(extract_top_level_blank_lines(source, &document));
    let indentation_observations = extract_indentation_observations(source, &document);

    FileStyleProfile {
        path: path.map(Path::to_owned),
        decisions,
        indentation_observations,
        diagnostics: diagnostics.to_string_vec(),
        has_parse_errors: diagnostics.has_errors(),
    }
}

pub fn profile_file(path: impl AsRef<Path>) -> io::Result<FileStyleProfile> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)?;
    Ok(profile_source_with_path(&source, Some(path)))
}

pub fn aggregate_repository_profile(files: &[FileStyleProfile]) -> RepositoryStyleProfile {
    let mut decision_counts = BTreeMap::<StyleDecisionKind, BTreeMap<StyleChoice, usize>>::new();
    let mut indentation_counts = BTreeMap::<IndentationDecisionKind, BTreeMap<isize, usize>>::new();

    for file in files {
        for decision in &file.decisions {
            *decision_counts
                .entry(decision.kind)
                .or_default()
                .entry(decision.choice)
                .or_default() += 1;
        }
        for observation in &file.indentation_observations {
            *indentation_counts
                .entry(observation.kind)
                .or_default()
                .entry(observation.delta)
                .or_default() += 1;
        }
    }

    RepositoryStyleProfile {
        file_count: files.len(),
        files_with_parse_errors: files.iter().filter(|file| file.has_parse_errors).count(),
        decision_counts,
        indentation_counts,
    }
}

pub fn format_repository_style_report(profile: &RepositoryStyleProfile) -> String {
    let mut report = Vec::new();
    report.push(format!("Analyzed {} standalone .slint file(s)", profile.file_count));

    if profile.files_with_parse_errors > 0 {
        report.push(format!("Files with parse errors: {}", profile.files_with_parse_errors));
    }

    let summaries = profile.summaries();
    if summaries.is_empty() {
        report.push("No style decisions observed.".into());
    } else {
        for summary in summaries {
            report.push(String::new());
            report.push(summary.kind.label().to_string());
            report.push(format!(
                "  dominant: {} ({}/{})",
                summary.kind.choice_label(summary.dominant_choice),
                summary.dominant_count,
                summary.total_observations
            ));

            for (choice, count) in summary.counts {
                report.push(format!("  - {}: {count}", summary.kind.choice_label(choice)));
            }
        }
    }

    let indentation_summaries = profile.indentation_summaries();
    if !indentation_summaries.is_empty() {
        for summary in indentation_summaries {
            report.push(String::new());
            report.push(summary.kind.label().to_string());
            report.push(format!(
                "  dominant: {} ({}/{})",
                format_indent_delta(summary.dominant_delta),
                summary.dominant_count,
                summary.total_observations
            ));

            for (delta, count) in summary.counts {
                report.push(format!("  - {}: {count}", format_indent_delta(delta)));
            }
        }
    }

    report.join("\n")
}

pub fn compare_repository_profiles(
    reference: &RepositoryStyleProfile,
    candidate: &RepositoryStyleProfile,
) -> RepositoryStyleComparison {
    let candidate_summaries = candidate
        .summaries()
        .into_iter()
        .map(|summary| (summary.kind, summary))
        .collect::<BTreeMap<_, _>>();
    let candidate_indentation_summaries = candidate
        .indentation_summaries()
        .into_iter()
        .map(|summary| (summary.kind, summary))
        .collect::<BTreeMap<_, _>>();

    let mut decisions = Vec::new();
    let mut indentation_observations = Vec::new();
    let mut total_candidate_observations = 0;
    let mut total_candidate_matches_reference_dominant = 0;
    let mut total_candidate_indentation_observations = 0;
    let mut total_candidate_matches_reference_indentation_dominant = 0;

    for reference_summary in reference.summaries() {
        let candidate_summary = candidate_summaries.get(&reference_summary.kind).cloned();
        let candidate_matches_reference_dominant = candidate_summary
            .as_ref()
            .and_then(|summary| {
                summary
                    .counts
                    .iter()
                    .find(|(choice, _)| *choice == reference_summary.dominant_choice)
                    .map(|(_, count)| *count)
            })
            .unwrap_or(0);

        total_candidate_observations +=
            candidate_summary.as_ref().map_or(0, |summary| summary.total_observations);
        total_candidate_matches_reference_dominant += candidate_matches_reference_dominant;

        decisions.push(StyleDecisionComparison {
            kind: reference_summary.kind,
            reference: reference_summary,
            candidate: candidate_summary,
            candidate_matches_reference_dominant,
        });
    }

    for reference_summary in reference.indentation_summaries() {
        let candidate_summary =
            candidate_indentation_summaries.get(&reference_summary.kind).cloned();
        let candidate_matches_reference_dominant = candidate_summary
            .as_ref()
            .and_then(|summary| {
                summary
                    .counts
                    .iter()
                    .find(|(delta, _)| *delta == reference_summary.dominant_delta)
                    .map(|(_, count)| *count)
            })
            .unwrap_or(0);

        total_candidate_indentation_observations +=
            candidate_summary.as_ref().map_or(0, |summary| summary.total_observations);
        total_candidate_matches_reference_indentation_dominant +=
            candidate_matches_reference_dominant;

        indentation_observations.push(IndentationObservationComparison {
            kind: reference_summary.kind,
            reference: reference_summary,
            candidate: candidate_summary,
            candidate_matches_reference_dominant,
        });
    }

    RepositoryStyleComparison {
        reference_file_count: reference.file_count,
        candidate_file_count: candidate.file_count,
        reference_files_with_parse_errors: reference.files_with_parse_errors,
        candidate_files_with_parse_errors: candidate.files_with_parse_errors,
        decisions,
        indentation_observations,
        total_candidate_observations,
        total_candidate_matches_reference_dominant,
        total_candidate_indentation_observations,
        total_candidate_matches_reference_indentation_dominant,
    }
}

pub fn format_repository_style_comparison_report(comparison: &RepositoryStyleComparison) -> String {
    let mut report = Vec::new();
    let total_reference_observations: usize =
        comparison.decisions.iter().map(|decision| decision.reference.total_observations).sum();
    report.push(format!(
        "Reference files: {}, formatter output files: {}",
        comparison.reference_file_count, comparison.candidate_file_count
    ));

    if comparison.reference_files_with_parse_errors > 0 {
        report.push(format!(
            "Reference files with parse errors: {}",
            comparison.reference_files_with_parse_errors
        ));
    }

    if comparison.candidate_files_with_parse_errors > 0 {
        report.push(format!(
            "Formatter output files with parse errors: {}",
            comparison.candidate_files_with_parse_errors
        ));
    }

    if comparison.decisions.is_empty() && comparison.indentation_observations.is_empty() {
        report.push("No comparable style decisions observed.".into());
        return report.join("\n");
    }

    if !comparison.decisions.is_empty() {
        report.push(format!("Reference observations: {total_reference_observations}"));
        report.push(format!("Formatter observations: {}", comparison.total_candidate_observations));
        report.push(format!(
            "Overall formatter alignment with reference dominant choices: {}",
            format_fraction(
                comparison.total_candidate_matches_reference_dominant,
                comparison.total_candidate_observations
            )
        ));
        if comparison.total_candidate_observations == 0 && total_reference_observations > 0 {
            report.push("Formatter output produced no comparable observations.".into());
        }
    }

    if !comparison.indentation_observations.is_empty() {
        let total_reference_indentation_observations: usize = comparison
            .indentation_observations
            .iter()
            .map(|decision| decision.reference.total_observations)
            .sum();
        report.push(format!(
            "Reference indentation observations: {total_reference_indentation_observations}"
        ));
        report.push(format!(
            "Formatter indentation observations: {}",
            comparison.total_candidate_indentation_observations
        ));
        report.push(format!(
            "Overall formatter alignment with reference dominant indentation deltas: {}",
            format_fraction(
                comparison.total_candidate_matches_reference_indentation_dominant,
                comparison.total_candidate_indentation_observations
            )
        ));
        report.push("indentation observations:".into());
    }

    for decision in &comparison.decisions {
        report.push(String::new());
        report.push(decision.kind.label().to_string());
        report.push(format!("  reference observations: {}", decision.reference.total_observations));
        report.push(format!(
            "  formatter observations: {}",
            decision.candidate.as_ref().map_or(0, |summary| summary.total_observations)
        ));
        report
            .push(format!("  reference dominant: {}", format_dominant_choice(&decision.reference)));
        report
            .push(format!("  formatter dominant: {}", format_candidate_dominant_choice(decision)));
        report.push(format!(
            "  formatter matches reference dominant: {}",
            format_fraction(
                decision.candidate_matches_reference_dominant,
                decision.candidate.as_ref().map_or(0, |summary| summary.total_observations)
            )
        ));
        report.push("  reference counts:".into());
        for (choice, count) in &decision.reference.counts {
            report.push(format!("    - {}: {count}", decision.kind.choice_label(*choice)));
        }

        report.push("  formatter counts:".into());
        if let Some(candidate) = &decision.candidate {
            for (choice, count) in &candidate.counts {
                report.push(format!("    - {}: {count}", decision.kind.choice_label(*choice)));
            }
        } else {
            report.push("    - not observed".into());
        }
    }

    for decision in &comparison.indentation_observations {
        report.push(String::new());
        report.push(decision.kind.label().to_string());
        report.push(format!("  reference observations: {}", decision.reference.total_observations));
        report.push(format!(
            "  formatter observations: {}",
            decision.candidate.as_ref().map_or(0, |summary| summary.total_observations)
        ));
        report.push(format!(
            "  reference dominant: {}",
            format_indentation_summary_choice(&decision.reference)
        ));
        report
            .push(format!("  formatter dominant: {}", format_candidate_indent_dominant(decision)));
        report.push(format!(
            "  formatter matches reference dominant: {}",
            format_fraction(
                decision.candidate_matches_reference_dominant,
                decision.candidate.as_ref().map_or(0, |summary| summary.total_observations)
            )
        ));
        report.push("  reference counts:".into());
        for (delta, count) in &decision.reference.counts {
            report.push(format!("    - {}: {count}", format_indent_delta(*delta)));
        }

        report.push("  formatter counts:".into());
        if let Some(candidate) = &decision.candidate {
            for (delta, count) in &candidate.counts {
                report.push(format!("    - {}: {count}", format_indent_delta(*delta)));
            }
        } else {
            report.push("    - not observed".into());
        }
    }

    report.join("\n")
}

pub fn collect_standalone_slint_files(paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let roots = if paths.is_empty() { vec![std::env::current_dir()?] } else { paths.to_vec() };
    let mut files = BTreeSet::new();

    for root in roots {
        collect_path(&root, &mut files)?;
    }

    Ok(files.into_iter().collect())
}

fn collect_path(path: &Path, files: &mut BTreeSet<PathBuf>) -> io::Result<()> {
    if path.is_dir() {
        let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.path());

        for entry in entries {
            collect_path(&entry.path(), files)?;
        }
    } else if path.extension().and_then(|extension| extension.to_str()) == Some("slint") {
        files.insert(path.to_owned());
    }

    Ok(())
}

fn extract_operator_spacing(source: &str, document: &SyntaxNode) -> Vec<StyleDecision> {
    let mut decisions = Vec::new();

    for token in tokens_in_node(document) {
        let kind = match token.kind() {
            SyntaxKind::Colon => classify_colon_spacing(&token),
            SyntaxKind::ColonEqual => Some(StyleDecisionKind::DeclarationColonEqualSpacing),
            SyntaxKind::DoubleArrow => Some(StyleDecisionKind::TwoWayBindingSpacing),
            SyntaxKind::FatArrow => Some(StyleDecisionKind::CallbackArrowSpacing),
            _ => None,
        };

        let Some(kind) = kind else {
            continue;
        };

        let Some(choice) = operator_spacing_choice(source, &token) else {
            continue;
        };

        decisions.push(StyleDecision { kind, choice });
    }

    decisions
}

fn classify_colon_spacing(token: &SyntaxToken) -> Option<StyleDecisionKind> {
    for ancestor in token.parent_ancestors() {
        let kind = match ancestor.kind() {
            SyntaxKind::ConditionalExpression => StyleDecisionKind::ConditionalColonSpacing,
            SyntaxKind::Binding
            | SyntaxKind::PropertyDeclaration
            | SyntaxKind::StatePropertyChange
            | SyntaxKind::ObjectMember => StyleDecisionKind::BindingColonSpacing,
            SyntaxKind::CallbackDeclarationParameter
            | SyntaxKind::ArgumentDeclaration
            | SyntaxKind::ObjectTypeMember
            | SyntaxKind::LetStatement => StyleDecisionKind::TypeAnnotationColonSpacing,
            _ => continue,
        };

        return Some(kind);
    }

    None
}

fn extract_element_brace_placement(source: &str, document: &SyntaxNode) -> Vec<StyleDecision> {
    document
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Element)
        .filter_map(|node| {
            opening_brace_placement(source, &node).map(|choice| StyleDecision {
                kind: StyleDecisionKind::ElementBracePlacement,
                choice,
            })
        })
        .collect()
}

fn extract_simple_block_layouts(source: &str, document: &SyntaxNode) -> Vec<StyleDecision> {
    let mut decisions = Vec::new();

    for node in document.descendants() {
        match node.kind() {
            SyntaxKind::CodeBlock if is_simple_code_block(&node) => {
                if let Some(choice) = inline_or_multiline_layout(source, &node) {
                    decisions.push(StyleDecision {
                        kind: StyleDecisionKind::SimpleCodeBlockLayout,
                        choice,
                    });
                }
            }
            SyntaxKind::ObjectLiteral if is_simple_object_literal(&node) => {
                if let Some(choice) = inline_or_multiline_layout(source, &node) {
                    decisions.push(StyleDecision {
                        kind: StyleDecisionKind::SimpleObjectLiteralLayout,
                        choice,
                    });
                }
            }
            _ => {}
        }
    }

    decisions
}

fn extract_top_level_blank_lines(source: &str, document: &SyntaxNode) -> Vec<StyleDecision> {
    let top_level_definitions: Vec<_> = document
        .children()
        .filter(|node| {
            matches!(
                node.kind(),
                SyntaxKind::Component
                    | SyntaxKind::ExportsList
                    | SyntaxKind::ImportSpecifier
                    | SyntaxKind::StructDeclaration
                    | SyntaxKind::EnumDeclaration
            )
        })
        .collect();

    let mut decisions = Vec::new();

    for pair in top_level_definitions.windows(2) {
        let previous_end = offset_from_text_size(pair[0].text_range().end());
        let next_start = offset_from_text_size(pair[1].text_range().start());
        let blank_lines = count_blank_lines_between(&source[previous_end..next_start]);
        let choice = match blank_lines {
            0 => StyleChoice::NoBlankLines,
            1 => StyleChoice::OneBlankLine,
            _ => StyleChoice::MultipleBlankLines,
        };

        decisions.push(StyleDecision { kind: StyleDecisionKind::TopLevelBlankLines, choice });
    }

    decisions
}

fn extract_indentation_observations(
    source: &str,
    document: &SyntaxNode,
) -> Vec<IndentationObservation> {
    let mut observations = Vec::new();

    for node in document.descendants() {
        match node.kind() {
            SyntaxKind::CodeBlock | SyntaxKind::ObjectLiteral | SyntaxKind::Element => {}
            _ => continue,
        }

        let Some(opening_brace) = first_token_of_kind(&node, SyntaxKind::LBrace) else {
            continue;
        };
        let Some(closing_brace) = last_token_of_kind(&node, SyntaxKind::RBrace) else {
            continue;
        };

        let opening_line = line_number(source, opening_brace.text_range().start());
        let closing_line = line_number(source, closing_brace.text_range().start());
        if opening_line == closing_line {
            continue;
        }

        let block_indent = line_indent_width(source, opening_brace.text_range().start()) as isize;
        let mut child_indents = Vec::new();

        for child in node.children() {
            let Some(first_token) = child.first_token() else {
                continue;
            };

            let child_line = line_number(source, first_token.text_range().start());
            if child_line <= opening_line || child_line >= closing_line {
                continue;
            }

            let child_indent = line_indent_width(source, first_token.text_range().start()) as isize;
            child_indents.push(child_indent);
            observations.push(IndentationObservation {
                kind: IndentationDecisionKind::MultilineBlockChildIndentation,
                delta: child_indent - block_indent,
            });
        }

        observations.push(IndentationObservation {
            kind: IndentationDecisionKind::MultilineBlockClosingBraceAlignment,
            delta: line_indent_width(source, closing_brace.text_range().start()) as isize
                - block_indent,
        });

        if child_indents.len() >= 2 {
            let spread = child_indents.iter().max().unwrap() - child_indents.iter().min().unwrap();
            observations.push(IndentationObservation {
                kind: IndentationDecisionKind::MultilineBlockSiblingIndentSpread,
                delta: spread,
            });
        }
    }

    observations
}

fn operator_spacing_choice(source: &str, token: &SyntaxToken) -> Option<StyleChoice> {
    let previous = previous_non_trivia_token(token)?;
    let next = next_non_trivia_token(token)?;

    let before = &source[offset_from_text_size(previous.text_range().end())
        ..offset_from_text_size(token.text_range().start())];
    let after = &source[offset_from_text_size(token.text_range().end())
        ..offset_from_text_size(next.text_range().start())];

    let has_space_before = horizontal_spaces_only(before)?;
    let has_space_after = horizontal_spaces_only(after)?;

    Some(match (has_space_before, has_space_after) {
        (false, false) => StyleChoice::NoSpaces,
        (true, false) => StyleChoice::SpaceBefore,
        (false, true) => StyleChoice::SpaceAfter,
        (true, true) => StyleChoice::SpacesAround,
    })
}

fn opening_brace_placement(source: &str, node: &SyntaxNode) -> Option<StyleChoice> {
    let opening_brace = first_token_of_kind(node, SyntaxKind::LBrace)?;
    let previous = previous_non_trivia_token(&opening_brace)?;

    Some(
        if line_number(source, previous.text_range().end())
            == line_number(source, opening_brace.text_range().start())
        {
            StyleChoice::SameLine
        } else {
            StyleChoice::NextLine
        },
    )
}

fn inline_or_multiline_layout(source: &str, node: &SyntaxNode) -> Option<StyleChoice> {
    let opening_brace = first_token_of_kind(node, SyntaxKind::LBrace)?;
    let closing_brace = last_token_of_kind(node, SyntaxKind::RBrace)?;

    Some(
        if line_number(source, opening_brace.text_range().start())
            == line_number(source, closing_brace.text_range().start())
        {
            StyleChoice::Inline
        } else {
            StyleChoice::Multiline
        },
    )
}

fn is_simple_code_block(node: &SyntaxNode) -> bool {
    node.children().count() <= 1
}

fn is_simple_object_literal(node: &SyntaxNode) -> bool {
    node.children().count() <= 1
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

    let end = offset_from_text_size(node.text_range().end());
    let mut tokens = Vec::new();

    loop {
        if offset_from_text_size(token.text_range().start()) >= end {
            break;
        }

        tokens.push(token.clone());

        let Some(next) = token.next_token() else {
            break;
        };
        if offset_from_text_size(next.text_range().start()) >= end {
            break;
        }
        token = next;
    }

    tokens
}

fn previous_non_trivia_token(token: &SyntaxToken) -> Option<SyntaxToken> {
    let mut current = token.prev_token();

    while let Some(candidate) = current {
        if !is_trivia(candidate.kind()) {
            return Some(candidate);
        }
        current = candidate.prev_token();
    }

    None
}

fn next_non_trivia_token(token: &SyntaxToken) -> Option<SyntaxToken> {
    let mut current = token.next_token();

    while let Some(candidate) = current {
        if !is_trivia(candidate.kind()) {
            return Some(candidate);
        }
        current = candidate.next_token();
    }

    None
}

fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment)
}

fn horizontal_spaces_only(text: &str) -> Option<bool> {
    if text.is_empty() {
        return Some(false);
    }

    if text.bytes().all(|byte| matches!(byte, b' ' | b'\t')) {
        return Some(true);
    }

    None
}

fn count_blank_lines_between(text: &str) -> usize {
    let parts: Vec<_> = text.split('\n').collect();
    if parts.len() < 3 {
        return 0;
    }

    parts[1..parts.len() - 1].iter().filter(|part| part.trim().is_empty()).count()
}

fn line_indent_width(source: &str, offset: i_slint_compiler::parser::TextSize) -> usize {
    let offset = offset_from_text_size(offset);
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    source[line_start..offset]
        .chars()
        .take_while(|character| matches!(character, ' ' | '\t'))
        .map(|character| if character == '\t' { 4 } else { 1 })
        .sum()
}

fn line_number(source: &str, offset: i_slint_compiler::parser::TextSize) -> usize {
    let offset = offset_from_text_size(offset);
    source.as_bytes()[..offset].iter().filter(|byte| **byte == b'\n').count() + 1
}

fn offset_from_text_size(size: i_slint_compiler::parser::TextSize) -> usize {
    u32::from(size) as usize
}

fn format_dominant_choice(summary: &StyleDecisionSummary) -> String {
    format!(
        "{} ({}, {:.1}%)",
        summary.kind.choice_label(summary.dominant_choice),
        format_ratio(summary.dominant_count, summary.total_observations),
        percentage(summary.dominant_count, summary.total_observations)
    )
}

fn format_candidate_dominant_choice(decision: &StyleDecisionComparison) -> String {
    let Some(candidate) = &decision.candidate else {
        return "not observed".into();
    };

    format_dominant_choice(candidate)
}

fn format_candidate_indent_dominant(decision: &IndentationObservationComparison) -> String {
    let Some(candidate) = &decision.candidate else {
        return "not observed".into();
    };

    format!(
        "{} ({}/{})",
        format_indent_delta(candidate.dominant_delta),
        candidate.dominant_count,
        candidate.total_observations
    )
}

fn format_indentation_summary_choice(summary: &IndentationObservationSummary) -> String {
    format!(
        "{} ({}/{})",
        format_indent_delta(summary.dominant_delta),
        summary.dominant_count,
        summary.total_observations
    )
}

fn format_fraction(count: usize, total: usize) -> String {
    if total == 0 {
        return "0/0 (n/a)".into();
    }

    format!("{count}/{total} ({:.1}%)", percentage(count, total))
}

fn format_ratio(count: usize, total: usize) -> String {
    format!("{count}/{total}")
}

fn percentage(count: usize, total: usize) -> f64 {
    if total == 0 { 0.0 } else { (count as f64 / total as f64) * 100.0 }
}

fn format_indent_delta(delta: isize) -> String {
    match delta {
        0 => "aligned".into(),
        delta if delta > 0 => format!("+{} spaces", delta),
        delta => format!("{delta} spaces"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_local_spacing_and_layout_decisions() {
        let source = r#"
Helper := Text { text: "ready"; }

export component Example inherits Window {
    in property <int> value: 42;
    callback ping(foo : int);
    ping => { value = 1; }
    Text { text <=> value; }
    out property <brush> accent: value ? red : blue;
    property <brush> brush: { color: red };
}
"#;

        let profile = profile_source(source);

        assert!(!profile.has_parse_errors, "{:?}", profile.diagnostics);
        assert!(profile.decisions.contains(&StyleDecision {
            kind: StyleDecisionKind::DeclarationColonEqualSpacing,
            choice: StyleChoice::SpacesAround,
        }));
        assert!(profile.decisions.contains(&StyleDecision {
            kind: StyleDecisionKind::BindingColonSpacing,
            choice: StyleChoice::SpaceAfter,
        }));
        assert!(profile.decisions.contains(&StyleDecision {
            kind: StyleDecisionKind::TypeAnnotationColonSpacing,
            choice: StyleChoice::SpacesAround,
        }));
        assert!(profile.decisions.contains(&StyleDecision {
            kind: StyleDecisionKind::ConditionalColonSpacing,
            choice: StyleChoice::SpacesAround,
        }));
        assert!(profile.decisions.contains(&StyleDecision {
            kind: StyleDecisionKind::CallbackArrowSpacing,
            choice: StyleChoice::SpacesAround,
        }));
        assert!(profile.decisions.contains(&StyleDecision {
            kind: StyleDecisionKind::TwoWayBindingSpacing,
            choice: StyleChoice::SpacesAround,
        }));
        assert!(profile.decisions.contains(&StyleDecision {
            kind: StyleDecisionKind::ElementBracePlacement,
            choice: StyleChoice::SameLine,
        }));
        assert!(profile.decisions.contains(&StyleDecision {
            kind: StyleDecisionKind::SimpleCodeBlockLayout,
            choice: StyleChoice::Inline,
        }));
        assert!(profile.decisions.contains(&StyleDecision {
            kind: StyleDecisionKind::SimpleObjectLiteralLayout,
            choice: StyleChoice::Inline,
        }));
    }

    #[test]
    fn profiles_multiline_block_indentation_observations() {
        let source = r#"
export component Example inherits Window {
    ping => {
            foo();
            bar();
    }
}
"#;

        let profile = profile_source(source);

        assert!(!profile.has_parse_errors, "{:?}", profile.diagnostics);
        assert!(profile.indentation_observations.contains(&IndentationObservation {
            kind: IndentationDecisionKind::MultilineBlockChildIndentation,
            delta: 8,
        }));
        assert!(profile.indentation_observations.contains(&IndentationObservation {
            kind: IndentationDecisionKind::MultilineBlockClosingBraceAlignment,
            delta: 0,
        }));
        assert!(profile.indentation_observations.contains(&IndentationObservation {
            kind: IndentationDecisionKind::MultilineBlockSiblingIndentSpread,
            delta: 0,
        }));
    }

    #[test]
    fn profiles_blank_lines_between_top_level_definitions() {
        let source = r#"
import { Button } from "std-widgets.slint";

export component First inherits Rectangle {}


export component Second inherits Rectangle {}
"#;

        let profile = profile_source(source);
        let blank_line_choices: Vec<_> = profile
            .decisions
            .iter()
            .filter(|decision| decision.kind == StyleDecisionKind::TopLevelBlankLines)
            .map(|decision| decision.choice)
            .collect();

        assert_eq!(
            blank_line_choices,
            vec![StyleChoice::OneBlankLine, StyleChoice::MultipleBlankLines]
        );
    }

    #[test]
    fn aggregates_profiles_and_formats_a_report() {
        let repository = aggregate_repository_profile(&[
            FileStyleProfile {
                path: None,
                diagnostics: Vec::new(),
                has_parse_errors: false,
                indentation_observations: Vec::new(),
                decisions: vec![
                    StyleDecision {
                        kind: StyleDecisionKind::BindingColonSpacing,
                        choice: StyleChoice::SpaceAfter,
                    },
                    StyleDecision {
                        kind: StyleDecisionKind::BindingColonSpacing,
                        choice: StyleChoice::SpaceAfter,
                    },
                ],
            },
            FileStyleProfile {
                path: None,
                diagnostics: Vec::new(),
                has_parse_errors: true,
                indentation_observations: Vec::new(),
                decisions: vec![StyleDecision {
                    kind: StyleDecisionKind::BindingColonSpacing,
                    choice: StyleChoice::NoSpaces,
                }],
            },
        ]);

        let summaries = repository.summaries();
        assert_eq!(repository.file_count, 2);
        assert_eq!(repository.files_with_parse_errors, 1);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].dominant_choice, StyleChoice::SpaceAfter);
        assert_eq!(summaries[0].dominant_count, 2);

        let report = format_repository_style_report(&repository);
        assert!(report.contains("Analyzed 2 standalone .slint file(s)"));
        assert!(report.contains("Files with parse errors: 1"));
        assert!(report.contains("binding ':' spacing"));
        assert!(report.contains("dominant: space after only (2/3)"));
    }

    #[test]
    fn compares_candidate_profile_with_reference_dominant_choices() {
        let reference = aggregate_repository_profile(&[
            FileStyleProfile {
                path: None,
                diagnostics: Vec::new(),
                has_parse_errors: false,
                indentation_observations: Vec::new(),
                decisions: vec![
                    StyleDecision {
                        kind: StyleDecisionKind::BindingColonSpacing,
                        choice: StyleChoice::SpaceAfter,
                    },
                    StyleDecision {
                        kind: StyleDecisionKind::BindingColonSpacing,
                        choice: StyleChoice::SpaceAfter,
                    },
                    StyleDecision {
                        kind: StyleDecisionKind::BindingColonSpacing,
                        choice: StyleChoice::NoSpaces,
                    },
                ],
            },
            FileStyleProfile {
                path: None,
                diagnostics: Vec::new(),
                has_parse_errors: false,
                indentation_observations: Vec::new(),
                decisions: vec![StyleDecision {
                    kind: StyleDecisionKind::ElementBracePlacement,
                    choice: StyleChoice::SameLine,
                }],
            },
        ]);
        let candidate = aggregate_repository_profile(&[
            FileStyleProfile {
                path: None,
                diagnostics: Vec::new(),
                has_parse_errors: false,
                indentation_observations: Vec::new(),
                decisions: vec![
                    StyleDecision {
                        kind: StyleDecisionKind::BindingColonSpacing,
                        choice: StyleChoice::NoSpaces,
                    },
                    StyleDecision {
                        kind: StyleDecisionKind::BindingColonSpacing,
                        choice: StyleChoice::SpaceAfter,
                    },
                ],
            },
            FileStyleProfile {
                path: None,
                diagnostics: Vec::new(),
                has_parse_errors: false,
                indentation_observations: Vec::new(),
                decisions: vec![StyleDecision {
                    kind: StyleDecisionKind::ElementBracePlacement,
                    choice: StyleChoice::NextLine,
                }],
            },
        ]);

        let comparison = compare_repository_profiles(&reference, &candidate);
        assert_eq!(comparison.total_candidate_observations, 3);
        assert_eq!(comparison.total_candidate_matches_reference_dominant, 1);
        assert_eq!(comparison.decisions.len(), 2);
        assert_eq!(comparison.decisions[0].candidate_matches_reference_dominant, 1);
        assert_eq!(
            comparison.decisions[1].candidate.as_ref().unwrap().dominant_choice,
            StyleChoice::NextLine
        );

        let report = format_repository_style_comparison_report(&comparison);
        assert!(report.contains("Reference observations: 4"));
        assert!(report.contains("Formatter observations: 3"));
        assert!(
            report.contains(
                "Overall formatter alignment with reference dominant choices: 1/3 (33.3%)"
            )
        );
        assert!(report.contains("  reference observations: 3"));
        assert!(report.contains("  formatter observations: 2"));
        assert!(report.contains("reference dominant: space after only (2/3, 66.7%)"));
        assert!(report.contains("formatter dominant: next line (1/1, 100.0%)"));
        assert!(report.contains("formatter matches reference dominant: 0/1 (0.0%)"));
    }

    #[test]
    fn compares_indentation_observations_with_reference_dominant_deltas() {
        let reference = aggregate_repository_profile(&[FileStyleProfile {
            path: None,
            diagnostics: Vec::new(),
            has_parse_errors: false,
            indentation_observations: vec![
                IndentationObservation {
                    kind: IndentationDecisionKind::MultilineBlockChildIndentation,
                    delta: 4,
                },
                IndentationObservation {
                    kind: IndentationDecisionKind::MultilineBlockClosingBraceAlignment,
                    delta: 0,
                },
            ],
            decisions: Vec::new(),
        }]);
        let candidate = aggregate_repository_profile(&[FileStyleProfile {
            path: None,
            diagnostics: Vec::new(),
            has_parse_errors: false,
            indentation_observations: vec![
                IndentationObservation {
                    kind: IndentationDecisionKind::MultilineBlockChildIndentation,
                    delta: 8,
                },
                IndentationObservation {
                    kind: IndentationDecisionKind::MultilineBlockClosingBraceAlignment,
                    delta: 0,
                },
            ],
            decisions: Vec::new(),
        }]);

        let comparison = compare_repository_profiles(&reference, &candidate);
        let report = format_repository_style_comparison_report(&comparison);

        assert!(report.contains(
            "Overall formatter alignment with reference dominant indentation deltas: 1/2 (50.0%)"
        ));
        assert!(report.contains("indentation observations:"));
        assert!(report.contains("multiline block direct-child indentation"));
        assert!(report.contains("reference dominant: +4 spaces (1/1)"));
        assert!(report.contains("formatter dominant: +8 spaces (1/1)"));
    }
}
