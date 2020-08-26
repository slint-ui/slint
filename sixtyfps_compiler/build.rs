/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use std::fs::read_dir;
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() -> std::io::Result<()> {
    let mut library_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    library_dir.push("widgets");

    println!("cargo:rerun-if-changed={}", library_dir.display());

    let output_file_path = Path::new(&std::env::var_os("OUT_DIR").unwrap())
        .join(Path::new("included_library").with_extension("rs"));

    let library_files: Vec<PathBuf> = read_dir(library_dir)?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.path().is_file()
                && entry.path().extension().unwrap_or_default() == std::ffi::OsStr::new("60")
        })
        .map(|entry| entry.path())
        .collect();

    let mut file = std::fs::File::create(&output_file_path)?;
    write!(
        file,
        "use crate::typeloader::VirtualFile; pub fn widget_library() -> &'static [&'static VirtualFile<'static>] {{ &["
    )?;
    write!(
        file,
        "{}",
        library_files
            .iter()
            .map(|file| format!(
                "&VirtualFile {{path: \"{}\" , contents: include_str!(\"{}\")}}",
                file.file_name().unwrap().to_string_lossy(),
                file.display()
            ))
            .collect::<Vec<_>>()
            .join(",")
    )?;
    write!(file, "] }}")?;

    println!("cargo:rustc-env=SIXTYFPS_WIDGETS_LIBRARY={}", output_file_path.display());

    Ok(())
}
