// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![cfg_attr(not(feature = "shared-fontdb"), no_std)]

pub mod builtin_structs;
pub mod enums;
pub mod key_codes;

#[cfg(feature = "shared-fontdb")]
pub mod sharedfontdb;

/// Detect the native style depending on the platform
pub fn get_native_style(has_qt: bool, target: &str) -> &'static str {
    // NOTE: duplicated in api/cpp/CMakeLists.txt
    if target.contains("android") {
        "material"
    } else if target.contains("windows") {
        "fluent"
    } else if target.contains("apple") {
        "cupertino"
    } else if target.contains("wasm") {
        "fluent"
    } else if target.contains("linux") | target.contains("bsd") {
        if has_qt {
            "qt"
        } else {
            "fluent"
        }
    } else if cfg!(target_os = "android") {
        "material"
    } else if cfg!(target_os = "windows") {
        "fluent"
    } else if cfg!(target_os = "macos") {
        "cupertino"
    } else if cfg!(target_family = "wasm") {
        "fluent"
    } else if has_qt {
        "qt"
    } else {
        "fluent"
    }
}
