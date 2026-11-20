// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Maps a window's item tree to an AccessKit node tree, independent of the platform adapter.
//!
//! The entire item tree is mapped to AccessKit's node tree. Any change to an individual accessible
//! item results in a tree update with just the changed nodes. Any change in the tree structure
//! results in a complete tree rebuild. This could be implemented more efficiently, but that isn't
//! essential: AccessKit avoids firing gratuitous events for full-tree updates as long as the node
//! IDs are stable.
//!
//! A backend owns the platform adapter and decides when to run the work: it implements
//! [`AccessKitHost`] to learn when something became dirty, and hands the [`TreeUpdate`]s that
//! [`AccessKitTree`] returns to its adapter. The backend must also call
//! [`AccessKitTree::build_new_tree`] when the tree structure changes (from
//! `WindowAdapterInternal::register_item_tree` and `unregister_item_tree`) and
//! [`AccessKitTree::focus_update`] when the focus changes (from
//! `WindowAdapterInternal::handle_focus_change`).

use alloc::boxed::Box;
use alloc::rc::{Rc, Weak};
use alloc::string::ToString;
use alloc::vec::Vec;
use core::pin::Pin;
use core::ptr::NonNull;
use std::collections::HashMap;

use accesskit::{
    Action, ActionRequest, Live, Node, NodeId, Orientation, Role, Toggled, TreeId, TreeInfo,
    TreeUpdate,
};

use crate::SharedString;
use crate::accessibility::{
    AccessibilityAction, AccessibleStringProperty, SupportedAccessibilityAction,
    find_exposed_flickable, find_exposed_text_input, nearest_accessible_item,
};
use crate::api::{LogicalPosition, Window};
use crate::input::FocusReason;
use crate::item_tree::{ItemTreeRc, ItemTreeRef, ItemTreeWeak};
use crate::items::{Flickable, ItemRc, ScrollBehavior, WindowItem};
use crate::lengths::{LogicalPoint, ScaleFactor};
use crate::properties::PropertyTracker;
use crate::window::{PopupWindowLocation, WindowAdapter, WindowInner};

/// What [`AccessKitTree`] needs from the backend that owns it.
///
/// The notifications are called from inside the property system, possibly while the tree is
/// borrowed, so they must not call back into the tree: schedule the work for later instead, for
/// example with a timer or `invoke_from_event_loop`.
pub trait AccessKitHost {
    /// Whether the window has the keyboard focus. While it hasn't, the root node is reported as
    /// the focused node.
    fn window_has_focus(&self) -> bool;
    /// An accessible property of some node changed: call [`AccessKitTree::rebuild_dirty_nodes`]
    /// later.
    fn nodes_dirty(&self);
    /// The focus delegate of the focused item changed: call [`AccessKitTree::focus_update`]
    /// later.
    fn focus_dirty(&self);
}

/// What a backend has sent to its platform adapter, and the rules for when a further update is
/// worth sending.
///
/// A backend keeps one next to its [`AccessKitTree`] and asks it before every update.
pub struct AccessKitUpdateState {
    initial_tree_sent: bool,
    pending_update: bool,
    enabled: bool,
}

impl Default for AccessKitUpdateState {
    fn default() -> Self {
        Self { initial_tree_sent: false, pending_update: false, enabled: true }
    }
}

impl AccessKitUpdateState {
    /// The platform asked for the tree and the whole tree was sent.
    pub fn set_initial_tree_sent(&mut self) {
        self.initial_tree_sent = true;
    }

    /// The platform does not use the tree anymore.
    pub fn set_deactivated(&mut self) {
        self.initial_tree_sent = false;
    }

    /// Whether updates are wanted. Backends whose platform can switch the accessibility services
    /// on and off without dropping the adapter turn this off; nothing is sent while it is.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// The platform has the tree and listens to updates.
    pub fn is_live(&self) -> bool {
        self.initial_tree_sent && self.enabled
    }

    /// Whether the whole tree should be sent again, because its structure changed. Returns `false`
    /// when that is not needed or already scheduled. When it returns `true`, the backend schedules
    /// the reload and calls [`Self::reload_started`] when it runs.
    pub fn should_schedule_reload(&mut self) -> bool {
        if self.pending_update || !self.initial_tree_sent {
            return false;
        }
        self.pending_update = true;
        true
    }

    /// The scheduled reload runs now. Returns whether the tree should be sent.
    pub fn reload_started(&mut self) -> bool {
        self.pending_update = false;
        self.enabled
    }

    /// Whether the nodes that were marked dirty should be sent. A pending reload sends everything
    /// anyway, and the focus node might not be known to the platform yet when the structure
    /// changed.
    pub fn should_send_dirty_nodes(&self, tree: &AccessKitTree) -> bool {
        self.is_live() && !self.pending_update && tree.has_dirty_nodes()
    }

    /// Whether a focus change should be sent. Sending one before the initial tree would send a
    /// `tree: None`, and for the reason of [`Self::should_send_dirty_nodes`] it waits for a pending
    /// reload.
    pub fn should_send_focus(&self) -> bool {
        self.is_live() && !self.pending_update
    }
}

