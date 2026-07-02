// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Generate a third-party license listing.
//!
//! This is a self-contained replacement for `cargo about`. It resolves the
//! dependency graph across all target platforms via `cargo metadata`,
//! restricting it to the features actually enabled in the analyzed crate and to
//! the crates linked into the program, then renders one table of those
//! dependencies followed by the canonical text of each license they use.

// cSpell: ignore licence rsplit

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

/// SPDX license identifiers we accept for third-party dependencies. A crate is
/// rejected unless its license expression is satisfiable by this set.
const ACCEPTED: &[&str] = &[
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "MPL-2.0",
    "Zlib",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CC0-1.0",
    "BSL-1.0",
    "ISC",
    "Unicode-DFS-2016",
    "Unicode-3.0",
    "OpenSSL",
    "Unlicense",
    "WTFPL",
];

/// Skip dependencies reachable only through dev-dependency edges.
const IGNORE_DEV_DEPENDENCIES: bool = true;

/// The output format of the generated listing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// Human-readable Markdown: a dependency table followed by the license texts.
    #[default]
    Markdown,
    /// Machine-readable JSON: `{ "crates": [...], "licenses": [...] }`, consumed
    /// by the Slint Viewer's in-app attribution page.
    Json,
    /// An iOS `Settings.bundle` directory (requires `-o`): a "Third-Party
    /// Licenses" child pane listing every crate, each opening a page with its
    /// author and license text. Surfaced in the system Settings app.
    IosSettingsBundle,
}

#[derive(Debug, clap::Parser)]
pub struct LicenseCommand {
    /// Path to the `Cargo.toml` whose dependencies should be analyzed.
    /// Defaults to `Cargo.toml` in the current directory.
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    /// Comma-separated features to enable on the analyzed crate (repeatable).
    #[arg(long, value_delimiter = ',')]
    features: Vec<String>,
    /// Do not enable the analyzed crate's default features.
    #[arg(long)]
    no_default_features: bool,
    /// Enable all features of the analyzed crate.
    #[arg(long)]
    all_features: bool,
    /// The output format.
    #[arg(long, value_enum, default_value_t)]
    format: Format,
    /// Where to write the result. Writes to stdout when omitted.
    #[arg(short = 'o', long)]
    output: Option<PathBuf>,
}

impl LicenseCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let manifest_path = match &self.manifest_path {
            Some(p) => std::env::current_dir()?.join(p),
            None => std::env::current_dir()?.join("Cargo.toml"),
        };
        generate(&GenerateArgs {
            manifest_path,
            features: Features {
                features: self.features.clone(),
                no_default_features: self.no_default_features,
                all_features: self.all_features,
            },
            format: self.format,
            output: self.output.clone(),
        })
    }
}

/// Which features to enable on the analyzed crate, mirroring cargo's flags.
#[derive(Debug, Default)]
pub struct Features {
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub all_features: bool,
}

pub struct GenerateArgs {
    pub manifest_path: PathBuf,
    pub features: Features,
    pub format: Format,
    pub output: Option<PathBuf>,
}

