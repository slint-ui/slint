// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![cfg_attr(not(any(feature = "shared-fontique", feature = "color-parsing")), no_std)]

extern crate alloc;

pub mod builtin_structs;
#[cfg(feature = "color-parsing")]
pub mod color_parsing;
pub mod enums;
pub mod key_codes;

#[cfg(feature = "shared-fontique")]
pub mod sharedfontique;

pub mod styled_text;

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
        if has_qt { "qt" } else { "fluent" }
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

/// Internal "magic" value for row and col numbers, to mean "auto", in GridLayoutInputData
/// Use the value 65536, so it's outside u16 range and not as likely as -1
/// (we can catch it as a literal at compile time, but not if it's a runtime value)
pub const ROW_COL_AUTO: f32 = u16::MAX as f32 + 1.;
