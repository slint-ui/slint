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

#[cfg(feature = "locale-decimal-separator")]
fn locale_from_string(locale: &str) -> Option<icu_locale_core::Locale> {
    // sys_locale may return locales with '_' (e.g. "de_DE.UTF-8"), normalize to BCP47 '-'
    let normalized = locale.replace('_', "-");
    // Strip encoding suffix like ".UTF-8"
    let bcp47 = normalized.split('.').next().unwrap_or(&normalized);
    bcp47.parse().ok()
}

/// Returns the decimal separator character for the given locale string,
/// or `None` if the locale cannot be parsed or has no ICU data.
#[cfg(feature = "locale-decimal-separator")]
pub fn decimal_separator_for_locale(locale: &str) -> Option<char> {
    use icu_decimal::provider::{Baked, DecimalSymbolsV1};
    use icu_provider::prelude::*;

    let locale = locale_from_string(locale)?;
    let data_locale = DataLocale::from(&locale);
    let request = DataRequest {
        id: DataIdentifierBorrowed::for_marker_attributes_and_locale(
            DataMarkerAttributes::empty(),
            &data_locale,
        ),
        ..Default::default()
    };

    DataProvider::<DecimalSymbolsV1>::load(&Baked, request)
        .ok()
        .and_then(|r| r.payload.get().decimal_separator().chars().next())
}

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

#[cfg(all(test, feature = "locale-decimal-separator"))]
mod tests {
    use super::decimal_separator_for_locale;

    #[test]
    fn test_decimal_separator_for_locale() {
        // Comma locales
        assert_eq!(decimal_separator_for_locale("de"), Some(','));
        assert_eq!(decimal_separator_for_locale("de-DE"), Some(','));
        assert_eq!(decimal_separator_for_locale("de_DE"), Some(','));
        assert_eq!(decimal_separator_for_locale("de_DE.UTF-8"), Some(','));
        assert_eq!(decimal_separator_for_locale("fr"), Some(','));
        assert_eq!(decimal_separator_for_locale("fr-FR"), Some(','));
        assert_eq!(decimal_separator_for_locale("it"), Some(','));
        assert_eq!(decimal_separator_for_locale("es"), Some(','));
        assert_eq!(decimal_separator_for_locale("pt"), Some(','));
        assert_eq!(decimal_separator_for_locale("nl"), Some(','));
        assert_eq!(decimal_separator_for_locale("sv"), Some(','));
        assert_eq!(decimal_separator_for_locale("ru"), Some(','));
        assert_eq!(decimal_separator_for_locale("pl"), Some(','));
        assert_eq!(decimal_separator_for_locale("cs"), Some(','));
        assert_eq!(decimal_separator_for_locale("tr"), Some(','));
        assert_eq!(decimal_separator_for_locale("vi"), Some(','));

        // Dot locales
        assert_eq!(decimal_separator_for_locale("en"), Some('.'));
        assert_eq!(decimal_separator_for_locale("en-US"), Some('.'));
        assert_eq!(decimal_separator_for_locale("en_GB"), Some('.'));
        assert_eq!(decimal_separator_for_locale("ja"), Some('.'));
        assert_eq!(decimal_separator_for_locale("zh"), Some('.'));
        assert_eq!(decimal_separator_for_locale("ko"), Some('.'));

        // Empty / unknown
        assert_eq!(decimal_separator_for_locale(""), None);
    }
}
