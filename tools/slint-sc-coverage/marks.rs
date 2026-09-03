// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Coverage points marked in the generated code, counted by LLVM.
//!
//! The generated code marks a point with
//! `origin!("<kind>[ <decision> <outcome>] <span> <path>")`, a macro that
//! expands to nothing, where the point is reached. LLVM measures the
//! generated code like any other; the execution count of the region a mark
//! precedes is the point's hit count.

use crate::{Point, Report};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A position in a file: line and column, both 1-based, the column in
/// characters like LLVM's.
pub type Position = (usize, usize);

/// The code regions of an `llvm-cov export`, by file.
#[derive(Default)]
pub struct Regions {
    pub files: BTreeMap<PathBuf, FileRegions>,
}

/// The code regions of one file, by function: their start, end, and
/// execution count.
#[derive(Default)]
pub struct FileRegions {
    functions: Vec<Vec<(Position, Position, u64)>>,
}

impl Regions {
    /// Read the code regions of an `llvm-cov export` in JSON: for each
    /// function, its `filenames`, and its `regions` as `[start line, start
    /// column, end line, end column, count, file index, expanded file index,
    /// kind]`, of which only the code regions (kind 0) count. Relative file
    /// names are relative to `base_dir`.
    pub fn parse(&mut self, export: &str, base_dir: &Path) -> Result<(), String> {
        let json: serde_json::Value =
            serde_json::from_str(export).map_err(|e| format!("not a coverage export: {e}"))?;
        let data = json["data"].as_array().ok_or("not a coverage export: no `data`")?;
        let functions = data.iter().flat_map(|d| d["functions"].as_array().into_iter().flatten());
        for function in functions {
            let filenames: Vec<PathBuf> = function["filenames"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|f| f.as_str())
                .map(|f| {
                    let path = base_dir.join(f);
                    path.canonicalize().unwrap_or(path)
                })
                .collect();
            let mut by_file: BTreeMap<&Path, Vec<(Position, Position, u64)>> = BTreeMap::new();
            for region in function["regions"].as_array().into_iter().flatten() {
                let field = |i: usize| region[i].as_u64().unwrap_or(0);
                if field(7) != 0 {
                    continue;
                }
                let Some(file) = filenames.get(field(5) as usize) else { continue };
                let start = (field(0) as usize, field(1) as usize);
                let end = (field(2) as usize, field(3) as usize);
                by_file.entry(file).or_default().push((start, end, field(4)));
            }
            for (file, mut regions) in by_file {
                regions.sort();
                self.files.entry(file.to_path_buf()).or_default().functions.push(regions);
            }
        }
        Ok(())
    }
}

impl FileRegions {
    /// The execution count of the code a mark at `position` belongs to, in
    /// the innermost function around it: the innermost region holding the
    /// position, which rustc makes of a block holding nothing but the mark,
    /// else the first region after it, the code the mark precedes. A mark
    /// expands to nothing, so it is never in a region of its own. An export
    /// of several binaries lists a function once per binary, whose counts add
    /// up.
    pub fn count_at(&self, position: Position) -> u64 {
        let extent = |regions: &Vec<(Position, Position, u64)>| {
            (regions.first().map(|r| r.0), regions.iter().map(|r| r.1).max())
        };
        let around: Vec<_> = self
            .functions
            .iter()
            .filter(|regions| {
                let (start, end) = extent(regions);
                start.is_some_and(|s| s <= position) && end.is_some_and(|e| position < e)
            })
            .collect();
        let Some(innermost) = around.iter().map(|regions| extent(regions).0).max() else {
            return 0;
        };
        around
            .iter()
            .filter(|regions| extent(regions).0 == innermost)
            .map(|regions| {
                let holding = regions
                    .iter()
                    .filter(|(start, end, _)| *start <= position && position < *end)
                    .max_by_key(|(start, _, _)| *start);
                let next = regions.iter().find(|(start, _, _)| *start >= position);
                let enclosing = regions.iter().rev().find(|(start, _, _)| *start < position);
                holding.or(next).or(enclosing).map_or(0, |(_, _, count)| *count)
            })
            .sum()
    }
}

/// The marks in generated code, with their position: each `origin!("...")`
/// invocation, as the compiler's token stream prints it.
pub fn marks(text: &str) -> Vec<(Position, String)> {
    const NAME: &str = "origin";
    let mut marks = Vec::new();
    let (mut line, mut column) = (1, 1);
    let mut rest = text;
    while let Some(found) = rest.find(NAME) {
        for c in rest[..found].chars() {
            if c == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        rest = &rest[found + NAME.len()..];
        let call = rest.trim_start().strip_prefix('!').map(str::trim_start);
        if let Some(call) = call.and_then(|call| call.strip_prefix('('))
            && let Some(literal) = call.trim_start().strip_prefix('"')
            && let Some(text) = string_literal(literal)
        {
            marks.push(((line, column), text));
        }
        column += NAME.len();
    }
    marks
}

/// The text of a string literal up to its closing quote.
fn string_literal(literal: &str) -> Option<String> {
    let mut text = String::new();
    let mut chars = literal.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(text),
            '\\' => text.push(chars.next()?),
            c => text.push(c),
        }
    }
    None
}

