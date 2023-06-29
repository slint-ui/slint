// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

//! This module contains the code for the highlight of some elements

// cSpell: ignore unerase

use crate::dynamic_component::{ComponentBox, DynamicComponentVRc, ErasedComponentBox};
use crate::Value;
use i_slint_compiler::diagnostics::{SourceFile, Spanned};
use i_slint_compiler::expression_tree::{Expression, Unit};
use i_slint_compiler::langtype::{ElementType, EnumerationValue, Type};
use i_slint_compiler::namedreference::NamedReference;
use i_slint_compiler::object_tree::{
    BindingsMap, Component, Document, Element, ElementRc, PropertyAnalysis, PropertyDeclaration,
    PropertyVisibility, RepeatedElementInfo,
};
use i_slint_compiler::parser::TextRange;
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

/// We do a depth-first walk of the tree.
fn next_item(item: &ItemRc) -> ItemRc {
    // We have a sibling, so find the "deepest first_child"
    if let Some(s) = item.next_sibling() {
        next_item_down(&s)
    } else if let Some(p) = item.parent_item() {
        // Walk further up the tree once out of siblings...
        p
    } else {
        // We are at the root of the tree: Start over, going for the
        // deepest first child again!
        next_item_down(item)
    }
}

fn next_item_down(item: &ItemRc) -> ItemRc {
    if let Some(child) = item.first_child() {
        next_item_down(&child)
    } else {
        item.clone()
    }
}

fn element_providing_item(component: &ErasedComponentBox, index: usize) -> Option<ElementRc> {
    generativity::make_guard!(guard);
    let c = component.unerase(guard);

    c.description().original_elements.get(index).cloned()
}

fn find_element_range(element: &ElementRc) -> Option<(Option<SourceFile>, TextRange)> {
    let e = &element.borrow();
    e.node.as_ref().and_then(|n| {
        n.parent()
            .filter(|p| p.kind() == i_slint_compiler::parser::SyntaxKind::SubElement)
            .map_or_else(
                || Some((n.source_file().cloned(), n.text_range())),
                |p| Some((p.source_file().cloned(), p.text_range())),
            )
    })
}

fn map_range_to_line(
    source_file: Option<SourceFile>,
    range: TextRange,
) -> (String, u32, u32, u32, u32) {
    source_file.map_or_else(
        || (String::new(), 0, 0, 0, 0),
        |sf| {
            let file_name = sf.path().to_string_lossy().to_string();
            let (start_line, start_column) = sf.line_column(range.start().into());
            let (end_line, end_column) = sf.line_column(range.end().into());
            (file_name, start_line as u32, start_column as u32, end_line as u32, end_column as u32)
        },
    )
}

struct DesignModeState {
    pub current_item: Option<ItemWeak>,
}

/// Use the last item we visited (if that covers the click area) or the root element
fn find_start_item(
    state: &RefCell<DesignModeState>,
    component: &DynamicComponentVRc,
    position: &LogicalPoint,
) -> ItemRc {
    state
        .try_borrow()
        .ok()
        .and_then(|s| s.current_item.clone())
        .and_then(|i| i.upgrade())
        .filter(|i| item_contains(i, position))
        .unwrap_or_else(|| ItemRc::new(VRc::into_dyn(component.clone()), 0))
}

fn item_contains(item: &ItemRc, position: &LogicalPoint) -> bool {
    let offset = item.map_to_window(LogicalPoint::default());
    let geometry = item.geometry().translate(offset.to_vector());

    geometry.contains(*position)
}

