// Copyright © 2026 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This pass converts the sub-elements of `Particles`, such as
//!    Particles {
//!        Emitter { ... } Gravity { ... }
//!    }
//! to the `emitters` and `affectors` array properties of the `Particles` element.
//! The sub-elements keep their properties, so their bindings stay live.

use crate::diagnostics::BuildDiagnostics;
use crate::element_type::ElementType;
use crate::expression_tree::*;
use crate::expression_types::Unit;
use crate::langtype::{BuiltinStruct, EnumerationValue, Type};
use crate::namedreference::NamedReference;
use crate::object_tree::*;
use crate::typeregister::builtin_structs;
use smol_str::SmolStr;
use std::rc::Rc;

const AREA: [&str; 4] = ["x", "y", "width", "height"];

pub fn compile_particles(doc: &Document, diag: &mut BuildDiagnostics) {
    doc.visit_all_used_components(|component| {
        recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
            if elem.borrow().builtin_type().is_some_and(|bt| bt.name == "Particles") {
                compile_element(elem, diag);
            }
        });
    });
}

/// Reports repeated sub-elements, and sub-elements given to an instance of a component whose
/// `Particles` base already has some.
///
/// Runs before inlining, while the sub-elements of the base are still its children,
/// and before repeated elements become components.
pub fn check_particles(component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    recurse_elem_including_sub_components_no_borrow(component, &(), &mut |elem, _| {
        let repeated: Vec<_> = elem
            .borrow()
            .children
            .iter()
            .filter(|c| c.borrow().repeated.is_some() && sub_element(c).is_some())
            .cloned()
            .collect();
        for child in repeated {
            diag.push_error(
                "'Emitter' and affector elements can't be repeated or conditional".into(),
                &*child.borrow(),
            );
            // Out of the tree, so that later passes don't treat it as an item
            remove_lowered_element(&child, elem);
        }

        let elem = elem.borrow();
        let ElementType::Component(base) = &elem.base_type else { return };
        let has_sub_elements = |e: &Element| e.children.iter().any(|c| sub_element(c).is_some());
        if has_sub_elements(&elem)
            && base.root_element.borrow().any_in_inheritance_chain(has_sub_elements)
        {
            diag.push_error(
                "The base type already has an 'Emitter' or affector. Declare all of them in one place"
                    .into(),
                &*elem,
            );
        }
    });
}

fn compile_element(elem: &ElementRc, diag: &mut BuildDiagnostics) {
    let children: Vec<_> = elem
        .borrow()
        .children
        .iter()
        .filter_map(|child| Some((child.clone(), sub_element(child)?)))
        .collect();

    if children.is_empty() {
        let is_component_root = elem
            .borrow()
            .enclosing_component
            .upgrade()
            .is_some_and(|c| Rc::ptr_eq(&c.root_element, elem));
        // A base component may have compiled the sub-elements already,
        // and `burst()` on a component root may be called from outside.
        // Anonymous elements get an id only after this pass, and calling `burst()` needs one.
        let compiled_in_base = matches!(elem.borrow().base_type, ElementType::Component(_));
        if !compiled_in_base && !is_component_root && elem.borrow().id.is_empty() {
            diag.push_warning(
                "This 'Particles' element never shows particles. Add an 'Emitter', or call 'burst()'"
                    .into(),
                &*elem.borrow(),
            );
        }
        return;
    }

    let mut emitters = Vec::new();
    let mut affectors = Vec::new();
    for (child, sub_element) in children {
        match sub_element {
            SubElement::Emitter => {
                emitters.push(lower_sub_element(&child, BuiltinStruct::ParticleEmitter, |_| None));
            }
            SubElement::Affector(kind) => {
                let has_area = AREA.iter().any(|p| child.borrow().is_binding_set(p, true));
                affectors.push(lower_sub_element(
                    &child,
                    BuiltinStruct::ParticleAffector,
                    |field| match field {
                        "kind" => Some(Expression::EnumerationValue(kind.clone())),
                        "has-area" => Some(Expression::BoolLiteral(has_area)),
                        _ => None,
                    },
                ));
            }
        }
        move_to_optimized_elements(&child, elem);
    }

    let mut elem = elem.borrow_mut();
    for (property, ty, values) in [
        ("emitters", BuiltinStruct::ParticleEmitter, emitters),
        ("affectors", BuiltinStruct::ParticleAffector, affectors),
    ] {
        let array = Expression::Array { element_ty: builtin_structs::get(&ty).into(), values };
        elem.set_binding(SmolStr::new_static(property), array.into());
    }
}

enum SubElement {
    Emitter,
    Affector(EnumerationValue),
}

fn sub_element(child: &ElementRc) -> Option<SubElement> {
    let child = child.borrow();
    let ElementType::Builtin(builtin) = &child.base_type else { return None };
    if builtin.name == "Emitter" {
        return Some(SubElement::Emitter);
    }
    crate::typeregister::BUILTIN
        .enums
        .ParticleAffectorKind
        .clone()
        .try_value_from_string(&crate::names::to_kebab_case(&builtin.name))
        .map(SubElement::Affector)
}

/// A struct whose fields read the properties of `child` of the same name.
/// Angles become degrees and durations milliseconds, as in the runtime struct.
/// `special` gives the fields that aren't properties, and the other fields get their default.
fn lower_sub_element(
    child: &ElementRc,
    ty: BuiltinStruct,
    special: impl Fn(&str) -> Option<Expression>,
) -> Expression {
    let ty = builtin_structs::get(&ty);
    let builtin = child.borrow().builtin_type().unwrap();
    let values = ty
        .fields
        .iter()
        .map(|(field, field_ty)| {
            let value = special(field).unwrap_or_else(|| {
                let Some(prop_ty) = builtin.native_class.lookup_property(field) else {
                    return Expression::default_value_for_type(field_ty);
                };
                let reference =
                    Expression::PropertyReference(NamedReference::new(child, field.clone()));
                let unit = match prop_ty {
                    Type::Angle => Unit::Deg,
                    Type::Duration => Unit::Ms,
                    _ => return reference,
                };
                Expression::BinaryExpression {
                    lhs: Box::new(reference),
                    rhs: Box::new(Expression::NumberLiteral(1., unit)),
                    op: '/',
                    source_location: None,
                }
            });
            debug_assert_eq!(value.ty(), *field_ty, "field {field}");
            (field.clone(), value)
        })
        .collect();
    Expression::Struct { ty, values }
}
