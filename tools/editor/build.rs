// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint_build::CompilerConfiguration;

fn main() {
    record_test_build();
    // Safety: there are no other threads at this point
    unsafe {
        // Make the compiler handle ComponentContainer:
        std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    }

    // Some tests use the ElementHandle API, which requires debug info
    slint_build::compile_with_config(
        "ui/main.slint",
        CompilerConfiguration::new().with_debug_info(true),
    )
    .unwrap();
}

fn record_test_build() {
    if std::env::var_os("CARGO_FEATURE_SYSTEM_TESTING").is_none() {
        return;
    }
    let git = |args: &[&str]| {
        std::process::Command::new("git").args(args).output().ok().filter(|o| o.status.success())
    };
    let mut revision = git(&["rev-parse", "HEAD"])
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    if git(&["status", "--porcelain", "--untracked-files=normal"])
        .is_some_and(|output| !output.stdout.is_empty())
    {
        revision.push_str("-dirty");
    }
    for reference in ["HEAD", "refs/heads", "index"] {
        if let Some(output) = git(&["rev-parse", "--git-path", reference]) {
            println!("cargo:rerun-if-changed={}", String::from_utf8_lossy(&output.stdout).trim());
        }
    }
    let mut features: Vec<_> = std::env::vars()
        .filter_map(|(name, _)| name.strip_prefix("CARGO_FEATURE_").map(str::to_owned))
        .collect();
    features.sort();
    println!("cargo:rustc-env=SLINT_EDITOR_BUILD_REVISION={revision}");
    println!("cargo:rustc-env=SLINT_EDITOR_BUILD_FEATURES={}", features.join(","));
}
