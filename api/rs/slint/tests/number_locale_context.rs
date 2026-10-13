// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Number formatting and parsing in generated code follow the window's locale.

mod common;

slint::slint! {
    export struct DefaultedField {
        // Folded to a string literal at compile time.
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

#[test]
fn formatting_follows_the_windows_locale() {
    use slint::ComponentHandle;
    use slint::private_unstable_api::re_exports::WindowInner;

    let _window = common::setup(64, 64);

    let ui = TestComponent::new().unwrap();
    let context = WindowInner::from_pub(ui.window()).context();

    context.set_locale("C");
    assert_eq!(ui.get_as_string(), "1.5");
    assert_eq!(ui.get_fixed(), "1.50");
    assert_eq!(ui.get_precise(), "1.50");
    assert!(!ui.get_is_number());
    assert_eq!(ui.get_parsed(), 0.0);
    assert_eq!(ui.get_label(), "42");

    context.set_locale("de_DE.UTF-8");
    assert_eq!(ui.get_as_string(), "1,5");
    assert_eq!(ui.get_fixed(), "1,50");
    assert_eq!(ui.get_precise(), "1,50");
    assert!(ui.get_is_number());
    assert_eq!(ui.get_parsed(), 2.5);
    assert_eq!(ui.get_label(), "42");
}
