// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Picks, for each builtin element, the most minimal native class that still has every property
//! the element uses. A binding that just restates the builtin default isn't a use,
//! and is dropped when the selected class doesn't have the property.

use smol_str::SmolStr;
use std::collections::HashSet;
use std::sync::Arc;

use crate::expression_tree::{BindingExpression, Expression};
use crate::langtype::{BuiltinElement, BuiltinPropertyDefault, NativeClass};
use crate::object_tree::Element;

pub fn select_native_class(elem: &Element, builtin: &BuiltinElement) -> Arc<NativeClass> {
    let analysis = elem.property_analysis.borrow();
    let native_properties_used: HashSet<_> = elem
        .bindings_including_synthetic()
        .filter(|(name, binding)| !is_default_value(builtin, name, &binding.borrow()))
        .map(|(k, _)| k)
        .chain(analysis.iter().filter(|(_, v)| v.is_used()).map(|(k, _)| k))
        .filter(|k| {
            !elem.property_declarations.contains_key(*k) && builtin.properties.contains_key(*k)
        })
        .collect();

    select_minimal_class_based_on_property_usage(
        &builtin.native_class,
        native_properties_used.into_iter(),
    )
}

/// The bindings of `elem` that just restate the default of a property that `class`,
/// the class selected by [`select_native_class`], doesn't have. They're dropped.
///
/// A referenced property keeps its default: the reference materializes it in the
/// enclosing component, which is how a lowered layout still reads its own alignment.
/// Call this before lowering the element's bindings,
/// which holds a `NamedReference` to every property it visits.
pub fn dropped_defaults<'a>(
    elem: &'a Element,
    builtin: &'a BuiltinElement,
    class: &'a NativeClass,
) -> impl Iterator<Item = &'a SmolStr> {
    elem.bindings_including_synthetic().filter_map(move |(name, binding)| {
        (is_default_value(builtin, name, &binding.borrow())
            && class.lookup_property(name).is_none()
            && !elem.named_references.is_referenced(name))
        .then_some(name)
    })
}

/// Whether this binding just sets the property to the default value of the builtin
/// element declaration, so that nothing changes if it goes away.
fn is_default_value(base_type: &BuiltinElement, name: &str, binding: &BindingExpression) -> bool {
    let Some(BuiltinPropertyDefault::Expr(default)) =
        base_type.properties.get(name).map(|p| &p.default_value)
    else {
        return false;
    };
    binding.animation.is_none()
        && binding.two_way_bindings.is_empty()
        && same_literal(binding.value_expression(), &default.to_expression())
}

/// Anything that isn't a literal compares as different, so a computed binding counts as a use.
fn same_literal(a: &Expression, b: &Expression) -> bool {
    match (a, b) {
        (Expression::NumberLiteral(a, a_unit), Expression::NumberLiteral(b, b_unit)) => {
            a == b && a_unit == b_unit
        }
        (Expression::BoolLiteral(a), Expression::BoolLiteral(b)) => a == b,
        (Expression::StringLiteral(a), Expression::StringLiteral(b)) => a == b,
        (Expression::EnumerationValue(a), Expression::EnumerationValue(b)) => a == b,
        // Colors and other converted literals arrive wrapped in a cast.
        (Expression::Cast { from: a, to: a_type }, Expression::Cast { from: b, to: b_type }) => {
            a_type == b_type && same_literal(a, b)
        }
        _ => false,
    }
}

fn lookup_property_distance(mut class: Arc<NativeClass>, name: &str) -> (usize, Arc<NativeClass>) {
    let mut distance = 0;
    loop {
        if class.properties.contains_key(name)
            || (class.parent.is_none() && ["x", "y", "width", "height"].contains(&name))
        {
            return (distance, class);
        }
        distance += 1;
        class = class.parent.as_ref().unwrap().clone();
    }
}

