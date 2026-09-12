// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Slint Code Coverage chapter of the qualification report, reporting which
//! parts of the `.slint` test cases the runs reached, from the measurement
//! the test driver keeps beside each case's lcov.
//!
//! The summary names every file; a page per file shows the source with the
//! points measured on each line.

use crate::Config;
use crate::coverage::Counts;
use crate::traceability::REPO_URL;
use anyhow::Context;
use slint_sc_coverage::{Entry, Measured};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

/// Name of the summary page this module writes into
/// [`Config::qualification_report_dir`], and the prefix of the per-file
/// pages below it. Both match the slugs the pages declare.
const PAGE_FILE: &str = "slint-coverage.mdx";
const SLUG: &str = "qualification-report/slint-coverage";

/// Extension of the per-case measurements the driver keeps, next to the
/// lcov of the same run.
const MEASUREMENT_EXTENSION: &str = "points.json";

/// The coverage of one `.slint` file, merged over the cases that measured
/// it: each case is measured on its own, so a file several cases import is
/// reported once with their counts added up.
struct FileCoverage {
    /// Repository-relative path with `/` separators.
    path: String,
    /// The points of each line, in column order.
    lines: BTreeMap<usize, Vec<Entry>>,
}

impl FileCoverage {
    /// Every point of the file, with how often the runs reached it. A
    /// decision contributes one per outcome.
    fn points(&self) -> impl Iterator<Item = (String, u64)> + '_ {
        self.lines.values().flatten().flat_map(Entry::items)
    }

    fn point_counts(&self) -> Counts {
        counts(self.points())
    }

    /// Only the outcomes of decisions, the metric a structural coverage
    /// argument calls branch coverage.
    fn branch_counts(&self) -> Counts {
        counts(
            self.lines
                .values()
                .flatten()
                .filter(|e| matches!(e, Entry::Decision { .. }))
                .flat_map(Entry::items),
        )
    }

    /// Slug of the page showing this file's source, the path under the
    /// summary's own slug.
    fn page_slug(&self) -> String {
        format!("{SLUG}/{}", self.path.trim_end_matches(".slint"))
    }
}

/// What a point is, as a table cell: `|` ends a cell, so the `||` of a
/// decision has to be escaped to survive one.
fn cell(what: &str) -> String {
    what.replace('|', "\\|")
}

fn counts(points: impl Iterator<Item = (String, u64)>) -> Counts {
    points.fold(Counts { count: 0, covered: 0 }, |acc, (_, count)| Counts {
        count: acc.count + 1,
        covered: acc.covered + u64::from(count > 0),
    })
}

/// The files and their coverage, ordered by the directory they are grouped
/// under and by path within it.
type Files = Vec<FileCoverage>;

pub fn generate(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut out = cfg.qualification_page(PAGE_FILE)?;

    writeln!(
        out,
        r#"---
title: Slint Code Coverage
description: Coverage of the .slint test cases, measured at the level of the Slint language.
slug: {SLUG}
---

The tests of the `slint-sc` runtime are written in Slint: each case is a `.slint` file that the driver compiles, runs, and measures.
`slint-sc-coverage` states that measurement in terms of the `.slint` source rather than of the generated code:
which elements, bindings, handlers and calls a run reached, and which outcomes of every `?:`, `&&` and `||` it took.
[Coverage of Slint Code](/safety-manual/slint-coverage/) describes the measurement,
and [Coverage Tool Verification](/qualification-plan/slint-coverage/) how the tool that takes it is tested.

This chapter is the evidence that a test traced to a requirement in the [Traceability Matrix](/qualification-report/traceability-matrix/) reached the code it is traced for.
The [Test Coverage](/qualification-report/test-coverage/) chapter reports the same runs at the level of the Rust code.

A point is one element, binding, handler, call, or one outcome of one decision.
The branch column counts the outcomes alone, and every one of them is also a point.
Each row links a page showing that file's source with the points measured on every line.
Complete coverage isn't asked of the cases, unlike the runtime's.
A case that reads properties without rendering never reaches the code of its root element.
The cases under `coverage/` exist to pin what an unreached point looks like."#
    )?;

    match &cfg.slint_coverage {
        None => crate::coverage::write_placeholder(&mut out)?,
        Some(dir) => {
            let files = read_measurements(dir)?;
            let sha = crate::traceability::git_head(&crate::root_dir());
            write_summary(&mut out, &files, &sha)?;
            for file in &files {
                write_source_page(cfg, file, &sha)?;
            }
        }
    }
    Ok(())
}

