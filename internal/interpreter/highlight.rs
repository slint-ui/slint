// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Highlight support for running component instances.
//!
//! Walks the LLR `debug_info` side table to map either a source location
//! or an object-tree `ElementRc` back to runtime flat item indices, then
//! reads geometries via `ItemRc::geometry()` and transforms them through
//! `map_to_item_tree`.

use crate::instance::{Instance, SubComponentInstance};
use i_slint_compiler::llr::{
    ItemInstanceIdx, RepeatedElementIdx, SubComponentIdx, SubComponentInstanceIdx,
};
use i_slint_compiler::object_tree::ElementRc;
use i_slint_core::graphics::euclid;
use i_slint_core::item_tree::{ItemTreeRc, ItemTreeVTable, TraversalOrder, VisitChildrenResult};
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
enum ElementPositionFilter {
    /// Include all elements.
    IncludeClipped,
    /// Exclude elements clipped by an ancestor `Clip` / `Flickable`.
    ExcludeClipped,
}

/// A rendered item under a point and one source element that represents it.
#[derive(Clone, Debug)]
pub struct ElementCandidate {
    /// Source element represented by the runtime item.
    pub source_location: i_slint_compiler::diagnostics::SourceLocation,
    /// Runtime geometry of the source element.
    pub geometry: HighlightedRect,
    /// Position of this runtime item among the instances of the source element.
    pub instance_index: usize,
}

/// Return the geometry of every runtime item whose source location covers
/// the given `(path, offset)` pair.
pub(crate) fn component_positions(
    instance: &VRc<ItemTreeVTable, Instance>,
    path: &Path,
    offset: u32,
) -> Vec<HighlightedRect> {
    component_positions_with_filter(instance, path, offset, ElementPositionFilter::IncludeClipped)
}

fn component_positions_with_filter(
    instance: &VRc<ItemTreeVTable, Instance>,
    path: &Path,
    offset: u32,
    filter: ElementPositionFilter,
) -> Vec<HighlightedRect> {
    positions_by_sources(instance, [(path, offset, SourceMatch::Contains)], filter)
}

pub(crate) fn element_candidates_at(
    root: &VRc<ItemTreeVTable, Instance>,
    position: LogicalPoint,
) -> Vec<ElementCandidate> {
    let root_item_tree = VRc::into_dyn(root.clone());
    i_slint_core::item_tree::ensure_item_tree_instantiated(&root_item_tree);
    let instances = all_instances(root);
    let mut runtime_items = Vec::new();
    collect_runtime_items_front_to_back(&root_item_tree, -1, &instances, &mut runtime_items);

    let mut candidates = Vec::new();
    for (instance, flat_index) in runtime_items {
        let item = ItemRc::new(VRc::into_dyn(instance.clone()), flat_index as u32);
        if !item.is_visible() {
            continue;
        }
        let Some(geometry) = item_geometry(&instance, root, flat_index) else {
            continue;
        };
        if !geometry.contains(position) {
            continue;
        }

        let compilation_unit = &instance.root_sub_component.compilation_unit;
        let Some((sub_component_path, local_item_index)) =
            instance.item_table.get(flat_index).and_then(Option::as_ref)
        else {
            continue;
        };
        let sub_component_index = sub_component_index_at_path(
            compilation_unit,
            instance.root_sub_component.sub_component_idx,
            sub_component_path,
        );
        if let Some(debug_info) =
            compilation_unit.sub_components[sub_component_index].debug_info.as_ref()
            && let Some(item_debug_entries) = debug_info.items.get(*local_item_index)
        {
            for item_debug_info in item_debug_entries.iter().rev() {
                push_runtime_item_candidate(
                    root,
                    &instance,
                    flat_index,
                    geometry,
                    &item_debug_info.source_location,
                    &mut candidates,
                );
            }
        }

        let mut current_sub_component_index = instance.root_sub_component.sub_component_idx;
        for sub_component_instance_index in sub_component_path.iter().copied() {
            let sub_component = &compilation_unit.sub_components[current_sub_component_index];
            if let Some(source_location) =
                sub_component.debug_info.as_ref().and_then(|debug_info| {
                    debug_info.sub_component_use_sites.get(sub_component_instance_index)
                })
            {
                push_source_candidates_at(
                    root,
                    source_location,
                    position,
                    ElementPositionFilter::ExcludeClipped,
                    &mut candidates,
                );
            }
            current_sub_component_index =
                sub_component.sub_components[sub_component_instance_index].ty;
        }

        let mut repeated_instance = Some(instance);
        while let Some(instance) = repeated_instance {
            let Some((parent_sub_component, repeated_element_index)) =
                instance.root_sub_component.repeated_in.get()
            else {
                break;
            };
            let Some(parent_sub_component) = parent_sub_component.upgrade() else {
                break;
            };
            if let Some(source_location) = parent_sub_component.compilation_unit.sub_components
                [parent_sub_component.sub_component_idx]
                .debug_info
                .as_ref()
                .and_then(|debug_info| debug_info.repeated_elements.get(*repeated_element_index))
            {
                push_source_candidates_at(
                    root,
                    source_location,
                    position,
                    ElementPositionFilter::ExcludeClipped,
                    &mut candidates,
                );
            }
            repeated_instance = parent_sub_component.root.get().and_then(|root| root.upgrade());
        }
    }
    candidates
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
    for (index, node_path, node_range) in
        element.borrow().debug.iter().enumerate().map(|(index, debug_info)| {
            let text_range = debug_info
                .node
                .QualifiedName()
                .map(|qualified_name| qualified_name.text_range())
                .or_else(|| {
                    debug_info
                        .node
                        .child_token(i_slint_compiler::parser::SyntaxKind::LBrace)
                        .map(|left_brace| left_brace.text_range())
                })
                .expect("An Element must contain a LBrace somewhere");
            (index, debug_info.node.source_file.path(), text_range)
        })
    {
        if node_path == path && node_range.contains(offset.into()) {
            result.push((element.clone(), index));
        }
    }
    let children = element.borrow().children.clone();
    for child in &children {
        visit_element_for_position(child, path, offset, result);
    }
}

