// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Highlight support for running component instances.
//!
use crate::instance::{Instance, SubComponentInstance};
use i_slint_compiler::diagnostics::SourceLocation;
use i_slint_compiler::llr::ItemInstanceIdx;
use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::source_path::SourcePath;
use i_slint_core::graphics::euclid;
use i_slint_core::item_tree::{ItemTreeRc, ItemTreeVTable, TraversalOrder, VisitChildrenResult};
use i_slint_core::items::ItemRc;
use i_slint_core::lengths::{ItemTransform, LogicalPoint, LogicalRect, LogicalVector};
use std::cell::OnceCell;
use std::collections::{BTreeMap, HashMap, HashSet};
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
    root: &VRc<ItemTreeVTable, Instance>,
    path: &SourcePath,
    offset: u32,
) -> Vec<HighlightedRect> {
    let sources = SourceOccurrences::new(root);
    sources.matching_occurrences(path, offset).map(|(_, geometry)| geometry).collect()
}

pub(crate) fn element_candidates_at(
    root: &VRc<ItemTreeVTable, Instance>,
    position: LogicalPoint,
) -> Vec<ElementCandidate> {
    let sources = SourceOccurrences::new(root);
    let root_item_tree = VRc::into_dyn(root.clone());
    let mut runtime_items = Vec::new();
    collect_runtime_items_front_to_back(&root_item_tree, 0, Some(root), &mut runtime_items);

    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let mut seen_enclosing_elements = HashSet::new();
    for (instance, flat_index) in runtime_items {
        let Some(&item_index) =
            sources.item_indices.get(&(std::ptr::from_ref(&*instance), flat_index))
        else {
            continue;
        };
        let item = &sources.items[item_index];
        let Some(geometry) = item.geometry(root) else { continue };
        if !geometry.contains(position) || !item.as_item_rc().is_visible() {
            continue;
        }

        let mut add_candidates = |source_location: &SourceLocation, item_indices: &[usize]| {
            for &item_index in item_indices {
                let Some(candidate) = sources.candidate_at(source_location, item_index, position)
                else {
                    continue;
                };
                if let Some(key) = source_key(source_location)
                    && seen.insert((key, candidate.instance_index))
                {
                    candidates.push(candidate);
                }
            }
        };
        let definition = &item.owner.compilation_unit.sub_components[item.owner.sub_component_idx];
        if let Some(debug_info) = &definition.debug_info
            && let Some(debug_entries) = debug_info.items.get(item.local_item_index)
        {
            for debug_entry in debug_entries.iter().rev() {
                add_candidates(&debug_entry.source_location, &[item_index]);
            }
        }

        let mut owner = Some(item.owner.clone());
        while let Some(component) = owner {
            let component_pointer = std::ptr::from_ref(&*component);
            if seen_enclosing_elements.insert(component_pointer)
                && let Some((source_location, item_indices)) =
                    sources.enclosing_elements.get(&component_pointer)
            {
                add_candidates(source_location, item_indices);
            }
            owner = component.parent.upgrade().map(Pin::new);
        }
    }
    candidates
}

// The same element can have source ranges of different lengths.
// Use only its file path and starting byte offset to group those entries.
type SourceKey = (SourcePath, usize);
type SourceElements = BTreeMap<SourceKey, SourceElement>;

struct RuntimeItem {
    instance: VRc<ItemTreeVTable, Instance>,
    flat_index: usize,
    owner: Pin<Rc<SubComponentInstance>>,
    local_item_index: ItemInstanceIdx,
    geometry: OnceCell<Option<HighlightedRect>>,
}

impl RuntimeItem {
    fn as_item_rc(&self) -> ItemRc {
        ItemRc::new(VRc::into_dyn(self.instance.clone()), self.flat_index as u32)
    }

    fn geometry(&self, root: &VRc<ItemTreeVTable, Instance>) -> Option<HighlightedRect> {
        *self.geometry.get_or_init(|| item_geometry(&self.instance, root, self.flat_index))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourcePriority {
    Item,
    ComponentUse,
    RepeatedElement,
}

struct SourceElement {
    source_location: SourceLocation,
    // Indices into `SourceOccurrences.items` for the runtime items representing this source element.
    item_indices: Vec<usize>,
    priority: SourcePriority,
    // Maps an index in `SourceOccurrences.items` to its position in `component_positions()`, skipping empty geometry.
    instance_indices: OnceCell<HashMap<usize, usize>>,
}

impl SourceElement {
    fn new(
        source_location: &SourceLocation,
        item_indices: Vec<usize>,
        priority: SourcePriority,
    ) -> Self {
        Self {
            source_location: source_location.clone(),
            item_indices,
            priority,
            instance_indices: OnceCell::new(),
        }
    }

    fn occurrences<'a>(
        &'a self,
        items: &'a [RuntimeItem],
        root: &'a VRc<ItemTreeVTable, Instance>,
    ) -> impl Iterator<Item = (usize, HighlightedRect)> + 'a {
        self.item_indices.iter().filter_map(move |&item_index| {
            items[item_index].geometry(root).map(|geometry| (item_index, geometry))
        })
    }
}

