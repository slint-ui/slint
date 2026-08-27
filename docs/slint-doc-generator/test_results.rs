// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Test Results chapter of the safety manual: the outcomes of the slint-sc
//! compiler and runtime test suites, from the CTRF-style reports and libtest
//! logs collected by scripts/slint_sc_test_suite.sh.

use crate::Config;
use crate::traceability::REPO_URL;
use anyhow::Context;
use serde_json::Value;
use std::io::Write;
use std::path::Path;

/// Name of the page this module writes into
/// [`Config::qualification_report_dir`], the section it belongs to.
const PAGE_FILE: &str = "test-results.mdx";

/// One row of the suite summary table.
struct Suite {
    name: String,
    tests: u64,
    passed: u64,
    failed: u64,
}

impl Suite {
    /// The summary row over per-case results.
    fn from_cases(name: &str, cases: &[Case]) -> Self {
        let passed = cases.iter().filter(|c| c.passed).count() as u64;
        Suite {
            name: name.into(),
            tests: cases.len() as u64,
            passed,
            failed: cases.len() as u64 - passed,
        }
    }
}

/// A per-case entry of a CTRF report.
struct Case {
    name: String,
    /// Repository-relative source of the case, for linking, from the
    /// report's `filePath`.
    file_path: String,
    passed: bool,
}

/// A CTRF-style report, as written by a test harness.
struct CtrfReport {
    /// `results.tool.name`: labels the suite, and names the test binary
    /// whose libtest summary row the report supersedes.
    tool: String,
    cases: Vec<Case>,
}

/// Whether a libtest suite name and a CTRF tool name refer to the same test
/// binary; libtest reports crate names with underscores.
fn same_suite(a: &str, b: &str) -> bool {
    a.replace('-', "_") == b.replace('-', "_")
}

pub fn generate(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut out = cfg.qualification_page(PAGE_FILE)?;

    writeln!(
        out,
        r#"---
title: Test Results
description: Results of the slint-sc compiler and runtime test suites.
slug: qualification-report/test-results
---

This chapter reports the outcome of running the slint-sc test suites:
the unit and syntax tests of the compiler with the `slint-sc` feature, and the unit tests and `.slint` test driver cases of the `slint-sc` runtime crate.
The syntax tests are reported per test file rather than as the single test that drives them."#
    )?;

    match &cfg.test_results {
        None => write_placeholder(&mut out)?,
        Some(results_dir) => write_results(&mut out, results_dir)?,
    }
    Ok(())
}

/// Body of the chapter in a build without collected results, e.g. the
/// regular docs build. The page must exist in every build because the
/// sidebar links it.
fn write_placeholder(out: &mut impl Write) -> std::io::Result<()> {
    writeln!(
        out,
        "\nThis build of the manual doesn't include test results.\n\
         Build it with `scripts/build_safety_manual_coverage.sh`; the published manual includes the results of the qualification test runs."
    )
}

