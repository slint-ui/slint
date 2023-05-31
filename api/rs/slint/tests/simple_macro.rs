// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use ::slint::slint;

#[test]
fn simple_window() {
    i_slint_backend_testing::init();
    slint!(export component X inherits Window{});
    X::new().unwrap();
}
#[test]
fn empty_stuff() {
    slint!();
    slint!(export struct Hei { abcd: bool });
    slint!(export global G { });
}

#[test]
fn test_serialize_deserialize_struct() {
    i_slint_backend_testing::init();
    slint! {
        @rust-attr(cfg_attr(feature="serde", derive(Serialize, Deserialize)))
        export struct TestStruct {
            foo: int,
        }
        export component Test { }
    }
    let data = TestStruct { foo: 1 };
    let serialized = serde_json::to_string(&data).unwrap();
    let deserialized: TestStruct = serde_json::from_str(&serialized).unwrap();
    assert_eq!(data, deserialized);
}
