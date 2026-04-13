// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! `ItemTreeVTable` implementation for [`Instance`].
//!
//! A single static vtable serves every runtime `Instance`; vtable calls
//! walk the instance's sub-component tree on demand rather than through
//! a precomputed offset table.

use crate::instance::Instance;
use i_slint_core::SharedString;
use i_slint_core::accessibility::{
    AccessibilityAction, AccessibleStringProperty, SupportedAccessibilityAction,
};
use i_slint_core::item_tree::{
    IndexRange, ItemTree, ItemTreeNode, ItemTreeVTable, ItemVisitorVTable, ItemWeak,
    TraversalOrder, VisitChildrenResult,
};
use i_slint_core::items::{AccessibleRole, ItemVTable};
use i_slint_core::layout::{LayoutInfo, Orientation};
use i_slint_core::lengths::LogicalRect;
use i_slint_core::slice::Slice;
use i_slint_core::window::WindowAdapterRc;
use std::pin::Pin;
use vtable::{VRef, VRefMut, VWeak};

i_slint_core::ItemTreeVTable_static!(static INTERPRETER_INSTANCE_VT for Instance);

/// Find the `sub_component_path` (sequence of `SubComponentInstanceIdx`)
/// from the parent instance's root to the given sub-component. Used by
/// `parent_node` to match entries in the parent's `dynamic_table`.
pub(crate) fn sub_component_path_of(
    target: &crate::instance::SubComponentInstance,
    parent_root: &Instance,
) -> Vec<i_slint_compiler::llr::SubComponentInstanceIdx> {
    fn walk(
        current: &crate::instance::SubComponentInstance,
        target_ptr: *const crate::instance::SubComponentInstance,
        path: &mut Vec<i_slint_compiler::llr::SubComponentInstanceIdx>,
    ) -> bool {
        if std::ptr::eq(current as *const _, target_ptr) {
            return true;
        }
        for (idx, nested) in current.sub_components.iter().enumerate() {
            path.push(idx.into());
            if walk(nested, target_ptr, path) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut path = Vec::new();
    walk(&parent_root.root_sub_component, target as *const _, &mut path);
    path
}

impl i_slint_core::item_tree::ItemTree for Instance {
    fn visit_children_item(
        self: Pin<&Self>,
        index: isize,
        order: TraversalOrder,
        visitor: VRefMut<'_, ItemVisitorVTable>,
    ) -> VisitChildrenResult {
        let this = self.get_ref();
        let weak = this.self_weak.get().unwrap().clone();
        i_slint_core::item_tree::visit_item_tree(
            self,
            &vtable::VRc::into_dyn(weak.upgrade().unwrap()),
            &this.tree_nodes[..],
            index,
            order,
            visitor,
            |_self, order, visitor, dyn_index| {
                _self.visit_dynamic_children(dyn_index, order, visitor)
            },
        )
    }

    fn get_item_ref(self: Pin<&Self>, index: u32) -> Pin<VRef<'_, ItemVTable>> {
        // The item_table is indexed by flat tree index (same ordering as
        // `tree_nodes`), pointing at the sub-component path + item slot
        // that backs each static item node.
        let this = self.get_ref();
        let entry = this
            .item_table
            .get(index as usize)
            .and_then(Option::as_ref)
            .expect("get_item_ref: tree index is not a static item");
        // Walk the path by borrowing — every intermediate sub-component
        // is owned by its parent via `sub_components`, so a reference
        // to the leaf is valid for the lifetime of `self`.
        let mut current: &crate::instance::SubComponentInstance = &this.root_sub_component;
        for &sub_idx in entry.0.iter() {
            current = &current.sub_components[sub_idx];
        }
        Pin::as_ref(&current.items[entry.1]).as_item_ref()
    }

    fn get_subtree_range(self: Pin<&Self>, index: u32) -> IndexRange {
        let Some((sub, rep_idx)) = self.get_ref().dynamic_at(index) else {
            return IndexRange { start: 0, end: 0 };
        };
        let repeater = &sub.repeaters[rep_idx];
        // Trigger lazy instantiation with the right init closure.
        self.get_ref().ensure_updated(index);
        let range = repeater.range();
        IndexRange { start: range.start, end: range.end }
    }

    fn get_subtree(
        self: Pin<&Self>,
        index: u32,
        subindex: usize,
        result: &mut VWeak<ItemTreeVTable, vtable::Dyn>,
    ) {
        self.get_ref().ensure_updated(index);
        let Some((sub, rep_idx)) = self.get_ref().dynamic_at(index) else {
            return;
        };
        let repeater = &sub.repeaters[rep_idx];
        if let Some(instance) = repeater.instance_at(subindex) {
            *result = vtable::VRc::downgrade(&vtable::VRc::into_dyn(instance));
        }
    }

    fn get_item_tree(self: Pin<&Self>) -> Slice<'_, ItemTreeNode> {
        Slice::from(&*self.get_ref().tree_nodes)
    }

    fn parent_node(self: Pin<&Self>, result: &mut ItemWeak) {
        // If this is a repeated sub-tree, point at the repeater's placeholder
        // in the parent instance. For a popup (parented but not repeated),
        // point at the parent instance's root item.
        let this = self.get_ref();
        // `embed_component` recorded where in the outer item tree this
        // instance lives. Return that as the parent; the core walks back
        // through it the same way as a repeated DynamicTree node.
        if let Some((outer_weak, outer_index)) = this.embedded_in.get()
            && let Some(outer) = outer_weak.upgrade()
        {
            *result = i_slint_core::items::ItemRc::new(outer, *outer_index).downgrade();
            return;
        }
        let Some(parent_sub) = this.parent_instance.upgrade() else { return };
        let Some(parent_root_vrc) = parent_sub.root.get().and_then(|w| w.upgrade()) else {
            return;
        };
        let parent_dyn = vtable::VRc::into_dyn(parent_root_vrc.clone());
        if let Some((_, repeater_idx)) = this.root_sub_component.repeated_in.get() {
            // Return the DynamicTree node itself in the parent's flat tree.
            // `parent_item` in i_slint_core detects that the returned parent
            // is a DynamicTree and walks one more level up to its parent
            // item. Returning the DynamicTree's own parent here skips that
            // adjustment and gives the caller the wrong node.
            let rep_idx = *repeater_idx;
            let parent_path = sub_component_path_of(&parent_sub, &parent_root_vrc);
            for (flat, entry) in parent_root_vrc.dynamic_table.iter().enumerate() {
                if let Some((path, idx)) = entry.as_ref()
                    && path.as_ref() == parent_path.as_slice()
                    && *idx == rep_idx
                {
                    *result = i_slint_core::items::ItemRc::new(parent_dyn, flat as u32).downgrade();
                    return;
                }
            }
        } else {
            // Popup case: ItemRc::new_root on the parent instance, which the
            // caller uses to traverse up to the window.
            *result = i_slint_core::items::ItemRc::new(parent_dyn, 0).downgrade();
        }
    }

    fn embed_component(
        self: Pin<&Self>,
        parent: &VWeak<ItemTreeVTable>,
        parent_item_tree_index: u32,
    ) -> bool {
        // Stash the outer item tree handle so `parent_node` can point at
        // the ComponentContainer slot that substitutes this instance in.
        let this = self.get_ref();
        this.embedded_in.set((parent.clone(), parent_item_tree_index)).is_ok()
    }

    fn subtree_index(self: Pin<&Self>) -> usize {
        // For repeated instances, return the model index so tab-focus
        // traversal can step to the next sibling via get_subtree(idx+1).
        let this = self.get_ref();
        let sc = &this.root_sub_component.compilation_unit.sub_components
            [this.root_sub_component.sub_component_idx];
        for (idx, prop) in sc.properties.iter_enumerated() {
            if prop.name == "model_index" {
                if let crate::Value::Number(n) =
                    Pin::as_ref(&this.root_sub_component.properties[idx]).get()
                {
                    return n as usize;
                }
            }
        }
        // Conditional: only one instance, index 0.
        0
    }

    fn layout_info(self: Pin<&Self>, orientation: Orientation) -> LayoutInfo {
        let this = self.get_ref();
        let sc_idx = this.root_sub_component.sub_component_idx;
        let cu = &this.root_sub_component.compilation_unit;
        let sc = &cu.sub_components[sc_idx];
        let expr = match orientation {
            Orientation::Horizontal => sc.layout_info_h.borrow(),
            Orientation::Vertical => sc.layout_info_v.borrow(),
        };
        let mut ctx = crate::eval::EvalContext::new(this.root_sub_component.clone());
        crate::eval::eval_expression(&mut ctx, &expr).try_into().unwrap_or_default()
    }

    fn item_geometry(self: Pin<&Self>, item_index: u32) -> LogicalRect {
        // `item_index` is the flat tree index. Resolve it via `item_table`
        // into the owning sub-component, then look up the geometry by
        // the item's `index_in_tree`. `sc.geometries` is keyed by the
        // sub-component-local tree index (set by `generate_item_indices`),
        // not by the raw `ItemInstanceIdx` slot.
        let this = self.get_ref();
        let Some(entry) = this.item_table.get(item_index as usize).and_then(Option::as_ref) else {
            return LogicalRect::default();
        };
        let mut owner_rc = this.root_sub_component.clone();
        for &sub_idx in entry.0.iter() {
            owner_rc = owner_rc.sub_components[sub_idx].clone();
        }
        let cu = owner_rc.compilation_unit.clone();
        let sc = &cu.sub_components[owner_rc.sub_component_idx];
        let item = &sc.items[entry.1];
        // When the flat tree crosses into a sub-component (non-empty path)
        // and we land on its root element (local tree index 0), the
        // sub-component's own root geometry can duplicate the parent's
        // placement. Example: material `SpinBox` wraps its `SpinBoxBase`
        // instance in an auto-generated `Opacity` item whose geometry
        // already carries the center-in-parent offset; the sub-component's
        // internal root then *also* applies the same offset via its
        // `y: root-1_y` binding, double-counting it.
        //
        // The compiler's `lower_property_to_element` +
        // `adjust_geometry_for_injected_parent` passes retarget the outer
        // element's `x`/`y` to a dummy (0) and hoist the original position
        // into the injected wrapper, so rust codegen reads the wrapper's
        // geometry from the *parent* sub-component at the placement slot
        // and never queries the inner root's geometry. Mirror that: when
        // the path is non-empty and we're on the sub-component's root,
        // return the parent's geometry entry for the instance's
        // `index_in_tree` instead of the inner root's own centering-aware
        // geometry. Fall through if the parent has no entry (the sub-
        // component was placed directly with no wrapper, e.g.
        // `box := SpinBox {}` inside a Window — in that case the
        // sub-component's own root geometry is the correct placement).
        let parent_placement = if !entry.0.is_empty() && item.index_in_tree == 0 {
            let mut parent_rc = this.root_sub_component.clone();
            for &sub_idx in &entry.0[..entry.0.len() - 1] {
                parent_rc = parent_rc.sub_components[sub_idx].clone();
            }
            let placement = entry.0[entry.0.len() - 1];
            let parent_sc = &cu.sub_components[parent_rc.sub_component_idx];
            let placement_idx = parent_sc.sub_components[placement].index_in_tree as usize;
            parent_sc
                .geometries
                .get(placement_idx)
                .and_then(|g| g.clone())
                .map(|expr| (expr, parent_rc))
        } else {
            None
        };
        let (expr_cell, ctx_owner) = if let Some(pair) = parent_placement {
            pair
        } else {
            let tree_local_idx = item.index_in_tree as usize;
            match sc.geometries.get(tree_local_idx) {
                Some(Some(expr)) => (expr.clone(), owner_rc),
                _ => return LogicalRect::default(),
            }
        };
        let expr = expr_cell.borrow();
        let mut ctx = crate::eval::EvalContext::new(ctx_owner);
        let crate::Value::Struct(s) = crate::eval::eval_expression(&mut ctx, &expr) else {
            return LogicalRect::default();
        };
        let as_f32 = |name: &str| -> f32 {
            match s.get_field(name) {
                Some(crate::Value::Number(n)) => *n as f32,
                _ => 0.0,
            }
        };
        LogicalRect::new(
            i_slint_core::lengths::LogicalPoint::new(as_f32("x"), as_f32("y")),
            i_slint_core::lengths::LogicalSize::new(as_f32("width"), as_f32("height")),
        )
    }

    fn accessible_role(self: Pin<&Self>, item_index: u32) -> AccessibleRole {
        let Some((owner, local_idx)) = resolve_accessible_item(self.get_ref(), item_index) else {
            return AccessibleRole::default();
        };
        let cu = owner.compilation_unit.clone();
        let sc = &cu.sub_components[owner.sub_component_idx];
        let Some(expr) = sc.accessible_prop.get(&(local_idx, "Role".to_string())) else {
            return AccessibleRole::default();
        };
        let expr = expr.borrow().clone();
        let mut ctx = crate::eval::EvalContext::new(owner);
        crate::eval::eval_expression(&mut ctx, &expr).try_into().unwrap_or_default()
    }

    fn accessible_string_property(
        self: Pin<&Self>,
        item_index: u32,
        what: AccessibleStringProperty,
        result: &mut SharedString,
    ) -> bool {
        let what_str = accessible_string_property_name(what);
        for (owner, local_idx) in resolve_accessible_candidates(self.get_ref(), item_index) {
            let cu = owner.compilation_unit.clone();
            let sc = &cu.sub_components[owner.sub_component_idx];
            if let Some(expr) = sc.accessible_prop.get(&(local_idx, what_str.into())) {
                let expr = expr.borrow().clone();
                let mut ctx = crate::eval::EvalContext::new(owner);
                if let crate::Value::String(s) = crate::eval::eval_expression(&mut ctx, &expr) {
                    *result = s;
                    return true;
                }
            }
        }
        false
    }

    fn accessibility_action(self: Pin<&Self>, item_index: u32, action: &AccessibilityAction) {
        let what = format!("Action{}", accessibility_action_name(action));
        for (owner, local_idx) in resolve_accessible_candidates(self.get_ref(), item_index) {
            let cu = owner.compilation_unit.clone();
            let sc = &cu.sub_components[owner.sub_component_idx];
            if let Some(expr) = sc.accessible_prop.get(&(local_idx, what.clone())) {
                let expr = expr.borrow().clone();
                let args = accessibility_action_args(action);
                let mut ctx = crate::eval::EvalContext::with_arguments(owner, args);
                crate::eval::eval_expression(&mut ctx, &expr);
                return;
            }
        }
    }

    fn supported_accessibility_actions(
        self: Pin<&Self>,
        item_index: u32,
    ) -> SupportedAccessibilityAction {
        let mut actions = SupportedAccessibilityAction::default();
        for (owner, local_idx) in resolve_accessible_candidates(self.get_ref(), item_index) {
            let cu = owner.compilation_unit.clone();
            let sc = &cu.sub_components[owner.sub_component_idx];
            for ((idx, key), _) in &sc.accessible_prop {
                if *idx == local_idx
                    && let Some(action_name) = key.strip_prefix("Action")
                {
                    actions |= match action_name {
                        "Default" => SupportedAccessibilityAction::Default,
                        "Decrement" => SupportedAccessibilityAction::Decrement,
                        "Increment" => SupportedAccessibilityAction::Increment,
                        "Expand" => SupportedAccessibilityAction::Expand,
                        "ReplaceSelectedText" => SupportedAccessibilityAction::ReplaceSelectedText,
                        "SetValue" => SupportedAccessibilityAction::SetValue,
                        _ => SupportedAccessibilityAction::default(),
                    };
                }
            }
        }
        actions
    }

    fn item_element_infos(self: Pin<&Self>, item_index: u32, result: &mut SharedString) -> bool {
        let this = self.get_ref();
        let Some(entry) = this.item_table.get(item_index as usize).and_then(Option::as_ref) else {
            return false;
        };
        let cu = &this.root_sub_component.compilation_unit;
        // For sub-component references, the wrapping element_infos
        // (e.g. `Button,TestCase::second,...`) is in the *parent*'s
        // element_infos map, keyed by the parent-local index where the
        // sub-component starts. Check the root's entry directly.
        if entry.0.is_empty() {
            // The item is in the root sub-component.
            let sc = &cu.sub_components[this.root_sub_component.sub_component_idx];
            let item_local_idx = sc.items[entry.1].index_in_tree;
            return sc
                .element_infos
                .get(&item_local_idx)
                .map(|infos| {
                    *result = infos.as_str().into();
                })
                .is_some();
        }
        // The item is inside a sub-component. First, check whether
        // `item_index` corresponds to the wrapping sub-component
        // reference at the root level — its element_infos is on the
        // root and includes the inner inheritance chain.
        let root_sc = &cu.sub_components[this.root_sub_component.sub_component_idx];
        if let Some(infos) = root_sc.element_infos.get(&item_index) {
            *result = infos.as_str().into();
            return true;
        }
        // Otherwise, walk down to the deepest sub-component and use
        // the local index.
        let mut owner: &crate::instance::SubComponentInstance = &this.root_sub_component;
        for &sub_idx in entry.0.iter() {
            owner = &owner.sub_components[sub_idx];
        }
        let sc = &cu.sub_components[owner.sub_component_idx];
        let item_local_idx = sc.items[entry.1].index_in_tree;
        if let Some(infos) = sc.element_infos.get(&item_local_idx) {
            *result = infos.as_str().into();
            true
        } else {
            false
        }
    }

    fn window_adapter(self: Pin<&Self>, do_create: bool, result: &mut Option<WindowAdapterRc>) {
        // A repeated instance's own `window_adapter` is unset; walk up via
        // `parent_instance` to the root `Instance` and read its adapter.
        let this = self.get_ref();
        if let Some(adapter) = this.window_adapter.get() {
            *result = Some(adapter.clone());
            return;
        }
        let mut parent_sub = this.parent_instance.upgrade();
        while let Some(sub) = parent_sub {
            let Some(root_vrc) = sub.root.get().and_then(|w| w.upgrade()) else { break };
            if let Some(adapter) = root_vrc.window_adapter.get() {
                *result = Some(adapter.clone());
                return;
            }
            parent_sub = root_vrc.parent_instance.upgrade();
        }
        if do_create {
            *result = this.window_adapter_or_default();
        }
    }
}

