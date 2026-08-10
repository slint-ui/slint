// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Weak;

use accesskit::{
    Action, ActionRequest, Live, Node, NodeId, Orientation, Role, Toggled, Tree, TreeId, TreeUpdate,
};
use i_slint_core::SharedString;
use i_slint_core::accessibility::{
    AccessibilityAction, AccessibleStringProperty, SupportedAccessibilityAction,
    find_text_input_with_rc,
};
use i_slint_core::api::Window;
use i_slint_core::input::FocusReason;
use i_slint_core::item_tree::{ItemTreeRc, ItemTreeRef, ItemTreeWeak, ParentItemTraversalMode};
use i_slint_core::items::{ItemRc, WindowItem};
use i_slint_core::lengths::{LogicalPoint, ScaleFactor};
use i_slint_core::window::{PopupWindowLocation, WindowInner};
use i_slint_core::{properties::PropertyTracker, window::WindowAdapter};

use super::WinitWindowAdapter;
use crate::SlintEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};

/// The AccessKit adapter tries to keep the given window adapter's item tree in sync with accesskit's node tree.
///
/// The entire item tree is mapped to accesskit's node tree. Any changes to an individual accessible item results
/// in an access kit tree update with just changed nodes. Any changes in the tree structure result in a complete
/// tree rebuild. This could be implemented more efficiently, but that isn't essential; AccessKit will avoid firing
/// gratuitous events for full-tree updates as long as the node IDs are stable.
///
/// For unix it's necessary to inform accesskit about any changes to the position or size of the window, hence
/// the `on_event` function that needs calling.
///
/// Similarly, when the window adapter is informed about a focus change, handle_focus_change must be called.
/// Finally, when a component is destroyed, `unregister_item_tree` must be called, which rebuilds the entire
/// tree at the moment.
///
/// If we wanted to move this to corelib, `on_event` gets replaced with listening to the events sent from the
/// platform adapter to the slint::Window. `handle_focus_change` is already internal to WindowInner, as well
/// as `component_destroyed`. The `WindowInner` would own this `AccessKit`.
pub struct AccessKitAdapter {
    inner: accesskit_winit::Adapter,
    window_adapter_weak: Weak<WinitWindowAdapter>,
    nodes: NodeCollection,
    global_property_tracker: Pin<Box<AccessibilityPropertyTracker>>,
    pending_update: bool,
    initial_tree_sent: bool,
}

impl AccessKitAdapter {
    pub fn new(
        window_adapter_weak: Weak<WinitWindowAdapter>,
        active_event_loop: &ActiveEventLoop,
        winit_window: &winit::window::Window,
        proxy: EventLoopProxy<SlintEvent>,
    ) -> Self {
        Self {
            inner: accesskit_winit::Adapter::with_event_loop_proxy(
                active_event_loop,
                winit_window,
                proxy,
            ),
            window_adapter_weak: window_adapter_weak.clone(),
            nodes: NodeCollection {
                next_component_id: 1,
                free_component_ids: Default::default(),
                root_node_id: NodeId(0),
                components_by_id: Default::default(),
                component_ids: Default::default(),
                all_nodes: Default::default(),
                focused_node_tracker: Box::pin(PropertyTracker::new_with_dirty_handler(
                    DelegateFocusPropertyTracker {
                        window_adapter_weak: window_adapter_weak.clone(),
                    },
                )),
                text_state: Default::default(),
            },
            global_property_tracker: Box::pin(PropertyTracker::new_with_dirty_handler(
                AccessibilityPropertyDirtyHandler {
                    window_adapter_weak: window_adapter_weak.clone(),
                },
            )),
            pending_update: false,
            initial_tree_sent: false,
        }
    }