pub fn on_element_selected(
    component_instance: &DynamicComponentVRc,
    callback: Box<dyn Fn(&str, u32, u32, u32, u32)>,
) {
    generativity::make_guard!(guard);
    let c = component_instance.unerase(guard);

    let state = RefCell::new(DesignModeState { current_item: None });
    let weak_component = VRc::downgrade(component_instance);

    let _ = c.description().set_callback_handler(
        c.borrow(),
        CURRENT_ELEMENT_CALLBACK_PROP,
        Box::new(move |values: &[Value]| -> Value {
            let position = LogicalPoint::new(
                if let Some(Value::Number(n)) = values.get(0) { *n as f32 } else { f32::MAX },
                if let Some(Value::Number(n)) = values.get(1) { *n as f32 } else { f32::MAX },
            );

            let Some(c) = weak_component.upgrade() else {
                callback("", 0, 0, 0, 0);
                return Value::Void;
            };

            let start_item = find_start_item(&state, &c, &position);

            let stop_at_item = start_item.clone();
            let mut i = start_item;
            let (f, sl, sc, el, ec) = loop {
                i = next_item(&i);

                if i == stop_at_item {
                    // Break out: We went round once.
                    break (String::new(), 0, 0, 0, 0);
                }

                if !item_contains(&i, &position) {
                    continue; // wrong position
                }

                state.borrow_mut().current_item = Some(i.downgrade());

                let component = i.component();
                let component_ref = VRc::borrow(component);
                let Some(component_box) = component_ref.downcast::<ErasedComponentBox>() else {
                    continue; // Skip components of unexpected type!
                };

                let Some((file, start_line, start_column, end_line, end_column)) =
                    element_providing_item(component_box, i.index())
                    .and_then(|e| {
                        highlight_elements(&c, vec![Rc::downgrade(&e)]);
                        find_element_range(&e)
                    }).map(|(sf, r)| {
                        map_range_to_line(sf, r)
                    }) else {
                    continue; // Skip any Item not part of an element with a node attached
                };

                if file.starts_with("builtin:/") {
                    continue; // Skip builtins
                }

                break (file, start_line, start_column, end_line, end_column);
            };

            callback(&f, sl, sc, el, ec);
            Value::Void
        }),
    );
}

fn design_mode(component: &std::pin::Pin<&ComponentBox>) -> bool {
    matches!(
        component
            .description()
            .get_property(component.borrow(), DESIGN_MODE_PROP)
            .unwrap_or_default(),
        Value::Bool(true)
    )
}

pub fn set_design_mode(component_instance: &DynamicComponentVRc, active: bool) {
    generativity::make_guard!(guard);
    let c = component_instance.unerase(guard);

    c.description()
        .set_binding(c.borrow(), DESIGN_MODE_PROP, Box::new(move || active.into()))
        .unwrap();

    highlight_elements(component_instance, Vec::new());
}

fn highlight_elements(
    component: &DynamicComponentVRc,
    elements: Vec<std::rc::Weak<RefCell<Element>>>,
) {
    let component_instance = VRc::downgrade(component);
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

    generativity::make_guard!(guard);
    let c = component.unerase(guard);

    c.description().set_binding(c.borrow(), HIGHLIGHT_PROP, Box::new(binding)).unwrap();
}

pub fn highlight(component_instance: &DynamicComponentVRc, path: PathBuf, offset: u32) {
    generativity::make_guard!(guard);
    let c = component_instance.unerase(guard);

    if design_mode(&c) {
        return;
    }

    let elements = find_element_at_offset(&c.description().original, path, offset);
    if elements.is_empty() {
        c.description()
            .set_property(c.borrow(), HIGHLIGHT_PROP, Value::Model(ModelRc::default()))
            .unwrap();
        return;
    };

    highlight_elements(
        component_instance,
        elements.into_iter().map(|e| Rc::downgrade(&e)).collect::<Vec<_>>(),
    );
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
        // This is not a repeater, it might be a popup menu which is not supported ATM
        parent.borrow().repeated.as_ref()?;

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
                    rust_attributes: None,
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
    let mouse_cursor_enum =
        i_slint_compiler::typeregister::BUILTIN_ENUMS.with(|e| e.MouseCursor.clone());
    let mouse_cursor_value =
        mouse_cursor_enum.values.iter().position(|v| v.as_str() == "crosshair").unwrap();
    bindings.insert(
        "mouse-cursor".into(),
        RefCell::new(
            Expression::EnumerationValue(EnumerationValue {
                value: mouse_cursor_value,
                enumeration: mouse_cursor_enum,
            })
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
                    Expression::PropertyReference(NamedReference::new(
                        &element.clone(),
                        "pressed-x",
                    )),
                    Expression::PropertyReference(NamedReference::new(
                        &element.clone(),
                        "pressed-y",
                    )),
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
