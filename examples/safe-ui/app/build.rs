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
/// otherwise the shared target directory.
fn find_slint_compiler(out_dir: &Path) -> PathBuf {
    if let Some(path) = env::var_os("SLINT_COMPILER") {
        return PathBuf::from(path);
    }

    // OUT_DIR is `<target>/<profile>/build/<pkg>-<hash>/out`; the binary sits
    // in `<target>/<profile>/`, just above the `build` component.
    let profile_dir = out_dir
        .ancestors()
        .find(|p| p.file_name().is_some_and(|name| name == "build"))
        .and_then(Path::parent)
        .expect("OUT_DIR is under a target profile directory");
    let compiler = profile_dir.join(format!("slint-compiler{}", env::consts::EXE_SUFFIX));
    assert!(
        compiler.exists(),
        "slint-compiler not found at {}.\n\
         Build it first with:\n    \
         cargo build -p slint-compiler --no-default-features --features slint-sc\n\
         or point SLINT_COMPILER at the binary.",
        compiler.display()
    );
    compiler
}