pub fn generate(args: &GenerateArgs) -> anyhow::Result<()> {
    let accepted: HashSet<&str> = ACCEPTED.iter().copied().collect();

    fetch_dependencies(&args.manifest_path)?;

    let (packages, allow) = resolve_packages(&args.manifest_path, &args.features)?;

    // Build one summary row per crate for the table, and collect every accepted
    // license id that appears so each one gets a body below (the table links to
    // them). Per-crate attribution lives in the table's Author / Copyright
    // column, so the texts need only one body per license.
    let mut license_names: BTreeMap<String, String> = BTreeMap::new();
    let mut used_ids: BTreeSet<String> = BTreeSet::new();
    let mut rows: Vec<CrateRow> = Vec::new();

    for pkg in &packages {
        // A few older crates declare only `license-file = "..."` (no SPDX
        // expression). Fall back to recognizing the canonical text of a
        // well-known license, and salvage a `Copyright …` line for the author
        // column from the same file.
        let (expression, fallback_author) = match pkg.license.clone() {
            Some(e) => (e, None),
            None => {
                let (id, copyright) = detect_from_license_file(pkg).with_context(|| {
                    format!("Crate {} {} has no license information", pkg.name, pkg.version)
                })?;
                (id.to_string(), Some(copyright))
            }
        };

        // Lax parsing accepts the deprecated `/` OR-separator and imprecise
        // identifiers (e.g. `apache2`) still found in older crates.
        let expr =
            spdx::Expression::parse_mode(&expression, spdx::ParseMode::LAX).with_context(|| {
                format!("Cannot parse license `{expression}` of {} {}", pkg.name, pkg.version)
            })?;

        // A `[package.metadata.slint-license.allow]` entry in the analyzed
        // crate's manifest extends the accepted set for *that one dependency
        // only*, so an exceptional license (e.g. LGPL-3.0 via dynamic linking)
        // is approved explicitly per-crate rather than relaxed project-wide.
        let pkg_allow = allow.get(pkg.name.as_str()).map(String::as_str);
        let pkg_accepted = |req: &spdx::LicenseReq| {
            let id = license_string(req);
            accepted.contains(id.as_str()) || pkg_allow == Some(id.as_str())
        };
        if !expr.evaluate(pkg_accepted) {
            bail!(
                "License `{expression}` of crate {} {} is not in the accepted list",
                pkg.name,
                pkg.version
            );
        }

        // Record the license ids the crate uses, so each gets a body.
        for req in expr.requirements() {
            if !pkg_accepted(&req.req) {
                continue;
            }
            let id = license_string(&req.req);
            license_names.entry(id.clone()).or_insert_with(|| license_full_name(&req.req));
            used_ids.insert(id);
        }

        // The `authors` field is optional and newer crates increasingly omit
        // it; salvage the copyright holder(s) from the license files shipped
        // in the crate's package instead. The upstream-declared authors win
        // over scraped copyright lines, which can name only the license's own
        // author (e.g. the FSF for verbatim LGPL boilerplate).
        let author = (!pkg.authors.is_empty())
            .then(|| pkg.authors.join(", "))
            .or(fallback_author)
            .or_else(|| authors_from_license_files(pkg))
            .unwrap_or_default();

        rows.push(CrateRow {
            name: pkg.name.to_string(),
            version: pkg.version.to_string(),
            author,
            license: expression,
        });
    }

    // One canonical license body per used license id, sorted by display name.
    let mut sections: Vec<LicenseSection> = used_ids
        .into_iter()
        .map(|id| {
            let name = license_names.get(&id).cloned().unwrap_or_else(|| id.clone());
            let text = spdx::license_id(&id)
                .map(|l| l.text().trim().to_string())
                .unwrap_or_else(|| format!("No license text available for {id}."));
            LicenseSection { id, name, text }
        })
        .collect();
    sections.sort_by(|a, b| a.name.cmp(&b.name));

    rows.sort_by(|a, b| a.name.cmp(&b.name).then(a.version.cmp(&b.version)));

    match args.format {
        Format::Markdown => write_output(&args.output, render_markdown(&rows, &sections))?,
        Format::Json => write_output(&args.output, render_json(&rows, &sections))?,
        // Unlike the single-document formats, this writes a directory tree.
        Format::IosSettingsBundle => {
            let dir = args
                .output
                .as_deref()
                .context("--format ios-settings-bundle requires -o <path to Settings.bundle>")?;
            render_ios_settings_bundle(&rows, &sections, dir)?;
        }
    }

    Ok(())
}

/// Write a rendered document to the output path, or stdout when none is given.
fn write_output(output: &Option<PathBuf>, rendered: String) -> anyhow::Result<()> {
    match output {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(path, rendered)
                .with_context(|| format!("Cannot write {}", path.display()))
        }
        None => {
            print!("{rendered}");
            Ok(())
        }
    }
}

/// One crate's row in the dependency table. The `Serialize` impl defines the
/// `crates` entries of the JSON format.
#[derive(serde::Serialize)]
struct CrateRow {
    name: String,
    version: String,
    author: String,
    license: String,
}

/// The canonical license body shown once under one license id. The
/// `Serialize` impl defines the `licenses` entries of the JSON format.
#[derive(serde::Serialize)]
struct LicenseSection {
    id: String,
    name: String,
    text: String,
}

/// Run `cargo fetch` (for all targets) so that `cargo metadata` can resolve the
/// full dependency graph offline.
fn fetch_dependencies(manifest_path: &Path) -> anyhow::Result<()> {
    let manifest = manifest_path.to_str().context("Non-UTF-8 manifest path")?;
    run_cargo(&["fetch", "--manifest-path", manifest])?;
    Ok(())
}

