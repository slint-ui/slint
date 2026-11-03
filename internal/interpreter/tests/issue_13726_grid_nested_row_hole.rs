// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! #13726, the interpreter twin of tests/cases/layout/issue_13726_grid_nested_row_hole.slint:
//! a repeated Row's constraint list is indexed by inner repeater slot, and its grid input data
//! must keep an empty slot's position too.

use i_slint_core::model::{ModelRc, VecModel};
use slint_interpreter::{Compiler, Value};
use std::rc::Rc;

const SOURCE: &str = r#"
export component TestCase inherits Window {
    width: 300px;
    height: 300px;

    in property <[[int]]> rows;

    grid := GridLayout {
        spacing: 0px;
        Row {
            Rectangle { min-width: 20px; min-height: 10px; }
            Rectangle { min-width: 30px; min-height: 10px; }
            Rectangle { min-height: 10px; }
            Rectangle { min-width: 30px; min-height: 10px; }
        }
        for cells in root.rows: Row {
            for c in cells: Rectangle { min-width: 5px; min-height: 10px; }
            Rectangle { colspan: 2; min-width: 50px; min-height: 10px; }
        }
    }

    out property <length> grid-preferred-width: grid.preferred-width;
}
"#;

#[test]
fn empty_inner_slot_keeps_its_position() {
    i_slint_backend_testing::init_no_event_loop();
    let result =
        spin_on::spin_on(Compiler::default().build_from_source(SOURCE.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let instance = result.component("TestCase").unwrap().create().unwrap();

    let inner: Rc<VecModel<Value>> =
        Rc::new(VecModel::from(vec![Value::Number(1.), Value::Number(1.)]));
    let rows: Rc<VecModel<Value>> =
        Rc::new(VecModel::from(vec![Value::Model(ModelRc::from(inner.clone()))]));
    instance.set_property("rows", Value::Model(ModelRc::from(rows.clone()))).unwrap();
    i_slint_backend_testing::mock_elapsed_time(16);
    let width =
        || -> f64 { instance.get_property("grid-preferred-width").unwrap().try_into().unwrap() };
    assert_eq!(width(), 100.);

    // Insert between the two inner cells and read the layout before the next frame.
    inner.insert(1, Value::Number(2.));
    assert_eq!(width(), 105.);

    i_slint_backend_testing::mock_elapsed_time(16);
    assert_eq!(width(), 105.);
}
