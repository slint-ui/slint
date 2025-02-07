// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use anyhow::{Context, Result};

use xshell::{Cmd, Shell};

use std::collections::BTreeMap;
use std::{ffi::OsStr, path::Path, path::PathBuf};

fn cmd<I>(sh: &Shell, command: impl AsRef<Path>, args: I) -> Result<Cmd<'_>>
where
    I: IntoIterator,
    I::Item: AsRef<OsStr>,
{
    let home_dir = std::env::var("HOME").context("HOME is not set in the environment")?;
    Ok(sh.cmd(command).args(args).env("PATH", &format!("/bin:/usr/bin:{home_dir}/.local/bin")))
}

pub fn find_reuse() -> Result<PathBuf> {
    which::which("reuse").context("Failed to find reuse")
}

pub fn reuse_download(sh: &Shell, reuse: &Path) -> Result<String> {
    Ok(cmd(sh, reuse, &["download", "--all"])?
        .read()
        .context("Failed to download missing licenses.")?)
}

pub fn reuse_lint(sh: &Shell, reuse: &Path) -> Result<()> {
    let output = cmd(sh, reuse, &["lint"])?.ignore_status().output()?;

    if !output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        println!("{}", &stdout);
        anyhow::bail!("Project is not reuse compliant!");
    }
    Ok(())
}

fn parse_spdx_data(sh: &Shell, reuse: &Path) -> Result<BTreeMap<PathBuf, Vec<String>>> {
    let output = cmd(sh, reuse, &["spdx"])?.read()?;

    let mut current_filename = String::new();
    let mut licenses = Vec::new();
    let mut result = BTreeMap::new();

    fn insert(v: &mut BTreeMap<PathBuf, Vec<String>>, f: &str, l: Vec<String>) -> Result<()> {
        if l.is_empty() {
            anyhow::bail!("No license info for \"{}\" available", f);
        }
        v.insert(PathBuf::from(f), l);
        Ok(())
    }

    for line in output.lines() {
        if line.starts_with("FileName: ") {
            if !current_filename.is_empty() {
                insert(&mut result, &current_filename, licenses)?;
                licenses = Vec::new();
            }

            current_filename = line[10..].into();
        } else if line.starts_with("LicenseInfoInFile: ") {
            let license = line[19..].into();
            licenses.push(license);
        }
    }

    if !current_filename.is_empty() {
        insert(&mut result, &current_filename, licenses)?;
    }

    Ok(result)
}

fn find_licenses_directories(dir: &Path) -> Result<Vec<PathBuf>> {
    assert!(dir.is_dir());

    let mut result = Vec::new();

    let licenses_name: Option<&OsStr> = Some(OsStr::new("LICENSES"));
    let dot_name: &OsStr = OsStr::new(".");

    for d in std::fs::read_dir(dir)?
        .filter(|d| d.as_ref().is_ok_and(|e| e.file_type().is_ok_and(|f| f.is_dir())))
    {
        let path = d?.path();
        let parent_path = path.parent().expect("This is a subdirectory, so it must have a parent!");
        if path.file_name() == licenses_name && parent_path != dot_name {
            let parent_path = parent_path.to_owned();
            result.push(parent_path);
        } else {
            result.append(&mut find_licenses_directories(&path)?)
        }
    }

    result.sort();

    Ok(result)
}

