// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! This module contains the code for the highlight of some elements

// cSpell: ignore unerase

use crate::dynamic_component::{ComponentBox, DynamicComponentVRc, ErasedComponentBox};
use crate::Value;
use i_slint_compiler::diagnostics::{SourceFile, Spanned};
use i_slint_compiler::expression_tree::{Expression, Unit};
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::namedreference::NamedReference;
use i_slint_compiler::object_tree::{
    BindingsMap, Component, Document, Element, ElementRc, PropertyAnalysis, PropertyDeclaration,
    PropertyVisibility, RepeatedElementInfo,
};
use i_slint_core::component::ComponentVTable;
use i_slint_core::item_tree::ItemWeak;
use i_slint_core::items::ItemRc;
use i_slint_core::lengths::LogicalPoint;
use i_slint_core::model::{ModelRc, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use vtable::VRc;

const HIGHLIGHT_PROP: &str = "$highlights";
const CURRENT_ELEMENT_CALLBACK_PROP: &str = "$currentElementCallback";
const DESIGN_MODE_PROP: &str = "$designMode";

fn next_item(item: &ItemRc) -> ItemRc {
    if let Some(s) = item.next_sibling() {
        return next_item_down(&s);
    }
    if let Some(p) = item.parent_item() {
        return p;
    } else {
        return next_item_down(item);
    }
}

fn next_item_down(item: &ItemRc) -> ItemRc {
    if let Some(child) = item.first_child() {
        return next_item_down(&child);
    } else {
        return item.clone();
    }
}

fn find_item(item: &ItemRc, position: &LogicalPoint) -> ItemRc {
    let mut next = item.clone();
    loop {
        next = next_item(&next);

        if next == *item {
            return next;
        }
        let offset = next.map_to_window(LogicalPoint::default());
        let geometry = next.geometry().translate(offset.to_vector());

        if geometry.contains(*position) {
            return next;
        }
    }
}

fn element_providing_item(component: &DynamicComponentVRc, index: usize) -> Option<ElementRc> {
    generativity::make_guard!(guard);
    let c = component.unerase(guard);

    return c.description().original_elements.get(index).cloned();
}

fn map_offset_to_line(
    source_file: Option<SourceFile>,
    start_offset: usize,
    end_offset: usize,
) -> (String, u32, u32, u32, u32) {
    if let Some(sf) = source_file {
        let file_name = sf.path().to_string_lossy().to_string();
        let (start_line, start_column) = sf.line_column(start_offset);
        let (end_line, end_column) = sf.line_column(end_offset);
        return (
            file_name,
            start_line as u32,
            start_column as u32,
            end_line as u32,
            end_column as u32,
        );
    } else {
        return (String::new(), 0, 0, 0, 0);
    }
}

struct DesignModeState {
    pub current_item: Option<ItemWeak>,
}

pub fn set_current_element_information_callback(
    component_instance: &DynamicComponentVRc,
    callback: Box<dyn Fn(String, u32, u32, u32, u32) -> ()>,
) {
    let weak_component = VRc::downgrade(component_instance);

    generativity::make_guard!(guard);
    let c = component_instance.unerase(guard);

    let _ = c.description().set_callback_handler(
        c.borrow(),
        CURRENT_ELEMENT_CALLBACK_PROP,
        Box::new(move |values: &[Value]| -> Value {
            static mut STATE: DesignModeState = DesignModeState { current_item: None };

            let position = LogicalPoint::new(
                if let Some(Value::Number(n)) = values.get(0) { *n as f32 } else { f32::MAX },
                if let Some(Value::Number(n)) = values.get(1) { *n as f32 } else { f32::MAX },
            );

            let c = if let Some(c) = weak_component.upgrade() {
                c
            } else {
                callback(String::new(), 0, 0, 0, 0);
                return Value::Void;
            };

            let start_item = unsafe { STATE.current_item.take() }
                .and_then(|i| i.upgrade())
                .unwrap_or_else(|| ItemRc::new(VRc::into_dyn(c.clone()), 0));

            let i = find_item(&start_item, &position);
            unsafe { STATE.current_item = Some(i.downgrade()) };

            if let Some((file, start_line, start_column, end_line, end_column)) =
                element_providing_item(
                    unsafe {
                        std::mem::transmute::<
                            &VRc<ComponentVTable>,
                            &VRc<ComponentVTable, ErasedComponentBox>,
                        >(&i.component())
                    },
                    i.index(),
                )
                .and_then(|e| {
                    let e = &e.borrow();
                    e.node.as_ref().map(|n| {
                        let offset = n.span().offset;
                        let length: usize = n.text().len().into();
                        map_offset_to_line(n.source_file().cloned(), offset, offset + length)
                    })
                })
            {
                callback(file, start_line, start_column, end_line, end_column);
            } else {
                callback(String::new(), 0, 0, 0, 0);
            }

            return Value::Void;
        }),
    );
}

pub fn design_mode(component_instance: &DynamicComponentVRc, active: bool) {
    generativity::make_guard!(guard);
    let c = component_instance.unerase(guard);

    c.description()
        .set_binding(c.borrow(), DESIGN_MODE_PROP, Box::new(move || active.into()))
        .unwrap();
}

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

    let component_instance = VRc::downgrade(component_instance);
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
        for idx in rep.0.range() {
            if let Some(c) = rep.0.component_at(idx) {
                generativity::make_guard!(guard);
                fill_model(rest, element, &c.unerase(guard), values);
            }
        }
    } else {
        let vrc = VRc::into_dyn(
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
            // This is not a repeater, it might be a popup menu which is not supported ATM
            return None;
        }
        let mut r = repeater_path(&parent)?;
        r.push(parent.borrow().id.clone());
        Some(r)
    } else {
        Some(vec![])
    }
}