/// Read every per-case measurement in `dir` and merge them into one entry
/// per file.
fn read_measurements(dir: &Path) -> anyhow::Result<Files> {
    let mut merged: BTreeMap<String, BTreeMap<usize, BTreeMap<Key, Entry>>> = BTreeMap::new();
    let mut read = 0;
    for entry in walkdir::WalkDir::new(dir) {
        let path = entry.with_context(|| format!("error reading {dir:?}"))?.into_path();
        if !path.to_string_lossy().ends_with(MEASUREMENT_EXTENSION) {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("error reading measurement {path:?}"))?;
        let measured: Measured = serde_json::from_str(&text)
            .with_context(|| format!("error parsing measurement {path:?}"))?;
        merge(measured, &mut merged);
        read += 1;
    }
    anyhow::ensure!(read > 0, "no *.{MEASUREMENT_EXTENSION} measurement in {dir:?}");

    let mut files: Files = merged
        .into_iter()
        .map(|(path, lines)| FileCoverage {
            path,
            lines: lines
                .into_iter()
                .map(|(line, entries)| (line, entries.into_values().collect()))
                .collect(),
        })
        .collect();
    // Ordered by the directory the summary groups by: sorted by path alone,
    // a subdirectory's files sort in between the files of its parent.
    files.sort_by(|a, b| (group(&a.path), &a.path).cmp(&(group(&b.path), &b.path)));
    Ok(files)
}

/// What makes two entries of a line the same point: its column, and what
/// sits there. A decision and a point never share a column, but keying on
/// both keeps a merge from folding them together if one ever did.
type Key = (usize, bool, String);

fn key(entry: &Entry) -> Key {
    match entry {
        Entry::Point { column, label, .. } => (*column, false, label.clone()),
        Entry::Decision { column, operator, .. } => (*column, true, operator.clone()),
    }
}

/// Add a case's measurement to the merge, summing the counts of the points
/// both reached.
fn merge(measured: Measured, into: &mut BTreeMap<String, BTreeMap<usize, BTreeMap<Key, Entry>>>) {
    for (path, lines) in measured {
        let file = into.entry(path).or_default();
        for (line, entries) in lines {
            let line = file.entry(line).or_default();
            for entry in entries {
                match line.entry(key(&entry)) {
                    std::collections::btree_map::Entry::Vacant(slot) => {
                        slot.insert(entry);
                    }
                    std::collections::btree_map::Entry::Occupied(mut slot) => {
                        add(slot.get_mut(), &entry)
                    }
                }
            }
        }
    }
}

fn add(into: &mut Entry, from: &Entry) {
    match (into, from) {
        (Entry::Point { count, .. }, Entry::Point { count: more, .. }) => *count += more,
        (Entry::Decision { counts, .. }, Entry::Decision { counts: more, .. }) => {
            for (count, more) in counts.iter_mut().zip(more) {
                *count += more;
            }
        }
        // `key` puts a point and a decision in different slots.
        _ => unreachable!("merging a point with a decision"),
    }
}

/// The directory a file is reported under; the cases are grouped in one
/// directory per part of the language they test.
fn group(path: &str) -> &str {
    path.rsplit_once('/').map_or(".", |(dir, _)| dir)
}

/// Sum of one metric across the given files.
fn sum<'a>(
    files: impl IntoIterator<Item = &'a FileCoverage>,
    metric: fn(&FileCoverage) -> Counts,
) -> Counts {
    files.into_iter().fold(Counts { count: 0, covered: 0 }, |acc, f| {
        let c = metric(f);
        Counts { count: acc.count + c.count, covered: acc.covered + c.covered }
    })
}