fn run_cargo(args: &[&str]) -> anyhow::Result<()> {
    let status = std::process::Command::new(std::env::var("CARGO").as_deref().unwrap_or("cargo"))
        .args(args)
        .status()
        .with_context(|| format!("Failed to run cargo {}", args.join(" ")))?;
    if !status.success() {
        bail!("cargo {} failed", args.join(" "));
    }
    Ok(())
}

/// Resolve the runtime dependency packages across all target platforms,
/// restricted to the features enabled in the analyzed crate and honoring
/// `IGNORE_DEV_DEPENDENCIES`.
///
/// `cargo metadata` is run without `--filter-platform`, so its resolve graph
/// spans every target. Its graph keeps optional dependencies even when no
/// active feature enables them, so the walk follows only the edges that the
/// resolved feature set of each crate actually activates.
///
/// Only crates linked into the program are listed: the walk never crosses a
/// `[build-dependencies]` edge or enters a proc-macro crate, since those are
/// used only while building and are not distributed.
fn resolve_packages(
    manifest_path: &Path,
    features: &Features,
) -> anyhow::Result<(Vec<cargo_metadata::Package>, BTreeMap<String, String>)> {
    let mut cmd = cargo_metadata::MetadataCommand::new();
    cmd.manifest_path(manifest_path);
    if !features.features.is_empty() {
        cmd.features(cargo_metadata::CargoOpt::SomeFeatures(features.features.clone()));
    }
    if features.no_default_features {
        cmd.features(cargo_metadata::CargoOpt::NoDefaultFeatures);
    }
    if features.all_features {
        cmd.features(cargo_metadata::CargoOpt::AllFeatures);
    }
    let metadata = cmd.exec()?;

    let allow = root_allow_overrides(&metadata)?;
    let packages: HashMap<cargo_metadata::PackageId, cargo_metadata::Package> =
        metadata.packages.iter().map(|p| (p.id.clone(), p.clone())).collect();

    let resolve = metadata.resolve.context("cargo metadata returned no resolve graph")?;
    let nodes: HashMap<_, _> = resolve.nodes.iter().map(|n| (n.id.clone(), n)).collect();

    // Seed the walk with the package the manifest points at. For a virtual
    // workspace manifest there is no single root, so fall back to all members.
    // Seeding from `resolve.root` (rather than every workspace member) ensures
    // we only consider the dependencies of the analyzed crate, not those of
    // unrelated examples elsewhere in the workspace.
    let mut stack: Vec<_> = match resolve.root.clone() {
        Some(root) => vec![root],
        None => metadata.workspace_members.clone(),
    };
    // The workspace's own crates are first-party, not third-party: we traverse
    // through them to reach their dependencies but never list them.
    let workspace: HashSet<&cargo_metadata::PackageId> =
        metadata.workspace_members.iter().collect();
    let mut wanted: BTreeSet<cargo_metadata::PackageId> = BTreeSet::new();
    let mut seen: HashSet<cargo_metadata::PackageId> = HashSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if !workspace.contains(&id) {
            wanted.insert(id.clone());
        }
        let (Some(node), Some(parent)) = (nodes.get(&id), packages.get(&id)) else { continue };
        let active: HashSet<&str> = node.features.iter().map(|f| f.as_str()).collect();
        for dep in &node.deps {
            let non_dev: Vec<_> = dep
                .dep_kinds
                .iter()
                .filter(|k| k.kind != cargo_metadata::DependencyKind::Development)
                .collect();
            // Drop dev-only edges.
            if IGNORE_DEV_DEPENDENCIES && !dep.dep_kinds.is_empty() && non_dev.is_empty() {
                continue;
            }
            // The non-dev kinds that apply to a real target platform (ignoring
            // synthetic build cfgs such as `cfg(fuzzing)`/`cfg(test)`/`cfg(miri)`,
            // which are never active in a normal build).
            let effective: Vec<_> = non_dev
                .iter()
                .filter(|k| !k.target.as_ref().is_some_and(|t| is_synthetic_cfg(&t.to_string())))
                .collect();
            // Drop edges that only exist under such synthetic cfgs, and edges
            // that are exclusively build-dependencies (not linked in).
            if !non_dev.is_empty() && effective.is_empty() {
                continue;
            }
            if !effective.is_empty()
                && effective.iter().all(|k| k.kind == cargo_metadata::DependencyKind::Build)
            {
                continue;
            }
            let Some(dep_pkg) = packages.get(&dep.pkg) else { continue };
            // Proc-macro crates run at build time and are not linked in.
            if is_proc_macro(dep_pkg) {
                continue;
            }
            if dependency_enabled(parent, &active, dep_pkg.name.as_str()) {
                stack.push(dep.pkg.clone());
            }
        }
    }

    let mut packages = packages;
    Ok((wanted.into_iter().filter_map(|id| packages.remove(&id)).collect(), allow))
}

