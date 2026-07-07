// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Highlight support for running component instances.
//!
//! Walks the LLR `debug_info` side table to map either a source location
//! or an object-tree `ElementRc` back to runtime flat item indices, then
//! reads geometries via `ItemRc::geometry()` and transforms them through
//! `map_to_item_tree`.

use crate::instance::{Instance, SubComponentInstance};
use i_slint_compiler::llr::{ItemInstanceIdx, SubComponentIdx, SubComponentInstanceIdx};
use i_slint_compiler::object_tree::ElementRc;
use i_slint_core::graphics::euclid;
use i_slint_core::item_tree::ItemTreeVTable;
use i_slint_core::items::ItemRc;
use i_slint_core::lengths::{LogicalPoint, LogicalRect};
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
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
/// ancestor. Public for downstream tooling such as the LSP element
/// selection, whose hit-testing needs the `ExcludeClipped` filter.
pub fn element_positions(
    instance: &VRc<ItemTreeVTable, Instance>,
    element: &ElementRc,
    filter: ElementPositionFilter,
) -> Vec<HighlightedRect> {
    // Match by source location: the LLR copies the element's
    // `source_location` onto every item it lowers, and the object-tree
    // element keeps the original node. `element_hash` would be more
    // compact, but passes that run after `inject_debug_hooks` (layout
    // lowering, property hoisting) create elements without a hash.
    let target = walk_to_native_root(element);
    let Some(target_loc) = source_location_of(&target) else {
        return Vec::new();
    };
    // A component use (`Button { }`) resolves to the definition's root
    // element, whose location matches every instantiation of the component.
    // Constrain the matches to item-table paths that descend through this
    // specific use site.
    let use_site = if Rc::ptr_eq(&target, element) { None } else { source_location_of(element) };
    positions_by_source(
        instance,
        &target_loc.0,
        target_loc.1,
        use_site.as_ref().map(|(p, o)| (p.as_path(), *o)),
        filter,
    )
}

/// The `(path, offset)` key under which the LLR debug info records
/// `element` — `Spanned::to_source_location` semantics (the qualified
/// name's start).
fn source_location_of(element: &ElementRc) -> Option<(std::path::PathBuf, u32)> {
    use i_slint_compiler::diagnostics::Spanned;
    let e = element.borrow();
    let path = e.source_file()?.path().to_path_buf();
    Some((path, e.span().offset as u32))
}

/// Descend into `base_type = Component(_)` wrappers until the element
/// has its own native item. For a component use like `Button { }`, the
/// runtime items belong to the wrapped component's root element, not to
/// the use-site element itself.
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
    element_node_at_source_code_position(instance, path, offset)
        .into_iter()
        .flat_map(|(element, _)| {
            element_positions(instance, &element, ElementPositionFilter::IncludeClipped)
        })
        .collect()
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
    // `inner_components` lists every component defined in the file,
    // exported or not.
    for component in &doc.inner_components {
        visit_element_for_position(&component.root_element, path, offset, &mut result);
    }
    result
}

fn visit_element_for_position(
    element: &ElementRc,
    path: &Path,
    offset: u32,
    result: &mut Vec<(ElementRc, usize)>,
) {
    if element.borrow().repeated.is_some() {
        // The children of a repeated element live in the component the
        // repeater pass wrapped around it, which is not part of
        // `inner_components` — descend explicitly. The wrapper's root
        // element carries the same source node as the repeated element.
        let base = match &element.borrow().base_type {
            i_slint_compiler::langtype::ElementType::Component(c) => Some(c.root_element.clone()),
            _ => None,
        };
        if let Some(root) = base {
            visit_element_for_position(&root, path, offset, result);
        }
        return;
    }
    for (index, node_path, node_range) in element.borrow().debug.iter().enumerate().map(|(i, n)| {
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
    }) {
        if node_path == path && node_range.contains(offset.into()) {
            result.push((element.clone(), index));
        }
    }
    let children = element.borrow().children.clone();
    for child in &children {
        visit_element_for_position(child, path, offset, result);
    }
}

