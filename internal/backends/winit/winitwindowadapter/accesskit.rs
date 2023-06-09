// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::cell::{Cell, RefCell};
use std::pin::Pin;
use std::rc::Weak;
use std::sync::{Arc, Condvar, Mutex};

use accesskit::{
    Action, ActionRequest, CheckedState, Node, NodeBuilder, NodeId, Role, Tree, TreeUpdate,
};
use i_slint_core::accessibility::AccessibleStringProperty;
use i_slint_core::items::{ItemRc, WindowItem};
use i_slint_core::{component::ComponentRef, lengths::ScaleFactor};
use i_slint_core::{component::ComponentVTable, window::WindowInner};
use i_slint_core::{properties::PropertyTracker, window::WindowAdapter};

use super::WinitWindowAdapter;

/// The AccessKit adapter tries to keep the given window adapter's item tree in sync with accesskit's node tree.
///
/// The entire item tree is mapped to accesskit's node tree. Any changes to an individual accessible item results
/// in an access kit tree update with just changed nodes. Any changes in the tree structure result in a complete
/// tree rebuild. This could be implemented more efficiently.
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
    tree_generation: Cell<u16>,
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
            tree_generation: Cell::new(0),
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
        self.inner.on_event(window, event)
    }

    pub fn handle_focus_change(&self, new: Option<ItemRc>) {
        self.inner.update_if_active(|| TreeUpdate {
            nodes: vec![],
            tree: None,
            focus: new.map(|item| node_id_for_item_rc(item, self.tree_generation.get())),
        })
    }

    fn handle_request(&self, request: ActionRequest) {
        let Some(window_adapter) = self.window_adapter_weak.upgrade() else { return };
        match request.action {
            Action::Focus => {
                if let Some(item) = item_rc_for_node_id(request.target, self.tree_generation.get())
                {
                    WindowInner::from_pub(window_adapter.window()).set_focus_item(&item);
                }
            }
            _ => {}
        }
    }

    pub fn unregister_component<'a>(&self, _component: ComponentRef) {
        self.all_nodes.borrow_mut().clear();

        self.tree_generation.set(self.tree_generation.get() + 1);

        let win = self.window_adapter_weak.clone();
        i_slint_core::timers::Timer::single_shot(Default::default(), move || {
            if let Some(window_adapter) = win.upgrade() {
                let self_ = &window_adapter.accesskit_adapter;
                self_.inner.update_if_active(|| self_.build_new_tree())
            };
        });
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
                let scale_factor = ScaleFactor::new(window.scale_factor());

                let all_nodes = self.all_nodes.borrow();

                let nodes = all_nodes.iter().filter_map(|cached_node| {
                    if !cached_node.tracker.is_dirty() {
                        return None;
                    }
                    cached_node
                        .tracker
                        .as_ref()
                        .evaluate_if_dirty(|| {
                            let item =
                                item_rc_for_node_id(cached_node.id, self.tree_generation.get())?;

                            let mut builder = self.build_node_without_children(&item, scale_factor);

                            builder.set_children(cached_node.children.clone());

                            let node = builder.build(&mut self.node_classes.borrow_mut());

                            Some((cached_node.id, node))
                        })
                        .unwrap() // Must be Some() because `is_dirty` returned true.
                });

                let update = TreeUpdate {
                    nodes: nodes.collect(),
                    tree: None,
                    focus: WindowInner::from_pub(window)
                        .focus_item
                        .borrow()
                        .upgrade()
                        .map(|item| node_id_for_item_rc(item, self.tree_generation.get())),
                };

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

        let (id, children) = tracker.as_ref().evaluate(|| {
            let mut builder = self.build_node_without_children(&item, scale_factor);

            let children = i_slint_core::accessibility::accessible_descendents(&item)
                .map(|child| self.build_node_for_item_recursively(child, nodes, scale_factor))
                .collect::<Vec<NodeId>>();

            builder.set_children(children.clone());

            let id = node_id_for_item_rc(item, self.tree_generation.get());

            let node = builder.build(&mut self.node_classes.borrow_mut());

            nodes.push((id, node));

            (id, children)
        });

        self.all_nodes.borrow_mut().push(CachedNode { id, children, tracker });
        id
    }

    fn build_new_tree(&self) -> TreeUpdate {
        let Some(window_adapter) = self.window_adapter_weak.upgrade() else { return Default::default(); };
        let window = window_adapter.window();

        self.global_property_tracker.as_ref().evaluate_as_dependency_root(|| {
            let window_inner = i_slint_core::window::WindowInner::from_pub(window);

            let tree_generation = self.tree_generation.get() + 1;
            self.tree_generation.set(tree_generation);

            let root_item = ItemRc::new(window_inner.component(), 0);

            let mut nodes = Vec::new();
            let root_id = self.build_node_for_item_recursively(
                root_item,
                &mut nodes,
                ScaleFactor::new(window.scale_factor()),
            );

            let update = TreeUpdate {
                nodes,
                tree: Some(Tree::new(root_id)),
                focus: window_inner
                    .focus_item
                    .borrow()
                    .upgrade()
                    .map(|item| node_id_for_item_rc(item, tree_generation)),
            };

            update
        })
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

// Encode a u128 accesskit node id for an ItemRc by assuming that the ItemRc remains
// alive and storing the item index, raw VRc inner pointer, and a tree generation index
// in the u128.
fn node_id_for_item_rc(mut item: ItemRc, tree_generation: u16) -> NodeId {
    while !item.is_accessible() {
        if let Some(parent) = item.parent_item() {
            item = parent;
        } else {
            break;
        }
    }

    let component_ptr = item.component().as_raw_ptr();
    let component_usize: u128 = component_ptr as usize as u128;
    let item_index: u128 = item.index() as u32 as u128;

    let encoded_id = std::num::NonZeroU128::new(
        (component_usize << 64) | ((tree_generation as u128) << 48) | item_index,
    )
    .unwrap();
    NodeId(encoded_id)
}

fn item_rc_for_node_id(id: NodeId, expected_generation: u16) -> Option<ItemRc> {
    let encoded_component = id.0.get() >> 64;
    let encoded_generation = ((id.0.get() >> 48) & 0xffff) as u16;
    let encoded_item_index = (id.0.get() & 0xffffffff) as u32;

    if encoded_generation != expected_generation {
        return None;
    }

    // Safety: When the tree structure changes, the tree generation value is increased, causing a mismatch
    // caught earlier. Therefore, at this point the pointer should be safe to convert back into a VRc.
    let component_rc = unsafe {
        vtable::VRc::<ComponentVTable, vtable::Dyn>::from_raw_ptr(encoded_component as *const _)
    };

    Some(ItemRc::new(component_rc, encoded_item_index as usize))
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
    wrapped_window_adapter_weak: send_wrapper::SendWrapper<Weak<WinitWindowAdapter>>,
}

impl ActionForwarder {
    pub fn new(window_adapter: &Weak<WinitWindowAdapter>) -> Self {
        Self { wrapped_window_adapter_weak: send_wrapper::SendWrapper::new(window_adapter.clone()) }
    }
}

impl accesskit::ActionHandler for ActionForwarder {
    fn do_action(&self, request: ActionRequest) {
        let wrapped_window_adapter_weak = self.wrapped_window_adapter_weak.clone();
        i_slint_core::api::invoke_from_event_loop(move || {
            let Some(window_adapter) = wrapped_window_adapter_weak.take().upgrade() else { return };
            window_adapter.accesskit_adapter.handle_request(request)
        })
        .ok();
    }
}
