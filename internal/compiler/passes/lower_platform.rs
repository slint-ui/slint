// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass lowers the access to the global Platform to constants and builtin function calls.
//!
//! Run it after `collect_globals`, so that it also visits globals,
//! and before `binding_analysis`, so that bindings using a constant `Platform.os` are constant.

use crate::expression_tree::{BuiltinFunction, Callable, Expression, NamedReference};
use crate::object_tree::{Component, visit_all_expressions};
use crate::typeregister::BUILTIN;
use std::rc::Rc;

pub fn lower_platform(component: &Rc<Component>, type_loader: &mut crate::typeloader::TypeLoader) {
    visit_all_expressions(component, |e, _| {
        e.visit_recursive_mut(&mut |e| match e {
            Expression::PropertyReference(nr) if is_platform(nr) => {
                if nr.name() == "os" {
                    *e = match &type_loader.compiler_config.const_operating_system {
                        Some(os) => Expression::EnumerationValue(
                            BUILTIN
                                .enums
                                .OperatingSystemType
                                .clone()
                                .try_value_from_string(os)
                                .unwrap_or_else(|| panic!("invalid const_operating_system '{os}'")),
                        ),
                        None => Expression::FunctionCall {
                            function: BuiltinFunction::DetectOperatingSystem.into(),
                            arguments: Vec::new(),
                            source_location: None,
                        },
                    };
                } else if nr.name() == "style-name" {
                    let style =
                        type_loader.resolved_style.strip_suffix("-dark").unwrap_or_else(|| {
                            type_loader
                                .resolved_style
                                .strip_suffix("-light")
                                .unwrap_or(&type_loader.resolved_style)
                        });
                    *e = Expression::StringLiteral(style.into());
                } else if nr.name() == "decimal-separator" {
                    *e = Expression::FunctionCall {
                        function: BuiltinFunction::DecimalSeparator.into(),
                        arguments: Vec::new(),
                        source_location: None,
                    }
                } else if nr.name() == "uses-mock-data" {
                    *e = Expression::BoolLiteral(type_loader.compiler_config.is_preview);
                }
            }
            Expression::FunctionCall { function, .. }
                if matches!(&*function, Callable::Function(nr)
                    if is_platform(nr) && nr.name() == "open-url") =>
            {
                *function = BuiltinFunction::OpenUrl.into();
            }
            Expression::FunctionCall { function, .. }
                if matches!(&*function, Callable::Function(nr)
                    if is_platform(nr) && nr.name() == "macos-bring-all-windows-to-front") =>
            {
                *function = BuiltinFunction::MacosBringAllWindowsToFront.into();
            }
            _ => {}
        })
    })
}

fn is_platform(nr: &NamedReference) -> bool {
    nr.element().borrow().builtin_type().is_some_and(|bt| bt.name == "Platform")
}
