// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Weak;
use std::sync::{Arc, Condvar, Mutex};

use accesskit::{
    Action, ActionRequest, CheckedState, Node, NodeBuilder, NodeId, Role, Tree, TreeUpdate,
};
use i_slint_core::accessibility::AccessibleStringProperty;
use i_slint_core::items::{ItemRc, WindowItem};
use i_slint_core::window::WindowInner;
use i_slint_core::{
    component::{ComponentRc, ComponentRef, ComponentWeak},
    lengths::ScaleFactor,
};
use i_slint_core::{properties::PropertyTracker, window::WindowAdapter};

use super::WinitWindowAdapter;

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
/// Finally, when a component is destroyed, `unregister_component` must be called, which rebuilds the entire
/// tree at the moment.
///
/// If we wanted to move this to corelib, `on_event` gets replaced with listening to the events sent from the
/// platform adapter to the slint::Window. `handle_focus_change` is already internal to WindowInner, as well
/// as `component_destroyed`. The `WindowInner` would own this `AccessKit`.
pub struct AccessKitAdapter {
    inner: accesskit_winit::Adapter,
    window_adapter_weak: Weak<WinitWindowAdapter>,

    node_classes: RefCell<accesskit::NodeClassSet>,
    next_component_id: Cell<usize>,
    components_by_id: RefCell<HashMap<usize, ComponentWeak>>,
    component_ids: RefCell<HashMap<NonNull<u8>, usize>>,
    all_nodes: RefCell<Vec<CachedNode>>,
    global_property_tracker: Pin<Box<PropertyTracker<AccessibilitiesPropertyTracker>>>,
}

impl AccessKitAdapter {
    pub fn new(
        window_adapter_weak: Weak<WinitWindowAdapter>,
        winit_window: &winit::window::Window,
    ) -> Self {
        let wrapped_window_adapter_weak =
            send_wrapper::SendWrapper::new(window_adapter_weak.clone());
        Self {
            inner: accesskit_winit::Adapter::with_action_handler(
                &winit_window,
                move || Self::build_initial_tree(wrapped_window_adapter_weak.clone()),
                Box::new(ActionForwarder::new(&window_adapter_weak)),
            ),
            window_adapter_weak: window_adapter_weak.clone(),
            node_classes: RefCell::new(accesskit::NodeClassSet::new()),
            next_component_id: Cell::new(1),
            components_by_id: Default::default(),
            component_ids: Default::default(),
            all_nodes: Default::default(),
            global_property_tracker: Box::pin(PropertyTracker::new_with_dirty_handler(
                AccessibilitiesPropertyTracker { window_adapter_weak: window_adapter_weak.clone() },
            )),
        }
    }

    pub fn on_event(
        &self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent<'_>,
    ) -> bool {
        match event {
            winit::event::WindowEvent::Focused(_) => {
                self.global_property_tracker.set_dirty();
                let win = self.window_adapter_weak.clone();
                i_slint_core::timers::Timer::single_shot(Default::default(), move || {
                    if let Some(window_adapter) = win.upgrade() {
                        window_adapter.accesskit_adapter.rebuild_tree_of_dirty_nodes();
                    };
                });
                true // keep processing
            }
            _ => self.inner.on_event(window, event),
        }
    }

    pub fn handle_focus_item_change(&self) {
        self.inner.update_if_active(|| TreeUpdate {
            nodes: vec![],
            tree: None,
            focus: self.focus_node(),
        })
    }

    fn focus_node(&self) -> Option<NodeId> {
        let window_adapter = self.window_adapter_weak.upgrade()?;
        if !window_adapter.winit_window().has_focus() {
            return None;
        }
        let window_inner = WindowInner::from_pub(window_adapter.window());
        let focus_item = window_inner.focus_item.borrow().upgrade().or_else(|| {
            window_inner.try_component().map(|component_rc| ItemRc::new(component_rc, 0))
        })?;
        self.find_node_id_by_item_rc(focus_item)
    }

    fn handle_request(&self, request: ActionRequest) {
        let Some(window_adapter) = self.window_adapter_weak.upgrade() else { return };
        match request.action {
            Action::Focus => {
                if let Some(item) = self.item_rc_for_node_id(request.target) {
                    WindowInner::from_pub(window_adapter.window()).set_focus_item(&item);
                }
            }
            _ => {}
        }
    }

