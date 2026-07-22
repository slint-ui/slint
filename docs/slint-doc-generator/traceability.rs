// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Traceability matrix between the requirement paragraphs of the Language
//! Specification (`{#sls.…}` anchors) and the test cases that reference them
//! with `//#sls.…` comments.

use crate::Config;
use anyhow::Context;
use std::collections::HashMap;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Canonical location of the specification chapters, relative to the
/// repository root. The safety manual syncs them at build time and serves
/// them under the `language/` slug.
const SPEC_DIR: &str = "docs/astro/src/content/docs/reference/language";

/// Sidebar order of the specification pages in docs/safety/astro.config.mjs.
/// Pages not listed here are ordered alphabetically.
const SPEC_PAGE_ORDER: &[&str] =
    &["index", "source-files", "lexical-structure", "file-structure", "imports", "exports"];

/// Directories scanned for `.slint` test cases with `//#sls.…` references,
/// as (kind label shown in the matrix, path relative to the repository root).
const TEST_ROOTS: &[(&str, &str)] =
    &[("case", "api/slint-sc/tests/cases"), ("syntax", "internal/compiler/tests/syntax/slint-sc")];

const REPO_URL: &str = env!("CARGO_PKG_REPOSITORY");

struct SpecPage {
    /// File name without the `.md` extension.
    stem: String,
    /// From the frontmatter.
    title: String,
    /// (anchor id, 1-based line number) in document order.
    anchors: Vec<(String, usize)>,
    /// Draft pages aren't published, so their anchors don't exist.
    draft: bool,
}

struct TestRef {
    id: String,
    /// Repository-relative path with `/` separators.
    file: String,
    line: usize,
    /// The [`TEST_ROOTS`] entry the file was found under.
    kind: &'static (&'static str, &'static str),
}

impl TestRef {
    /// Path relative to the test root, for display.
    fn short(&self) -> &str {
        &self.file[self.kind.1.len() + 1..]
    }
}

pub fn generate(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let root = crate::root_dir();
    let pages = scan_spec_pages(&root.join(SPEC_DIR))?;

    let mut refs = Vec::new();
    for kind in TEST_ROOTS {
        scan_test_refs(&root, kind, &mut refs)?;
    }

    let errors = check(&pages, &refs);
    if !errors.is_empty() {
        eprintln!("error: traceability:");
        for e in &errors {
            eprintln!("  {e}");
        }
        return Err(anyhow::anyhow!("{} traceability error(s)", errors.len()).into());
    }

    let mut tests_by_id: HashMap<&str, Vec<&TestRef>> = HashMap::new();
    for r in &refs {
        let files = tests_by_id.entry(&r.id).or_default();
        if !files.iter().any(|t| t.file == r.file) {
            files.push(r);
        }
    }

    write_matrix(cfg, &root, &pages, &tests_by_id)
}

/// Validate the parsed pages and test references, returning one message per
/// problem: duplicate anchor ids across the specification, and test
/// references without a matching anchor.
fn check(pages: &[SpecPage], refs: &[TestRef]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen: HashMap<&str, (&str, usize)> = HashMap::new();
    for p in pages {
        for (id, line) in &p.anchors {
            match seen.get(id.as_str()) {
                Some((stem, first)) => errors.push(format!(
                    "{SPEC_DIR}/{}.md:{line}: duplicate anchor `{{#{id}}}`, already defined at {SPEC_DIR}/{stem}.md:{first}",
                    p.stem
                )),
                None => {
                    seen.insert(id, (&p.stem, *line));
                }
            }
        }
    }
    for r in refs {
        if !seen.contains_key(r.id.as_str()) {
            errors.push(format!(
                "{}:{}: `//#{}` has no `{{#{}}}` anchor in {SPEC_DIR}/",
                r.file, r.line, r.id, r.id
            ));
        }
    }
    errors
}

/// Parse a `{#sls.…}` marker at the end of a line, mirroring `ID_MARKER` in
/// docs/common/src/utils/rehype-sls-ids.mjs.
fn anchor_id(line: &str) -> Option<&str> {
    let t = line.trim_end().strip_suffix('}')?;
    let id = &t[t.rfind("{#")? + 2..];
    (id.len() > "sls.".len()
        && id.starts_with("sls.")
        && id.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'_')
        }))
    .then_some(id)
}

