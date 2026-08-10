// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Test Coverage chapter of the safety manual, reporting the LLVM
//! source-based coverage of the slint-sc test suite from a
//! `cargo llvm-cov report --json` export.

use crate::Config;
use crate::traceability::REPO_URL;
use anyhow::Context;
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Name of the page this module writes into
/// [`Config::qualification_plan_dir`], the section it belongs to.
const PAGE_FILE: &str = "test-coverage.mdx";

/// Covered/total counts of one metric, from the `count`/`covered` pairs of
/// the export's `summary` objects.
struct Counts {
    count: u64,
    covered: u64,
}

impl Counts {
    /// Table cell like `95.2% (240/252)`, or `-` when nothing was
    /// instrumented. The percentage is computed from the counts rather than
    /// taken from the export's float fields.
    fn cell(&self) -> String {
        if self.count == 0 {
            "-".into()
        } else {
            let percent = self.covered as f64 / self.count as f64 * 100.0;
            format!("{percent:.1}% ({}/{})", self.covered, self.count)
        }
    }
}

struct Summary {
    lines: Counts,
    functions: Counts,
    regions: Counts,
}

/// How many of a file's functions are fully tested (every code region
/// executed), partially tested (executed, but some code regions weren't),
/// and untested (never executed).
#[derive(Default, Clone, Copy)]
struct FnStats {
    full: u64,
    partial: u64,
    untested: u64,
}

impl FnStats {
    fn add(&mut self, other: FnStats) {
        self.full += other.full;
        self.partial += other.partial;
        self.untested += other.untested;
    }

    fn total(&self) -> u64 {
        self.full + self.partial + self.untested
    }
}

struct FileCoverage {
    /// Repository-relative path with `/` separators.
    path: String,
    summary: Summary,
    fn_stats: FnStats,
    /// Start of each code region that never executed, in document order, for
    /// pointing at the gap rather than only counting it.
    uncovered_regions: Vec<(u64, u64)>,
}

/// The llvm-cov HTML report installed under the site's `public/` directory,
/// serving per-line execution counts.
struct DetailReport {
    /// Where the report was installed, for checking which pages exist.
    dir: PathBuf,
}

impl DetailReport {
    /// Path of the per-line page of `path`, relative to the report root.
    /// llvm-cov mirrors the (remapped, repository-relative) source path
    /// below `coverage/`.
    fn page(path: &str) -> String {
        format!("coverage/{path}.html")
    }

    /// Site-root-absolute URL of the per-line page of `path`, if the report
    /// has one.
    fn link(&self, path: &str) -> Option<String> {
        let page = Self::page(path);
        self.dir.join(&page).exists().then(|| format!("/coverage/{page}"))
    }
}

