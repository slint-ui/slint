// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use anyhow::Context;
use xshell::{Shell, cmd};

#[derive(Debug, clap::Parser)]
pub struct NodePackageOptions {
    #[arg(long, action)]
    pub sha1: Option<String>,
}

fn cp_r(
    sh: &Shell,
    src: &std::path::Path,
    dst: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if src.is_dir() {
        if !dst.exists() {
            sh.create_dir(dst).unwrap();
        } else {
            assert!(dst.is_dir());
        }

        for f in sh.read_dir(src)? {
            let src = src.join(f.file_name().unwrap());
            let dst = dst.join(f.file_name().unwrap());

            cp_r(sh, &src, &dst)?
        }
        Ok(())
    } else {
        sh.copy_file(src, dst).map_err(|e| e.into())
    }
}

pub fn generate(sha1: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = super::root_dir();
    let js_dir = root.join("api").join("js");
    let node_dir = js_dir.join("node");

    // The npm package ships the Rust sources of the slint-js crate, which
    // live one level above the package directory. Copy them in for packing,
    // with a Cargo.toml stripped of workspace inheritance and relative paths.
    let cargo_toml_path = js_dir.join("Cargo.toml");

    println!("Removing relative paths from {}", cargo_toml_path.to_string_lossy());

    let sh = Shell::new()?;

    let workspace_source =
        sh.read_file(root.join("Cargo.toml")).context("Failed to read workspace Cargo.toml")?;
    let workspace_toml: toml_edit::DocumentMut =
        workspace_source.parse().context("Error parsing workspace Cargo.toml")?;

    let workspace_package_fields = workspace_toml
        .get("workspace")
        .and_then(|workspace_table| workspace_table.get("package"))
        .ok_or_else(|| {
            "Could not locate workspace.package table in workspace Cargo.toml".to_string()
        })?;

    let workspace_dependency_fields = workspace_toml
        .get("workspace")
        .and_then(|workspace_table| workspace_table.get("dependencies"))
        .ok_or_else(|| {
            "Could not locate workspace.dependencies table in workspace Cargo.toml".to_string()
        })?;

    let toml_source =
        sh.read_file(cargo_toml_path.clone()).context("Failed to read Node Cargo.toml")?;

    let mut toml: toml_edit::DocumentMut =
        toml_source.parse().context("Error parsing Cargo.toml")?;

    // Replace workspace fields
    let package_table = toml["package"]
        .as_table_mut()
        .ok_or("Error locating [package] table in Node Cargo.toml".to_string())?;
    let keys_for_workspace_replacement = package_table
        .iter()
        .filter_map(|(name, value)| {
            if value
                .as_table()
                .and_then(|entry| entry.get("workspace"))
                .and_then(|maybe_workspace| maybe_workspace.as_bool())
                .unwrap_or(false)
            {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    for key_to_replace in keys_for_workspace_replacement {
        let data = workspace_package_fields
            .get(key_to_replace.to_string())
            .ok_or_else(|| {
                format!(
                    "Could not locate workspace field for {key_to_replace} in workspace toml file"
                )
            })?
            .clone();
        toml["package"][&key_to_replace] = data;
    }

    // Remove all `path = ` entries from dependencies and substitute workspace = true
    let rewrite_dep_table = |dep_table: &mut toml_edit::Table| {
        let deps: Vec<_> = dep_table.iter().map(|(name, _)| name.to_string()).collect();

        deps.iter().for_each(|name| {
            if let Some(dep_config) = dep_table[name].as_inline_table_mut() {
                if name.contains("slint")
                    && let Some(sha1) = &sha1
                {
                    dep_config
                        .insert("git", toml_edit::Value::from("https://github.com/slint-ui/slint"));
                    dep_config.insert("rev", toml_edit::Value::from(sha1));
                }
                if dep_config.remove("workspace").is_some() {
                    let workspace_config = &workspace_dependency_fields[name];
                    if let Some(data) = workspace_config.as_inline_table() {
                        for (k, v) in data.iter() {
                            if k == "features" {
                                // TODO: merge features = []; for now preserve what's in Cargo.toml
                                continue;
                            }
                            dep_config.insert(k, v.clone());
                        }
                    }
                }
                dep_config.remove("path");
            }
        });
    };

    for dep_key in ["dependencies", "build-dependencies"].iter() {
        if let Some(table) = toml[dep_key].as_table_mut() {
            rewrite_dep_table(table);
        }
    }

    // The engine-specific dependencies live in [target.'cfg(...)'.dependencies] tables
    if let Some(target_table) = toml.get_mut("target").and_then(|t| t.as_table_mut()) {
        let targets: Vec<_> = target_table.iter().map(|(name, _)| name.to_string()).collect();
        for target in targets {
            if let Some(table) =
                target_table[&target].get_mut("dependencies").and_then(|d| d.as_table_mut())
            {
                rewrite_dep_table(table);
            }
        }
    }

    let edited_toml = toml.to_string();

    println!("Copying the slint-js crate sources into the package");

    sh.write_file(node_dir.join("Cargo.toml"), edited_toml).context("Error writing Cargo.toml")?;
    sh.copy_file(js_dir.join("build.rs"), node_dir.join("build.rs"))
        .context("Error copying build.rs into the node dir for packaging")?;
    cp_r(&sh, &js_dir.join("src"), &node_dir.join("src"))?;

    println!("Putting LICENSE information into place for the source package");

    sh.copy_file(root.join("LICENSE.md"), node_dir.join("LICENSE.md"))
        .context("Error copying LICENSE.md into the node dir for packaging")?;

    cp_r(&sh, &root.join("LICENSES"), &node_dir.join("LICENSES"))?;

    let package_json_source =
        sh.read_file(node_dir.join("package.json")).context("Error reading package.json")?;

    let package_json: serde_json::Value = serde_json::from_str(&package_json_source)?;

    let file_name = node_dir.join(format!(
        "{}-{}.tgz",
        package_json["name"].as_str().unwrap(),
        package_json["version"].as_str().unwrap()
    ));

    sh.remove_path(file_name.clone()).context("Error deleting old archive")?;

    println!("Running pnpm package to create the tarball");

    {
        let _p = sh.push_dir(node_dir.clone());
        cmd!(sh, "pnpm pack").run()?;
    }

    println!("Removing the copied crate sources");

    sh.remove_path(node_dir.join("Cargo.toml")).context("Error deleting Cargo.toml copy")?;
    sh.remove_path(node_dir.join("build.rs")).context("Error deleting build.rs copy")?;
    sh.remove_path(node_dir.join("src")).context("Error deleting src copy")?;

    sh.remove_path(node_dir.join("LICENSE.md")).context("Error deleting LICENSE.md copy")?;

    println!("Source package created and located in {}", file_name.to_string_lossy());

    Ok(())
}
