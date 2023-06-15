// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use anyhow::Context;
use xshell::{cmd, Shell};

fn cp_r(
    sh: &Shell,
    src: &std::path::Path,
    dst: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if src.is_dir() {
        if !dst.exists() {
            sh.create_dir(&dst).unwrap();
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

pub fn generate() -> Result<(), Box<dyn std::error::Error>> {
    let root = super::root_dir();
    let node_dir = root.join("api").join("node");

    let cargo_toml_path = node_dir.join("native").join("Cargo.toml");

    println!("Removing relative paths from {}", cargo_toml_path.to_string_lossy());

    let sh = Shell::new()?;

    let toml_source =
        sh.read_file(cargo_toml_path.clone()).context("Failed to read Node Cargo.toml")?;

    let mut toml: toml_edit::Document = toml_source.parse().context("Error parsing Cargo.toml")?;

    // Remove all `path = ` entries from dependencies
    for dep_key in ["dependencies", "build-dependencies"].iter() {
        let dep_table = match toml[dep_key].as_table_mut() {
            Some(table) => table,
            _ => continue,
        };
        let deps: Vec<_> = dep_table.iter().map(|(name, _)| name.to_string()).collect();
        deps.iter().for_each(|name| {
            dep_table[name].as_inline_table_mut().map(|dep_config| dep_config.remove("path"));
        });
    }

    let edited_toml = toml.to_string();

    sh.write_file(cargo_toml_path.clone(), edited_toml).context("Error writing Cargo.toml")?;

    println!("Putting LICENSE information into place for the source package");

    sh.copy_file(root.join("LICENSE.md"), node_dir.join("LICENSE.md"))
        .context("Error copying LICENSE.md into the node dir for packaging")?;

    cp_r(&sh, &root.join("LICENSES"), &node_dir.join("LICENSES"))?;

    let package_json_source =
        sh.read_file(&node_dir.join("package.json")).context("Error reading package.json")?;

    let package_json: serde_json::Value = serde_json::from_str(&package_json_source)?;

    let file_name = node_dir.join(format!(
        "{}-{}.tgz",
        package_json["name"].as_str().unwrap(),
        package_json["version"].as_str().unwrap()
    ));

    sh.remove_path(file_name.clone()).context("Error deleting old archive")?;

    println!("Running npm package to create the tarball");

    {
        let _p = sh.push_dir(node_dir.clone());
        cmd!(sh, "npm pack").run()?;
    }

    println!("Reverting Cargo.toml");

    sh.write_file(cargo_toml_path, toml_source).context("Error writing Cargo.toml")?;

    sh.remove_path(node_dir.join("LICENSE.md")).context("Error deleting LICENSE.md copy")?;

    println!("Source package created and located in {}", file_name.to_string_lossy());

    Ok(())
}
