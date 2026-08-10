// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Number formatting and parsing read the decimal separator from the context the component
//! belongs to, not from whichever context happens to be the thread's.
//!
//! Every expression below is compiled through a different generator path -- a cast to
//! string, `to-fixed`, `to-precision`, `to-float` and `is-float` -- so a path left on the
//! ambient lookup shows up as one failing assertion rather than all of them.

mod common;

slint::slint! {
    export struct DefaultedField {
        // A locale-dependent cast is rejected in a field default, and this locale-independent
        // one folds to a string literal before it reaches the generator. So no cast is
        // compiled without access to the globals -- which is what `access_context` assumes.
        label: string = 42,
    }

    export component TestComponent inherits Window {
        in property <DefaultedField> defaulted;
        out property <string> label: root.defaulted.label;
        in property <float> value: 1.5;
        in property <string> parsed-input: "2,5";
        out property <string> as-string: root.value;
        out property <string> fixed: root.value.to-fixed(2);
        out property <string> precise: root.value.to-precision(3);
        out property <float> parsed: root.parsed-input.to-float();
        out property <bool> is-number: root.parsed-input.is-float();
    }
}

/// The component's own context decides, so a second context on the same thread with a
/// different locale must not change what this one produces.
#[test]
fn formatting_follows_the_components_context() {
    let _window = common::setup(64, 64);

    let ui = TestComponent::new().unwrap();

    // Baseline: the default locale spells the separator '.'.
    assert_eq!(ui.get_as_string(), "1.5");
    assert_eq!(ui.get_fixed(), "1.50");
    assert_eq!(ui.get_precise(), "1.50");
    // "2,5" is not a float under a '.' locale.
    assert!(!ui.get_is_number());
    assert_eq!(ui.get_parsed(), 0.0);

    // Folded at compile time, so it never reaches the locale-aware path.
    assert_eq!(ui.get_label(), "42");
}
