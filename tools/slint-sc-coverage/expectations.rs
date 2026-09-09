// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The coverage a test case expects, stated in its source.
//!
//! A `//#cov(...)` comment ends every line holding coverage points and
//! states them in reading order, `+` when reached and `-` when not: an
//! element by the type it is written with, a binding, handler or call by its
//! name, and a decision by its operator and the status of its true and its
//! false outcome:
//!
//! ```slint
//! Rectangle {                                       //#cov(Rectangle+)
//!     background: alarm && muted ? red : gray;      //#cov(background+ &&+- ?+-)
//!     TouchArea { clicked => { root.pressed(); } }  //#cov(TouchArea+ clicked- pressed-)
//! }
//! ```
//!
//! A case with an annotation states its whole coverage: a line without one
//! has no point. A case without any is not compared; to state one, write
//! `//#cov()` anywhere and have the driver rewrite it. A point in another
//! file, which the case cannot annotate, is stated in a ```` ```coverage ````
//! block at the end of the case, one line each as [`Report::listing`] prints
//! it.

use crate::Report;
use std::collections::BTreeMap;
use std::path::Path;

/// What an annotation starts with.
const OPENER: &str = "//#cov(";
/// The environment variable that has the test driver rewrite what a case
/// states from the measurement.
pub const UPDATE_VAR: &str = "SLINT_COVERAGE_TEST_UPDATE";

/// Whether the case states its coverage: it has an annotation.
pub fn is_stated(source: &str) -> bool {
    source.lines().any(|line| split(line).is_some())
}

