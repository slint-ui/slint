// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Cargo application target resolution.

use crate::project::{CargoApplicationMetadata, ProjectRunTarget};
use cargo_metadata::{Metadata, Package, Target};
use std::path::PathBuf;

/// A runnable Cargo binary selected for a Springboard project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCargoApplication {
    pub manifest_path: PathBuf,
    pub package: String,
    pub binary: String,
    pub features: Vec<String>,
    pub live_preview_feature: String,
}

/// An error while selecting a Cargo application target.
#[derive(Debug)]
pub struct CargoApplicationError {
    message: String,
}

impl CargoApplicationError {
    fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl std::fmt::Display for CargoApplicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for CargoApplicationError {}

/// Resolve the desktop Cargo binary configured for a project.
///
/// Returns `Ok(None)` when the project has no `[app]` table and no `Cargo.toml` at its root.
pub fn resolve_cargo_application(
    project: &ProjectRunTarget,
) -> Result<Option<ResolvedCargoApplication>, CargoApplicationError> {
    let (manifest_path, configured) = match &project.app {
        Some(configured) => (configured.manifest_path.clone(), Some(configured)),
        None => {
            let manifest = project.project_root.join("Cargo.toml");
            if !manifest.is_file() {
                return Ok(None);
            }
            (
                std::fs::canonicalize(&manifest).map_err(|error| {
                    CargoApplicationError::new(format!(
                        "Failed to resolve Cargo manifest {}: {error}",
                        manifest.display()
                    ))
                })?,
                None,
            )
        }
    };

    let mut command = cargo_metadata::MetadataCommand::new();
    command.manifest_path(&manifest_path).current_dir(&project.project_root).no_deps();
    let metadata = command.exec().map_err(|error| {
        CargoApplicationError::new(format!(
            "Failed to read Cargo metadata from {}: {error}",
            manifest_path.display()
        ))
    })?;
    resolve_from_metadata(manifest_path, configured, &metadata)
}

fn resolve_from_metadata(
    manifest_path: PathBuf,
    configured: Option<&CargoApplicationMetadata>,
    metadata: &Metadata,
) -> Result<Option<ResolvedCargoApplication>, CargoApplicationError> {
    let packages = metadata.workspace_packages();
    let configured_package = configured.and_then(|app| app.package.as_deref());
    let configured_binary = configured.and_then(|app| app.binary.as_deref());

    let selection = if let Some(package_name) = configured_package {
        let package =
            packages.iter().copied().find(|package| package.name == package_name).ok_or_else(
                || {
                    CargoApplicationError::new(format!(
                        "Cargo workspace does not contain configured package `{package_name}`"
                    ))
                },
            )?;
        select_from_package(package, configured_binary)?.map(|binary| (package, binary))
    } else if let Some(binary_name) = configured_binary {
        let matches = packages
            .iter()
            .flat_map(|package| {
                package
                    .targets
                    .iter()
                    .filter(move |target| target.is_bin() && target.name == binary_name)
                    .map(move |target| (*package, target))
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {
                return Err(CargoApplicationError::new(format!(
                    "Cargo workspace does not contain configured binary `{binary_name}`"
                )));
            }
            [selection] => Some(*selection),
            _ => return Err(ambiguous_error(&matches)),
        }
    } else {
        let default_runs = packages
            .iter()
            .filter_map(|package| {
                let default_run = package.default_run.as_deref()?;
                package
                    .targets
                    .iter()
                    .find(|target| target.is_bin() && target.name == default_run)
                    .map(|target| (*package, target))
            })
            .collect::<Vec<_>>();
        match default_runs.as_slice() {
            [selection] => Some(*selection),
            [] => {
                let binaries = all_binaries(&packages);
                match binaries.as_slice() {
                    [] if configured.is_none() => return Ok(None),
                    [] => {
                        return Err(CargoApplicationError::new(
                            "The configured Cargo workspace has no runnable binary",
                        ));
                    }
                    [selection] => Some(*selection),
                    _ => return Err(ambiguous_error(&binaries)),
                }
            }
            _ => return Err(ambiguous_error(&default_runs)),
        }
    };

    let Some((package, binary)) = selection else {
        return if configured.is_some() {
            Err(CargoApplicationError::new("The configured Cargo package has no runnable binary"))
        } else {
            Ok(None)
        };
    };
    Ok(Some(ResolvedCargoApplication {
        manifest_path,
        package: package.name.to_string(),
        binary: binary.name.clone(),
        features: configured.map(|app| app.features.clone()).unwrap_or_default(),
        live_preview_feature: configured
            .map(|app| app.live_preview_feature.clone())
            .unwrap_or_else(|| "slint/live-preview".into()),
    }))
}

fn select_from_package<'a>(
    package: &'a Package,
    configured_binary: Option<&str>,
) -> Result<Option<&'a Target>, CargoApplicationError> {
    if let Some(binary_name) = configured_binary {
        return package
            .targets
            .iter()
            .find(|target| target.is_bin() && target.name == binary_name)
            .map(Some)
            .ok_or_else(|| {
                CargoApplicationError::new(format!(
                    "Cargo package `{}` does not contain configured binary `{binary_name}`",
                    package.name
                ))
            });
    }
    if let Some(default_run) = package.default_run.as_deref()
        && let Some(target) =
            package.targets.iter().find(|target| target.is_bin() && target.name == default_run)
    {
        return Ok(Some(target));
    }
    let binaries = package.targets.iter().filter(|target| target.is_bin()).collect::<Vec<_>>();
    match binaries.as_slice() {
        [] => Ok(None),
        [binary] => Ok(Some(*binary)),
        _ => Err(CargoApplicationError::new(format!(
            "Cargo package `{}` has multiple binaries ({}); set `app.binary` in slint.toml",
            package.name,
            binaries.iter().map(|target| target.name.as_str()).collect::<Vec<_>>().join(", ")
        ))),
    }
}

