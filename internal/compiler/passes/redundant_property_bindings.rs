// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Report bindings that set a property to the value it has without them

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{BindingExpression, Expression, Unit};
use crate::langtype::{
    BuiltinElement, BuiltinPropertyDefault, DefaultSizeBinding, ElementType, PropertyLookupMode,
};
use crate::layout::{LayoutConstraints, MergedFixedSize, Orientation};
use crate::object_tree::{
    Component, Document, Element, ElementRc, PropertyVisibility,
    recurse_elem_including_sub_components,
};
use smol_str::SmolStr;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

pub fn check_redundant_property_bindings(doc: &Document, diag: &mut BuildDiagnostics) {
    if doc.node.as_ref().and_then(|n| n.source_file()).is_some_and(|sf| sf.path().is_builtin()) {
        return;
    }
    for component in &doc.inner_components {
        let changed_by_states = changed_by_states(component);
        let mut check = |elem: &ElementRc, parent: &Option<ElementRc>| {
            check_element(elem, parent.as_ref(), &changed_by_states, diag);
            Some(elem.clone())
        };
        recurse_elem_including_sub_components(component, &None, &mut check);
    }
}

type ChangedByStates = HashSet<(*const RefCell<Element>, SmolStr)>;

fn changed_by_states(component: &Component) -> ChangedByStates {
    let mut changed = HashSet::new();
    recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
        for state in &elem.borrow().states {
            changed.extend(
                state
                    .property_changes
                    .iter()
                    .map(|(nr, ..)| (Rc::as_ptr(&nr.element()), nr.name().clone())),
            );
        }
    });
    changed
}

/// Whether another binding of the same value can replace `binding`: a binding replaces the
/// animation and the state changes along with the value.
fn is_replaceable(
    binding: &BindingExpression,
    elem: &ElementRc,
    name: &SmolStr,
    changed_by_states: &ChangedByStates,
) -> bool {
    binding.animation.is_none()
        && binding.two_way_bindings.is_empty()
        && !changed_by_states.contains(&(Rc::as_ptr(elem), name.clone()))
}

fn check_element(
    elem: &ElementRc,
    parent: Option<&ElementRc>,
    changed_by_states: &ChangedByStates,
    diag: &mut BuildDiagnostics,
) {
    if is_path_element(&elem.borrow(), parent) {
        return;
    }
    let e = elem.borrow();
    for (name, binding) in e.real_bindings() {
        let binding = binding.borrow();
        if !is_redundant(elem, parent, name, &binding, changed_by_states) {
            continue;
        }
        let pair = match name.as_str() {
            "width" => Some("height"),
            "height" => Some("width"),
            _ => None,
        };
        let pair_is_set = pair.is_some_and(|pair| {
            e.real_bindings().any(|(pair_name, pair_binding)| {
                pair_name == pair
                    && !is_redundant(
                        elem,
                        parent,
                        pair_name,
                        &pair_binding.borrow(),
                        changed_by_states,
                    )
            })
        });
        if !pair_is_set && let Some(span) = &binding.span {
            diag.push_info_with_span(
                format!("Property '{name}' is set to its default value and can be removed"),
                span.clone(),
            );
        }
    }
}

fn is_redundant(
    elem: &ElementRc,
    parent: Option<&ElementRc>,
    name: &SmolStr,
    binding: &BindingExpression,
    changed_by_states: &ChangedByStates,
) -> bool {
    let e = elem.borrow();
    let value = binding.value_expression();
    if !(is_literal(value) || matches!(value, Expression::PropertyReference(_)))
        || !is_replaceable(binding, elem, name, changed_by_states)
        || e.property_declarations.contains_key(name)
        || !e.lookup_property(name, PropertyLookupMode::ComponentLocal).is_valid()
        || super::lower_shadows::shapes_drop_shadow(&e, name)
        || (parent.is_some_and(|p| p.borrow().builtin_type().is_some_and(|b| b.name == "Dialog"))
            && name == "kind"
            && super::lower_layout::is_standard_button(&e))
    {
        return false;
    }
    sets_default(elem, name, value)
        || parent.is_some_and(|parent| {
            is_parent_size(value, parent, name) && fills_parent_by_default(elem, parent, name)
        })
}

