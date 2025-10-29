// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::ffi::OsStr;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

fn main() -> std::io::Result<()> {
    println!("cargo:rustc-check-cfg=cfg(slint_debug_property)");

    let cargo_manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let library_dir = PathBuf::from("widgets");

    println!("cargo:rerun-if-changed={}", library_dir.display());

    let output_file_path = Path::new(&std::env::var_os("OUT_DIR").unwrap())
        .join(Path::new("included_library").with_extension("rs"));

    let mut file = BufWriter::new(std::fs::File::create(&output_file_path)?);
    write!(
        file,
        r#"
fn widget_library() -> &'static [(&'static str, &'static BuiltinDirectory<'static>)] {{
    &[
"#
    )?;

    for style in cargo_manifest_dir.join(&library_dir).read_dir()?.filter_map(Result::ok) {
        if !style.file_type().is_ok_and(|f| f.is_dir()) {
            continue;
        }
        let path = style.path();
        writeln!(
            file,
            "(\"{}\", &[{}]),",
            path.file_name().unwrap().to_string_lossy(),
            process_style(&cargo_manifest_dir, &path)?
        )?;
    }

    writeln!(file, "]\n}}")?;
    file.flush()?;

    let stable_library_path =
        copy_to_stable_location(&cargo_manifest_dir, &output_file_path)?;

    println!("cargo:rerun-if-env-changed=CARGO_TARGET_DIR");
    println!("cargo:rustc-env=SLINT_WIDGETS_LIBRARY={}", stable_library_path.display());

    Ok(())
}

fn copy_to_stable_location(
    cargo_manifest_dir: &Path,
    generated_file: &Path,
) -> std::io::Result<PathBuf> {
    let out_dir =
        generated_file.parent().expect("generated file to have a parent directory");

    let workspace_dir = std::env::var_os("CARGO_WORKSPACE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            cargo_manifest_dir
                .ancestors()
                .nth(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| cargo_manifest_dir.to_path_buf())
        });

    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            out_dir.ancestors().find_map(|ancestor| {
                (ancestor.file_name() == Some(OsStr::new("target")))
                    .then(|| ancestor.to_path_buf())
            })
        })
        .unwrap_or_else(|| workspace_dir.join("target"));

    let target_triple =
        std::env::var("TARGET").or_else(|_| std::env::var("HOST")).unwrap_or_else(|_| "host".into());
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());

    let stable_dir =
        target_dir.join("slint-widget-cache").join(target_triple).join(profile);
    std::fs::create_dir_all(&stable_dir)?;

    let stable_path = stable_dir.join("included_library.rs");
    let contents = std::fs::read(generated_file)?;
    std::fs::write(&stable_path, contents)?;

    Ok(stable_path)
}

fn process_style(cargo_manifest_dir: &Path, path: &Path) -> std::io::Result<String> {
    let library_files: Vec<PathBuf> = cargo_manifest_dir
        .join(path)
        .read_dir()?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|f| !f.is_dir())
                && entry
                    .path()
                    .extension()
                    .map(|ext| {
                        ext == std::ffi::OsStr::new("slint")
                            || ext == std::ffi::OsStr::new("60")
                            || ext == std::ffi::OsStr::new("svg")
                            || ext == std::ffi::OsStr::new("svgz")
                    })
                    .unwrap_or_default()
        })
        .map(|entry| entry.path())
        .collect();

    Ok(library_files
        .iter()
        .map(|file| {
            format!(
                "&BuiltinFile {{path: r#\"{}\"# , contents: include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), r#\"/{}\"#))}}",
                file.file_name().unwrap().to_string_lossy(),
                file.strip_prefix(cargo_manifest_dir).unwrap().display()
            )
        })
        .collect::<Vec<_>>()
        .join(","))
}
