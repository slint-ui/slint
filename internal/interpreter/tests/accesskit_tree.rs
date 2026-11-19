// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Pins the AccessKit node tree that `i_slint_core::accessibility::accesskit_tree` builds for an
//! item tree: roles, properties, actions, NodeId stability, focus and action requests. The
//! platform adapters on desktop and Android share this builder.

use accesskit::{
    Action, ActionData, ActionRequest, Node, NodeId, Role, Toggled, TreeId, TreeUpdate,
};
use i_slint_core::accessibility::accesskit_tree::{
    AccessKitHost, AccessKitTree, DeferredAccessKitAction,
};
use i_slint_core::api::ComponentHandle;
use i_slint_core::window::{WindowAdapter, WindowInner};
use slint_interpreter::{Compiler, ComponentInstance, Value};
use std::cell::Cell;
use std::rc::Rc;

const SOURCE: &str = r#"
export component TestCase inherits Window {
    width: 400px;
    height: 300px;

    in-out property <string> label: "Play";
    in-out property <bool> checked: false;
    in-out property <float> volume: 5;
    in-out property <string> last-action;
    callback activated();

    VerticalLayout {
        play := Rectangle {
            height: 40px;
            accessible-role: button;
            accessible-label: root.label;
            accessible-description: "Starts playback";
            accessible-id: "play-button";
            accessible-action-default => {
                root.last-action = "default";
                root.activated();
            }
        }

        mute := Rectangle {
            height: 40px;
            accessible-role: checkbox;
            accessible-label: "Mute";
            accessible-checkable: true;
            accessible-checked: root.checked;
            accessible-action-default => { root.checked = !root.checked; }
        }

        volume-slider := Rectangle {
            height: 40px;
            accessible-role: slider;
            accessible-label: "Volume";
            accessible-value: root.volume;
            accessible-value-minimum: 0;
            accessible-value-maximum: 10;
            accessible-value-step: 2;
            accessible-action-increment => { root.volume += 2; }
            accessible-action-decrement => { root.volume -= 2; }
            accessible-action-set-value(v) => { root.volume = v.to-float(); }
        }

        disabled-button := Rectangle {
            height: 40px;
            accessible-role: button;
            accessible-label: "Locked";
            accessible-enabled: false;
        }

        field := TextInput {
            height: 40px;
            accessible-label: "Name";
            single-line: true;
            text: "Hello";
        }
    }
}
"#;

#[derive(Default)]
struct TestHost {
    focused: Cell<bool>,
    nodes_dirty: Cell<u32>,
    focus_dirty: Cell<u32>,
}

impl AccessKitHost for TestHost {
    fn window_has_focus(&self) -> bool {
        self.focused.get()
    }
    fn nodes_dirty(&self) {
        self.nodes_dirty.set(self.nodes_dirty.get() + 1);
    }
    fn focus_dirty(&self) {
        self.focus_dirty.set(self.focus_dirty.get() + 1);
    }
}

struct Fixture {
    instance: ComponentInstance,
    /// Keeps the window adapter alive, the tree only holds a weak reference.
    _adapter: Rc<dyn WindowAdapter>,
    host: Rc<TestHost>,
    tree: AccessKitTree,
}

impl Fixture {
    fn new() -> Self {
        Self::with_source(SOURCE)
    }

    fn with_source(source: &str) -> Self {
        i_slint_backend_testing::init_no_event_loop();
        let result = i_slint_compiler::poll_ready(
            Compiler::default().build_from_source(source.into(), Default::default()),
        );
        assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
        let instance = result.component("TestCase").unwrap().create().unwrap();
        instance.show().unwrap();
        let adapter = WindowInner::from_pub(instance.window()).window_adapter();
        let host = Rc::new(TestHost::default());
        let tree = AccessKitTree::new(Rc::downgrade(&adapter), host.clone());
        Self { instance, _adapter: adapter, host, tree }
    }
}

fn node_by_label<'a>(update: &'a TreeUpdate, label: &str) -> (NodeId, &'a Node) {
    update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(label))
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| panic!("no node labelled {label:?} in {:#?}", update.nodes))
}