/// A mark is `<kind>[ <decision> <outcome>] <span> <path>`, the span as
/// `line:column-line:column`.
pub fn parse_mark(mark: &str) -> Result<Point, String> {
    let bad = || format!("bad mark `{mark}`");
    let parse = |s: &str| s.parse::<usize>().map_err(|_| bad());
    let (kind, rest) = mark.split_once(' ').ok_or_else(bad)?;
    let (branch, rest) = match kind {
        "branch" => {
            let (decision, rest) = rest.split_once(' ').ok_or_else(bad)?;
            let (outcome, rest) = rest.split_once(' ').ok_or_else(bad)?;
            (Some((parse(decision)?, outcome == "true")), rest)
        }
        _ => (None, rest),
    };
    let (span, path) = rest.split_once(' ').ok_or_else(bad)?;
    let (start, _end) = span.split_once('-').ok_or_else(bad)?;
    let (line, column) = start.split_once(':').ok_or_else(bad)?;
    Ok(Point {
        kind: kind.to_string(),
        file: PathBuf::from(path),
        line: parse(line)?,
        column: parse(column)?,
        branch,
    })
}

/// Add the points marked in a generated file to the report, with the
/// counts of the code around the marks. Returns how many marks it found.
pub fn add(generated: &str, regions: &FileRegions, report: &mut Report) -> Result<usize, String> {
    let marks = marks(generated);
    for (position, mark) in &marks {
        report.add(&parse_mark(mark)?, regions.count_at(*position));
    }
    Ok(marks.len())
}

/// Export the coverage of the binaries from the profiles with `llvm-cov`,
/// as JSON.
pub fn export_coverage(objects: &[PathBuf], profiles: &[PathBuf]) -> Result<String, String> {
    // One merged profile per call: calls run concurrently in a test driver.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let profdata = std::env::temp_dir()
        .join(format!("slint-sc-coverage-{}-{call}.profdata", std::process::id()));
    let merge = Command::new(find_llvm_tool("llvm-profdata"))
        .arg("merge")
        .arg("-o")
        .arg(&profdata)
        .args(profiles)
        .output()
        .map_err(|e| format!("cannot run llvm-profdata: {e}"))?;
    if !merge.status.success() {
        return Err(format!("llvm-profdata failed:\n{}", String::from_utf8_lossy(&merge.stderr)));
    }
    let mut export = Command::new(find_llvm_tool("llvm-cov"));
    export.arg("export").arg("-format=text").arg("-instr-profile").arg(&profdata);
    export.arg(&objects[0]);
    for object in &objects[1..] {
        export.arg("-object").arg(object);
    }
    let output = export.output();
    let _ = std::fs::remove_file(&profdata);
    let output = output.map_err(|e| format!("cannot run llvm-cov: {e}"))?;
    if !output.status.success() {
        return Err(format!("llvm-cov failed:\n{}", String::from_utf8_lossy(&output.stderr)));
    }
    String::from_utf8(output.stdout).map_err(|e| e.to_string())
}