fn is_path_element(elem: &Element, parent: Option<&ElementRc>) -> bool {
    let Some(builtin) = elem.builtin_type() else { return false };
    parent.and_then(|p| p.borrow().builtin_type()).is_some_and(|p| {
        p.name == "Path"
            && p.additional_accepted_child_types.contains_key(&builtin.native_class.class_name)
    })
}

/// Whether `value` is what `name` is on `elem` without its binding: the value the nearest base
/// component binds, the default of the type it declares, or else the builtin element's default.
/// Each style implements the widgets differently, so there is no default from a widget.
fn sets_default(elem: &ElementRc, name: &SmolStr, value: &Expression) -> bool {
    let mut base = elem.borrow().base_type.clone();
    loop {
        match base {
            ElementType::Component(component) => {
                let root = component.root_element.borrow();
                if root.source_file().is_some_and(|sf| sf.path().is_builtin()) {
                    return false;
                }
                if let Some(binding) = root.binding(name) {
                    return value.same_literal(binding.value_expression())
                        && is_replaceable(
                            &binding,
                            &component.root_element,
                            name,
                            &changed_by_states(&component),
                        );
                }
                if let Some(declaration) = root.property_declarations.get(name) {
                    return declaration.is_alias.is_none()
                        && value.same_literal(&Expression::default_value_for_type(
                            &declaration.property_type,
                        ));
                }
                base = root.base_type.clone();
            }
            ElementType::Builtin(builtin) => {
                return builtin_default(&builtin, name)
                    .or_else(|| reserved_default(elem, &builtin, name))
                    .is_some_and(|default| value.same_literal(&default));
            }
            _ => return false,
        }
    }
}

fn is_literal(e: &Expression) -> bool {
    e.same_literal(e)
}

fn builtin_default(builtin: &BuiltinElement, name: &str) -> Option<Expression> {
    let info = builtin.properties.get(name)?;
    match &info.default_value {
        BuiltinPropertyDefault::None
            if matches!(
                info.property_visibility,
                PropertyVisibility::Input | PropertyVisibility::InOut
            ) =>
        {
            Some(Expression::default_value_for_type(&info.ty))
        }
        default => default.expr_without_element(),
    }
}

/// The properties that passes like `lower_property_to_element` lower to extra elements
/// whenever they're set
fn reserved_default(elem: &ElementRc, builtin: &BuiltinElement, name: &str) -> Option<Expression> {
    match name {
        "opacity" | "visible" => super::materialize_fake_properties::initialize(elem, name),
        "cache-rendering-hint" => Some(Expression::BoolLiteral(false)),
        "clip" if builtin.name == "Rectangle" => Some(Expression::BoolLiteral(false)),
        _ => None,
    }
}

/// Whether `default_geometry` binds `name` to the parent's when it isn't set,
/// and an explicit binding adds nothing to the parent's implicit layout info
fn fills_parent_by_default(elem: &ElementRc, parent: &ElementRc, name: &str) -> bool {
    let orientation = match name {
        "width" => Orientation::Horizontal,
        "height" => Orientation::Vertical,
        _ => return false,
    };
    let elem_base = &elem.borrow().base_type;
    let ElementType::Builtin(elem_type) = elem_base else { return false };
    let ElementType::Builtin(parent_type) = &parent.borrow().base_type else { return false };
    let parent_positions_children = parent_type.default_size_binding != DefaultSizeBinding::None
        || matches!(parent_type.name.as_str(), "Empty" | "Window" | "PopupWindow");
    elem_type.default_size_binding == DefaultSizeBinding::ExpandsToParentGeometry
        && crate::layout::has_no_intrinsic_size(elem_base)
        && parent_positions_children
        && !super::flickable::is_flickable_element(parent)
        && !LayoutConstraints::build(elem, None, MergedFixedSize::Ignored)
            .has_explicit_restrictions(orientation)
}

fn is_parent_size(value: &Expression, parent: &ElementRc, name: &str) -> bool {
    match value {
        Expression::NumberLiteral(v, Unit::Percent) => (*v - 100.).abs() < 0.001,
        Expression::PropertyReference(nr) => nr.name() == name && Rc::ptr_eq(&nr.element(), parent),
        _ => false,
    }
}
