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
pub mod unicode_utils;

pub const DEFAULT_DECIMAL_SEPARATOR: char = '.';

/// Formats a float the way Slint converts it to a string, before the locale's
/// decimal separator is substituted.
///
/// Both the runtime conversion and the compiler's constant folding use this,
/// so they can't diverge.
pub struct FormattedNumber(pub f64);

impl core::fmt::Display for FormattedNumber {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Number from which the increment of f32 is 1, so that we print enough precision
        // to be able to represent all integers
        if self.0.abs() < 16777216. {
            write!(f, "{}", self.0 as f32)
        } else {
            write!(f, "{}", self.0)
        }
    }
}

#[derive(Clone)]
pub struct TranslationsBundled {
    pub language: &'static str,
    pub decimal_separator: char,
}

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
pub fn decimal_separator_for_locale(locale: &str) -> char {
    use icu_decimal::provider::{Baked, DecimalSymbolsV1};
    use icu_provider::prelude::*;

    let locale = if let Some(locale) = locale_from_string(locale) {
        locale
    } else {
        return DEFAULT_DECIMAL_SEPARATOR;
    };
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
        .unwrap_or(DEFAULT_DECIMAL_SEPARATOR)
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

#[test]
fn test_formatted_number() {
    let format = |n: f64| alloc::format!("{}", FormattedNumber(n));
    assert_eq!(format(45.), "45");
    assert_eq!(format(45.12), "45.12");
    assert_eq!(format(-1325466.), "-1325466");
    assert_eq!(format(0.), "0");
    assert_eq!(format(16777216.), "16777216");
    assert_eq!(format(16777217.), "16777217");
    assert_eq!(format(-16777217.), "-16777217");
    assert_eq!(format(16777215.5), "16777216");
    assert_eq!(format(-16777215.5), "-16777216");
    assert_eq!(format(f64::NAN), "NaN");
}

#[cfg(all(test, feature = "locale-decimal-separator"))]
mod tests {
    use super::decimal_separator_for_locale;
    use crate::DEFAULT_DECIMAL_SEPARATOR;

    #[test]
    fn test_decimal_separator_for_locale() {
        // Comma locales
        assert_eq!(decimal_separator_for_locale("de"), ',');
        assert_eq!(decimal_separator_for_locale("de-DE"), ',');
        assert_eq!(decimal_separator_for_locale("de_DE"), ',');
        assert_eq!(decimal_separator_for_locale("de_DE.UTF-8"), ',');
        assert_eq!(decimal_separator_for_locale("fr"), ',');
        assert_eq!(decimal_separator_for_locale("fr-FR"), ',');
        assert_eq!(decimal_separator_for_locale("it"), ',');
        assert_eq!(decimal_separator_for_locale("es"), ',');
        assert_eq!(decimal_separator_for_locale("pt"), ',');
        assert_eq!(decimal_separator_for_locale("nl"), ',');
        assert_eq!(decimal_separator_for_locale("sv"), ',');
        assert_eq!(decimal_separator_for_locale("ru"), ',');
        assert_eq!(decimal_separator_for_locale("pl"), ',');
        assert_eq!(decimal_separator_for_locale("cs"), ',');
        assert_eq!(decimal_separator_for_locale("tr"), ',');
        assert_eq!(decimal_separator_for_locale("vi"), ',');

        // Dot locales
        assert_eq!(decimal_separator_for_locale("en"), '.');
        assert_eq!(decimal_separator_for_locale("en-US"), '.');
        assert_eq!(decimal_separator_for_locale("en_GB"), '.');
        assert_eq!(decimal_separator_for_locale("ja"), '.');
        assert_eq!(decimal_separator_for_locale("zh"), '.');
        assert_eq!(decimal_separator_for_locale("ko"), '.');

        // Empty / unknown
        assert_eq!(decimal_separator_for_locale(""), DEFAULT_DECIMAL_SEPARATOR);
    }
}
