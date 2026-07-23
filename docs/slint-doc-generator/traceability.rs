// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Traceability matrix between the requirement paragraphs of the Language
//! Specification and the generated SC API Reference (`{#sls.…}` anchors) and
//! the test cases that reference them with `//#sls.…` comments.

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

/// Handwritten safety-manual pages may also state requirements.
const SAFETY_DOCS_DIR: &str = "docs/safety/src/content/docs";

/// Subdirectories of [`SAFETY_DOCS_DIR`] whose anchors are already scanned
/// from their canonical source: the generated pages and the specification
/// chapters synced from [`SPEC_DIR`].
const SAFETY_DOCS_EXCLUDE: &[&str] = &["generated", "language"];

/// Name of the matrix this module writes into
/// [`Config::qualification_plan_dir`], the section it belongs to.
const MATRIX_FILE: &str = "traceability-matrix.mdx";

const REPO_URL: &str = env!("CARGO_PKG_REPOSITORY");

struct SpecPage {
    /// Repository-relative path with `/` separators, for error messages.
    file: String,
    /// From the frontmatter.
    title: String,
    /// Site-relative URL of the page, for linking its anchors from the matrix.
    /// The matrix sits two levels deep, so it starts with `../../`.
    base: String,
    /// The specification index heads its section; every other page nests
    /// under one.
    top_level: bool,
    /// (anchor id, 1-based line number) in document order.
    anchors: Vec<(String, usize)>,
    /// Draft pages aren't published, so their anchors don't exist.
    draft: bool,
    /// Covers the full language only: the safety manual leaves the chapter
    /// out, so its anchors, if any, aren't part of the traceability corpus.
    not_in_sc: bool,
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
    let spec_pages = scan_spec_pages(&root.join(SPEC_DIR))?;
    let reference_pages = scan_reference_pages(cfg, &root)?;
    let safety_pages = scan_safety_pages(&root)?;

    let mut refs = Vec::new();
    for kind in TEST_ROOTS {
        scan_test_refs(&root, kind, &mut refs)?;
    }

    let errors = check(&[&spec_pages, &reference_pages, &safety_pages], &refs);
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

    write_matrix(cfg, &root, &spec_pages, &reference_pages, &safety_pages, &tests_by_id)
}

/// Validate the parsed pages and test references, returning one message per
/// problem: duplicate anchor ids across all the scanned pages, and test
/// references without a matching anchor.
fn check(page_sets: &[&[SpecPage]], refs: &[TestRef]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen: HashMap<&str, (&str, usize)> = HashMap::new();
    for p in page_sets.iter().flat_map(|set| set.iter()) {
        for (id, line) in &p.anchors {
            match seen.get(id.as_str()) {
                Some((file, first)) => errors.push(format!(
                    "{}:{line}: duplicate anchor `{{#{id}}}`, already defined at {file}:{first}",
                    p.file
                )),
                None => {
                    seen.insert(id, (&p.file, *line));
                }
            }
        }
    }
    for r in refs {
        if !seen.contains_key(r.id.as_str()) {
            errors.push(format!(
                "{}:{}: `//#{}` has no `{{#{}}}` anchor in the specification, the SC reference, or the safety manual",
                r.file, r.line, r.id, r.id
            ));
        }
    }
    errors
}

/// Split a trailing `{#sls.…}` marker off a line, returning the prose before
/// it and the identifier. Mirrors `ID_MARKER` in
/// docs/common/src/utils/rehype-sls-ids.mjs (and the `.sls-id` styling in
/// docs/common/src/styles/sls-ids.css). The sources write the marker in its
/// MDX-safe escaped form `\{#sls.…}`; the backslash sits before the `{` found
/// via `rfind`, so both forms parse the same here.
fn split_marker(line: &str) -> Option<(&str, &str)> {
    let t = line.trim_end().strip_suffix('}')?;
    let start = t.rfind("{#")?;
    let id = &t[start + 2..];
    let is_id = id.len() > "sls.".len()
        && id.starts_with("sls.")
        && id.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'_')
        });
    // Keep the escaping backslash out of the prose.
    is_id.then(|| (line[..start].trim_end().trim_end_matches('\\').trim_end(), id))
}

/// The identifier of a trailing `{#sls.…}` marker.
fn anchor_id(line: &str) -> Option<&str> {
    split_marker(line).map(|(_, id)| id)
}