struct SourceOccurrences<'a> {
    root: &'a VRc<ItemTreeVTable, Instance>,
    items: Vec<RuntimeItem>,
    // Maps an owning `Instance` and its flat item index to an index in `items`.
    item_indices: HashMap<(*const Instance, usize), usize>,
    elements: SourceElements,
    // Indices into `items` for the items whose geometry represents this component use or repeated row.
    enclosing_elements: HashMap<*const SubComponentInstance, (SourceLocation, Vec<usize>)>,
}

impl<'a> SourceOccurrences<'a> {
    fn new(root: &'a VRc<ItemTreeVTable, Instance>) -> Self {
        i_slint_core::item_tree::ensure_item_tree_instantiated(&VRc::into_dyn(root.clone()));
        let mut sources = Self {
            root,
            items: Vec::new(),
            item_indices: HashMap::new(),
            elements: BTreeMap::new(),
            enclosing_elements: HashMap::new(),
        };
        let mut local_item_indices = HashMap::new();
        for instance in all_instances(root) {
            for (flat_index, entry) in instance.item_table.iter().enumerate() {
                let Some((path, local_item_index)) = entry else { continue };
                let mut owner = instance.root_sub_component.clone();
                for &component_index in path {
                    owner = owner.sub_components[component_index].clone();
                }
                let item_index = sources.items.len();
                local_item_indices
                    .insert((std::ptr::from_ref(&*owner), *local_item_index), item_index);
                sources
                    .item_indices
                    .insert((std::ptr::from_ref(&*instance), flat_index), item_index);
                sources.items.push(RuntimeItem {
                    instance: instance.clone(),
                    flat_index,
                    owner,
                    local_item_index: *local_item_index,
                    geometry: OnceCell::new(),
                });
            }
        }
        sources.elements = sources.collect_sources(&root.root_sub_component, &local_item_indices);
        for element in sources.elements.values_mut() {
            element.item_indices.sort_unstable();
            element.item_indices.dedup();
        }
        for (_, item_indices) in sources.enclosing_elements.values_mut() {
            item_indices.sort_unstable();
            item_indices.dedup();
        }
        sources
    }

    fn collect_sources(
        &mut self,
        component: &Pin<Rc<SubComponentInstance>>,
        // Maps a sub-component instance and an index in its `items` array to an index in `SourceOccurrences.items`.
        local_item_indices: &HashMap<(*const SubComponentInstance, ItemInstanceIdx), usize>,
    ) -> SourceElements {
        let definition = &component.compilation_unit.sub_components[component.sub_component_idx];
        let mut elements = BTreeMap::new();
        if let Some(debug_info) = &definition.debug_info {
            for (local_item_index, debug_entries) in debug_info.items.iter_enumerated() {
                let Some(&item_index) =
                    local_item_indices.get(&(std::ptr::from_ref(&**component), local_item_index))
                else {
                    continue;
                };
                for debug_entry in debug_entries {
                    merge_sources(
                        &mut elements,
                        [SourceElement::new(
                            &debug_entry.source_location,
                            vec![item_index],
                            SourcePriority::Item,
                        )],
                    );
                }
            }
        }

        for (component_index, child) in component.sub_components.iter_enumerated() {
            let child_elements = self.collect_sources(child, local_item_indices);
            if let Some(source_location) = definition
                .debug_info
                .as_ref()
                .and_then(|debug_info| debug_info.sub_component_use_sites.get(component_index))
            {
                let root_items = component_root_items(child, &child_elements, source_location);
                self.enclosing_elements.insert(
                    std::ptr::from_ref(&**child),
                    (source_location.clone(), root_items.clone()),
                );
                merge_sources(
                    &mut elements,
                    [SourceElement::new(source_location, root_items, SourcePriority::ComponentUse)],
                );
            }
            merge_sources(&mut elements, child_elements.into_values());
        }

        for (repeated_index, repeater) in component.repeaters.iter_enumerated() {
            let source_location = definition
                .debug_info
                .as_ref()
                .and_then(|debug_info| debug_info.repeated_elements.get(repeated_index));
            let mut root_items = Vec::new();
            for row in repeater.instances_vec() {
                let row_elements =
                    self.collect_sources(&row.root_sub_component, local_item_indices);
                if let Some(source_location) = source_location {
                    let row_root_items = component_root_items(
                        &row.root_sub_component,
                        &row_elements,
                        source_location,
                    );
                    self.enclosing_elements.insert(
                        std::ptr::from_ref(&*row.root_sub_component),
                        (source_location.clone(), row_root_items.clone()),
                    );
                    root_items.extend(row_root_items);
                }
                merge_sources(&mut elements, row_elements.into_values());
            }
            if let Some(source_location) = source_location {
                merge_sources(
                    &mut elements,
                    [SourceElement::new(
                        source_location,
                        root_items,
                        SourcePriority::RepeatedElement,
                    )],
                );
            }
        }
        elements
    }

