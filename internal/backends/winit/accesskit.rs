// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::HashMap;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Weak;

use accesskit::{
    Action, ActionRequest, DefaultActionVerb, Node, NodeBuilder, NodeId, Role, Toggled, Tree,
    TreeUpdate,
};
use i_slint_core::accessibility::{
    AccessibilityAction, AccessibleStringProperty, SupportedAccessibilityAction,
};
use i_slint_core::item_tree::{ItemTreeRc, ItemTreeRef, ItemTreeWeak};
use i_slint_core::items::{ItemRc, WindowItem};
use i_slint_core::lengths::ScaleFactor;
use i_slint_core::window::WindowInner;
use i_slint_core::SharedString;
use i_slint_core::{properties::PropertyTracker, window::WindowAdapter};

use super::WinitWindowAdapter;
use crate::SlintUserEvent;
use winit::event_loop::EventLoopProxy;

/// The AccessKit adapter tries to keep the given window adapter's item tree in sync with accesskit's node tree.
///
/// The entire item tree is mapped to accesskit's node tree. Any changes to an individual accessible item results
/// in an access kit tree update with just changed nodes. Any changes in the tree structure result in a complete
/// tree rebuild. This could be implemented more efficiently, but that isn't essential; AccessKit will avoid firing
/// gratuitious events for full-tree updates as long as the node IDs are stable.
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
    global_property_tracker: Pin<Box<PropertyTracker<AccessibilitiesPropertyTracker>>>,
    pending_update: bool,
}

impl AccessKitAdapter {
    pub fn new(
        window_adapter_weak: Weak<WinitWindowAdapter>,
        winit_window: &winit::window::Window,
        proxy: EventLoopProxy<SlintUserEvent>,
    ) -> Self {
        Self {
            inner: accesskit_winit::Adapter::with_event_loop_proxy(winit_window, proxy),
            window_adapter_weak: window_adapter_weak.clone(),
            nodes: NodeCollection {
                next_component_id: 1,
                root_node_id: NodeId(0),
                components_by_id: Default::default(),
                component_ids: Default::default(),
                all_nodes: Default::default(),
            },
            global_property_tracker: Box::pin(PropertyTracker::new_with_dirty_handler(
                AccessibilitiesPropertyTracker { window_adapter_weak: window_adapter_weak.clone() },
            )),
            pending_update: false,
        }
    }