fn request(action: Action, target_node: NodeId, data: Option<ActionData>) -> ActionRequest {
    ActionRequest { action, target_tree: TreeId::ROOT, target_node, data }
}

#[test]
fn new_tree_describes_the_item_tree() {
    let mut f = Fixture::new();
    let update = f.tree.build_new_tree();

    let tree_info = update.tree.as_ref().expect("a full tree carries the tree info");
    assert_eq!(tree_info.toolkit_name.as_deref(), Some("Slint"));
    let root = update.nodes.iter().find(|(id, _)| *id == tree_info.root).unwrap().1.clone();
    assert_eq!(root.role(), Role::Window);

    let (_, play) = node_by_label(&update, "Play");
    assert_eq!(play.role(), Role::Button);
    assert_eq!(play.description(), Some("Starts playback"));
    assert_eq!(play.author_id(), Some("play-button"));
    assert!(play.supports_action(Action::Click));
    assert!(play.supports_action(Action::Focus));
    assert!(!play.supports_action(Action::Increment));
    assert!(!play.is_disabled());
    assert_eq!(play.toggled(), None);

    let (_, locked) = node_by_label(&update, "Locked");
    assert!(locked.is_disabled());

    let (_, mute) = node_by_label(&update, "Mute");
    assert_eq!(mute.role(), Role::CheckBox);
    assert_eq!(mute.toggled(), Some(Toggled::False));

    let (_, volume) = node_by_label(&update, "Volume");
    assert_eq!(volume.role(), Role::Slider);
    assert_eq!(volume.numeric_value(), Some(5.));
    assert_eq!(volume.min_numeric_value(), Some(0.));
    assert_eq!(volume.max_numeric_value(), Some(10.));
    assert_eq!(volume.numeric_value_step(), Some(2.));
    assert!(volume.supports_action(Action::Increment));
    assert!(volume.supports_action(Action::Decrement));
    assert!(volume.supports_action(Action::SetValue));
    assert!(!volume.supports_action(Action::Click));
}

#[test]
fn node_bounds_are_physical_pixels() {
    let mut f = Fixture::new();
    f.instance.window().dispatch_event(i_slint_core::platform::WindowEvent::ScaleFactorChanged {
        scale_factor: 2.,
    });
    let update = f.tree.build_new_tree();
    let root_id = update.tree.as_ref().unwrap().root;
    let root = &update.nodes.iter().find(|(id, _)| *id == root_id).unwrap().1;
    let bounds = root.bounds().unwrap();
    assert_eq!((bounds.x0, bounds.y0, bounds.x1, bounds.y1), (0., 0., 800., 600.));

    let (_, play) = node_by_label(&update, "Play");
    let play_bounds = play.bounds().unwrap();
    assert_eq!((play_bounds.x0, play_bounds.y0), (0., 0.));
    assert_eq!(play_bounds.y1 - play_bounds.y0, 80.);
}

#[test]
fn physical_offset_moves_the_bounds() {
    let mut f = Fixture::new();
    let before = f.tree.build_new_tree();
    f.tree.set_physical_offset(10., 24.);
    let after = f.tree.build_new_tree();

    let (_, a) = node_by_label(&before, "Mute");
    let (_, b) = node_by_label(&after, "Mute");
    let (a, b) = (a.bounds().unwrap(), b.bounds().unwrap());
    assert_eq!((b.x0 - a.x0, b.y0 - a.y0), (10., 24.));
    assert_eq!((b.x1 - a.x1, b.y1 - a.y1), (10., 24.));
}

#[test]
fn node_ids_survive_a_rebuild() {
    let mut f = Fixture::new();
    let first = f.tree.build_new_tree();
    f.instance.set_property("label", Value::String("Pause".into())).unwrap();
    let second = f.tree.build_new_tree();

    let ids = |update: &TreeUpdate| {
        update.nodes.iter().map(|(id, node)| (*id, node.role())).collect::<Vec<_>>()
    };
    assert_eq!(ids(&first), ids(&second));
    assert_eq!(node_by_label(&first, "Play").0, node_by_label(&second, "Pause").0);
    assert_eq!(first.tree.as_ref().unwrap().root, second.tree.as_ref().unwrap().root);
}

