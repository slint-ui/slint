// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The interpreter side of `tests/cases/layout/grid_repeated_row_inner_empty_slot.slint` (#13726).

use i_slint_core::model::{ModelRc, VecModel};
use slint_interpreter::{Compiler, Value};
use std::rc::Rc;

#[test]
fn inner_empty_slot_keeps_its_grid_position() {
    i_slint_backend_testing::init_no_event_loop();
    let source =
        include_str!("../../../tests/cases/layout/grid_repeated_row_inner_empty_slot.slint");
    let result =
        spin_on::spin_on(Compiler::default().build_from_source(source.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let instance = result.component("TestCase").unwrap().create().unwrap();

    let inner = Rc::new(VecModel::from(vec![Value::Number(1.), Value::Number(1.)]));
    let outer = VecModel::from(vec![Value::Model(ModelRc::from(inner.clone()))]);
    instance.set_property("rows", Value::Model(ModelRc::new(outer))).unwrap();
    i_slint_backend_testing::mock_elapsed_time(16);

    inner.insert(1, Value::Number(2.));
    inner.insert(1, Value::Number(2.));
    i_slint_backend_testing::mock_elapsed_time(16);

    assert_eq!(instance.get_property("probe").unwrap(), Value::Number(105.));
    assert_eq!(instance.get_property("grid-preferred-width").unwrap(), Value::Number(110.));
}
