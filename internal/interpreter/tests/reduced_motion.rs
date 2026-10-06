// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The interpreter honors the operating system's reduced-motion setting in every animation.

use i_slint_backend_testing::MotionPreference;
use slint_interpreter::{Compiler, Value};

const SOURCE: &str = r#"
export component TestCase inherits Window {
    in property <bool> cond;
    in property <bool> gate: true;
    out property <length> eased: eased-rect.x;
    out property <length> gated: gated-rect.x;
    eased-rect := Rectangle {
        x: cond ? 100px : 0px;
        animate x { duration: 1s; }
    }
    gated-rect := Rectangle {
        x: cond ? 100px : 0px;
        animate x { duration: 1s; enabled: root.gate; }
    }
}
"#;

#[test]
fn animations_follow_the_motion_preference() {
    i_slint_backend_testing::init_no_event_loop();
    let result =
        spin_on::spin_on(Compiler::default().build_from_source(SOURCE.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let instance = result.component("TestCase").unwrap().create().unwrap();
    assert_eq!(instance.get_property("eased").unwrap(), Value::Number(0.));
    assert_eq!(instance.get_property("gated").unwrap(), Value::Number(0.));

    i_slint_backend_testing::set_motion_preference(MotionPreference::Reduced);
    instance.set_property("cond", Value::Bool(true)).unwrap();
    assert_eq!(instance.get_property("eased").unwrap(), Value::Number(100.));
    assert_eq!(instance.get_property("gated").unwrap(), Value::Number(100.));

    i_slint_backend_testing::set_motion_preference(MotionPreference::NoPreference);
    instance.set_property("cond", Value::Bool(false)).unwrap();
    assert_eq!(instance.get_property("eased").unwrap(), Value::Number(100.));
    i_slint_backend_testing::mock_elapsed_time(500);
    assert_eq!(instance.get_property("eased").unwrap(), Value::Number(50.));
    assert_eq!(instance.get_property("gated").unwrap(), Value::Number(50.));
}
