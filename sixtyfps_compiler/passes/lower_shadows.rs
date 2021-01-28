/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! Pass that lowers synthetic `drop-shadow-*` properties to proper shadow elements
// At the moment only shadows on `Rectangle` elements are supported, i.e. the drop shadow
// of a rectangle is a box shadow.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::ExpressionSpanned;
use crate::langtype::Type;
use crate::{expression_tree::Expression, object_tree::*};
use crate::{expression_tree::NamedReference, typeregister::TypeRegister};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// Creates a new element for the drop shadow properties that'll be a sibling to the specified
// sibling element.
fn create_box_shadow_element(
    shadow_property_bindings: HashMap<String, ExpressionSpanned>,
    sibling_element: &ElementRc,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) -> Option<Element> {
    if matches!(sibling_element.borrow().native_class(), Some(native)
       if native.class_name != "Rectangle" && native.class_name != "BorderRectangle" && native.class_name != "Clip")
    {
        for (shadow_prop_name, shadow_prop_binding) in shadow_property_bindings {
            diag.push_error(
                format!(
                    "The {} property is only supported on Rectangle and Clip elements right now",
                    shadow_prop_name
                ),
                &shadow_prop_binding,
            );
        }
        return None;
    }

    let mut element = Element {
        id: format!("{}_shadow", sibling_element.borrow().id),
        base_type: type_register.lookup_element("BoxShadow").unwrap(),
        enclosing_component: sibling_element.borrow().enclosing_component.clone(),
        bindings: shadow_property_bindings
            .into_iter()
            .map(|(shadow_prop_name, expr)| {
                (shadow_prop_name.strip_prefix("drop_shadow_").unwrap().to_string(), expr)
            })
            .collect(),
        ..Default::default()
    };

    // FIXME: remove the border_radius manual mapping.
    if sibling_element.borrow().bindings.contains_key("border_radius") {
        element.bindings.insert(
            "border_radius".to_string(),
            Expression::PropertyReference(NamedReference::new(sibling_element, "border_radius"))
                .into(),
        );
    }

    Some(element)
}

// For a repeated element, this function creates a new element for the drop shadow properties that
// will act as the new root element in the repeater. The former root will become a child.
fn inject_shadow_element_in_repeated_element(
    shadow_property_bindings: HashMap<String, ExpressionSpanned>,
    repeated_element: &ElementRc,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    // Since we're going to replace the repeated element's component, we need to assert that
    // outside this function no strong reference exists to it. Then we can unwrap and
    // replace the root element.
    let component = repeated_element.borrow().base_type.as_component().clone();
    debug_assert_eq!(Rc::strong_count(&component), 2);

    // Any elements with a weak reference to the repeater's component will need fixing later.
    let mut elements_with_enclosing_component_reference = Vec::new();
    recurse_elem(&component.root_element, &(), &mut |element: &ElementRc, _| {
        if let Some(enclosing_component) = element.borrow().enclosing_component.upgrade() {
            if Rc::ptr_eq(&enclosing_component, &component) {
                elements_with_enclosing_component_reference.push(element.clone());
            }
        }
    });

    let element_with_shadow_property = &component.root_element;

    let mut shadow_element = match create_box_shadow_element(
        shadow_property_bindings,
        element_with_shadow_property,
        type_register,
        diag,
    ) {
        Some(element) => element,
        None => return,
    };

    // The values for properties that affect the geometry may be supplied in two different ways:
    //
    //   * When coming from the outside, for example by the repeater being inside a layout, we need
    //     the values to apply to the shadow element and the old root just needs to follow.
    //   * When coming from the inside, for example when the repeater just creates rectangles that
    //     calculate their own position, we need to move those bindings as well to the shadow.
    //
    //  Finally we default geometry pass following this shadow lowering will apply a binding to
    //  the width and height of the inner to follow the size of the parent (the shadow).
    {
        let mut element_with_shadow_property = element_with_shadow_property.borrow_mut();
        for (binding_to_move, _) in crate::typeregister::RESERVED_GEOMETRY_PROPERTIES.iter() {
            let binding_to_move = binding_to_move.to_string();
            if let Some(binding) = element_with_shadow_property.bindings.remove(&binding_to_move) {
                shadow_element.bindings.insert(binding_to_move, binding);
            }
        }
    }

    let shadow_element = ElementRc::new(RefCell::new(shadow_element));
    elements_with_enclosing_component_reference.push(shadow_element.clone());

    // Replace the repeated component's element with our shadow element. That requires a bit of reference counting
    // surgery and relies on nobody having a strong reference left to the component, which we take out of the Rc.
    drop(std::mem::take(&mut repeated_element.borrow_mut().base_type));

    debug_assert_eq!(Rc::strong_count(&component), 1);

    let mut component = Rc::try_unwrap(component).expect("internal compiler error: more than one strong reference left to repeated component when lowering shadow properties");

    let element_with_shadow_property =
        std::mem::replace(&mut component.root_element, shadow_element.clone());
    shadow_element.borrow_mut().children.push(element_with_shadow_property);

    let component = Rc::new(component);
    repeated_element.borrow_mut().base_type = Type::Component(component.clone());

    for elem in elements_with_enclosing_component_reference {
        elem.borrow_mut().enclosing_component = Rc::downgrade(&component);
    }
}

