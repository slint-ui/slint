// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Highlight support for running component instances.
//!
//! Walks the LLR `debug_info` side table to map either a source location
//! or an object-tree `ElementRc` back to runtime flat item indices, then
//! reads geometries via `ItemRc::geometry()` and transforms them through
//! `map_to_item_tree`.

use crate::instance::Instance;
use i_slint_compiler::llr::{ItemInstanceIdx, SubComponentIdx, SubComponentInstanceIdx};
use i_slint_compiler::object_tree::ElementRc;
use i_slint_core::graphics::euclid;
use i_slint_core::item_tree::ItemTreeVTable;
use i_slint_core::items::ItemRc;
use i_slint_core::lengths::{LogicalPoint, LogicalRect};
use std::path::Path;
use vtable::VRc;

/// The rectangle of an element, which may be rotated around its center.
#[derive(Clone, Copy, Debug, Default)]
pub struct HighlightedRect {
    /// The element's geometry.
    pub rect: LogicalRect,
    /// In degrees, around the center of the element.
    pub angle: f32,
}
impl HighlightedRect {
    /// Returns true if `position` lies inside the (potentially rotated) rectangle.
    pub fn contains(&self, position: LogicalPoint) -> bool {
        let center = self.rect.center();
        let rotation = euclid::Rotation2D::radians((-self.angle).to_radians());
        let transformed = center + rotation.transform_vector(position - center);
        self.rect.contains(transformed)
    }
}

/// Argument to filter the elements returned by the highlight helpers.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ElementPositionFilter {
    /// Include all elements.
    IncludeClipped,
    /// Exclude elements clipped by an ancestor `Clip` / `Flickable`.
    ExcludeClipped,
}

/// Return the screen rectangles of every runtime item matching the
/// given `ElementRc`, optionally filtering out those clipped by an
/// ancestor. Public so downstream tooling (LSP element selection) can
/// mirror `Rust`'s `highlight::element_positions` semantics, including
/// the `ExcludeClipped` filter that hit-testing needs.
pub fn element_positions(
    instance: &VRc<ItemTreeVTable, Instance>,
    element: &ElementRc,
    filter: ElementPositionFilter,
) -> Vec<HighlightedRect> {
    // Match by the element's source location: the LLR copies the same
    // `source_location` onto every item it lowers, and the object tree
    // element keeps the original node. `element_hash` would be more
    // compact but depends on the `inject_debug_hooks` pass, which
    // doesn't always reach every element — layout lowering, property
    // hoisting, etc. create new elements after the pass — so
    // source-location comparison is more reliable.
    //
    // For a user-level component instantiation (e.g. `Button { }` in
    // source), the element itself has no native item — the runtime items
    // live inside the wrapped sub-component. Walk into `base_type` until
    // we land on an element the LLR actually lowered.
    let target = walk_to_native_root(element);
    let Some(debug_first) = target.borrow().debug.first().cloned() else {
        return Vec::new();
    };
    let path = debug_first.node.source_file.path().to_path_buf();
    let offset: u32 = debug_first.node.text_range().start().into();
    positions_by_source(instance, &path, offset, filter)
}

/// Descend into `base_type = Component(_)` wrappers until the element
/// points at a native item. `Button { }` in source yields an object-tree
/// element whose `debug[0]` sits at the user's call site but whose
/// concrete runtime items belong to the wrapped component — chase the
/// chain so our source-location lookup lands on a real LLR item.
fn walk_to_native_root(element: &ElementRc) -> ElementRc {
    let mut current = element.clone();
    loop {
        let next = {
            let b = current.borrow();
            if let i_slint_compiler::langtype::ElementType::Component(c) = &b.base_type {
                Some(c.root_element.clone())
            } else {
                None
            }
        };
        match next {
            Some(n) => current = n,
            None => return current,
        }
    }
}

/// Return the geometry of every runtime item whose source location covers
/// the given `(path, offset)` pair.
pub(crate) fn component_positions(
    instance: &VRc<ItemTreeVTable, Instance>,
    path: &Path,
    offset: u32,
) -> Vec<HighlightedRect> {
    let cu = &instance.root_sub_component.compilation_unit;
    let mut results = Vec::new();
    for sc_idx in 0..cu.sub_components.len() {
        let sc_idx: SubComponentIdx = sc_idx.into();
        let sc = &cu.sub_components[sc_idx];
        let Some(debug) = sc.debug_info.as_ref() else { continue };
        for (local_idx, item_dbg) in debug.items.iter_enumerated() {
            let Some(loc_path) = item_dbg.source_location.source_file.as_ref().map(|f| f.path())
            else {
                continue;
            };
            if loc_path != path {
                continue;
            }
            let span_start = item_dbg.source_location.span.offset as u32;
            if span_start > offset {
                continue;
            }
            for flat_idx in find_flat_indices_for_item(instance, sc_idx, local_idx) {
                if let Some(rect) = item_flat_index_to_rect(instance, flat_idx) {
                    results.push(rect);
                }
            }
        }
    }
    results
}