const NODE_ID_INDEX_BITS: u32 = 16;
const NODE_ID_INDEX_MASK: u64 = (1 << NODE_ID_INDEX_BITS) - 1; // 0xFFFF
const NODE_ID_COMPONENT_BITS: u32 = 22;
const NODE_ID_COMPONENT_MASK: u64 = (1 << NODE_ID_COMPONENT_BITS) - 1; // 0x3FFFFF

// NodeIds for the TextRun children of a text input:
//
//   bits 38..=63 : sub-index, allocated per parent, never 0
//   bits 0..=37  : the parent NodeId, which `encode_item_node_id` fits in exactly these bits
//
// A regular NodeId leaves the sub-index bits clear, so a non-zero sub-index is what tells the two
// apart.
const NODE_ID_PARENT_BITS: u32 = NODE_ID_INDEX_BITS + NODE_ID_COMPONENT_BITS;
const NODE_ID_PARENT_MASK: u64 = (1u64 << NODE_ID_PARENT_BITS) - 1;
const NODE_ID_SUB_INDEX_BITS: u32 = u64::BITS - NODE_ID_PARENT_BITS;
const NODE_ID_SUB_INDEX_MASK: u64 = (1u64 << NODE_ID_SUB_INDEX_BITS) - 1;

fn encode_sub_node_id(parent: NodeId, sub_index: u32) -> NodeId {
    debug_assert!(sub_index >= 1, "sub_index 0 would collide with the parent NodeId");
    debug_assert!(
        (sub_index as u64) <= NODE_ID_SUB_INDEX_MASK,
        "sub_index exceeds {NODE_ID_SUB_INDEX_BITS} bits"
    );
    debug_assert_eq!(
        parent.0 & !NODE_ID_PARENT_MASK,
        0,
        "parent NodeId occupies more than {NODE_ID_PARENT_BITS} bits"
    );
    NodeId(((sub_index as u64) << NODE_ID_PARENT_BITS) | (parent.0 & NODE_ID_PARENT_MASK))
}

fn decode_sub_node_id(id: NodeId) -> Option<(NodeId, u32)> {
    let sub_index = (id.0 >> NODE_ID_PARENT_BITS) as u32;
    (sub_index != 0).then_some((NodeId(id.0 & NODE_ID_PARENT_MASK), sub_index))
}

fn is_text_input_role(role: Role) -> bool {
    matches!(
        role,
        Role::TextInput
            | Role::MultilineTextInput
            | Role::PasswordInput
            | Role::SearchInput
            | Role::NumberInput
    )
}

fn wraps_text_input(role: Role) -> bool {
    is_text_input_role(role) || role == Role::SpinButton
}

/// The AccessKit node tree of one window, kept in sync with its item tree.
///
/// This only builds the nodes: it neither owns the platform adapter nor schedules anything. See
/// the [module documentation](self).
pub struct AccessKitTree {
    window_adapter_weak: Weak<dyn WindowAdapter>,
    host: Rc<dyn AccessKitHost>,
    next_component_id: u32,
    free_component_ids: Vec<u32>,
    components_by_id: HashMap<u32, ItemTreeWeak>,
    component_ids: HashMap<NonNull<u8>, u32>,
    all_nodes: Vec<CachedNode>,
    root_node_id: NodeId,
    global_property_tracker: Pin<Rc<AccessibilityPropertyTracker>>,
    focused_node_tracker: Pin<Box<PropertyTracker<false, DelegateFocusPropertyTracker>>>,
    /// Emission state, keyed by the inner `TextInput`'s `ItemRc`. Its per-entry property tracker
    /// stays empty on purpose: the state has to outlive the edits it describes, so that NodeIds
    /// stay stable and screen readers see "node updated" rather than "subtree replaced".
    text_state: crate::item_rendering::ItemCache<
        crate::textlayout::sharedparley::CachedTextInputAccessibilityState,
    >,
    /// Added to the bounds of every node, in physical pixels.
    physical_offset: (f64, f64),
}

impl AccessKitTree {
    pub fn new(window_adapter_weak: Weak<dyn WindowAdapter>, host: Rc<dyn AccessKitHost>) -> Self {
        Self {
            global_property_tracker: Rc::pin(PropertyTracker::new_with_dirty_handler(
                AccessibilityPropertyDirtyHandler { host: host.clone() },
            )),
            focused_node_tracker: Box::pin(PropertyTracker::new_with_dirty_handler(
                DelegateFocusPropertyTracker { host: host.clone() },
            )),
            window_adapter_weak,
            host,
            next_component_id: 1,
            free_component_ids: Default::default(),
            root_node_id: NodeId(0),
            components_by_id: Default::default(),
            component_ids: Default::default(),
            all_nodes: Default::default(),
            text_state: Default::default(),
            physical_offset: (0., 0.),
        }
    }

    /// Moves every node by `x` and `y` physical pixels, for a platform whose AccessKit
    /// coordinates do not start at the window's origin. The offset only applies to nodes that are
    /// built afterwards, so call [`Self::build_new_tree`] to apply it to the whole tree.
    pub fn set_physical_offset(&mut self, x: f64, y: f64) {
        self.physical_offset = (x, y);
    }

    /// Marks all nodes as possibly changed, so that the next [`Self::rebuild_dirty_nodes`]
    /// re-evaluates them.
    pub fn set_dirty(&self) {
        self.global_property_tracker.set_dirty();
    }

    /// Forgets what was kept for the text of the accessibility client that went away.
    pub fn clear_text_state(&mut self) {
        self.text_state.clear_all();
    }

