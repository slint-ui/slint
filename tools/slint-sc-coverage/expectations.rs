// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The coverage a test case expects, stated in its source.
//!
//! A `//#c` line follows every line holding coverage points, with a caret
//! in the column of each: `^+` when the point was reached and `^-` when not,
//! and for a decision the status of its true and its false outcome, like
//! `^+-`:
//!
//! ```slint
//! Rectangle {
//! //#c^+
//!     background: alarm && muted ? red : gray;
//! //#c            ^+    ^+-      ^+-
//!     TouchArea { clicked => { root.pressed(); } }
//! //#c^+          ^-           ^-
//! }
//! ```
//!
//! A caret line starts in the first column, so a point in the four columns
//! of `//#c` cannot be stated. A case with a caret line states its whole
//! coverage: a line without one has no point. A case without any is not
//! compared; to state one, write a `//#c` line anywhere and have the driver
//! rewrite it. A point in another file, which the case cannot annotate, is
//! stated in a ```` ```coverage ```` block at the end of the case, one line
//! each as [`Report::listing`] prints it.

use crate::{Entry, Report};
use std::collections::BTreeMap;
use std::path::Path;

/// What a caret line starts with.
const OPENER: &str = "//#c";
/// The marks of a caret: the status of a point, or of both outcomes of a
/// decision.
const MARKS: [&str; 6] = ["++", "+-", "-+", "--", "+", "-"];
const EXPECTED: &str = "`^+` or `^-` for a point, `^++`, `^+-`, `^-+` or `^--` for a decision";
/// The environment variable that has the test driver rewrite what a case
/// states from the measurement.
pub const UPDATE_VAR: &str = "SLINT_COVERAGE_TEST_UPDATE";

/// Whether the case states its coverage: it has a caret line.
pub fn is_stated(source: &str) -> bool {
    source.lines().any(is_caret_line)
}

/// Check that the coverage the case states is the one measured, and describe
/// the difference.
pub fn check(source: &str, case: &Path, report: &Report) -> Result<(), String> {
    if !is_stated(source) {
        return Ok(());
    }
    for (i, line) in source.lines().enumerate() {
        if is_caret_line(line) {
            check_caret_line(line).map_err(|e| format!("line {}: {e}", i + 1))?;
        }
    }
    let mut message = String::new();
    let annotated = annotate(source, &report.lines_of(case))?;
    for ((line, code, stated), (_, _, measured)) in
        code_lines(source).into_iter().zip(code_lines(&annotated))
    {
        if stated != measured {
            let stated = stated.unwrap_or("(no caret line)");
            let measured = measured.unwrap_or("(no caret line)");
            message.push_str(&format!(
                "line {line}:\n  {code}\n  stated:   {stated}\n  measured: {measured}\n"
            ));
        }
    }
    let listing = report.listing(case);
    if with_block(&annotated, &listing)? != annotated {
        message.push_str("the ```coverage block differs from the points of the other files:\n");
        for line in &listing {
            message.push_str(&format!("  {line}\n"));
        }
    }
    if message.is_empty() {
        return Ok(());
    }
    Err(format!(
        "the coverage differs from what the case states:\n{message}set {UPDATE_VAR}=1 to rewrite the case"
    ))
}

/// The source with what it states rewritten from the measurement.
pub fn update(source: &str, case: &Path, report: &Report) -> Result<String, String> {
    if !is_stated(source) {
        return Ok(source.to_string());
    }
    with_block(&annotate(source, &report.lines_of(case))?, &report.listing(case))
}

/// Whether the line is a caret line. An indented one is a caret line too, so
/// that it is rejected rather than taken for a comment.
fn is_caret_line(line: &str) -> bool {
    line.trim_start().starts_with(OPENER)
}

/// Check that a caret line is well-formed: carets with their marks, apart
/// or one after the other, and nothing else. A caret line that is not is
/// rejected on its own, before what it states is compared.
fn check_caret_line(line: &str) -> Result<(), String> {
    let mut rest = line.strip_prefix(OPENER).ok_or("a caret line starts in the first column")?;
    let mut column = OPENER.len() + 1;
    let mut carets = 0;
    loop {
        let spaces = rest.len() - rest.trim_start_matches(' ').len();
        rest = &rest[spaces..];
        column += spaces;
        if rest.is_empty() {
            break;
        }
        let marks = rest.strip_prefix('^').and_then(|after| {
            MARKS.into_iter().find(|marks| {
                let next = after.strip_prefix(marks).and_then(|next| next.chars().next());
                after.starts_with(marks) && matches!(next, None | Some(' ' | '^'))
            })
        });
        let Some(marks) = marks else {
            return Err(format!("column {column}: expected {EXPECTED}"));
        };
        carets += 1;
        rest = &rest[1 + marks.len()..];
        column += 1 + marks.len();
    }
    if carets == 0 {
        return Err("a caret line without a caret".into());
    }
    Ok(())
}