    pub fn process_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) {
        if matches!(event, winit::event::WindowEvent::Focused(_)) {
            self.global_property_tracker.set_dirty();
            let win = self.window_adapter_weak.clone();
            i_slint_core::timers::Timer::single_shot(Default::default(), move || {
                if let Some(window_adapter) = win.upgrade() {
                    window_adapter.accesskit_adapter.borrow_mut().rebuild_tree_of_dirty_nodes();
                };
            });
        }
        self.inner.process_event(window, event);
    }

    pub fn process_accesskit_event(&mut self, window_event: accesskit_winit::WindowEvent) {
        match window_event {
            accesskit_winit::WindowEvent::InitialTreeRequested => {
                self.inner.update_if_active(|| {
                    self.nodes.build_new_tree(
                        &self.window_adapter_weak,
                        self.global_property_tracker.as_ref(),
                    )
                });
            }
            accesskit_winit::WindowEvent::ActionRequested(r) => self.handle_request(r),
            accesskit_winit::WindowEvent::AccessibilityDeactivated => (),
        }
    }

    pub fn handle_focus_item_change(&mut self) {
        self.inner.update_if_active(|| TreeUpdate {
            nodes: vec![],
            tree: None,
            focus: self.nodes.focus_node(&self.window_adapter_weak),
        })
    }

    fn handle_request(&self, request: ActionRequest) {
        let Some(window_adapter) = self.window_adapter_weak.upgrade() else { return };
        let a = match request.action {
            Action::Default => AccessibilityAction::Default,
            Action::Focus => {
                if let Some(item) = self.nodes.item_rc_for_node_id(request.target) {
                    WindowInner::from_pub(window_adapter.window()).set_focus_item(&item, true);
                }
                return;
            }
            Action::Decrement => AccessibilityAction::Decrement,
            Action::Increment => AccessibilityAction::Increment,
            Action::ReplaceSelectedText => {
                let Some(accesskit::ActionData::Value(v)) = request.data else { return };
                AccessibilityAction::ReplaceSelectedText(SharedString::from(&*v))
            }
            Action::SetValue => match request.data.unwrap() {
                accesskit::ActionData::Value(v) => {
                    AccessibilityAction::SetValue(SharedString::from(&*v))
                }
                accesskit::ActionData::NumericValue(v) => {
                    AccessibilityAction::SetValue(i_slint_core::format!("{v}"))
                }
                _ => return,
            },
            _ => return,
        };
        if let Some(item) = self.nodes.item_rc_for_node_id(request.target) {
            item.accessible_action(&a);
        }
    }

    pub fn reload_tree(&mut self) {
        if self.pending_update {
            return;
        }
        self.pending_update = true;
        let win = self.window_adapter_weak.clone();
        i_slint_core::timers::Timer::single_shot(Default::default(), move || {
            if let Some(window_adapter) = win.upgrade() {
                let mut self_ = window_adapter.accesskit_adapter.borrow_mut();
                let self_ = &mut *self_;
                self_.pending_update = false;
                self_.inner.update_if_active(|| {
                    self_.nodes.build_new_tree(&win, self_.global_property_tracker.as_ref())
                })
            };
        });
    }

    pub fn unregister_item_tree(&mut self, component: ItemTreeRef) {
        let component_ptr = ItemTreeRef::as_ptr(component);
        if let Some(component_id) = self.nodes.component_ids.remove(&component_ptr) {
            self.nodes.components_by_id.remove(&component_id);
        }
        self.reload_tree();
    }

    fn rebuild_tree_of_dirty_nodes(&mut self) {
        if !self.global_property_tracker.is_dirty() {
            return;
        }

        // It's possible that we may have been triggered by a timer, but in the meantime
        // the node tree has been emptied due to a tree structure change.
        if self.nodes.all_nodes.is_empty() {
            return;
        }

        let Some(window_adapter) = self.window_adapter_weak.upgrade() else { return };
        let window = window_adapter.window();

        self.inner.update_if_active(|| {
            self.global_property_tracker.as_ref().evaluate_as_dependency_root(|| {
                let nodes = self.nodes.all_nodes.iter().filter_map(|cached_node| {
                    cached_node.tracker.as_ref().evaluate_if_dirty(|| {
                        let scale_factor = ScaleFactor::new(window.scale_factor());
                        let item = self.nodes.item_rc_for_node_id(cached_node.id)?;

                        let mut builder =
                            self.nodes.build_node_without_children(&item, scale_factor);

                        builder.set_children(cached_node.children.clone());

                        let node = builder.build();

                        Some((cached_node.id, node))
                    })?
                });

                TreeUpdate {
                    nodes: nodes.collect(),
                    tree: None,
                    focus: self.nodes.focus_node(&self.window_adapter_weak),
                }
            })
        })
    }
}

struct NodeCollection {
    next_component_id: u32,
    components_by_id: HashMap<u32, ItemTreeWeak>,
    component_ids: HashMap<NonNull<u8>, u32>,
    all_nodes: Vec<CachedNode>,
    root_node_id: NodeId,
}

impl NodeCollection {
    fn focus_node(&self, window_adapter_weak: &Weak<WinitWindowAdapter>) -> NodeId {
        window_adapter_weak
            .upgrade()
            .filter(|window_adapter| {
                window_adapter.winit_window().map_or(false, |winit_window| winit_window.has_focus())
            })
            .and_then(|window_adapter| {
                let window_inner = WindowInner::from_pub(window_adapter.window());
                window_inner
                    .focus_item
                    .borrow()
                    .upgrade()
                    .or_else(|| {
                        window_inner
                            .try_component()
                            .map(|component_rc| ItemRc::new(component_rc, 0))
                    })
                    .and_then(|focus_item| self.find_node_id_by_item_rc(focus_item))
            })
            .unwrap_or_else(|| self.root_node_id)
    }

    fn item_rc_for_node_id(&self, id: NodeId) -> Option<ItemRc> {
        let component_id: u32 = (id.0 >> u32::BITS) as _;
        let index: u32 = (id.0 & u32::MAX as u64) as _;
        let component = self.components_by_id.get(&component_id)?.upgrade()?;
        Some(ItemRc::new(component, index))
    }