fn find_flat_indices_for_item(
    instance: &VRc<ItemTreeVTable, Instance>,
    target: &ItemTarget,
) -> Vec<usize> {
    let compilation_unit = &instance.root_sub_component.compilation_unit;
    let root_sub_component_index = instance.root_sub_component.sub_component_idx;
    let mut flat_indices = Vec::new();
    for (flat_index, entry) in instance.item_table.iter().enumerate() {
        let Some((sub_component_path, local_index)) = entry.as_ref() else { continue };
        if *local_index != target.local_item_index {
            continue;
        }
        if sub_component_index_at_path(
            compilation_unit,
            root_sub_component_index,
            sub_component_path,
        ) != target.sub_component_index
        {
            continue;
        }
        if !target.use_sites.iter().all(|use_site| {
            path_passes_use_site(
                compilation_unit,
                root_sub_component_index,
                sub_component_path,
                *use_site,
            )
        }) {
            continue;
        }
        flat_indices.push(flat_index);
    }
    flat_indices
}

fn path_passes_use_site(
    compilation_unit: &i_slint_compiler::llr::CompilationUnit,
    mut current_sub_component_index: SubComponentIdx,
    path: &[SubComponentInstanceIdx],
    use_site: UseSite,
) -> bool {
    for &sub_component_instance_index in path {
        if current_sub_component_index == use_site.parent_sub_component
            && sub_component_instance_index == use_site.sub_component_instance_index
        {
            return true;
        }
        current_sub_component_index = compilation_unit.sub_components[current_sub_component_index]
            .sub_components[sub_component_instance_index]
            .ty;
    }
    false
}

/// `root` plus every instantiated repeated / conditional row instance
/// below it, recursively.
fn all_instances(root: &VRc<ItemTreeVTable, Instance>) -> Vec<VRc<ItemTreeVTable, Instance>> {
    let mut instances = Vec::new();
    collect_instances(root, &mut instances);
    instances
}

fn collect_instances(
    instance: &VRc<ItemTreeVTable, Instance>,
    instances: &mut Vec<VRc<ItemTreeVTable, Instance>>,
) {
    instances.push(instance.clone());
    collect_row_instances(&instance.root_sub_component, instances);
}

