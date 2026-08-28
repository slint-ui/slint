// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Keeps the `[profile.*]` settings of the repository's workspaces in sync.
//!
//! The workspaces share one `target/` directory via `.cargo/config.toml`, and
//! cargo keys its build cache on the profile: any workspace that disagrees
//! rebuilds the common library crates instead of reusing them. Every workspace
//! is checked, so a new one has to either match the root or be listed below.

use anyhow::{Context, Result, anyhow};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Workspaces that deliberately do not share the root's profiles. Each entry
/// must name an existing workspace: a stale path would put the workspace it
/// used to name back under the rule and let `--fix-it` edit it.
const INTENTIONALLY_DIFFERENT: &[&str] = &[
    // Optimizes for size and aborts on panic in dev too.
    "examples/safe-ui/Cargo.toml",
    // Built for embedded targets - see each manifest's header.
    "examples/mcu-board-support/Cargo.toml",
    "examples/mcu-embassy/Cargo.toml",
    "examples/uefi-demo/Cargo.toml",
    // Built by cargo-fuzz, which sets its own profiles.
    "internal/compiler/fuzz/Cargo.toml",
];

/// Profiles a workspace need not declare, because only the root builds them.
/// One that does declare them still has to match.
const OPTIONAL_PROFILES: &[&str] = &["package-release"];

#[derive(Debug, clap::Parser)]
pub struct ProfilesCheck {
    /// Write the root's settings into the workspaces that differ, instead of
    /// only reporting them.
    #[arg(long, action)]
    fix_it: bool,
}

/// A `Cargo.toml` that opens a workspace, parsed once.
struct WorkspaceManifest {
    path: PathBuf,
    relative: String,
    document: toml_edit::DocumentMut,
}

impl ProfilesCheck {
    pub fn check_profiles(&self) -> Result<(), Box<dyn std::error::Error>> {
        let root_manifest = super::root_dir().join("Cargo.toml");
        let root_profiles = flatten_profiles(&read_manifest(&root_manifest)?);
        let mut manifests = workspace_manifests()?;

        // Check this before writing anything: a stale entry would let the
        // loop below edit a manifest that is meant to be left alone.
        let workspaces: BTreeSet<&str> =
            manifests.iter().map(|manifest| manifest.relative.as_str()).collect();
        for exempt in INTENTIONALLY_DIFFERENT {
            if !workspaces.contains(exempt) {
                return Err(anyhow!(
                    "{exempt} is listed in INTENTIONALLY_DIFFERENT in \
                     xtask/src/check_profiles.rs, but is not a workspace in this repository. \
                     Remove the entry, or point it at the workspace's new path."
                )
                .into());
            }
        }

        let mut diverged = false;
        for manifest in &mut manifests {
            if manifest.path == root_manifest
                || INTENTIONALLY_DIFFERENT.contains(&manifest.relative.as_str())
            {
                continue;
            }

            diverged |= self
                .check_manifest(manifest, &root_profiles)
                .with_context(|| format!("checking {}", manifest.relative))?;
        }

        if diverged {
            Err(anyhow!(
                "The profile settings above diverge from the root workspace's Cargo.toml. \
                 Run `cargo xtask check_profiles --fix-it` to adopt the root's values, or add \
                 the workspace to INTENTIONALLY_DIFFERENT in xtask/src/check_profiles.rs if the \
                 divergence is on purpose."
            )
            .into())
        } else {
            println!("All workspace profiles are in sync.");
            Ok(())
        }
    }

