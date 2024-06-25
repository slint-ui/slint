// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use ::slint::slint;

#[test]
fn simple_window() {
    i_slint_backend_testing::init_no_event_loop();
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
    i_slint_backend_testing::init_no_event_loop();
    slint! {

        @rust-attr(derive(serde::Serialize, serde::Deserialize))
        export enum TestEnum {
            hello, world, xxx
        }

        @rust-attr(derive(serde::Serialize, serde::Deserialize))
        export struct TestStruct {
            enum: TestEnum,
            foo: int,
        }
        export component Test inherits Window { }
    }
    let data = TestStruct { foo: 1, r#enum: TestEnum::World };
    let serialized = serde_json::to_string(&data).unwrap();
    let deserialized: TestStruct = serde_json::from_str(&serialized).unwrap();
    assert_eq!(data, deserialized);
}