/// The source with the caret line of each line rewritten from the points
/// measured on it. Fails when a point sits in a column a caret cannot
/// reach, which is why a case states such a point in its ```` ```coverage ````
/// block instead.
pub fn annotate(source: &str, lines: &BTreeMap<usize, Vec<Entry>>) -> Result<String, String> {
    let mut out = String::with_capacity(source.len());
    for (i, line) in source.split_inclusive('\n').enumerate() {
        let (line, newline) = match line.strip_suffix('\n') {
            Some(line) => (line, "\n"),
            None => (line, ""),
        };
        if is_caret_line(line) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
        if let Some(entries) = lines.get(&(i + 1)) {
            out.push_str(&caret_line(entries).map_err(|e| format!("line {}: {e}", i + 1))?);
            out.push('\n');
        }
        if newline.is_empty() {
            out.pop();
        }
    }
    Ok(out)
}

/// The caret line stating the points measured on a line: a caret per point,
/// and one per decision with the status of both outcomes.
fn caret_line(entries: &[Entry]) -> Result<String, String> {
    let status = |reached: bool| if reached { '+' } else { '-' };
    let mut out = String::from(OPENER);
    for entry in entries {
        let column = entry.column();
        let marks: String = match entry {
            Entry::Point { count, .. } => status(*count > 0).into(),
            Entry::Decision { counts, .. } => counts.iter().map(|&c| status(c > 0)).collect(),
        };
        if column <= out.len() {
            return Err(format!(
                "no room for a caret in column {column}: the caret line reaches column {}",
                out.len()
            ));
        }
        out.extend(std::iter::repeat_n(' ', column - 1 - out.len()));
        out.push('^');
        out.push_str(&marks);
    }
    Ok(out)
}

/// Every line but the caret lines, with its number and the caret line that
/// follows it, if one does.
fn code_lines(source: &str) -> Vec<(usize, &str, Option<&str>)> {
    let mut lines = Vec::new();
    for (i, line) in source.lines().enumerate() {
        match (is_caret_line(line), lines.last_mut()) {
            (true, Some((_, _, carets))) => *carets = Some(line),
            (true, None) => {}
            (false, _) => lines.push((i + 1, line, None)),
        }
    }
    lines
}