/// Read the `[package.metadata.slint-license.allow]` table from the analyzed
/// crate's manifest: a map of crate name to an SPDX id additionally allowed
/// for that crate only. Used to explicitly permit an unusual license (e.g.
/// LGPL-3.0 via dynamic linking) for one specific dependency, without
/// relaxing the project-wide allowlist.
fn root_allow_overrides(
    metadata: &cargo_metadata::Metadata,
) -> anyhow::Result<BTreeMap<String, String>> {
    let Some(root) = metadata.root_package() else {
        return Ok(BTreeMap::new());
    };
    let Some(table) = root.metadata.get("slint-license").and_then(|v| v.get("allow")) else {
        return Ok(BTreeMap::new());
    };
    let table = table.as_object().context(
        "[package.metadata.slint-license.allow] must be a table of crate-name → SPDX id",
    )?;
    let mut out = BTreeMap::new();
    for (name, value) in table {
        let id = value.as_str().with_context(|| {
            format!("[package.metadata.slint-license.allow.{name}] must be a string SPDX id")
        })?;
        out.insert(name.clone(), id.to_string());
    }
    Ok(out)
}

/// Whether a package is a proc-macro crate (compiled for the host and not
/// linked into the program, hence a build-time dependency).
fn is_proc_macro(pkg: &cargo_metadata::Package) -> bool {
    pkg.targets.iter().any(|t| t.is_proc_macro())
}

/// Whether a `cfg(...)` target predicate references a synthetic build flag
/// (`fuzzing`, `test`, `miri`, `clippy`) that is never set for a real target
/// platform in a normal build. Quoted values (e.g. `target_os = "..."`) are
/// ignored so only bare cfg flags are matched.
fn is_synthetic_cfg(target: &str) -> bool {
    let mut unquoted = String::with_capacity(target.len());
    let mut in_quote = false;
    for c in target.chars() {
        match c {
            '"' => in_quote = !in_quote,
            _ if !in_quote => unquoted.push(c),
            _ => {}
        }
    }
    unquoted
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|tok| matches!(tok, "fuzzing" | "test" | "miri" | "clippy"))
}

/// Whether `parent` actually pulls in the dependency crate named `dep_name`
/// given its resolved set of `active` features. A non-optional (non-dev)
/// dependency is always pulled in; an optional one only if some active feature
/// activates it (`name` / `dep:name` / `name/feature`, but not the weak
/// `name?/feature`).
fn dependency_enabled(
    parent: &cargo_metadata::Package,
    active: &HashSet<&str>,
    dep_name: &str,
) -> bool {
    let mut matched = false;
    for dep in &parent.dependencies {
        if dep.kind == cargo_metadata::DependencyKind::Development || dep.name != dep_name {
            continue;
        }
        matched = true;
        if !dep.optional {
            return true;
        }
        // The feature gate uses the dependency's local name (its rename if any).
        let key = dep.rename.as_deref().unwrap_or(&dep.name);
        if active.contains(key) {
            return true;
        }
        let dep_gate = format!("dep:{key}");
        let feature_prefix = format!("{key}/");
        let enables = |value: &str| value == dep_gate || value.starts_with(&feature_prefix);
        if active
            .iter()
            .any(|f| parent.features.get(*f).is_some_and(|v| v.iter().any(|x| enables(x))))
        {
            return true;
        }
    }
    // If no matching manifest entry was found (unusual), keep the dependency.
    !matched
}

