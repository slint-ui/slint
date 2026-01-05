// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Integration tests for token processing in the slint! macro
//! 
//! These tests verify that:
//! 1. Identifiers with hyphens are correctly merged when touching (test case 5)
//! 2. Identifiers and hyphens remain separate when not touching
//! 3. The macro correctly handles different spacing scenarios
//!
//! These are compilation tests - if the code compiles, the token processing worked correctly.

use slint::slint;

#[test]
fn test_identifier_with_hyphen_merged() {
    // Test case 5: This tests that fill_token_vec correctly merges
    // an identifier and a hyphen when they are touching.
    // In Slint, foo-bar is a valid identifier.
    slint! {
        component TestComponent {
            property <string> foo-bar: "test";
        }
    }
    
    // If this compiles successfully, it means the tokens were merged correctly
    // into a single identifier "foo-bar" rather than "foo", "-", "bar"
}

#[test]
fn test_multiple_hyphenated_identifiers() {
    // Additional test to verify multiple hyphenated identifiers work
    slint! {
        component MultiHyphen {
            property <int> my-custom-property: 42;
            property <string> another-hyphenated-name: "value";
        }
    }
    
    // Compilation success means both hyphenated identifiers were correctly processed
}

#[test]
fn test_hyphen_as_operator() {
    // Test that hyphens as minus operators (with spacing) are handled correctly
    slint! {
        component MathComponent {
            property <int> result: 10 - 5;
        }
    }
    
    // Compilation success means the minus operator was correctly identified
    // (not merged with adjacent tokens)
}

#[test]
fn test_mixed_hyphen_usage() {
    // Test mixing hyphenated identifiers and minus operators
    slint! {
        component MixedUsage {
            property <int> my-value: 20;
            property <int> computed: my-value - 10;
        }
    }
    
    // Compilation success means both uses of hyphen were correctly distinguished:
    // - "my-value" was merged into a single identifier
    // - " - " was kept as a separate minus operator
}