/// Parse one specification chapter: frontmatter title and draft flag, and the
/// anchors in document order. Anchors inside HTML comments and code fences
/// don't render, so they don't count.
fn parse_spec_page(stem: &str, text: &str) -> SpecPage {
    let mut page = SpecPage {
        stem: stem.to_string(),
        title: String::new(),
        anchors: Vec::new(),
        draft: false,
    };
    let mut in_comment = false;
    let mut in_fence = false;
    let mut frontmatter_delimiters = 0;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if frontmatter_delimiters < 2 {
            if t == "---" {
                frontmatter_delimiters += 1;
            } else if let Some(title) = t.strip_prefix("title:") {
                page.title = title.trim().to_string();
            } else if t == "draft: true" {
                page.draft = true;
            }
            continue;
        }
        if in_comment {
            in_comment = !t.contains("-->");
            continue;
        }
        if t.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if t.starts_with("<!--") {
            in_comment = !t.contains("-->");
            continue;
        }
        if let Some(id) = anchor_id(line) {
            page.anchors.push((id.to_string(), i + 1));
        }
    }
    page
}

fn scan_spec_pages(dir: &Path) -> Result<Vec<SpecPage>, Box<dyn std::error::Error>> {
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .context(format!("error reading {dir:?}"))?
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    paths.sort_by_key(|p| {
        let stem = p.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        (SPEC_PAGE_ORDER.iter().position(|s| *s == stem).unwrap_or(SPEC_PAGE_ORDER.len()), stem)
    });

    let mut pages = Vec::new();
    for path in paths {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).context(format!("error reading {path:?}"))?;
        let page = parse_spec_page(&stem, &text);
        if !page.draft {
            pages.push(page);
        }
    }
    Ok(pages)
}

fn scan_test_refs(
    repo_root: &Path,
    kind: &'static (&'static str, &'static str),
    refs: &mut Vec<TestRef>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in walkdir::WalkDir::new(repo_root.join(kind.1)).sort_by_file_name() {
        let entry = entry?;
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|e| e != "slint") {
            continue;
        }
        let text = std::fs::read_to_string(entry.path())
            .context(format!("error reading {:?}", entry.path()))?;
        let file = entry
            .path()
            .strip_prefix(repo_root)?
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        for (i, line) in text.lines().enumerate() {
            if let Some(id) = line.trim().strip_prefix("//#") {
                refs.push(TestRef {
                    id: id.trim().to_string(),
                    file: file.clone(),
                    line: i + 1,
                    kind,
                });
            }
        }
    }
    Ok(())
}

/// The commit to link test files to on GitHub.
fn git_head(repo_root: &Path) -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "master".to_string())
}

fn write_matrix(
    cfg: &Config,
    repo_root: &Path,
    pages: &[SpecPage],
    tests_by_id: &HashMap<&str, Vec<&TestRef>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let sha = git_head(repo_root);
    std::fs::create_dir_all(&cfg.generated_dir)?;
    let path = cfg.generated_dir.join("traceability-matrix.md");
    let mut file =
        BufWriter::new(std::fs::File::create(&path).context(format!("error creating {path:?}"))?);

    let total: usize = pages.iter().map(|p| p.anchors.len()).sum();
    let covered = pages
        .iter()
        .flat_map(|p| &p.anchors)
        .filter(|(id, _)| tests_by_id.contains_key(id.as_str()))
        .count();

    writeln!(
        file,
        r#"---
title: Traceability Matrix
description: Mapping between the requirement paragraphs of the Language Specification and the test cases that verify them.
slug: qualification-plan/traceability-matrix
---

Each requirement paragraph in the [Language Specification](../../language/) carries a unique identifier,
shown as a `[sls.…]` badge at the end of the paragraph.
A test case declares which requirements it verifies by listing their identifiers in `//#sls.…` comments.
This matrix lists every requirement paragraph with the test cases that declare it.
Requirements not yet covered by any test are marked ❌.

Tests marked `case:` are executed test cases from `{case_root}/`,
tests marked `syntax:` are compiler syntax tests from `{syntax_root}/`.

**Coverage: {covered} of {total} requirement paragraphs are covered by at least one test.**"#,
        case_root = TEST_ROOTS[0].1,
        syntax_root = TEST_ROOTS[1].1,
    )?;

    // The index page's title heads the section; the other pages nest under it.
    for page in pages {
        if page.anchors.is_empty() {
            continue;
        }
        // The matrix page is two levels deep, so `../../` is the site root.
        let base = if page.stem == "index" {
            writeln!(file, "\n## {}", page.title)?;
            "../../language/".to_string()
        } else {
            writeln!(file, "\n### {}", page.title)?;
            format!("../../language/{}/", page.stem)
        };
        writeln!(file, "\n| Paragraph | Tests |")?;
        writeln!(file, "| --- | --- |")?;
        for (id, _) in &page.anchors {
            let tests = match tests_by_id.get(id.as_str()) {
                Some(files) => files
                    .iter()
                    .map(|t| {
                        format!("[`{}: {}`]({REPO_URL}/blob/{sha}/{})", t.kind.0, t.short(), t.file)
                    })
                    .collect::<Vec<_>>()
                    .join("<br/>"),
                None => "❌".to_string(),
            };
            writeln!(file, "| [`{id}`]({base}#{id}) | {tests} |")?;
        }
    }
    Ok(())
}