/// Fallback for crates that declare only `license-file = "..."` (no SPDX
/// `license` field): read that file and recognize the canonical text of one of
/// the licenses in `ACCEPTED`, plus the copyright holder(s) from its
/// `Copyright …` lines. Bails when either cannot be identified.
fn detect_from_license_file(
    pkg: &cargo_metadata::Package,
) -> anyhow::Result<(&'static str, String)> {
    let path = pkg.license_file().context("no license-file field either")?;
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Cannot read license file {path}"))?;
    let normalized = text.to_ascii_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    // Distinctive phrases per license. All required phrases must be present.
    const FINGERPRINTS: &[(&str, &[&str])] = &[
        (
            "MIT",
            &["permission is hereby granted, free of charge", "the software is provided \"as is\""],
        ),
        ("Apache-2.0", &["apache license", "version 2.0", "licensed under the apache license"]),
        ("BSL-1.0", &["boost software license", "version 1.0"]),
        ("ISC", &["permission to use, copy, modify, and/or distribute this software"]),
        ("Zlib", &["this software is provided 'as-is', without any express or implied warranty"]),
        ("Unlicense", &["this is free and unencumbered software released into the public domain"]),
        ("CC0-1.0", &["creative commons cc0 1.0 universal"]),
        ("LGPL-3.0-or-later", &["gnu lesser general public license", "version 3, 29 june 2007"]),
    ];
    let id = FINGERPRINTS
        .iter()
        .find(|(_, needles)| needles.iter().all(|n| normalized.contains(n)))
        .map(|(id, _)| *id)
        .with_context(|| format!("Cannot recognize the contents of {path} as a known license"))?;
    let copyright = copyright_holders([&text])
        .with_context(|| format!("Cannot find a copyright holder in {path}"))?;
    Ok((id, copyright))
}

/// Fallback author for crates whose manifest carries no `authors`: salvage
/// the copyright holder(s) from the license files shipped in the crate's
/// package (`LICENSE*` / `COPYING*` / `NOTICE*` next to its `Cargo.toml`).
/// Returns `None` when no such file yields an attribution line.
fn authors_from_license_files(pkg: &cargo_metadata::Package) -> Option<String> {
    let dir = pkg.manifest_path.parent()?;
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path.file_name().and_then(|n| n.to_str()).is_some_and(|name| {
                    let name = name.to_ascii_lowercase();
                    ["license", "licence", "copying", "notice"]
                        .iter()
                        .any(|prefix| name.starts_with(prefix))
                })
        })
        .collect();
    paths.sort();
    // Unreadable (e.g. non-UTF-8) files are skipped.
    copyright_holders(paths.iter().filter_map(|path| std::fs::read_to_string(path).ok()))
}

/// Salvage any `Copyright …` lines from the given license texts. The leading
/// `Copyright`, year and surrounding punctuation are stripped, leaving just
/// the rights holders. Matches are deduplicated and joined with `, `; returns
/// `None` when no copyright line is found.
fn copyright_holders<T: AsRef<str>>(texts: impl IntoIterator<Item = T>) -> Option<String> {
    let mut holders: Vec<String> = Vec::new();
    for text in texts {
        for raw in text.as_ref().lines() {
            let line = raw.trim().trim_end_matches('.');
            let Some(rest) =
                line.get(..9).filter(|s| s.eq_ignore_ascii_case("Copyright")).map(|_| &line[9..])
            else {
                continue;
            };
            // Only lines where the keyword is followed by a `(c)`/`©` marker
            // or a year carry an attribution. This skips license prose that
            // merely mentions a "copyright license ..." as well as the
            // Apache-2.0 appendix template
            // `Copyright [yyyy] [name of copyright owner]`.
            let rest = rest.trim_start();
            if !rest.starts_with(['(', '©']) && !rest.starts_with(|c: char| c.is_ascii_digit()) {
                continue;
            }
            // Strip an optional `(c)` / `©` marker first (only here may
            // `c`/`C` be consumed), then a leading year or year-range and its
            // separators.
            let rest = rest
                .trim_start_matches(|c: char| "()cC©".contains(c))
                .trim_start_matches(|c: char| {
                    c.is_ascii_digit() || c.is_ascii_whitespace() || ",-".contains(c)
                })
                .trim();
            // License files wrap their lines, which can leave a holder-list
            // entry ending in a dangling conjunction ("… Sun Microsystems
            // or"). Drop it.
            let rest = rest
                .strip_suffix(" or")
                .or_else(|| rest.strip_suffix(" and"))
                .unwrap_or(rest)
                .trim();
            if !rest.is_empty() && !holders.iter().any(|h| h == rest) {
                holders.push(rest.to_string());
            }
        }
    }
    (!holders.is_empty()).then(|| holders.join(", "))
}

