// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! This module contains the code for the highlight of some elements

use crate::dynamic_item_tree::{DynamicComponentVRc, ItemTreeBox};
use i_slint_compiler::object_tree::{Component, Element, ElementRc};
use i_slint_core::graphics::euclid;
use i_slint_core::items::ItemRc;
use i_slint_core::lengths::{LogicalPoint, LogicalRect};
use smol_str::SmolStr;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use vtable::VRc;

fn normalize_repeated_element(element: ElementRc) -> ElementRc {
    if element.borrow().repeated.is_some()
        && let i_slint_compiler::langtype::ElementType::Component(base) =
            &element.borrow().base_type
        && base.parent_element().is_some()
    {
        return base.root_element.clone();
    }

    element
}

/// The rectangle of an element, which may be rotated around its center
#[derive(Clone, Copy, Debug, Default)]
pub struct HighlightedRect {
    /// The element's geometry
    pub rect: LogicalRect,
    /// In degrees, around the center of the element
    pub angle: f32,
    /// Absolute origin of this instance's parent coordinate system (in root coordinates).
    ///
    /// `rect.origin - parent_origin` yields the element's position relative to its parent,
    /// which matches the `x`/`y` properties written to the source. This is computed from the
    /// instance's own ancestors, so it stays correct even if the element is positioned outside
    /// of (or with a negative offset relative to) its parent.
    ///
    /// Both values are in root coordinates, so the subtraction only recovers the source `x`/`y`
    /// while the parent frame is axis-aligned and unscaled — recovering it under a rotated or
    /// scaled ancestor would additionally need to map the delta through the inverse ancestor
    /// transform.
    pub parent_origin: LogicalPoint,
    /// Absolute rotation (in degrees) of this instance's parent coordinate system.
    ///
    /// `angle - parent_rotation` yields the element's own rotation relative to its parent, which
    /// matches the `rotation-angle`/`transform-rotation` property written to the source.
    pub parent_rotation: f32,
}
impl HighlightedRect {
    /// return true if the point is inside the (potentially rotated) rectangle
    pub fn contains(&self, position: LogicalPoint) -> bool {
        let center = self.rect.center();
        let rotation = euclid::Rotation2D::radians((-self.angle).to_radians());
        let transformed = center + rotation.transform_vector(position - center);
        self.rect.contains(transformed)
    }
}

fn collect_highlight_data(
    component: &DynamicComponentVRc,
    elements: &[std::rc::Weak<RefCell<Element>>],
) -> Vec<HighlightedRect> {
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
) -> Vec<HighlightedRect> {
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
) -> Vec<HighlightedRect> {
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
    values: &mut Vec<HighlightedRect>,
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
            if geometry.size.is_empty() {
                return;
            }
            // Injected geometry wrappers (opacity/transform/clip/... created by
            // `lower_property_to_element`) take over the element's geometry and lay the element
            // out at (0,0) inside themselves, so measuring the parent frame from the element
            // directly would collapse `rect.origin - parent_origin` to ~0.
            let description = component_instance.description();
            let item_tree = item_rc.item_tree().clone();
            let mut anchor = item_rc.clone();
            while let Some(parent) = anchor
                .parent_item(i_slint_core::item_tree::ParentItemTraversalMode::StopAtPopups)
            {
                if !VRc::ptr_eq(parent.item_tree(), &item_tree) {
                    break; // crossed into another component instance's item tree
                }
                if !description
                    .original_elements
                    .get(parent.index() as usize)
                    .is_some_and(|parent_element| parent_element.borrow().is_geometry_wrapper)
                {
                    break;
                }
                anchor = parent;
            }

            let origin = item_rc.map_to_item_tree(geometry.origin, &root_vrc);
            // `map_to_item_tree` does not add the item's own x/y, so mapping the zero point of
            // the anchor yields the absolute origin of the element's source-parent coordinate
            // system.
            let parent_origin = anchor.map_to_item_tree(LogicalPoint::default(), &root_vrc);
            // The source parent's absolute rotation: map a unit x-vector of the anchor's frame.
            // `map_to_item_tree` applies the ancestors' transforms but not the anchor's own, so
            // this excludes the element's own rotation (applied by its injected `Transform`).
            let parent_rotation = {
                let frame_x_axis =
                    anchor.map_to_item_tree(LogicalPoint::new(1.0, 0.0), &root_vrc);
                let delta = frame_x_axis - parent_origin;
                delta.y.atan2(delta.x).to_degrees()
            };
            let top_right = item_rc.map_to_item_tree(
                geometry.origin + euclid::vec2(geometry.size.width, 0.),
                &root_vrc,
            );
            let delta = top_right - origin;
            let width = delta.length();
            let height = geometry.size.height * width / geometry.size.width;
            // Compute the angle between the origin(top-right) and top-left corner
            let angle_rad = delta.y.atan2(delta.x);
            let (sin, cos) = angle_rad.sin_cos();
            let center = euclid::point2(
                origin.x + (width / 2.0) * cos - (height / 2.0) * sin,
                origin.y + (width / 2.0) * sin + (height / 2.0) * cos,
            );
            values.push(HighlightedRect {
                rect: LogicalRect {
                    origin: center - euclid::vec2(width / 2.0, height / 2.0),
                    size: euclid::size2(width, height),
                },
                angle: angle_rad.to_degrees(),
                parent_origin,
                parent_rotation,
            });
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
    if let Some(parent) = enclosing.parent_element() {
        // This is not a repeater, it might be a popup menu which is not supported ATM
        parent.borrow().repeated.as_ref()?;

        let mut r = repeater_path(&parent)?;
        r.push(parent.borrow().id.clone());
        Some(r)
    } else {
        Some(Vec::new())
    }
}