fn collect_row_instances(
    sub_component: &Pin<Rc<SubComponentInstance>>,
    instances: &mut Vec<VRc<ItemTreeVTable, Instance>>,
) {
    for repeater in sub_component.repeaters.iter() {
        repeater.track_instance_changes();
        for row in repeater.instances_vec() {
            collect_instances(&row, instances);
        }
    }
    for nested_sub_component in sub_component.sub_components.iter() {
        collect_row_instances(nested_sub_component, instances);
    }
}

fn collect_runtime_items_front_to_back(
    item_tree: &ItemTreeRc,
    index: isize,
    instances: &[VRc<ItemTreeVTable, Instance>],
    runtime_items: &mut Vec<(VRc<ItemTreeVTable, Instance>, usize)>,
) {
    let mut children = Vec::new();
    let mut collect_child = |child_item_tree: &ItemTreeRc,
                             child_index: u32,
                             _: Pin<i_slint_core::items::ItemRef<'_>>|
     -> VisitChildrenResult {
        children.push((child_item_tree.clone(), child_index));
        VisitChildrenResult::CONTINUE
    };
    vtable::new_vref!(
        let mut collect_child: VRefMut<i_slint_core::item_tree::ItemVisitorVTable>
            for i_slint_core::item_tree::ItemVisitor = &mut collect_child
    );
    VRc::borrow_pin(item_tree).as_ref().visit_children_item(
        index,
        TraversalOrder::FrontToBack,
        collect_child,
    );
    for (child_item_tree, child_index) in children {
        collect_runtime_items_front_to_back(
            &child_item_tree,
            child_index as isize,
            instances,
            runtime_items,
        );
    }
    if index < 0 {
        return;
    }
    if let Some(instance) = instances
        .iter()
        .find(|instance| VRc::ptr_eq(&VRc::into_dyn((*instance).clone()), item_tree))
    {
        runtime_items.push((instance.clone(), index as usize));
    }
}

/// Walk the LLR sub_components tree to resolve `path` into its concrete
/// [`SubComponentIdx`].
fn sub_component_index_at_path(
    compilation_unit: &i_slint_compiler::llr::CompilationUnit,
    root_sub_component_index: SubComponentIdx,
    path: &[SubComponentInstanceIdx],
) -> SubComponentIdx {
    let mut current_sub_component_index = root_sub_component_index;
    for &sub_component_instance_index in path {
        let nested_sub_component = &compilation_unit.sub_components[current_sub_component_index]
            .sub_components[sub_component_instance_index];
        current_sub_component_index = nested_sub_component.ty;
    }
    current_sub_component_index
}

fn push_runtime_item_candidate(
    root: &VRc<ItemTreeVTable, Instance>,
    instance: &VRc<ItemTreeVTable, Instance>,
    flat_index: usize,
    geometry: HighlightedRect,
    source_location: &i_slint_compiler::diagnostics::SourceLocation,
    candidates: &mut Vec<ElementCandidate>,
) {
    let Some(source_file) = source_location.source_file.as_ref() else { return };
    let Ok(source_offset) = u32::try_from(source_location.span.offset) else { return };
    let Some(instance_index) =
        items_by_source(root, source_file.path(), source_offset, SourceMatch::Start)
            .iter()
            .position(|(source_instance, source_flat_index)| {
                VRc::ptr_eq(source_instance, instance) && *source_flat_index == flat_index
            })
    else {
        return;
    };
    push_candidate(source_location, geometry, instance_index, candidates);
}

fn push_source_candidates_at(
    root: &VRc<ItemTreeVTable, Instance>,
    source_location: &i_slint_compiler::diagnostics::SourceLocation,
    position: LogicalPoint,
    filter: ElementPositionFilter,
    candidates: &mut Vec<ElementCandidate>,
) {
    let Some(source_file) = source_location.source_file.as_ref() else { return };
    let Ok(source_offset) = u32::try_from(source_location.span.offset) else { return };
    for (instance_index, (instance, flat_index)) in
        items_by_source(root, source_file.path(), source_offset, SourceMatch::Start)
            .into_iter()
            .enumerate()
    {
        if filter == ElementPositionFilter::ExcludeClipped {
            let item = ItemRc::new(VRc::into_dyn(instance.clone()), flat_index as u32);
            if !item.is_visible() {
                continue;
            }
        }
        let Some(geometry) = item_geometry(&instance, root, flat_index) else {
            continue;
        };
        if geometry.contains(position) {
            push_candidate(source_location, geometry, instance_index, candidates);
        }
    }
}

