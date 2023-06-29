// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

// cspell:ignore slintdocs pipenv pipfile

use anyhow::{Context, Result};
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

fn symlink_file<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> Result<()> {
    if dst.as_ref().exists() {
        std::fs::remove_file(dst.as_ref()).context("Error removing old symlink")?;
    }
    #[cfg(target_os = "windows")]
    return std::os::windows::fs::symlink_file(&src, &dst).context("Error creating symlink");
    #[cfg(not(target_os = "windows"))]
    return std::os::unix::fs::symlink(&src, &dst).context(format!(
        "Error creating symlink from {} to {}",
        src.as_ref().display(),
        dst.as_ref().display()
    ));
}

fn symlink_dir<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> Result<()> {
    if dst.as_ref().exists() {
        std::fs::remove_dir_all(dst.as_ref()).context("Error removing old symlink")?;
    }
    #[cfg(target_os = "windows")]
    return std::os::windows::fs::symlink_dir(&src, &dst).context("Error creating symlink");
    #[cfg(not(target_os = "windows"))]
    return std::os::unix::fs::symlink(&src, &dst).context(format!(
        "Error creating symlink from {} to {}",
        src.as_ref().display(),
        dst.as_ref().display()
    ));
}

fn symlink_files_in_dir<S: AsRef<Path>, T: AsRef<Path>, TS: AsRef<Path>>(
    src: S,
    target: T,
    target_to_source: TS,
) -> Result<()> {
    for entry in std::fs::read_dir(src.as_ref()).context("Error reading docs source directory")? {
        let entry = entry.context("Error reading directory entry")?;
        let path = entry.path();
        let file_name = path.file_name().unwrap();
        let symlink_source = target_to_source.as_ref().to_path_buf().join(&file_name);
        let symlink_target = target.as_ref().to_path_buf().join(path.file_name().unwrap());
        if path.is_file() {
            symlink_file(symlink_source, symlink_target).context("Could not symlink file")?;
        } else if path.is_dir() {
            symlink_dir(symlink_source, symlink_target).context("Could not symlink directory")?;
        }
    }
    Ok(())
}

pub fn generate(show_warnings: bool) -> Result<(), Box<dyn std::error::Error>> {
    generate_enum_docs()?;

    let root = super::root_dir();

    let docs_source_dir = root.join("docs/language");
    let docs_build_dir = root.join("target/slintdocs");
    let html_static_dir = docs_build_dir.join("_static");

    std::fs::create_dir_all(docs_build_dir.as_path()).context("Error creating docs build dir")?;
    std::fs::create_dir_all(html_static_dir.as_path())
        .context("Error creating _static path for docs build")?;

    symlink_files_in_dir(
        &docs_source_dir,
        &docs_build_dir,
        ["..", "..", "docs", "language"].iter().collect::<PathBuf>(),
    )
    .context(format!("Error creating symlinks from docs source {docs_source_dir:?} to docs build dir {docs_build_dir:?}"))?;

    let pip_env = vec![(OsString::from("PIPENV_PIPFILE"), docs_source_dir.join("Pipfile"))];

    println!("Running pipenv install");

    super::run_command("pipenv", &["install"], pip_env.clone())
        .context("Error running pipenv install")?;

    println!("Running sphinx-build");

    let output = super::run_command(
        "pipenv",
        &[
            "run",
            "sphinx-build",
            docs_build_dir.to_str().unwrap(),
            docs_build_dir.join("html").to_str().unwrap(),
        ],
        pip_env,
    )
    .context("Error running pipenv install")?;

    println!("{}", String::from_utf8_lossy(&output.stdout));

    if show_warnings {
        println!("{}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}

pub fn generate_enum_docs() -> Result<(), Box<dyn std::error::Error>> {
    let mut enums: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();

    macro_rules! gen_enums {
        ($( $(#[doc = $enum_doc:literal])* $(#[non_exhaustive])? enum $Name:ident { $( $(#[doc = $value_doc:literal])* $Value:ident,)* })*) => {
            $(
                let mut entry = format!("## `{}`\n\n", stringify!($Name));
                $(entry += &format!("{}\n", $enum_doc);)*
                entry += "\n";
                $(
                    let mut has_val = false;
                    entry += &format!("* **`{}`**:", to_kebab_case(stringify!($Value)));
                    $(
                        if has_val {
                            entry += "\n   ";
                        }
                        entry += &format!("{}", $value_doc);
                        has_val = true;
                    )*
                    entry += "\n";
                )*
                entry += "\n";
                enums.insert(stringify!($Name).to_string(), entry);
            )*
        }
    }

    #[allow(unused)] // for 'has_val'
    {
        i_slint_common::for_each_enums!(gen_enums);
    }

    let root = super::root_dir();

    let path = root.join("docs/language/src/builtins/enums.md");
    let mut file = std::fs::File::create(&path).context(format!("error creating {path:?}"))?;

    file.write_all(
        br#"<!-- Generated with `cargo xtask slintdocs` from internal/commons/enums.rs -->
# Builtin Enumerations

"#,
    )?;

    for (_, v) in enums {
        // BTreeMap<i64, String>
        write!(file, "{v}")?;
    }

    /// Convert a ascii pascal case string to kebab case
    fn to_kebab_case(str: &str) -> String {
        let mut result = Vec::with_capacity(str.len());
        for x in str.as_bytes() {
            if x.is_ascii_uppercase() {
                if !result.is_empty() {
                    result.push(b'-');
                }
                result.push(x.to_ascii_lowercase());
            } else {
                result.push(*x);
            }
        }
        String::from_utf8(result).unwrap()
    }

    Ok(())
}
