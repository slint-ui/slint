// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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

    println!("cargo:rustc-env=SLINT_WIDGETS_LIBRARY={}", output_file_path.display());

    Ok(())
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