fn write_results(out: &mut impl Write, dir: &Path) -> anyhow::Result<()> {
    // The toolchain that built and ran the suites, recorded by
    // scripts/slint_sc_test_suite.sh. Reported here so the manual states the
    // toolchain per evidence run instead of hand-maintaining a version in
    // prose, where it rots.
    let toolchain = std::fs::read_to_string(dir.join("toolchain.txt"))
        .with_context(|| format!("error reading {dir:?}/toolchain.txt; re-run scripts/slint_sc_test_suite.sh to record the toolchain"))?;
    writeln!(
        out,
        "\n## Toolchain\n\nThe suites were built and run with:\n\n```text\n{}```",
        toolchain
    )?;

    // scripts/slint_sc_test_suite.sh decides what gets collected: every
    // *.log is a captured `cargo test` log, every *.json a CTRF-style
    // report from one of the harnesses.
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("error reading test results directory {dir:?}"))?
        .filter_map(|entry| Some(entry.ok()?.path()))
        .collect();
    files.sort();

    let mut suites: Vec<Suite> = Vec::new();
    let mut reports: Vec<CtrfReport> = Vec::new();
    for path in &files {
        let text = || {
            std::fs::read_to_string(path)
                .with_context(|| format!("error reading test results file {path:?}"))
        };
        match path.extension().and_then(|e| e.to_str()) {
            Some("log") => {
                let parsed = parse_libtest_log(&text()?)
                    .with_context(|| format!("error parsing {path:?}"))?;
                anyhow::ensure!(!parsed.is_empty(), "no test result lines in {path:?}");
                suites.extend(parsed);
            }
            Some("json") => reports
                .push(parse_ctrf(&text()?).with_context(|| format!("error parsing {path:?}"))?),
            _ => {}
        }
    }
    anyhow::ensure!(!reports.is_empty(), "no CTRF reports in {dir:?}");

    // A harness report supersedes the libtest summary row of the binary
    // that ran it (e.g. the syntax tests are one libtest test, but the
    // report has one entry per test file). Reports of standalone harnesses
    // get their own row.
    for report in &reports {
        let row = Suite::from_cases(&report.tool, &report.cases);
        match suites.iter_mut().find(|s| same_suite(&s.name, &report.tool)) {
            Some(suite) => *suite = row,
            None => suites.push(row),
        }
    }
    suites.retain(|s| s.tests > 0);

    let total = |f: fn(&Suite) -> u64| suites.iter().map(f).sum::<u64>();
    let sha = crate::traceability::git_head(&crate::root_dir());
    writeln!(
        out,
        "\n{commit}\n\n\
         **{tests} tests: {passed} passed, {failed} failed.**\n\n\
         | Suite | Tests | Passed | Failed |\n| --- | --- | --- | --- |",
        commit = crate::traceability::commit_line(&sha),
        tests = total(|s| s.tests),
        passed = total(|s| s.passed),
        failed = total(|s| s.failed),
    )?;
    for s in &suites {
        writeln!(out, "| `{}` | {} | {} | {} |", s.name, s.tests, s.passed, s.failed)?;
    }

    reports.sort_by_key(|r| presentation(&r.tool).order);
    for report in &mut reports {
        let listing = presentation(&report.tool);
        if let Some(prefix) = listing.subset {
            report.cases.retain(|c| c.name.starts_with(prefix));
            anyhow::ensure!(
                !report.cases.is_empty(),
                "no `{prefix}` cases in the `{}` report",
                report.tool
            );
        }
        report.cases.sort_by(|a, b| a.name.cmp(&b.name));
        writeln!(
            out,
            "\n## {}\n\n{}\n\n| {} | Result |\n| --- | --- |",
            listing.heading, listing.intro, listing.column
        )?;
        for case in &report.cases {
            writeln!(
                out,
                "| [`{}`]({REPO_URL}/blob/{sha}/{}) | {} |",
                case.name,
                case.file_path,
                outcome(case.passed),
            )?;
        }
    }
    Ok(())
}

/// How the manual presents one report's per-case listing.
struct Listing<'a> {
    /// Position among the listings.
    order: usize,
    heading: &'a str,
    intro: &'a str,
    /// Header of the case-name column.
    column: &'a str,
    /// Only list the cases with this name prefix.
    subset: Option<&'a str>,
}

/// The editorial presentation of the known harnesses' reports; a report from
/// an unknown harness is listed as-is under its tool name.
fn presentation(tool: &str) -> Listing<'_> {
    match tool {
        "syntax-tests" => Listing {
            order: 0,
            heading: "Slint Compiler",
            intro: "The `slint-sc/` subset of the compiler's syntax tests: each file is compiled in Slint SC mode and its diagnostics are checked against the expectations embedded in the file.",
            column: "Test file",
            subset: Some("slint-sc/"),
        },
        "slint-sc-driver" => Listing {
            order: 1,
            heading: "Slint SC Runtime",
            intro: "The test driver cases: each case is a `.slint` file compiled with `slint-compiler`; the embedded Rust test code is built against the runtime and executed.",
            column: "Case",
            subset: None,
        },
        _ => Listing { order: usize::MAX, heading: tool, intro: "", column: "Case", subset: None },
    }
}

fn outcome(passed: bool) -> &'static str {
    if passed { "✅" } else { "❌" }
}

/// Parse a CTRF-style report as written by the test harnesses: the tool
/// name and `results.tests[].{name,filePath,status}`. The summary counts
/// are derived from the cases rather than read from the report.
fn parse_ctrf(text: &str) -> anyhow::Result<CtrfReport> {
    let v: Value = serde_json::from_str(text)?;
    let results = v.get("results").context("missing `results`")?;
    let tool = results
        .get("tool")
        .and_then(|t| t.get("name"))
        .and_then(Value::as_str)
        .context("missing `tool.name`")?;
    let mut cases = Vec::new();
    for t in results.get("tests").and_then(Value::as_array).context("missing `tests`")? {
        let field = |key: &str| {
            t.get(key).and_then(Value::as_str).with_context(|| format!("missing test `{key}`"))
        };
        cases.push(Case {
            name: field("name")?.into(),
            file_path: field("filePath")?.into(),
            passed: field("status")? == "passed",
        });
    }
    Ok(CtrfReport { tool: tool.into(), cases })
}