/// The headline totals, one table per directory with a row per file, its
/// source page and a sum row, and the points no run reached.
fn write_summary(out: &mut impl Write, files: &Files, sha: &str) -> std::io::Result<()> {
    writeln!(
        out,
        "\n{commit}\n\n**Points reached: {points}. Branch outcomes taken: {branches}.**",
        commit = crate::traceability::commit_line(sha),
        points = sum(files, FileCoverage::point_counts).cell(),
        branches = sum(files, FileCoverage::branch_counts).cell(),
    )?;

    // `read_measurements` orders by directory, so the files of one are contiguous.
    for chunk in files.chunk_by(|a, b| group(&a.path) == group(&b.path)) {
        writeln!(out, "\n## {}\n", group(&chunk[0].path))?;
        writeln!(out, "| File | Points | Branches | Per-line |\n| --- | --- | --- | --- |")?;
        for file in chunk {
            let name = file.path.rsplit('/').next().unwrap_or(&file.path);
            writeln!(
                out,
                "| [`{name}`]({REPO_URL}/blob/{sha}/{}) | {} | {} | [view](/{}/) |",
                file.path,
                file.point_counts().cell(),
                file.branch_counts().cell(),
                file.page_slug(),
            )?;
        }
        writeln!(
            out,
            "| **Sum** | **{}** | **{}** | |",
            sum(chunk, FileCoverage::point_counts).cell(),
            sum(chunk, FileCoverage::branch_counts).cell(),
        )?;
    }

    writeln!(out, "\n## Never Reached\n")?;
    let mut any = false;
    for file in files {
        for (line, entries) in &file.lines {
            for entry in entries {
                for (what, _) in entry.items().iter().filter(|(_, count)| *count == 0) {
                    if !any {
                        writeln!(
                            out,
                            "The points below exist in the source and no run reached them.\n\n\
                             | Location | Point |\n| --- | --- |"
                        )?;
                        any = true;
                    }
                    writeln!(
                        out,
                        "| [`{path}:{line}:{column}`]({REPO_URL}/blob/{sha}/{path}#L{line}) | {what} |",
                        path = file.path,
                        column = entry.column(),
                        what = cell(what),
                    )?;
                }
            }
        }
    }
    if !any {
        writeln!(out, "Every coverage point of the cases was reached.")?;
    }
    Ok(())
}

/// The page of one file: its source with the points measured on each line,
/// and a table naming every point.
fn write_source_page(
    cfg: &Config,
    file: &FileCoverage,
    sha: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = file.path.rsplit('/').next().unwrap_or(&file.path);
    let mut out = cfg.qualification_page(&format!(
        "{}.mdx",
        file.page_slug().trim_start_matches("qualification-report/")
    ))?;
    writeln!(
        out,
        r#"---
title: {name}
description: The coverage points of {path} and which of them the runs reached.
slug: {slug}
---

The coverage of [`{path}`]({REPO_URL}/blob/{sha}/{path}), from the runs reported in [Slint Code Coverage](/{SLUG}/).
Points reached: {points}. Branch outcomes taken: {branches}."#,
        path = file.path,
        slug = file.page_slug(),
        points = file.point_counts().cell(),
        branches = file.branch_counts().cell(),
    )?;

    let source = std::fs::read_to_string(crate::root_dir().join(&file.path))
        .with_context(|| format!("error reading {}", file.path))?;
    write_source(&mut out, file, &source)?;

    writeln!(out, "\n## Points\n\n| Line | Column | Point | Count |\n| --- | --- | --- | --- |")?;
    for (line, entries) in &file.lines {
        for entry in entries {
            for (what, count) in entry.items() {
                writeln!(out, "| {line} | {} | {} | {count} |", entry.column(), cell(&what))?;
            }
        }
    }
    Ok(())
}

/// The source, with a caret line under every line that holds points: `^+`
/// where a run reached the point and `^-` where none did, and for a
/// decision the status of its true and its false outcome. This is the
/// notation the cases state their coverage in, so the page and the source
/// read the same way.
fn write_source(out: &mut impl Write, file: &FileCoverage, source: &str) -> std::io::Result<()> {
    let annotated = slint_sc_coverage::expectations::annotate(source, &file.lines);
    // A point in one of the first columns leaves no room for a caret; the
    // Points table below the source states those either way.
    let legend = if annotated.is_ok() {
        "\nA `//#c` line marks the points of the line above it, `^+` where a run reached the point and `^-` where none did.\n\
         A decision carries the status of its true and of its false outcome, like `^+-`.\n"
    } else {
        ""
    };
    let body = annotated.as_deref().unwrap_or(source);
    let fence = fence(body);
    writeln!(out, "{legend}\n{fence}slint\n{}\n{fence}", body.trim_end())
}