fn push_candidate(
    source_location: &i_slint_compiler::diagnostics::SourceLocation,
    geometry: HighlightedRect,
    instance_index: usize,
    candidates: &mut Vec<ElementCandidate>,
) {
    let Some(source_file) = source_location.source_file.as_ref() else { return };
    if candidates.iter().any(|candidate| {
        candidate.instance_index == instance_index
            && candidate.source_location.span.offset == source_location.span.offset
            && candidate.source_location.source_file.as_ref().is_some_and(|candidate_source_file| {
                candidate_source_file.path() == source_file.path()
            })
    }) {
        return;
    }
    candidates.push(ElementCandidate {
        source_location: source_location.clone(),
        geometry,
        instance_index,
    });
}

fn item_geometry(
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
        if !parent.is_injected_wrapper() {
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

fn positions_by_sources<'a>(
    root: &VRc<ItemTreeVTable, Instance>,
    sources: impl IntoIterator<Item = (&'a Path, u32, SourceMatch)>,
    filter: ElementPositionFilter,
) -> Vec<HighlightedRect> {
    let mut matching_items = Vec::new();
    for (target_path, target_offset, source_match) in sources {
        for (instance, flat_index) in
            items_by_source(root, target_path, target_offset, source_match)
        {
            if !matching_items.iter().any(|(existing_instance, existing_index)| {
                VRc::ptr_eq(existing_instance, &instance) && *existing_index == flat_index
            }) {
                matching_items.push((instance, flat_index));
            }
        }
    }
    matching_items
        .into_iter()
        .filter_map(|(instance, flat_index)| {
            if filter == ElementPositionFilter::ExcludeClipped {
                let item = ItemRc::new(VRc::into_dyn(instance.clone()), flat_index as u32);
                if !item.is_visible() {
                    return None;
                }
            }
            item_geometry(&instance, root, flat_index)
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
    source_match: SourceMatch,
) -> Vec<(VRc<ItemTreeVTable, Instance>, usize)> {
    let compilation_unit = root.root_sub_component.compilation_unit.clone();
    let mut targets = Vec::new();
    collect_item_targets(
        &compilation_unit,
        target_path,
        target_offset,
        source_match,
        &[],
        None,
        &[],
        &mut targets,
    );
    let mut results = Vec::new();
    for instance in all_instances(root) {
        for target in &targets {
            if target.repeated_element_use.is_some_and(|repeated_element_use| {
                !instance_belongs_to_repeated_element(&instance, repeated_element_use)
            }) {
                continue;
            }
            for flat_index in find_flat_indices_for_item(&instance, target) {
                if !results.iter().any(|(existing_instance, existing_index)| {
                    VRc::ptr_eq(existing_instance, &instance) && *existing_index == flat_index
                }) {
                    results.push((instance.clone(), flat_index));
                }
            }
        }
    }
    results
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SourceMatch {
    Contains,
    Start,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct UseSite {
    parent_sub_component: SubComponentIdx,
    sub_component_instance_index: SubComponentInstanceIdx,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct RepeatedElementUse {
    parent_sub_component: SubComponentIdx,
    repeated_element_index: RepeatedElementIdx,
}

#[derive(Clone, Eq, PartialEq)]
struct ItemTarget {
    sub_component_index: SubComponentIdx,
    local_item_index: ItemInstanceIdx,
    use_sites: Vec<UseSite>,
    repeated_element_use: Option<RepeatedElementUse>,
}

fn collect_item_targets(
    compilation_unit: &i_slint_compiler::llr::CompilationUnit,
    target_path: &Path,
    target_offset: u32,
    source_match: SourceMatch,
    use_sites: &[UseSite],
    repeated_element_use: Option<RepeatedElementUse>,
    ignored_repeated_elements: &[RepeatedElementUse],
    targets: &mut Vec<ItemTarget>,
) {
    let matching_repeated_elements = compilation_unit
        .sub_components
        .iter_enumerated()
        .flat_map(|(sub_component_index, sub_component)| {
            sub_component.debug_info.iter().flat_map(move |debug_info| {
                debug_info.repeated_elements.iter_enumerated().filter_map(
                    move |(repeated_element_index, source_location)| {
                        source_location_matches(
                            source_location,
                            target_path,
                            target_offset,
                            source_match,
                        )
                        .then_some(RepeatedElementUse {
                            parent_sub_component: sub_component_index,
                            repeated_element_index,
                        })
                        .filter(|matching_repeated_element| {
                            !ignored_repeated_elements.contains(matching_repeated_element)
                        })
                    },
                )
            })
        })
        .collect::<Vec<_>>();

    if !matching_repeated_elements.is_empty() {
        for child_repeated_element_use in matching_repeated_elements.iter().copied() {
            collect_item_targets(
                compilation_unit,
                target_path,
                target_offset,
                source_match,
                use_sites,
                Some(child_repeated_element_use),
                &matching_repeated_elements,
                targets,
            );
        }
        return;
    }

    let matching_use_sites = compilation_unit
        .sub_components
        .iter_enumerated()
        .flat_map(|(sub_component_index, sub_component)| {
            sub_component.debug_info.iter().flat_map(move |debug_info| {
                debug_info.sub_component_use_sites.iter_enumerated().filter_map(
                    move |(sub_component_instance_index, source_location)| {
                        source_location_matches(
                            source_location,
                            target_path,
                            target_offset,
                            source_match,
                        )
                        .then_some((sub_component_index, sub_component_instance_index))
                    },
                )
            })
        })
        .collect::<Vec<_>>();

    if !matching_use_sites.is_empty() {
        for (sub_component_index, sub_component_instance_index) in matching_use_sites {
            let sub_component = &compilation_unit.sub_components[sub_component_index];
            let child_sub_component_index =
                sub_component.sub_components[sub_component_instance_index].ty;
            let Some(child_debug_info) =
                compilation_unit.sub_components[child_sub_component_index].debug_info.as_ref()
            else {
                continue;
            };
            let Some(child_source_file) = child_debug_info.source_location.source_file.as_ref()
            else {
                continue;
            };
            let Ok(child_source_offset) =
                u32::try_from(child_debug_info.source_location.span.offset)
            else {
                continue;
            };
            let child_ignored_repeated_elements = if child_source_file.path() == target_path
                && child_source_offset == target_offset
            {
                ignored_repeated_elements
            } else {
                &[]
            };
            let mut child_use_sites = use_sites.to_vec();
            let child_use_site =
                UseSite { parent_sub_component: sub_component_index, sub_component_instance_index };
            if child_use_sites.contains(&child_use_site) {
                continue;
            }
            child_use_sites.push(child_use_site);
            collect_item_targets(
                compilation_unit,
                child_source_file.path(),
                child_source_offset,
                SourceMatch::Start,
                &child_use_sites,
                repeated_element_use,
                child_ignored_repeated_elements,
                targets,
            );
        }
        return;
    }

    for (sub_component_index, sub_component) in compilation_unit.sub_components.iter_enumerated() {
        let Some(debug_info) = sub_component.debug_info.as_ref() else { continue };

        for (local_item_index, item_debug_entries) in debug_info.items.iter_enumerated() {
            if item_debug_entries.iter().any(|item_debug_info| {
                source_location_matches(
                    &item_debug_info.source_location,
                    target_path,
                    target_offset,
                    source_match,
                )
            }) {
                let target = ItemTarget {
                    sub_component_index,
                    local_item_index,
                    use_sites: use_sites.to_vec(),
                    repeated_element_use,
                };
                if !targets.contains(&target) {
                    targets.push(target);
                }
            }
        }
    }
}

fn instance_belongs_to_repeated_element(
    instance: &VRc<ItemTreeVTable, Instance>,
    repeated_element_use: RepeatedElementUse,
) -> bool {
    let Some((parent_sub_component, repeated_element_index)) =
        instance.root_sub_component.repeated_in.get()
    else {
        return false;
    };
    *repeated_element_index == repeated_element_use.repeated_element_index
        && parent_sub_component.upgrade().is_some_and(|parent_sub_component| {
            parent_sub_component.sub_component_idx == repeated_element_use.parent_sub_component
        })
}

fn source_location_matches(
    source_location: &i_slint_compiler::diagnostics::SourceLocation,
    target_path: &Path,
    target_offset: u32,
    source_match: SourceMatch,
) -> bool {
    let Some(source_file) = source_location.source_file.as_ref() else { return false };
    if source_file.path() != target_path {
        return false;
    }
    let target_offset = target_offset as usize;
    match source_match {
        SourceMatch::Contains => {
            source_location.span.offset <= target_offset
                && target_offset
                    < source_location.span.offset.saturating_add(source_location.span.length)
        }
        SourceMatch::Start => source_location.span.offset == target_offset,
    }
}

#[cfg(all(test, feature = "internal"))]
mod tests {
    use crate::{
        ComponentInstance,
        debug_hook::tests::{compile_with_debug_hooks, test_path},
    };
    use i_slint_core::item_tree::ParentItemTraversalMode;
    use i_slint_core::items::{BoxShadow, Clip, ItemRc, Layer, Opacity, Transform};
    use vtable::VRc;

    fn geometry_of(
        instance: &ComponentInstance,
        code: &str,
        id: &str,
    ) -> crate::highlight::HighlightedRect {
        let id_position = code.find(id).unwrap_or_else(|| panic!("{id} not found"));
        let offset = id_position + code[id_position..].find("Rectangle").unwrap();
        *instance.component_positions(&test_path(), offset as u32).first().expect("geometry")
    }

    fn runtime_item_of(instance: &ComponentInstance, source: &str, element_id: &str) -> ItemRc {
        let element_id_position =
            source.find(element_id).unwrap_or_else(|| panic!("{element_id} not found"));
        let offset = element_id_position + source[element_id_position..].find("Rectangle").unwrap();
        let (runtime_instance, flat_index) = super::items_by_source(
            instance.inner.vrc(),
            &test_path(),
            offset as u32,
            super::SourceMatch::Start,
        )
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("runtime item for {element_id}"));
        ItemRc::new(VRc::into_dyn(runtime_instance), flat_index as u32)
    }

    #[test]
    fn injected_wrapper_classification() {
        let code = r#"
export component Win inherits Window {
    wrapped := Rectangle {
        opacity: 0.5;
        visible: true;
        transform-rotation: 45deg;
        cache-rendering-hint: true;
        drop-shadow-blur: 2px;
        drop-shadow-color: black;
    }
    ordinary-clip := Rectangle { clip: true; }
}
"#;
        let instance = compile_with_debug_hooks(code);
        let mut ancestors = Vec::new();
        let mut current = Some(runtime_item_of(&instance, code, "wrapped"));
        while let Some(item) = current {
            current = item.parent_item(ParentItemTraversalMode::StopAtPopups);
            ancestors.push(item);
        }

        for wrapper_classification in [
            ancestors
                .iter()
                .find(|item| item.downcast::<Transform>().is_some())
                .map(ItemRc::is_injected_wrapper),
            ancestors
                .iter()
                .find(|item| item.downcast::<Opacity>().is_some())
                .map(ItemRc::is_injected_wrapper),
            ancestors
                .iter()
                .find(|item| item.downcast::<Layer>().is_some())
                .map(ItemRc::is_injected_wrapper),
            ancestors
                .iter()
                .find(|item| {
                    item.downcast::<Clip>()
                        .is_some_and(|clip| clip.as_pin_ref().is_visibility_clip())
                })
                .map(ItemRc::is_injected_wrapper),
        ] {
            assert_eq!(wrapper_classification, Some(true));
        }

        let root_item_tree = VRc::into_dyn(instance.inner.vrc().clone());
        let box_shadow = (0..instance.inner.vrc().item_table.len())
            .map(|flat_index| ItemRc::new(root_item_tree.clone(), flat_index as u32))
            .find(|item| item.downcast::<BoxShadow>().is_some())
            .expect("box shadow");
        assert!(!box_shadow.is_injected_wrapper());

        let ordinary_clip = (0..instance.inner.vrc().item_table.len())
            .map(|flat_index| ItemRc::new(root_item_tree.clone(), flat_index as u32))
            .find(|item| {
                item.downcast::<Clip>().is_some_and(|clip| !clip.as_pin_ref().is_visibility_clip())
            })
            .expect("ordinary clip");
        assert!(!ordinary_clip.is_injected_wrapper());
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
    clipped := Rectangle {
        clip: true;
        x: 90px;
        y: 15px;
        width: 30px;
        height: 25px;
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
        check("clipped", (90.0, 15.0));
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
        let geometries = instance.component_positions(&test_path(), offset as u32);
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
