// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Conversions of identifiers and names

use alloc::string::String;
use alloc::vec::Vec;
use smol_str::SmolStr;

/// Convert a ascii kebab string to pascal case
pub fn to_pascal_case(str: &str) -> String {
    let mut result = Vec::with_capacity(str.len());
    let mut next_upper = true;
    for x in str.as_bytes() {
        if *x == b'-' {
            next_upper = true;
        } else if next_upper {
            result.push(x.to_ascii_uppercase());
            next_upper = false;
        } else {
            result.push(*x);
        }
    }
    String::from_utf8(result).unwrap()
}

/// Convert a ascii pascal case string to kebab case
pub fn to_kebab_case(str: &str) -> String {
    let mut result = Vec::with_capacity(str.len());
    for x in str.as_bytes() {
        if x.is_ascii_uppercase() {
            if !result.is_empty() {
                result.push(b'-');
            }
            result.push(x.to_ascii_lowercase());
        } else {
            result.push(*x);
        }
    }
    String::from_utf8(result).unwrap()
}

pub fn normalize_identifier(ident: &str) -> SmolStr {
    if is_identifier_normalized(ident) {
        // one bulk copy instead of the char-by-char builder below
        return SmolStr::new(ident);
    }
    let mut builder = smol_str::SmolStrBuilder::default();
    for (pos, c) in ident.chars().enumerate() {
        match (pos, c) {
            (0, '-') | (0, '_') => builder.push('_'),
            (_, '_') => builder.push('-'),
            (_, c) => builder.push(c),
        }
    }
    builder.finish()
}

/// Returns true if [`normalize_identifier`] would return `ident` unchanged.
/// Lets callers skip the copy (and heap allocation for long identifiers).
pub fn is_identifier_normalized(ident: &str) -> bool {
    // '-' and '_' are ASCII, so a byte scan is UTF-8-safe
    let b = ident.as_bytes();
    b.first() != Some(&b'-') && !b[1.min(b.len())..].contains(&b'_')
}

#[test]
fn test_normalize_identifier() {
    assert_eq!(normalize_identifier("true"), SmolStr::new("true"));
    assert_eq!(normalize_identifier("foo_bar"), SmolStr::new("foo-bar"));
    assert_eq!(normalize_identifier("-foo_bar"), SmolStr::new("_foo-bar"));
    assert_eq!(normalize_identifier("-foo-bar"), SmolStr::new("_foo-bar"));
    assert_eq!(normalize_identifier("foo_bar_"), SmolStr::new("foo-bar-"));
    assert_eq!(normalize_identifier("foo_bar-"), SmolStr::new("foo-bar-"));
    assert_eq!(normalize_identifier("_foo_bar_"), SmolStr::new("_foo-bar-"));
    assert_eq!(normalize_identifier("__1"), SmolStr::new("_-1"));
    assert_eq!(normalize_identifier("--1"), SmolStr::new("_-1"));
    assert_eq!(normalize_identifier("--1--"), SmolStr::new("_-1--"));
}

#[test]
fn test_is_identifier_normalized() {
    for ident in
        ["true", "foo-bar", "foo_bar", "-foo", "_foo", "foo-bar-", "", "-", "_", "ä_ö", "ä-ö"]
    {
        assert_eq!(
            is_identifier_normalized(ident),
            normalize_identifier(ident) == ident,
            "{ident:?}"
        );
    }
}