    /// Compares one workspace against the root. Returns whether to fail.
    fn check_manifest(
        &self,
        manifest: &mut WorkspaceManifest,
        root_profiles: &BTreeMap<String, String>,
    ) -> Result<bool> {
        let profiles = flatten_profiles(&manifest.document);
        let declared: BTreeSet<&str> = profiles.keys().map(|path| profile_name(path)).collect();
        let relative = &manifest.relative;

        let mut fixed = 0;
        let mut diverged = false;

        for (path, expected) in root_profiles {
            let current = profiles.get(path);
            if current == Some(expected) {
                continue;
            }
            // Only skip an optional profile the workspace leaves out entirely.
            let profile = profile_name(path);
            if !declared.contains(profile) && OPTIONAL_PROFILES.contains(&profile) {
                continue;
            }

            let current = current.map_or("missing", String::as_str);
            eprintln!("  {relative}: profile.{path} is {current}, the root has {expected}");
            diverged = true;

            if self.fix_it {
                set_profile_setting(&mut manifest.document, path, expected)
                    .with_context(|| format!("setting profile.{path}"))?;
                fixed += 1;
            }
        }

        // A setting the root does not have is a deliberate choice, so report
        // it for a human instead of removing it.
        for path in profiles.keys() {
            if !root_profiles.contains_key(path) {
                eprintln!("  {relative}: profile.{path} is not set by the root workspace");
                diverged = true;
            }
        }

        if fixed > 0 {
            std::fs::write(&manifest.path, manifest.document.to_string())
                .with_context(|| format!("writing {relative}"))?;
            eprintln!("  {relative}: updated {fixed} setting(s)");
        }

        // When fixing, this runs in the autofix job, which drops all its other
        // fixes if a step fails. The lint job runs the check and fails there.
        Ok(diverged && !self.fix_it)
    }
}

fn read_manifest(path: &std::path::Path) -> Result<toml_edit::DocumentMut> {
    std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.to_string_lossy()))?
        .parse()
        .with_context(|| format!("parsing {}", path.to_string_lossy()))
}

/// Every `Cargo.toml` that opens a workspace, exempt ones included, so that
/// `INTENTIONALLY_DIFFERENT` can be validated against it.
fn workspace_manifests() -> Result<Vec<WorkspaceManifest>> {
    let mut manifests = Vec::new();

    for path in super::collect_files()? {
        if path.file_name().is_none_or(|name| name != "Cargo.toml") {
            continue;
        }
        let document = read_manifest(&path)?;
        if document.contains_key("workspace") {
            manifests.push(WorkspaceManifest {
                relative: super::repo_relative(&path),
                path,
                document,
            });
        }
    }

    Ok(manifests)
}

/// Flattens `[profile]` into `<profile>.<setting>` paths, so that manifests
/// compare equal however the tables are spelled out.
fn flatten_profiles(document: &toml_edit::DocumentMut) -> BTreeMap<String, String> {
    let mut settings = BTreeMap::new();
    if let Some(profiles) = document.get("profile").and_then(|item| item.as_table_like()) {
        flatten_table(profiles, "", &mut settings);
    }
    settings
}

fn flatten_table(
    table: &dyn toml_edit::TableLike,
    prefix: &str,
    settings: &mut BTreeMap<String, String>,
) {
    for (key, item) in table.iter() {
        let path = if prefix.is_empty() { key.to_string() } else { format!("{prefix}.{key}") };
        match item.as_table_like() {
            Some(nested) => flatten_table(nested, &path, settings),
            None => {
                settings.insert(path, item.to_string().trim().to_string());
            }
        }
    }
}

/// The name of the profile a flattened setting path belongs to.
fn profile_name(path: &str) -> &str {
    path.split_once('.').map_or(path, |(name, _)| name)
}

/// Sets `profile.<path>`, creating the tables in the dotted-header form the
/// manifests use.
fn set_profile_setting(
    document: &mut toml_edit::DocumentMut,
    path: &str,
    value: &str,
) -> Result<()> {
    let value: toml_edit::Value =
        value.parse().with_context(|| format!("parsing value {value}"))?;

    let keys: Vec<&str> = std::iter::once("profile").chain(path.split('.')).collect();
    let (setting, tables) = keys.split_last().expect("the path has at least two keys");

    let mut table: &mut dyn toml_edit::TableLike = document.as_table_mut();
    for key in tables {
        table = table
            .entry(key)
            .or_insert_with(|| {
                let mut table = toml_edit::Table::new();
                table.set_implicit(true);
                toml_edit::Item::Table(table)
            })
            .as_table_like_mut()
            .ok_or_else(|| anyhow!("profile.{path} is not a table"))?;
    }
    table.insert(setting, toml_edit::Item::Value(value));

    Ok(())
}