    fn find_node_id_by_item_rc(&self, mut item: ItemRc) -> Option<NodeId> {
        while !item.is_accessible() {
            if let Some(parent) = item.parent_item() {
                item = parent;
            } else {
                break;
            }
        }

        self.encode_item_node_id(&item)
    }

    fn encode_item_node_id(&self, item: &ItemRc) -> Option<NodeId> {
        let component = item.item_tree();
        let component_ptr = ItemTreeRef::as_ptr(ItemTreeRc::borrow(component));
        let component_id = *(self.component_ids.get(&component_ptr)?);
        let index = item.index();
        Some(NodeId((component_id as u64) << u32::BITS | (index as u64 & u32::MAX as u64)))
    }

    fn build_node_for_item_recursively(
        &mut self,
        item: ItemRc,
        nodes: &mut Vec<(NodeId, Node)>,
        scale_factor: ScaleFactor,
    ) -> NodeId {
        let tracker = Box::pin(PropertyTracker::default());

        let mut builder =
            tracker.as_ref().evaluate(|| self.build_node_without_children(&item, scale_factor));

        let children = i_slint_core::accessibility::accessible_descendents(&item)
            .map(|child| self.build_node_for_item_recursively(child, nodes, scale_factor))
            .collect::<Vec<NodeId>>();

        builder.set_children(children.clone());

        let component = item.item_tree();
        let component_ptr = ItemTreeRef::as_ptr(ItemTreeRc::borrow(component));
        if !self.component_ids.contains_key(&component_ptr) {
            let component_id = self.next_component_id;
            self.next_component_id += 1;
            self.component_ids.insert(component_ptr, component_id);
            self.components_by_id.insert(component_id, ItemTreeRc::downgrade(component));
        }

        let id = self.encode_item_node_id(&item).unwrap();
        self.all_nodes.push(CachedNode { id, children, tracker });
        let node = builder.build();

        nodes.push((id, node));

        id
    }

    fn build_new_tree(
        &mut self,
        window_adapter_weak: &Weak<WinitWindowAdapter>,
        property_tracker: Pin<&PropertyTracker<AccessibilitiesPropertyTracker>>,
    ) -> TreeUpdate {
        let Some(window_adapter) = window_adapter_weak.upgrade() else {
            return TreeUpdate {
                nodes: Default::default(),
                tree: Default::default(),
                focus: self.root_node_id,
            };
        };
        let window = window_adapter.window();
        let window_inner = i_slint_core::window::WindowInner::from_pub(window);

        let root_item = ItemRc::new(window_inner.component(), 0);

        self.all_nodes.clear();
        let mut nodes = Vec::new();

        let root_id = property_tracker.evaluate_as_dependency_root(|| {
            self.build_node_for_item_recursively(
                root_item,
                &mut nodes,
                ScaleFactor::new(window.scale_factor()),
            )
        });
        self.root_node_id = root_id;

        TreeUpdate {
            nodes,
            tree: Some(Tree::new(root_id)),
            focus: self.focus_node(window_adapter_weak),
        }
    }

