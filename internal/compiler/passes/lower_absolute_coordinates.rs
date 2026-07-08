// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass creates bindings to the `absolute-position` property that can be used to compute
//! the window-absolute coordinates of elements.

use std::cell::RefCell;
use std::rc::Rc;

use crate::expression_tree::{BuiltinFunction, Expression};
use crate::object_tree::Component;

pub fn lower_absolute_coordinates(component: &Rc<Component>) {
    let mut to_materialize = std::collections::HashSet::new();

    crate::object_tree::visit_all_named_references(component, &mut |nr| {
        if nr.name() == "absolute-position" {
            to_materialize.insert(nr.clone());
        }
    });

    for nr in to_materialize {
        let elem = nr.element();

        // `ItemAbsolutePosition` already returns the element's own absolute window position: it
        // maps the element's geometry origin through the ancestor transforms at runtime. We do
        // not add the element's `x`/`y` here — doing so would double-count the offset once a
        // wrapper element (Opacity/Layer/Transform) is injected around the element (the wrapper
        // takes over the element's geometry, so `map_to_window` already includes it), and it
        // would also be wrong under a parent scale/rotation. Computing everything at runtime
        // keeps it correct regardless of injected wrappers and cross-component inlining.
        //
        // The `materialize_fake_properties` pass creates the actual property later.
        let binding = Expression::FunctionCall {
            function: BuiltinFunction::ItemAbsolutePosition.into(),
            arguments: vec![Expression::ElementReference(Rc::downgrade(&elem))],
            source_location: None,
        };

        elem.borrow_mut().bindings.insert(nr.name().clone(), RefCell::new(binding.into()));
    }
}
