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
use i_slint_core::lengths::{ItemTransform, LogicalPoint, LogicalRect, LogicalVector};
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
    /// Whether `rect` and `angle` describe the element's rendered shape.
    ///
    /// The two of them describe a rotated rectangle.
    /// An element renders as one while its own and its ancestors' transforms keep its two axes
    /// at a right angle.
    /// A non-uniform scale combined with a rotation shears it into a parallelogram instead,
    /// which only `local_rect` still describes.
    pub renders_as_rectangle: bool,
    /// The element's rectangle in its parent's coordinate system.
    ///
    /// This is what the source `x`, `y`, `width` and `height` describe.
    /// Unlike `rect`, no transform of the element or of any of its ancestors applies to it.
    pub local_rect: LogicalRect,
    /// Maps this instance's parent coordinate system to root coordinates.
    ///
    /// Invert it to turn a position picked in root coordinates into one that can be written to
    /// the source.
    /// It is composed from the instance's own ancestors,
    /// so it stays correct even if the element is positioned outside of
    /// (or with a negative offset relative to) its parent.
    pub parent_transform: ItemTransform,
    /// Evaluated parent-relative rotation in degrees, including complete turns.
    pub transform_rotation: f32,
    /// Evaluated corner radii in logical pixels.
    pub corner_radii: CornerRadii,
}
/// Evaluated rectangle corner radii in logical pixels.
#[derive(Clone, Copy, Debug, Default)]
pub struct CornerRadii {
    /// Top-left radius.
    pub top_left: f32,
    /// Top-right radius.
    pub top_right: f32,
    /// Bottom-left radius.
    pub bottom_left: f32,
    /// Bottom-right radius.
    pub bottom_right: f32,
}

impl HighlightedRect {
    /// Absolute origin of this instance's parent coordinate system, in root coordinates.
    pub fn parent_origin(&self) -> LogicalPoint {
        self.parent_transform.transform_point(LogicalPoint::default().cast()).cast()
    }