/// The source with the content of its ```` ```coverage ```` block replaced
/// by the listing of the other files' points.
fn with_block(source: &str, listing: &[String]) -> Result<String, String> {
    const FENCE: &str = "```coverage\n";
    let Some(start) = source.find(FENCE) else {
        if listing.is_empty() {
            return Ok(source.to_string());
        }
        return Err(format!(
            "points in other files, and no ```coverage block to state them in:\n{}",
            listing.join("\n")
        ));
    };
    let content = start + FENCE.len();
    let end = content + source[content..].find("```").ok_or("unterminated ```coverage block")?;
    let mut block = listing.join("\n");
    if !block.is_empty() {
        block.push('\n');
    }
    Ok(format!("{}{block}{}", &source[..content], &source[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Kind, Point};

    const CASE: &str = "/src/a.slint";

    fn add(report: &mut Report, kind: Kind, name: &str, line: usize, column: usize, count: u64) {
        let point = Point { kind, name: name.into(), file: CASE.into(), line, column };
        report.add(&point, count);
    }

    /// The report of the case, the `level` and the `TouchArea` lines at the
    /// given lines: the caret lines shift them.
    fn report(level: usize, touch: usize) -> Report {
        let mut report = Report::default();
        add(&mut report, Kind::Element, "Window", 1, 36, 1);
        add(&mut report, Kind::Binding, "level", level, 31, 4);
        add(&mut report, Kind::Branch { outcome: true }, "&&", level, 33, 4);
        add(&mut report, Kind::Branch { outcome: false }, "&&", level, 33, 0);
        add(&mut report, Kind::Branch { outcome: true }, "?", level, 38, 2);
        add(&mut report, Kind::Branch { outcome: false }, "?", level, 38, 2);
        add(&mut report, Kind::Element, "TouchArea", touch, 5, 1);
        add(&mut report, Kind::Handler, "clicked", touch, 17, 0);
        add(&mut report, Kind::Call, "pressed", touch, 30, 0);
        report.add(
            &Point {
                kind: Kind::Element,
                name: "Led".into(),
                file: "/src/lib/b.slint".into(),
                line: 2,
                column: 1,
            },
            3,
        );
        report
    }

    const SOURCE: &str = "export component TestCase inherits Window {
//#c
    out property <int> level: a && b ? 1 : 2;
    TouchArea { clicked => { root.pressed(); } }
}

/*
```coverage
```
*/
";

    const STATED: &str = "export component TestCase inherits Window {
//#c                               ^+
    out property <int> level: a && b ? 1 : 2;
//#c                          ^+^+-  ^++
    TouchArea { clicked => { root.pressed(); } }
//#c^+          ^-           ^-
}

/*
```coverage
+ lib/b.slint:2:1 element Led
```
*/
";

    #[test]
    fn updating() {
        let case = Path::new(CASE);
        assert_eq!(update(SOURCE, case, &report(3, 4)).unwrap(), STATED);
        assert!(check(STATED, case, &report(3, 5)).is_ok());
        // Updating what is stated changes nothing; a stale caret line goes.
        assert_eq!(update(STATED, case, &report(3, 5)).unwrap(), STATED);
        let stale = STATED.replace("}\n\n/*", "}\n//#c ^+\n\n/*");
        assert_eq!(update(&stale, case, &report(3, 5)).unwrap(), STATED);
        // Without a caret line, nothing is stated.
        let unstated = SOURCE.replacen("//#c\n", "", 1);
        assert_eq!(update(&unstated, case, &report(2, 3)).unwrap(), unstated);
        assert!(check(&unstated, case, &report(2, 3)).is_ok());
        // A point under the opener cannot be stated.
        let mut report = report(3, 4);
        add(&mut report, Kind::Element, "Rectangle", 5, 1, 1);
        let error = update(SOURCE, case, &report).unwrap_err();
        assert_eq!(
            error,
            "line 5: no room for a caret in column 1: the caret line reaches column 4"
        );
    }

    #[test]
    fn checking() {
        let case = Path::new(CASE);
        let differing = STATED
            .replace("^+^+-  ^++", "^+^++  ^++")
            .replace("//#c^+          ^-           ^-\n", "");
        let error = check(&differing, case, &report(3, 5)).unwrap_err();
        assert_eq!(
            error,
            "the coverage differs from what the case states:
line 3:
      out property <int> level: a && b ? 1 : 2;
  stated:   //#c                          ^+^++  ^++
  measured: //#c                          ^+^+-  ^++
line 5:
      TouchArea { clicked => { root.pressed(); } }
  stated:   (no caret line)
  measured: //#c^+          ^-           ^-
set SLINT_COVERAGE_TEST_UPDATE=1 to rewrite the case"
        );
        let missing = STATED.replace("+ lib/b.slint:2:1 element Led\n", "");
        let error = check(&missing, case, &report(3, 5)).unwrap_err();
        assert!(error.contains("the ```coverage block differs"), "{error}");
        assert!(error.contains("\n  + lib/b.slint:2:1 element Led\n"), "{error}");
    }

    #[test]
    fn malformed() {
        let case = Path::new(CASE);
        let good = "//#c^+          ^-           ^-";
        for (wrong, error) in [
            (
                "//#c^ +          ^-           ^-",
                "line 6: column 5: expected `^+` or `^-` for a point, `^++`, `^+-`, `^-+` or `^--` for a decision",
            ),
            (
                "//#c^+          ^ -           ^-",
                "line 6: column 17: expected `^+` or `^-` for a point, `^++`, `^+-`, `^-+` or `^--` for a decision",
            ),
            (
                "//#c^+          ^-+-          ^-",
                "line 6: column 17: expected `^+` or `^-` for a point, `^++`, `^+-`, `^-+` or `^--` for a decision",
            ),
            (
                "//#c^+          ^-           ^",
                "line 6: column 30: expected `^+` or `^-` for a point, `^++`, `^+-`, `^-+` or `^--` for a decision",
            ),
            (
                "//#c^+          ^-           ^- x",
                "line 6: column 33: expected `^+` or `^-` for a point, `^++`, `^+-`, `^-+` or `^--` for a decision",
            ),
            (" //#c^+          ^-           ^-", "line 6: a caret line starts in the first column"),
            (
                "//#c^…",
                "line 6: column 5: expected `^+` or `^-` for a point, `^++`, `^+-`, `^-+` or `^--` for a decision",
            ),
            ("//#c ", "line 6: a caret line without a caret"),
        ] {
            let source = STATED.replacen(good, wrong, 1);
            assert_eq!(check(&source, case, &report(3, 5)).unwrap_err(), error, "{wrong}");
        }
    }
}
