// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let slint_file = Path::new("main.slint");
    println!("cargo:rerun-if-changed={}", slint_file.display());
    println!("cargo:rerun-if-env-changed=SLINT_COMPILER");

    let compiler = find_slint_compiler(&out_dir);
    println!("cargo:rerun-if-changed={}", compiler.display());

    let generated = out_dir.join("main.rs");
    let status = Command::new(&compiler)
        .arg("--slint-sc")
        .arg(slint_file)
        .arg("-o")
        .arg(&generated)
        .status()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", compiler.display()));
    assert!(status.success(), "slint-compiler failed on {}", slint_file.display());
}

/// Locate the prebuilt `slint-compiler` binary: the `SLINT_COMPILER` override,
/// otherwise the host build products for this profile.
fn find_slint_compiler(out_dir: &Path) -> PathBuf {
    if let Some(path) = env::var_os("SLINT_COMPILER") {
        return PathBuf::from(path);
    }

    // OUT_DIR is `<target-dir>[/<triple>]/<profile>/build/<pkg>-<hash>/out`, so
    // the profile directory sits just above the `build` component.
    let profile_dir = out_dir
        .ancestors()
        .find(|p| p.file_name().is_some_and(|name| name == "build"))
        .and_then(Path::parent)
        .expect("OUT_DIR is under a target profile directory");
    let profile = profile_dir.file_name().expect("the profile directory has a name");

    // Building for a target adds a `<triple>` directory above the profile, and
    // the compiler runs on the host, so its build products are the ones without
    // that directory.
    let target = env::var("TARGET").expect("cargo sets TARGET for build scripts");
    let compiler_dir = profile_dir
        .parent()
        .filter(|dir| dir.file_name().is_some_and(|name| name == target.as_str()))
        .and_then(Path::parent)
        .map_or_else(|| profile_dir.to_path_buf(), |target_dir| target_dir.join(profile));

    let compiler = compiler_dir.join(format!("slint-compiler{}", env::consts::EXE_SUFFIX));
    // `dev` is the profile whose directory is named `debug`, and it's the default.
    let profile_flag = match profile.to_str() {
        Some("debug") | None => String::new(),
        Some("release") => " --release".into(),
        Some(profile) => format!(" --profile {profile}"),
    };
    assert!(
        compiler.exists(),
        "slint-compiler not found at {}.\n\
         Build it first:\n    \
         cargo build -p slint-compiler --no-default-features --features slint-sc{}\n\
         or point SLINT_COMPILER at the binary.",
        compiler.display(),
        profile_flag
    );
    compiler
}