fn take_shadow_property_bindings(element: &ElementRc) -> HashMap<String, ExpressionSpanned> {
    crate::typeregister::RESERVED_DROP_SHADOW_PROPERTIES
        .iter()
        .flat_map(|(shadow_property_name, _)| {
            let shadow_property_name = shadow_property_name.to_string();
            let mut element = element.borrow_mut();
            element.bindings.remove(&shadow_property_name).map(|binding| {
                // Remove the drop-shadow property that was also materialized as a fake property by now.
                element.property_declarations.remove(&shadow_property_name);
                (shadow_property_name, binding)
            })
        })
        .collect()
}

pub fn lower_shadow_properties(
    component: &Rc<Component>,
    type_register: &TypeRegister,
    diag: &mut BuildDiagnostics,
) {
    for (shadow_prop_name, shadow_prop_binding) in
        take_shadow_property_bindings(&component.root_element)
    {
        diag.push_error(
            format!("The {} property cannot be used on the root element", shadow_prop_name),
            &shadow_prop_binding,
        );
    }

    recurse_elem_including_sub_components_no_borrow(&component, &(), &mut |elem, _| {
        // When encountering a repeater where the repeated element has a `drop-shadow` property, we create a new
        // dedicated shadow element and make the previously reepeated element a child of that. This ensures rendering
        // underneath while maintaining the hierarchy for the repeater.
        // The geometry properties are aliased using two-way bindings (which may be eliminated in a later pass).

        if elem.borrow().repeated.is_some() {
            let component = elem.borrow().base_type.as_component().clone(); // CHECK if clone can be removed if we change borrow

            let drop_shadow_properties = take_shadow_property_bindings(&component.root_element);
            if !drop_shadow_properties.is_empty() {
                drop(component);
                inject_shadow_element_in_repeated_element(
                    drop_shadow_properties,
                    elem,
                    type_register,
                    diag,
                );
            }
        }

        let old_children = {
            let mut elem = elem.borrow_mut();
            let new_children = Vec::with_capacity(elem.children.len());
            std::mem::replace(&mut elem.children, new_children)
        };

        // When encountering a `drop-shadow` property in a supported element, we create a new dedicated
        // shadow element and insert it *before* the element that had the `drop-shadow` property, to ensure
        // that it is rendered underneath.
        for child in old_children {
            let drop_shadow_properties = take_shadow_property_bindings(&child);
            if !drop_shadow_properties.is_empty() {
                let mut shadow_elem = match create_box_shadow_element(
                    drop_shadow_properties,
                    &child,
                    type_register,
                    diag,
                ) {
                    Some(element) => element,
                    None => continue,
                };

                // Install bindings from the remaining properties of the shadow element to the
                // original, such as x/y/width/height.
                for (prop, _) in crate::typeregister::RESERVED_GEOMETRY_PROPERTIES.iter() {
                    let prop = prop.to_string();
                    if !shadow_elem.bindings.contains_key(&prop) {
                        let binding_ref = Expression::PropertyReference(NamedReference {
                            element: Rc::downgrade(&child),
                            name: prop.clone(),
                        });
                        shadow_elem.bindings.insert(prop, binding_ref.into());
                    }
                }

                elem.borrow_mut().children.push(ElementRc::new(RefCell::new(shadow_elem)));
            }
            elem.borrow_mut().children.push(child);
        }
    });
}
