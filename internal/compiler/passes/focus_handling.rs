// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! This pass follows the forward-focus property on the root element to determine the initial focus item
//! as well as handle the forward for `focus()` calls in code.

use std::cell::RefCell;
use std::rc::Rc;

use crate::diagnostics::{BuildDiagnostics, SourceLocation, Spanned};
use crate::expression_tree::{BuiltinFunction, Expression};
use crate::langtype::{ElementType, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::*;
use by_address::ByAddress;
use std::collections::{HashMap, HashSet};

/// Generate setup code to pass window focus to the root item or a forwarded focus if applicable.
pub fn call_focus_on_init(component: &Rc<Component>) {
    if let Some(focus_call_code) = call_focus_function(&component.root_element, None) {
        component.init_code.borrow_mut().focus_setting_code.push(focus_call_code);
    }
}

/// Remove any `forward-focus` bindings, resolve any local '.focus()' calls and create a 'focus()'
/// function on the root element if necessary.
pub fn replace_forward_focus_bindings_with_focus_functions(
    doc: &Document,
    diag: &mut BuildDiagnostics,
) {
    for component in doc.inner_components.iter() {
        // Phase 1: Collect all forward-forward bindings
        let mut local_forwards = LocalFocusForwards::collect(component, diag);

        // Phase 2: Filter out focus-forward bindings that aren't callable
        local_forwards.remove_uncallable_forwards();

        // Phase 3: For `focus-forward` in the root element, create a `focus()` function that's callable from the outside
        if let Some((root_focus_forward, focus_forward_location)) =
            local_forwards.focus_forward_for_element(&component.root_element)
        {
            if let Some(set_focus_code) =
                call_focus_function(&root_focus_forward, Some(&focus_forward_location))
            {
                component.root_element.borrow_mut().property_declarations.insert(
                    "focus".into(),
                    PropertyDeclaration {
                        property_type: Type::Function {
                            return_type: Type::Void.into(),
                            args: vec![],
                        },
                        visibility: PropertyVisibility::Public,
                        ..Default::default()
                    },
                );
                component
                    .root_element
                    .borrow_mut()
                    .bindings
                    .insert("focus".into(), RefCell::new(set_focus_code.into()));
            }
        }

        // Phase 4: All calls to `.focus()` may need to be changed with `focus-forward` resolved or changed from the built-in
        // SetFocusItem() call to a regular function call to the component's focus() function.
        visit_all_expressions(component, |e, _| {
            local_forwards.resolve_focus_calls_in_expression(e)
        });
    }
}

/// map all `forward-focus: some-target` bindings. The key is the element that had the binding,
/// the target is Some(ElementRc) if it's valid. The error remove_uncallable_forwards pass will
/// set them to None if the target is not focusable. They're not removed otherwise we'd get
/// errors on `focus()` call sites, which isn't helpful.
struct LocalFocusForwards<'a> {
    forwards:
        HashMap<ByAddress<Rc<RefCell<Element>>>, Option<(Rc<RefCell<Element>>, SourceLocation)>>,
    diag: &'a mut BuildDiagnostics,
}

impl<'a> LocalFocusForwards<'a> {
    fn collect(component: &Rc<Component>, diag: &'a mut BuildDiagnostics) -> Self {
        let mut forwards = HashMap::new();

        recurse_elem_no_borrow(&component.root_element, &(), &mut |elem, _| {
            let Some(forward_focus_binding) =
                elem.borrow_mut().bindings.remove("forward-focus").map(RefCell::into_inner)
            else {
                return;
            };

            let Expression::ElementReference(focus_target) = &forward_focus_binding.expression
            else {
                // resolve expressions pass will produce type error
                return;
            };

            let focus_target = focus_target.upgrade().unwrap();
            let location = forward_focus_binding.to_source_location();

            if Rc::ptr_eq(&elem, &focus_target) {
                diag.push_error("forward-focus can't refer to itself".into(), &location);
                return;
            }

            if forwards
                .insert(ByAddress(elem.clone()), (focus_target, location.clone()).into())
                .is_some()
            {
                diag.push_error(
                    "only one forward-focus binding can point to an element".into(),
                    &location,
                );
                return;
            }
        });

        Self { forwards, diag }
    }

