// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The coverage of `.slint` files compiled for Slint SC with `--coverage`.
//!
//! The compiler instruments the generated code with a coverage point for
//! every element, binding, callback handler and call, and both outcomes of
//! every `?:`, `&&` and `||`. However the points are counted, a [`Report`]
//! gathers their hit counts by source location and writes them as lcov, as a
//! summary, or as the listing the test driver compares with a case's
//! expectations.

pub mod marks;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A coverage point of the `.slint` source.
pub struct Point {
    /// `element`, `binding`, `handler`, `call`, or `branch`.
    pub kind: String,
    pub file: PathBuf,
    /// 1-based.
    pub line: usize,
    /// 1-based.
    pub column: usize,
    /// The decision ordinal within the binding and the outcome, of a `branch` point.
    pub branch: Option<(usize, bool)>,
}

/// The coverage of every `.slint` file, by line.
#[derive(Default)]
pub struct Report {
    files: BTreeMap<PathBuf, BTreeMap<usize, LineCoverage>>,
}

/// The coverage of one source line. A binding inlined in several places has
/// several points at its location, whose counts add up.
#[derive(Default)]
struct LineCoverage {
    /// The hit count of each point, by column and kind.
    points: BTreeMap<(usize, String), u64>,
    /// The taken counts of the true and the false outcome of each decision,
    /// by the column of the binding holding it and its ordinal.
    branches: BTreeMap<(usize, usize), [u64; 2]>,
}

/// The outcomes, in the order lcov numbers the branches of a decision.
const ARMS: [&str; 2] = ["true", "false"];

impl LineCoverage {
    fn count(&self) -> u64 {
        self.points.values().sum()
    }

    /// Every point and decision outcome on the line, by column, each with
    /// whether it was reached: `element`, `branch 0 false`...
    fn entries(&self) -> Vec<(usize, String, bool)> {
        let points =
            self.points.iter().map(|((column, kind), &count)| (*column, kind.clone(), count > 0));
        let branches = self.branches.iter().flat_map(|(&(column, ordinal), arms)| {
            (0..2)
                .map(move |arm| (column, format!("branch {ordinal} {}", ARMS[arm]), arms[arm] > 0))
        });
        let mut entries: Vec<_> = points.chain(branches).collect();
        entries.sort_by_key(|entry| entry.0);
        entries
    }
}

