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

mod edition_1_19 {
    slint::slint! {
        #[edition = "1.19"]
        export component Test inherits Window {
            out property <length> parent-min-width: parent-rect.min-width;
            parent-rect := Rectangle {
                if true: Rectangle { min-width: 50px; }
            }
        }
    }
}

mod no_edition {
    slint::slint! {
        export component Test inherits Window {
            out property <length> parent-min-width: parent-rect.min-width;
            parent-rect := Rectangle {
                if true: Rectangle { min-width: 50px; }
            }
        }
    }
}

mod edition_over_project_file {
    // The project file next to test.slint selects edition 1.0.
    slint::slint! {
        #[edition = "1.19"]
        export { Test } from "edition-project/test.slint";
    }
}

mod project_file_edition {
    slint::slint! {
        export { Test } from "edition-project/test.slint";
    }
}

#[test]
fn the_edition_attribute_selects_the_behavior() {
    i_slint_backend_testing::init_no_event_loop();
    assert_eq!(edition_1_19::Test::new().unwrap().get_parent_min_width(), 50.);
    assert_eq!(no_edition::Test::new().unwrap().get_parent_min_width(), 0.);
    assert_eq!(edition_over_project_file::Test::new().unwrap().get_parent_min_width(), 50.);
    assert_eq!(project_file_edition::Test::new().unwrap().get_parent_min_width(), 0.);
}