    fn matching_occurrences(
        &self,
        path: &SourcePath,
        offset: u32,
    ) -> impl Iterator<Item = (usize, HighlightedRect)> {
        self.elements
            .values()
            .filter(|element| source_location_contains(&element.source_location, path, offset))
            .flat_map(|element| element.occurrences(&self.items, self.root))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
    }

    fn candidate_at(
        &self,
        source_location: &SourceLocation,
        item_index: usize,
        position: LogicalPoint,
    ) -> Option<ElementCandidate> {
        let element = self.elements.get(&source_key(source_location)?)?;
        let instance_indices = element.instance_indices.get_or_init(|| {
            element
                .occurrences(&self.items, self.root)
                .enumerate()
                .map(|(instance_index, (item_index, _))| (item_index, instance_index))
                .collect()
        });
        let &instance_index = instance_indices.get(&item_index)?;
        let item = &self.items[item_index];
        let geometry = item.geometry(self.root)?;
        if !geometry.contains(position) || !item.as_item_rc().is_visible() {
            return None;
        }
        Some(ElementCandidate {
            source_location: element.source_location.clone(),
            geometry,
            instance_index,
        })
    }
}

fn source_key(source_location: &SourceLocation) -> Option<SourceKey> {
    Some((source_location.source_file.as_ref()?.path().clone(), source_location.span.offset))
}

fn merge_sources(elements: &mut SourceElements, sources: impl IntoIterator<Item = SourceElement>) {
    for source in sources {
        let Some(key) = source_key(&source.source_location) else { continue };
        let element = elements.entry(key).or_insert_with(|| {
            SourceElement::new(&source.source_location, Vec::new(), source.priority)
        });
        if source.priority > element.priority {
            *element = source;
        } else if source.priority == element.priority {
            element.item_indices.extend(source.item_indices);
        }
    }
}

fn component_root_items(
    component: &SubComponentInstance,
    elements: &SourceElements,
    source_location: &SourceLocation,
) -> Vec<usize> {
    component.compilation_unit.sub_components[component.sub_component_idx]
        .debug_info
        .as_ref()
        .and_then(|debug_info| source_key(&debug_info.source_location))
        .or_else(|| source_key(source_location))
        .and_then(|key| elements.get(&key))
        .map(|element| element.item_indices.clone())
        .unwrap_or_default()
}