/// Writes the chapter and returns the gaps it shows: the files that aren't
/// completely covered. A build without a coverage export measures nothing, so
/// it reports no gaps.
pub fn generate(cfg: &Config) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut out = cfg.qualification_page(PAGE_FILE)?;

    writeln!(
        out,
        r#"---
title: Test Coverage
description: LLVM source-based code coverage of the slint-sc runtime.
slug: qualification-plan/test-coverage
---

The tests of the `slint-sc` runtime crate run under LLVM source-based coverage instrumentation with [`cargo llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov):
the unit tests, and the test driver that compiles the `.slint` test cases and runs them against the instrumented runtime.
This chapter reports the measured line, function, and region coverage per source file of the runtime, addressing [SR_TEST_COVERAGE](/requirements/test-coverage/).
A function counts as fully tested when every code region in it was executed, as partially tested when it was executed but some of its code regions weren't, and as untested when it was never executed."#
    )?;

    match &cfg.coverage_json {
        None => {
            write_placeholder(&mut out)?;
            Ok(Vec::new())
        }
        Some(json) => {
            let text = std::fs::read_to_string(json)
                .context(format!("error reading coverage export {json:?}"))?;
            let files =
                parse_export(&text).context(format!("error parsing coverage export {json:?}"))?;
            let detail = cfg
                .coverage_html
                .as_deref()
                .map(|src| install_html_report(cfg, src))
                .transpose()?;
            if let Some(d) = &detail
                && !files.iter().any(|f| d.link(&f.path).is_some())
            {
                // A single file without a page renders as `-`; no page for any
                // file means the layout assumption broke, so fail loudly.
                return Err(anyhow::anyhow!(
                    "no per-line page found for any covered file in {:?}",
                    d.dir
                )
                .into());
            }
            let sha = crate::traceability::git_head(&crate::root_dir());
            write_report(&mut out, &files, &sha, detail.as_ref())?;
            Ok(shortfalls(&files))
        }
    }
}

/// The files that aren't completely covered, one message per file naming the
/// metrics that fall short and where the code that never executed is. The
/// qualification plan admits no exceptions, so any shortfall is a gap.
fn shortfalls(files: &[FileCoverage]) -> Vec<String> {
    let mut gaps = Vec::new();
    for f in files {
        let metrics = [
            ("line", &f.summary.lines),
            ("function", &f.summary.functions),
            ("region", &f.summary.regions),
        ];
        let short: Vec<String> = metrics
            .iter()
            .filter(|(_, c)| c.covered < c.count)
            .map(|(name, c)| format!("{name} coverage {}", c.cell()))
            .collect();
        if short.is_empty() {
            continue;
        }
        let mut msg = format!("{}: {}", f.path, short.join(", "));
        if !f.uncovered_regions.is_empty() {
            let at: Vec<String> = f
                .uncovered_regions
                .iter()
                .map(|(line, col)| format!("{}:{line}:{col}", f.path))
                .collect();
            msg.push_str("; never executed at ");
            msg.push_str(&at.join(", "));
        }
        gaps.push(msg);
    }
    gaps
}

/// Body of the chapter in a build without a coverage export, e.g. the
/// regular docs build. The page must exist in every build because the
/// sidebar links it.
fn write_placeholder(out: &mut impl Write) -> std::io::Result<()> {
    writeln!(
        out,
        "\nThis build of the manual doesn't include coverage data.\n\
         Build it with `scripts/build_safety_manual_coverage.sh`; the published manual includes the measured coverage."
    )
}

/// Parse the export into per-file summaries. The reports are generated with
/// `--remap-path-prefix`, so workspace files are repository-relative; skip
/// absolute paths (dependencies outside the workspace) and generated code
/// under `target/`.
fn parse_export(text: &str) -> anyhow::Result<Vec<FileCoverage>> {
    let v: Value = serde_json::from_str(text)?;
    let data = v.get("data").and_then(|d| d.get(0)).context("missing `data[0]`")?;
    let mut files = Vec::new();
    for f in data.get("files").and_then(Value::as_array).context("missing `files`")? {
        let filename = f.get("filename").and_then(Value::as_str).context("missing `filename`")?;
        // Not is_absolute(): a rooted unix path isn't "absolute" on Windows,
        // but isn't repository-relative either.
        if Path::new(filename).has_root() {
            continue;
        }
        let path = filename.replace(std::path::MAIN_SEPARATOR, "/");
        if path.starts_with("target/") {
            continue;
        }
        let summary = f.get("summary").with_context(|| format!("{path}: missing `summary`"))?;
        files.push(FileCoverage {
            path,
            summary: parse_summary(summary)?,
            fn_stats: FnStats::default(),
            uncovered_regions: Vec::new(),
        });
    }
    anyhow::ensure!(!files.is_empty(), "no repository-relative files in the coverage export");
    files.sort_by(|a, b| a.path.cmp(&b.path));
    parse_fn_stats(data, &mut files)?;
    Ok(files)
}

/// A code region's source span: start line/column, end line/column.
type Span = (u64, u64, u64, u64);

/// Classify every function of the reported files as fully tested, partially
/// tested, or untested. The export lists one entry per instantiation;
/// entries with the same file and code-region spans are one source function,
/// with the execution counts of their shared regions summed -- like llvm-cov
/// groups instantiations in the file summaries.
fn parse_fn_stats(data: &Value, files: &mut [FileCoverage]) -> anyhow::Result<()> {
    let index: std::collections::HashMap<&str, usize> =
        files.iter().enumerate().map(|(i, f)| (f.path.as_str(), i)).collect();
    let mut merged: std::collections::HashMap<
        (usize, Vec<Span>),
        std::collections::HashMap<Span, u64>,
    > = std::collections::HashMap::new();
    for f in data.get("functions").and_then(Value::as_array).context("missing `functions`")? {
        let file = f
            .get("filenames")
            .and_then(|v| v.get(0))
            .and_then(Value::as_str)
            .context("missing `filenames[0]`")?;
        // Functions of dependencies, generated code, and the test sources
        // themselves belong to no reported file.
        let Some(&idx) = index.get(file) else { continue };
        let regions = code_regions(f)?;
        if regions.is_empty() {
            continue;
        }
        let mut key: Vec<Span> = regions.iter().map(|(span, _)| *span).collect();
        key.sort_unstable();
        let counts = merged.entry((idx, key)).or_default();
        for (span, count) in regions {
            *counts.entry(span).or_default() += count;
        }
    }
    for ((idx, _), counts) in merged {
        let file = &mut files[idx];
        if counts.values().all(|c| *c == 0) {
            file.fn_stats.untested += 1;
        } else if counts.values().all(|c| *c > 0) {
            file.fn_stats.full += 1;
        } else {
            file.fn_stats.partial += 1;
        }
        file.uncovered_regions.extend(
            counts.iter().filter(|(_, c)| **c == 0).map(|((line, col, ..), _)| (*line, *col)),
        );
    }
    for file in files.iter_mut() {
        file.uncovered_regions.sort_unstable();
        file.uncovered_regions.dedup();
    }
    Ok(())
}

/// The code regions of one function entry: source span and execution count.
/// Gap and skipped regions don't count towards being fully tested.
fn code_regions(function: &Value) -> anyhow::Result<Vec<(Span, u64)>> {
    /// Region kind of a regular code region in the llvm-cov export.
    const CODE: u64 = 0;
    let mut out = Vec::new();
    for r in function.get("regions").and_then(Value::as_array).context("missing `regions`")? {
        let n = |i: usize| r.get(i).and_then(Value::as_u64).context("malformed region");
        if n(7)? == CODE {
            out.push(((n(0)?, n(1)?, n(2)?, n(3)?), n(4)?));
        }
    }
    Ok(out)
}

fn parse_summary(summary: &Value) -> anyhow::Result<Summary> {
    let counts = |key: &str| {
        let metric = summary.get(key).with_context(|| format!("missing `{key}` summary"))?;
        let field = |name: &str| {
            metric
                .get(name)
                .and_then(Value::as_u64)
                .with_context(|| format!("missing `{key}.{name}`"))
        };
        anyhow::Ok(Counts { count: field("count")?, covered: field("covered")? })
    };
    Ok(Summary {
        lines: counts("lines")?,
        functions: counts("functions")?,
        regions: counts("regions")?,
    })
}

/// Copy the llvm-cov HTML report into the site's `public/` directory, where
/// Astro serves it verbatim under `/coverage/`. The destination is cleared
/// first; it's gitignored.
fn install_html_report(cfg: &Config, src: &Path) -> anyhow::Result<DetailReport> {
    anyhow::ensure!(
        src.join("index.html").exists(),
        "{src:?} is not an llvm-cov HTML report: no index.html"
    );
    // The construction pins the deletion below to our own earlier copy.
    let dest = cfg.astro_dir.join("public").join("coverage");
    if dest.exists() {
        std::fs::remove_dir_all(&dest).with_context(|| format!("error clearing {dest:?}"))?;
    }
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let target = dest.join(entry.path().strip_prefix(src)?);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)
                .with_context(|| format!("error creating {target:?}"))?;
        } else {
            std::fs::copy(entry.path(), &target)
                .with_context(|| format!("error copying {:?} to {target:?}", entry.path()))?;
        }
    }
    Ok(DetailReport { dir: dest })
}

/// The section a file belongs to: the crate root, i.e. its first two path
/// segments (`api/slint-sc`, `internal/compiler`, ...).
fn group(path: &str) -> &str {
    path.match_indices('/').nth(1).map_or(path, |(i, _)| &path[..i])
}

/// Sum of one metric across the reported files, so the headline and the
/// per-crate sum rows match the tables after filtering.
fn sum<'a>(
    files: impl IntoIterator<Item = &'a FileCoverage>,
    metric: impl Fn(&Summary) -> &Counts,
) -> Counts {
    files.into_iter().fold(Counts { count: 0, covered: 0 }, |acc, f| {
        let c = metric(&f.summary);
        Counts { count: acc.count + c.count, covered: acc.covered + c.covered }
    })
}

/// Sum of the function test status across the reported files.
fn sum_fn_stats<'a>(files: impl IntoIterator<Item = &'a FileCoverage>) -> FnStats {
    files.into_iter().fold(FnStats::default(), |mut acc, f| {
        acc.add(f.fn_stats);
        acc
    })
}

/// `120 fully tested, 30 partially tested, 12 untested`.
fn fn_stats_sentence(stats: &FnStats) -> String {
    format!(
        "{} fully tested, {} partially tested, {} untested",
        stats.full, stats.partial, stats.untested
    )
}

/// The headline totals and one table per crate, a row per file and a sum
/// row, with per-line detail links when the HTML report ships with the
/// build. Branch coverage is omitted: stable Rust emits no branch data.
fn write_report(
    out: &mut impl Write,
    files: &[FileCoverage],
    sha: &str,
    detail: Option<&DetailReport>,
) -> std::io::Result<()> {
    writeln!(
        out,
        "\n{commit}\n\n\
         **Line coverage: {lines}. Function coverage: {functions}. Region coverage: {regions}.**",
        commit = crate::traceability::commit_line(sha),
        lines = sum(files, |s| &s.lines).cell(),
        functions = sum(files, |s| &s.functions).cell(),
        regions = sum(files, |s| &s.regions).cell(),
    )?;

    let stats = sum_fn_stats(files);
    let share = |n| Counts { count: stats.total(), covered: n }.cell();
    writeln!(
        out,
        "\n| Functions | Share |\n| --- | --- |\n\
         | Fully tested | {} |\n| Partially tested | {} |\n| Untested | {} |",
        share(stats.full),
        share(stats.partial),
        share(stats.untested),
    )?;

    if detail.is_some() {
        writeln!(
            out,
            "\nPer-line execution counts are in the [detailed coverage report](/coverage/index.html), linked per file in the tables below."
        )?;
    }

    // The trailing Per-line column exists only when the HTML report ships
    // with the build; `extra` renders one cell of it.
    let extra = |cell: String| detail.map_or(String::new(), |_| format!(" {cell} |"));
    // `parse_export` sorts by path, so the files of a crate are contiguous.
    for chunk in files.chunk_by(|a, b| group(&a.path) == group(&b.path)) {
        let section = group(&chunk[0].path);
        writeln!(out, "\n## {section}\n")?;
        writeln!(out, "| File | Lines | Functions | Regions |{}", extra("Per-line".into()))?;
        writeln!(out, "| --- | --- | --- | --- |{}", extra("---".into()))?;
        for f in chunk {
            // Path shown relative to the section heading; `section` is either
            // a strict prefix of the path or the whole path.
            let short = f.path.get(section.len() + 1..).unwrap_or(&f.path);
            let per_line = detail
                .and_then(|d| d.link(&f.path))
                .map_or("-".into(), |url| format!("[view]({url})"));
            writeln!(
                out,
                "| [`{short}`]({REPO_URL}/blob/{sha}/{}) | {} | {} | {} |{}",
                f.path,
                f.summary.lines.cell(),
                f.summary.functions.cell(),
                f.summary.regions.cell(),
                extra(per_line),
            )?;
        }
        writeln!(
            out,
            "| **Sum** | **{}** | **{}** | **{}** |{}",
            sum(chunk, |s| &s.lines).cell(),
            sum(chunk, |s| &s.functions).cell(),
            sum(chunk, |s| &s.regions).cell(),
            extra(String::new()),
        )?;
        writeln!(out, "\nFunctions: {}.", fn_stats_sentence(&sum_fn_stats(chunk)))?;
    }
    Ok(())
}

#[test]
fn test_shortfalls() {
    let file = |path: &str, covered: u64, uncovered_regions: &[(u64, u64)]| FileCoverage {
        path: path.into(),
        summary: Summary {
            lines: Counts { count: 10, covered },
            functions: Counts { count: 2, covered: 2 },
            regions: Counts { count: 10, covered },
        },
        fn_stats: FnStats::default(),
        uncovered_regions: uncovered_regions.to_vec(),
    };

    // A completely covered file is no gap.
    assert!(shortfalls(&[file("api/slint-sc/lib.rs", 10, &[])]).is_empty());

    // A shortfall names the metrics that fall short and where the code that
    // never executed is; the complete metric isn't mentioned.
    let gaps = shortfalls(&[file("api/slint-sc/lib.rs", 8, &[(30, 1)])]);
    assert_eq!(gaps.len(), 1, "{gaps:?}");
    assert_eq!(
        gaps[0],
        "api/slint-sc/lib.rs: line coverage 80.0% (8/10), region coverage 80.0% (8/10); \
         never executed at api/slint-sc/lib.rs:30:1"
    );
    assert!(!gaps[0].contains("function coverage"), "{}", gaps[0]);

    // Every incomplete file is reported, not just the first.
    assert_eq!(shortfalls(&[file("a.rs", 8, &[]), file("b.rs", 9, &[])]).len(), 2);
}

#[test]
fn test_cell() {
    assert_eq!(Counts { count: 0, covered: 0 }.cell(), "-");
    assert_eq!(Counts { count: 252, covered: 240 }.cell(), "95.2% (240/252)");
    assert_eq!(Counts { count: 3, covered: 3 }.cell(), "100.0% (3/3)");
}

#[test]
fn test_group() {
    assert_eq!(group("api/slint-sc/lib.rs"), "api/slint-sc");
    assert_eq!(group("internal/compiler/generator/slint_sc.rs"), "internal/compiler");
    assert_eq!(group("tools/compiler/main.rs"), "tools/compiler");
    assert_eq!(group("shallow.rs"), "shallow.rs");
}

#[test]
fn test_detail_page() {
    // llvm-cov mirrors the remapped source path below `coverage/`.
    assert_eq!(DetailReport::page("api/slint-sc/lib.rs"), "coverage/api/slint-sc/lib.rs.html");
    // No page on disk, no link.
    let detail = DetailReport { dir: PathBuf::from("/nonexistent") };
    assert_eq!(detail.link("api/slint-sc/lib.rs"), None);
}

#[test]
fn test_parse_export() {
    let summary = |lines: (u64, u64)| {
        format!(
            r#"{{"lines": {{"count": {}, "covered": {}, "percent": 0}},
                 "functions": {{"count": 4, "covered": 2, "percent": 0}},
                 "regions": {{"count": 10, "covered": 5, "percent": 0}}}}"#,
            lines.0, lines.1
        )
    };
    // Functions, all in lib.rs unless noted: `a` has two instantiations
    // whose merged regions are all executed (fully tested; the kind-3 gap
    // region doesn't count), `b` has an unexecuted region (partially
    // tested), `c` never ran (untested), `d` is in a skipped file.
    let functions = r#"[
        {"name": "a", "count": 2, "filenames": ["api/slint-sc/lib.rs"],
         "regions": [[1,1,5,2,2,0,0,0], [2,1,3,2,0,0,0,0]]},
        {"name": "a", "count": 1, "filenames": ["api/slint-sc/lib.rs"],
         "regions": [[1,1,5,2,1,0,0,0], [2,1,3,2,4,0,0,0], [4,1,4,9,0,0,0,3]]},
        {"name": "b", "count": 3, "filenames": ["api/slint-sc/lib.rs"],
         "regions": [[10,1,20,2,3,0,0,0], [12,1,13,2,0,0,0,0]]},
        {"name": "c", "count": 0, "filenames": ["api/slint-sc/lib.rs"],
         "regions": [[30,1,40,2,0,0,0,0]]},
        {"name": "d", "count": 5, "filenames": ["/root/.cargo/registry/dep.rs"],
         "regions": [[1,1,2,2,5,0,0,0]]}
    ]"#;
    let text = format!(
        r#"{{"data": [{{"files": [
            {{"filename": "api/slint-sc/lib.rs", "summary": {}}},
            {{"filename": "target/llvm-cov-target/generated.rs", "summary": {}}},
            {{"filename": "/root/.cargo/registry/dep.rs", "summary": {}}}
        ], "functions": {functions}, "totals": {{}}}}], "type": "llvm.coverage.json.export", "version": "2.0.1"}}"#,
        summary((10, 8)),
        summary((1, 1)),
        summary((2, 2)),
    );
    // Only the repository-relative file outside `target/` is kept.
    let files = parse_export(&text).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "api/slint-sc/lib.rs");
    assert_eq!(files[0].summary.lines.cell(), "80.0% (8/10)");
    assert_eq!(files[0].summary.functions.cell(), "50.0% (2/4)");
    assert_eq!(sum(&files, |s| &s.regions).cell(), "50.0% (5/10)");
    let stats = files[0].fn_stats;
    assert_eq!((stats.full, stats.partial, stats.untested), (1, 1, 1));
    assert_eq!(fn_stats_sentence(&stats), "1 fully tested, 1 partially tested, 1 untested");

    // `b`'s unexecuted region and `c`, which never ran, are located for the
    // gap message; `a`'s regions all executed once merged.
    assert_eq!(files[0].uncovered_regions, [(12, 1), (30, 1)]);

    // An export without any repository file is an error, not an empty page.
    let empty = r#"{"data": [{"files": [], "totals": {}}]}"#;
    assert!(parse_export(empty).is_err());

    // So is a malformed summary.
    let bad = r#"{"data": [{"files": [{"filename": "a/b/c.rs", "summary": {}}]}]}"#;
    assert!(parse_export(bad).is_err());
}