    fn build_node_without_children(&self, item: &ItemRc, scale_factor: ScaleFactor) -> NodeBuilder {
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
                    i_slint_core::items::AccessibleRole::List => Role::List,
                    i_slint_core::items::AccessibleRole::Slider => Role::Slider,
                    i_slint_core::items::AccessibleRole::Spinbox => Role::SpinButton,
                    i_slint_core::items::AccessibleRole::Tab => Role::Tab,
                    i_slint_core::items::AccessibleRole::TabList => Role::TabList,
                    i_slint_core::items::AccessibleRole::Text => Role::Label,
                    i_slint_core::items::AccessibleRole::Table => Role::Table,
                    i_slint_core::items::AccessibleRole::Tree => Role::Tree,
                    i_slint_core::items::AccessibleRole::TextInput => Role::TextInput,
                    i_slint_core::items::AccessibleRole::ProgressIndicator => {
                        Role::ProgressIndicator
                    }
                    i_slint_core::items::AccessibleRole::Switch => Role::Switch,
                    _ => Role::Unknown,
                },
                item.accessible_string_property(
                    i_slint_core::accessibility::AccessibleStringProperty::Label,
                )
                .map(|x| x.to_string()),
            )
        };

        let mut builder = NodeBuilder::new(role);

        if let Some(label) = label {
            builder.set_name(label);
        }

        let geometry = item.geometry();
        let absolute_origin = item.map_to_window(geometry.origin);
        let physical_origin = (absolute_origin * scale_factor).cast::<f64>();
        let physical_size = (geometry.size * scale_factor).cast::<f64>();
        builder.set_bounds(accesskit::Rect {
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
            builder.set_toggled(if is_checked { Toggled::True } else { Toggled::False });
        }

        if let Some(description) =
            item.accessible_string_property(AccessibleStringProperty::Description)
        {
            builder.set_description(description.to_string());
        }

        if matches!(
            role,
            Role::Button
                | Role::CheckBox
                | Role::ComboBox
                | Role::Slider
                | Role::SpinButton
                | Role::Tab
        ) {
            builder.add_action(Action::Focus);
        }

        if let Some(min) = item
            .accessible_string_property(AccessibleStringProperty::ValueMinimum)
            .and_then(|min| min.parse().ok())
        {
            builder.set_min_numeric_value(min);
        }
        if let Some(max) = item
            .accessible_string_property(AccessibleStringProperty::ValueMaximum)
            .and_then(|max| max.parse().ok())
        {
            builder.set_max_numeric_value(max);
        }
        if let Some(step) = item
            .accessible_string_property(AccessibleStringProperty::ValueStep)
            .and_then(|step| step.parse().ok())
        {
            builder.set_numeric_value_step(step);
        }

        if let Some(value) = item.accessible_string_property(AccessibleStringProperty::Value) {
            if let Ok(value) = value.parse() {
                builder.set_numeric_value(value);
            } else {
                builder.set_value(value.to_string());
            }
        }

        if let Some(placeholder) = item
            .accessible_string_property(AccessibleStringProperty::PlaceholderText)
            .filter(|x| !x.is_empty())
        {
            builder.set_placeholder(placeholder.to_string());
        }

        let supported = item.supported_accessibility_actions();
        if supported.contains(SupportedAccessibilityAction::Default) {
            builder.add_action(accesskit::Action::Default);
            builder.set_default_action_verb(if is_checked {
                DefaultActionVerb::Uncheck
            } else if is_checkable {
                DefaultActionVerb::Check
            } else {
                DefaultActionVerb::Click
            });
        }
        if supported.contains(SupportedAccessibilityAction::Decrement) {
            builder.add_action(accesskit::Action::Decrement);
            if builder.default_action_verb().is_none() {
                builder.set_default_action_verb(DefaultActionVerb::Click);
            }
        }
        if supported.contains(SupportedAccessibilityAction::Increment) {
            builder.add_action(accesskit::Action::Increment);
            if builder.default_action_verb().is_none() {
                builder.set_default_action_verb(DefaultActionVerb::Click);
            }
        }
        if supported.contains(SupportedAccessibilityAction::SetValue) {
            builder.add_action(accesskit::Action::SetValue);
            if builder.default_action_verb().is_none() {
                builder.set_default_action_verb(DefaultActionVerb::Focus);
            }
        }
        if supported.contains(SupportedAccessibilityAction::ReplaceSelectedText) {
            builder.add_action(accesskit::Action::ReplaceSelectedText);
            if builder.default_action_verb().is_none() {
                builder.set_default_action_verb(DefaultActionVerb::Focus);
            }
        }

        builder
    }
}

struct AccessibilitiesPropertyTracker {
    window_adapter_weak: Weak<WinitWindowAdapter>,
}

impl i_slint_core::properties::PropertyDirtyHandler for AccessibilitiesPropertyTracker {
    fn notify(self: Pin<&Self>) {
        let win = self.window_adapter_weak.clone();
        i_slint_core::timers::Timer::single_shot(Default::default(), move || {
            if let Some(window_adapter) = win.upgrade() {
                window_adapter.accesskit_adapter.borrow_mut().rebuild_tree_of_dirty_nodes();
            };
        })
    }
}

struct CachedNode {
    id: NodeId,
    children: Vec<NodeId>,
    tracker: Pin<Box<PropertyTracker>>,
}

impl From<accesskit_winit::Event> for SlintUserEvent {
    fn from(value: accesskit_winit::Event) -> Self {
        SlintUserEvent(crate::event_loop::CustomEvent::Accesskit(value))
    }
}
