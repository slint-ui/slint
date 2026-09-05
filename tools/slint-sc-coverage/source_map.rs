// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Coverage points mapped to ranges of the generated code, counted by LLVM.
//!
//! The Slint SC compiler writes, next to the generated code, a map declaring
//! its coverage points (`point <id> <kind>[ <decision> <outcome>] <span>
//! <path>`) and the ranges of the code that are one (`range <start>-<end>
//! <id>`). LLVM measures the generated code like any other; the execution
//! count of the code at a range is the point's hit count, and a point
//! without a range was never reached.

use crate::{Point, Report};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// A position in a file: line and column, both 1-based, the column in
/// characters like LLVM's.
pub type Position = (usize, usize);

/// The code regions of an `llvm-cov export`, by file.
#[derive(Default)]
pub struct Regions {
    pub files: HashMap<PathBuf, FileRegions>,
}

/// The code regions of one file, by function.
#[derive(Default)]
pub struct FileRegions {
    functions: Vec<Function>,
}

/// The regions of one function in one file, sorted by start, and their extent.
struct Function {
    start: Position,
    end: Position,
    regions: Vec<(Position, Position, u64)>,
}

/// The parts of an `llvm-cov export` in JSON the mapping reads: for each
/// function, its `filenames`, and its `regions` as `[start line, start
/// column, end line, end column, count, file index, expanded file index,
/// kind]`, of which only the code regions (kind 0) count.
#[derive(Deserialize)]
struct Export {
    data: Vec<ExportData>,
}

#[derive(Deserialize)]
struct ExportData {
    functions: Vec<ExportFunction>,
}

#[derive(Deserialize)]
struct ExportFunction {
    filenames: Vec<String>,
    regions: Vec<[u64; 8]>,
}

impl Regions {
    /// Read an export; relative file names are relative to `base_dir`.
    pub fn parse(&mut self, export: &str, base_dir: &Path) -> Result<(), String> {
        let export: Export =
            serde_json::from_str(export).map_err(|e| format!("not a coverage export: {e}"))?;
        let mut canonical: HashMap<&str, PathBuf> = HashMap::new();
        for function in export.data.iter().flat_map(|d| &d.functions) {
            let mut by_file: HashMap<usize, Vec<(Position, Position, u64)>> = HashMap::new();
            for region in &function.regions {
                let [start_line, start_column, end_line, end_column, count, file, _, kind] =
                    *region;
                if kind == 0 {
                    let start = (start_line as usize, start_column as usize);
                    let end = (end_line as usize, end_column as usize);
                    by_file.entry(file as usize).or_default().push((start, end, count));
                }
            }
            for (file, mut regions) in by_file {
                let Some(name) = function.filenames.get(file) else { continue };
                let path = canonical.entry(name).or_insert_with(|| {
                    let path = base_dir.join(name);
                    path.canonicalize().unwrap_or(path)
                });
                regions.sort();
                let function = Function {
                    start: regions[0].0,
                    end: regions.iter().map(|r| r.1).max().unwrap_or(regions[0].1),
                    regions,
                };
                self.files.entry(path.clone()).or_default().functions.push(function);
            }
        }
        Ok(())
    }
}

impl FileRegions {
    /// The execution count of the code of a point's range, in the innermost
    /// function around its start: the innermost region holding the start,
    /// which rustc makes of an empty block, else the first region starting
    /// within the range, its code. Code LLVM never instantiated has no
    /// region, and counts as never reached. An export of several binaries
    /// lists a function once per binary, whose counts add up.
    pub fn count_in(&self, (start, end): (Position, Position)) -> u64 {
        let around = self.functions.iter().filter(|f| f.start <= start && start < f.end);
        let Some(innermost) = around.clone().map(|f| f.start).max() else { return 0 };
        around
            .filter(|f| f.start == innermost)
            .map(|f| {
                let holding = f
                    .regions
                    .iter()
                    .filter(|(from, to, _)| *from <= start && start < *to)
                    .max_by_key(|(from, _, _)| *from);
                let within = f.regions.iter().find(|(from, _, _)| *from >= start && *from < end);
                holding.or(within).map_or(0, |(_, _, count)| *count)
            })
            .sum()
    }
}

/// A map: the points by id, and the ranges of the code, each with the id of
/// its point.
pub struct Map {
    pub points: Vec<Point>,
    pub ranges: Vec<((Position, Position), usize)>,
}