    pub fn register_component<'a>(&self) {
        let win = self.window_adapter_weak.clone();
        i_slint_core::timers::Timer::single_shot(Default::default(), move || {
            if let Some(window_adapter) = win.upgrade() {
                let self_ = &window_adapter.accesskit_adapter;
                self_.inner.update_if_active(|| self_.build_new_tree())
            };
        });
    }

    pub fn unregister_component<'a>(&self, component: ComponentRef) {
        let component_ptr = ComponentRef::as_ptr(component);
        if let Some(component_id) = self.component_ids.borrow_mut().remove(&component_ptr) {
            self.components_by_id.borrow_mut().remove(&component_id);
        }

        let win = self.window_adapter_weak.clone();
        i_slint_core::timers::Timer::single_shot(Default::default(), move || {
            if let Some(window_adapter) = win.upgrade() {
                let self_ = &window_adapter.accesskit_adapter;
                self_.inner.update_if_active(|| self_.build_new_tree())
            };
        });
    }

    fn item_rc_for_node_id(&self, id: NodeId) -> Option<ItemRc> {
        let component_id: usize = (id.0.get() >> usize::BITS) as _;
        let index: usize = (id.0.get() & usize::MAX as u128) as _;
        let component = self.components_by_id.borrow().get(&component_id)?.upgrade()?;
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
        let component = item.component();
        let component_ptr = ComponentRef::as_ptr(ComponentRc::borrow(&component));
        let component_id = *(self.component_ids.borrow().get(&component_ptr)?);
        let index = item.index();
        Some(NodeId(
            std::num::NonZeroU128::new(
                (component_id as u128) << usize::BITS | (index as u128 & usize::MAX as u128),
            )
            .unwrap(),
        ))
    }

    fn rebuild_tree_of_dirty_nodes(&self) {
        if !self.global_property_tracker.is_dirty() {
            return;
        }

        // It's possible that we may have been triggered by a timer, but in the meantime
        // the node tree has been emptied due to a tree structure change.
        if self.all_nodes.borrow().is_empty() {
            return;
        }

        let Some(window_adapter) = self.window_adapter_weak.upgrade() else { return };
        let window = window_adapter.window();

        self.inner.update_if_active(|| {
            self.global_property_tracker.as_ref().evaluate_as_dependency_root(|| {
                let all_nodes = self.all_nodes.borrow();
                let nodes = all_nodes.iter().filter_map(|cached_node| {
                    cached_node.tracker.as_ref().evaluate_if_dirty(|| {
                        let scale_factor = ScaleFactor::new(window.scale_factor());
                        let item = self.item_rc_for_node_id(cached_node.id)?;

                        let mut builder = self.build_node_without_children(&item, scale_factor);

                        builder.set_children(cached_node.children.clone());

                        let node = builder.build(&mut self.node_classes.borrow_mut());

                        Some((cached_node.id, node))
                    })?
                });

                let update =
                    TreeUpdate { nodes: nodes.collect(), tree: None, focus: self.focus_node() };

                update
            })
        })
    }

    fn build_node_for_item_recursively(
        &self,
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

        let component = item.component();
        let component_ptr = ComponentRef::as_ptr(ComponentRc::borrow(&component));
        if !self.component_ids.borrow().contains_key(&component_ptr) {
            let component_id = self.next_component_id.get();
            self.next_component_id.set(component_id + 1);
            self.component_ids.borrow_mut().insert(component_ptr, component_id);
            self.components_by_id
                .borrow_mut()
                .insert(component_id, ComponentRc::downgrade(&component));
        }

        let id = self.encode_item_node_id(&item).unwrap();
        self.all_nodes.borrow_mut().push(CachedNode { id, children, tracker });
        let node = builder.build(&mut self.node_classes.borrow_mut());

        nodes.push((id, node));

        id
    }

    fn build_new_tree(&self) -> TreeUpdate {
        let Some(window_adapter) = self.window_adapter_weak.upgrade() else { return Default::default(); };
        let window = window_adapter.window();
        let window_inner = i_slint_core::window::WindowInner::from_pub(window);

        let root_item = ItemRc::new(window_inner.component(), 0);

        self.all_nodes.borrow_mut().clear();
        let mut nodes = Vec::new();

        let root_id = self.global_property_tracker.as_ref().evaluate_as_dependency_root(|| {
            self.build_node_for_item_recursively(
                root_item,
                &mut nodes,
                ScaleFactor::new(window.scale_factor()),
            )
        });

        let update = TreeUpdate { nodes, tree: Some(Tree::new(root_id)), focus: self.focus_node() };
        update
    }

    fn build_initial_tree(
        wrapped_window_adapter_weak: send_wrapper::SendWrapper<Weak<WinitWindowAdapter>>,
    ) -> TreeUpdate {
        if wrapped_window_adapter_weak.valid() {
            return wrapped_window_adapter_weak
                .take()
                .upgrade()
                .map(|adapter| adapter.accesskit_adapter.build_new_tree())
                .unwrap_or_default();
        }

        let update_from_main_thread = Arc::new((Mutex::new(None), Condvar::new()));

        if let Err(_) = i_slint_core::api::invoke_from_event_loop({
            let update_from_main_thread = update_from_main_thread.clone();
            move || {
                let (lock, wait_condition) = &*update_from_main_thread;
                let mut update = lock.lock().unwrap();

                *update = Some(Self::build_initial_tree(wrapped_window_adapter_weak));

                wait_condition.notify_one();
            }
        }) {
            return Default::default();
        }

        let (lock, wait_condition) = &*update_from_main_thread;
        let mut update = lock.lock().unwrap();
        while update.is_none() {
            update = wait_condition.wait(update).unwrap();
        }

        return update.take().unwrap();
    }

    fn build_node_without_children(&self, item: &ItemRc, scale_factor: ScaleFactor) -> NodeBuilder {
        let (role, label) = if let Some(window_item) = item.downcast::<WindowItem>() {
            (Role::Window, window_item.as_pin_ref().title().to_string())
        } else {
            (
                match item.accessible_role() {
                    i_slint_core::items::AccessibleRole::None => Role::Unknown,
                    i_slint_core::items::AccessibleRole::Button => Role::Button,
                    i_slint_core::items::AccessibleRole::Checkbox => Role::CheckBox,
                    i_slint_core::items::AccessibleRole::Combobox => Role::ComboBoxGrouping,
                    i_slint_core::items::AccessibleRole::Slider => Role::Slider,
                    i_slint_core::items::AccessibleRole::Spinbox => Role::SpinButton,
                    i_slint_core::items::AccessibleRole::Tab => Role::Tab,
                    i_slint_core::items::AccessibleRole::Text => Role::StaticText,
                },
                item.accessible_string_property(
                    i_slint_core::accessibility::AccessibleStringProperty::Label,
                )
                .to_string(),
            )
        };

        let mut builder = NodeBuilder::new(role);

        builder.set_name(label);

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

        if item.accessible_string_property(AccessibleStringProperty::Checkable) == "true" {
            builder.set_checked_state(
                if item.accessible_string_property(AccessibleStringProperty::Checked) == "true" {
                    CheckedState::True
                } else {
                    CheckedState::False
                },
            );
        }

        builder.set_description(
            item.accessible_string_property(AccessibleStringProperty::Description).to_string(),
        );

        if matches!(
            role,
            Role::Button
                | Role::CheckBox
                | Role::ComboBoxGrouping
                | Role::Slider
                | Role::SpinButton
                | Role::Tab
        ) {
            builder.add_action(Action::Focus);
        }

        let min = item.accessible_string_property(AccessibleStringProperty::ValueMinimum);
        let max = item.accessible_string_property(AccessibleStringProperty::ValueMaximum);
        let step = item.accessible_string_property(AccessibleStringProperty::ValueStep);
        let value = item.accessible_string_property(AccessibleStringProperty::Value).to_string();

        match (min.parse(), max.parse(), value.parse(), step.parse()) {
            (Ok(min), Ok(max), Ok(value), Ok(step)) => {
                builder.set_min_numeric_value(min);
                builder.set_max_numeric_value(max);
                builder.set_numeric_value(value);
                builder.set_numeric_value_step(step);
            }
            _ => {
                builder.set_value(value);
            }
        }

        builder
    }
}

