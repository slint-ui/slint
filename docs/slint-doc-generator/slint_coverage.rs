// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Slint Code Coverage chapter of the qualification report, reporting which
//! parts of the `.slint` test cases the runs reached, from the lcov that
//! `slint-sc-coverage` writes.

use crate::Config;
use crate::coverage::Counts;
use crate::traceability::REPO_URL;
use anyhow::Context;
use std::collections::BTreeMap;
use std::io::Write;

/// Name of the page this module writes into
/// [`Config::qualification_report_dir`], the section it belongs to.
const PAGE_FILE: &str = "slint-coverage.mdx";

/// The coverage of one `.slint` file, summed over the runs that measured it.
/// Each case is measured on its own, so a file several cases import is
/// reported once with their counts added up.
#[derive(Default)]
struct FileCoverage {
    /// Execution count of every line that holds a coverage point.
    lines: BTreeMap<u64, u64>,
    /// Taken count of every outcome, by line, decision within the line, and
    /// outcome within the decision.
    outcomes: BTreeMap<(u64, u64, u64), u64>,
}

impl FileCoverage {
    fn line_counts(&self) -> Counts {
        Counts {
            count: self.lines.len() as u64,
            covered: self.lines.values().filter(|c| **c > 0).count() as u64,
        }
    }

    fn outcome_counts(&self) -> Counts {
        Counts {
            count: self.outcomes.len() as u64,
            covered: self.outcomes.values().filter(|c| **c > 0).count() as u64,
        }
    }
}

/// The files and their coverage, ordered by the directory they are grouped
/// under and by path within it.
type Files = Vec<(String, FileCoverage)>;

/// A point of the `.slint` source that no run reached.
struct Unreached {
    path: String,
    line: u64,
    /// What sits there, as far as lcov says: a line, or one outcome of one
    /// of the decisions on it.
    what: String,
}

pub fn generate(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut out = cfg.qualification_page(PAGE_FILE)?;

    writeln!(
        out,
        r#"---
title: Slint Code Coverage
description: Coverage of the .slint test cases, measured at the level of the Slint language.
slug: qualification-report/slint-coverage
---

The tests of the `slint-sc` runtime are written in Slint: each case is a `.slint` file that the driver compiles, runs, and measures.
`slint-sc-coverage` states that measurement in terms of the `.slint` source rather than of the generated code:
which elements, bindings, handlers and calls a run reached, and which outcomes of every `?:`, `&&` and `||` it took.
[Coverage of Slint Code](/safety-manual/slint-coverage/) describes the measurement,
and [Coverage Tool Verification](/qualification-plan/slint-coverage/) how the tool that takes it is tested.

This chapter is the evidence that a test traced to a requirement in the [Traceability Matrix](/qualification-report/traceability-matrix/) reached the code it is traced for.
The [Test Coverage](/qualification-report/test-coverage/) chapter reports the same runs at the level of the Rust code.

A line counts below when it holds at least one coverage point, and it is covered when a point on it was reached.
A branch is one outcome of one decision, so a decision contributes two.
Complete coverage isn't asked of the cases, unlike the runtime's.
A case that reads properties without rendering never reaches the code of its root element.
The cases under `coverage/` exist to pin what an unreached point looks like."#
    )?;

    match &cfg.slint_lcov {
        None => crate::coverage::write_placeholder(&mut out)?,
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .context(format!("error reading lcov report {path:?}"))?;
            let files = parse_lcov(&text).context(format!("error parsing lcov report {path:?}"))?;
            let sha = crate::traceability::git_head(&crate::root_dir());
            write_report(&mut out, &files, &sha)?;
        }
    }
    Ok(())
}