fn all_binaries<'a>(packages: &[&'a Package]) -> Vec<(&'a Package, &'a Target)> {
    packages
        .iter()
        .flat_map(|package| {
            package.targets.iter().filter(|target| target.is_bin()).map(|target| (*package, target))
        })
        .collect()
}

fn ambiguous_error(candidates: &[(&Package, &Target)]) -> CargoApplicationError {
    let candidates = candidates
        .iter()
        .map(|(package, binary)| format!("{}:{}", package.name, binary.name))
        .collect::<Vec<_>>()
        .join(", ");
    CargoApplicationError::new(format!(
        "Cargo workspace has multiple runnable binaries ({candidates}); set `app.package` and `app.binary` in slint.toml"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{PROJECT_MANIFEST_FILE, load_project_run_target};
    use std::path::Path;

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn project(directory: &tempfile::TempDir, app: &str) -> ProjectRunTarget {
        write(&directory.path().join("app.slint"), "export component App inherits Window {}\n");
        write(
            &directory.path().join(PROJECT_MANIFEST_FILE),
            &format!("entry = \"app.slint\"\ncomponent = \"App\"\n{app}"),
        );
        load_project_run_target(directory.path()).unwrap().unwrap()
    }

    fn package(directory: &Path, name: &str, extra: &str, binaries: &[&str]) {
        write(
            &directory.join("Cargo.toml"),
            &format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n{extra}"
            ),
        );
        for binary in binaries {
            write(&directory.join(format!("src/bin/{binary}.rs")), "fn main() {}\n");
        }
    }

    #[test]
    fn detects_a_single_binary_without_app_metadata() {
        let directory = tempfile::tempdir().unwrap();
        package(directory.path(), "demo", "", &["demo"]);
        let target = project(&directory, "");

        let resolved = resolve_cargo_application(&target).unwrap().unwrap();

        assert_eq!(resolved.package, "demo");
        assert_eq!(resolved.binary, "demo");
        assert_eq!(resolved.live_preview_feature, "slint/live-preview");
    }

    #[test]
    fn default_run_selects_one_of_multiple_binaries() {
        let directory = tempfile::tempdir().unwrap();
        package(directory.path(), "demo", "default-run = \"second\"\n", &["first", "second"]);
        let target = project(&directory, "");

        assert_eq!(resolve_cargo_application(&target).unwrap().unwrap().binary, "second");
    }

    #[test]
    fn explicit_metadata_resolves_a_workspace_package_and_renamed_feature() {
        let directory = tempfile::tempdir().unwrap();
        write(
            &directory.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"first\", \"second\"]\nresolver = \"3\"\n",
        );
        package(&directory.path().join("first"), "first", "", &["first"]);
        package(&directory.path().join("second"), "second", "", &["runner", "tool"]);
        let target = project(
            &directory,
            "\n[app]\nkind = \"cargo\"\nmanifest = \"Cargo.toml\"\npackage = \"second\"\nbinary = \"runner\"\nfeatures = [\"logging\"]\nlive-preview-feature = \"preview-ui\"\n",
        );

        let resolved = resolve_cargo_application(&target).unwrap().unwrap();

        assert_eq!(resolved.package, "second");
        assert_eq!(resolved.binary, "runner");
        assert_eq!(resolved.features, ["logging"]);
        assert_eq!(resolved.live_preview_feature, "preview-ui");
    }

    #[test]
    fn ambiguous_workspaces_require_an_explicit_selection() {
        let directory = tempfile::tempdir().unwrap();
        write(
            &directory.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"first\", \"second\"]\nresolver = \"3\"\n",
        );
        package(&directory.path().join("first"), "first", "", &["first"]);
        package(&directory.path().join("second"), "second", "", &["second"]);
        let target = project(&directory, "");

        let error = resolve_cargo_application(&target).unwrap_err().to_string();

        assert!(error.contains("multiple runnable binaries"));
        assert!(error.contains("app.package"));
    }

    #[test]
    fn a_relative_manifest_is_resolved_from_the_project_root() {
        let directory = tempfile::tempdir().unwrap();
        package(&directory.path().join("rust"), "demo", "", &["demo"]);
        let target =
            project(&directory, "\n[app]\nkind = \"cargo\"\nmanifest = \"rust/Cargo.toml\"\n");

        let resolved = resolve_cargo_application(&target).unwrap().unwrap();

        assert_eq!(
            resolved.manifest_path,
            std::fs::canonicalize(directory.path().join("rust/Cargo.toml")).unwrap()
        );
    }

    #[test]
    fn projects_without_a_cargo_manifest_have_no_rust_application() {
        let directory = tempfile::tempdir().unwrap();
        let target = project(&directory, "");

        assert_eq!(resolve_cargo_application(&target).unwrap(), None);
    }
}
