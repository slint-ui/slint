// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Coverage points counted by LLVM as marker functions.
//!
//! Code generated with `--coverage` calls an empty marker function
//! `slint_cov_<hash>::p<id>_` where a point is reached, and the compiler
//! writes a `.slintcov` map locating the points next to the generated code.
//! A profile of the instrumented code has the execution count of each
//! marker, which is the point's hit count.

use crate::{Point, Report};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A `.slintcov` map: the hash naming the marker module, and the points by
/// marker id.
pub struct Map {
    pub hash: String,
    pub points: Vec<Point>,
}

/// A map is the `slint-sc-coverage-map 1` header, the `hash`, the files by
/// index, and the points by id with their kind, file index, span and, for a
/// branch, its decision ordinal and outcome.
pub fn parse_map(text: &str) -> Result<Map, String> {
    let mut lines = text.lines();
    if lines.next() != Some("slint-sc-coverage-map 1") {
        return Err("not a slint-sc coverage map (version 1)".into());
    }
    let mut map = Map { hash: String::new(), points: Vec::new() };
    let mut files: Vec<PathBuf> = Vec::new();
    for line in lines {
        let fields: Vec<&str> = line.split(' ').collect();
        let parse = |s: &str| s.parse::<usize>().map_err(|e| format!("{line}: {e}"));
        match fields.as_slice() {
            ["hash", hash] => map.hash = hash.to_string(),
            ["file", index, path @ ..] => {
                if parse(index)? != files.len() {
                    return Err(format!("file index out of order: {line}"));
                }
                files.push(PathBuf::from(path.join(" ")));
            }
            ["point", id, kind, file, span, branch @ ..] => {
                if parse(id)? != map.points.len() {
                    return Err(format!("point id out of order: {line}"));
                }
                let (start, _end) = span.split_once('-').ok_or(format!("bad span: {line}"))?;
                let (line_no, column) = start.split_once(':').ok_or(format!("bad span: {line}"))?;
                let branch = match branch {
                    [] => None,
                    [decision, outcome] => {
                        let ordinal = parse(decision.strip_prefix('d').unwrap_or(decision))?;
                        Some((ordinal, *outcome == "true"))
                    }
                    _ => return Err(format!("bad point: {line}")),
                };
                let file = files.get(parse(file)?).ok_or(format!("unknown file: {line}"))?;
                map.points.push(Point {
                    kind: kind.to_string(),
                    file: file.clone(),
                    line: parse(line_no)?,
                    column: parse(column)?,
                    branch,
                });
            }
            _ => return Err(format!("unexpected line: {line}")),
        }
    }
    if map.hash.is_empty() {
        return Err("missing hash".into());
    }
    Ok(map)
}

/// The execution counts of the marker functions in a text profile, by the
/// hash of their map and the id of their point. A hash is present as soon as
/// one function of its module is, even when no point was reached.
pub type Counts = BTreeMap<String, BTreeMap<usize, u64>>;

/// Parse a profile in the text format of `llvm-profdata merge --text`. A
/// record is the function name, its hash, the number of counters, and the
/// counters, each on a line, separated from the next by a blank line; `#`
/// lines are comments and `:` lines the profile's flags. A marker function
/// has one counter, its entry count.
pub fn parse_profile(text: &str) -> Counts {
    let mut counts = Counts::new();
    let mut lines = text.lines().filter(|line| !line.starts_with('#') && !line.starts_with(':'));
    while let Some(name) = lines.next() {
        if name.is_empty() {
            continue;
        }
        let _hash = lines.next();
        let counters: usize = lines.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        let first = lines.next().and_then(|count| count.parse().ok()).unwrap_or(0);
        for _ in 1..counters {
            lines.next();
        }
        if let Some((hash, point)) = parse_marker(name) {
            let module = counts.entry(hash).or_default();
            if let Some(point) = point {
                *module.entry(point).or_default() += first;
            }
        }
    }
    counts
}

/// The map hash and point id of a function of a `slint_cov_<hash>` module, from
/// its mangled or demangled name; the id is `None` for the module's `branch`
/// helper. In a mangled name, the `p<id>_` identifier is prefixed with its
/// length; in a demangled one, with `::`.
fn parse_marker(name: &str) -> Option<(String, Option<usize>)> {
    let start = name.find("slint_cov_")? + "slint_cov_".len();
    let hash = name.get(start..start + 8)?;
    if !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let rest = name[start + 8..].trim_start_matches(|c: char| c.is_ascii_digit() || c == ':');
    let point = rest
        .strip_prefix('p')
        .and_then(|rest| rest.split_once('_'))
        .and_then(|(id, _)| id.parse().ok());
    Some((hash.to_owned(), point))
}