impl Report {
    /// Count that the point was hit `count` times.
    pub fn add(&mut self, point: &Point, count: u64) {
        let file = self.files.entry(point.file.clone()).or_default();
        let line = file.entry(point.line).or_default();
        match point.branch {
            Some((ordinal, outcome)) => {
                let arms = line.branches.entry((point.column, ordinal)).or_default();
                arms[!outcome as usize] += count;
            }
            None => *line.points.entry((point.column, point.kind.clone())).or_default() += count,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The lcov records, the paths relative to `base_dir`.
    pub fn lcov(&self, base_dir: &Path) -> String {
        let mut out = String::new();
        for (path, lines) in &self.files {
            out.push_str(&format!("TN:\nSF:{}\n", display(path, base_dir)));
            let totals = Totals::of(lines);
            for (line, coverage) in lines {
                // A block of lcov is a decision, numbered within its line.
                for (block, arms) in coverage.branches.values().enumerate() {
                    for (arm, count) in arms.iter().enumerate() {
                        out.push_str(&format!("BRDA:{line},{block},{arm},{count}\n"));
                    }
                }
            }
            if totals.branches > 0 {
                out.push_str(&format!("BRF:{}\nBRH:{}\n", totals.branches, totals.branches_hit));
            }
            for (line, coverage) in lines {
                out.push_str(&format!("DA:{line},{}\n", coverage.count()));
            }
            out.push_str(&format!("LF:{}\nLH:{}\nend_of_record\n", lines.len(), totals.lines_hit));
        }
        out
    }

    /// Print the coverage of each file and its unreached points to standard
    /// error, and return how many points were never reached.
    pub fn summary(&self, base_dir: &Path) -> usize {
        let mut gaps = 0;
        for (path, lines) in &self.files {
            let totals = Totals::of(lines);
            let path = display(path, base_dir);
            eprintln!(
                "{path}: lines {}/{}, branches {}/{}",
                totals.lines_hit,
                lines.len(),
                totals.branches_hit,
                totals.branches
            );
            for (line, coverage) in lines {
                for (column, what, reached) in coverage.entries() {
                    if !reached {
                        eprintln!("  {path}:{line}:{column}: {what} never reached");
                        gaps += 1;
                    }
                }
            }
        }
        gaps
    }

    /// The listing a test case's ```` ```coverage ```` block states: one line
    /// per point, `+` when it was reached and `-` when not, then its
    /// location and kind, like `+ 13:30 branch 0 false`. The points of
    /// `case` come first without a path, those of another file with its path
    /// relative to the case's directory.
    pub fn listing(&self, case: &Path) -> Vec<String> {
        let case_dir = case.parent().unwrap_or(Path::new(""));
        let mut files: Vec<_> = self.files.iter().collect();
        files.sort_by_key(|(path, _)| (*path != case, path.as_path()));
        let mut listing = Vec::new();
        for (path, lines) in files {
            let prefix = match path == case {
                true => String::new(),
                false => format!("{}:", display(path, case_dir)),
            };
            for (line, coverage) in lines {
                for (column, what, reached) in coverage.entries() {
                    let status = if reached { '+' } else { '-' };
                    listing.push(format!("{status} {prefix}{line}:{column} {what}"));
                }
            }
        }
        listing
    }
}

/// Compare a case's listing with the one its ```` ```coverage ```` block
/// expects, and describe the difference.
pub fn check_listing(expected: &str, actual: &[String]) -> Result<(), String> {
    let expected: Vec<&str> = expected.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if expected == actual {
        return Ok(());
    }
    let mut message = String::from("the coverage differs from the ```coverage block:\n");
    for line in &expected {
        if !actual.iter().any(|a| a == line) {
            message.push_str(&format!("  expected, not measured: {line}\n"));
        }
    }
    for line in actual {
        if !expected.contains(&line.as_str()) {
            message.push_str(&format!("  measured, not expected: {line}\n"));
        }
    }
    message.push_str("the measured coverage, for the block:\n```coverage\n");
    for line in actual {
        message.push_str(line);
        message.push('\n');
    }
    message.push_str("```");
    Err(message)
}

/// The lines and branch outcomes of a file, and how many were reached.
struct Totals {
    lines_hit: usize,
    branches: usize,
    branches_hit: usize,
}

impl Totals {
    fn of(lines: &BTreeMap<usize, LineCoverage>) -> Self {
        let arms = || lines.values().flat_map(|l| l.branches.values().flatten());
        Self {
            lines_hit: lines.values().filter(|l| l.count() > 0).count(),
            branches: arms().count(),
            branches_hit: arms().filter(|&&count| count > 0).count(),
        }
    }
}

fn display(path: &Path, base_dir: &Path) -> String {
    let path = path.strip_prefix(base_dir).unwrap_or(path);
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(
        kind: &str,
        file: &str,
        line: usize,
        column: usize,
        branch: Option<(usize, bool)>,
    ) -> Point {
        Point { kind: kind.into(), file: file.into(), line, column, branch }
    }

    fn report() -> Report {
        let mut report = Report::default();
        report.add(&point("element", "/src/a.slint", 7, 36, None), 3);
        report.add(&point("binding", "/src/a.slint", 13, 30, None), 4);
        report.add(&point("branch", "/src/a.slint", 13, 30, Some((0, true))), 4);
        report.add(&point("branch", "/src/a.slint", 13, 30, Some((0, false))), 0);
        // Inlined twice, the counts add up.
        report.add(&point("binding", "/src/a.slint", 13, 50, None), 1);
        report.add(&point("binding", "/src/a.slint", 13, 50, None), 2);
        report.add(&point("handler", "/src/a.slint", 20, 5, None), 0);
        report.add(&point("element", "/src/lib/b.slint", 2, 1, None), 3);
        report
    }

    #[test]
    fn lcov() {
        assert_eq!(
            report().lcov(Path::new("/src")),
            "TN:
SF:a.slint
BRDA:13,0,0,4
BRDA:13,0,1,0
BRF:2
BRH:1
DA:7,3
DA:13,7
DA:20,0
LF:3
LH:2
end_of_record
TN:
SF:lib/b.slint
DA:2,3
LF:1
LH:1
end_of_record
"
        );
        assert_eq!(report().summary(Path::new("/src")), 2);
    }

    #[test]
    fn listing() {
        let listing = report().listing(Path::new("/src/a.slint"));
        assert_eq!(
            listing,
            [
                "+ 7:36 element",
                "+ 13:30 binding",
                "+ 13:30 branch 0 true",
                "- 13:30 branch 0 false",
                "+ 13:50 binding",
                "- 20:5 handler",
                "+ lib/b.slint:2:1 element",
            ]
        );
        assert!(check_listing(&listing.join("\n"), &listing).is_ok());
        let differing = check_listing("+ 7:36 element\n+ 20:5 handler\n", &listing).unwrap_err();
        assert!(differing.contains("expected, not measured: + 20:5 handler"));
        assert!(differing.contains("measured, not expected: - 20:5 handler"));
        assert!(differing.contains("```coverage\n+ 7:36 element\n"));
    }
}