const REPEATED_SOURCE: &str = r#"
export component TestCase inherits Window {
    width: 400px;
    height: 300px;
    in-out property <bool> show: true;

    VerticalLayout {
        if root.show: Rectangle {
            height: 40px;
            accessible-role: button;
            accessible-label: "Shown";
        }
        Rectangle {
            height: 40px;
            accessible-role: button;
            accessible-label: "Always";
        }
    }
}
"#;

#[test]
fn a_destroyed_component_does_not_hand_its_node_ids_to_the_next_one() {
    let mut f = Fixture::with_source(REPEATED_SOURCE);
    let first = node_by_label(&f.tree.build_new_tree(), "Shown").0;

    f.instance.set_property("show", Value::Bool(false)).unwrap();
    f.tree.build_new_tree();
    f.instance.set_property("show", Value::Bool(true)).unwrap();
    let second = node_by_label(&f.tree.build_new_tree(), "Shown").0;

    assert_ne!(first, second);
}

#[test]
fn changed_property_rebuilds_only_its_node() {
    let mut f = Fixture::new();
    let first = f.tree.build_new_tree();
    let (play_id, _) = node_by_label(&first, "Play");
    let (mute_id, _) = node_by_label(&first, "Mute");

    assert_eq!(f.host.nodes_dirty.get(), 0);

    f.instance.set_property("checked", Value::Bool(true)).unwrap();
    assert_eq!(f.host.nodes_dirty.get(), 1, "the host is told once");
    assert!(f.tree.has_dirty_nodes());

    let update = f.tree.rebuild_dirty_nodes();
    assert!(update.tree.is_none(), "a partial update has no tree info");
    let changed: Vec<NodeId> = update.nodes.iter().map(|(id, _)| *id).collect();
    assert_eq!(changed, vec![mute_id]);
    assert_ne!(changed, vec![play_id]);
    assert_eq!(update.nodes[0].1.toggled(), Some(Toggled::True));

    // Nothing changed since: a second rebuild is empty.
    assert!(f.tree.rebuild_dirty_nodes().nodes.is_empty());
}

#[test]
fn set_dirty_rebuilds_the_focus() {
    let mut f = Fixture::new();
    f.tree.build_new_tree();
    assert!(f.tree.rebuild_dirty_nodes().nodes.is_empty());
    f.tree.set_dirty();
    assert!(f.tree.has_dirty_nodes());
    let update = f.tree.rebuild_dirty_nodes();
    assert!(update.nodes.is_empty(), "no node changed");
}

#[test]
fn focus_follows_the_window_and_the_focused_item() {
    let mut f = Fixture::new();
    let update = f.tree.build_new_tree();
    let root_id = update.tree.as_ref().unwrap().root;
    let (field_id, _) = node_by_label(&update, "Name");

    // An unfocused window reports the root.
    f.host.focused.set(false);
    assert_eq!(f.tree.focus_node(), root_id);

    // Without a focused item, the window's root item has the focus.
    f.host.focused.set(true);
    assert_eq!(f.tree.focus_node(), root_id);

    let DeferredAccessKitAction::SetFocus(item) =
        f.tree.handle_request(request(Action::Focus, field_id, None)).unwrap()
    else {
        panic!("Focus must map to SetFocus");
    };
    DeferredAccessKitAction::SetFocus(item).invoke(f.instance.window());
    assert_eq!(f.tree.focus_node(), field_id);
    assert_eq!(f.tree.focus_update().focus, field_id);
    assert!(f.tree.focus_update().nodes.is_empty());
}

