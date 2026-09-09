// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The coverage of `.slint` files compiled for Slint SC with `--coverage`.
//!
//! The compiler instruments the generated code with a coverage point for
//! every element, binding, callback handler and call, and both outcomes of
//! every `?:`, `&&` and `||`. However the points are counted, a [`Report`]
//! gathers their hit counts by source location and writes them as lcov, as a
//! summary, or as what the test driver compares with a case's expectations.

pub mod expectations;
pub mod source_map;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A coverage point of the `.slint` source.
pub struct Point {
    /// `element`, `binding`, `handler`, `call`, or `branch`.
    pub kind: String,
    /// The property of a binding, the callback of a handler or a call.
    pub name: Option<String>,
    pub file: PathBuf,
    /// 1-based.
    pub line: usize,
    /// 1-based; a decision's is its operator's.
    pub column: usize,
    /// The operator (`?`, `&&`, `||`) and the outcome, of a `branch` point.
    pub branch: Option<(String, bool)>,
}

impl Point {
    /// The point as the listing names it: `element`, `binding level`,
    /// `branch ? true`.
    fn label(&self) -> String {
        match (&self.branch, &self.name) {
            (Some((op, outcome)), _) => format!("branch {op} {}", ARMS[!*outcome as usize]),
            (None, Some(name)) => format!("{} {name}", self.kind),
            (None, None) => self.kind.clone(),
        }
    }
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
    /// The hit count of each point, by column and label.
    points: BTreeMap<(usize, String), u64>,
    /// The taken counts of the true and the false outcome of each decision,
    /// by the column of its operator, with the operator.
    branches: BTreeMap<usize, (String, [u64; 2])>,
}

/// The outcomes, in the order lcov numbers the branches of a decision.
const ARMS: [&str; 2] = ["true", "false"];

impl LineCoverage {
    fn count(&self) -> u64 {
        self.points.values().sum()
    }

    /// Every point and decision outcome on the line, by column, each with
    /// whether it was reached: `element`, `binding level`, `branch ? false`...
    fn entries(&self) -> Vec<(usize, String, bool)> {
        let points =
            self.points.iter().map(|((column, label), &count)| (*column, label.clone(), count > 0));
        let branches = self.branches.iter().flat_map(|(&column, (op, arms))| {
            (0..2).map(move |arm| (column, format!("branch {op} {}", ARMS[arm]), arms[arm] > 0))
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
        match &point.branch {
            Some((op, outcome)) => {
                let entry =
                    line.branches.entry(point.column).or_insert_with(|| (op.clone(), [0; 2]));
                entry.1[!*outcome as usize] += count;
            }
            None => *line.points.entry((point.column, point.label())).or_default() += count,
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
                for (block, (_, arms)) in coverage.branches.values().enumerate() {
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

    /// Every point and decision outcome of `file`, by line: the column, what
    /// it is (`element`, `binding level`, `branch ? false`) and whether it was
    /// reached, in column order.
    pub fn lines_of(&self, file: &Path) -> BTreeMap<usize, Vec<(usize, String, bool)>> {
        let lines = self.files.get(file).into_iter().flatten();
        lines.map(|(line, coverage)| (*line, coverage.entries())).collect()
    }

    /// The points of every file but `case`, one line each: `+` when reached
    /// and `-` when not, then the path relative to the case's directory, the
    /// location and what it is, like `+ lib/b.slint:13:30 branch ? false`.
    pub fn listing(&self, case: &Path) -> Vec<String> {
        let case_dir = case.parent().unwrap_or(Path::new(""));
        let mut listing = Vec::new();
        for (path, lines) in self.files.iter().filter(|(path, _)| path.as_path() != case) {
            let path = display(path, case_dir);
            for (line, coverage) in lines {
                for (column, what, reached) in coverage.entries() {
                    let status = if reached { '+' } else { '-' };
                    listing.push(format!("{status} {path}:{line}:{column} {what}"));
                }
            }
        }
        listing
    }
}

/// The lines and branch outcomes of a file, and how many were reached.
struct Totals {
    lines_hit: usize,
    branches: usize,
    branches_hit: usize,
}

impl Totals {
    fn of(lines: &BTreeMap<usize, LineCoverage>) -> Self {
        let arms = || lines.values().flat_map(|l| l.branches.values().flat_map(|(_, arms)| arms));
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

    fn point(kind: &str, name: Option<&str>, line: usize, column: usize) -> Point {
        Point {
            kind: kind.into(),
            name: name.map(String::from),
            file: "/src/a.slint".into(),
            line,
            column,
            branch: None,
        }
    }

    fn branch(op: &str, outcome: bool, line: usize, column: usize) -> Point {
        Point { branch: Some((op.into(), outcome)), ..point("branch", None, line, column) }
    }

    fn report() -> Report {
        let mut report = Report::default();
        report.add(&point("element", Some("Window"), 7, 36), 3);
        report.add(&point("binding", Some("pick"), 13, 30), 4);
        report.add(&branch("?", true, 13, 37), 4);
        report.add(&branch("?", false, 13, 37), 0);
        // Inlined twice, the counts add up.
        report.add(&point("binding", Some("len"), 13, 50), 1);
        report.add(&point("binding", Some("len"), 13, 50), 2);
        report.add(&point("handler", Some("clicked"), 20, 5), 0);
        report.add(
            &Point { file: "/src/lib/b.slint".into(), ..point("element", Some("Led"), 2, 1) },
            3,
        );
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
        let case = Path::new("/src/a.slint");
        assert_eq!(report().listing(case), ["+ lib/b.slint:2:1 element Led"]);
        let lines = report().lines_of(case);
        assert_eq!(lines.keys().copied().collect::<Vec<_>>(), [7, 13, 20]);
        assert_eq!(
            lines[&13],
            [
                (30, "binding pick".to_string(), true),
                (37, "branch ? true".to_string(), true),
                (37, "branch ? false".to_string(), false),
                (50, "binding len".to_string(), true),
            ]
        );
    }
}
