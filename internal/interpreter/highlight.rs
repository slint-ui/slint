// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains the code for the highlight of some elements

use crate::dynamic_item_tree::{DynamicComponentVRc, ItemTreeBox};
use i_slint_compiler::object_tree::{Component, Element, ElementRc};
use i_slint_core::items::ItemRc;
use i_slint_core::lengths::LogicalRect;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use vtable::VRc;

fn normalize_repeated_element(element: ElementRc) -> ElementRc {
    if element.borrow().repeated.is_some() {
        if let i_slint_compiler::langtype::ElementType::Component(base) =
            &element.borrow().base_type
        {
            if base.parent_element.upgrade().is_some() {
                return base.root_element.clone();
            }
        }
    }

    element
}

fn collect_highlight_data(
    component: &DynamicComponentVRc,
    elements: &[std::rc::Weak<RefCell<Element>>],
) -> Vec<i_slint_core::lengths::LogicalRect> {
    let component_instance = VRc::downgrade(component);
    let component_instance = component_instance.upgrade().unwrap();
    generativity::make_guard!(guard);
    let c = component_instance.unerase(guard);
    let mut values = Vec::new();
    for element in elements.iter().filter_map(|e| e.upgrade()) {
        let element = normalize_repeated_element(element);
        if let Some(repeater_path) = repeater_path(&element) {
            fill_highlight_data(
                &repeater_path,
                &element,
                &c,
                &c,
                ElementPositionFilter::IncludeClipped,
                &mut values,
            );
        }
    }
    values
}

pub(crate) fn component_positions(
    component_instance: &DynamicComponentVRc,
    path: &Path,
    offset: u32,
) -> Vec<i_slint_core::lengths::LogicalRect> {
    generativity::make_guard!(guard);
    let c = component_instance.unerase(guard);

    let elements =
        find_element_node_at_source_code_position(&c.description().original, path, offset);
    collect_highlight_data(
        component_instance,
        &elements.into_iter().map(|(e, _)| Rc::downgrade(&e)).collect::<Vec<_>>(),
    )
}

/// Argument to filter the elements in the [`element_positions`] function
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ElementPositionFilter {
    /// Include all elements
    IncludeClipped,
    /// Exclude elements that are not visible because they are clipped
    ExcludeClipped,
}

/// Return the positions of all instances of a specific element
pub fn element_positions(
    component_instance: &DynamicComponentVRc,
    element: &ElementRc,
    filter_clipped: ElementPositionFilter,
) -> Vec<LogicalRect> {
    generativity::make_guard!(guard);
    let c = component_instance.unerase(guard);

    let mut values = Vec::new();

    let element = normalize_repeated_element(element.clone());
    if let Some(repeater_path) = repeater_path(&element) {
        fill_highlight_data(&repeater_path, &element, &c, &c, filter_clipped, &mut values);
    }
    values
}

pub(crate) fn element_node_at_source_code_position(
    component_instance: &DynamicComponentVRc,
    path: &Path,
    offset: u32,
) -> Vec<(ElementRc, usize)> {
    generativity::make_guard!(guard);
    let c = component_instance.unerase(guard);

    find_element_node_at_source_code_position(&c.description().original, path, offset)
}

fn fill_highlight_data(
    repeater_path: &[SmolStr],
    element: &ElementRc,
    component_instance: &ItemTreeBox,
    root_component_instance: &ItemTreeBox,
    filter_clipped: ElementPositionFilter,
    values: &mut Vec<i_slint_core::lengths::LogicalRect>,
) {
    if element.borrow().repeated.is_some() {
        // avoid a panic
        return;
    }

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
                    filter_clipped,
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
        if filter_clipped == ElementPositionFilter::IncludeClipped || item_rc.is_visible() {
            let geometry = item_rc.geometry();
            let origin = item_rc.map_to_item_tree(geometry.origin, &root_vrc);
            let size = geometry.size;
            values.push(LogicalRect { origin, size });
        }
    }
}

// Go over all elements in original to find the one that is highlighted
fn find_element_node_at_source_code_position(
    component: &Rc<Component>,
    path: &Path,
    offset: u32,
) -> Vec<(ElementRc, usize)> {
    let mut result = Vec::new();
    i_slint_compiler::object_tree::recurse_elem_including_sub_components(
        component,
        &(),
        &mut |elem, &()| {
            if elem.borrow().repeated.is_some() {
                return;
            }
            for (index, node_path, node_range) in
                elem.borrow().debug.iter().enumerate().map(|(i, n)| {
                    let text_range = n
                        .node
                        .QualifiedName()
                        .map(|n| n.text_range())
                        .or_else(|| {
                            n.node
                                .child_token(i_slint_compiler::parser::SyntaxKind::LBrace)
                                .map(|n| n.text_range())
                        })
                        .expect("A Element must contain a LBrace somewhere pretty early");

                    (i, n.node.source_file.path(), text_range)
                })
            {
                if node_path == path && node_range.contains(offset.into()) {
                    result.push((elem.clone(), index));
                }
            }
        },
    );
    result
}

fn repeater_path(elem: &ElementRc) -> Option<Vec<SmolStr>> {
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
