// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! This module contains the code for the highlight of some elements

use crate::dynamic_item_tree::{DynamicComponentVRc, ItemTreeBox};
use i_slint_compiler::object_tree::{Component, Element, ElementRc};
use i_slint_core::items::ItemRc;
use i_slint_core::lengths::LogicalRect;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use vtable::VRc;

/// The kind of Element examined.
pub enum ComponentKind {
    /// The component is actually a layout
    Layout,
    /// The component is actually an element
    Element,
}

/// Positions of the Element in the UI
#[derive(Default)]
pub struct ComponentPositions {
    /// The kind of element looked at
    pub kind: Option<ComponentKind>,
    /// The geometry information of all occurrences of this element in the UI
    pub geometries: Vec<i_slint_core::lengths::LogicalRect>,
}

fn collect_highlight_data(
    component: &DynamicComponentVRc,
    elements: &[std::rc::Weak<RefCell<Element>>],
) -> ComponentPositions {
    let component_instance = VRc::downgrade(component);
    let component_instance = component_instance.upgrade().unwrap();
    generativity::make_guard!(guard);
    let c = component_instance.unerase(guard);
    let mut values = ComponentPositions::default();
    for element in elements.iter().filter_map(|e| e.upgrade()) {
        if let Some(repeater_path) = repeater_path(&element) {
            fill_highlight_data(&repeater_path, &element, &c, &c, &mut values);
        }
    }
    values
}

pub(crate) fn component_positions(
    component_instance: &DynamicComponentVRc,
    path: PathBuf,
    offset: u32,
) -> ComponentPositions {
    generativity::make_guard!(guard);
    let c = component_instance.unerase(guard);

    let elements = find_element_at_offset(&c.description().original, path, offset.into());
    collect_highlight_data(
        component_instance,
        &elements.into_iter().map(|e| Rc::downgrade(&e)).collect::<Vec<_>>(),
    )
}

pub(crate) fn element_position(
    component_instance: &DynamicComponentVRc,
    element: &ElementRc,
) -> Option<LogicalRect> {
    generativity::make_guard!(guard);
    let c = component_instance.unerase(guard);

    let mut values = ComponentPositions::default();
    if let Some(repeater_path) = repeater_path(element) {
        fill_highlight_data(&repeater_path, &element, &c, &c, &mut values);
    }
    values.geometries.get(0).cloned()
}

fn fill_highlight_data(
    repeater_path: &[String],
    element: &ElementRc,
    component_instance: &ItemTreeBox,
    root_component_instance: &ItemTreeBox,
    values: &mut ComponentPositions,
) {
    if let [first, rest @ ..] = repeater_path {
        generativity::make_guard!(guard);
        let rep = crate::dynamic_item_tree::get_repeater_by_name(
            component_instance.borrow_instance(),
            first.as_str(),
            guard,
        );
        for idx in rep.0.range() {
            if let Some(c) = rep.0.instance_at(idx) {
                generativity::make_guard!(guard);
                fill_highlight_data(
                    rest,
                    element,
                    &c.unerase(guard),
                    root_component_instance,
                    values,
                );
            }
        }
    } else {
        let vrc = VRc::into_dyn(
            component_instance.borrow_instance().self_weak().get().unwrap().upgrade().unwrap(),
        );
        let root_vrc = VRc::into_dyn(
            root_component_instance.borrow_instance().self_weak().get().unwrap().upgrade().unwrap(),
        );
        let index = element.borrow().item_index.get().copied().unwrap();
        let item_rc = ItemRc::new(vrc.clone(), index);
        let geometry = item_rc.geometry();
        let origin = item_rc.map_to_item_tree(geometry.origin, &root_vrc);
        let size = geometry.size;

        if values.kind.is_none() {
            values.kind = if element.borrow().layout.is_some() {
                Some(ComponentKind::Layout)
            } else {
                Some(ComponentKind::Element)
            };
        }

        values.geometries.push(LogicalRect { origin, size });
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