/// Parse one page: frontmatter title, slug and draft flag, and the anchors in
/// document order. Anchors inside HTML comments and code fences don't render,
/// so they don't count. The caller turns the slug into [`SpecPage::base`].
fn parse_spec_page(file: &str, text: &str) -> (SpecPage, Option<String>) {
    let mut slug = None;
    let mut page = SpecPage {
        file: file.to_string(),
        title: String::new(),
        base: String::new(),
        top_level: false,
        anchors: Vec::new(),
        draft: false,
        not_in_sc: false,
    };
    let mut in_comment = false;
    let mut in_fence = false;
    // `<NotInSC>` regions aren't published in the safety manual, so whatever
    // they enclose states no requirement. See rehype-not-in-sc.mjs.
    let mut in_not_in_sc = false;
    let mut frontmatter_delimiters = 0;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if frontmatter_delimiters < 2 {
            if t == "---" {
                frontmatter_delimiters += 1;
            } else if let Some(title) = t.strip_prefix("title:") {
                page.title = title.trim().to_string();
            } else if let Some(s) = t.strip_prefix("slug:") {
                slug = Some(s.trim().to_string());
            } else if t == "draft: true" {
                page.draft = true;
            } else if t == "notInSC: true" {
                page.not_in_sc = true;
            }
            continue;
        }
        if in_comment {
            in_comment = !t.contains("-->") && !t.contains("*/}");
            continue;
        }
        if t.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if t == "<NotInSC>" {
            in_not_in_sc = true;
            continue;
        }
        if t == "</NotInSC>" {
            in_not_in_sc = false;
            continue;
        }
        if in_not_in_sc {
            continue;
        }
        // Both comment forms: markdown pages use `<!-- -->`, MDX `{/* */}`.
        if t.starts_with("<!--") {
            in_comment = !t.contains("-->");
            continue;
        }
        if t.starts_with("{/*") {
            in_comment = !t.contains("*/}");
            continue;
        }
        if let Some(id) = anchor_id(line) {
            page.anchors.push((id.to_string(), i + 1));
        }
    }
    (page, slug)
}

/// Repository-relative path with `/` separators.
fn repo_relative(path: &Path, repo_root: &Path) -> String {
    let relative = path.strip_prefix(repo_root).unwrap_or(path).to_string_lossy().into_owned();
    if std::path::MAIN_SEPARATOR == '/' {
        relative
    } else {
        relative.replace(std::path::MAIN_SEPARATOR, "/")
    }
}

fn scan_spec_pages(dir: &Path) -> Result<Vec<SpecPage>, Box<dyn std::error::Error>> {
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .context(format!("error reading {dir:?}"))?
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "md" || e == "mdx"))
        .collect();
    paths.sort_by_key(|p| {
        let stem = p.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        (SPEC_PAGE_ORDER.iter().position(|s| *s == stem).unwrap_or(SPEC_PAGE_ORDER.len()), stem)
    });

    let mut pages = Vec::new();
    for path in paths {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).context(format!("error reading {path:?}"))?;
        let file = path.file_name().unwrap_or_default().to_string_lossy();
        let (mut page, _) = parse_spec_page(&format!("{SPEC_DIR}/{file}"), &text);
        if page.not_in_sc {
            continue;
        }
        // The index page is served at the root of the specification.
        page.top_level = stem == "index";
        page.base = if page.top_level {
            "../../language/".to_string()
        } else {
            format!("../../language/{stem}/")
        };
        if !page.draft {
            pages.push(page);
        }
    }
    Ok(pages)
}

/// Parse the generated SC API reference pages for their anchors. The pages
/// are written by `element_docs`/`mdx` earlier in the same run.
fn scan_reference_pages(
    cfg: &Config,
    repo_root: &Path,
) -> Result<Vec<SpecPage>, Box<dyn std::error::Error>> {
    let mut pages = Vec::new();
    for entry in walkdir::WalkDir::new(cfg.reference_dir()).sort_by_file_name() {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension().is_none_or(|e| e != "md" && e != "mdx")
        {
            continue;
        }
        let file = repo_relative(path, repo_root);
        let text = std::fs::read_to_string(path).context(format!("error reading {path:?}"))?;
        let (mut page, slug) = parse_spec_page(&file, &text);
        if page.anchors.is_empty() || page.draft {
            continue;
        }
        // Every generated page sets its own slug; without one the matrix
        // couldn't link to the anchors it just found.
        let slug = slug
            .ok_or_else(|| anyhow::anyhow!("{file}: generated page carries anchors but no slug"))?;
        page.base = format!("../../{slug}/");
        pages.push(page);
    }
    Ok(pages)
}

/// The site slug of a handwritten safety-manual page, derived from its path
/// relative to [`SAFETY_DOCS_DIR`] like Astro does.
fn safety_page_slug(relative: &str) -> &str {
    let slug =
        relative.strip_suffix(".mdx").or_else(|| relative.strip_suffix(".md")).unwrap_or(relative);
    slug.strip_suffix("index").unwrap_or(slug).trim_end_matches('/')
}