struct AccessibilitiesPropertyTracker {
    window_adapter_weak: Weak<WinitWindowAdapter>,
}

impl i_slint_core::properties::PropertyDirtyHandler for AccessibilitiesPropertyTracker {
    fn notify(&self) {
        let win = self.window_adapter_weak.clone();
        i_slint_core::timers::Timer::single_shot(Default::default(), move || {
            if let Some(window_adapter) = win.upgrade() {
                window_adapter.accesskit_adapter.rebuild_tree_of_dirty_nodes();
            };
        })
    }
}

struct CachedNode {
    id: NodeId,
    children: Vec<NodeId>,
    tracker: Pin<Box<PropertyTracker>>,
}

struct ActionForwarder {
    wrapped_window_adapter_weak: Arc<send_wrapper::SendWrapper<Weak<WinitWindowAdapter>>>,
}

impl ActionForwarder {
    pub fn new(window_adapter: &Weak<WinitWindowAdapter>) -> Self {
        Self {
            wrapped_window_adapter_weak: Arc::new(send_wrapper::SendWrapper::new(
                window_adapter.clone(),
            )),
        }
    }
}

impl accesskit::ActionHandler for ActionForwarder {
    fn do_action(&self, request: ActionRequest) {
        let wrapped_window_adapter_weak = self.wrapped_window_adapter_weak.clone();
        i_slint_core::api::invoke_from_event_loop(move || {
            let Some(window_adapter) = wrapped_window_adapter_weak.as_ref().clone().take().upgrade() else { return };
            window_adapter.accesskit_adapter.handle_request(request)
        })
        .ok();
    }
}
