// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
    let mut borrow_mut = elem.borrow_mut();
    let old_type = match borrow_mut.property_declarations.get_mut(prop) {
        Some(decl) => {
            if !matches!(decl.property_type, Type::InferredCallback | Type::InferredProperty) {
                // already processed;
                return;
            };
            // mark the type as invalid now so that we catch recursion
            std::mem::replace(&mut decl.property_type, Type::Invalid)
        }
        None => {
            // Unresolved callback from a base component?
            debug_assert!(matches!(
                borrow_mut.lookup_property(prop).property_type,
                Type::InferredCallback | Type::InferredProperty
            ));
            // It is still unresolved because there is an error in that component
            assert!(diag.has_errors());
            return;
        }
    };
    drop(borrow_mut);

    let borrow = elem.borrow();
    let Some(binding) = borrow.bindings.get(prop) else {
        assert!(diag.has_errors());
        return;
    };
    let nr = match super::ignore_debug_hooks(&binding.borrow().expression) {
        Expression::Uncompiled(node) => {
            let Some(node) = syntax_nodes::TwoWayBinding::new(node.clone()) else {
                assert!(
                    diag.has_errors(),
                    "The parser only avoid missing types for two way bindings"
                );
                return;
            };
            let mut lookup_ctx = LookupCtx::empty_context(type_register, diag);
            lookup_ctx.property_name = Some(prop);
            lookup_ctx.property_type = old_type.clone();
            lookup_ctx.component_scope = &scope.0;
            crate::passes::resolving::resolve_two_way_binding(node, &mut lookup_ctx)
        }
        _ => panic!("There should be a Uncompiled expression at this point."),
    };
    drop(borrow);

    let mut ty = Type::Invalid;
    if let Some(nr) = &nr {
        let element = nr.element();
        let same_element = Rc::ptr_eq(&element, elem);
        if same_element && nr.name() == prop {
            diag.push_error(
                "Cannot alias to itself".to_string(),
                &elem.borrow().property_declarations[prop].type_node(),
            );
            return;
        }
        ty = nr.ty();
        if matches!(ty, Type::InferredCallback | Type::InferredProperty) {
            if same_element {
                resolve_alias(&element, nr.name(), scope, type_register, diag)
            } else {
                resolve_alias(&element, nr.name(), &recompute_scope(&element), type_register, diag)
            };
            ty = nr.ty();
        }
    }

    if old_type == Type::InferredProperty {
        if !ty.is_property_type() {
            diag.push_error(
                format!("Could not infer type of property '{prop}'"),
                &elem.borrow().property_declarations[prop].type_node(),
            );
        } else {
            elem.borrow_mut().property_declarations.get_mut(prop).unwrap().property_type = ty;
        }
    } else if old_type == Type::InferredCallback {
        if !matches!(ty, Type::Callback { .. }) {
            if nr.is_some() && ty == Type::Invalid {
                debug_assert!(diag.has_errors());
            } else {
                diag.push_error(
                    format!("Binding to callback '{prop}' must bind to another callback"),
                    &elem.borrow().property_declarations[prop].type_node(),
                );
            }
        } else {
            let nr = nr.unwrap();
            let is_global = nr.element().borrow().base_type == crate::langtype::ElementType::Global;
            let purity = nr.element().borrow().lookup_property(nr.name()).declared_pure;
            let mut elem = elem.borrow_mut();
            let decl = elem.property_declarations.get_mut(prop).unwrap();
            if decl.pure.unwrap_or(false) != purity.unwrap_or(false) {
                diag.push_error(
                    format!("Purity of callbacks '{prop}' and '{nr:?}' doesn't match"),
                    &decl.type_node(),
                );
            }
            if is_global {
                diag.push_warning("Aliases to global callback are deprecated. Export the global to access the global callback directly from native code".into(), &decl.node);
            }
            decl.property_type = ty;
        }
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
    let _ = recurse(&root, element, &mut scope);
    ComponentScope(scope)
}