/// The canonical SPDX string for a requirement's license, e.g. `MIT`,
/// `LGPL-3.0-or-later` (spdx renders the `-or-later`/`+` modifier) or
/// `LicenseRef-Slint-Software-3.0`. The `WITH <exception>` clause is
/// intentionally omitted, as licenses are keyed by id alone.
fn license_string(req: &spdx::LicenseReq) -> String {
    req.license.to_string()
}

/// The human-readable name for a requirement's license, e.g. `MIT License`,
/// falling back to the SPDX id for `LicenseRef-` licenses with no entry.
fn license_full_name(req: &spdx::LicenseReq) -> String {
    let id = license_string(req);
    spdx::license_id(&id).map(|l| l.full_name.to_string()).unwrap_or(id)
}

/// The crates.io page for the exact version of a crate.
fn crate_url(name: &str, version: &str) -> String {
    format!("https://crates.io/crates/{name}/{version}")
}

/// Escape a value for use inside a Markdown table cell: `|` would otherwise end
/// the cell and newlines would break the row.
fn table_cell(value: &str) -> String {
    value.replace('\\', "\\\\").replace('|', "\\|").replace(['\n', '\r'], " ")
}

/// Render the dependency table followed by the license texts as Markdown. The
/// output carries no front matter so it can be shipped verbatim in the binary
/// packages; the docs site generators (`docs/common/src/utils/thirdparty.ts`)
/// prepend the Starlight front matter they need.
fn render_markdown(rows: &[CrateRow], sections: &[LicenseSection]) -> String {
    let mut out = String::new();
    let ids: HashSet<&str> = sections.iter().map(|s| s.id.as_str()).collect();

    out.push_str("## Dependencies\n\n");
    out.push_str("Third-party crates linked into the program and distributed with it.\n\n");
    let mut selected: Vec<&CrateRow> = rows.iter().collect();
    selected.sort_by(|a, b| a.name.cmp(&b.name).then(a.version.cmp(&b.version)));
    if selected.is_empty() {
        out.push_str("_None._\n\n");
    } else {
        out.push_str("| Name | Author / Copyright | License |\n| --- | --- | --- |\n");
        for r in selected {
            let author = if r.author.is_empty() { "—" } else { r.author.as_str() };
            out.push_str(&format!(
                "| [{name} {version}]({url}) | {author} | {license} |\n",
                name = table_cell(&r.name),
                version = table_cell(&r.version),
                url = crate_url(&r.name, &r.version),
                author = table_cell(author),
                license = link_license_ids(&r.license, &ids),
            ));
        }
        out.push('\n');
    }

    out.push_str("## License texts\n\n");
    for s in sections {
        out.push_str(&format!("### <a id=\"{id}\"></a> {name}\n\n", id = s.id, name = s.name));
        out.push_str(&format!("```\n{}\n```\n\n", s.text.trim_end()));
    }
    out
}

/// Render the dependencies and license texts as JSON for the Slint Viewer's
/// in-app attribution page:
/// `{ "crates": [{ name, version, author, license }], "licenses": [{ id, name, text }] }`.
/// `rows` and `sections` are expected pre-sorted by the caller.
fn render_json(rows: &[CrateRow], sections: &[LicenseSection]) -> String {
    let mut out = serde_json::to_string_pretty(&serde_json::json!({
        "crates": rows,
        "licenses": sections,
    }))
    // In-memory serialization of string-only data cannot fail.
    .expect("serializing license data to JSON");
    out.push('\n');
    out
}