pub(crate) fn add_highlighting(doc: &Document) {
    add_highlight_items(doc);
    add_current_item_callback(doc);

    i_slint_compiler::passes::resolve_native_classes::resolve_native_classes(&doc.root_component);

    // Since we added a child, we must recompute the indices in the root component
    clean_item_indices(&doc.root_component);
    for p in doc.root_component.popup_windows.borrow().iter() {
        clean_item_indices(&p.component);
    }
    i_slint_compiler::passes::generate_item_indices::generate_item_indices(&doc.root_component);
}

/// Add the `for rect in $highlights: $Highlight := Rectangle { ... }`
fn add_highlight_items(doc: &Document) {
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
            pure: None,
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
                base_type: doc.local_registry.lookup_builtin_element("Rectangle").unwrap(),
                bindings,
                ..Default::default()
            })),
            ..Default::default()
        });

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
}

/// Add the elements necessary to trigger the current item callback
fn add_current_item_callback(doc: &Document) {
    doc.root_component.root_element.borrow_mut().property_declarations.insert(
        CURRENT_ELEMENT_CALLBACK_PROP.into(),
        PropertyDeclaration {
            property_type: Type::Callback {
                return_type: None,
                args: vec![Type::Int32, Type::Int32],
            },
            node: None,
            expose_in_public_api: false,
            is_alias: None,
            visibility: PropertyVisibility::Private,
            pure: None,
        },
    );
    doc.root_component.root_element.borrow_mut().property_analysis.borrow_mut().insert(
        CURRENT_ELEMENT_CALLBACK_PROP.into(),
        PropertyAnalysis {
            is_set: true,
            is_set_externally: true,
            is_read: true,
            is_read_externally: true,
            is_linked_to_read_only: false,
        },
    );
    doc.root_component.root_element.borrow_mut().property_declarations.insert(
        DESIGN_MODE_PROP.into(),
        PropertyDeclaration {
            property_type: Type::Bool,
            node: None,
            expose_in_public_api: false,
            is_alias: None,
            visibility: PropertyVisibility::Input,
            pure: None,
        },
    );
    doc.root_component.root_element.borrow_mut().property_analysis.borrow_mut().insert(
        DESIGN_MODE_PROP.into(),
        PropertyAnalysis {
            is_set: true,
            is_set_externally: true,
            is_read: true,
            is_read_externally: true,
            is_linked_to_read_only: false,
        },
    );

    let element = Rc::new(RefCell::new(Element {
        enclosing_component: Rc::downgrade(&doc.root_component),
        id: "$DesignModeArea".into(),
        base_type: doc.local_registry.lookup_builtin_element("TouchArea").unwrap(),
        ..Default::default()
    }));

    let callback_prop =
        NamedReference::new(&doc.root_component.root_element, CURRENT_ELEMENT_CALLBACK_PROP);
    let request_prop = NamedReference::new(&doc.root_component.root_element, DESIGN_MODE_PROP);

    let mut bindings: BindingsMap = Default::default();
    bindings.insert("x".into(), RefCell::new(Expression::NumberLiteral(0.0, Unit::Px).into()));
    bindings.insert("y".into(), RefCell::new(Expression::NumberLiteral(0.0, Unit::Px).into()));
    bindings.insert(
        "width".into(),
        RefCell::new(
            Expression::PropertyReference(NamedReference::new(
                &doc.root_component.root_element,
                "width",
            ))
            .into(),
        ),
    );
    bindings.insert(
        "height".into(),
        RefCell::new(
            Expression::PropertyReference(NamedReference::new(
                &doc.root_component.root_element,
                "height",
            ))
            .into(),
        ),
    );
    bindings
        .insert("enabled".into(), RefCell::new(Expression::PropertyReference(request_prop).into()));
    bindings.insert(
        "clicked".into(),
        RefCell::new(
            Expression::FunctionCall {
                function: Box::new(Expression::CallbackReference(callback_prop, None)),
                arguments: vec![
                    Expression::PropertyReference(
                        NamedReference::new(&element.clone(), "pressed-x").into(),
                    ),
                    Expression::PropertyReference(
                        NamedReference::new(&element.clone(), "pressed-y").into(),
                    ),
                ],
                source_location: None,
            }
            .into(),
        ),
    );

    core::mem::swap(&mut element.borrow_mut().bindings, &mut bindings);

    doc.root_component.root_element.borrow_mut().children.push(element);
}

fn clean_item_indices(cmp: &Rc<Component>) {
    i_slint_compiler::object_tree::recurse_elem_including_sub_components(
        cmp,
        &(),
        &mut |e, &()| {
            e.borrow_mut().item_index = Default::default();
            e.borrow_mut().item_index_of_first_children = Default::default();
        },
    );
}