#[test]
fn test_anchor_id() {
    assert_eq!(anchor_id("Some text. {#sls.foo.bar}"), Some("sls.foo.bar"));
    assert_eq!(anchor_id("Text. {#sls.a-b_c.d2}  "), Some("sls.a-b_c.d2"));
    assert_eq!(anchor_id("{#sls.x}"), Some("sls.x"));
    assert_eq!(anchor_id("no marker here"), None);
    assert_eq!(anchor_id("not at the end {#sls.x} more"), None);
    assert_eq!(anchor_id("wrong prefix {#foo.bar}"), None);
    assert_eq!(anchor_id("bad chars {#sls.Foo}"), None);
    assert_eq!(anchor_id("bad chars {#sls.a b}"), None);
    assert_eq!(anchor_id("empty tail {#sls.}"), None);
}

#[test]
fn test_parse_spec_page() {
    let text = r#"---
title: Some Chapter
description: not an anchor {#sls.frontmatter}
---

A paragraph. {#sls.one}

<!--
Commented out. {#sls.commented}
-->

```slint
// code {#sls.fenced}
```

Another paragraph. {#sls.two}
"#;
    let page = parse_spec_page("chapter", text);
    assert_eq!(page.stem, "chapter");
    assert_eq!(page.title, "Some Chapter");
    assert!(!page.draft);
    assert_eq!(page.anchors, [("sls.one".to_string(), 6), ("sls.two".to_string(), 16)]);

    let draft = parse_spec_page("draft", "---\ntitle: Draft\ndraft: true\n---\nText. {#sls.d}\n");
    assert!(draft.draft);
    assert_eq!(draft.anchors, [("sls.d".to_string(), 5)]);
}

#[test]
fn test_check_reports_all_errors() {
    let page = |stem: &str, anchors: &[(&str, usize)]| SpecPage {
        stem: stem.to_string(),
        title: String::new(),
        anchors: anchors.iter().map(|(id, line)| (id.to_string(), *line)).collect(),
        draft: false,
    };
    let test_ref = |id: &str, file: &str, line| TestRef {
        id: id.to_string(),
        file: file.to_string(),
        line,
        kind: &TEST_ROOTS[0],
    };

    let pages = [page("a", &[("sls.one", 5), ("sls.two", 8)]), page("b", &[("sls.one", 3)])];
    let refs = [
        test_ref("sls.two", "tests/t1.slint", 4),
        test_ref("sls.gone", "tests/t1.slint", 9),
        test_ref("sls.also-gone", "tests/t2.slint", 7),
    ];

    // All errors are reported in one pass, not just the first.
    let errors = check(&pages, &refs);
    assert_eq!(errors.len(), 3);
    assert!(errors[0].contains("b.md:3"), "{}", errors[0]);
    assert!(errors[0].contains("{#sls.one}") && errors[0].contains("a.md:5"), "{}", errors[0]);
    assert!(
        errors[1].contains("tests/t1.slint:9") && errors[1].contains("sls.gone"),
        "{}",
        errors[1]
    );
    assert!(
        errors[2].contains("tests/t2.slint:7") && errors[2].contains("sls.also-gone"),
        "{}",
        errors[2]
    );

    let clean_pages = [page("a", &[("sls.one", 5), ("sls.two", 8)])];
    assert!(check(&clean_pages, &refs[..1]).is_empty());
}
