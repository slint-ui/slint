// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

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
use strum::IntoEnumIterator;

/// Generate setup code to pass window focus to the root item or a forwarded focus if applicable.
pub fn call_focus_on_init(component: &Rc<Component>) {
    if let Some(focus_call_code) =
        call_set_focus_function(&component.root_element, None, FocusFunctionType::SetFocus)
    {
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

        // Phase 3: For `focus-forward` in the root element, create `focus()` and `clear-focus()` functions that are callable from the outside
        if let Some((root_focus_forward, focus_forward_location)) =
            local_forwards.focus_forward_for_element(&component.root_element)
        {
            for function in FocusFunctionType::iter() {
                if let Some(set_or_clear_focus_code) = call_set_focus_function(
                    &root_focus_forward,
                    Some(&focus_forward_location),
                    function,
                ) {
                    component.root_element.borrow_mut().property_declarations.insert(
                        function.name().into(),
                        PropertyDeclaration {
                            property_type: Type::Function {
                                return_type: Type::Void.into(),
                                args: vec![],
                            },
                            visibility: PropertyVisibility::Public,
                            ..Default::default()
                        },
                    );
                    component.root_element.borrow_mut().bindings.insert(
                        function.name().into(),
                        RefCell::new(set_or_clear_focus_code.into()),
                    );
                }
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
                // resolve expressions pass has produced type errors
                debug_assert!(diag.has_error());
                return;
            };

            let focus_target = focus_target.upgrade().unwrap();
            let location = forward_focus_binding.to_source_location();

            if Rc::ptr_eq(elem, &focus_target) {
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
            if call_set_focus_function(target, None, FocusFunctionType::SetFocus).is_none() {
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
        let (mut focus_redirect, mut location) = self.get(element)?;

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

        for focus_function in FocusFunctionType::iter() {
            if let Expression::FunctionCall { function, arguments, .. } = expr {
                if let Expression::BuiltinFunctionReference(
                    builtin_function_type,
                    source_location,
                ) = function.as_ref()
                {
                    if *builtin_function_type != focus_function.as_builtin_function() {
                        continue;
                    }
                    if arguments.len() != 1 {
                        assert!(
                            self.diag.has_error(),
                            "Invalid argument generated for {} call",
                            focus_function.name()
                        );
                        return;
                    }
                    if let Expression::ElementReference(weak_focus_target) = &arguments[0] {
                        let mut focus_target = weak_focus_target.upgrade().expect(
                            "internal compiler error: weak focus/clear-focus parameter cannot be dangling"
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

                        if let Some(set_or_clear_focus_code) = call_set_focus_function(
                            &focus_target,
                            source_location.as_ref(),
                            focus_function,
                        ) {
                            *expr = set_or_clear_focus_code;
                        } else {
                            self.diag.push_error(
                                format!(
                                    "{}() can only be called on focusable elements",
                                    focus_function.name(),
                                ),
                                source_location,
                            );
                        }
                    }
                }
            }
        }
    }
}

#[derive(Copy, Clone, strum::EnumIter)]
enum FocusFunctionType {
    SetFocus,
    ClearFocus,
}

impl FocusFunctionType {
    fn name(&self) -> &'static str {
        match self {
            Self::SetFocus => "focus",
            Self::ClearFocus => "clear-focus",
        }
    }

    fn as_builtin_function(&self) -> BuiltinFunction {
        match self {
            Self::SetFocus => BuiltinFunction::SetFocusItem,
            Self::ClearFocus => BuiltinFunction::ClearFocusItem,
        }
    }
}

fn call_set_focus_function(
    element: &ElementRc,
    source_location: Option<&SourceLocation>,
    function_type: FocusFunctionType,
) -> Option<Expression> {
    let function_name = function_type.name();
    let declares_focus_function = {
        let mut element = element.clone();
        loop {
            if element.borrow().property_declarations.contains_key(function_name) {
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
                NamedReference::new(element, function_name),
                None,
            )),
            arguments: vec![],
            source_location: source_location.cloned(),
        })
    } else if builtin_focus_function {
        let source_location = source_location.cloned();
        Some(Expression::FunctionCall {
            function: Box::new(Expression::BuiltinFunctionReference(
                function_type.as_builtin_function(),
                source_location.clone(),
            )),
            arguments: vec![Expression::ElementReference(Rc::downgrade(element))],
            source_location,
        })
    } else {
        None
    }
}
