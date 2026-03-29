// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use ::slint::slint;

#[test]
fn styled_text_can_be_created_from_rust_markdown() {
    i_slint_backend_testing::init_no_event_loop();

    slint! {
        export component Test inherits Window {
            in-out property <styled-text> text;
        }
    }

    let component = Test::new().unwrap();
    let greeting = slint::parse_markdown("Hello *world*!").unwrap();
    component.set_text(greeting.clone());
    assert_eq!(component.get_text(), greeting);

    let emphasis = slint::parse_markdown("*world*").unwrap();
    let interpolated = slint::parse_markdown_with_arguments("Hello {}!", &[emphasis]).unwrap();
    component.set_text(interpolated.clone());
    assert_eq!(component.get_text(), interpolated);

    let plain = slint::string_to_styled_text("plain text");
    component.set_text(plain.clone());
    assert_eq!(component.get_text(), plain);
}