/// Scan the instance's flat `item_table` and return every flat index
/// whose entry points at `(sub_component_path → target_sc_idx, target_local)`.
/// With `use_site` set, only paths descending through a sub-component
/// instance whose use-site element sits at that `(path, offset)` match.
fn find_flat_indices_for_item(
    instance: &VRc<ItemTreeVTable, Instance>,
    target_sc_idx: SubComponentIdx,
    target_local: ItemInstanceIdx,
    use_site: Option<(&Path, u32)>,
) -> Vec<usize> {
    let cu = &instance.root_sub_component.compilation_unit;
    let root_ty = instance.root_sub_component.sub_component_idx;
    let mut out = Vec::new();
    for (flat, entry) in instance.item_table.iter().enumerate() {
        let Some((path, local_idx)) = entry.as_ref() else { continue };
        if *local_idx != target_local {
            continue;
        }
        if sub_component_idx_at_path(cu, root_ty, path) != target_sc_idx {
            continue;
        }
        if let Some((us_path, us_offset)) = use_site
            && !path_passes_use_site(cu, root_ty, path, us_path, us_offset)
        {
            continue;
        }
        out.push(flat);
    }
    out
}

/// Whether any step of `path` descends through a sub-component instance
/// whose use-site element is recorded at `(us_path, us_offset)`.
fn path_passes_use_site(
    cu: &i_slint_compiler::llr::CompilationUnit,
    mut current: SubComponentIdx,
    path: &[SubComponentInstanceIdx],
    us_path: &Path,
    us_offset: u32,
) -> bool {
    for &instance_idx in path {
        if let Some(debug) = cu.sub_components[current].debug_info.as_ref()
            && let Some(loc) = debug.sub_component_use_sites.get(instance_idx)
            && loc.source_file.as_ref().is_some_and(|f| f.path() == us_path)
            && loc.span.offset as u32 == us_offset
        {
            return true;
        }
        current = cu.sub_components[current].sub_components[instance_idx].ty;
    }
    false
}

/// `root` plus every instantiated repeated / conditional row instance
/// below it, recursively.
fn all_instances(root: &VRc<ItemTreeVTable, Instance>) -> Vec<VRc<ItemTreeVTable, Instance>> {
    let mut out = Vec::new();
    collect_instances(root, &mut out);
    out
}

fn collect_instances(
    inst: &VRc<ItemTreeVTable, Instance>,
    out: &mut Vec<VRc<ItemTreeVTable, Instance>>,
) {
    out.push(inst.clone());
    collect_row_instances(&inst.root_sub_component, out);
}

fn collect_row_instances(
    sub: &Pin<Rc<SubComponentInstance>>,
    out: &mut Vec<VRc<ItemTreeVTable, Instance>>,
) {
    for rep in sub.repeaters.iter() {
        for row in rep.instances_vec() {
            collect_instances(&row, out);
        }
    }
    for nested in sub.sub_components.iter() {
        collect_row_instances(nested, out);
    }
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
    root: &VRc<ItemTreeVTable, Instance>,
    flat_idx: usize,
) -> Option<HighlightedRect> {
    let vrc = VRc::into_dyn(instance.clone());
    let root_vrc = VRc::into_dyn(root.clone());
    let item_rc = ItemRc::new(vrc, flat_idx as u32);
    let geometry = item_rc.geometry();
    if geometry.size.is_empty() {
        return None;
    }
    let origin = item_rc.map_to_item_tree(geometry.origin, &root_vrc);
    let top_right = item_rc
        .map_to_item_tree(geometry.origin + euclid::vec2(geometry.size.width, 0.), &root_vrc);
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
    root: &VRc<ItemTreeVTable, Instance>,
    target_path: &Path,
    target_offset: u32,
    use_site: Option<(&Path, u32)>,
    filter: ElementPositionFilter,
) -> Vec<HighlightedRect> {
    let cu = root.root_sub_component.compilation_unit.clone();
    let mut results = Vec::new();
    // Repeated / conditional rows are separate instances with their own
    // item tables, so search all of them, mapping geometry back into the
    // root instance's coordinates.
    for instance in all_instances(root) {
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
                for flat_idx in find_flat_indices_for_item(&instance, sc_idx, local_idx, use_site) {
                    if filter == ElementPositionFilter::ExcludeClipped {
                        let dyn_rc = vtable::VRc::into_dyn(instance.clone());
                        let item_rc = i_slint_core::items::ItemRc::new(dyn_rc, flat_idx as u32);
                        if !item_rc.is_visible() {
                            continue;
                        }
                    }
                    if let Some(rect) = item_flat_index_to_rect(&instance, root, flat_idx) {
                        results.push(rect);
                    }
                }
            }
        }
    }
    results
}
