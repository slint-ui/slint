// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    println!("cargo::rerun-if-env-changed=SLINT_SPRINGBOARD_DEFAULT_ARTIFACT_CHANNEL");
    let channel = std::env::var("SLINT_SPRINGBOARD_DEFAULT_ARTIFACT_CHANNEL")
        .ok()
        .filter(|channel| !channel.trim().is_empty())
        .unwrap_or_else(|| format!("v{}", env!("CARGO_PKG_VERSION")));
    println!("cargo::rustc-env=SLINT_SPRINGBOARD_DEFAULT_ARTIFACT_CHANNEL={channel}");
}
