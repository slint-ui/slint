// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Report whole-file line churn between original and formatted output,
//! ported from the Topiary-based formatter's `format-metrics diff` command.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct DiffRun {
    compared_files: usize,
    skipped_files_with_parse_errors: usize,
    files_changed: usize,
    inserted_lines: usize,
    removed_lines: usize,
    blank_inserted_lines: usize,
    blank_removed_lines: usize,
    file_summaries: Vec<FileDiffSummary>,
}

pub fn run(corpus_label: &str, files: &[PathBuf]) -> io::Result<String> {
    let mut run = DiffRun::default();

    for file in files {
        let source = fs::read_to_string(file)?;
        let Some(formatted) = super::format_source(&source) else {
            run.skipped_files_with_parse_errors += 1;
            continue;
        };

        run.compared_files += 1;
        let summary = summarize_file_diff(file, &source, &formatted);
        if summary.inserted_lines + summary.removed_lines > 0 {
            run.files_changed += 1;
            run.inserted_lines += summary.inserted_lines;
            run.removed_lines += summary.removed_lines;
            run.blank_inserted_lines += summary.blank_inserted_lines;
            run.blank_removed_lines += summary.blank_removed_lines;
            run.file_summaries.push(summary);
        }
    }

    Ok(format_report(corpus_label, &run))
}

struct FileDiffSummary {
    path: PathBuf,
    inserted_lines: usize,
    removed_lines: usize,
    blank_inserted_lines: usize,
    blank_removed_lines: usize,
    sample: Vec<LineDiffSample>,
}

struct LineDiffSample {
    prefix: char,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineDiffOperation<'a> {
    Equal(&'a str),
    Delete(&'a str),
    Insert(&'a str),
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

    for operation in diff {
        let (prefix, line, line_count, blank_count) = match operation {
            LineDiffOperation::Equal(_) => continue,
            LineDiffOperation::Delete(line) => {
                ('-', line, &mut removed_lines, &mut blank_removed_lines)
            }
            LineDiffOperation::Insert(line) => {
                ('+', line, &mut inserted_lines, &mut blank_inserted_lines)
            }
        };
        *line_count += 1;
        if line.trim().is_empty() {
            *blank_count += 1;
        }
        if sample.len() < 12 {
            sample.push(LineDiffSample { prefix, text: line.to_string() });
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

fn myers_line_diff<'a>(
    original: &'a [&'a str],
    formatted: &'a [&'a str],
) -> Vec<LineDiffOperation<'a>> {
    if original == formatted {
        return original.iter().copied().map(LineDiffOperation::Equal).collect();
    }

    let original_length = original.len();
    let formatted_length = formatted.len();
    let max_edit_distance = original_length + formatted_length;
    let diagonal_offset = max_edit_distance as isize;
    let mut furthest_reaching_x = vec![0usize; 2 * max_edit_distance + 1];
    let mut trace = Vec::with_capacity(max_edit_distance + 1);

    for edit_distance in 0..=max_edit_distance {
        let edit_distance = edit_distance as isize;
        for diagonal in (-edit_distance..=edit_distance).step_by(2) {
            let diagonal_index = (diagonal + diagonal_offset) as usize;
            let mut original_index = if diagonal == -edit_distance
                || (diagonal != edit_distance
                    && furthest_reaching_x[diagonal_index - 1]
                        < furthest_reaching_x[diagonal_index + 1])
            {
                furthest_reaching_x[diagonal_index + 1]
            } else {
                furthest_reaching_x[diagonal_index - 1] + 1
            };

            let mut formatted_index = (original_index as isize - diagonal) as usize;
            while original_index < original_length
                && formatted_index < formatted_length
                && original[original_index] == formatted[formatted_index]
            {
                original_index += 1;
                formatted_index += 1;
            }
            furthest_reaching_x[diagonal_index] = original_index;
            if original_index >= original_length && formatted_index >= formatted_length {
                trace.push(furthest_reaching_x.clone());
                return backtrack_line_diff(&trace, original, formatted);
            }
        }
        trace.push(furthest_reaching_x.clone());
    }

    unreachable!("myers diff should always converge");
}

fn backtrack_line_diff<'a>(
    trace: &[Vec<usize>],
    original: &'a [&'a str],
    formatted: &'a [&'a str],
) -> Vec<LineDiffOperation<'a>> {
    let mut original_index = original.len();
    let mut formatted_index = formatted.len();
    let diagonal_offset = (original.len() + formatted.len()) as isize;
    let mut operations = Vec::new();

    for edit_distance in (1..trace.len()).rev() {
        let furthest_reaching_x = &trace[edit_distance - 1];
        let diagonal = original_index as isize - formatted_index as isize;
        let previous_diagonal = if diagonal == -(edit_distance as isize)
            || (diagonal != edit_distance as isize
                && furthest_reaching_x[(diagonal + diagonal_offset - 1) as usize]
                    < furthest_reaching_x[(diagonal + diagonal_offset + 1) as usize])
        {
            diagonal + 1
        } else {
            diagonal - 1
        };
        let previous_original_index =
            furthest_reaching_x[(previous_diagonal + diagonal_offset) as usize];
        let previous_formatted_index =
            (previous_original_index as isize - previous_diagonal) as usize;

        while original_index > previous_original_index && formatted_index > previous_formatted_index
        {
            original_index -= 1;
            formatted_index -= 1;
            operations.push(LineDiffOperation::Equal(original[original_index]));
        }

        if original_index == previous_original_index {
            formatted_index -= 1;
            operations.push(LineDiffOperation::Insert(formatted[formatted_index]));
        } else {
            original_index -= 1;
            operations.push(LineDiffOperation::Delete(original[original_index]));
        }
    }

    while original_index > 0 && formatted_index > 0 {
        original_index -= 1;
        formatted_index -= 1;
        operations.push(LineDiffOperation::Equal(original[original_index]));
    }

    operations.reverse();
    operations
}