    /// Translates an AccessKit request into the action to run on the item.
    ///
    /// The action touches the item tree, so run it with [`DeferredAccessKitAction::invoke`] once
    /// the tree is no longer borrowed.
    pub fn handle_request(&self, request: ActionRequest) -> Option<DeferredAccessKitAction> {
        let a = match request.action {
            Action::Click => AccessibilityAction::Default,
            Action::Focus => {
                return self
                    .item_rc_for_node_id(request.target_node)
                    .map(DeferredAccessKitAction::SetFocus);
            }
            Action::Decrement => AccessibilityAction::Decrement,
            Action::Increment => AccessibilityAction::Increment,
            Action::ReplaceSelectedText => {
                let Some(accesskit::ActionData::Value(v)) = request.data else { return None };
                AccessibilityAction::ReplaceSelectedText(SharedString::from(&*v))
            }
            Action::SetValue => match request.data {
                Some(accesskit::ActionData::Value(v)) => {
                    AccessibilityAction::SetValue(SharedString::from(&*v))
                }
                Some(accesskit::ActionData::NumericValue(v)) => {
                    AccessibilityAction::SetValue(crate::format!("{v}"))
                }
                _ => return None,
            },
            Action::ScrollUp
            | Action::ScrollDown
            | Action::ScrollLeft
            | Action::ScrollRight
            | Action::SetScrollOffset => return self.scroll_request(&request),
            Action::Expand => AccessibilityAction::Expand,
            Action::SetTextSelection => {
                let Some(accesskit::ActionData::SetTextSelection(sel)) = request.data.as_ref()
                else {
                    return None;
                };
                // A TextPosition names a TextRun sub-NodeId, so decode it back to the wrapper.
                let (wrapper_parent, _) = decode_sub_node_id(sel.focus.node)?;
                if wrapper_parent != request.target_node {
                    return None;
                }
                let wrapper_item = self.item_rc_for_node_id(wrapper_parent)?;
                let window_adapter = self.window_adapter_weak.upgrade()?;
                let (inner_item_rc, text_input) = find_exposed_text_input(&wrapper_item)?;
                let state =
                    self.text_state.get_or_update_cache_entry_ref(&inner_item_rc, Default::default);
                let (anchor, focus) = state.decode_selection(
                    window_adapter.renderer(),
                    text_input.as_pin_ref(),
                    &inner_item_rc,
                    inner_item_rc.geometry().size,
                    &sel.anchor,
                    &sel.focus,
                )?;
                AccessibilityAction::SetSelectionOffsets(anchor as i32, focus as i32)
            }
            _ => return None,
        };
        self.item_rc_for_node_id(request.target_node)
            .map(|item| DeferredAccessKitAction::InvokeAccessibleAction(item, a))
    }

    /// Forgets a destroyed item tree. The caller must then rebuild the tree with
    /// [`Self::build_new_tree`].
    pub fn forget_item_tree(&mut self, component: ItemTreeRef) {
        self.forget_component(ItemTreeRef::as_ptr(component));
    }

    /// A tree update that only carries the focus.
    pub fn focus_update(&mut self) -> TreeUpdate {
        TreeUpdate {
            nodes: Vec::new(),
            tree: None,
            tree_id: TreeId::ROOT,
            focus: self.focus_node(),
        }
    }

    /// Whether [`Self::rebuild_dirty_nodes`] has something to send.
    ///
    /// This is `false` when a notification arrives after a tree structure change emptied the
    /// tree: the pending [`Self::build_new_tree`] sends everything anyway.
    pub fn has_dirty_nodes(&self) -> bool {
        (self.global_property_tracker.is_dirty() || self.focused_node_tracker.is_dirty())
            && !self.all_nodes.is_empty()
    }

    /// The update for the nodes whose properties changed since they were built.
    ///
    /// Only call this when the platform adapter is active and [`Self::has_dirty_nodes`] is
    /// `true`, because it consumes the dirty state.
    pub fn rebuild_dirty_nodes(&mut self) -> TreeUpdate {
        let Some(window_adapter) = self.window_adapter_weak.upgrade() else {
            return self.empty_update();
        };
        let window = window_adapter.window();

        let tracker = self.global_property_tracker.clone();
        tracker.as_ref().evaluate_as_dependency_root(|| {
            let nodes_vec: Vec<(NodeId, Node)> = self
                .all_nodes
                .iter()
                .flat_map(|cached_node| {
                    cached_node
                        .tracker
                        .as_ref()
                        .evaluate_if_dirty(|| {
                            let scale_factor = ScaleFactor::new(window.scale_factor());
                            let Some(item) = self.item_rc_for_node_id(cached_node.id) else {
                                return Vec::new();
                            };

                            let mut node = self.build_node_without_children(
                                &item,
                                scale_factor,
                                Default::default(),
                            );
                            node.set_children(cached_node.children.clone());

                            let mut emitted: Vec<(NodeId, Node)> = Vec::new();
                            self.try_emit_text_input_accessibility(
                                &item,
                                &mut node,
                                cached_node.id,
                                scale_factor,
                                Default::default(),
                                &mut emitted,
                                &window_adapter,
                            );

                            let mut out = Vec::with_capacity(1 + emitted.len());
                            out.push((cached_node.id, node));
                            out.extend(emitted);
                            out
                        })
                        .unwrap_or_default()
                })
                .collect();

            TreeUpdate {
                nodes: nodes_vec,
                tree: None,
                tree_id: TreeId::ROOT,
                focus: self.focus_node(),
            }
        })
    }