    /// Absolute rotation (in degrees) of this instance's parent coordinate system.
    ///
    /// `angle - parent_rotation()` yields the element's own rotation relative to its parent,
    /// which matches `transform-rotation` modulo complete turns.
    /// Use `transform_rotation` to retain those turns.
    pub fn parent_rotation(&self) -> f32 {
        self.parent_transform.m12.atan2(self.parent_transform.m11).to_degrees()
    }

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
    for repeater in sub.repeaters.iter() {
        repeater.track_instance_changes();
        for row in repeater.instances_vec() {
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

/// Whether the item's LLR debug info marks it as an injected geometry
/// wrapper (`Element::is_injected_wrapper_element`).
fn is_injected_wrapper_element(instance: &VRc<ItemTreeVTable, Instance>, flat_idx: usize) -> bool {
    let cu = &instance.root_sub_component.compilation_unit;
    let root_ty = instance.root_sub_component.sub_component_idx;
    let Some(Some((path, local_idx))) = instance.item_table.get(flat_idx) else {
        return false;
    };
    let sc_idx = sub_component_idx_at_path(cu, root_ty, path);
    cu.sub_components[sc_idx]
        .debug_info
        .as_ref()
        .and_then(|debug| debug.items.get(*local_idx))
        .is_some_and(|item_debug| item_debug.is_injected_wrapper_element)
}

fn item_flat_index_to_rect(
    instance: &VRc<ItemTreeVTable, Instance>,
    root: &VRc<ItemTreeVTable, Instance>,
    flat_idx: usize,
) -> Option<HighlightedRect> {
    let vrc = VRc::into_dyn(instance.clone());
    let root_vrc = VRc::into_dyn(root.clone());
    let item_rc = ItemRc::new(vrc.clone(), flat_idx as u32);
    let geometry = item_rc.geometry();
    if geometry.size.is_empty() {
        return None;
    }
    // Injected geometry wrappers (opacity/transform/clip/... created by
    // `lower_property_to_element`) take over the element's geometry and lay the element
    // out at (0,0) inside themselves, so measuring the parent frame from the element
    // directly would collapse `rect.origin - parent_origin` to ~0.
    let mut transform_rotation = 0.;
    let mut anchor = item_rc.clone();
    while let Some(parent) =
        anchor.parent_item(i_slint_core::item_tree::ParentItemTraversalMode::StopAtPopups)
    {
        if !VRc::ptr_eq(parent.item_tree(), &vrc) {
            break; // crossed into another component instance's item tree
        }
        if !is_injected_wrapper_element(instance, parent.index() as usize) {
            break;
        }
        if let Some(transform) = i_slint_core::items::ItemRef::downcast_pin::<
            i_slint_core::items::Transform,
        >(parent.borrow())
        {
            transform_rotation += transform.transform_rotation();
        }
        anchor = parent;
    }

    // Neither transform adds its item's own x/y, so `parent_transform` maps the element's
    // source-parent coordinate system, and `to_root` the coordinate system the injected
    // wrappers place the element in — the element's own transform included.
    let to_root = item_rc.transform_to_item_tree(&root_vrc);
    let parent_transform = anchor.transform_to_item_tree(&root_vrc);
    let map_axis = |axis: euclid::Vector2D<f32, _>| to_root.transform_vector(axis).cast();

    let origin: LogicalPoint = to_root.transform_point(geometry.origin.cast()).cast();
    // Both edges are measured, so a scale along one axis is not assumed to match the other.
    let size = geometry.size.cast::<f32>();
    let x_axis = map_axis(euclid::vec2(size.width, 0.));
    let y_axis = map_axis(euclid::vec2(0., size.height));
    let width = x_axis.length();
    let height = y_axis.length();
    let center = origin + (x_axis + y_axis) / 2.0;
    Some(HighlightedRect {
        rect: LogicalRect {
            origin: center - euclid::vec2(width / 2.0, height / 2.0),
            size: euclid::size2(width, height),
        },
        angle: x_axis.y.atan2(x_axis.x).to_degrees(),
        renders_as_rectangle: are_perpendicular(x_axis, y_axis),
        local_rect: if anchor == item_rc { geometry } else { anchor.geometry() },
        parent_transform,
        transform_rotation,
        corner_radii: item_corner_radii(item_rc.borrow()),
    })
}

/// Whether two axes still meet at a right angle.
/// The cosine between them is compared, so the tolerance does not depend on how long they are.
fn are_perpendicular(x: LogicalVector, y: LogicalVector) -> bool {
    let squared_lengths = x.square_length() * y.square_length();
    let dot = x.dot(y);
    squared_lengths == 0. || dot * dot < 1.0e-6 * squared_lengths
}

fn positions_by_source(
    root: &VRc<ItemTreeVTable, Instance>,
    target_path: &Path,
    target_offset: u32,
    use_site: Option<(&Path, u32)>,
    filter: ElementPositionFilter,
) -> Vec<HighlightedRect> {
    items_by_source(root, target_path, target_offset, use_site)
        .into_iter()
        .filter_map(|(instance, flat_idx)| {
            if filter == ElementPositionFilter::ExcludeClipped {
                let item = ItemRc::new(VRc::into_dyn(instance.clone()), flat_idx as u32);
                if !item.is_visible() {
                    return None;
                }
            }
            item_flat_index_to_rect(&instance, root, flat_idx)
        })
        .collect()
}

fn item_corner_radii(item: Pin<i_slint_core::items::ItemRef<'_>>) -> CornerRadii {
    use i_slint_core::items::{BasicBorderRectangle, BorderRectangle, ItemRef};
    if let Some(rect) = ItemRef::downcast_pin::<BorderRectangle>(item) {
        CornerRadii {
            top_left: rect.border_top_left_radius().get(),
            top_right: rect.border_top_right_radius().get(),
            bottom_left: rect.border_bottom_left_radius().get(),
            bottom_right: rect.border_bottom_right_radius().get(),
        }
    } else if let Some(rect) = ItemRef::downcast_pin::<BasicBorderRectangle>(item) {
        let radius = rect.border_radius().get();
        CornerRadii {
            top_left: radius,
            top_right: radius,
            bottom_left: radius,
            bottom_right: radius,
        }
    } else {
        CornerRadii::default()
    }
}

fn items_by_source(
    root: &VRc<ItemTreeVTable, Instance>,
    target_path: &Path,
    target_offset: u32,
    use_site: Option<(&Path, u32)>,
) -> Vec<(VRc<ItemTreeVTable, Instance>, usize)> {
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
                    results.push((instance.clone(), flat_idx));
                }
            }
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use crate::{
        ComponentInstance,
        debug_hook::tests::{compile_with_debug_hooks, test_path},
    };

    fn geometry_of(
        instance: &ComponentInstance,
        code: &str,
        id: &str,
    ) -> crate::highlight::HighlightedRect {
        let id_position = code.find(id).unwrap_or_else(|| panic!("{id} not found"));
        let offset = id_position + code[id_position..].find("Rectangle").unwrap();
        let (element, _) = instance
            .element_node_at_source_code_position(&test_path(), offset as u32)
            .first()
            .cloned()
            .unwrap_or_else(|| panic!("element {id} not resolved"));
        *instance.element_positions(&element).first().expect("geometry")
    }

    // With debug_hooks enabled every element is wrapped in injected geometry wrappers
    // (`Transform`, plus `Opacity` etc. when those props are set), which take over the element's
    // geometry. `element_positions` must still report a `parent_origin` from which the element's
    // own `x`/`y` can be recovered (`rect.origin - parent_origin == x/y`), otherwise the editor
    // commits wrong coordinates when repositioning. This must hold through stacked wrappers and
    // for elements nested below a non-root parent.
    #[test]
    fn debug_hooks_parent_origin() {
        let code = r#"
export component Win inherits Window {
    width: 300px;
    height: 200px;
    plain := Rectangle {
        x: 30px;
        y: 40px;
        width: 50px;
        height: 60px;
    }
    faded := Rectangle {
        // extra Opacity and visibility-Clip wrappers stacked around the Transform wrapper
        opacity: 0.5;
        visible: true;
        x: 70px;
        y: 80px;
        width: 40px;
        height: 30px;
    }
    outer := Rectangle {
        x: 10px;
        y: 20px;
        width: 120px;
        height: 100px;
        nested := Rectangle {
            x: 5px;
            y: 7px;
            width: 20px;
            height: 20px;
        }
    }
}"#;
        let instance = compile_with_debug_hooks(code);

        let check = |id: &str, expected: (f32, f32)| {
            let geometry = geometry_of(&instance, code, id);
            let x = geometry.rect.origin.x - geometry.parent_origin().x;
            let y = geometry.rect.origin.y - geometry.parent_origin().y;
            assert!(
                (x - expected.0).abs() < 0.5 && (y - expected.1).abs() < 0.5,
                "{id}: source-relative position ({x}, {y}) should be {expected:?}"
            );
        };

        check("plain", (30.0, 40.0));
        check("faded", (70.0, 80.0));
        check("nested", (5.0, 7.0));
    }

    #[test]
    fn debug_hooks_local_rect() {
        let code = r#"
export component Win inherits Window {
    width: 300px;
    height: 300px;
    outer := Rectangle {
        x: 50px;
        y: 60px;
        width: 160px;
        height: 140px;
        transform-rotation: 30deg;
        inner := Rectangle {
            x: 20px;
            y: 25px;
            width: 40px;
            height: 30px;
            transform-rotation: 15deg;
            transform-origin: { x: 0px, y: 0px };
        }
    }
}"#;
        let instance = compile_with_debug_hooks(code);

        let check = |id: &str, expected: (f32, f32, f32, f32)| {
            let rect = geometry_of(&instance, code, id).local_rect;
            let actual = (rect.origin.x, rect.origin.y, rect.width(), rect.height());
            assert!(
                (actual.0 - expected.0).abs() < 0.5
                    && (actual.1 - expected.1).abs() < 0.5
                    && (actual.2 - expected.2).abs() < 0.5
                    && (actual.3 - expected.3).abs() < 0.5,
                "{id}: parent-relative rectangle {actual:?} should be {expected:?}"
            );
        };

        check("outer", (50.0, 60.0, 160.0, 140.0));
        check("inner", (20.0, 25.0, 40.0, 30.0));
    }

    #[test]
    fn scaled_geometry_is_measured_per_axis() {
        let code = r#"
export component Win inherits Window {
    width: 400px;
    height: 400px;
    stretched := Rectangle {
        x: 20px;
        y: 20px;
        width: 80px;
        height: 40px;
        transform-scale-x: 3;
    }
    wide := Rectangle {
        x: 20px;
        y: 200px;
        width: 200px;
        height: 100px;
        transform-scale-x: 3;
        sheared := Rectangle {
            width: 80px;
            height: 40px;
            transform-rotation: 30deg;
        }
    }
}"#;
        let instance = compile_with_debug_hooks(code);

        // Both axes are measured, so a scale along one of them doesn't stretch the other.
        let stretched = geometry_of(&instance, code, "stretched");
        assert!(
            (stretched.rect.width() - 240.).abs() < 0.5
                && (stretched.rect.height() - 40.).abs() < 0.5,
            "stretched: {:?}",
            stretched.rect
        );
        assert!(stretched.renders_as_rectangle);

        // A rotation below a non-uniform scale is a parallelogram, which `rect` can't express.
        let sheared = geometry_of(&instance, code, "sheared");
        assert!(!sheared.renders_as_rectangle);
        assert!((sheared.angle - 30.).abs() > 1., "sheared angle: {}", sheared.angle);
    }

    #[test]
    fn debug_hooks_parent_rotation() {
        let code = r#"
export component Win inherits Window {
    width: 300px;
    height: 300px;
    outer := Rectangle {
        x: 50px;
        y: 50px;
        width: 160px;
        height: 160px;
        transform-rotation: 30deg;
        inner := Rectangle {
            x: 20px;
            y: 20px;
            width: 40px;
            height: 40px;
            transform-rotation: 15deg;
        }
    }
}"#;
        let instance = compile_with_debug_hooks(code);

        let check = |id: &str, expected: f32| {
            let geometry = geometry_of(&instance, code, id);
            let rotation = geometry.angle - geometry.parent_rotation();
            assert!(
                (rotation - expected).abs() < 0.5,
                "{id}: source-relative rotation {rotation} should be {expected}"
            );
        };

        check("outer", 30.0);
        check("inner", 15.0);
    }
    #[test]
    fn evaluated_rotation_and_radii_share_geometry_instances() {
        let code = r#"
export component Win inherits Window {
    width: 300px;
    height: 300px;
    outer := Rectangle {
        width: 200px;
        height: 200px;
        transform-rotation: 30deg;
        for angle in [382.25deg, -397.5deg]: Rectangle {
            width: 40px;
            height: 40px;
            transform-rotation: angle;
            border-top-left-radius: 1.5px;
            border-top-right-radius: 2.5px;
            border-bottom-left-radius: 3.5px;
            border-bottom-right-radius: 4.5px;
        }
    }
}"#;
        let instance = compile_with_debug_hooks(code);
        let offset = code
            .find(
                "Rectangle {
            width",
            )
            .unwrap();
        let (element, _) = instance
            .element_node_at_source_code_position(&test_path(), offset as u32)
            .first()
            .cloned()
            .unwrap();
        let geometries = instance.element_positions(&element);
        assert_eq!(geometries.len(), 2);
        for (geometry, expected) in geometries.iter().zip([382.25, -397.5]) {
            assert_eq!(geometry.transform_rotation, expected);
            assert!((geometry.parent_rotation() - 30.).abs() < 0.001);
            let radii = geometry.corner_radii;
            assert_eq!(
                [radii.top_left, radii.top_right, radii.bottom_left, radii.bottom_right],
                [1.5, 2.5, 3.5, 4.5]
            );
        }
    }
}
