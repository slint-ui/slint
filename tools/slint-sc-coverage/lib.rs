// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The coverage of `.slint` files compiled for Slint SC with `--coverage`.
//!
//! The compiler maps the generated code to a coverage point for every
//! element, binding, callback handler and call, and both outcomes of every
//! `?:`, `&&` and `||`. However the points are counted, a [`Report`] gathers
//! their hit counts by source location and writes them as lcov, as a
//! summary, or as what the test driver compares with a case's expectations.

pub mod expectations;
pub mod source_map;

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// A coverage point of the `.slint` source.
pub struct Point {
    pub kind: Kind,
    /// The type an element is written with, the property of a binding, the
    /// callback of a handler or a call, or the operator of a decision (`?`,
    /// `&&`, `||`).
    pub name: String,
    pub file: PathBuf,
    /// 1-based.
    pub line: usize,
    /// 1-based; a decision's is its operator's.
    pub column: usize,
}

/// What a coverage point is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Element,
    Binding,
    Handler,
    Call,
    /// One outcome of a decision.
    Branch {
        outcome: bool,
    },
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Element => "element",
            Kind::Binding => "binding",
            Kind::Handler => "handler",
            Kind::Call => "call",
            Kind::Branch { .. } => "branch",
        })
    }
}

impl Point {
    /// The point as the listing names it: `element Rectangle`, `binding
    /// level`, `branch ? true`.
    fn label(&self) -> String {
        match self.kind {
            Kind::Branch { outcome } => format!("branch {} {}", self.name, ARMS[!outcome as usize]),
            kind => format!("{kind} {}", self.name),
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

/// A point of a line, or a decision with both its outcomes, with whether it
/// was reached.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Entry {
    Point { column: usize, label: String, reached: bool },
    Decision { column: usize, operator: String, reached: [bool; 2] },
}

impl Entry {
    pub fn column(&self) -> usize {
        match self {
            Entry::Point { column, .. } | Entry::Decision { column, .. } => *column,
        }
    }

    /// What the entry states, one item per point and outcome: what it is,
    /// like `binding pick` or `branch ? false`, and whether it was reached.
    fn items(&self) -> Vec<(String, bool)> {
        match self {
            Entry::Point { label, reached, .. } => vec![(label.clone(), *reached)],
            Entry::Decision { operator, reached, .. } => (0..2)
                .map(|arm| (format!("branch {operator} {}", ARMS[arm]), reached[arm]))
                .collect(),
        }
    }
}

impl LineCoverage {
    /// The execution count of the line: the hits of its points, or the
    /// evaluations of its decisions when it holds no point, as the line of an
    /// operator of a decision written over several lines does.
    fn count(&self) -> u64 {
        if self.points.is_empty() {
            return self.branches.values().map(|(_, arms)| arms[0] + arms[1]).sum();
        }
        self.points.values().sum()
    }

    /// Every point and decision on the line, in column order.
    fn entries(&self) -> Vec<Entry> {
        let points = self.points.iter().map(|((column, label), &count)| Entry::Point {
            column: *column,
            label: label.clone(),
            reached: count > 0,
        });
        let branches = self.branches.iter().map(|(&column, (operator, arms))| Entry::Decision {
            column,
            operator: operator.clone(),
            reached: [arms[0] > 0, arms[1] > 0],
        });
        let mut entries: Vec<_> = points.chain(branches).collect();
        entries.sort_by_key(Entry::column);
        entries
    }
}

impl Report {
    /// Count that the point was hit `count` times.
    pub fn add(&mut self, point: &Point, count: u64) {
        let file = self.files.entry(point.file.clone()).or_default();
        let line = file.entry(point.line).or_default();
        match point.kind {
            Kind::Branch { outcome } => {
                let entry = line
                    .branches
                    .entry(point.column)
                    .or_insert_with(|| (point.name.clone(), [0; 2]));
                entry.1[!outcome as usize] += count;
            }
            _ => *line.points.entry((point.column, point.label())).or_default() += count,
        }
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
                for entry in coverage.entries() {
                    for (what, reached) in entry.items() {
                        if !reached {
                            eprintln!("  {path}:{line}:{}: {what} never reached", entry.column());
                            gaps += 1;
                        }
                    }
                }
            }
        }
        gaps
    }

    /// Every point and decision of `file`, by line, in column order.
    pub fn lines_of(&self, file: &Path) -> BTreeMap<usize, Vec<Entry>> {
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
                for entry in coverage.entries() {
                    for (what, reached) in entry.items() {
                        let status = if reached { '+' } else { '-' };
                        listing.push(format!("{status} {path}:{line}:{} {what}", entry.column()));
                    }
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

    fn point(kind: Kind, name: &str, line: usize, column: usize) -> Point {
        Point { kind, name: name.into(), file: "/src/a.slint".into(), line, column }
    }

    fn report() -> Report {
        let mut report = Report::default();
        report.add(&point(Kind::Element, "Window", 7, 36), 3);
        report.add(&point(Kind::Binding, "pick", 13, 30), 4);
        report.add(&point(Kind::Branch { outcome: true }, "?", 13, 37), 4);
        report.add(&point(Kind::Branch { outcome: false }, "?", 13, 37), 0);
        // Inlined twice, the counts add up.
        report.add(&point(Kind::Binding, "len", 13, 50), 1);
        report.add(&point(Kind::Binding, "len", 13, 50), 2);
        // The operator of a decision written over several lines is alone on
        // its line, which the decision alone counts.
        report.add(&point(Kind::Branch { outcome: true }, "?", 15, 9), 2);
        report.add(&point(Kind::Branch { outcome: false }, "?", 15, 9), 1);
        report.add(&point(Kind::Handler, "clicked", 20, 5), 0);
        report.add(
            &Point { file: "/src/lib/b.slint".into(), ..point(Kind::Element, "Led", 2, 1) },
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
BRDA:15,0,0,2
BRDA:15,0,1,1
BRF:4
BRH:3
DA:7,3
DA:13,7
DA:15,3
DA:20,0
LF:4
LH:3
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
        assert_eq!(lines.keys().copied().collect::<Vec<_>>(), [7, 13, 15, 20]);
        assert_eq!(
            lines[&13],
            [
                Entry::Point { column: 30, label: "binding pick".into(), reached: true },
                Entry::Decision { column: 37, operator: "?".into(), reached: [true, false] },
                Entry::Point { column: 50, label: "binding len".into(), reached: true },
            ]
        );
    }
}
