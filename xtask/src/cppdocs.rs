// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore cppdocs pipenv pipfile

use anyhow::{Context, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[path = "../../api/cpp/cbindgen.rs"]
mod cbindgen;

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
        let filetype = entry.file_type().context("Cannot determine file type")?;
        if filetype.is_file() {
            symlink_file(symlink_source, symlink_target).context("Could not symlink file")?;
        } else if filetype.is_dir() {
            symlink_dir(symlink_source, symlink_target).context("Could not symlink directory")?;
        }
    }
    Ok(())
}

pub fn generate(show_warnings: bool, experimental: bool) -> Result<(), Box<dyn std::error::Error>> {
    let root = super::root_dir();

    let docs_source_dir = root.join("api/cpp");
    let docs_build_dir = root.join("target/cppdocs");
    let html_static_dir = docs_build_dir.join("_static");

    std::fs::create_dir_all(docs_build_dir.as_path()).context("Error creating docs build dir")?;
    std::fs::create_dir_all(html_static_dir.as_path())
        .context("Error creating _static path for docs build")?;

    symlink_files_in_dir(
        docs_source_dir.join("docs"),
        &docs_build_dir,
        ["..", "..", "api", "cpp", "docs"].iter().collect::<PathBuf>(),
    )
    .context("Error creating symlinks from docs source to docs build dir")?;

    symlink_file(
        ["..", "..", "api", "cpp", "README.md"].iter().collect::<PathBuf>(),
        docs_build_dir.join("README.md"),
    )?;

    let generated_headers_dir = docs_build_dir.join("generated_include");
    let enabled_features = cbindgen::EnabledFeatures {
        interpreter: true,
        live_reload: false,
        testing: true,
        backend_qt: true,
        backend_winit: true,
        backend_winit_x11: false,
        backend_winit_wayland: false,
        backend_linuxkms: true,
        backend_linuxkms_noseat: false,
        renderer_femtovg: true,
        renderer_skia: true,
        renderer_skia_opengl: false,
        renderer_skia_vulkan: false,
        renderer_software: true,
        gettext: true,
        accessibility: true,
        system_testing: true,
        freestanding: true,
        experimental,
    };
    cbindgen::gen_all(&root, &generated_headers_dir, enabled_features)?;

    let uv_project = vec![(OsString::from("UV_PROJECT"), docs_source_dir.join("docs"))];

    println!("Generating third-party license list with cargo-about");

    let cargo_about_output = super::run_command(
        "cargo",
        &[
            "about",
            "generate",
            "--manifest-path",
            "api/cpp/Cargo.toml",
            "api/cpp/docs/thirdparty.hbs",
            "-o",
            docs_build_dir.join("thirdparty.md").to_str().unwrap(),
        ],
        std::iter::empty::<(std::ffi::OsString, std::ffi::OsString)>(),
    )?;

    println!(
        "{}\n{}",
        String::from_utf8_lossy(&cargo_about_output.stdout),
        String::from_utf8_lossy(&cargo_about_output.stderr)
    );

    println!("Running sphinx-build");

    let output = super::run_command(
        "uv",
        &[
            "run",
            "sphinx-build",
            docs_build_dir.to_str().unwrap(),
            docs_build_dir.join("html").to_str().unwrap(),
        ],
        uv_project,
    )
    .context("Error running pipenv install")?;

    println!("{}", String::from_utf8_lossy(&output.stdout));

    if show_warnings {
        println!("{}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}
