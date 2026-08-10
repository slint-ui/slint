// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::PathBuf;

/// Mirror the `/STACK` setting from `.cargo/config.toml`, which is not shipped
/// in the published crate.
fn bump_windows_stack_size() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8000000");
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_REMOTE");
    bump_windows_stack_size();
    // The slint!{} macro in remote.rs needs the experimental
    // new_with_existing_window constructor.
    if std::env::var_os("CARGO_FEATURE_REMOTE").is_some() {
        println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");
        // Android needs the Material style so the UI's ListView scrolls by touch.
        println!("cargo:rerun-if-env-changed=SLINT_STYLE");
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android")
            && std::env::var_os("SLINT_STYLE").is_none()
        {
            println!("cargo:rustc-env=SLINT_STYLE=material");
        }
        generate_third_party_licenses();
    }
}

/// Write `$OUT_DIR/third-party-licenses.json` for the in-app third-party
/// attribution page of the remote viewer's idle screen (remote.rs embeds the
/// file with `include_str!`). The page ships on Android; on every other target
/// the empty document `{"crates":[],"licenses":[]}` is written, which leaves
/// the page's entry point hidden. iOS surfaces its licenses through a
/// Settings.bundle instead (see scripts/generate_ios_settings_bundle.bash), so
/// its in-app page is intentionally empty.
///
/// The data comes from the workspace's `cargo xtask license` generator, which
/// only exists in a repo checkout. The shipped Android app is always built from
/// one, so it gets the full attribution; everywhere else (desktop builds,
/// `cargo install` from crates.io) the empty document is used.
fn generate_third_party_licenses() {
    println!("cargo:rerun-if-env-changed=SLINT_VIEWER_FORCE_ATTRIBUTION");

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let out_json = out_dir.join("third-party-licenses.json");

    // The in-app page ships on Android. SLINT_VIEWER_FORCE_ATTRIBUTION=1 forces
    // generation in other builds to develop/test the page on desktop.
    let wanted = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android")
        || std::env::var("SLINT_VIEWER_FORCE_ATTRIBUTION").as_deref() == Ok("1");

    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let workspace_dir = manifest_dir.ancestors().nth(2).unwrap().to_path_buf();
    let in_workspace = workspace_dir.join("xtask/Cargo.toml").exists();

    if !wanted || !in_workspace {
        if wanted {
            println!(
                "cargo:warning=Building outside the Slint workspace; the third-party license attribution page will be empty."
            );
        }
        std::fs::write(&out_json, "{\"crates\":[],\"licenses\":[]}\n")
            .expect("failed to write empty third-party-licenses.json");
        return;
    }

    // The `xtask` alias comes from the workspace's `.cargo/config.toml`,
    // discovered through the working directory.
    let status = std::process::Command::new(std::env::var_os("CARGO").unwrap())
        .args(["xtask", "license", "--format", "json", "-o"])
        .arg(&out_json)
        // The generator analyzes this manifest's dependencies.
        .current_dir(&manifest_dir)
        // Run xtask in its own target directory: sharing the outer build's
        // would deadlock on cargo's build-directory lock.
        .env("CARGO_TARGET_DIR", out_dir.join("xtask-target"))
        // Don't leak target-build flags into the nested host build of xtask
        // (the Android packaging scripts set path-remapping and linker flags).
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .status()
        .expect("failed to run cargo xtask license");
    // Fail the build loudly: a generator failure here means a license outside
    // the accepted set entered the dependency tree (or the generator broke) and
    // the app would ship without correct attribution.
    assert!(status.success(), "cargo xtask license failed with {status}");
}
