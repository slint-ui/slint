/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

//! This pass sets the width and heights of item which don't have their size specified
//! (neither explicitly, not in a layout)
//! The size is set according to the size in the element's DefaultSizeBinding.
//! If there is a layout within the object, the size will be a binding that depends on the layout
//! constraints.
//!
//! The pass must be run before the materialize_fake_properties pass. Since that pass needs to be
//! run before the lower_layout pass, it means we can't rely on the information from lower_layout,
//! and we will detect layouts in this pass

use std::rc::Rc;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BuiltinFunction, Expression, NamedReference};
use crate::langtype::DefaultSizeBinding;
use crate::langtype::Type;
use crate::object_tree::{Component, ElementRc};

/// Helper structure that computes the minimum and maxiumum size constraint of an element given the inner layout
#[derive(Default)]
struct ConstraintCalculator {
    /// Vector containing all the inner layout of the element.
    inner_layout: Vec<ElementRc>,
}

impl ConstraintCalculator {
    fn constrainted_binding(&self, property: &str, exp: Expression) -> Expression {
        if self.inner_layout.is_empty() {
            exp
        } else {
            let min = format!("minimum_{}", property);
            let max = format!("maximum_{}", property);
            let mut idx = 0;
            debug_assert_eq!(exp.ty(), Type::Length);
            let mut code = exp;
            for layout in self.inner_layout.iter() {
                code = Self::make_constaint(
                    code,
                    Expression::PropertyReference(NamedReference::new(layout, &min)),
                    '>',
                    idx,
                );
                code = Self::make_constaint(
                    code,
                    Expression::PropertyReference(NamedReference::new(layout, &max)),
                    '<',
                    idx + 1,
                );
                idx += 2;
            }
            code
        }
    }

    fn make_constaint(base: Expression, rhs: Expression, op: char, id: usize) -> Expression {
        let n1 = format!("minmax_lhs{}", id);
        let n2 = format!("minmax_rhs{}", id);
        let a1 = Box::new(Expression::ReadLocalVariable { name: n1.clone(), ty: Type::Length });
        let a2 = Box::new(Expression::ReadLocalVariable { name: n2.clone(), ty: Type::Length });
        Expression::CodeBlock(vec![
            Expression::StoreLocalVariable { name: n1, value: Box::new(base) },
            Expression::StoreLocalVariable { name: n2, value: Box::new(rhs) },
            Expression::Condition {
                condition: Box::new(Expression::BinaryExpression {
                    lhs: a1.clone(),
                    rhs: a2.clone(),
                    op,
                }),
                true_expr: a1,
                false_expr: a2,
            },
        ])
    }
}

pub fn default_geometry(root_component: &Rc<Component>, _diag: &mut BuildDiagnostics) {
    crate::object_tree::recurse_elem_including_sub_components(
        &root_component,
        &None,
        &mut |elem: &ElementRc, parent: &Option<ElementRc>| {
            if !is_layout(&elem.borrow().base_type)
                && !parent.as_ref().map_or(false, |p| is_layout(&p.borrow().base_type))
            {
                let base_type = elem.borrow().base_type.clone();
                if let (Some(parent), Type::Builtin(builtin_type)) = (parent, base_type) {
                    let inner_layout = elem
                        .borrow()
                        .children
                        .iter()
                        .filter(|c| is_layout(&c.borrow().base_type))
                        .cloned()
                        .collect();
                    let cc = ConstraintCalculator { inner_layout };
                    match builtin_type.default_size_binding {
                        DefaultSizeBinding::None => {}
                        DefaultSizeBinding::ExpandsToParentGeometry => {
                            make_default_100(elem, parent, "width", &cc);
                            make_default_100(elem, parent, "height", &cc);
                        }
                        DefaultSizeBinding::ImplicitSize => {
                            make_default_implicit(elem, "width", &cc);
                            make_default_implicit(elem, "height", &cc);
                        }
                    }
                }
            }
            Some(elem.clone())
        },
    )
}

/// Return true if this type is a layout that sets the geometry (width/height) of its children, and that has constraints
fn is_layout(base_type: &Type) -> bool {
    if let Type::Builtin(be) = base_type {
        match be.native_class.class_name.as_str() {
            "Row" | "GridLayout" | "HorizontalLayout" | "VerticalLayout" => true,
            "PathLayout" => true,
            _ => false,
        }
    } else {
        false
    }
}

fn make_default_100(
    elem: &ElementRc,
    parent_element: &ElementRc,
    property: &str,
    cc: &ConstraintCalculator,
) {
    elem.borrow_mut().bindings.entry(property.to_owned()).or_insert_with(|| {
        cc.constrainted_binding(
            property,
            Expression::PropertyReference(NamedReference::new(parent_element, property)),
        )
        .into()
    });
}

fn make_default_implicit(elem: &ElementRc, property: &str, cc: &ConstraintCalculator) {
    elem.borrow_mut().bindings.entry(property.into()).or_insert_with(|| {
        cc.constrainted_binding(
            property,
            Expression::StructFieldAccess {
                base: Expression::FunctionCall {
                    function: Box::new(Expression::BuiltinFunctionReference(
                        BuiltinFunction::ImplicitItemSize,
                    )),
                    arguments: vec![Expression::ElementReference(Rc::downgrade(elem))],
                    source_location: None,
                }
                .into(),
                name: property.into(),
            },
        )
        .into()
    });
}