    fn empty_update(&self) -> TreeUpdate {
        TreeUpdate {
            nodes: Default::default(),
            tree: Default::default(),
            tree_id: TreeId::ROOT,
            focus: self.root_node_id,
        }
    }

    /// The node that has the accessibility focus.
    pub fn focus_node(&mut self) -> NodeId {
        self.window_adapter_weak
            .upgrade()
            .filter(|_| self.host.window_has_focus())
            .and_then(|window_adapter| {
                let window_inner = WindowInner::from_pub(window_adapter.window());
                window_inner
                    .focus_item
                    .borrow()
                    .upgrade()
                    .map(|focus_item| {
                        let parent = nearest_accessible_item(focus_item);
                        self.focused_node_tracker
                            .as_ref()
                            .evaluate(|| {
                                crate::accessibility::accessible_focus_delegate_position(&parent)
                            })
                            .and_then(|position| {
                                crate::accessibility::accessible_descendents(&parent).nth(position)
                            })
                            .unwrap_or(parent)
                    })
                    .or_else(|| window_inner.try_component().map(ItemRc::new_root))
                    .map(|focus_item| self.find_node_id_by_item_rc(focus_item))
            })
            .unwrap_or(self.root_node_id)
    }

    fn item_rc_for_node_id(&self, id: NodeId) -> Option<ItemRc> {
        let component_id: u32 = ((id.0 >> NODE_ID_INDEX_BITS) & NODE_ID_COMPONENT_MASK) as _;
        let index: u32 = (id.0 & NODE_ID_INDEX_MASK) as _;
        let component = self.components_by_id.get(&component_id)?.upgrade()?;
        Some(ItemRc::new(component, index))
    }

    fn find_node_id_by_item_rc(&mut self, mut item: ItemRc) -> NodeId {
        item = nearest_accessible_item(item);

        self.encode_item_node_id(&item)
    }

    fn alloc_component_id(&mut self) -> u32 {
        self.free_component_ids.pop().unwrap_or_else(|| {
            let id = self.next_component_id;
            self.next_component_id += 1;
            id
        })
    }

    fn forget_component(&mut self, component_ptr: NonNull<u8>) {
        if let Some(component_id) = self.component_ids.remove(&component_ptr) {
            self.components_by_id.remove(&component_id);
            self.free_component_ids.push(component_id);
        }
        self.text_state.component_destroyed_at(component_ptr);
    }

    /// Forgets the components that were destroyed without being unregistered: their
    /// `unregister_item_tree` is skipped when it finds the adapter borrowed, such as while
    /// building the tree destroys repeated components (#13670).
    fn forget_destroyed_components(&mut self) {
        let destroyed = self
            .component_ids
            .iter()
            .filter(|(_, id)| self.components_by_id.get(id).is_none_or(|c| c.upgrade().is_none()))
            .map(|(component_ptr, _)| *component_ptr)
            .collect::<Vec<_>>();
        for component_ptr in destroyed {
            self.forget_component(component_ptr);
        }
    }

    fn encode_item_node_id(&mut self, item: &ItemRc) -> NodeId {
        let component = item.item_tree();
        let component_ptr = ItemTreeRef::as_ptr(ItemTreeRc::borrow(component));
        let component_id = match self.component_ids.get(&component_ptr) {
            Some(&component_id) => component_id,
            None => {
                let component_id = self.alloc_component_id();
                self.component_ids.insert(component_ptr, component_id);
                self.components_by_id.insert(component_id, ItemTreeRc::downgrade(component));
                component_id
            }
        };

        debug_assert!(
            (component_id as u64) <= NODE_ID_COMPONENT_MASK,
            "component_id exceeds {NODE_ID_COMPONENT_BITS} bits"
        );
        let index = item.index();
        NodeId((component_id as u64) << NODE_ID_INDEX_BITS | (index as u64 & NODE_ID_INDEX_MASK))
    }

    fn build_node_for_item_recursively(
        &mut self,
        item: ItemRc,
        nodes: &mut Vec<(NodeId, Node)>,
        popups: &[AccessiblePopup],
        scale_factor: ScaleFactor,
        window_position: LogicalPoint,
        window_adapter: &Rc<dyn WindowAdapter>,
    ) -> NodeId {
        let id = self.encode_item_node_id(&item);

        let popup_children = popups
            .iter()
            .filter_map(|popup| {
                if popup.parent_node != id {
                    return None;
                }

                let popup_item = ItemRc::new_root(popup.component.clone());
                Some(self.build_node_for_item_recursively(
                    popup_item,
                    nodes,
                    popups,
                    scale_factor,
                    popup.location,
                    window_adapter,
                ))
            })
            .collect::<Vec<_>>();

        let descendant_children = crate::accessibility::accessible_descendents(&item)
            .map(|child| {
                self.build_node_for_item_recursively(
                    child,
                    nodes,
                    popups,
                    scale_factor,
                    window_position,
                    window_adapter,
                )
            })
            .chain(popup_children)
            .collect::<Vec<NodeId>>();

        let tracker = Box::pin(PropertyTracker::default());
        // One tracker for both the wrapper attributes and the text emission, so that either
        // going dirty rebuilds the node.
        let (node, text_run_nodes) = {
            let mut text_run_nodes: Vec<(NodeId, Node)> = Vec::new();
            let node = tracker.as_ref().evaluate(|| {
                let mut n = self.build_node_without_children(&item, scale_factor, window_position);
                n.set_children(descendant_children.clone());
                self.try_emit_text_input_accessibility(
                    &item,
                    &mut n,
                    id,
                    scale_factor,
                    window_position,
                    &mut text_run_nodes,
                    window_adapter,
                );
                n
            });
            (node, text_run_nodes)
        };

        // Only the regular descendants: every emit pushes the TextRun children again, and
        // `accesskit_consumer` rejects a child that appears twice.
        self.all_nodes.push(CachedNode { id, children: descendant_children, tracker });

        nodes.push((id, node));
        nodes.extend(text_run_nodes);

        id
    }

