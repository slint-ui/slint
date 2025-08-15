// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass transforms the PopupWindow element into a component

use crate::diagnostics::{BuildDiagnostics, SourceLocation};
use crate::expression_tree::{BindingExpression, Expression, NamedReference};
use crate::langtype::{ElementType, EnumerationValue, Type};
use crate::object_tree::*;
use crate::typeregister::TypeRegister;
use smol_str::{format_smolstr, SmolStr};
use std::rc::{Rc, Weak};

const CLOSE_ON_CLICK: &str = "close-on-click";
const CLOSE_POLICY: &str = "close-policy";

pub fn lower_popups(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    let window_type = type_register.lookup_builtin_element("Window").unwrap();

    recurse_elem_including_sub_components_no_borrow(
        component,
        &None,
        &mut |elem, parent_element: &Option<ElementRc>| {
            if is_popup_window(elem) {
                lower_popup_window(elem, parent_element.as_ref(), &window_type, diag);
            }
            Some(elem.clone())
        },
    )
}

pub fn is_popup_window(element: &ElementRc) -> bool {
    match &element.borrow().base_type {
        ElementType::Builtin(base_type) => base_type.name == "PopupWindow",
        ElementType::Component(base_type) => base_type.inherits_popup_window.get(),
        _ => false,
    }
}