/// Remove ANSI escape sequences; the captured logs contain them when cargo
/// runs with colors forced, as on CI.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip parameter bytes up to and including the final letter.
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The per-binary summaries of a captured `cargo test` log: pairs each
/// `Running ...` (or `Doc-tests ...`) line with the `test result:` line that
/// follows it.
fn parse_libtest_log(text: &str) -> anyhow::Result<Vec<Suite>> {
    let mut suites = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let line = strip_ansi(line);
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Running ") {
            current = Some(binary_name(rest));
        } else if let Some(rest) = t.strip_prefix("Doc-tests ") {
            current = Some(format!("doc-tests {}", rest.trim()));
        } else if let Some(rest) = t.strip_prefix("test result:") {
            let name =
                current.take().with_context(|| format!("test result without a suite: {t}"))?;
            suites.push(parse_result_line(name, rest)?);
        }
    }
    Ok(suites)
}

/// The test binary's name from the tail of a `Running` line, e.g.
/// `unittests lib.rs (target/debug/deps/i_slint_compiler-1a2b3c)` ->
/// `i_slint_compiler`.
fn binary_name(rest: &str) -> String {
    let path = rest.rfind('(').map_or(rest, |i| rest[i + 1..].trim_end_matches(')'));
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    file.rsplit_once('-').map_or(file, |(name, _)| name).to_string()
}

/// The counts from the tail of a `test result:` line, e.g.
/// ` ok. 113 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; ...`.
/// Ignored tests aren't run, so they don't count.
fn parse_result_line(name: String, rest: &str) -> anyhow::Result<Suite> {
    let mut prev: Option<u64> = None;
    let mut passed = None;
    let mut failed = None;
    for token in rest.split_whitespace() {
        match token.trim_end_matches(';') {
            "passed" => passed = prev,
            "failed" => failed = prev,
            _ => prev = token.parse().ok(),
        }
    }
    let passed = passed.with_context(|| format!("no passed count in test result:{rest}"))?;
    let failed = failed.with_context(|| format!("no failed count in test result:{rest}"))?;
    Ok(Suite { name, tests: passed + failed, passed, failed })
}

#[test]
fn test_parse_ctrf() {
    let text = r#"{"results": {
        "tool": {"name": "slint-sc-driver"},
        "summary": {"tests": 2, "passed": 1, "failed": 1},
        "tests": [
            {"name": "component/window", "filePath": "api/slint-sc/tests/cases/component/window.slint", "status": "passed"},
            {"name": "lexical/comments", "filePath": "api/slint-sc/tests/cases/lexical/comments.slint", "status": "failed"}
        ]
    }}"#;
    let report = parse_ctrf(text).unwrap();
    assert_eq!(report.tool, "slint-sc-driver");
    assert_eq!(report.cases.len(), 2);
    assert_eq!(report.cases[0].name, "component/window");
    assert_eq!(report.cases[0].file_path, "api/slint-sc/tests/cases/component/window.slint");
    assert!(report.cases[0].passed);
    assert!(!report.cases[1].passed);

    let suite = Suite::from_cases(&report.tool, &report.cases);
    assert_eq!((suite.tests, suite.passed, suite.failed), (2, 1, 1));

    assert!(parse_ctrf(r#"{"results": {}}"#).is_err());
    // A report without per-case source paths is malformed.
    assert!(
        parse_ctrf(
            r#"{"results": {"tool": {"name": "x"}, "tests": [{"name": "a", "status": "passed"}]}}"#
        )
        .is_err()
    );
}

#[test]
fn test_same_suite() {
    // libtest reports the crate name with underscores; the CTRF tool name
    // uses the crate's hyphenated spelling.
    assert!(same_suite("syntax_tests", "syntax-tests"));
    assert!(same_suite("driver", "driver"));
    assert!(!same_suite("driver", "slint-sc-driver"));
}

#[test]
fn test_parse_libtest_log() {
    // The first `Running` line is colored, as cargo emits it when colors
    // are forced (e.g. on CI).
    let log = "\
   Compiling i-slint-compiler v1.18.0\n\
\u{1b}[1m\u{1b}[92m     Running\u{1b}[0m unittests lib.rs (target/debug/deps/i_slint_compiler-1a2b3c4d)\n\
running 113 tests\n\
test parser::tests::basic ... ok\n\
test result: ok. 113 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 1.88s\n\
     Running tests/consistent_styles.rs (target/debug/deps/consistent_styles-9a8b7c)\n\
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s\n\
     Running tests/syntax_tests.rs (target\\debug\\deps\\syntax_tests-5c4d3e)\n\
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.68s\n\
   Doc-tests i_slint_compiler\n\
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
    let suites = parse_libtest_log(log).unwrap();
    let rows: Vec<_> =
        suites.iter().map(|s| (s.name.as_str(), s.tests, s.passed, s.failed)).collect();
    assert_eq!(
        rows,
        [
            ("i_slint_compiler", 113, 113, 0),
            ("consistent_styles", 1, 1, 0),
            ("syntax_tests", 2, 1, 1),
            ("doc-tests i_slint_compiler", 0, 0, 0),
        ]
    );
}
