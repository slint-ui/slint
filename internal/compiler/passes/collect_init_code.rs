// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Passe that collects the code from init callbacks from elements and moves it into the component's init_code.

use std::rc::Rc;

use crate::langtype::ElementType;
use crate::object_tree::{recurse_elem, Component};

pub fn collect_init_code(component: &Rc<Component>) {
    recurse_elem(&component.root_element, &(), &mut |elem, _| {
        if elem.borrow().repeated.is_some() {
            if let ElementType::Component(base) = &elem.borrow().base_type {
                if base.parent_element.upgrade().is_some() {
                    collect_init_code(base);
                }
            }
        }

        if let Some(init_callback) = elem.borrow_mut().bindings.remove("init") {
            component
                .init_code
                .borrow_mut()
                .constructor_code
                .push(init_callback.into_inner().expression);
        }
    });
}