/// An LLVM tool: from the environment variable of its upper-cased name, the
/// Rust toolchain's llvm-tools component, or PATH.
fn find_llvm_tool(name: &str) -> PathBuf {
    if let Some(path) = std::env::var_os(name.to_uppercase().replace('-', "_")) {
        return path.into();
    }
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let run = |args: &[&str]| {
        Command::new(&rustc)
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
    };
    let sysroot = run(&["--print", "sysroot"]);
    let host = run(&["-vV"]).and_then(|verbose| {
        verbose.lines().find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
    });
    if let (Some(sysroot), Some(host)) = (sysroot, host) {
        let path = Path::new(sysroot.trim())
            .join("lib/rustlib")
            .join(host.trim())
            .join("bin")
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        if path.exists() {
            return path;
        }
    }
    name.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generated code as the compiler prints it: one line, tokens spaced.
    const GENERATED: &str = r#"pub fn get_pick (& self) -> i32 { slint_sc :: private_unstable_api :: origin ! ("binding 13:30-13:42 /src/ternary.slint") ; if self . property_a { slint_sc :: private_unstable_api :: origin ! ("branch 0 true 13:30-13:42 /src/ternary.slint") ; 7i32 } else { slint_sc :: private_unstable_api :: origin ! ("branch 0 false 13:30-13:42 /src/ternary.slint") ; 3i32 } }
"#;

    /// The column after `needle` in the generated code.
    fn after(needle: &str) -> usize {
        GENERATED.find(needle).unwrap() + needle.len() + 1
    }

    /// The export of that code as rustc instruments it: regions for the
    /// signature, the condition and each arm's value, none for the marks;
    /// once per binary of the export, one that ran the function, one not.
    fn export() -> String {
        let region = |start: usize, len: usize, count: u64| {
            format!("[1,{start},1,{},{count},0,0,0]", start + len)
        };
        let function = |counts: [u64; 4]| {
            format!(
                r#"{{"name":"get_pick","filenames":["gen/out.rs"],"regions":[{},{},{},{}]}}"#,
                region(1, "pub fn get_pick (& self) -> i32 {".len(), counts[0]),
                region(after("if "), "self . property_a".len(), counts[1]),
                region(after("true 13:30-13:42 /src/ternary.slint\") ; "), 4, counts[2]),
                region(after("false 13:30-13:42 /src/ternary.slint\") ; "), 4, counts[3]),
            )
        };
        // A function whose arms hold nothing but a mark: the arms are regions.
        let empty_arms = format!(
            r#"{{"name":"empty","filenames":["gen/empty.rs"],"regions":[{},{},{}]}}"#,
            region(1, EMPTY_ARMS.find('{').unwrap() + 1, 2),
            region(
                EMPTY_ARMS.find("{ slint").unwrap() + 1,
                EMPTY_ARMS.find("} else").unwrap() + 2 - EMPTY_ARMS.find("{ slint").unwrap(),
                2
            ),
            region(
                EMPTY_ARMS.rfind("{ slint").unwrap() + 1,
                EMPTY_ARMS.len() - EMPTY_ARMS.rfind("{ slint").unwrap(),
                0
            ),
        );
        format!(
            r#"{{"data":[{{"functions":[{},{},{}]}}]}}"#,
            function([4, 4, 4, 0]),
            function([0; 4]),
            empty_arms
        )
    }

    const EMPTY_ARMS: &str = r#"fn h (x : bool) { if x { slint_sc :: private_unstable_api :: origin ! ("branch 0 true 5:1-5:9 /src/a.slint") ; } else { slint_sc :: private_unstable_api :: origin ! ("branch 0 false 5:1-5:9 /src/a.slint") ; } }"#;

    #[test]
    fn finds_marks() {
        let found = marks(GENERATED);
        let texts: Vec<&str> = found.iter().map(|(_, text)| text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "binding 13:30-13:42 /src/ternary.slint",
                "branch 0 true 13:30-13:42 /src/ternary.slint",
                "branch 0 false 13:30-13:42 /src/ternary.slint"
            ]
        );
        assert_eq!(found[0].0, (1, GENERATED.find("origin").unwrap() + 1));
        assert_eq!(found[2].0, (1, GENERATED.rfind("origin").unwrap() + 1));
        assert_eq!(marks(r#"origin ! ("a \"b\" c\\d")"#)[0].1, r#"a "b" c\d"#);
    }

    #[test]
    fn marks_to_report() {
        let mut regions = Regions::default();
        regions.parse(&export(), Path::new("/base")).unwrap();
        let file = &regions.files[Path::new("/base/gen/out.rs")];
        let mut report = Report::default();
        assert_eq!(add(GENERATED, file, &mut report).unwrap(), 3);
        assert_eq!(
            report.listing(Path::new("/src/ternary.slint")),
            ["+ 13:30 binding", "+ 13:30 branch 0 true", "- 13:30 branch 0 false"]
        );
        // Outside every function.
        assert_eq!(file.count_at((2, 1)), 0);
        // Marks alone in their arms are in the arms' regions.
        let empty = &regions.files[Path::new("/base/gen/empty.rs")];
        let mut report = Report::default();
        assert_eq!(add(EMPTY_ARMS, empty, &mut report).unwrap(), 2);
        assert_eq!(
            report.listing(Path::new("/src/a.slint")),
            ["+ 5:1 branch 0 true", "- 5:1 branch 0 false"]
        );
    }

    #[test]
    fn mark_with_spaces_in_path() {
        let point = parse_mark("element 7:36-7:43 /my dir/a.slint").unwrap();
        assert_eq!((point.kind.as_str(), point.line, point.column), ("element", 7, 36));
        assert_eq!(point.file, Path::new("/my dir/a.slint"));
        let point = parse_mark("branch 1 false 7:36-7:43 /a.slint").unwrap();
        assert_eq!(point.branch, Some((1, false)));
        assert!(parse_mark("nonsense").is_err());
    }
}