/// Resolve a flat tree index to (owning sub-component, local index_in_tree)
/// for accessibility lookups.
fn resolve_accessible_item(
    instance: &Instance,
    item_index: u32,
) -> Option<(Pin<std::rc::Rc<crate::instance::SubComponentInstance>>, u32)> {
    let entry = instance.item_table.get(item_index as usize).and_then(Option::as_ref)?;
    let mut owner = instance.root_sub_component.clone();
    for &sub_idx in entry.0.iter() {
        let next = owner.sub_components[sub_idx].clone();
        owner = next;
    }
    let cu = &owner.compilation_unit;
    let sc = &cu.sub_components[owner.sub_component_idx];
    let local_idx = sc.items[entry.1].index_in_tree;
    Some((owner, local_idx))
}

/// Returns the candidates to look up an accessible property for a given
/// flat tree index. The first candidate is the wrapping sub-component
/// reference at the root level (if applicable); the second is the
/// deepest item itself. This matches the Rust codegen which routes
/// outer-element queries to the inner sub-component's root.
fn resolve_accessible_candidates(
    instance: &Instance,
    item_index: u32,
) -> Vec<(Pin<std::rc::Rc<crate::instance::SubComponentInstance>>, u32)> {
    let mut out = Vec::new();
    let Some(entry) = instance.item_table.get(item_index as usize).and_then(Option::as_ref) else {
        return out;
    };
    // First candidate: a wrapping sub-component reference at the root.
    // Its accessible_prop entry is keyed by the root-local flat index.
    if !entry.0.is_empty() {
        out.push((instance.root_sub_component.clone(), item_index));
    }
    // Second candidate: the deepest item itself.
    let mut owner = instance.root_sub_component.clone();
    for &sub_idx in entry.0.iter() {
        let next = owner.sub_components[sub_idx].clone();
        owner = next;
    }
    let cu = &owner.compilation_unit;
    let sc = &cu.sub_components[owner.sub_component_idx];
    let local_idx = sc.items[entry.1].index_in_tree;
    out.push((owner, local_idx));
    out
}