/// A code fence longer than the longest run of backticks in the source, so
/// that a case carrying a code block of its own -- every case ends in one,
/// holding the Rust of the test -- doesn't close the block early.
fn fence(source: &str) -> String {
    let longest = source.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.saturating_add(1).max(3))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(column: usize, label: &str, count: u64) -> Entry {
        Entry::Point { column, label: label.into(), count }
    }

    fn decision(column: usize, counts: [u64; 2]) -> Entry {
        Entry::Decision { column, operator: "?".into(), counts }
    }

    fn file() -> FileCoverage {
        FileCoverage {
            path: "cases/a.slint".into(),
            lines: BTreeMap::from([
                (7, vec![point(1, "element Window", 1)]),
                (13, vec![point(30, "binding pick", 4), decision(37, [4, 0])]),
                (20, vec![point(5, "handler clicked", 0)]),
            ]),
        }
    }

    #[test]
    fn metrics() {
        // Five points: three plain ones and the two outcomes of the
        // decision. The handler and the false outcome were never reached.
        assert_eq!(file().point_counts().cell(), "60.0% (3/5)");
        // The branch column counts the outcomes alone.
        assert_eq!(file().branch_counts().cell(), "50.0% (1/2)");
        assert_eq!(file().page_slug(), "qualification-report/slint-coverage/cases/a");
    }

    #[test]
    fn merging() {
        // The same file measured by two cases: the counts of a point both
        // reached add up, and an outcome only one took counts as taken.
        let one: Measured = BTreeMap::from([(
            "cases/a.slint".into(),
            BTreeMap::from([(13, vec![point(30, "binding pick", 2), decision(37, [1, 0])])]),
        )]);
        let two: Measured = BTreeMap::from([(
            "cases/a.slint".into(),
            BTreeMap::from([(13, vec![point(30, "binding pick", 3), decision(37, [0, 5])])]),
        )]);
        let mut merged = BTreeMap::new();
        merge(one, &mut merged);
        merge(two, &mut merged);
        let line = &merged["cases/a.slint"][&13];
        let entries: Vec<&Entry> = line.values().collect();
        assert_eq!(*entries[0], point(30, "binding pick", 5));
        assert_eq!(*entries[1], decision(37, [1, 5]));
    }

    #[test]
    fn source_page_marks_the_points() {
        let source = "Window {\n    property <int> p: c ? 1 : 2;\n}\n";
        let file = FileCoverage {
            path: "cases/a.slint".into(),
            lines: BTreeMap::from([(2, vec![point(23, "binding p", 3), decision(25, [3, 0])])]),
        };
        let mut out = Vec::new();
        write_source(&mut out, &file, source).unwrap();
        let out = String::from_utf8(out).unwrap();
        // The caret sits under the point it describes: the binding was
        // evaluated, and its decision never took the false outcome.
        assert!(out.contains("\n//#c                  ^+^+-\n"), "{out}");
        assert!(out.contains("```slint\n"), "{out}");
    }

    #[test]
    fn source_with_its_own_code_block() {
        // Every case ends in a ```rust block holding the test code; the
        // fence around the source has to outlast it.
        let source = "Window { }\n\n/*\n```rust\nlet x = 1;\n```\n*/\n";
        let file = FileCoverage {
            path: "cases/a.slint".into(),
            lines: BTreeMap::from([(1, vec![point(1, "element Window", 1)])]),
        };
        let mut out = Vec::new();
        write_source(&mut out, &file, source).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("````slint\n"), "{out}");
        assert!(out.trim_end().ends_with("\n````"), "{out}");
        // The case's own block is left as it is, inside the longer fence.
        assert!(out.contains("```rust\n"), "{out}");
    }

    #[test]
    fn a_decision_survives_a_table_cell() {
        // `||` would otherwise end the cell and split the row.
        assert_eq!(cell("branch || false"), "branch \\|\\| false");
        assert_eq!(cell("binding width"), "binding width");
    }

    #[test]
    fn grouping() {
        assert_eq!(
            group("api/slint-sc/tests/cases/expr/ternary.slint"),
            "api/slint-sc/tests/cases/expr"
        );
        assert_eq!(group("a.slint"), ".");
    }
}