fn populate_license_map(
    license_map: &mut BTreeMap<PathBuf, Vec<String>>,
    file_map: BTreeMap<PathBuf, Vec<String>>,
) {
    // longer names are sorted after shorter, so look from the back.
    //
    // FIXME: This is rather inefficient! Hope it is OK for the use case at hand.
    for (file, file_lic) in file_map.iter().rev() {
        for (dir, dir_lic) in license_map.iter_mut().rev() {
            if file.starts_with(dir) {
                // There should not be more than maybe 5 or so licenses applicable to any
                // directory, so this is probably OK
                for l in file_lic {
                    if !dir_lic.contains(l) {
                        dir_lic.push(l.clone());
                    }
                }
                break;
            }
        }
    }
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

fn validate_license_directory(dir: &Path, licenses: &[String], fix_it: bool) -> Result<()> {
    let top_dir =
        PathBuf::from(".").canonicalize().context("Failed to canonicalize the top directory")?;
    let lic_dir = dir.join("LICENSES").canonicalize().with_context(|| {
        format!("Failed to canonicalize \"{}\"", dir.join("LICENSES").to_string_lossy())
    })?;

    if !lic_dir.is_dir() {
        anyhow::bail!("\"{}\" is not a directory", lic_dir.to_string_lossy());
    }

    let mut linked_licenses = Vec::new();
    let mut to_add = Vec::new();
    let mut to_remove = Vec::new();

    for d in lic_dir
        .read_dir()
        .with_context(|| format!("Failed to read \"{}\" directory", lic_dir.to_string_lossy()))?
    {
        let child = d
            .with_context(|| {
                format!("Failed to read in LICENSES directory \"{}\"", lic_dir.to_string_lossy())
            })?
            .path();

        if !is_symlink(&child) {
            if child.is_file() && fix_it {
                to_remove.push(child.clone());
                continue;
            } else {
                anyhow::bail!("\"{}\" is a not a symlink!", child.to_string_lossy());
            }
        }

        let ext = child.extension().unwrap_or_default();
        let file_stem = child.file_stem().unwrap_or_default().to_string_lossy().to_string();

        if ext.is_empty() || (ext != "txt" && ext != "md" && ext != "html") {
            anyhow::bail!("Invalid extension for LICENSE symlink \"{}\"", child.to_string_lossy());
        }

        if file_stem.is_empty() || !licenses.contains(&file_stem) {
            if !fix_it {
                anyhow::bail!("LICENSE symlink \"{}\" is not necessary", child.to_string_lossy());
            } else {
                to_remove.push(child.clone());
                continue;
            }
        } else {
            linked_licenses.push(file_stem.clone());
        }

        let link_target = std::fs::read_link(&child).with_context(|| {
            format!("Could not extract link target of \"{}\"", child.to_string_lossy())
        })?;
        let link_target_file_stem = link_target.file_stem().unwrap_or_default().to_string_lossy();
        let link_target_extension = link_target.extension().unwrap_or_default();

        let validated_link_target = lic_dir.join(&link_target).canonicalize().unwrap_or_default();
        if validated_link_target.as_os_str().is_empty() {
            if !fix_it {
                anyhow::bail!(
                    "License symlink \"{}\" does not point to any existing location",
                    child.to_string_lossy()
                );
            }
            // Path validation failed
            to_remove.push(child.clone());
            to_add.push(file_stem.clone());
            continue;
        }
        if link_target_extension != ext || link_target_file_stem != file_stem {
            if !fix_it {
                anyhow::bail!(
                    "LICENSE symlink \"{}\" renames the license.",
                    child.to_string_lossy()
                );
            } else {
                to_remove.push(child.clone());
                to_add.push(file_stem.clone());
                continue;
            }
        }

        if !validated_link_target.is_absolute() || !validated_link_target.starts_with(&top_dir) {
            if !fix_it {
                let c = child.to_string_lossy();
                anyhow::bail!("LICENSE symlink \"{}\" points outside the repository", c);
            } else {
                to_remove.push(child.clone());
                to_add.push(file_stem.clone());
                continue;
            }
        }

        if !validated_link_target.starts_with(top_dir.join("LICENSES")) {
            if !fix_it {
                let c = child.to_string_lossy();
                anyhow::bail!(
                    "LICENSE symlink \"{}\" points to a random place in the repository",
                    c
                );
            } else {
                to_remove.push(child.clone());
                to_add.push(file_stem.clone());
                continue;
            }
        }

        if !validated_link_target.is_file() {
            if !fix_it {
                let c = child.to_string_lossy();
                anyhow::bail!("LICENSE symlink \"{}\" does not point to a file", c);
            } else {
                to_remove.push(child.clone());
                to_add.push(file_stem.clone());
                continue;
            }
        }
    }

    if !fix_it {
        return Ok(());
    }

    // Remove old symlinks
    for rm in to_remove {
        println!("Removing symlink \"{}\"...", rm.to_string_lossy());
        std::fs::remove_file(&rm).with_context(|| {
            format!("Failed to remove LICENSE symlink \"{}\"", rm.to_string_lossy())
        })?;
    }

    for l in licenses {
        let l = l.to_string();
        if !linked_licenses.contains(&l) {
            to_add.push(l);
        }
    }

    let mut license_filenames_to_add = Vec::new();

    if !to_add.is_empty() {
        let top_lic = PathBuf::from("LICENSES");
        for l in top_lic.read_dir().context("Failed to read the top level LICENSES directory")? {
            let path =
                l.context("Failed to read an entry in the top level LICENSES directory")?.path();
            if path.is_file() {
                let file_stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                if to_add.contains(&file_stem) {
                    license_filenames_to_add.push(path.file_name().unwrap_or_default().to_owned());
                }
            }
        }
    }
    if license_filenames_to_add.len() != to_add.len() {
        anyhow::bail!("Not all licenses were found in top level LICENSES directory");
    }

    for license_file in license_filenames_to_add {
        // build symlink target path
        let mut target_link_path = PathBuf::new();

        for _ in 0..lic_dir.components().count() - top_dir.components().count() {
            target_link_path = target_link_path.join("..");
        }
        target_link_path = target_link_path.join("LICENSES");
        target_link_path = target_link_path.join(&license_file);

        let source_path = lic_dir.join(&license_file);

        println!(
            "Creating LICENSE symlink {} -> {}",
            &source_path.to_string_lossy(),
            &target_link_path.to_string_lossy()
        );

        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(&target_link_path, &source_path);
        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_file(&target_link_path, &source_path);

        result.with_context(|| {
            format!(
                "Failed to create symlink \"{}\" -> \"{}\"",
                source_path.to_string_lossy(),
                target_link_path.to_string_lossy()
            )
        })?;
    }

    Ok(())
}

pub fn scan_symlinks(sh: &Shell, reuse: &Path, fix_it: bool) -> Result<()> {
    let license_directories = find_licenses_directories(&PathBuf::from("."))
        .context("Failed to scan for directories containing LICENSES subfolders")?;

    if license_directories.is_empty() {
        return Ok(());
    }

    let mut license_map = license_directories
        .iter()
        .map(|p| (p.clone(), Vec::<String>::new()))
        .collect::<BTreeMap<_, _>>();

    let file_data = parse_spdx_data(sh, reuse).context("Failed to parse SPDX project data")?;

    populate_license_map(&mut license_map, file_data);

    for (dir, licenses) in license_map {
        validate_license_directory(&dir, &licenses, fix_it)?;
    }

    Ok(())
}

#[derive(Debug, clap::Parser)]
pub struct ReuseComplianceCheck {
    #[arg(long, action)]
    fix_symlinks: bool,
    #[arg(long, action)]
    download_missing_licenses: bool,
}

impl ReuseComplianceCheck {
    pub fn check_reuse_compliance(&self) -> Result<()> {
        if !std::env::current_dir()
            .context("Can not access current work directory")?
            .join("REUSE.toml")
            .is_file()
        {
            anyhow::bail!("No REUSE.toml file found in current directory");
        }

        let sh = Shell::new()?;

        let reuse = find_reuse().context("Can not find reuse. Please make sure it is installed")?;

        println!("Reuse binary \"{}\".", reuse.to_string_lossy());

        if self.download_missing_licenses {
            let output = reuse_download(&sh, &reuse)?;
            println!("{}", &output);
        }

        reuse_lint(&sh, &reuse)?;

        scan_symlinks(&sh, &reuse, self.fix_symlinks)
    }
}