/// Parse the `SF`, `DA` and `BRDA` records of an lcov file into the coverage
/// of each file it names. The suite concatenates one report per case, so a
/// file occurs once per case that reached it and the counts are summed.
fn parse_lcov(text: &str) -> anyhow::Result<Files> {
    let mut files: BTreeMap<String, FileCoverage> = BTreeMap::new();
    let mut current = String::new();
    for (number, record) in text.lines().enumerate() {
        let at = |what: &str| format!("line {}: {record}: {what}", number + 1);
        let Some((tag, value)) = record.split_once(':') else { continue };
        if tag == "SF" {
            current = value.replace('\\', "/");
            files.entry(current.clone()).or_default();
            continue;
        }
        if tag != "DA" && tag != "BRDA" {
            continue;
        }
        let file = files.get_mut(&current).with_context(|| at("no `SF` record"))?;
        // lcov spells a count that was never instrumented `-`.
        let fields: Vec<u64> = value
            .split(',')
            .map(|f| if f == "-" { Ok(0) } else { f.parse() })
            .collect::<Result<_, _>>()
            .with_context(|| at("malformed count"))?;
        match (tag, fields.as_slice()) {
            ("DA", &[line, count, ..]) => *file.lines.entry(line).or_default() += count,
            ("BRDA", &[line, block, outcome, count]) => {
                *file.outcomes.entry((line, block, outcome)).or_default() += count
            }
            _ => anyhow::bail!("{}", at("malformed record")),
        }
    }
    anyhow::ensure!(!files.is_empty(), "no file in the lcov report");
    let mut files: Files = files.into_iter().collect();
    // Ordered by the directory the report groups by: sorted by path alone, a
    // subdirectory's files sort in between the files of its parent.
    files.sort_by(|(a, _), (b, _)| (group(a), a).cmp(&(group(b), b)));
    Ok(files)
}

/// The directory a file is reported under; the cases are grouped in one
/// directory per part of the language they test.
fn group(path: &str) -> &str {
    path.rsplit_once('/').map_or(".", |(dir, _)| dir)
}

/// Sum of one metric across the given files.
fn sum<'a>(
    files: impl IntoIterator<Item = &'a (String, FileCoverage)>,
    metric: impl Fn(&FileCoverage) -> Counts,
) -> Counts {
    files.into_iter().fold(Counts { count: 0, covered: 0 }, |acc, (_, f)| {
        let c = metric(f);
        Counts { count: acc.count + c.count, covered: acc.covered + c.covered }
    })
}

/// The outcomes of a decision, in the order lcov numbers them.
const ARMS: [&str; 2] = ["true", "false"];

/// Every point that no run reached, in path and line order.
fn unreached(files: &Files) -> Vec<Unreached> {
    let mut out = Vec::new();
    for (path, coverage) in files {
        let mut points: Vec<Unreached> = coverage
            .lines
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(line, _)| Unreached { path: path.clone(), line: *line, what: "line".into() })
            .collect();
        points.extend(coverage.outcomes.iter().filter(|(_, count)| **count == 0).map(
            |((line, block, outcome), _)| Unreached {
                path: path.clone(),
                line: *line,
                what: format!(
                    "decision {}, {} outcome",
                    block + 1,
                    ARMS.get(*outcome as usize).copied().unwrap_or("further")
                ),
            },
        ));
        points.sort_by(|a, b| (a.line, &a.what).cmp(&(b.line, &b.what)));
        out.extend(points);
    }
    out
}