fn lower_popup_window(
    popup_window_element: &ElementRc,
    parent_element: Option<&ElementRc>,
    window_type: &ElementType,
    diag: &mut BuildDiagnostics,
) {
    if let Some(binding) = popup_window_element.borrow().bindings.get(CLOSE_ON_CLICK) {
        if popup_window_element.borrow().bindings.contains_key(CLOSE_POLICY) {
            diag.push_error(
                "close-policy and close-on-click cannot be set at the same time".into(),
                &binding.borrow().span,
            );
        } else {
            diag.push_property_deprecation_warning(
                CLOSE_ON_CLICK,
                CLOSE_POLICY,
                &binding.borrow().span,
            );
            if !matches!(
                super::ignore_debug_hooks(&binding.borrow().expression),
                Expression::BoolLiteral(_)
            ) {
                report_const_error(CLOSE_ON_CLICK, &binding.borrow().span, diag);
            }
        }
    } else if let Some(binding) = popup_window_element.borrow().bindings.get(CLOSE_POLICY) {
        if !matches!(
            super::ignore_debug_hooks(&binding.borrow().expression),
            Expression::EnumerationValue(_)
        ) {
            report_const_error(CLOSE_POLICY, &binding.borrow().span, diag);
        }
    }

    let parent_component = popup_window_element.borrow().enclosing_component.upgrade().unwrap();
    let parent_element = match parent_element {
        None => {
            if matches!(popup_window_element.borrow().base_type, ElementType::Builtin(_)) {
                popup_window_element.borrow_mut().base_type = window_type.clone();
            }
            parent_component.inherits_popup_window.set(true);
            return;
        }
        Some(parent_element) => parent_element,
    };

    if Rc::ptr_eq(&parent_component.root_element, popup_window_element) {
        diag.push_error(
            "PopupWindow cannot be directly repeated or conditional".into(),
            &*popup_window_element.borrow(),
        );
        return;
    }

    // Remove the popup_window_element from its parent
    let mut parent_element_borrowed = parent_element.borrow_mut();
    let index = parent_element_borrowed
        .children
        .iter()
        .position(|child| Rc::ptr_eq(child, popup_window_element))
        .expect("PopupWindow must be a child of its parent");
    parent_element_borrowed.children.remove(index);
    parent_element_borrowed.has_popup_child = true;
    drop(parent_element_borrowed);
    if let Some(parent_cip) = &mut *parent_component.child_insertion_point.borrow_mut() {
        if Rc::ptr_eq(&parent_cip.parent, parent_element) && parent_cip.insertion_index > index {
            parent_cip.insertion_index -= 1;
        }
    }

    if matches!(popup_window_element.borrow().base_type, ElementType::Builtin(_)) {
        popup_window_element.borrow_mut().base_type = window_type.clone();
    }

    let map_close_on_click_value = |b: &BindingExpression| {
        let Expression::BoolLiteral(v) = super::ignore_debug_hooks(&b.expression) else {
            assert!(diag.has_errors());
            return None;
        };
        let enum_ty = crate::typeregister::BUILTIN.with(|e| e.enums.PopupClosePolicy.clone());
        let s = if *v { "close-on-click" } else { "no-auto-close" };
        Some(EnumerationValue {
            value: enum_ty.values.iter().position(|v| v == s).unwrap(),
            enumeration: enum_ty,
        })
    };

    let close_policy =
        popup_window_element.borrow_mut().bindings.remove(CLOSE_POLICY).and_then(|b| {
            let b = b.into_inner();
            if let Expression::EnumerationValue(v) = super::ignore_debug_hooks(&b.expression) {
                Some(v.clone())
            } else {
                assert!(diag.has_errors());
                None
            }
        });
    let close_policy = close_policy
        .or_else(|| {
            popup_window_element
                .borrow_mut()
                .bindings
                .remove(CLOSE_ON_CLICK)
                .and_then(|b| map_close_on_click_value(&b.borrow()))
        })
        .or_else(|| {
            // check bases
            let mut base = popup_window_element.borrow().base_type.clone();
            while let ElementType::Component(b) = base {
                let base_policy = b
                    .root_element
                    .borrow()
                    .bindings
                    .get(CLOSE_POLICY)
                    .and_then(|b| {
                        let b = b.borrow();
                        if let Expression::EnumerationValue(v) = &b.expression {
                            return Some(v.clone());
                        }
                        assert!(diag.has_errors());
                        None
                    })
                    .or_else(|| {
                        b.root_element
                            .borrow()
                            .bindings
                            .get(CLOSE_ON_CLICK)
                            .and_then(|b| map_close_on_click_value(&b.borrow()))
                    });
                if let Some(base_policy) = base_policy {
                    return Some(base_policy);
                }
                base = b.root_element.borrow().base_type.clone();
            }
            None
        })
        .unwrap_or_else(|| EnumerationValue {
            value: 0,
            enumeration: crate::typeregister::BUILTIN.with(|e| e.enums.PopupClosePolicy.clone()),
        });

    let popup_comp = Rc::new(Component {
        root_element: popup_window_element.clone(),
        parent_element: Rc::downgrade(parent_element),
        ..Component::default()
    });

    let weak = Rc::downgrade(&popup_comp);
    recurse_elem(&popup_comp.root_element, &(), &mut |e, _| {
        e.borrow_mut().enclosing_component = weak.clone()
    });

    // Take a reference to the x/y coordinates, to be read when calling show_popup(), and
    // converted to absolute coordinates in the run-time library.
    let coord_x = NamedReference::new(&popup_comp.root_element, SmolStr::new_static("x"));
    let coord_y = NamedReference::new(&popup_comp.root_element, SmolStr::new_static("y"));

    // Meanwhile, set the geometry x/y to zero, because we'll be shown as a top-level and
    // children should be rendered starting with a (0, 0) offset.
    {
        let mut popup_mut = popup_comp.root_element.borrow_mut();
        let name = format_smolstr!("popup-{}-dummy", popup_mut.id);
        popup_mut.property_declarations.insert(name.clone(), Type::LogicalLength.into());
        drop(popup_mut);
        let dummy1 = NamedReference::new(&popup_comp.root_element, name.clone());
        let dummy2 = NamedReference::new(&popup_comp.root_element, name.clone());
        let mut popup_mut = popup_comp.root_element.borrow_mut();
        popup_mut.geometry_props.as_mut().unwrap().x = dummy1;
        popup_mut.geometry_props.as_mut().unwrap().y = dummy2;
    }

    // Throw error when accessing the popup from outside
    // FIXME:
    // - the span is the span of the PopupWindow, that's wrong, we should have the span of the reference
    // - There are other object reference than in the NamedReference
    // - Maybe this should actually be allowed
    visit_all_named_references(&parent_component, &mut |nr| {
        let element = &nr.element();
        if check_element(element, &weak, diag, popup_window_element) {
            // just set it to whatever is a valid NamedReference, otherwise we'll panic later
            *nr = coord_x.clone();
        }
    });
    visit_all_expressions(&parent_component, |exp, _| {
        exp.visit_recursive_mut(&mut |exp| {
            if let Expression::ElementReference(ref element) = exp {
                let elem = element.upgrade().unwrap();
                if !Rc::ptr_eq(&elem, popup_window_element) {
                    check_element(&elem, &weak, diag, popup_window_element);
                }
            }
        });
    });

    super::focus_handling::call_focus_on_init(&popup_comp);

    parent_component.popup_windows.borrow_mut().push(PopupWindow {
        component: popup_comp,
        x: coord_x,
        y: coord_y,
        close_policy,
        parent_element: parent_element.clone(),
    });
}

fn report_const_error(prop: &str, span: &Option<SourceLocation>, diag: &mut BuildDiagnostics) {
    diag.push_error(format!("The {prop} property only supports constants at the moment"), span);
}

fn check_element(
    element: &ElementRc,
    popup_comp: &Weak<Component>,
    diag: &mut BuildDiagnostics,
    popup_window_element: &ElementRc,
) -> bool {
    if Weak::ptr_eq(&element.borrow().enclosing_component, popup_comp) {
        diag.push_error(
            "Cannot access the inside of a PopupWindow from enclosing component".into(),
            &*popup_window_element.borrow(),
        );
        true
    } else {
        false
    }
}