/// Add the points of the map to the report with their counts from the
/// profile, or return `false` when the map is not in the profile: its code
/// was never built with coverage, or it is left over from an earlier build.
pub fn add(map: &Map, counts: &Counts, report: &mut Report) -> bool {
    let Some(counts) = counts.get(&map.hash) else { return false };
    for (id, point) in map.points.iter().enumerate() {
        report.add(point, counts.get(&id).copied().unwrap_or(0));
    }
    true
}

/// Merge the profiles into the text format with `llvm-profdata`.
pub fn merge_profiles(profiles: &[PathBuf]) -> Result<String, String> {
    let llvm_profdata = find_llvm_profdata();
    let output = Command::new(&llvm_profdata)
        .args(["merge", "--text", "-o", "-"])
        .args(profiles)
        .output()
        .map_err(|e| format!("cannot run {}: {e}", llvm_profdata.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} failed:\n{}",
            llvm_profdata.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| e.to_string())
}

/// `LLVM_PROFDATA`, the Rust toolchain's llvm-tools component, or PATH.
fn find_llvm_profdata() -> PathBuf {
    if let Some(path) = std::env::var_os("LLVM_PROFDATA") {
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
            .join(format!("llvm-profdata{}", std::env::consts::EXE_SUFFIX));
        if path.exists() {
            return path;
        }
    }
    "llvm-profdata".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "slint-sc-coverage-map 1
hash eb0c1e08
file 0 /src/ternary.slint
point 0 element 0 7:36-7:43
point 1 binding 0 13:30-13:42
point 2 branch 0 13:30-13:42 d0 true
point 3 branch 0 13:30-13:42 d0 false
point 4 handler 0 20:5-21:6
";

    /// The `llvm-profdata merge --text` format: the mangled marker names of
    /// `mod slint_cov_eb0c1e08` of crate `test`, and an unrelated function.
    const PROFILE: &str = ":ir
_RNvNtCs4pr0Rk6C0G8_4test18slint_cov_eb0c1e083p0_
# Func Hash:
1
# Num Counters:
1
# Counter Values:
3

_RNvNtCs4pr0Rk6C0G8_4test18slint_cov_eb0c1e083p1_
1
1
4

_RNvNtCs4pr0Rk6C0G8_4test18slint_cov_eb0c1e083p2_
1
1
4

_RNvNtCs4pr0Rk6C0G8_4test18slint_cov_eb0c1e083p3_
1
1
0

_RNvNtCs4pr0Rk6C0G8_4test18slint_cov_eb0c1e086branch
1
2
5
5

_RNvCs4pr0Rk6C0G8_4test4main
1
3
1
0
0
";

    #[test]
    fn marker_names() {
        assert_eq!(
            parse_marker("_RNvNtCs4pr0Rk6C0G8_4test18slint_cov_eb0c1e084p12_"),
            Some(("eb0c1e08".into(), Some(12)))
        );
        assert_eq!(
            parse_marker("test::slint_cov_eb0c1e08::p12_"),
            Some(("eb0c1e08".into(), Some(12)))
        );
        assert_eq!(
            parse_marker("_RNvNtCs4pr0Rk6C0G8_4test18slint_cov_eb0c1e086branch"),
            Some(("eb0c1e08".into(), None))
        );
        assert_eq!(parse_marker("_RNvCs4pr0Rk6C0G8_4test4main"), None);
    }

    #[test]
    fn map_and_profile() {
        let counts = parse_profile(PROFILE);
        assert_eq!(counts["eb0c1e08"][&3], 0);
        let map = parse_map(MAP).unwrap();
        let mut report = Report::default();
        assert!(add(&map, &counts, &mut report));
        // The handler's marker is absent from the profile: never linked, never reached.
        assert_eq!(
            report.listing(Path::new("/src/ternary.slint")),
            [
                "+ 7:36 element",
                "+ 13:30 binding",
                "+ 13:30 branch 0 true",
                "- 13:30 branch 0 false",
                "- 20:5 handler"
            ]
        );
        let stale = Map { hash: "00000000".into(), points: Vec::new() };
        assert!(!add(&stale, &counts, &mut report));
        assert!(parse_map("slint-sc-coverage-map 2\n").is_err());
        assert!(
            parse_map("slint-sc-coverage-map 1\nfile 0 a\npoint 0 element 1 1:1-1:2\n").is_err()
        );
    }
}
