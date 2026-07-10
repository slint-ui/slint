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
enum LineDiffOp<'a> {
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