    fn remove_uncallable_forwards(&mut self) {
        for target_and_location in self.forwards.values_mut() {
            let (target, source_location) = target_and_location.as_ref().unwrap();
            if call_focus_function(target, None).is_none() {
                self.diag.push_error(
                    "Cannot forward focus to unfocusable element".into(),
                    source_location,
                );
                *target_and_location = None;
            }
        }
    }

    fn get(&self, element: &ElementRc) -> Option<(ElementRc, SourceLocation)> {
        self.forwards.get(&ByAddress(element.clone())).cloned().flatten()
    }

    fn focus_forward_for_element(
        &mut self,
        element: &ElementRc,
    ) -> Option<(ElementRc, SourceLocation)> {
        let Some((mut focus_redirect, mut location)) = self.get(element) else {
            return None;
        };

        let mut visited: HashSet<ByAddress<Rc<RefCell<Element>>>> = HashSet::new();
        loop {
            if !visited.insert(ByAddress(focus_redirect.clone())) {
                self.diag.push_error("forward-focus loop".into(), &location);
                return None;
            }
            if let Some((redirect, new_location)) = self.get(&focus_redirect) {
                focus_redirect = redirect;
                location = new_location;
            } else {
                return Some((focus_redirect, location));
            }
        }
    }

    fn resolve_focus_calls_in_expression(&mut self, expr: &mut Expression) {
        expr.visit_mut(|e| self.resolve_focus_calls_in_expression(e));

        if let Expression::FunctionCall { function, arguments, .. } = expr {
            if let Expression::BuiltinFunctionReference(
                BuiltinFunction::SetFocusItem,
                source_location,
            ) = function.as_ref()
            {
                if arguments.len() != 1 {
                    panic!(
                        "internal compiler error: Invalid argument generated for SetFocusItem call"
                    );
                }
                if let Expression::ElementReference(weak_focus_target) = &arguments[0] {
                    let mut focus_target = weak_focus_target.upgrade().expect(
                        "internal compiler error: weak SetFocusItem parameter cannot be dangling",
                    );

                    if self.forwards.contains_key(&ByAddress(focus_target.clone())) {
                        let Some((next_focus_target, _)) =
                            self.focus_forward_for_element(&focus_target)
                        else {
                            // There's no need to report an additional error that focus() can't be called. Invalid
                            // forward-focus bindings have already diagnostics produced for them.
                            return;
                        };
                        focus_target = next_focus_target;
                    }

                    if let Some(set_focus_code) =
                        call_focus_function(&focus_target, source_location.as_ref())
                    {
                        *expr = set_focus_code;
                    } else {
                        self.diag.push_error(
                            "focus() can only be called on focusable elements".into(),
                            source_location,
                        );
                    }
                }
            }
        }
    }
}

fn call_focus_function(
    element: &ElementRc,
    source_location: Option<&SourceLocation>,
) -> Option<Expression> {
    let declares_focus_function = {
        let mut element = element.clone();
        loop {
            if element.borrow().property_declarations.contains_key("focus") {
                break true;
            }
            let base = element.borrow().base_type.clone();
            match base {
                ElementType::Component(compo) => element = compo.root_element.clone(),
                _ => break false,
            }
        }
    };
    let builtin_focus_function =
        element.borrow().builtin_type().map_or(false, |ty| ty.accepts_focus);

    if declares_focus_function {
        Some(Expression::FunctionCall {
            function: Box::new(Expression::FunctionReference(
                NamedReference::new(&element, "focus"),
                None,
            )),
            arguments: vec![],
            source_location: source_location.cloned(),
        })
    } else if builtin_focus_function {
        let source_location = source_location.cloned();
        Some(Expression::FunctionCall {
            function: Box::new(Expression::BuiltinFunctionReference(
                BuiltinFunction::SetFocusItem,
                source_location.clone(),
            )),
            arguments: vec![Expression::ElementReference(Rc::downgrade(&element))],
            source_location,
        })
    } else {
        None
    }
}
