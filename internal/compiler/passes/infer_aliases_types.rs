// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Passes that resolve the type of two way bindings.
//!
//! Before this pass, two way binding that did not specified the type have Type::Void
//! type and their bindings are still a Expression::Uncompiled,
//! this pass will attempt to assign a type to these based on the type of property they alias.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::Expression;
use crate::langtype::Type;
use crate::lookup::LookupCtx;
use crate::object_tree::{Document, ElementRc};
use crate::parser::syntax_nodes;
use crate::typeregister::TypeRegister;
use std::rc::Rc;

#[derive(Clone)]
struct ComponentScope(Vec<ElementRc>);

pub fn resolve_aliases(doc: &Document, diag: &mut BuildDiagnostics) {
    for component in doc.inner_components.iter() {
        let scope = ComponentScope(vec![]);
        crate::object_tree::recurse_elem_no_borrow(
            &component.root_element,
            &scope,
            &mut |elem, scope| {
                let mut new_scope = scope.clone();
                new_scope.0.push(elem.clone());

                let mut need_resolving = vec![];
                for (prop, decl) in elem.borrow().property_declarations.iter() {
                    if matches!(decl.property_type, Type::InferredProperty | Type::InferredCallback)
                    {
                        need_resolving.push(prop.clone());
                    }
                }
                // make it deterministic
                need_resolving.sort();
                for n in need_resolving {
                    resolve_alias(elem, &n, &new_scope, &doc.local_registry, diag);
                }
                new_scope
            },
        );
    }
}

fn resolve_alias(
    elem: &ElementRc,
    prop: &str,
    scope: &ComponentScope,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let old_type = match elem.borrow_mut().property_declarations.get_mut(prop) {
        Some(decl) => {
            if !matches!(decl.property_type, Type::InferredCallback | Type::InferredProperty) {
                // already processed;
                return;
            };
            // mark the type as invalid now so that we catch recursion
            std::mem::replace(&mut decl.property_type, Type::Invalid)
        }
        None => panic!("called with not an alias?"),
    };

    let nr = match &elem.borrow().bindings[prop].borrow().expression {
        Expression::Uncompiled(node) => {
            let node = syntax_nodes::TwoWayBinding::new(node.clone())
                .expect("The parser only avoid missing types for two way bindings");
            let mut lookup_ctx = LookupCtx::empty_context(type_register, diag);
            lookup_ctx.property_name = Some(prop);
            lookup_ctx.property_type = old_type.clone();
            lookup_ctx.component_scope = &scope.0;
            crate::passes::resolving::resolve_two_way_binding(node, &mut lookup_ctx)
        }
        _ => panic!("There should be a Uncompiled expression at this point."),
    };

    let mut ty = Type::Invalid;
    if let Some(nr) = &nr {
        ty = nr.ty();
        if matches!(ty, Type::InferredCallback | Type::InferredProperty) {
            let element = nr.element();
            if Rc::ptr_eq(&element, elem) {
                resolve_alias(&element, nr.name(), scope, type_register, diag)
            } else {
                resolve_alias(&element, nr.name(), &recompute_scope(&element), type_register, diag)
            };
            ty = nr.ty();
        }
    }

    if ty == Type::Invalid && old_type == Type::InferredProperty {
        diag.push_error(
            format!("Could not infer type of property '{}'", prop),
            &elem.borrow().property_declarations[prop].type_node(),
        );
    } else {
        elem.borrow_mut().property_declarations.get_mut(prop).unwrap().property_type = ty;
    }
}

/// Recompute the scope of element
///
/// (since there is no parent mapping, we need to recursively search for the element)
fn recompute_scope(element: &ElementRc) -> ComponentScope {
    fn recurse(
        base: &ElementRc,
        needle: &ElementRc,
        scope: &mut Vec<ElementRc>,
    ) -> std::ops::ControlFlow<()> {
        scope.push(base.clone());
        if Rc::ptr_eq(base, needle) {
            return std::ops::ControlFlow::Break(());
        }
        for child in &base.borrow().children {
            if recurse(child, needle, scope).is_break() {
                return std::ops::ControlFlow::Break(());
            }
        }
        scope.pop();
        std::ops::ControlFlow::Continue(())
    }

    let root = element.borrow().enclosing_component.upgrade().unwrap().root_element.clone();
    let mut scope = Vec::new();
    recurse(&root, element, &mut scope);
    ComponentScope(scope)
}
