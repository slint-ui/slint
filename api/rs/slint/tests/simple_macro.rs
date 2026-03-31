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

#[test]
fn test_multiple_rust_attrs() {
    i_slint_backend_testing::init_no_event_loop();
    slint! {
        @rust-attr(derive(serde::Serialize, serde::Deserialize))
        @rust-attr(serde(rename_all = "camelCase"))
        export struct MultiAttr {
            field-foo: int,
            field-bar: float,
        }

        export component TestAttrs inherits Window { }
    }
    let data = MultiAttr { field_foo: 1, field_bar: 1.0 };
    let value = serde_json::to_value(&data).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "fieldFoo": 1,
            "fieldBar": 1.0
        })
    );
}

#[test]
fn test_struct_with_length_field_in_two_way_binding() {
    i_slint_backend_testing::init_no_event_loop();
    slint! {
        export struct RectangleStuff {
            x: length,
            y: length,
            width: length,
            height: length,
        }

        export component TestLengthStruct inherits Window {
            in-out property <RectangleStuff> stuff: {
                x: 50px,
                y: 50px,
                width: 100px,
                height: 100px
            };

            Rectangle {
                x <=> root.stuff.x;
                y <=> root.stuff.y;
                width <=> root.stuff.width;
                height <=> root.stuff.height;
            }
        }
    }

    let component = TestLengthStruct::new().unwrap();
    let stuff = component.get_stuff();
    assert_eq!(stuff.x, 50.0);
    assert_eq!(stuff.y, 50.0);
    assert_eq!(stuff.width, 100.0);
    assert_eq!(stuff.height, 100.0);
}