/// Look up the `(ElementRc, index)` tuples whose `debug` entries cover
/// the given source offset. Uses the `TypeLoader` stored on the instance
/// (if available) to walk the original object-tree `Document`.
pub(crate) fn element_node_at_source_code_position(
    instance: &VRc<ItemTreeVTable, Instance>,
    path: &SourcePath,
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
    path: &SourcePath,
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
    index: u32,
    instance: Option<&VRc<ItemTreeVTable, Instance>>,
    runtime_items: &mut Vec<(VRc<ItemTreeVTable, Instance>, usize)>,
) {
    let mut visit_child = |child_item_tree: &ItemTreeRc,
                           child_index: u32,
                           _: Pin<i_slint_core::items::ItemRef<'_>>|
     -> VisitChildrenResult {
        let resolved_instance;
        let child_instance = if VRc::ptr_eq(child_item_tree, item_tree) {
            instance
        } else {
            resolved_instance = VRc::borrow(child_item_tree)
                .downcast::<Instance>()
                .and_then(|instance| instance.self_weak.get())
                .and_then(|instance| instance.upgrade());
            resolved_instance.as_ref()
        };
        collect_runtime_items_front_to_back(
            child_item_tree,
            child_index,
            child_instance,
            runtime_items,
        );
        VisitChildrenResult::CONTINUE
    };
    vtable::new_vref!(
        let mut visit_child: VRefMut<i_slint_core::item_tree::ItemVisitorVTable>
            for i_slint_core::item_tree::ItemVisitor = &mut visit_child
    );
    VRc::borrow_pin(item_tree).as_ref().visit_children_item(
        index as isize,
        TraversalOrder::FrontToBack,
        visit_child,
    );
    if let Some(instance) = instance {
        runtime_items.push((instance.clone(), index as usize));
    }
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
        if !parent.is_geometry_wrapper() {
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

#[cfg(all(test, feature = "internal"))]
fn items_by_source(
    root: &VRc<ItemTreeVTable, Instance>,
    target_path: &SourcePath,
    target_offset: u32,
    _source_match: SourceMatch,
) -> Vec<(VRc<ItemTreeVTable, Instance>, usize)> {
    let sources = SourceOccurrences::new(root);
    sources
        .elements
        .get(&(target_path.clone(), target_offset as usize))
        .into_iter()
        .flat_map(|element| &element.item_indices)
        .map(|&item_index| {
            let item = &sources.items[item_index];
            (item.instance.clone(), item.flat_index)
        })
        .collect()
}

#[cfg(all(test, feature = "internal"))]
enum SourceMatch {
    Start,
}

fn source_location_contains(
    source_location: &SourceLocation,
    target_path: &SourcePath,
    target_offset: u32,
) -> bool {
    let Some(source_file) = source_location.source_file.as_ref() else { return false };
    if source_file.path() != target_path {
        return false;
    }
    let target_offset = target_offset as usize;
    source_location.span.offset <= target_offset
        && target_offset < source_location.span.offset.saturating_add(source_location.span.length)
}

#[cfg(all(test, feature = "internal"))]
mod tests {
    use crate::{
        ComponentInstance,
        debug_hook::tests::{compile_with_debug_hooks, test_path},
    };
    use i_slint_compiler::source_path::SourcePath;
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
        *instance
            .component_positions(&SourcePath::new(test_path()), offset as u32)
            .first()
            .expect("geometry")
    }

    fn runtime_item_of(instance: &ComponentInstance, source: &str, element_id: &str) -> ItemRc {
        let element_id_position =
            source.find(element_id).unwrap_or_else(|| panic!("{element_id} not found"));
        let offset = element_id_position + source[element_id_position..].find("Rectangle").unwrap();
        let (runtime_instance, flat_index) = super::items_by_source(
            instance.inner.vrc(),
            &SourcePath::new(test_path()),
            offset as u32,
            super::SourceMatch::Start,
        )
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("runtime item for {element_id}"));
        ItemRc::new(VRc::into_dyn(runtime_instance), flat_index as u32)
    }

    #[test]
    fn geometry_wrapper_classification() {
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
                .map(ItemRc::is_geometry_wrapper),
            ancestors
                .iter()
                .find(|item| item.downcast::<Opacity>().is_some())
                .map(ItemRc::is_geometry_wrapper),
            ancestors
                .iter()
                .find(|item| item.downcast::<Layer>().is_some())
                .map(ItemRc::is_geometry_wrapper),
            ancestors
                .iter()
                .find(|item| {
                    item.downcast::<Clip>()
                        .is_some_and(|clip| clip.as_pin_ref().is_visibility_clip())
                })
                .map(ItemRc::is_geometry_wrapper),
        ] {
            assert_eq!(wrapper_classification, Some(true));
        }

        let root_item_tree = VRc::into_dyn(instance.inner.vrc().clone());
        let box_shadow = (0..instance.inner.vrc().item_table.len())
            .map(|flat_index| ItemRc::new(root_item_tree.clone(), flat_index as u32))
            .find(|item| item.downcast::<BoxShadow>().is_some())
            .expect("box shadow");
        assert!(!box_shadow.is_geometry_wrapper());

        let ordinary_clip = (0..instance.inner.vrc().item_table.len())
            .map(|flat_index| ItemRc::new(root_item_tree.clone(), flat_index as u32))
            .find(|item| {
                item.downcast::<Clip>().is_some_and(|clip| !clip.as_pin_ref().is_visibility_clip())
            })
            .expect("ordinary clip");
        assert!(!ordinary_clip.is_geometry_wrapper());
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
        clipped-child := Rectangle {
            x: 3px;
            y: 4px;
            width: 10px;
            height: 12px;
        }
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

        for (element_id, expected_position) in [
            ("plain", (30.0, 40.0)),
            ("faded", (70.0, 80.0)),
            ("clipped", (90.0, 15.0)),
            ("clipped-child", (3.0, 4.0)),
            ("nested", (5.0, 7.0)),
        ] {
            let geometry = geometry_of(&instance, code, element_id);
            let horizontal_position = geometry.rect.origin.x - geometry.parent_origin().x;
            let vertical_position = geometry.rect.origin.y - geometry.parent_origin().y;
            assert!(
                (horizontal_position - expected_position.0).abs() < 0.5
                    && (vertical_position - expected_position.1).abs() < 0.5,
                "{element_id}: source-relative position ({horizontal_position}, {vertical_position}) should be {expected_position:?}"
            );
        }
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
        let geometries = instance.component_positions(&SourcePath::new(test_path()), offset as u32);
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