pub fn parse_map(text: &str) -> Result<Map, String> {
    let mut lines = text.lines();
    if lines.next() != Some("slint-sc-source-map 1") {
        return Err("not a slint-sc source map (version 1)".into());
    }
    let mut map = Map { points: Vec::new(), ranges: Vec::new() };
    for line in lines {
        let parsed = match line.split_once(' ') {
            Some(("point", rest)) => rest.split_once(' ').and_then(|(id, record)| {
                (id.parse::<usize>().ok()? == map.points.len()).then_some(())?;
                map.points.push(parse_record(record)?);
                Some(())
            }),
            Some(("range", rest)) => rest.split_once(' ').and_then(|(positions, id)| {
                let position = |text: &str| -> Option<Position> {
                    let (line_no, column) = text.split_once(':')?;
                    Some((line_no.parse().ok()?, column.parse().ok()?))
                };
                let (start, end) = positions.split_once('-')?;
                let id = id.parse().ok()?;
                (id < map.points.len()).then_some(())?;
                map.ranges.push(((position(start)?, position(end)?), id));
                Some(())
            }),
            _ => None,
        };
        parsed.ok_or_else(|| format!("bad map line `{line}`"))?;
    }
    Ok(map)
}

/// A record is `<kind>[ <decision> <outcome>] <span> <path>`, the span as
/// `line:column-line:column`.
fn parse_record(record: &str) -> Option<Point> {
    let (kind, rest) = record.split_once(' ')?;
    let (branch, rest) = match kind {
        "branch" => {
            let (decision, rest) = rest.split_once(' ')?;
            let (outcome, rest) = rest.split_once(' ')?;
            (Some((decision.parse().ok()?, outcome == "true")), rest)
        }
        _ => (None, rest),
    };
    let (span, path) = rest.split_once(' ')?;
    let (start, _end) = span.split_once('-')?;
    let (line, column) = start.split_once(':')?;
    Some(Point {
        kind: kind.to_string(),
        file: PathBuf::from(path),
        line: line.parse().ok()?,
        column: column.parse().ok()?,
        branch,
    })
}

/// Add the points of a generated file's map to the report, with the counts
/// of the code at their ranges. Returns how many points the map declares.
pub fn add(map: &str, regions: &FileRegions, report: &mut Report) -> Result<usize, String> {
    let map = parse_map(map)?;
    let mut counts = vec![0; map.points.len()];
    for (range, id) in &map.ranges {
        counts[*id] += regions.count_in(*range);
    }
    for (point, count) in map.points.iter().zip(counts) {
        report.add(point, count);
    }
    Ok(map.points.len())
}

