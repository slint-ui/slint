// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The profile the `SLINT_SC_COVERAGE` static of code generated with
//! `--coverage` prints: the `slint-sc-coverage 1` header, the files, the
//! points with their kind, file index, span and, for a branch, its decision
//! ordinal and outcome, and the count of each point reached, by its index.
//! A `process` record names the process that wrote it.
//!
//! The counters are one table per process, so the tests of one binary each
//! write a snapshot of the same table: within a process the counts are the
//! largest of the snapshots, and across processes they add up.

use crate::{Point, Report};
use std::collections::BTreeMap;
use std::fmt::Display;
use std::path::{Path, PathBuf};

/// A profile: the process that wrote it, if it says, and its points with
/// their counts, in the order of the table.
pub struct Profile {
    pub process: Option<String>,
    pub points: Vec<(Point, u64)>,
}

pub fn parse(text: &str) -> Result<Profile, String> {
    let mut lines = text.lines();
    if lines.next() != Some("slint-sc-coverage 1") {
        return Err("not a slint-sc coverage profile (version 1)".into());
    }
    let mut files: Vec<PathBuf> = Vec::new();
    let mut profile = Profile { process: None, points: Vec::new() };
    for line in lines {
        let fields: Vec<&str> = line.split(' ').collect();
        let parse = |s: &str| s.parse::<usize>().map_err(|e| format!("{line}: {e}"));
        match fields.as_slice() {
            ["file", path @ ..] => files.push(PathBuf::from(path.join(" "))),
            ["point", kind, file, span, branch @ ..] => {
                let (start, _end) = span.split_once('-').ok_or(format!("bad span: {line}"))?;
                let (line_no, column) = start.split_once(':').ok_or(format!("bad span: {line}"))?;
                let branch = match branch {
                    [] => None,
                    [decision, outcome] => Some((parse(decision)?, *outcome == "true")),
                    _ => return Err(format!("bad point: {line}")),
                };
                let file = files.get(parse(file)?).ok_or(format!("unknown file: {line}"))?;
                let point = Point {
                    kind: kind.to_string(),
                    file: file.clone(),
                    line: parse(line_no)?,
                    column: parse(column)?,
                    branch,
                };
                profile.points.push((point, 0));
            }
            ["count", id, count] => {
                let point =
                    profile.points.get_mut(parse(id)?).ok_or(format!("unknown point: {line}"))?;
                point.1 = count.parse().map_err(|e| format!("{line}: {e}"))?;
            }
            ["process", id] => profile.process = Some(id.to_string()),
            _ => return Err(format!("unexpected line: {line}")),
        }
    }
    Ok(profile)
}

/// Add the profiles to the report: the snapshots of one process merged by
/// the largest count of each point, the processes added up. A profile that
/// names no process is a process of its own.
pub fn add_all(profiles: &[Profile], report: &mut Report) -> Result<(), String> {
    let mut by_process: BTreeMap<Option<&str>, Vec<&Profile>> = BTreeMap::new();
    for profile in profiles {
        match &profile.process {
            Some(id) => by_process.entry(Some(id)).or_default().push(profile),
            None => {
                for (point, count) in &profile.points {
                    report.add(point, *count);
                }
            }
        }
    }
    for (process, snapshots) in by_process {
        let first = snapshots[0];
        for snapshot in &snapshots[1..] {
            if snapshot.points.len() != first.points.len() {
                return Err(format!(
                    "the profiles of process {} are of different tables",
                    process.unwrap_or_default()
                ));
            }
        }
        for (index, (point, _)) in first.points.iter().enumerate() {
            let count = snapshots.iter().map(|s| s.points[index].1).max().unwrap_or(0);
            report.add(point, count);
        }
    }
    Ok(())
}

/// Writes the profile of a `SLINT_SC_COVERAGE` static when dropped, so that
/// a test writes it however it ends. The file is named after the process and
/// the thread, which `cargo test` names after the test.
pub struct Dump<'a> {
    counters: &'a dyn Display,
    path: PathBuf,
}

impl<'a> Dump<'a> {
    /// The profile goes into `dir`.
    pub fn into_dir(counters: &'a dyn Display, dir: &Path) -> Self {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("main").replace(['/', '\\', ':'], "-");
        let path = dir.join(format!("{}-{name}.slintcov", std::process::id()));
        Self { counters, path }
    }

    /// The profile goes into the directory `SLINT_SC_COVERAGE_DIR` names,
    /// or nowhere when it is unset.
    pub fn from_env(counters: &'a dyn Display) -> Option<Self> {
        let dir = std::env::var_os("SLINT_SC_COVERAGE_DIR")?;
        Some(Self::into_dir(counters, Path::new(&dir)))
    }
}

impl Drop for Dump<'_> {
    fn drop(&mut self) {
        let profile = format!("{}process {}\n", self.counters, std::process::id());
        if let Err(e) = std::fs::write(&self.path, profile) {
            eprintln!("cannot write the coverage profile {}: {e}", self.path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = "slint-sc-coverage 1
file /src/ternary.slint
point element 0 7:36-7:43
point binding 0 13:30-13:42
point branch 0 13:30-13:42 0 true
point branch 0 13:30-13:42 0 false
point handler 0 20:5-21:6
count 0 3
count 1 4
count 2 4
";

    #[test]
    fn profile_to_report() {
        let mut report = Report::default();
        let profile = parse(PROFILE).unwrap();
        assert_eq!(profile.process, None);
        // A second run of the same code adds up.
        add_all(&[profile, parse(PROFILE).unwrap()], &mut report).unwrap();
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
        assert!(report.lcov(Path::new("/src")).contains("DA:13,8\n"));
    }

    #[test]
    fn snapshots_of_a_process() {
        // Two tests of one process: the later snapshot holds the earlier
        // one's hits, and its own; a third process adds up.
        let early = parse(&format!("{PROFILE}process 7\n")).unwrap();
        let late = parse(&format!("{PROFILE}count 3 1\ncount 4 2\nprocess 7\n")).unwrap();
        let other = parse(&format!("{PROFILE}process 9\n")).unwrap();
        let mut report = Report::default();
        add_all(&[early, late, other], &mut report).unwrap();
        let lcov = report.lcov(Path::new("/src"));
        assert!(lcov.contains("DA:7,6\n"), "{lcov}");
        assert!(lcov.contains("DA:20,2\n"), "{lcov}");
        assert!(lcov.contains("BRDA:13,0,1,1\n"), "{lcov}");
    }

    #[test]
    fn dump_on_drop() {
        let dir =
            std::env::temp_dir().join(format!("slint-sc-coverage-dump-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        {
            let _dump = Dump::into_dir(&PROFILE, &dir);
        }
        let written = std::fs::read_dir(&dir).unwrap().next().unwrap().unwrap().path();
        let text = std::fs::read_to_string(&written).unwrap();
        assert_eq!(parse(&text).unwrap().process, Some(std::process::id().to_string()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn malformed_profiles() {
        assert!(parse("slint-sc-coverage 2\n").is_err());
        assert!(parse("slint-sc-coverage 1\nfile a\npoint element 1 1:1-1:2\n").is_err());
        assert!(parse("slint-sc-coverage 1\nfile a\ncount 0 1\n").is_err());
    }
}