/// Split at `\n`, stripping one trailing `\r` per line; a trailing newline
/// does not start an extra empty line.
fn split_logical_lines(source: &str) -> Vec<&str> {
    let mut lines: Vec<&str> =
        source.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line)).collect();
    if source.ends_with('\n') || source.is_empty() {
        lines.pop();
    }
    lines
}

fn format_report(corpus_label: &str, run: &DiffRun) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Corpus: {corpus_label}"));
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

    if run.file_summaries.is_empty() {
        lines.push("No formatter churn detected.".into());
        return lines.join("\n");
    }

    let mut file_summaries = run.file_summaries.iter().collect::<Vec<_>>();
    file_summaries.sort_by(|left, right| {
        let left_churn = left.inserted_lines + left.removed_lines;
        let right_churn = right.inserted_lines + right.removed_lines;
        right_churn
            .cmp(&left_churn)
            .then_with(|| right.inserted_lines.cmp(&left.inserted_lines))
            .then_with(|| left.path.cmp(&right.path))
    });

    lines.push(String::new());
    lines.push("Most diff-heavy files:".into());
    for file in file_summaries.iter().take(10) {
        lines.push(format!(
            "  - {} (+{} / -{}, blank +{} / -{})",
            file.path.display(),
            file.inserted_lines,
            file.removed_lines,
            file.blank_inserted_lines,
            file.blank_removed_lines
        ));
    }
    if file_summaries.len() > 10 {
        lines.push(format!("  - … and {} more", file_summaries.len() - 10));
    }

    lines.push(String::new());
    lines.push("Sample diff lines:".into());
    for file in file_summaries.iter().take(3) {
        lines.push(format!("  - {}", file.path.display()));
        for sample in &file.sample {
            lines.push(format!("    {}{}", sample.prefix, sample.text));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_summary_counts_blank_line_drift() {
        let summary = summarize_file_diff(Path::new("sample.slint"), "A\n\nB\n", "A\nB\n\n");

        assert_eq!(summary.inserted_lines, 1);
        assert_eq!(summary.removed_lines, 1);
        assert_eq!(summary.blank_inserted_lines, 1);
        assert_eq!(summary.blank_removed_lines, 1);
    }
}
