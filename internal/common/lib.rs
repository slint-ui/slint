// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![cfg_attr(not(feature = "shared-fontique"), no_std)]

pub mod builtin_structs;
pub mod enums;
pub mod key_codes;

#[cfg(feature = "shared-fontique")]
pub mod sharedfontique;

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

/// MenuItem with this title are actually MenuSeparator
///
/// Use a private unicode character so we are sure it is not used in the user's code
pub const MENU_SEPARATOR_PLACEHOLDER_TITLE: &str = "\u{E001}⸺";