    /// Emits the TextRun children of a text input, and the value and selection on `wrapper_node`.
    fn try_emit_text_input_accessibility(
        &self,
        item: &ItemRc,
        wrapper_node: &mut Node,
        wrapper_id: NodeId,
        scale_factor: ScaleFactor,
        window_position: LogicalPoint,
        text_run_nodes: &mut Vec<(NodeId, Node)>,
        window_adapter: &Rc<dyn WindowAdapter>,
    ) {
        if !wraps_text_input(wrapper_node.role()) {
            return;
        }
        let Some((inner_item_rc, text_input)) = find_exposed_text_input(item) else {
            return;
        };
        let mut state =
            self.text_state.get_or_update_cache_entry_ref(&inner_item_rc, Default::default);

        // The inner `TextInput`'s geometry: a `LineEdit` insets it for its border, so measuring
        // from the wrapper's origin would place every TextRun off by the padding.
        let inner_geometry = inner_item_rc.geometry();
        let inner_absolute_origin =
            inner_item_rc.map_to_window(inner_geometry.origin) + window_position.to_vector();
        let physical_origin = (inner_absolute_origin * scale_factor).cast::<f64>();

        // Borrows the font context itself, so we must not be holding it here.
        let emitted_runs = state.emit(
            window_adapter.renderer(),
            text_input.as_pin_ref(),
            &inner_item_rc,
            inner_geometry.size,
            text_run_nodes,
            wrapper_node,
            wrapper_id,
            (
                physical_origin.x + self.physical_offset.0,
                physical_origin.y + self.physical_offset.1,
            ),
            encode_sub_node_id,
        );

        if emitted_runs
            && item
                .supported_accessibility_actions()
                .contains(SupportedAccessibilityAction::SetSelectionOffsets)
        {
            wrapper_node.add_action(Action::SetTextSelection);
        }
    }