/// Export the coverage of the binaries from the profiles with `llvm-cov`,
/// as JSON.
pub fn export_coverage(objects: &[PathBuf], profiles: &[PathBuf]) -> Result<String, String> {
    // One merged profile per call: calls run concurrently in a test driver.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let profdata = std::env::temp_dir()
        .join(format!("slint-sc-coverage-{}-{call}.profdata", std::process::id()));
    let merge = Command::new(llvm_tool("llvm-profdata"))
        .arg("merge")
        .arg("-o")
        .arg(&profdata)
        .args(profiles)
        .output()
        .map_err(|e| format!("cannot run llvm-profdata: {e}"))?;
    if !merge.status.success() {
        return Err(format!("llvm-profdata failed:\n{}", String::from_utf8_lossy(&merge.stderr)));
    }
    let mut export = Command::new(llvm_tool("llvm-cov"));
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
fn llvm_tool(name: &str) -> PathBuf {
    if let Some(path) = std::env::var_os(name.to_uppercase().replace('-', "_")) {
        return path.into();
    }
    // The toolchain's `bin` beside its target libraries, found once.
    static BIN: OnceLock<Option<PathBuf>> = OnceLock::new();
    let bin = BIN.get_or_init(|| {
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let output = Command::new(rustc).args(["--print", "target-libdir"]).output().ok()?;
        let libdir = String::from_utf8(output.stdout).ok()?;
        Some(Path::new(libdir.trim()).parent()?.join("bin"))
    });
    match bin {
        Some(bin) => {
            let path = bin.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            if path.exists() { path } else { name.into() }
        }
        None => name.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generated code as the compiler prints it: one line, tokens spaced,
    /// with the map of its points.
    const GENERATED: &str = r#"pub fn get_pick (& self) -> i32 { if self . property_a { 7i32 } else { 3i32 } }
"#;

    fn map() -> String {
        let range = |from: &str, to: &str, id: usize| {
            let start = GENERATED.find(from).unwrap() + 1;
            let end = GENERATED.find(to).unwrap() + to.len() + 1;
            format!("range 1:{start}-1:{end} {id}\n")
        };
        format!(
            "slint-sc-source-map 1\n\
             point 0 binding 13:30-13:42 /src/ternary.slint\n\
             point 1 branch 0 true 13:30-13:42 /src/ternary.slint\n\
             point 2 branch 0 false 13:30-13:42 /src/ternary.slint\n\
             point 3 handler 20:5-21:6 /src/ternary.slint\n{}{}{}",
            range("if", "} }", 0),
            range("{ 7i32", "7i32 }", 1),
            range("{ 3i32", "3i32 }", 2),
        )
    }

    /// The export of that code as rustc instruments it: regions for the
    /// signature, the condition and each arm's value; once per binary of the
    /// export, one that ran the function, one not.
    fn export() -> String {
        let region = |from: &str, count: u64| {
            let start = GENERATED.find(from).unwrap() + 1;
            format!("[1,{start},1,{},{count},0,0,0]", start + from.len())
        };
        let function = |counts: [u64; 4]| {
            format!(
                r#"{{"name":"get_pick","filenames":["gen/out.rs"],"regions":[{},{},{},{}]}}"#,
                region("pub fn get_pick (& self) -> i32 {", counts[0]),
                region("self . property_a", counts[1]),
                region("7i32", counts[2]),
                region("3i32", counts[3]),
            )
        };
        format!(
            r#"{{"data":[{{"functions":[{},{}],"files":[{{"segments":[[1,2,3,true,true,false]]}}]}}]}}"#,
            function([4, 4, 4, 0]),
            function([0; 4])
        )
    }

    #[test]
    fn map_to_report() {
        let mut regions = Regions::default();
        regions.parse(&export(), Path::new("/base")).unwrap();
        let file = &regions.files[Path::new("/base/gen/out.rs")];
        let mut report = Report::default();
        // The handler has no range: never reached.
        assert_eq!(add(&map(), file, &mut report).unwrap(), 4);
        assert_eq!(
            report.listing(Path::new("/src/ternary.slint")),
            [
                "+ 13:30 binding",
                "+ 13:30 branch 0 true",
                "- 13:30 branch 0 false",
                "- 20:5 handler"
            ]
        );
        // Outside every function, and a range whose code LLVM never instantiated.
        assert_eq!(file.count_in(((2, 1), (2, 5))), 0);
        let after = GENERATED.find("} }").unwrap() + 2;
        assert_eq!(file.count_in(((1, after), (1, after + 1))), 0);
        assert!(parse_map("slint-sc-source-map 2\n").is_err());
        assert!(parse_map("slint-sc-source-map 1\nrange nonsense\n").is_err());
        assert!(parse_map("slint-sc-source-map 1\nrange 1:1-1:2 0\n").is_err());
        assert!(parse_map("slint-sc-source-map 1\npoint 1 element 1:1-1:2 a\n").is_err());
    }

    #[test]
    fn empty_block() {
        // A block holding nothing is a region of its own, holding the range's start.
        let code = "fn h (x : bool) { if x { } else { } }";
        let (then_at, else_at) = (code.find("{ }").unwrap() + 1, code.rfind("{ }").unwrap() + 1);
        let map = format!(
            "slint-sc-source-map 1\npoint 0 branch 0 true 5:1-5:9 /src/a.slint\npoint 1 branch 0 false 5:1-5:9 /src/a.slint\nrange 1:{then_at}-1:{} 0\nrange 1:{else_at}-1:{} 1\n",
            then_at + 3,
            else_at + 3,
        );
        let region = |start: usize, len: usize, count: u64| {
            format!("[1,{start},1,{},{count},0,0,0]", start + len)
        };
        let export = format!(
            r#"{{"data":[{{"functions":[{{"name":"h","filenames":["gen/h.rs"],"regions":[{},{},{}]}}]}}]}}"#,
            region(1, code.find('{').unwrap() + 1, 2),
            region(then_at, 3, 2),
            region(else_at, 3, 0),
        );
        let mut regions = Regions::default();
        regions.parse(&export, Path::new("/base")).unwrap();
        let mut report = Report::default();
        let file = &regions.files[Path::new("/base/gen/h.rs")];
        assert_eq!(add(&map, file, &mut report).unwrap(), 2);
        assert_eq!(
            report.listing(Path::new("/src/a.slint")),
            ["+ 5:1 branch 0 true", "- 5:1 branch 0 false"]
        );
    }

    #[test]
    fn record_with_spaces_in_path() {
        let point = parse_record("element 7:36-7:43 /my dir/a.slint").unwrap();
        assert_eq!((point.kind.as_str(), point.line, point.column), ("element", 7, 36));
        assert_eq!(point.file, Path::new("/my dir/a.slint"));
        let point = parse_record("branch 1 false 7:36-7:43 /a.slint").unwrap();
        assert_eq!(point.branch, Some((1, false)));
        assert!(parse_record("nonsense").is_none());
    }
}
