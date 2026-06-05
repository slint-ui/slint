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
    pub output: Option<PathBuf>,
}

pub fn generate(args: &GenerateArgs) -> anyhow::Result<()> {
    let accepted: HashSet<&str> = ACCEPTED.iter().copied().collect();

    fetch_dependencies(&args.manifest_path)?;

    let packages = resolve_packages(&args.manifest_path, &args.features)?;

    // Build one summary row per crate for the table, and collect every accepted
    // license id that appears so each one gets a body below (the table links to
    // them). Per-crate attribution lives in the table's Author / Copyright
    // column, so the texts need only one body per license.
    let mut license_names: BTreeMap<String, String> = BTreeMap::new();
    let mut used_ids: BTreeSet<String> = BTreeSet::new();
    let mut rows: Vec<CrateRow> = Vec::new();

    for pkg in &packages {
        let Some(expression) = pkg.license.clone() else {
            bail!("Crate {} {} has no license information", pkg.name, pkg.version);
        };

        // Lax parsing accepts the deprecated `/` OR-separator and imprecise
        // identifiers (e.g. `apache2`) still found in older crates.
        let expr =
            spdx::Expression::parse_mode(&expression, spdx::ParseMode::LAX).with_context(|| {
                format!("Cannot parse license `{expression}` of {} {}", pkg.name, pkg.version)
            })?;

        if !expr.evaluate(|req| accepted.contains(license_string(req).as_str())) {
            bail!(
                "License `{expression}` of crate {} {} is not in the accepted list",
                pkg.name,
                pkg.version
            );
        }

        // Record the accepted license ids the crate uses, so each gets a body.
        for req in expr.requirements() {
            let id = license_string(&req.req);
            if !accepted.contains(id.as_str()) {
                continue;
            }
            license_names.entry(id.clone()).or_insert_with(|| license_full_name(&req.req));
            used_ids.insert(id);
        }

        rows.push(CrateRow {
            name: pkg.name.to_string(),
            version: pkg.version.to_string(),
            author: pkg.authors.join(", "),
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

    let rendered = render_markdown(&rows, &sections);

    match &args.output {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(path, rendered)
                .with_context(|| format!("Cannot write {}", path.display()))?;
        }
        None => print!("{rendered}"),
    }

    Ok(())
}

/// One crate's row in the dependency table.
struct CrateRow {
    name: String,
    version: String,
    author: String,
    license: String,
}

/// The canonical license body shown once under one license id.
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
) -> anyhow::Result<Vec<cargo_metadata::Package>> {
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
        let active: HashSet<&str> = node.features.iter().map(String::as_str).collect();
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
    Ok(wanted.into_iter().filter_map(|id| packages.remove(&id)).collect())
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

/// The canonical SPDX string for a requirement, e.g. `MIT` or
/// `LicenseRef-Slint-Software-3.0`.
fn license_string(req: &spdx::LicenseReq) -> String {
    match &req.license {
        spdx::LicenseItem::Spdx { id, .. } => id.name.to_string(),
        spdx::LicenseItem::Other { doc_ref, lic_ref } => match doc_ref {
            Some(doc_ref) => format!("DocumentRef-{doc_ref}:LicenseRef-{lic_ref}"),
            None => format!("LicenseRef-{lic_ref}"),
        },
    }
}

fn license_full_name(req: &spdx::LicenseReq) -> String {
    match &req.license {
        spdx::LicenseItem::Spdx { id, .. } => id.full_name.to_string(),
        spdx::LicenseItem::Other { .. } => license_string(req),
    }
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