fn accessible_string_property_name(what: AccessibleStringProperty) -> &'static str {
    match what {
        AccessibleStringProperty::Checkable => "Checkable",
        AccessibleStringProperty::Checked => "Checked",
        AccessibleStringProperty::DelegateFocus => "DelegateFocus",
        AccessibleStringProperty::Description => "Description",
        AccessibleStringProperty::Enabled => "Enabled",
        AccessibleStringProperty::Expandable => "Expandable",
        AccessibleStringProperty::Expanded => "Expanded",
        AccessibleStringProperty::Id => "Id",
        AccessibleStringProperty::ItemCount => "ItemCount",
        AccessibleStringProperty::ItemIndex => "ItemIndex",
        AccessibleStringProperty::ItemSelectable => "ItemSelectable",
        AccessibleStringProperty::ItemSelected => "ItemSelected",
        AccessibleStringProperty::Label => "Label",
        AccessibleStringProperty::PlaceholderText => "PlaceholderText",
        AccessibleStringProperty::ReadOnly => "ReadOnly",
        AccessibleStringProperty::Value => "Value",
        AccessibleStringProperty::ValueMaximum => "ValueMaximum",
        AccessibleStringProperty::ValueMinimum => "ValueMinimum",
        AccessibleStringProperty::ValueStep => "ValueStep",
    }
}

fn accessibility_action_name(action: &AccessibilityAction) -> &'static str {
    match action {
        AccessibilityAction::Default => "Default",
        AccessibilityAction::Decrement => "Decrement",
        AccessibilityAction::Increment => "Increment",
        AccessibilityAction::Expand => "Expand",
        AccessibilityAction::ReplaceSelectedText(_) => "ReplaceSelectedText",
        AccessibilityAction::SetValue(_) => "SetValue",
    }
}

fn accessibility_action_args(action: &AccessibilityAction) -> Vec<crate::Value> {
    match action {
        AccessibilityAction::ReplaceSelectedText(s) | AccessibilityAction::SetValue(s) => {
            vec![crate::Value::String(s.clone())]
        }
        _ => Vec::new(),
    }
}