/// Write an iOS `Settings.bundle` at `out_dir` so the licenses appear in the
/// system Settings app under the application. The root has a single
/// "Third-Party Licenses" child pane (`Licenses.plist`) listing every crate;
/// each crate links to its own page (`crate_NNNN.plist`) showing the author
/// and the text of each license the crate uses.
///
/// `PSChildPaneSpecifier.File` references a plist by name at the bundle root
/// (no subdirectory, no extension), so every page is a flat top-level file.
fn render_ios_settings_bundle(
    rows: &[CrateRow],
    sections: &[LicenseSection],
    out_dir: &Path,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("Cannot create {}", out_dir.display()))?;

    // Root: a header group plus the child pane holding the crate list.
    let mut root = String::new();
    plist_specifier(&mut root, &[("Type", "PSGroupSpecifier"), ("Title", "Slint Viewer")]);
    plist_specifier(
        &mut root,
        &[
            ("Type", "PSChildPaneSpecifier"),
            ("Title", "Third-Party Licenses"),
            ("File", "Licenses"),
        ],
    );
    write_plist(&out_dir.join("Root.plist"), &root)?;

    // For each crate: a child pane in the list (its `Title` becomes the pushed
    // page's navigation-bar title) and the page itself, holding the author and
    // the full text of each license the crate uses.
    let mut list = String::new();
    for (i, row) in rows.iter().enumerate() {
        let page_file = format!("crate_{i:04}");
        plist_specifier(
            &mut list,
            &[
                ("Type", "PSChildPaneSpecifier"),
                ("Title", &format!("{} {}", row.name, row.version)),
                ("File", &page_file),
            ],
        );

        let mut page = String::new();
        if !row.author.is_empty() {
            plist_specifier(
                &mut page,
                &[
                    ("Type", "PSGroupSpecifier"),
                    ("Title", "Copyright"),
                    ("FooterText", &row.author),
                ],
            );
        }
        // The license ids the crate's SPDX expression references that have a
        // body; iterate `sections` so they come out in the same sorted order.
        let crate_ids: BTreeSet<String> =
            match spdx::Expression::parse_mode(&row.license, spdx::ParseMode::LAX) {
                Ok(expr) => expr.requirements().map(|r| license_string(&r.req)).collect(),
                Err(_) => BTreeSet::new(),
            };
        for section in sections.iter().filter(|s| crate_ids.contains(&s.id)) {
            plist_specifier(
                &mut page,
                &[
                    ("Type", "PSGroupSpecifier"),
                    ("Title", &section.name),
                    ("FooterText", &section.text),
                ],
            );
        }
        write_plist(&out_dir.join(format!("{page_file}.plist")), &page)?;
    }
    write_plist(&out_dir.join("Licenses.plist"), &list)?;

    Ok(())
}

/// Append one preference-specifier `<dict>` of string key/value pairs to a
/// plist's `PreferenceSpecifiers` array.
fn plist_specifier(out: &mut String, pairs: &[(&str, &str)]) {
    out.push_str("    <dict>\n");
    for (key, value) in pairs {
        out.push_str(&format!(
            "      <key>{}</key>\n      <string>{}</string>\n",
            xml_escape(key),
            xml_escape(value)
        ));
    }
    out.push_str("    </dict>\n");
}

/// Wrap rendered preference specifiers in a complete XML plist and write it.
fn write_plist(path: &Path, specifiers: &str) -> anyhow::Result<()> {
    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n  <key>PreferenceSpecifiers</key>\n  <array>\n{specifiers}  </array>\n</dict>\n</plist>\n"
    );
    std::fs::write(path, document).with_context(|| format!("Cannot write {}", path.display()))
}

/// Escape text for an XML plist body (`<string>`/`<key>` content).
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Turn each known SPDX license id in `expression` into a link to its text
/// section, leaving operators (`OR`/`AND`/`WITH`), parentheses and unknown
/// words untouched. SPDX expressions contain no `|`, so no cell escaping is
/// needed.
fn link_license_ids(expression: &str, ids: &HashSet<&str>) -> String {
    let mut out = String::new();
    let mut word = String::new();
    for c in expression.chars() {
        if c.is_alphanumeric() || matches!(c, '.' | '-' | '+') {
            word.push(c);
        } else {
            push_license_word(&mut out, &word, ids);
            word.clear();
            out.push(c);
        }
    }
    push_license_word(&mut out, &word, ids);
    out
}

/// Append one whitespace/parenthesis-delimited word of a license expression to
/// `out`, linking it to its text section when it is a known license id and
/// emitting it verbatim otherwise (operators such as `OR`/`AND`/`WITH`).
fn push_license_word(out: &mut String, word: &str, ids: &HashSet<&str>) {
    if word.is_empty() {
        return;
    }
    if ids.contains(word) {
        out.push_str(&format!("[{word}](#{word})"));
    } else {
        out.push_str(word);
    }
}
