// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Report bindings that set a property to the value it has without them

use super::default_geometry;
use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::element_type::{BuiltinElement, ElementType, PropertyLookupMode};
use crate::expression_tree::{BindingExpression, Expression};
use crate::native_class::{BuiltinPropertyDefault, PropertyVisibility};
use crate::object_tree::{
    Component, Document, Element, ElementRc, recurse_elem_including_sub_components,
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
fn is_replaceable(binding: &BindingExpression, changed_by_state: bool) -> bool {
    binding.animation.is_none() && binding.two_way_bindings.is_empty() && !changed_by_state
}

fn check_element(
    elem: &ElementRc,
    parent: Option<&ElementRc>,
    changed_by_states: &ChangedByStates,
    diag: &mut BuildDiagnostics,
) {
    if is_path_element(elem, parent) {
        return;
    }
    let e = elem.borrow();
    let redundant = e
        .real_bindings()
        .filter(|(name, binding)| {
            is_redundant(elem, parent, name, &binding.borrow(), changed_by_states)
        })
        .collect::<Vec<_>>();
    for (name, binding) in &redundant {
        // `width` and `height` read as a pair: `width: 100%; height: 50%;` keeps both
        let pair = match name.as_str() {
            "width" => Some("height"),
            "height" => Some("width"),
            _ => None,
        };
        let pair_is_set = pair.is_some_and(|pair| {
            e.binding(pair).is_some() && !redundant.iter().any(|(name, _)| *name == pair)
        });
        if !pair_is_set && let Some(span) = &binding.borrow().span {
            diag.push_info_with_span(
                format!(
                    "Property '{name}' is set to its default value, its binding can be removed"
                ),
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
    let parent_size = parent.filter(|p| default_geometry::is_parent_size(value, p, name));
    if !(is_literal(value) || parent_size.is_some())
        || !is_replaceable(binding, changed_by_states.contains(&(Rc::as_ptr(elem), name.clone())))
        || e.property_declarations.contains_key(name)
        || !e.lookup_property(name, PropertyLookupMode::ComponentLocal).is_valid()
        || super::lower_shadows::shapes_drop_shadow(&e, name)
        || parent.is_some_and(|p| super::lower_layout::dialog_requires_binding(p, &e, name))
    {
        return false;
    }
    sets_default(elem, name, value)
        || parent_size.is_some_and(|p| default_geometry::fills_parent_by_default(elem, p, name))
}

fn is_path_element(elem: &ElementRc, parent: Option<&ElementRc>) -> bool {
    parent.and_then(|p| p.borrow().builtin_type()).is_some_and(|p| {
        p.name == "Path" && super::compile_paths::path_element_type(elem, &p).is_some()
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
                        && is_replaceable(&binding, component.state_changes_root_property(name));
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
        default => Expression::from_builtin_default(default, None),
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
