// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::cell::Cell;
use std::rc::Rc;

use i_slint_core::api::SetRenderingNotifierError;
use i_slint_core::model::VecModel;
use slint_interpreter::{ComponentHandle, ComponentInstance, RenderingState, Value};

/// The width and height of the window in [`assert_notifier_removes_elements()`].
pub const SIZE: u32 = 8;

/// Removes a `for` and an `if` element from a rendering notifier, which then calls
/// `take_snapshot()` and `set_rendering_notifier()` while it's running.
/// `render_frame` renders a frame through the renderer's notifier call sites and returns
/// the number of red pixels in its top row.
pub fn assert_notifier_removes_elements(render_frame: impl Fn(&ComponentInstance) -> usize) {
    let source = r#"
        export component TestCase inherits Window {
            width: 8px;
            height: 8px;
            background: white;
            in property <[int]> model;
            in property <bool> condition: true;
            for value[index] in root.model: Rectangle {
                x: index * 2px;
                width: 2px;
                background: red;
            }
            if root.condition: Rectangle {
                x: 6px;
                width: 1px;
                background: red;
            }
        }
    "#;
    let result = crate::testing::poll_once(
        slint_interpreter::Compiler::default().build_from_source(source.into(), Default::default()),
    )
    .unwrap();
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let component = result.components().last().unwrap().create().unwrap();
    let model = Rc::new(VecModel::from(vec![Value::from(1.), Value::from(2.), Value::from(3.)]));
    component.set_property("model", Value::Model(model.clone().into())).unwrap();

    let frame = Rc::new(Cell::new(0));
    component
        .window()
        .set_rendering_notifier({
            let frame = frame.clone();
            let component = component.as_weak();
            move |state, _| {
                if !matches!(state, RenderingState::BeforeRendering) {
                    return;
                }
                frame.set(frame.get() + 1);
                if frame.get() != 2 {
                    return;
                }
                let component = component.upgrade().unwrap();
                model.set_vec(vec![Value::from(1.)]);
                component.set_property("condition", false.into()).unwrap();
                component.window().take_snapshot().unwrap();
                assert!(matches!(
                    component.window().set_rendering_notifier(|_, _| {}),
                    Err(SetRenderingNotifierError::AlreadySet)
                ));
            }
        })
        .unwrap();
    component.show().unwrap();

    for expected_red_pixels in [7, 2, 2] {
        assert_eq!(render_frame(&component), expected_red_pixels);
    }
    assert_eq!(frame.get(), 3);
}
