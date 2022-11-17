// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains the code for the highlight of some elements

use crate::dynamic_component::{ComponentBox, DynamicComponentVRc};
use crate::Value;
use i_slint_compiler::expression_tree::{Expression, Unit};
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::namedreference::NamedReference;
use i_slint_compiler::object_tree::{
    BindingsMap, Component, Document, Element, ElementRc, PropertyAnalysis, PropertyDeclaration,
    PropertyVisibility, RepeatedElementInfo,
};
use i_slint_core::items::ItemRc;
use i_slint_core::model::{ModelRc, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

const HIGHLIGHT_PROP: &str = "$highlights";

pub fn highlight(component_instance: &DynamicComponentVRc, path: PathBuf, offset: u32) {
    generativity::make_guard!(guard);
    let c = component_instance.unerase(guard);
    let elements = find_element_at_offset(&c.description().original, path, offset);
    if elements.is_empty() {
        c.description()
            .set_property(c.borrow(), HIGHLIGHT_PROP, Value::Model(ModelRc::default()))
            .unwrap();
        return;
    };

    let elements = elements.into_iter().map(|e| Rc::downgrade(&e)).collect::<Vec<_>>();

    let component_instance = vtable::VRc::downgrade(component_instance);
    let binding = move || {
        let component_instance = component_instance.upgrade().unwrap();
        generativity::make_guard!(guard);
        let c = component_instance.unerase(guard);
        let mut values = Vec::<Value>::new();
        for element in elements.iter().filter_map(|e| e.upgrade()) {
            if let Some(repeater_path) = repeater_path(&element) {
                fill_model(&repeater_path, &element, &c, &mut values);
            }
        }
        Value::Model(ModelRc::new(VecModel::from(values)))
    };

    c.description().set_binding(c.borrow(), HIGHLIGHT_PROP, Box::new(binding)).unwrap();
}

fn fill_model(
    repeater_path: &[String],
    element: &ElementRc,
    component_instance: &ComponentBox,
    values: &mut Vec<Value>,
) {
    if let [first, rest @ ..] = repeater_path {
        generativity::make_guard!(guard);
        let rep = crate::dynamic_component::get_repeater_by_name(
            component_instance.borrow_instance(),
            first.as_str(),
            guard,
        );
        for idx in 0..rep.0.len() {
            if let Some(c) = rep.0.component_at(idx) {
                generativity::make_guard!(guard);
                fill_model(rest, element, &c.unerase(guard), values);
            }
        }
    } else {
        let vrc = vtable::VRc::into_dyn(
            component_instance.borrow_instance().self_weak().get().unwrap().upgrade().unwrap(),
        );
        let index = element.borrow().item_index.get().copied().unwrap();
        let item_rc = ItemRc::new(vrc, index);

        let geom = item_rc.geometry();
        let position = item_rc.map_to_window(geom.origin);

        values.push(Value::Struct(
            [
                ("width".into(), Value::Number(geom.width() as f64)),
                ("height".into(), Value::Number(geom.height() as f64)),
                ("x".into(), Value::Number(position.x as f64)),
                ("y".into(), Value::Number(position.y as f64)),
            ]
            .into_iter()
            .collect(),
        ));
    }
}

// Go over all elements in original to find the one that is highlighted
fn find_element_at_offset(component: &Rc<Component>, path: PathBuf, offset: u32) -> Vec<ElementRc> {
    let mut result = Vec::<ElementRc>::new();
    i_slint_compiler::object_tree::recurse_elem_including_sub_components(
        component,
        &(),
        &mut |elem, &()| {
            if elem.borrow().repeated.is_some() {
                return;
            }
            if let Some(node) = elem.borrow().node.as_ref().and_then(|n| n.QualifiedName()) {
                if node.source_file.path() == path && node.text_range().contains(offset.into()) {
                    result.push(elem.clone());
                }
            }
        },
    );
    result
}

fn repeater_path(elem: &ElementRc) -> Option<Vec<String>> {
    let enclosing = elem.borrow().enclosing_component.upgrade().unwrap();
    if let Some(parent) = enclosing.parent_element.upgrade() {
        if parent.borrow().repeated.is_none() {
            // This is not a repeater, it is possibily a popup menu which is not supported ATM
            return None;
        }
        let mut r = repeater_path(&parent)?;
        r.push(parent.borrow().id.clone());
        Some(r)
    } else {
        Some(vec![])
    }
}

/// Add the `for rect in $highlights: $Highlight := Rectangle { ... }`
pub(crate) fn add_highlight_items(doc: &Document) {
    let geom_props = ["width", "height", "x", "y"];
    doc.root_component.root_element.borrow_mut().property_declarations.insert(
        HIGHLIGHT_PROP.into(),
        PropertyDeclaration {
            property_type: Type::Array(
                Type::Struct {
                    fields: geom_props
                        .iter()
                        .map(|x| (x.to_string(), Type::LogicalLength))
                        .collect(),
                    name: None,
                    node: None,
                }
                .into(),
            ),
            node: None,
            expose_in_public_api: false,
            is_alias: None,
            visibility: PropertyVisibility::Input,
        },
    );
    doc.root_component.root_element.borrow_mut().property_analysis.borrow_mut().insert(
        HIGHLIGHT_PROP.into(),
        PropertyAnalysis {
            is_set: true,
            is_set_externally: true,
            is_read: true,
            is_read_externally: true,
            is_linked_to_read_only: false,
        },
    );

    let repeated = Rc::new_cyclic(|repeated| {
        let mut bindings: BindingsMap = geom_props
            .iter()
            .map(|x| {
                (
                    x.to_string(),
                    RefCell::new(
                        Expression::StructFieldAccess {
                            base: Expression::RepeaterModelReference { element: repeated.clone() }
                                .into(),
                            name: x.to_string(),
                        }
                        .into(),
                    ),
                )
            })
            .collect();
        bindings.insert(
            "border-width".into(),
            RefCell::new(Expression::NumberLiteral(1., Unit::Px).into()),
        );
        bindings.insert(
            "border-color".into(),
            RefCell::new(
                Expression::Cast {
                    from: Expression::Cast {
                        from: Expression::NumberLiteral(0xff0000ffu32 as f64, Unit::None).into(),
                        to: Type::Color,
                    }
                    .into(),
                    to: Type::Brush,
                }
                .into(),
            ),
        );

        let base = Rc::new_cyclic(|comp| Component {
            id: "$Highlight".into(),
            parent_element: repeated.clone(),
            root_element: Rc::new(RefCell::new(Element {
                enclosing_component: comp.clone(),
                id: "$Highlight".into(),
                base_type: doc.local_registry.lookup_element("Rectangle").unwrap(),
                bindings,
                ..Default::default()
            })),
            ..Default::default()
        });

        i_slint_compiler::passes::resolve_native_classes::resolve_native_classes(&base);

        RefCell::new(Element {
            id: "$Highlight".into(),
            enclosing_component: Rc::downgrade(&doc.root_component),
            base_type: ElementType::Component(base),
            repeated: Some(RepeatedElementInfo {
                model: Expression::PropertyReference(NamedReference::new(
                    &doc.root_component.root_element,
                    HIGHLIGHT_PROP,
                )),
                model_data_id: String::default(),
                index_id: String::default(),
                is_conditional_element: false,
                is_listview: None,
            }),
            ..Default::default()
        })
    });

    doc.root_component.root_element.borrow_mut().children.push(repeated);

    // Since we added a child, we must recompute the indices in the root component
    clean_item_indices(&doc.root_component);
    for p in doc.root_component.popup_windows.borrow().iter() {
        clean_item_indices(&p.component);
    }
    i_slint_compiler::passes::generate_item_indices::generate_item_indices(&doc.root_component);
}

fn clean_item_indices(cmp: &Rc<Component>) {
    i_slint_compiler::object_tree::recurse_elem_including_sub_components(
        &cmp,
        &(),
        &mut |e, &()| {
            e.borrow_mut().item_index = Default::default();
            e.borrow_mut().item_index_of_first_children = Default::default();
        },
    );
}