    pub fn process_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) {
        if matches!(event, winit::event::WindowEvent::Focused(_)) {
            self.global_property_tracker.set_dirty();
            self.invoke_later(|self_cell, _| self_cell.borrow_mut().rebuild_tree_of_dirty_nodes());
        }
        self.inner.process_event(window, event);
    }

    pub fn process_accesskit_event(
        &mut self,
        window_event: accesskit_winit::WindowEvent,
    ) -> Option<DeferredAccessKitAction> {
        match window_event {
            accesskit_winit::WindowEvent::InitialTreeRequested => {
                self.inner.update_if_active(|| {
                    self.nodes.build_new_tree(
                        &self.window_adapter_weak,
                        self.global_property_tracker.as_ref(),
                    )
                });
                self.initial_tree_sent = true;
                None
            }
            accesskit_winit::WindowEvent::ActionRequested(r) => self.handle_request(r),
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {
                self.initial_tree_sent = false;
                self.nodes.text_state.clear_all();
                None
            }
        }
    }

    pub fn handle_focus_item_change(&mut self) {
        // Ignore focus changes until an initial tree was sent, to avoid sending `tree: None`.
        if !self.initial_tree_sent {
            return;
        }
        // Don't send a tree update now with an empty tree/node list when we know that the structure
        // if the tree has changed. It might be that the focus node is not known yet to AccessKit.
        // The pending update will take care of setting the focus node.
        if self.pending_update {
            return;
        }
        self.inner.update_if_active(|| TreeUpdate {
            nodes: Vec::new(),
            tree: None,
            tree_id: TreeId::ROOT,
            focus: self.nodes.focus_node(&self.window_adapter_weak),
        })
    }

    fn handle_request(&self, request: ActionRequest) -> Option<DeferredAccessKitAction> {
        let a = match request.action {
            Action::Click => AccessibilityAction::Default,
            Action::Focus => {
                return self
                    .nodes
                    .item_rc_for_node_id(request.target_node)
                    .map(DeferredAccessKitAction::SetFocus);
            }
            Action::Decrement => AccessibilityAction::Decrement,
            Action::Increment => AccessibilityAction::Increment,
            Action::ReplaceSelectedText => {
                let Some(accesskit::ActionData::Value(v)) = request.data else { return None };
                AccessibilityAction::ReplaceSelectedText(SharedString::from(&*v))
            }
            Action::SetValue => match request.data.unwrap() {
                accesskit::ActionData::Value(v) => {
                    AccessibilityAction::SetValue(SharedString::from(&*v))
                }
                accesskit::ActionData::NumericValue(v) => {
                    AccessibilityAction::SetValue(i_slint_core::format!("{v}"))
                }
                _ => return None,
            },
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
                let wrapper_item = self.nodes.item_rc_for_node_id(wrapper_parent)?;
                let window_adapter = self.window_adapter_weak.upgrade()?;
                let (inner_item_rc, text_input) = find_text_input_with_rc(&wrapper_item)?;
                let state = self
                    .nodes
                    .text_state
                    .get_or_update_cache_entry_ref(&inner_item_rc, Default::default);
                let (anchor, focus) = state.decode_selection(
                    window_adapter.renderer().as_core_renderer(),
                    text_input.as_pin_ref(),
                    &inner_item_rc,
                    inner_item_rc.geometry().size,
                    &sel.anchor,
                    &sel.focus,
                )?;
                AccessibilityAction::SetSelection(anchor as i32, focus as i32)
            }
            _ => return None,
        };
        self.nodes
            .item_rc_for_node_id(request.target_node)
            .map(|item| DeferredAccessKitAction::InvokeAccessibleAction(item, a))
    }

    pub fn reload_tree(&mut self) {
        if self.pending_update {
            return;
        }
        self.pending_update = true;

        self.invoke_later(|self_cell, win| {
            let mut self_ = self_cell.borrow_mut();
            let self_ = &mut *self_;
            self_.pending_update = false;
            self_.inner.update_if_active(|| {
                self_.nodes.build_new_tree(&win, self_.global_property_tracker.as_ref())
            })
        });
    }

    pub fn unregister_item_tree(&mut self, component: ItemTreeRef) {
        let component_ptr = ItemTreeRef::as_ptr(component);
        if let Some(component_id) = self.nodes.component_ids.remove(&component_ptr) {
            self.nodes.components_by_id.remove(&component_id);
            self.nodes.free_component_ids.push(component_id);
        }
        self.nodes.text_state.component_destroyed(component);
        self.reload_tree();
    }

    fn rebuild_tree_of_dirty_nodes(&mut self) {
        if !self.global_property_tracker.is_dirty() && !self.nodes.focused_node_tracker.is_dirty() {
            return;
        }

        // It's possible that we may have been triggered by a timer, but in the meantime
        // the node tree has been emptied due to a tree structure change.
        if self.nodes.all_nodes.is_empty() {
            return;
        }

        // Don't end a tree update now with an empty tree/node list when we know that the structure
        // if the tree has changed. It might be that the focus node is not known yet to AccessKit.
        // The pending update will take care rebuilding the entire tree anyway.
        if self.pending_update {
            return;
        }

        let Some(window_adapter) = self.window_adapter_weak.upgrade() else { return };
        let window = window_adapter.window();

        self.inner.update_if_active(|| {
            self.global_property_tracker.as_ref().evaluate_as_dependency_root(|| {
                let nodes_vec: Vec<(NodeId, Node)> = self
                    .nodes
                    .all_nodes
                    .iter()
                    .flat_map(|cached_node| {
                        cached_node
                            .tracker
                            .as_ref()
                            .evaluate_if_dirty(|| {
                                let scale_factor = ScaleFactor::new(window.scale_factor());
                                let Some(item) = self.nodes.item_rc_for_node_id(cached_node.id)
                                else {
                                    return Vec::new();
                                };

                                let mut node = self.nodes.build_node_without_children(
                                    &item,
                                    scale_factor,
                                    Default::default(),
                                );
                                node.set_children(cached_node.children.clone());

                                let mut emitted: Vec<(NodeId, Node)> = Vec::new();
                                self.nodes.try_emit_text_input_accessibility(
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
                    focus: self.nodes.focus_node(&self.window_adapter_weak),
                }
            })
        })
    }

    fn invoke_later(
        &self,
        callback: impl FnOnce(&std::cell::RefCell<Self>, Weak<WinitWindowAdapter>) + 'static,
    ) {
        let win = self.window_adapter_weak.clone();
        i_slint_core::timers::Timer::single_shot(Default::default(), move || {
            WinitWindowAdapter::with_access_kit_adapter_from_weak_window_adapter(
                win.clone(),
                move |self_cell| callback(self_cell, win),
            );
        });
    }
}

fn accessible_parent_for_item_rc(mut item: ItemRc) -> ItemRc {
    while !item.is_accessible() {
        if let Some(parent) = item.parent_item(ParentItemTraversalMode::StopAtPopups) {
            item = parent;
        } else {
            break;
        }
    }

    item
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

struct NodeCollection {
    next_component_id: u32,
    free_component_ids: Vec<u32>,
    components_by_id: HashMap<u32, ItemTreeWeak>,
    component_ids: HashMap<NonNull<u8>, u32>,
    all_nodes: Vec<CachedNode>,
    root_node_id: NodeId,
    focused_node_tracker: Pin<Box<PropertyTracker<false, DelegateFocusPropertyTracker>>>,
    /// Emission state, keyed by the inner `TextInput`'s `ItemRc`. Its per-entry property tracker
    /// stays empty on purpose: the state has to outlive the edits it describes, so that NodeIds
    /// stay stable and screen readers see "node updated" rather than "subtree replaced".
    text_state: i_slint_core::item_rendering::ItemCache<
        i_slint_core::textlayout::sharedparley::CachedTextInputAccessibilityState,
    >,
}

impl NodeCollection {
    fn focus_node(&mut self, window_adapter_weak: &Weak<WinitWindowAdapter>) -> NodeId {
        window_adapter_weak
            .upgrade()
            .filter(|window_adapter| {
                window_adapter.winit_window().is_some_and(|winit_window| winit_window.has_focus())
            })
            .and_then(|window_adapter| {
                let window_inner = WindowInner::from_pub(window_adapter.window());
                window_inner
                    .focus_item
                    .borrow()
                    .upgrade()
                    .map(|focus_item| {
                        let parent = accessible_parent_for_item_rc(focus_item);
                        self.focused_node_tracker
                            .as_ref()
                            .evaluate(|| {
                                parent.accessible_string_property(
                                    AccessibleStringProperty::DelegateFocus,
                                )
                            })
                            .and_then(|s| s.parse::<usize>().ok())
                            .and_then(|i| {
                                i_slint_core::accessibility::accessible_descendents(&parent).nth(i)
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
        item = accessible_parent_for_item_rc(item);

        self.encode_item_node_id(&item)
    }

    fn alloc_component_id(&mut self) -> u32 {
        self.free_component_ids.pop().unwrap_or_else(|| {
            let id = self.next_component_id;
            self.next_component_id += 1;
            id
        })
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
        window_adapter: &std::rc::Rc<WinitWindowAdapter>,
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

        let descendant_children = i_slint_core::accessibility::accessible_descendents(&item)
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
        window_adapter: &std::rc::Rc<WinitWindowAdapter>,
    ) {
        if !is_text_input_role(wrapper_node.role()) {
            return;
        }
        let Some((inner_item_rc, text_input)) = find_text_input_with_rc(item) else {
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

        let mut update =
            TreeUpdate { nodes: Vec::new(), tree: None, tree_id: TreeId::ROOT, focus: NodeId(0) };

        // Borrows the font context itself, so we must not be holding it here.
        let emitted_runs = state.emit(
            window_adapter.renderer().as_core_renderer(),
            text_input.as_pin_ref(),
            &inner_item_rc,
            inner_geometry.size,
            &mut update,
            wrapper_node,
            wrapper_id,
            (physical_origin.x, physical_origin.y),
            encode_sub_node_id,
        );

        if emitted_runs
            && item
                .supported_accessibility_actions()
                .contains(SupportedAccessibilityAction::SetSelection)
        {
            wrapper_node.add_action(Action::SetTextSelection);
        }

        text_run_nodes.extend(update.nodes);
    }

    fn tree_info(&self, root: NodeId) -> Tree {
        let mut tree = Tree::new(root);
        tree.toolkit_name = Some("Slint".into());
        tree.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());
        tree
    }

    fn build_new_tree(
        &mut self,
        window_adapter_weak: &Weak<WinitWindowAdapter>,
        property_tracker: Pin<&AccessibilityPropertyTracker>,
    ) -> TreeUpdate {
        let Some(window_adapter) = window_adapter_weak.upgrade() else {
            return TreeUpdate {
                nodes: Default::default(),
                tree: Default::default(),
                tree_id: TreeId::ROOT,
                focus: self.root_node_id,
            };
        };
        let window = window_adapter.window();
        let window_inner = i_slint_core::window::WindowInner::from_pub(window);
        window_inner.ensure_tree_instantiated();

        let root_item = ItemRc::new_root(window_inner.component());

        let popups = window_inner
            .active_popups()
            .iter()
            .filter_map(|popup| {
                let PopupWindowLocation::ChildWindow(location) = popup.location else {
                    return None;
                };

                let parent_item = accessible_parent_for_item_rc(popup.parent_item.upgrade()?);
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

        let root_id = property_tracker.evaluate_as_dependency_root(|| {
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

        TreeUpdate {
            nodes,
            tree: Some(self.tree_info(root_id)),
            tree_id: TreeId::ROOT,
            focus: self.focus_node(window_adapter_weak),
        }
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
                    i_slint_core::items::AccessibleRole::None => Role::Unknown,
                    i_slint_core::items::AccessibleRole::Button => Role::Button,
                    i_slint_core::items::AccessibleRole::Checkbox => Role::CheckBox,
                    i_slint_core::items::AccessibleRole::Combobox => Role::ComboBox,
                    i_slint_core::items::AccessibleRole::Groupbox => Role::Group,
                    i_slint_core::items::AccessibleRole::List => Role::ListBox,
                    i_slint_core::items::AccessibleRole::Slider => Role::Slider,
                    i_slint_core::items::AccessibleRole::Spinbox => Role::SpinButton,
                    i_slint_core::items::AccessibleRole::Tab => Role::Tab,
                    i_slint_core::items::AccessibleRole::TabList => Role::TabList,
                    i_slint_core::items::AccessibleRole::TabPanel => Role::TabPanel,
                    i_slint_core::items::AccessibleRole::Text => Role::Label,
                    i_slint_core::items::AccessibleRole::Table => Role::Table,
                    i_slint_core::items::AccessibleRole::Tree => Role::Tree,
                    i_slint_core::items::AccessibleRole::TextInput => {
                        if let Some(text_input) = i_slint_core::accessibility::find_text_input(item)
                        {
                            if !text_input.single_line.get_internal() {
                                Role::MultilineTextInput
                            } else {
                                match text_input.input_type.get_internal() {
                                    i_slint_core::items::InputType::Decimal
                                    | i_slint_core::items::InputType::Number => Role::NumberInput,
                                    i_slint_core::items::InputType::Password => Role::PasswordInput,
                                    i_slint_core::items::InputType::Search => Role::SearchInput,
                                    i_slint_core::items::InputType::Text | _ => Role::TextInput,
                                }
                            }
                        } else {
                            Role::TextInput
                        }
                    }
                    i_slint_core::items::AccessibleRole::ProgressIndicator => {
                        Role::ProgressIndicator
                    }
                    i_slint_core::items::AccessibleRole::Switch => Role::Switch,
                    i_slint_core::items::AccessibleRole::ListItem => Role::ListBoxOption,
                    i_slint_core::items::AccessibleRole::Image => Role::Image,
                    i_slint_core::items::AccessibleRole::RadioButton => Role::RadioButton,
                    i_slint_core::items::AccessibleRole::RadioGroup => Role::RadioGroup,
                    i_slint_core::items::AccessibleRole::Banner => Role::Banner,
                    i_slint_core::items::AccessibleRole::Complementary => Role::Complementary,
                    i_slint_core::items::AccessibleRole::ContentInfo => Role::ContentInfo,
                    i_slint_core::items::AccessibleRole::Form => Role::Form,
                    i_slint_core::items::AccessibleRole::Main => Role::Main,
                    i_slint_core::items::AccessibleRole::Navigation => Role::Navigation,
                    i_slint_core::items::AccessibleRole::Region => Role::Region,
                    i_slint_core::items::AccessibleRole::Search => Role::Search,
                    _ => Role::Unknown,
                },
                item.accessible_string_property(
                    i_slint_core::accessibility::AccessibleStringProperty::Label,
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

        let geometry = item.geometry();
        let absolute_origin = item.map_to_window(geometry.origin) + window_position.to_vector();
        let physical_origin = (absolute_origin * scale_factor).cast::<f64>();
        let physical_size = (geometry.size * scale_factor).cast::<f64>();
        node.set_bounds(accesskit::Rect {
            x0: physical_origin.x,
            y0: physical_origin.y,
            x1: physical_origin.x + physical_size.width,
            y1: physical_origin.y + physical_size.height,
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
            .and_then(|s| s.parse::<i_slint_core::items::Orientation>().ok())
        {
            node.set_orientation(match orientation {
                i_slint_core::items::Orientation::Horizontal => Orientation::Horizontal,
                i_slint_core::items::Orientation::Vertical => Orientation::Vertical,
            });
        }

        if let Some(live) = item
            .accessible_string_property(AccessibleStringProperty::LiveRegion)
            .and_then(|s| s.parse::<i_slint_core::items::AccessibleLiveness>().ok())
        {
            node.set_live(match live {
                i_slint_core::items::AccessibleLiveness::Off => Live::Off,
                i_slint_core::items::AccessibleLiveness::Polite => Live::Polite,
                i_slint_core::items::AccessibleLiveness::Assertive => Live::Assertive,
                _ => Live::Off,
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
    window_adapter_weak: Weak<WinitWindowAdapter>,
}

impl i_slint_core::properties::PropertyDirtyHandler for AccessibilityPropertyDirtyHandler {
    fn notify(self: Pin<&Self>) {
        let win = self.window_adapter_weak.clone();
        i_slint_core::timers::Timer::single_shot(Default::default(), move || {
            WinitWindowAdapter::with_access_kit_adapter_from_weak_window_adapter(
                win,
                |self_cell| {
                    self_cell.borrow_mut().rebuild_tree_of_dirty_nodes();
                },
            );
        })
    }
}

struct DelegateFocusPropertyTracker {
    window_adapter_weak: Weak<WinitWindowAdapter>,
}

impl i_slint_core::properties::PropertyDirtyHandler for DelegateFocusPropertyTracker {
    fn notify(self: Pin<&Self>) {
        let win = self.window_adapter_weak.clone();
        i_slint_core::timers::Timer::single_shot(Default::default(), move || {
            WinitWindowAdapter::with_access_kit_adapter_from_weak_window_adapter(
                win,
                |self_cell| {
                    self_cell.borrow_mut().handle_focus_item_change();
                },
            );
        })
    }
}

struct CachedNode {
    id: NodeId,
    children: Vec<NodeId>,
    tracker: Pin<Box<PropertyTracker>>,
}

impl From<accesskit_winit::Event> for SlintEvent {
    fn from(value: accesskit_winit::Event) -> Self {
        SlintEvent(crate::event_loop::CustomEvent::Accesskit(value))
    }
}

pub enum DeferredAccessKitAction {
    SetFocus(ItemRc),
    InvokeAccessibleAction(ItemRc, AccessibilityAction),
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
        }
    }
}

struct AccessiblePopup {
    location: LogicalPoint,
    parent_node: NodeId,
    component: ItemTreeRc,
}