    fn tree_info(&self, root: NodeId) -> TreeInfo {
        let mut tree = TreeInfo::new(root);
        tree.toolkit_name = Some("Slint".into());
        tree.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());
        tree
    }

    /// Builds all nodes from scratch.
    ///
    /// The first update of a platform adapter, and the update after the tree structure changed,
    /// come from here.
    pub fn build_new_tree(&mut self) -> TreeUpdate {
        let Some(window_adapter) = self.window_adapter_weak.upgrade() else {
            return self.empty_update();
        };
        let window = window_adapter.window();
        let window_inner = crate::window::WindowInner::from_pub(window);
        window_inner.ensure_tree_instantiated();

        let Some(component) = window_inner.try_component() else {
            return self.empty_update();
        };
        let root_item = ItemRc::new_root(component);

        let popups = window_inner
            .active_popups()
            .iter()
            .filter_map(|popup| {
                let PopupWindowLocation::ChildWindow(location) = popup.location else {
                    return None;
                };

                let parent_item = nearest_accessible_item(popup.parent_item.upgrade()?);
                let parent_node = self.encode_item_node_id(if parent_item.is_accessible() {
                    &parent_item
                } else {
                    &root_item
                });

                Some(AccessiblePopup { location, parent_node, component: popup.component.clone() })
            })
            .collect::<Vec<_>>();

        self.all_nodes.clear();
        let mut nodes = Vec::new();

        let property_tracker = self.global_property_tracker.clone();
        let root_id = property_tracker.as_ref().evaluate_as_dependency_root(|| {
            self.build_node_for_item_recursively(
                root_item,
                &mut nodes,
                &popups,
                ScaleFactor::new(window.scale_factor()),
                Default::default(),
                &window_adapter,
            )
        });
        self.root_node_id = root_id;
        self.forget_destroyed_components();

        TreeUpdate {
            nodes,
            tree: Some(self.tree_info(root_id)),
            tree_id: TreeId::ROOT,
            focus: self.focus_node(),
        }
    }

    /// Reports the scroll position and the scroll actions of the `Flickable` that `item` exposes.
    ///
    /// An action is only offered while the content can move in that direction, which is what
    /// assistive technologies use to tell whether there is more to scroll to.
    fn add_scroll_properties(&self, item: &ItemRc, node: &mut Node, scale_factor: ScaleFactor) {
        let Some((flickable_item, flickable)) = find_exposed_flickable(item) else { return };
        let metrics = flickable.as_pin_ref().scroll_metrics(&flickable_item);
        let offset = (metrics.offset * scale_factor).cast::<f64>();
        let max_offset = (metrics.max_offset * scale_factor).cast::<f64>();
        // Below a tenth of a pixel is a rounding error of the layout, not content
        const EPSILON: f64 = 0.1;

        if max_offset.x > EPSILON {
            node.set_scroll_x(offset.x);
            node.set_scroll_x_min(0.);
            node.set_scroll_x_max(max_offset.x);
            if offset.x > EPSILON {
                node.add_action(Action::ScrollLeft);
            }
            if offset.x < max_offset.x - EPSILON {
                node.add_action(Action::ScrollRight);
            }
        }
        if max_offset.y > EPSILON {
            node.set_scroll_y(offset.y);
            node.set_scroll_y_min(0.);
            node.set_scroll_y_max(max_offset.y);
            if offset.y > EPSILON {
                node.add_action(Action::ScrollUp);
            }
            if offset.y < max_offset.y - EPSILON {
                node.add_action(Action::ScrollDown);
            }
        }
        if max_offset.x > EPSILON || max_offset.y > EPSILON {
            node.add_action(Action::SetScrollOffset);
        }
    }

    /// The scroll that a `Scroll*` or `SetScrollOffset` request asks for.
    fn scroll_request(&self, request: &ActionRequest) -> Option<DeferredAccessKitAction> {
        let item = self.item_rc_for_node_id(request.target_node)?;
        let (flickable_item, flickable) = find_exposed_flickable(&item)?;
        let metrics = flickable.as_pin_ref().scroll_metrics(&flickable_item);

        // A page moves 90% of the visible size, so the next page repeats the last 10%.
        const PAGE: f32 = 0.9;
        // The items are not known here, so one item is approximated by 10% of the visible size.
        const ITEM: f32 = 0.1;
        let fraction = match request.data {
            Some(accesskit::ActionData::ScrollUnit(accesskit::ScrollUnit::Item)) => ITEM,
            _ => PAGE,
        };
        let amount = |visible: crate::Coord| visible as f32 * fraction;
        let (current_x, current_y) = (metrics.offset.x as f32, metrics.offset.y as f32);
        let (x, y) = match request.action {
            Action::ScrollLeft => (current_x - amount(metrics.visible_size.width), current_y),
            Action::ScrollRight => (current_x + amount(metrics.visible_size.width), current_y),
            Action::ScrollUp => (current_x, current_y - amount(metrics.visible_size.height)),
            Action::ScrollDown => (current_x, current_y + amount(metrics.visible_size.height)),
            Action::SetScrollOffset => {
                let Some(accesskit::ActionData::SetScrollOffset(point)) = request.data else {
                    return None;
                };
                let scale_factor = self.window_adapter_weak.upgrade()?.window().scale_factor();
                (point.x as f32 / scale_factor, point.y as f32 / scale_factor)
            }
            _ => return None,
        };
        Some(DeferredAccessKitAction::ScrollTo(flickable_item, LogicalPosition::new(x, y)))
    }

    fn build_node_without_children(
        &self,
        item: &ItemRc,
        scale_factor: ScaleFactor,
        window_position: LogicalPoint,
    ) -> Node {
        let is_checkable = item
            .accessible_string_property(AccessibleStringProperty::Checkable)
            .is_some_and(|x| x == "true");

        let (role, label) = if let Some(window_item) = item.downcast::<WindowItem>() {
            (Role::Window, Some(window_item.as_pin_ref().title().to_string()))
        } else {
            (
                match item.accessible_role() {
                    crate::items::AccessibleRole::None => Role::Unknown,
                    crate::items::AccessibleRole::Button => Role::Button,
                    crate::items::AccessibleRole::Checkbox => Role::CheckBox,
                    crate::items::AccessibleRole::Combobox => Role::ComboBox,
                    crate::items::AccessibleRole::Groupbox => Role::Group,
                    crate::items::AccessibleRole::List => Role::ListBox,
                    crate::items::AccessibleRole::Slider => Role::Slider,
                    crate::items::AccessibleRole::Spinbox => Role::SpinButton,
                    crate::items::AccessibleRole::Tab => Role::Tab,
                    crate::items::AccessibleRole::TabList => Role::TabList,
                    crate::items::AccessibleRole::TabPanel => Role::TabPanel,
                    crate::items::AccessibleRole::Text => Role::Label,
                    crate::items::AccessibleRole::Table => Role::Table,
                    crate::items::AccessibleRole::Tree => Role::Tree,
                    crate::items::AccessibleRole::TextInput => {
                        if let Some(text_input) = crate::accessibility::find_text_input(item) {
                            if !text_input.single_line.get_internal() {
                                Role::MultilineTextInput
                            } else {
                                match text_input.input_type.get_internal() {
                                    crate::items::InputType::Decimal
                                    | crate::items::InputType::Number => Role::NumberInput,
                                    crate::items::InputType::Password => Role::PasswordInput,
                                    crate::items::InputType::Search => Role::SearchInput,
                                    crate::items::InputType::Text => Role::TextInput,
                                }
                            }
                        } else {
                            Role::TextInput
                        }
                    }
                    crate::items::AccessibleRole::ProgressIndicator => Role::ProgressIndicator,
                    crate::items::AccessibleRole::Switch => Role::Switch,
                    crate::items::AccessibleRole::ListItem => Role::ListBoxOption,
                    crate::items::AccessibleRole::Image => Role::Image,
                    crate::items::AccessibleRole::RadioButton => Role::RadioButton,
                    crate::items::AccessibleRole::RadioGroup => Role::RadioGroup,
                    crate::items::AccessibleRole::WindowTitleBar => Role::TitleBar,
                    crate::items::AccessibleRole::Banner => Role::Banner,
                    crate::items::AccessibleRole::Complementary => Role::Complementary,
                    crate::items::AccessibleRole::ContentInfo => Role::ContentInfo,
                    crate::items::AccessibleRole::Form => Role::Form,
                    crate::items::AccessibleRole::Main => Role::Main,
                    crate::items::AccessibleRole::Navigation => Role::Navigation,
                    crate::items::AccessibleRole::Region => Role::Region,
                    crate::items::AccessibleRole::Search => Role::Search,
                },
                item.accessible_string_property(
                    crate::accessibility::AccessibleStringProperty::Label,
                )
                .map(|x| x.to_string()),
            )
        };

        let mut node = Node::new(role);

        if let Some(label) = label {
            if role == Role::Label {
                node.set_value(label);
            } else {
                node.set_label(label);
            }
        }

        if item
            .accessible_string_property(AccessibleStringProperty::Enabled)
            .is_some_and(|x| x != "true")
        {
            node.set_disabled();
        }

        if !item.is_visible() {
            node.set_hidden();
        }

        if item.borrow().as_ref().clips_children() {
            node.set_clips_children();
        }

        self.add_scroll_properties(item, &mut node, scale_factor);

        let geometry = item.geometry();
        let absolute_origin = item.map_to_window(geometry.origin) + window_position.to_vector();
        let physical_origin = (absolute_origin * scale_factor).cast::<f64>();
        let (x0, y0) = (
            physical_origin.x + self.physical_offset.0,
            physical_origin.y + self.physical_offset.1,
        );
        let physical_size = (geometry.size * scale_factor).cast::<f64>();
        node.set_bounds(accesskit::Rect {
            x0,
            y0,
            x1: x0 + physical_size.width,
            y1: y0 + physical_size.height,
        });

        let is_checked = is_checkable
            && item
                .accessible_string_property(AccessibleStringProperty::Checked)
                .is_some_and(|x| x == "true");
        if is_checkable {
            node.set_toggled(if is_checked { Toggled::True } else { Toggled::False });
        }

        if let Some(description) =
            item.accessible_string_property(AccessibleStringProperty::Description)
        {
            node.set_description(description.to_string());
        }

        if let Some(id) = item.accessible_string_property(AccessibleStringProperty::Id) {
            node.set_author_id(id.to_string());
        }

        if item
            .accessible_string_property(AccessibleStringProperty::Expandable)
            .is_some_and(|x| x == "true")
        {
            node.set_expanded(
                item.accessible_string_property(AccessibleStringProperty::Expanded)
                    .is_some_and(|x| x == "true"),
            );
        }

        if matches!(
            role,
            Role::Button
                | Role::CheckBox
                | Role::ComboBox
                | Role::ListBoxOption
                | Role::MultilineTextInput
                | Role::NumberInput
                | Role::PasswordInput
                | Role::SearchInput
                | Role::Slider
                | Role::SpinButton
                | Role::Tab
                | Role::TextInput
        ) {
            node.add_action(Action::Focus);
        }

        if let Some(min) = item
            .accessible_string_property(AccessibleStringProperty::ValueMinimum)
            .and_then(|min| min.parse().ok())
        {
            node.set_min_numeric_value(min);
        }
        if let Some(max) = item
            .accessible_string_property(AccessibleStringProperty::ValueMaximum)
            .and_then(|max| max.parse().ok())
        {
            node.set_max_numeric_value(max);
        }
        if let Some(step) = item
            .accessible_string_property(AccessibleStringProperty::ValueStep)
            .and_then(|step| step.parse().ok())
        {
            node.set_numeric_value_step(step);
        }

        if let Some(value) = item.accessible_string_property(AccessibleStringProperty::Value) {
            match value.parse() {
                Ok(numeric) if role == Role::SpinButton => {
                    node.set_numeric_value(numeric);
                    node.set_value(value.to_string());
                }
                Ok(numeric) if !is_text_input_role(role) => node.set_numeric_value(numeric),
                _ => node.set_value(value.to_string()),
            }
        }

        if let Some(placeholder) =
            item.accessible_string_property(AccessibleStringProperty::PlaceholderText)
        {
            node.set_placeholder(placeholder.to_string());
        }

        if item
            .accessible_string_property(AccessibleStringProperty::ReadOnly)
            .is_some_and(|x| x == "true")
        {
            node.set_read_only();
        }

        if let Some(orientation) = item
            .accessible_string_property(AccessibleStringProperty::Orientation)
            .and_then(|s| s.parse::<crate::items::Orientation>().ok())
        {
            node.set_orientation(match orientation {
                crate::items::Orientation::Horizontal => Orientation::Horizontal,
                crate::items::Orientation::Vertical => Orientation::Vertical,
            });
        }

        if let Some(live) = item
            .accessible_string_property(AccessibleStringProperty::LiveRegion)
            .and_then(|s| s.parse::<crate::items::AccessibleLiveness>().ok())
        {
            node.set_live(match live {
                crate::items::AccessibleLiveness::Off => Live::Off,
                crate::items::AccessibleLiveness::Polite => Live::Polite,
                crate::items::AccessibleLiveness::Assertive => Live::Assertive,
            });
        }

        if item
            .accessible_string_property(AccessibleStringProperty::ItemSelectable)
            .is_some_and(|x| x == "true")
        {
            node.set_selected(
                item.accessible_string_property(AccessibleStringProperty::ItemSelected)
                    .is_some_and(|x| x == "true"),
            );
        }

        if let Some(position_in_set) = item
            .accessible_string_property(AccessibleStringProperty::ItemIndex)
            .and_then(|s| s.parse::<usize>().ok())
        {
            node.set_position_in_set(position_in_set);
        }
        if let Some(size_of_set) = item
            .accessible_string_property(AccessibleStringProperty::ItemCount)
            .and_then(|s| s.parse::<usize>().ok())
        {
            node.set_size_of_set(size_of_set);
        }

        let supported = item.supported_accessibility_actions();
        if supported.contains(SupportedAccessibilityAction::Default) {
            node.add_action(accesskit::Action::Click);
        }
        if supported.contains(SupportedAccessibilityAction::Decrement) {
            node.add_action(accesskit::Action::Decrement);
        }
        if supported.contains(SupportedAccessibilityAction::Increment) {
            node.add_action(accesskit::Action::Increment);
        }
        if supported.contains(SupportedAccessibilityAction::SetValue) {
            node.add_action(accesskit::Action::SetValue);
        }
        if supported.contains(SupportedAccessibilityAction::ReplaceSelectedText) {
            node.add_action(accesskit::Action::ReplaceSelectedText);
        }
        if supported.contains(SupportedAccessibilityAction::Expand) {
            node.add_action(accesskit::Action::Expand);
        }

        node
    }
}