/// Parse the handwritten safety-manual pages for their anchors.
fn scan_safety_pages(repo_root: &Path) -> Result<Vec<SpecPage>, Box<dyn std::error::Error>> {
    let dir = repo_root.join(SAFETY_DOCS_DIR);
    let mut pages = Vec::new();
    for entry in walkdir::WalkDir::new(&dir)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            !(e.file_type().is_dir()
                && e.depth() == 1
                && SAFETY_DOCS_EXCLUDE.contains(&e.file_name().to_string_lossy().as_ref()))
        })
        .flatten()
    {
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension().is_none_or(|e| e != "md" && e != "mdx")
        {
            continue;
        }
        let file = repo_relative(path, repo_root);
        let text = std::fs::read_to_string(path).context(format!("error reading {path:?}"))?;
        let (mut page, slug) = parse_spec_page(&file, &text);
        if page.anchors.is_empty() || page.draft {
            continue;
        }
        let relative = repo_relative(path, &dir);
        let slug = slug.unwrap_or_else(|| safety_page_slug(&relative).to_string());
        page.base = format!("../../{slug}/");
        pages.push(page);
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
        let file = repo_relative(entry.path(), repo_root);
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

/// Whether a paragraph is informative rather than a testable requirement:
/// the document conventions (`sls.meta.…`) and examples. These are excluded
/// from the matrix; examples are compiled by the doctests test instead.
fn informative(id: &str) -> bool {
    id.starts_with("sls.meta.") || id.split('.').any(|s| s == "example")
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
    spec_pages: &[SpecPage],
    reference_pages: &[SpecPage],
    safety_pages: &[SpecPage],
    tests_by_id: &HashMap<&str, Vec<&TestRef>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let sha = git_head(repo_root);
    let dir = cfg.qualification_plan_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(MATRIX_FILE);
    let mut file =
        BufWriter::new(std::fs::File::create(&path).context(format!("error creating {path:?}"))?);

    let all = || spec_pages.iter().chain(reference_pages).chain(safety_pages);
    let total = all().flat_map(|p| &p.anchors).filter(|(id, _)| !informative(id)).count();
    let covered = all()
        .flat_map(|p| &p.anchors)
        .filter(|(id, _)| !informative(id) && tests_by_id.contains_key(id.as_str()))
        .count();

    writeln!(
        file,
        r#"---
title: Traceability Matrix
description: Mapping between the requirement paragraphs of the Language Specification and the SC API Reference, and the test cases that verify them.
slug: qualification-plan/traceability-matrix
---

Each requirement paragraph in the [Language Specification](../../language/), the [SC API Reference](../../reference/), and the other chapters of this manual carries a unique identifier,
shown as a `[sls.…]` badge at the end of the paragraph.
A test case declares which requirements it verifies by listing their identifiers in `//#sls.…` comments.
This matrix lists every requirement paragraph with the test cases that declare it.
Requirements not yet covered by any test are marked ❌.
Informative paragraphs — the document conventions (`sls.meta.…`) and examples — are not listed;
the examples are compiled by the doctests test instead.

Tests marked `case:` are executed test cases from `{case_root}/`,
tests marked `syntax:` are compiler syntax tests from `{syntax_root}/`.

**Coverage: {covered} of {total} requirement paragraphs are covered by at least one test.**"#,
        case_root = TEST_ROOTS[0].1,
        syntax_root = TEST_ROOTS[1].1,
    )?;

    for page in spec_pages {
        write_page(&mut file, page, tests_by_id, &sha)?;
    }

    // Sections are skipped entirely when they state no testable requirement.
    if reference_pages.iter().flat_map(|p| &p.anchors).any(|(id, _)| !informative(id)) {
        writeln!(file, "\n## SC API Reference")?;
        for page in reference_pages {
            write_page(&mut file, page, tests_by_id, &sha)?;
        }
    }
    if safety_pages.iter().flat_map(|p| &p.anchors).any(|(id, _)| !informative(id)) {
        writeln!(file, "\n## Safety Manual")?;
        for page in safety_pages {
            write_page(&mut file, page, tests_by_id, &sha)?;
        }
    }
    Ok(())
}

/// One section of the matrix: the page's heading and a row per anchor.
fn write_page(
    out: &mut impl Write,
    page: &SpecPage,
    tests_by_id: &HashMap<&str, Vec<&TestRef>>,
    sha: &str,
) -> std::io::Result<()> {
    let anchors: Vec<&(String, usize)> =
        page.anchors.iter().filter(|(id, _)| !informative(id)).collect();
    // The specification index heads the section the other pages nest under,
    // so its title appears even when it states no requirement of its own.
    if page.top_level {
        writeln!(out, "\n## {}", page.title)?;
    }
    if anchors.is_empty() {
        return Ok(());
    }
    if !page.top_level {
        writeln!(out, "\n### {}", page.title)?;
    }
    writeln!(out, "\n| Paragraph | Tests |")?;
    writeln!(out, "| --- | --- |")?;
    for (id, _) in anchors {
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
        writeln!(out, "| [`{id}`]({}#{id}) | {tests} |", page.base)?;
    }
    Ok(())
}

#[test]
fn test_split_marker() {
    // The prose keeps neither the marker nor the escaping backslash.
    assert_eq!(split_marker("Some text. {#sls.foo.bar}"), Some(("Some text.", "sls.foo.bar")));
    assert_eq!(
        split_marker(r"MDX-escaped. \{#sls.foo.bar}"),
        Some(("MDX-escaped.", "sls.foo.bar"))
    );
    assert_eq!(split_marker("no marker here"), None);
}

#[test]
fn test_informative() {
    assert!(informative("sls.meta.purpose"));
    assert!(informative("sls.file.example.intro"));
    assert!(informative("sls.file.example.description"));
    assert!(!informative("sls.lex.identifier.normalization-example"));
    assert!(!informative("sls.file.component.body"));
}

#[test]
fn test_anchor_id() {
    assert_eq!(anchor_id("Some text. {#sls.foo.bar}"), Some("sls.foo.bar"));
    assert_eq!(anchor_id(r"MDX-escaped. \{#sls.foo.bar}"), Some("sls.foo.bar"));
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
    let (page, slug) = parse_spec_page("spec/chapter.md", text);
    assert_eq!(page.file, "spec/chapter.md");
    assert_eq!(page.title, "Some Chapter");
    assert_eq!(slug, None);
    assert!(!page.draft);
    assert_eq!(page.anchors, [("sls.one".to_string(), 6), ("sls.two".to_string(), 16)]);

    let (draft, _) =
        parse_spec_page("spec/draft.md", "---\ntitle: Draft\ndraft: true\n---\nText. {#sls.d}\n");
    assert!(draft.draft);
    assert_eq!(draft.anchors, [("sls.d".to_string(), 5)]);

    assert!(!page.not_in_sc);
    // The flag is parsed; scan_spec_pages skips such pages, anchors and all.
    let (flagged, _) = parse_spec_page(
        "spec/functions.mdx",
        "---\ntitle: Functions\nnotInSC: true\n---\nFull-language prose. \\{#sls.fn.decl}\n",
    );
    assert!(flagged.not_in_sc);
    assert_eq!(flagged.anchors, [("sls.fn.decl".to_string(), 5)]);

    let (reference, slug) = parse_spec_page(
        "generated/elements/rectangle.mdx",
        "---\ntitle: Rectangle\nslug: reference/elements/rectangle\n---\nProse. \\{#sls.ref.rectangle.purpose}\n",
    );
    assert_eq!(slug.as_deref(), Some("reference/elements/rectangle"));
    assert_eq!(reference.anchors, [("sls.ref.rectangle.purpose".to_string(), 5)]);
}

#[test]
fn test_check_reports_all_errors() {
    let page = |stem: &str, anchors: &[(&str, usize)]| SpecPage {
        file: format!("{stem}.md"),
        title: String::new(),
        base: String::new(),
        top_level: false,
        anchors: anchors.iter().map(|(id, line)| (id.to_string(), *line)).collect(),
        draft: false,
        not_in_sc: false,
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
    let errors = check(&[&pages], &refs);
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
    assert!(check(&[&clean_pages], &refs[..1]).is_empty());

    // A duplicate between the specification and the reference pages is
    // caught, and a test may reference a reference anchor.
    let reference = [page("rectangle", &[("sls.one", 2), ("sls.ref.covered", 4)])];
    let errors = check(&[&clean_pages, &reference], &[test_ref("sls.ref.covered", "t.slint", 1)]);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("{#sls.one}"), "{}", errors[0]);
}

#[test]
fn test_safety_page_slug() {
    assert_eq!(safety_page_slug("reference/rendering.md"), "reference/rendering");
    assert_eq!(safety_page_slug("safety-policy.md"), "safety-policy");
    assert_eq!(safety_page_slug("reference/index.md"), "reference");
    assert_eq!(safety_page_slug("index.mdx"), "");
}