/// Check that the coverage the case states is the one measured, and describe
/// the difference.
pub fn check(source: &str, case: &Path, report: &Report) -> Result<(), String> {
    if !is_stated(source) {
        return Ok(());
    }
    for (i, line) in source.lines().enumerate() {
        annotation(line).map_err(|e| format!("line {}: {e}", i + 1))?;
    }
    let mut message = String::new();
    let annotated = annotate(source, &report.lines_of(case));
    for (i, (stated, measured)) in source.lines().zip(annotated.lines()).enumerate() {
        if stated != measured {
            let line = i + 1;
            message
                .push_str(&format!("line {line}:\n  stated:   {stated}\n  measured: {measured}\n"));
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
    with_block(&annotate(source, &report.lines_of(case)), &report.listing(case))
}

/// The items an annotation states, when the line ends with one.
fn annotation(line: &str) -> Result<Option<Vec<&str>>, String> {
    let Some((_, annotation)) = split(line) else {
        return Ok(None);
    };
    let inner = annotation[OPENER.len()..]
        .strip_suffix(')')
        .ok_or("the annotation must end the line with `)`")?;
    let items: Vec<&str> = inner.split(' ').collect();
    if let Some(item) = items.iter().find(|item| !is_item(item)) {
        return Err(format!(
            "`{item}` is not a coverage item: a name and `+` or `-` for an element, binding, \
             handler or call, `?`, `&&` or `||` and both outcomes for a decision"
        ));
    }
    Ok(Some(items))
}

/// The line's code and its annotation, when it has one.
fn split(line: &str) -> Option<(&str, &str)> {
    line.rfind(OPENER).map(|at| line.split_at(at))
}

fn is_item(item: &str) -> bool {
    let is_status = |c: char| c == '+' || c == '-';
    if let Some(outcomes) = ["?", "&&", "||"].iter().find_map(|op| item.strip_prefix(op)) {
        return outcomes.len() == 2 && outcomes.chars().all(is_status);
    }
    let Some(name) = item.strip_suffix(is_status) else {
        return false;
    };
    name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The source with the annotation of each line rewritten from the points
/// measured on it, in column order.
fn annotate(source: &str, lines: &BTreeMap<usize, Vec<(usize, String, bool)>>) -> String {
    let mut out = String::with_capacity(source.len());
    for (i, line) in source.split_inclusive('\n').enumerate() {
        let (line, newline) = match line.strip_suffix('\n') {
            Some(line) => (line, "\n"),
            None => (line, ""),
        };
        let (code, annotated) = match split(line) {
            Some((code, _)) => (code, true),
            None => (line, false),
        };
        match lines.get(&(i + 1)) {
            Some(entries) => {
                // A new annotation follows the code after a space, an existing
                // one keeps its spacing.
                out.push_str(if annotated { code } else { line.trim_end() });
                if !annotated {
                    out.push(' ');
                }
                out.push_str(&format!("{OPENER}{})", items(entries).join(" ")));
            }
            None if annotated => out.push_str(code.trim_end()),
            None => out.push_str(line),
        }
        out.push_str(newline);
    }
    out
}

/// The items stating the points measured on a line: the two outcomes of a
/// decision, listed one after the other, make one item.
fn items(entries: &[(usize, String, bool)]) -> Vec<String> {
    let status = |reached: bool| if reached { '+' } else { '-' };
    let mut items = Vec::new();
    let mut entries = entries.iter();
    while let Some((_, what, reached)) = entries.next() {
        let (kind, name) = what.split_once(' ').unwrap_or((what, ""));
        items.push(match kind {
            "branch" => {
                let op = name.strip_suffix(" true").unwrap_or(name);
                let (_, _, false_reached) =
                    entries.next().expect("the false outcome follows the true one");
                format!("{op}{}{}", status(*reached), status(*false_reached))
            }
            _ => format!("{name}{}", status(*reached)),
        });
    }
    items
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
    use crate::Point;

    const CASE: &str = "/src/a.slint";

    fn add(report: &mut Report, what: &str, line: usize, column: usize, count: u64) {
        let mut words = what.split(' ');
        let kind = words.next().unwrap();
        let name = words.next().map(String::from);
        let branch = words.next().map(|outcome| (name.clone().unwrap(), outcome == "true"));
        let point = Point { kind: kind.into(), name, file: CASE.into(), line, column, branch };
        report.add(&point, count);
    }

    fn report() -> Report {
        let mut report = Report::default();
        add(&mut report, "element Window", 1, 36, 1);
        add(&mut report, "binding level", 2, 31, 4);
        add(&mut report, "branch && true", 2, 33, 4);
        add(&mut report, "branch && false", 2, 33, 0);
        add(&mut report, "branch ? true", 2, 38, 2);
        add(&mut report, "branch ? false", 2, 38, 2);
        add(&mut report, "element TouchArea", 3, 5, 1);
        add(&mut report, "handler clicked", 3, 17, 0);
        add(&mut report, "call pressed", 3, 30, 0);
        report.add(
            &Point {
                kind: "element".into(),
                name: Some("Led".into()),
                file: "/src/lib/b.slint".into(),
                line: 2,
                column: 1,
                branch: None,
            },
            3,
        );
        report
    }

    const SOURCE: &str = "export component TestCase inherits Window { //#cov()
    out property <int> level: a && b ? 1 : 2;
    TouchArea { clicked => { root.pressed(); } } // a comment
}

/*
```coverage
```
*/
";

    const STATED: &str = "export component TestCase inherits Window { //#cov(Window+)
    out property <int> level: a && b ? 1 : 2; //#cov(level+ &&+- ?++)
    TouchArea { clicked => { root.pressed(); } } // a comment //#cov(TouchArea+ clicked- pressed-)
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
        assert_eq!(update(SOURCE, case, &report()).unwrap(), STATED);
        assert!(check(STATED, case, &report()).is_ok());
        // An existing annotation keeps its spacing, a stale one goes.
        let aligned = STATED.replace("Window { //#cov(Window+)", "Window {    //#cov(Window+)");
        assert_eq!(update(&aligned, case, &report()).unwrap(), aligned);
        let stale = STATED.replace("}\n\n/*", "} //#cov(Rectangle+)\n\n/*");
        assert_eq!(update(&stale, case, &report()).unwrap(), STATED);
        // Without an annotation, nothing is stated.
        let unstated = SOURCE.replace(" //#cov()", "");
        assert_eq!(update(&unstated, case, &report()).unwrap(), unstated);
        assert!(check(&unstated, case, &report()).is_ok());
    }

    #[test]
    fn checking() {
        let case = Path::new(CASE);
        let differing = STATED.replace("&&+- ?++", "&&++ ?++").replace("pressed-", "pressed+");
        let error = check(&differing, case, &report()).unwrap_err();
        assert_eq!(
            error,
            "the coverage differs from what the case states:
line 2:
  stated:       out property <int> level: a && b ? 1 : 2; //#cov(level+ &&++ ?++)
  measured:     out property <int> level: a && b ? 1 : 2; //#cov(level+ &&+- ?++)
line 3:
  stated:       TouchArea { clicked => { root.pressed(); } } // a comment //#cov(TouchArea+ clicked- pressed+)
  measured:     TouchArea { clicked => { root.pressed(); } } // a comment //#cov(TouchArea+ clicked- pressed-)
set SLINT_COVERAGE_TEST_UPDATE=1 to rewrite the case"
        );
        let missing = STATED.replace("+ lib/b.slint:2:1 element Led\n", "");
        let error = check(&missing, case, &report()).unwrap_err();
        assert!(error.contains("the ```coverage block differs"), "{error}");
        assert!(error.contains("\n  + lib/b.slint:2:1 element Led\n"), "{error}");
    }

    #[test]
    fn malformed() {
        let case = Path::new(CASE);
        for (item, wrong) in [
            ("level+", "level +"),
            ("level+", "level"),
            ("level+", "+level"),
            ("level+", "+"),
            ("&&+-", "&& +-"),
            ("&&+-", "&&+"),
            ("?++", "?+++"),
            ("clicked-", "clicked-)"),
            ("(Window+)", "()"),
        ] {
            let wrong = STATED.replacen(item, wrong, 1);
            let error = check(&wrong, case, &report()).unwrap_err();
            assert!(error.starts_with("line "), "{item}: {error}");
            assert!(error.contains("not a coverage item") || error.contains("`)`"), "{error}");
        }
    }
}