/// The headline totals, one table per directory with a row per file and a sum
/// row, and the points no run reached.
fn write_report(out: &mut impl Write, files: &Files, sha: &str) -> std::io::Result<()> {
    writeln!(
        out,
        "\n{commit}\n\n**Line coverage: {lines}. Branch coverage: {branches}.**",
        commit = crate::traceability::commit_line(sha),
        lines = sum(files, FileCoverage::line_counts).cell(),
        branches = sum(files, FileCoverage::outcome_counts).cell(),
    )?;

    // `parse_lcov` orders by directory, so the files of one are contiguous.
    for chunk in files.chunk_by(|(a, _), (b, _)| group(a) == group(b)) {
        writeln!(out, "\n## {}\n", group(&chunk[0].0))?;
        writeln!(out, "| File | Lines | Branches |\n| --- | --- | --- |")?;
        for (path, coverage) in chunk {
            let name = path.rsplit('/').next().unwrap_or(path);
            writeln!(
                out,
                "| [`{name}`]({REPO_URL}/blob/{sha}/{path}) | {} | {} |",
                coverage.line_counts().cell(),
                coverage.outcome_counts().cell(),
            )?;
        }
        writeln!(
            out,
            "| **Sum** | **{}** | **{}** |",
            sum(chunk, FileCoverage::line_counts).cell(),
            sum(chunk, FileCoverage::outcome_counts).cell(),
        )?;
    }

    let unreached = unreached(files);
    writeln!(out, "\n## Never Reached\n")?;
    if unreached.is_empty() {
        return writeln!(out, "Every coverage point of the cases was reached.");
    }
    writeln!(
        out,
        "The points below exist in the source and no run reached them.\n\
         A `line` row is a line whose coverage points were all missed, and a decision row is one outcome that was never taken.\n\n\
         | Location | Point |\n| --- | --- |"
    )?;
    for point in &unreached {
        writeln!(
            out,
            "| [`{path}:{line}`]({REPO_URL}/blob/{sha}/{path}#L{line}) | {what} |",
            path = point.path,
            line = point.line,
            what = point.what,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two cases, the second importing the file of a third record: `a.slint`
    /// has a line that was never reached and a decision that only ever took
    /// its true outcome, `b.slint` is completely covered, and `lib.slint` is
    /// measured once per case that imports it.
    const LCOV: &str = "TN:
SF:cases/a.slint
BRDA:13,0,0,4
BRDA:13,0,1,0
BRF:2
BRH:1
DA:7,3
DA:13,4
DA:20,0
LF:3
LH:2
end_of_record
TN:
SF:cases/b.slint
DA:2,1
LF:1
LH:1
end_of_record
TN:
SF:cases/lib/lib.slint
DA:5,2
LF:1
LH:1
end_of_record
TN:
SF:cases/lib/lib.slint
DA:5,1
LF:1
LH:1
end_of_record
";

    #[test]
    fn parse() {
        let files = parse_lcov(LCOV).unwrap();
        let paths: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(paths, ["cases/a.slint", "cases/b.slint", "cases/lib/lib.slint"]);

        assert_eq!(files[0].1.line_counts().cell(), "66.7% (2/3)");
        assert_eq!(files[0].1.outcome_counts().cell(), "50.0% (1/2)");
        // A file without a decision has no branch to report.
        assert_eq!(files[1].1.outcome_counts().cell(), "-");
        // The two records of the imported file are one row, their counts added.
        assert_eq!(files[2].1.lines[&5], 3);

        assert_eq!(sum(&files, FileCoverage::line_counts).cell(), "80.0% (4/5)");
        assert_eq!(sum(&files, FileCoverage::outcome_counts).cell(), "50.0% (1/2)");

        // A report without a file is an error, not an empty page.
        assert!(parse_lcov("TN:\n").is_err());
        // So is a record before any `SF`, and a malformed one.
        assert!(parse_lcov("DA:1,1\n").is_err());
        assert!(parse_lcov("SF:a.slint\nDA:1\n").is_err());
        assert!(parse_lcov("SF:a.slint\nDA:1,x\n").is_err());
    }

    #[test]
    fn never_reached() {
        let points = unreached(&parse_lcov(LCOV).unwrap());
        let rows: Vec<(&str, u64, &str)> =
            points.iter().map(|p| (p.path.as_str(), p.line, p.what.as_str())).collect();
        assert_eq!(
            rows,
            [("cases/a.slint", 13, "decision 1, false outcome"), ("cases/a.slint", 20, "line"),]
        );
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