fn select_minimal_class_based_on_property_usage<'a>(
    class: &Arc<NativeClass>,
    properties_used: impl Iterator<Item = &'a SmolStr>,
) -> Arc<NativeClass> {
    let mut minimal_class = class.clone();
    while let Some(class) = minimal_class.parent.clone() {
        minimal_class = class;
    }
    let (_min_distance, minimal_class) = properties_used.fold(
        (usize::MAX, minimal_class),
        |(current_distance, current_class), prop_name| {
            let (prop_distance, prop_class) = lookup_property_distance(class.clone(), prop_name);

            if prop_distance < current_distance {
                (prop_distance, prop_class)
            } else {
                (current_distance, current_class)
            }
        },
    );
    minimal_class
}

#[test]
fn test_select_minimal_class_based_on_property_usage() {
    use crate::langtype::{BuiltinPropertyInfo, Type};
    use smol_str::ToSmolStr;
    let first = Arc::new(NativeClass::new_with_properties(
        "first_class",
        [("first_prop".to_smolstr(), BuiltinPropertyInfo::new(Type::Int32))].iter().cloned(),
    ));

    let mut second = NativeClass::new_with_properties(
        "second_class",
        [("second_prop".to_smolstr(), BuiltinPropertyInfo::new(Type::Int32))].iter().cloned(),
    );
    second.parent = Some(first.clone());
    let second = Arc::new(second);

    let reduce_to_first =
        select_minimal_class_based_on_property_usage(&second, ["first_prop".to_smolstr()].iter());

    assert_eq!(reduce_to_first.class_name, first.class_name);

    let reduce_to_second =
        select_minimal_class_based_on_property_usage(&second, ["second_prop".to_smolstr()].iter());

    assert_eq!(reduce_to_second.class_name, second.class_name);

    let reduce_to_second = select_minimal_class_based_on_property_usage(
        &second,
        ["first_prop".to_smolstr(), "second_prop".to_smolstr()].iter(),
    );

    assert_eq!(reduce_to_second.class_name, second.class_name);
}

#[test]
fn builtin_defaults_are_comparable() {
    let tr = crate::typeregister::TypeRegister::builtin();
    let tr = tr.borrow();
    for (name, element) in tr.all_elements() {
        let crate::langtype::ElementType::Builtin(element) = element else { continue };
        for (property, info) in &element.properties {
            if let BuiltinPropertyDefault::Expr(default) = &info.default_value {
                let default = default.to_expression();
                assert!(
                    same_literal(&default, &default),
                    "the default of {name}::{property} is a shape same_literal doesn't compare, \
                     so the property always counts as used: {default:?}"
                );
            }
        }
    }
}

#[test]
fn select_minimal_class() {
    use smol_str::ToSmolStr;
    let tr = crate::typeregister::TypeRegister::builtin();
    let tr = tr.borrow();
    let rect = tr.lookup_element("Rectangle").unwrap();
    let rect = rect.as_builtin();
    assert_eq!(
        select_minimal_class_based_on_property_usage(
            &rect.native_class,
            ["x".to_smolstr(), "width".to_smolstr()].iter()
        )
        .class_name,
        "Empty",
    );
    assert_eq!(
        select_minimal_class_based_on_property_usage(&rect.native_class, [].iter()).class_name,
        "Empty",
    );
    assert_eq!(
        select_minimal_class_based_on_property_usage(
            &rect.native_class,
            ["border-width".to_smolstr()].iter()
        )
        .class_name,
        "BasicBorderRectangle",
    );
    assert_eq!(
        select_minimal_class_based_on_property_usage(
            &rect.native_class,
            ["border-width".to_smolstr(), "x".to_smolstr()].iter()
        )
        .class_name,
        "BasicBorderRectangle",
    );
    assert_eq!(
        select_minimal_class_based_on_property_usage(
            &rect.native_class,
            ["border-top-left-radius".to_smolstr(), "x".to_smolstr()].iter()
        )
        .class_name,
        "BorderRectangle",
    );
}