/// Look up the `(ElementRc, index)` tuples whose `debug` entries cover
/// the given source offset. Uses the `TypeLoader` stored on the instance
/// (if available) to walk the original object-tree `Document`.
pub(crate) fn element_node_at_source_code_position(
    instance: &VRc<ItemTreeVTable, Instance>,
    path: &Path,
    offset: u32,
) -> Vec<(ElementRc, usize)> {
    let Some(type_loader) = instance.type_loaders.type_loader.as_ref() else {
        return Vec::new();
    };
    let Some(doc) = type_loader.get_document(path) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    // `inner_components` contains every component defined in the file
    // (both exported and non-exported), so iterating it covers all
    // source locations without duplication.
    for component in &doc.inner_components {
        find_element_node_at_source_code_position(component, path, offset, &mut result);
    }
    result
}

fn find_element_node_at_source_code_position(
    component: &std::rc::Rc<i_slint_compiler::object_tree::Component>,
    path: &Path,
    offset: u32,
    result: &mut Vec<(ElementRc, usize)>,
) {
    // Use plain recurse_elem (no sub-component descent) because we already
    // iterate all components in the document individually.
    i_slint_compiler::object_tree::recurse_elem(&component.root_element, &(), &mut |elem, &()| {
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
                    .expect("An Element must contain a LBrace somewhere");
                (i, n.node.source_file.path(), text_range)
            })
        {
            if node_path == path && node_range.contains(offset.into()) {
                result.push((elem.clone(), index));
            }
        }
    });
}

/// Scan the instance's flat `item_table` and return every flat index
/// whose entry points at `(sub_component_path → target_sc_idx, target_local)`.
fn find_flat_indices_for_item(
    instance: &VRc<ItemTreeVTable, Instance>,
    target_sc_idx: SubComponentIdx,
    target_local: ItemInstanceIdx,
) -> Vec<usize> {
    let cu = &instance.root_sub_component.compilation_unit;
    let mut out = Vec::new();
    for (flat, entry) in instance.item_table.iter().enumerate() {
        let Some((path, local_idx)) = entry.as_ref() else { continue };
        if *local_idx != target_local {
            continue;
        }
        if sub_component_idx_at_path(cu, instance.root_sub_component.sub_component_idx, path)
            == target_sc_idx
        {
            out.push(flat);
        }
    }
    out
}

/// Walk the LLR sub_components tree to resolve `path` into its concrete
/// [`SubComponentIdx`].
fn sub_component_idx_at_path(
    cu: &i_slint_compiler::llr::CompilationUnit,
    root_idx: SubComponentIdx,
    path: &[SubComponentInstanceIdx],
) -> SubComponentIdx {
    let mut current = root_idx;
    for &instance_idx in path {
        let nested = &cu.sub_components[current].sub_components[instance_idx];
        current = nested.ty;
    }
    current
}

fn item_flat_index_to_rect(
    instance: &VRc<ItemTreeVTable, Instance>,
    flat_idx: usize,
) -> Option<HighlightedRect> {
    let vrc = VRc::into_dyn(instance.clone());
    let item_rc = ItemRc::new(vrc.clone(), flat_idx as u32);
    let geometry = item_rc.geometry();
    if geometry.size.is_empty() {
        return None;
    }
    let origin = item_rc.map_to_item_tree(geometry.origin, &vrc);
    let top_right =
        item_rc.map_to_item_tree(geometry.origin + euclid::vec2(geometry.size.width, 0.), &vrc);
    let delta = top_right - origin;
    let width = delta.length();
    let height = if geometry.size.width == 0.0 {
        0.0
    } else {
        geometry.size.height * width / geometry.size.width
    };
    let angle_rad = delta.y.atan2(delta.x);
    let (sin, cos) = angle_rad.sin_cos();
    let center = euclid::point2(
        origin.x + (width / 2.0) * cos - (height / 2.0) * sin,
        origin.y + (width / 2.0) * sin + (height / 2.0) * cos,
    );
    Some(HighlightedRect {
        rect: LogicalRect {
            origin: center - euclid::vec2(width / 2.0, height / 2.0),
            size: euclid::size2(width, height),
        },
        angle: angle_rad.to_degrees(),
    })
}

fn positions_by_source(
    instance: &VRc<ItemTreeVTable, Instance>,
    target_path: &Path,
    target_offset: u32,
    filter: ElementPositionFilter,
) -> Vec<HighlightedRect> {
    let cu = &instance.root_sub_component.compilation_unit;
    let mut results = Vec::new();
    for sc_idx in 0..cu.sub_components.len() {
        let sc_idx: SubComponentIdx = sc_idx.into();
        let sc = &cu.sub_components[sc_idx];
        let Some(debug) = sc.debug_info.as_ref() else { continue };
        for (local_idx, item_dbg) in debug.items.iter_enumerated() {
            let Some(source_file) = item_dbg.source_location.source_file.as_ref() else {
                continue;
            };
            if source_file.path() != target_path {
                continue;
            }
            if item_dbg.source_location.span.offset as u32 != target_offset {
                continue;
            }
            for flat_idx in find_flat_indices_for_item(instance, sc_idx, local_idx) {
                if filter == ElementPositionFilter::ExcludeClipped {
                    let dyn_rc = vtable::VRc::into_dyn(instance.clone());
                    let item_rc =
                        i_slint_core::items::ItemRc::new(dyn_rc, flat_idx as u32);
                    if !item_rc.is_visible() {
                        continue;
                    }
                }
                if let Some(rect) = item_flat_index_to_rect(instance, flat_idx) {
                    results.push(rect);
                }
            }
        }
    }
    results
}