type AccessibilityPropertyTracker = PropertyTracker<true, AccessibilityPropertyDirtyHandler>;

struct AccessibilityPropertyDirtyHandler {
    host: Rc<dyn AccessKitHost>,
}

impl crate::properties::PropertyDirtyHandler for AccessibilityPropertyDirtyHandler {
    fn notify(self: Pin<&Self>) {
        self.host.nodes_dirty();
    }
}

struct DelegateFocusPropertyTracker {
    host: Rc<dyn AccessKitHost>,
}

impl crate::properties::PropertyDirtyHandler for DelegateFocusPropertyTracker {
    fn notify(self: Pin<&Self>) {
        self.host.focus_dirty();
    }
}

struct CachedNode {
    id: NodeId,
    children: Vec<NodeId>,
    tracker: Pin<Box<PropertyTracker>>,
}

pub enum DeferredAccessKitAction {
    SetFocus(ItemRc),
    InvokeAccessibleAction(ItemRc, AccessibilityAction),
    /// Scrolls the `Flickable` item so that the given position of the content, in logical
    /// pixels, is at the top left of the visible part.
    ScrollTo(ItemRc, LogicalPosition),
}

impl DeferredAccessKitAction {
    pub fn invoke(&self, window: &Window) {
        match self {
            DeferredAccessKitAction::SetFocus(item) => {
                // pretend this event was caused by a mouse for compatibility purposes
                WindowInner::from_pub(window).set_focus_item(item, true, FocusReason::PointerClick);
            }
            DeferredAccessKitAction::InvokeAccessibleAction(item, accessibility_action) => {
                item.accessible_action(accessibility_action);
            }
            DeferredAccessKitAction::ScrollTo(item, position) => {
                if let Some(flickable) = item.downcast::<Flickable>() {
                    flickable.as_pin_ref().scroll_to(item, *position, ScrollBehavior::Instant);
                }
            }
        }
    }
}