#[test]
fn requests_map_to_accessibility_actions() {
    let mut f = Fixture::new();
    let update = f.tree.build_new_tree();
    let (play_id, _) = node_by_label(&update, "Play");
    let (mute_id, _) = node_by_label(&update, "Mute");
    let (volume_id, _) = node_by_label(&update, "Volume");

    let activated = Rc::new(Cell::new(0));
    let counter = activated.clone();
    f.instance
        .set_callback("activated", move |_| {
            counter.set(counter.get() + 1);
            Value::Void
        })
        .unwrap();

    let invoke = |f: &Fixture, request: ActionRequest| {
        f.tree.handle_request(request).expect("request is understood").invoke(f.instance.window());
    };
    let volume =
        |f: &Fixture| -> f64 { f.instance.get_property("volume").unwrap().try_into().unwrap() };

    invoke(&f, request(Action::Click, play_id, None));
    assert_eq!(activated.get(), 1);
    assert_eq!(f.instance.get_property("last-action").unwrap(), Value::String("default".into()));

    invoke(&f, request(Action::Click, mute_id, None));
    assert_eq!(f.instance.get_property("checked").unwrap(), Value::Bool(true));

    invoke(&f, request(Action::Increment, volume_id, None));
    assert_eq!(volume(&f), 7.);
    invoke(&f, request(Action::Decrement, volume_id, None));
    invoke(&f, request(Action::Decrement, volume_id, None));
    assert_eq!(volume(&f), 3.);
    invoke(&f, request(Action::SetValue, volume_id, Some(ActionData::NumericValue(8.))));
    assert_eq!(volume(&f), 8.);
    invoke(&f, request(Action::SetValue, volume_id, Some(ActionData::Value("1".into()))));
    assert_eq!(volume(&f), 1.);

    // Unsupported actions, a missing payload and unknown nodes are not translated.
    assert!(f.tree.handle_request(request(Action::ScrollDown, volume_id, None)).is_none());
    assert!(f.tree.handle_request(request(Action::SetValue, volume_id, None)).is_none());
    assert!(f.tree.handle_request(request(Action::ReplaceSelectedText, volume_id, None)).is_none());
    assert!(f.tree.handle_request(request(Action::Click, NodeId(0xFFFF_FFFF), None)).is_none());
}

#[test]
fn text_input_has_a_value_and_text_runs() {
    let mut f = Fixture::new();
    let first = f.tree.build_new_tree();

    // The builder reads `single-line` without evaluating its binding, so let the item evaluate it
    // the way rendering would, before building the tree that is checked.
    let (field_id, _) = node_by_label(&first, "Name");
    let Some(DeferredAccessKitAction::SetFocus(field_item)) =
        f.tree.handle_request(request(Action::Focus, field_id, None))
    else {
        panic!("Focus must map to SetFocus");
    };
    let text_input = i_slint_core::accessibility::find_text_input(&field_item).unwrap();
    assert!(text_input.as_pin_ref().single_line());

    let update = f.tree.build_new_tree();
    let (field_id, field) = node_by_label(&update, "Name");
    assert_eq!(field.role(), Role::TextInput);
    assert_eq!(field.value(), Some("Hello"));
    assert!(field.supports_action(Action::Focus));

    // The TextRun children come after the regular children and use sub-index NodeIds, which
    // leave the wrapper's own NodeId bits intact.
    let runs: Vec<_> = field.children().to_vec();
    assert!(!runs.is_empty(), "a text input emits TextRun children");
    for run in &runs {
        assert_ne!(*run, field_id);
        let (_, node) = update.nodes.iter().find(|(id, _)| id == run).expect("child is emitted");
        assert_eq!(node.role(), Role::TextRun);
        assert_eq!(node.value(), Some("Hello"));
    }
    // Rebuilding emits the same TextRun NodeIds again.
    let rebuilt = f.tree.build_new_tree();
    let (_, again) = node_by_label(&rebuilt, "Name");
    assert_eq!(again.children(), field.children());
}

#[test]
fn clearing_the_text_state_keeps_the_tree_usable() {
    let mut f = Fixture::new();
    f.tree.build_new_tree();
    f.tree.clear_text_state();
    let update = f.tree.build_new_tree();
    assert!(update.tree.is_some());
}
