// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Reading `@testable` properties back through `ElementHandle` under the interpreter.
//! The same source and expectations run against generated Rust code in
//! `tests/cases/testing/testable_properties.slint`, keeping the two runtimes'
//! encodings identical.
//!
//! `@testable` is experimental, so `compile` enables experimental features for the
//! whole process before building: every test in this file shares `SOURCE`, and none
//! of them exercises the "experimental disabled" rejection path.

use i_slint_backend_testing::ElementHandle;
use slint_interpreter::{Compiler, ComponentInstance, Value};

fn compile(code: &str) -> ComponentInstance {
    i_slint_backend_testing::init_no_event_loop();
    // SAFETY: set once, before any component is compiled; no test in this file
    // reads or relies on a concurrently different value.
    unsafe {
        std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    }
    let result =
        spin_on::spin_on(Compiler::default().build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    result.component("TestCase").expect("component should compile").create().unwrap()
}

const SOURCE: &str = r#"
    enum Mood { happy, grumpy }

    component Base inherits Rectangle {
        @testable in property <string> base-text: "from-base";
        property <int> base-private: 42;
        Text { text: root.base-text; }
    }

    component Widget inherits Base {
        @testable in-out property <bool> flag: true;
        @testable out property <float> ratio: flag ? 0.5 : 0.25;
        @testable in property <float> big: 0.0;
        @testable in property <length> len: 12px;
        @testable in property <duration> dur: 250ms;
        @testable in property <angle> ang: 90deg;
        @testable in property <percent> pct: 25%;
        @testable in property <color> tint: #112233;
        @testable in property <brush> solid: #445566;
        @testable in property <Mood> mood: Mood.grumpy;
        @testable in property <{a: int, b: string}> record: { a: 1, b: "x" };
        in property <int> gone-unread: 4;
        in-out property <int> public-unmarked: 6;
        @testable property <int> testable-unread: 55;
        @testable property <{a: int, b: string}> testable-struct: { a: 3, b: "s" };
        @testable in-out property <string> aliased <=> inner.text;
        inner := Text { text: "hello"; }
        Rectangle {
            x: root.len;
            background: root.tint;
            border-color: root.solid;
            opacity: root.flag && root.mood == Mood.grumpy && root.record.a == 1 && root.public-unmarked == 6 ? root.ratio : 0.0;
            border-width: root.dur > 100ms && root.ang > 45deg ? 1px : 2px;
            width: root.pct * 1px;
            height: root.ratio * 100px;
        }
    }

    component Row inherits Rectangle {
        @testable in property <bool> selected;
        background: selected ? #ff0000 : #00ff00;
    }

    export component TestCase inherits Window {
        width: 300px;
        height: 300px;
        in-out property <bool> toggle: true;
        VerticalLayout {
            w := Widget {
                property <int> use-site-private: 7;
                @testable in property <int> extra: root.toggle ? 8 : 9;
                flag: root.toggle;
            big: root.toggle ? 0.1 : 100000000000000000000.0;
                len: root.toggle ? 12px : 13px;
                dur: root.toggle ? 250ms : 100ms;
                ang: root.toggle ? 90deg : 45deg;
                pct: root.toggle ? 25% : 75%;
                tint: root.toggle ? #112233 : #445566;
                solid: root.toggle ? #445566 : #112233;
                mood: root.toggle ? Mood.grumpy : Mood.happy;
                record: root.toggle ? { a: 1, b: "x" } : { a: 2, b: "y" };
                base-text: root.toggle ? "from-base" : "other";
            }
            for item in [true, false]: Row { selected: item; }
        }
        out property <int> read-extra: w.extra + w.use-site-private;
    }
"#;

#[test]
fn testable_properties_are_listed_with_types() {
    let instance = compile(SOURCE);
    let widget = ElementHandle::find_by_element_id(&instance, "TestCase::w").next().unwrap();

    let mut props = widget.testable_properties().unwrap();
    props.sort();
    let props: Vec<(&str, &str)> =
        props.iter().map(|(name, ty)| (name.as_str(), ty.as_str())).collect();
    assert_eq!(
        props,
        [
            ("aliased", "string"),
            ("ang", "angle"),
            ("base-text", "string"),
            ("big", "float"),
            ("dur", "duration"),
            ("extra", "int"),
            ("flag", "bool"),
            ("len", "length"),
            ("mood", "Mood"),
            ("pct", "percent"),
            ("ratio", "float"),
            ("record", "{ a: int,b: string,}"),
            ("solid", "brush"),
            ("testable-struct", "{ a: int,b: string,}"),
            ("testable-unread", "int"),
            ("tint", "color"),
        ]
    );
    // Not listed: the unmarked declarations, public and read (public-unmarked),
    // unread (gone-unread) or private (base-private, use-site-private).
}

#[test]
fn testable_property_values_encode_per_type() {
    let instance = compile(SOURCE);
    let widget = ElementHandle::find_by_element_id(&instance, "TestCase::w").next().unwrap();

    let value = |name: &str| widget.testable_property_value(name).map(|v| v.to_string());
    assert_eq!(value("flag").as_deref(), Some("true"));
    assert_eq!(value("ratio").as_deref(), Some("0.5"));
    // The shortest-round-trip encoding, identical across runtimes (see debug_info).
    assert_eq!(value("big").as_deref(), Some("0.1"));
    assert_eq!(value("len").as_deref(), Some("12"));
    assert_eq!(value("dur").as_deref(), Some("250"));
    assert_eq!(value("ang").as_deref(), Some("90"));
    assert_eq!(value("pct").as_deref(), Some("25"));
    assert_eq!(value("tint").as_deref(), Some("#112233ff"));
    assert_eq!(value("solid").as_deref(), Some("#445566ff"));
    assert_eq!(value("mood").as_deref(), Some("grumpy"));
    assert_eq!(value("extra").as_deref(), Some("8"));
    assert_eq!(value("base-text").as_deref(), Some("from-base"));
    assert_eq!(value("aliased").as_deref(), Some("hello"));
    // A struct value has no string encoding; the property is only listed.
    assert_eq!(value("record"), None);
    assert_eq!(value("testable-struct"), None);
    // Listed regardless of visibility and use, with the binding kept alive.
    assert_eq!(value("testable-unread").as_deref(), Some("55"));
    // Unmarked declarations are not listed, whether public and read, unread, or private.
    assert_eq!(value("public-unmarked"), None);
    assert_eq!(value("gone-unread"), None);
    assert_eq!(value("use-site-private"), None);
    assert_eq!(value("no-such-property"), None);
}

#[test]
fn repeated_rows_read_their_own_values() {
    let instance = compile(SOURCE);
    let rows: Vec<_> = ElementHandle::find_by_element_type_name(&instance, "Row").collect();
    assert_eq!(rows.len(), 2);
    let selected: Vec<_> =
        rows.iter().map(|r| r.testable_property_value("selected").unwrap().to_string()).collect();
    assert_eq!(selected, ["true", "false"]);
}

#[test]
fn values_track_property_changes() {
    let instance = compile(SOURCE);
    let widget = ElementHandle::find_by_element_id(&instance, "TestCase::w").next().unwrap();
    assert_eq!(widget.testable_property_value("flag").unwrap(), "true");
    instance.set_property("toggle", Value::Bool(false)).unwrap();
    assert_eq!(widget.testable_property_value("flag").unwrap(), "false");
    assert_eq!(widget.testable_property_value("len").unwrap(), "13");
    assert_eq!(widget.testable_property_value("mood").unwrap(), "happy");
    assert_eq!(widget.testable_property_value("big").unwrap(), "100000000000000000000");
}