struct AccessiblePopup {
    location: LogicalPoint,
    parent_node: NodeId,
    component: ItemTreeRc,
}

#[cfg(test)]
mod update_state_tests {
    use super::AccessKitUpdateState;

    #[test]
    fn nothing_is_sent_before_the_initial_tree() {
        let mut state = AccessKitUpdateState::default();
        assert!(!state.should_send_focus());
        assert!(!state.should_schedule_reload());
        state.set_initial_tree_sent();
        assert!(state.should_send_focus());
        state.set_deactivated();
        assert!(!state.should_send_focus());
    }

    #[test]
    fn a_reload_is_scheduled_once_and_blocks_focus_updates() {
        let mut state = AccessKitUpdateState::default();
        state.set_initial_tree_sent();
        assert!(state.should_schedule_reload());
        assert!(!state.should_schedule_reload());
        assert!(!state.should_send_focus());
        assert!(state.reload_started());
        assert!(state.should_send_focus());
        assert!(state.should_schedule_reload());
    }

    #[test]
    fn nothing_is_sent_while_disabled() {
        let mut state = AccessKitUpdateState::default();
        state.set_initial_tree_sent();
        state.set_enabled(false);
        assert!(!state.is_live());
        assert!(!state.should_send_focus());
        assert!(state.should_schedule_reload());
        assert!(!state.reload_started());
        state.set_enabled(true);
        assert!(state.should_send_focus());
    }
}
