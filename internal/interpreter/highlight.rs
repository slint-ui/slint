// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains the code for the highlight of some elements

use crate::dynamic_component::ComponentBox;
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

pub fn highlight(component_instance: &ComponentBox, path: PathBuf, offset: u32) {
    let element = if let Some(element) =
        find_element(&component_instance.description().original, path, offset)
    {
        element
    } else {
        component_instance
            .description()
            .set_property(
                component_instance.borrow(),
                HIGHLIGHT_PROP,
                Value::Model(ModelRc::default()),
            )
            .unwrap();
        return;
    };

    //let item_path = find_item_for_element(element.clone(), component_instance.description());
    let mut values = Vec::<Value>::new();
    let repeater_path = repeater_path(&element);

    eprintln!("--> {repeater_path:?},  {:?}", element.borrow().id);

    fill_model(&repeater_path, &element, component_instance, &mut values);

    component_instance
        .description()
        .set_property(
            component_instance.borrow(),
            HIGHLIGHT_PROP,
            Value::Model(ModelRc::new(VecModel::from(values))),
        )
        .unwrap();
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
        let mut position = geom.origin;
        let mut parent_item = item_rc.clone();
        loop {
            parent_item = match parent_item.parent_item() {
                None => break,
                Some(pi) => pi,
            };
            position += parent_item.borrow().as_ref().geometry().origin.to_vector();
        }

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
/*
fn fill_model(element: ElementRc, component_instance: &ComponentBox, values: &mut Vec<Value>) {
    let enclosing = element.borrow().enclosing_component.upgrade().unwrap();
    if Rc::ptr_eq(&enclosing, &component_instance.description().original) {
        component_instance.description().items[element.borrow().id.as_str()];
    }
    if let Some(parent) = enclosing.parent_element.upgrade() {
        generativity::make_guard!(guard);
        let rep = crate::dynamic_component::get_repeater_by_name(
            component_instance.borrow_instance(),
            &element.borrow().id,
            guard,
        );
        for idx in 0..rep.0.len() {
            if let Some(c) = rep.0.component_at(idx) {
                generativity::make_guard!(guard);
                fill_model(element, component_instance, values);
            }

        }
    }

    todo!()
}
*/
/*fn fill_model(repeater_idx: &[usize], component_instance: &ComponentBox, values: &mut Vec<Value>) {
    if let [first, rest @ ..] = repeater_idx {
        generativity::make_guard!(guard):
        let rep = component_instance.description().repeater[*first].unerase(guard);
        rep.offset
        component_instance.borrow_instance().instance
        fill_model(repeater_idx, component_instance, values)
    }
    let v = Value::Struct(
        ["width", "height", "x", "y"].iter().map(|x| (x.to_string(), Value::Number(50.))).collect(),
    );
}*/

// Go over all elements in original to find the one that is highlighted
fn find_element(component: &Rc<Component>, path: PathBuf, offset: u32) -> Option<ElementRc> {
    let mut result = None;
    i_slint_compiler::object_tree::recurse_elem_including_sub_components(
        component,
        &(),
        &mut |elem, &()| {
            if elem.borrow().repeated.is_some() {
                return;
            }
            if let Some(node) = &elem.borrow().node {
                if node.source_file.path() == path && node.text_range().contains(offset.into()) {
                    result = Some(elem.clone());
                }
            }
        },
    );
    result
}

/*
struct ItemPath {
    repeater_idx: Vec<usize>,
    item_id: String,
}

fn find_item_for_element(elem: ElementRc, des: Rc<ComponentDescription>) -> ItemPath {
    generativity::make_guard!(guard);
    let enclosing = elem.borrow().enclosing_component.upgrade().unwrap();
    let repeater_idx = if let Some(parent) = enclosing.parent_element.upgrade() {
        find_repeater_path(parent, des.clone(), guard).0
    } else {
        vec![]
    };
    ItemPath { item_id: elem.borrow().id.clone(), repeater_idx }
}

/// return the path of repeater and the parent's description
fn find_repeater_path<'id>(
    repeated: ElementRc,
    des: Rc<ComponentDescription>,
    guard: generativity::Guard<'id>,
) -> (Vec<usize>, Rc<ComponentDescription<'id>>) {
    let enclosing = repeated.borrow().enclosing_component.upgrade().unwrap();
    if let Some(parent) = enclosing.parent_element.upgrade() {
        generativity::make_guard!(guard_2);
        let (mut path, des) = find_repeater_path(parent, des, guard_2);
        let idx = des.repeater_names[repeated.borrow().id.as_str()];
        path.push(idx);
        (path, des.repeater[idx].unerase(guard).component_to_repeat.clone())
    } else {
        let idx = des.repeater_names[repeated.borrow().id.as_str()];
        (vec![idx], des.repeater[idx].unerase(guard).component_to_repeat.clone())
    }
}*/

fn repeater_path(elem: &ElementRc) -> Vec<String> {
    let enclosing = elem.borrow().enclosing_component.upgrade().unwrap();
    if let Some(parent) = enclosing.parent_element.upgrade() {
        let mut r = repeater_path(&parent);
        r.push(parent.borrow().id.clone());
        r
    } else {
        vec![]
    }
}

/// Add the `for rect in $highlights: $highlights := Rectangle { }`
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
                id: "$Highlight_root".into(),
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
