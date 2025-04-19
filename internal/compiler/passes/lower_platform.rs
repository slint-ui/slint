// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pass lower the access to the global Platform to constants and builtin function calls.

use crate::expression_tree::{BuiltinFunction, Expression};
use crate::object_tree::{visit_all_expressions, Component};
use std::rc::Rc;

pub fn lower_platform(component: &Rc<Component>, type_loader: &mut crate::typeloader::TypeLoader) {
    visit_all_expressions(component, |e, _| {
        e.visit_recursive_mut(&mut |e| match e {
            Expression::PropertyReference(nr)
                if nr.element().borrow().builtin_type().is_some_and(|bt| bt.name == "Platform") =>
            {
                if nr.name() == "os" {
                    *e = Expression::FunctionCall {
                        function: BuiltinFunction::DetectOperatingSystem.into(),
                        arguments: vec![],
                        source_location: None,
                    };
                } else if nr.name() == "style-name" {
                    let style =
                        type_loader.resolved_style.strip_suffix("-dark").unwrap_or_else(|| {
                            type_loader
                                .resolved_style
                                .strip_suffix("-light")
                                .unwrap_or(&type_loader.resolved_style)
                        });
                    *e = Expression::StringLiteral(style.into()).into();
                }
            }

            _ => {}
        })
    })
}
