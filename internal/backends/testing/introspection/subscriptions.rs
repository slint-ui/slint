// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Subscriptions that push changes of an element's properties, or of an element tree,
//! once per event-loop pass, when the change handlers run.

use i_slint_core::properties::ChangeTracker;
use prost::Message;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::{Rc, Weak};

use super::{ArenaIndex, IntrospectionState, index_to_handle, proto, tree_node};
use crate::ElementHandle;
use crate::systest::{NotificationSink, SubscriptionNotification};

/// The property and tree subscriptions, which share one ID space.
#[derive(Default)]
pub(crate) struct Subscriptions {
    last_id: Cell<u64>,
    trackers: RefCell<HashMap<u64, ChangeTracker>>,
}

impl Subscriptions {
    fn next_id(&self) -> u64 {
        let id = self.last_id.get() + 1;
        self.last_id.set(id);
        id
    }

    fn insert(&self, id: u64, tracker: ChangeTracker) {
        self.trackers.borrow_mut().insert(id, tracker);
    }

    /// Ends the subscription `id`, unless it already ended.
    pub(crate) fn remove(&self, id: u64) {
        self.trackers.borrow_mut().remove(&id);
    }

    pub(crate) fn clear(&self) -> Vec<u64> {
        let mut ids: Vec<u64> = self.trackers.take().into_keys().collect();
        ids.sort_unstable();
        ids
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.trackers.borrow().len()
    }
}

/// Initializes `tracker`, and returns the result of the first `eval`, which `notify` doesn't get.
fn init_tracker<Data: 'static, T: Default + PartialEq + 'static>(
    tracker: &ChangeTracker,
    data: Data,
    eval: impl Fn(&Data) -> T + 'static,
    notify: impl Fn(&Data, &T) + 'static,
) -> Rc<T> {
    let first = Rc::new(RefCell::new(None));
    let first_slot = Rc::downgrade(&first);
    tracker.init(
        data,
        move |data| {
            let value = Rc::new(eval(data));
            if let Some(slot) = first_slot.upgrade() {
                *slot.borrow_mut() = Some(value.clone());
            }
            value
        },
        move |data, value: &Rc<T>| notify(data, value),
    );
    first.take().expect("ChangeTracker::init evaluates right away")
}

/// Removes the subscription `id`, and tells the debugger through `sink`.
fn end(state: &Weak<IntrospectionState>, sink: &NotificationSink, id: u64) {
    sink(SubscriptionNotification::Ended { subscription: id });
    if let Some(state) = state.upgrade() {
        state.subscriptions.remove(id);
    }
}

struct PropertiesSubscription {
    id: u64,
    element: ElementHandle,
    state: Weak<IntrospectionState>,
    sink: NotificationSink,
    last_pushed: RefCell<Vec<proto::TestableProperty>>,
}

impl PropertiesSubscription {
    /// Returns None once the element is destroyed, or if it doesn't support inspection.
    fn properties(&self) -> Option<Vec<proto::TestableProperty>> {
        self.element.track_existence();
        super::inspectable_properties(&self.element)
    }

    fn push_changes(&self, properties: Option<&[proto::TestableProperty]>) {
        let Some(properties) = properties else {
            return end(&self.state, &self.sink, self.id);
        };
        let changed: Vec<_> = {
            let last_pushed = self.last_pushed.borrow();
            properties
                .iter()
                .enumerate()
                .filter(|(index, property)| last_pushed.get(*index) != Some(property))
                .map(|(_, property)| property.clone())
                .collect()
        };
        *self.last_pushed.borrow_mut() = properties.to_vec();
        if !changed.is_empty() {
            (self.sink)(SubscriptionNotification::PropertiesChanged {
                subscription: self.id,
                changed: proto::ElementPropertiesChanged { changed }.encode_to_vec(),
            });
        }
    }
}

pub(crate) fn subscribe_element_properties(
    state: &Rc<IntrospectionState>,
    element: ArenaIndex,
    sink: &NotificationSink,
) -> Result<proto::SubscribeElementPropertiesResponse, String> {
    const REQUEST: &str = "subscribe_element_properties";
    let element = state.element(REQUEST, element)?;
    let subscription_id = state.subscriptions.next_id();
    let subscription = Rc::new(PropertiesSubscription {
        id: subscription_id,
        element,
        state: Rc::downgrade(state),
        sink: sink.clone(),
        last_pushed: Default::default(),
    });
    let tracker = ChangeTracker::default();
    let properties = init_tracker(
        &tracker,
        subscription.clone(),
        |subscription| subscription.properties(),
        |subscription, properties| subscription.push_changes(properties.as_deref()),
    );
    let Some(properties) = properties.as_ref() else {
        if subscription.element.is_valid() {
            return Ok(proto::SubscribeElementPropertiesResponse::default());
        }
        return Err(format!("Element handle for {REQUEST} refers to element that was destroyed"));
    };
    *subscription.last_pushed.borrow_mut() = properties.clone();
    state.subscriptions.insert(subscription_id, tracker);
    Ok(proto::SubscribeElementPropertiesResponse {
        subscription_id,
        properties: properties.clone(),
        available: true,
    })
}

/// The elements of a tree walk in pre-order, with their depths.
#[derive(Default)]
struct TreeShape(Vec<(ElementHandle, usize)>);

impl PartialEq for TreeShape {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self.0.iter().zip(&other.0).all(|((element, depth), (other, other_depth))| {
                depth == other_depth && element.is_same_element(other)
            })
    }
}

/// Walks the tree under `root`, or the trees of every window.
fn tree_shape(state: &IntrospectionState, root: Option<&ElementHandle>) -> TreeShape {
    let roots = match root {
        Some(root) => vec![root.clone()],
        None => state.window_root_elements(),
    };
    let mut shape = Vec::new();
    for root in roots {
        root.instantiate_tree();
        root.visit_tree(|element, depth| {
            shape.push((element, depth));
            std::ops::ControlFlow::<()>::Continue(())
        });
    }
    TreeShape(shape)
}

/// Mints and pins a handle for each element of `shape`.
fn pin_shape(state: &IntrospectionState, shape: &TreeShape) -> Vec<(ArenaIndex, usize)> {
    shape
        .0
        .iter()
        .map(|(element, depth)| {
            let handle = state.element_to_handle(element.clone());
            // Pinned before minting the next handle, which could evict it.
            state.pin_handle(handle);
            (handle, *depth)
        })
        .collect()
}

struct TreeSubscription {
    id: u64,
    state: Weak<IntrospectionState>,
    root: Option<ElementHandle>,
    sink: NotificationSink,
    /// The pinned handles of the last pushed tree, with their depths.
    handles: RefCell<Vec<(ArenaIndex, usize)>>,
}

impl TreeSubscription {
    /// Returns None once the root is destroyed.
    fn shape(&self) -> Option<TreeShape> {
        if let Some(root) = &self.root {
            root.track_existence();
            if !root.is_valid() {
                return None;
            }
        }
        Some(match self.state.upgrade() {
            Some(state) => tree_shape(&state, self.root.as_ref()),
            None => TreeShape::default(),
        })
    }

    fn push_changes(&self, shape: Option<&TreeShape>) {
        let Some(shape) = shape else {
            return end(&self.state, &self.sink, self.id);
        };
        let Some(state) = self.state.upgrade() else { return };
        if shape.0.len() > state.handles_per_response() {
            return end(&self.state, &self.sink, self.id);
        }
        let new = pin_shape(&state, shape);
        let old = self.handles.take();
        for (handle, _) in &old {
            state.unpin_handle(*handle);
        }
        let diff = TreeDiff::new(&old, &new);
        let changed = proto::ElementTreeChanged {
            removed: diff.removed_roots.into_iter().map(index_to_handle).collect(),
            inserted: diff
                .inserted
                .into_iter()
                .map(|(after, range)| proto::InsertedElements {
                    after: after.map(index_to_handle),
                    elements: range
                        .map(|index| tree_node(&shape.0[index].0, new[index].0, new[index].1))
                        .collect(),
                })
                .collect(),
        };
        *self.handles.borrow_mut() = new;
        if !changed.removed.is_empty() || !changed.inserted.is_empty() {
            (self.sink)(SubscriptionNotification::ElementTreeChanged {
                subscription: self.id,
                changed: changed.encode_to_vec(),
            });
        }
    }
}

impl Drop for TreeSubscription {
    fn drop(&mut self) {
        if let Some(state) = self.state.upgrade() {
            for (handle, _) in self.handles.get_mut() {
                state.unpin_handle(*handle);
            }
        }
    }
}

/// The change from one pre-order list of handles to another.
#[derive(Default)]
struct TreeDiff {
    /// The removed handles whose parent survives.
    removed_roots: Vec<ArenaIndex>,
    /// Runs of the new list, each after the handle that precedes it there.
    inserted: Vec<(Option<ArenaIndex>, Range<usize>)>,
}

impl TreeDiff {
    fn new(old: &[(ArenaIndex, usize)], new: &[(ArenaIndex, usize)]) -> Self {
        Self::by_key(old, new).unwrap_or_else(|| Self {
            removed_roots: old
                .iter()
                .filter(|(_, depth)| *depth == 0)
                .map(|(handle, _)| *handle)
                .collect(),
            inserted: if new.is_empty() { Vec::new() } else { vec![(None, 0..new.len())] },
        })
    }

    /// Returns None when the survivors changed their order.
    fn by_key(old: &[(ArenaIndex, usize)], new: &[(ArenaIndex, usize)]) -> Option<Self> {
        let old_handles: HashSet<ArenaIndex> = old.iter().map(|(handle, _)| *handle).collect();
        let new_handles: HashSet<ArenaIndex> = new.iter().map(|(handle, _)| *handle).collect();
        let mut diff = Self::default();
        // The depths of the old elements on the path to the current one, and whether each is removed.
        let mut old_ancestors: Vec<(usize, bool)> = Vec::new();
        let mut enter_old = |depth: usize, removed: bool| {
            while old_ancestors.last().is_some_and(|(ancestor, _)| *ancestor >= depth) {
                old_ancestors.pop();
            }
            let parent_removed = old_ancestors.last().is_some_and(|(_, removed)| *removed);
            old_ancestors.push((depth, removed));
            parent_removed
        };
        let (mut i, mut j) = (0, 0);
        loop {
            if let Some(&(handle, depth)) = old.get(i)
                && !new_handles.contains(&handle)
            {
                if !enter_old(depth, true) {
                    diff.removed_roots.push(handle);
                }
                i += 1;
            } else if j < new.len() && !old_handles.contains(&new[j].0) {
                let start = j;
                while j < new.len() && !old_handles.contains(&new[j].0) {
                    j += 1;
                }
                diff.inserted.push((start.checked_sub(1).map(|after| new[after].0), start..j));
            } else {
                match (old.get(i), new.get(j)) {
                    (None, None) => return Some(diff),
                    (Some(old), Some(new)) if old.0 == new.0 => {
                        debug_assert_eq!(old.1, new.1, "a surviving element changed its depth");
                        enter_old(old.1, false);
                        i += 1;
                        j += 1;
                    }
                    _ => return None,
                }
            }
        }
    }
}

pub(crate) fn subscribe_element_tree(
    state: &Rc<IntrospectionState>,
    root: Option<ArenaIndex>,
    sink: &NotificationSink,
) -> Result<proto::SubscribeElementTreeResponse, String> {
    state.ensure_windows_instantiated();
    let root = root.map(|root| state.element("subscribe_element_tree", root)).transpose()?;
    let subscription_id = state.subscriptions.next_id();
    let subscription = Rc::new(TreeSubscription {
        id: subscription_id,
        state: Rc::downgrade(state),
        root,
        sink: sink.clone(),
        handles: Default::default(),
    });
    let tracker = ChangeTracker::default();
    let shape = init_tracker(
        &tracker,
        subscription.clone(),
        |subscription| subscription.shape(),
        |subscription, shape| subscription.push_changes(shape.as_ref()),
    );
    let Some(shape) = shape.as_ref() else {
        return Err(
            "Element handle for subscribe_element_tree refers to element that was destroyed".into(),
        );
    };
    let limit = state.handles_per_response();
    if shape.0.len() > limit {
        return Err(format!(
            "The element tree has {} elements, more than the {limit} a subscription can hold; \
             subscribe to a subtree instead",
            shape.0.len()
        ));
    }
    let handles = pin_shape(state, shape);
    let elements = shape
        .0
        .iter()
        .zip(&handles)
        .map(|((element, _), (handle, depth))| tree_node(element, *handle, *depth))
        .collect();
    *subscription.handles.borrow_mut() = handles;
    state.subscriptions.insert(subscription_id, tracker);
    let debug_info_available = match &subscription.root {
        Some(root) => root.has_debug_info(),
        None => state.window_root_elements().iter().all(ElementHandle::has_debug_info),
    };
    Ok(proto::SubscribeElementTreeResponse { subscription_id, elements, debug_info_available })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{ELEMENT_HANDLE_CAP, handle_to_index, window_adapter_for_test};
    use slint::{Model, VecModel};

    slint::slint! {
        #[edition = "1.19"]
        export component App inherits Window {
            width: 100px;
            height: 100px;
            in property <[int]> model;
            in property <bool> condition;
            VerticalLayout {
                for value in model: row := Text { text: value; }
            }
            if condition: conditional := Rectangle {
                conditional-child := Rectangle { }
            }
            popup := PopupWindow {
                popup-text := Text { text: "popup"; }
            }
            public function show-popup() { popup.show(); }
            public function close-popup() { popup.close(); }
        }
    }

    struct Fixture {
        app: App,
        rows: Rc<VecModel<i32>>,
        state: Rc<IntrospectionState>,
        sink: NotificationSink,
        pushed: Rc<RefCell<Vec<SubscriptionNotification>>>,
    }

    impl Fixture {
        fn new() -> Self {
            crate::init_no_event_loop();
            let state = Rc::new(IntrospectionState::new());
            let app = App::new().unwrap();
            let rows = Rc::new(VecModel::from(vec![1, 2]));
            app.set_model(rows.clone().into());
            state.add_window(&window_adapter_for_test(&app));
            let (sink, pushed) = capture();
            Self { app, rows, state, sink, pushed }
        }

        fn subscribe_tree(&self) -> proto::SubscribeElementTreeResponse {
            subscribe_element_tree(&self.state, None, &self.sink).unwrap()
        }

        fn tree_changes(&self) -> Vec<proto::ElementTreeChanged> {
            tree_changes(&self.pushed)
        }

        fn id(&self, handle: proto::Handle) -> String {
            let element = self.state.element("test", handle_to_index(handle).unwrap()).unwrap();
            element.id().unwrap_or_default().to_string()
        }

        /// The id and depth of each inserted element, per run.
        fn inserted(&self, changed: &proto::ElementTreeChanged) -> Vec<Vec<(String, u32)>> {
            changed
                .inserted
                .iter()
                .map(|run| {
                    run.elements
                        .iter()
                        .map(|node| (self.id(node.handle.unwrap()), node.depth))
                        .collect()
                })
                .collect()
        }
    }

    /// Runs the change handlers and returns the tree changes they pushed.
    fn tree_changes(
        pushed: &RefCell<Vec<SubscriptionNotification>>,
    ) -> Vec<proto::ElementTreeChanged> {
        ChangeTracker::run_change_handlers();
        pushed
            .take()
            .into_iter()
            .map(|notification| match notification {
                SubscriptionNotification::ElementTreeChanged { changed, .. } => {
                    proto::ElementTreeChanged::decode(&changed[..]).unwrap()
                }
                _ => panic!("expected a tree change"),
            })
            .collect()
    }

    fn root(state: &IntrospectionState) -> ArenaIndex {
        state.windows.borrow().values().next().unwrap().root_element_handle
    }

    fn capture() -> (NotificationSink, Rc<RefCell<Vec<SubscriptionNotification>>>) {
        let pushed = Rc::new(RefCell::new(Vec::new()));
        let sink_pushed = pushed.clone();
        (Rc::new(move |notification| sink_pushed.borrow_mut().push(notification)), pushed)
    }

    fn is_pinned(state: &IntrospectionState, handle: &proto::Handle) -> bool {
        let index = handle_to_index(*handle).unwrap();
        state.element_handles.borrow().get(index).is_some_and(|tracked| tracked.pins.get() > 0)
    }

    #[test]
    fn unsubscribing_an_ended_subscription_succeeds() {
        let fixture = Fixture::new();
        let id = fixture.subscribe_tree().subscription_id;
        assert_eq!(fixture.state.subscriptions.clear(), vec![id]);
        fixture.state.subscriptions.remove(id);
        fixture.state.subscriptions.remove(42);
    }

    #[test]
    fn subscribing_to_a_stale_handle_errors() {
        let fixture = Fixture::new();
        let stale = ArenaIndex::default();
        let err = subscribe_element_properties(&fixture.state, stale, &fixture.sink).unwrap_err();
        assert!(err.contains("Invalid element handle"), "got: {err}");
        let err = subscribe_element_tree(&fixture.state, Some(stale), &fixture.sink).unwrap_err();
        assert!(err.contains("Invalid element handle"), "got: {err}");
        assert_eq!(fixture.state.subscriptions.len(), 0);
    }

    /// Generated Rust code doesn't support inspection.
    #[test]
    fn property_subscription_is_unavailable() {
        let fixture = Fixture::new();
        let response =
            subscribe_element_properties(&fixture.state, root(&fixture.state), &fixture.sink)
                .unwrap();
        assert!(!response.available);
        assert!(response.properties.is_empty());
        assert_eq!(fixture.state.subscriptions.len(), 0);
    }

    slint::slint! {
        #[edition = "1.19"]
        export component Large inherits Window {
            in property <int> count;
            for _ in count: Rectangle { }
        }
    }

    /// More elements than one response can hold.
    const LARGE_COUNT: i32 = 9000;

    fn large(count: i32) -> (Large, Rc<IntrospectionState>) {
        crate::init_no_event_loop();
        let state = Rc::new(IntrospectionState::new());
        let app = Large::new().unwrap();
        app.set_count(count);
        state.add_window(&window_adapter_for_test(&app));
        assert!(LARGE_COUNT as usize > state.handles_per_response());
        (app, state)
    }

    #[test]
    fn subscribing_to_a_tree_larger_than_a_response_errors() {
        let (_app, state) = large(LARGE_COUNT);
        let (sink, _) = capture();
        let err = subscribe_element_tree(&state, None, &sink).unwrap_err();
        assert!(err.contains("subscribe to a subtree"), "got: {err}");
        assert_eq!(state.subscriptions.len(), 0);
        assert_eq!(state.pinned_handle_count.get(), 0);
    }

    #[test]
    fn a_tree_growing_larger_than_a_response_ends_its_subscription() {
        let (app, state) = large(1);
        let (sink, pushed) = capture();
        let id = subscribe_element_tree(&state, None, &sink).unwrap().subscription_id;
        assert_eq!(state.pinned_handle_count.get(), 2);
        app.set_count(LARGE_COUNT);
        ChangeTracker::run_change_handlers();
        assert!(matches!(
            pushed.take().as_slice(),
            [SubscriptionNotification::Ended { subscription }] if *subscription == id
        ));
        assert_eq!(state.subscriptions.len(), 0);
        assert_eq!(state.pinned_handle_count.get(), 0);
    }

    #[test]
    fn model_rows_push_inserted_and_removed_runs() {
        let fixture = Fixture::new();
        let initial = fixture.subscribe_tree();
        let rows: Vec<_> = initial
            .elements
            .iter()
            .filter_map(|node| node.handle)
            .filter(|handle| fixture.id(*handle) == "App::row")
            .collect();
        assert_eq!(rows.len(), 2);

        fixture.rows.push(3);
        fixture.rows.push(4);
        let [changed] = fixture.tree_changes().try_into().unwrap();
        assert!(changed.removed.is_empty());
        assert_eq!(
            fixture.inserted(&changed),
            [[("App::row".to_string(), 2), ("App::row".into(), 2)]]
        );
        assert_eq!(changed.inserted[0].after, Some(rows[1]));

        fixture.rows.remove(0);
        let [changed] = fixture.tree_changes().try_into().unwrap();
        assert_eq!(changed.removed, [rows[0]]);
        assert!(changed.inserted.is_empty());
    }

    #[test]
    fn changing_row_data_pushes_nothing() {
        let fixture = Fixture::new();
        fixture.subscribe_tree();
        fixture.rows.set_row_data(0, 10);
        assert!(fixture.tree_changes().is_empty());
        assert_eq!(fixture.rows.row_count(), 2);
    }

    #[test]
    fn toggling_a_condition_pushes_the_subtree() {
        let fixture = Fixture::new();
        fixture.subscribe_tree();

        fixture.app.set_condition(true);
        let [changed] = fixture.tree_changes().try_into().unwrap();
        assert!(changed.removed.is_empty());
        assert_eq!(
            fixture.inserted(&changed),
            [[("App::conditional".to_string(), 1), ("App::conditional-child".into(), 2)]]
        );
        let conditional = changed.inserted[0].elements[0].handle.unwrap();
        let child = changed.inserted[0].elements[1].handle.unwrap();
        assert!(is_pinned(&fixture.state, &child));

        fixture.app.set_condition(false);
        let [changed] = fixture.tree_changes().try_into().unwrap();
        assert_eq!(changed.removed, [conditional]);
        assert!(changed.inserted.is_empty());
        assert!(!is_pinned(&fixture.state, &child));
    }

    #[test]
    fn destroying_the_root_ends_its_tree_subscription() {
        let fixture = Fixture::new();
        fixture.app.set_condition(true);
        fixture.state.ensure_windows_instantiated();
        let conditional =
            ElementHandle::find_by_element_id(&fixture.app, "App::conditional").next().unwrap();
        let conditional = fixture.state.element_to_handle(conditional);
        let id = subscribe_element_tree(&fixture.state, Some(conditional), &fixture.sink)
            .unwrap()
            .subscription_id;

        fixture.app.set_condition(false);
        ChangeTracker::run_change_handlers();
        assert!(matches!(
            fixture.pushed.take().as_slice(),
            [SubscriptionNotification::Ended { subscription }] if *subscription == id
        ));
        assert_eq!(fixture.state.subscriptions.len(), 0);
        assert_eq!(fixture.state.pinned_handle_count.get(), 0);
    }

    #[test]
    fn opening_and_closing_a_popup_pushes_it_under_its_parent() {
        let fixture = Fixture::new();
        fixture.subscribe_tree();

        fixture.app.invoke_show_popup();
        let [changed] = fixture.tree_changes().try_into().unwrap();
        assert!(changed.removed.is_empty());
        assert_eq!(
            fixture.inserted(&changed),
            [[("App::popup".to_string(), 1), ("App::popup-text".into(), 2)]]
        );
        assert_eq!(changed.inserted[0].after, Some(index_to_handle(root(&fixture.state))));
        let popup = changed.inserted[0].elements[0].handle.unwrap();

        fixture.app.invoke_close_popup();
        let [changed] = fixture.tree_changes().try_into().unwrap();
        assert_eq!(changed.removed, [popup]);
        assert!(changed.inserted.is_empty());
    }

    #[test]
    fn pinned_handles_survive_eviction() {
        let fixture = Fixture::new();
        let initial = fixture.subscribe_tree();
        let others: Vec<_> = (0..ELEMENT_HANDLE_CAP / 100 + 10)
            .map(|_| {
                let app = super::super::hundred_rectangles::App::new().unwrap();
                super::super::mention_window(&fixture.state, &app);
                app
            })
            .collect();
        let arena = fixture.state.element_handles.borrow();
        assert!(arena.len() <= ELEMENT_HANDLE_CAP + initial.elements.len());
        for node in &initial.elements {
            assert!(arena.contains_key(handle_to_index(node.handle.unwrap()).unwrap()));
        }
        drop((arena, others));
    }

    #[test]
    fn unsubscribing_a_tree_unpins_its_handles() {
        let fixture = Fixture::new();
        let initial = fixture.subscribe_tree();
        let other = fixture.subscribe_tree();
        assert!(
            initial.elements.iter().all(|node| is_pinned(&fixture.state, &node.handle.unwrap()))
        );

        fixture.state.subscriptions.remove(initial.subscription_id);
        assert!(
            initial.elements.iter().all(|node| is_pinned(&fixture.state, &node.handle.unwrap()))
        );
        fixture.state.subscriptions.remove(other.subscription_id);
        assert!(
            !initial.elements.iter().any(|node| is_pinned(&fixture.state, &node.handle.unwrap()))
        );
        assert_eq!(fixture.state.pinned_handle_count.get(), 0);

        fixture.rows.push(3);
        assert!(fixture.tree_changes().is_empty());
    }

    #[test]
    fn subscription_without_root_pushes_a_window_added_later() {
        crate::init_no_event_loop();
        let state = Rc::new(IntrospectionState::new());
        let (sink, pushed) = capture();
        let initial = subscribe_element_tree(&state, None, &sink).unwrap();
        assert!(initial.elements.is_empty());

        let app = App::new().unwrap();
        state.add_window(&window_adapter_for_test(&app));
        let [changed] = tree_changes(&pushed).try_into().unwrap();
        assert!(changed.removed.is_empty());
        let [run] = changed.inserted.try_into().unwrap();
        assert_eq!(run.after, None);
        assert_eq!(run.elements[0].handle, Some(index_to_handle(root(&state))));
        assert_eq!(run.elements[0].depth, 0);
    }

    #[test]
    fn tree_diff() {
        let mut arena = slotmap::SlotMap::<ArenaIndex, ()>::with_key();
        let [a, b, c, d] = std::array::from_fn(|_| arena.insert(()));
        #[rustfmt::skip]
        let cases: [(&[(ArenaIndex, usize)], &[(ArenaIndex, usize)], &[ArenaIndex], &[(Option<ArenaIndex>, Range<usize>)]); 6] = [
            (&[(a, 0), (b, 0)], &[(c, 0), (a, 0), (b, 0)], &[], &[(None, 0..1)]),
            (&[(a, 0), (b, 0)], &[(a, 0), (c, 0), (b, 0)], &[], &[(Some(a), 1..2)]),
            (&[(a, 0), (b, 0)], &[(a, 0), (b, 0), (c, 0)], &[], &[(Some(b), 2..3)]),
            (&[(a, 0), (b, 1), (c, 2), (d, 1)], &[(a, 0), (d, 1)], &[b], &[]),
            (&[(a, 0), (b, 1), (c, 1)], &[(a, 0), (d, 1), (c, 1)], &[b], &[(Some(a), 1..2)]),
            (&[(a, 0), (b, 1), (c, 0)], &[(c, 0), (a, 0), (b, 1)], &[a, c], &[(None, 0..3)]),
        ];
        for (old, new, removed_roots, inserted) in cases {
            let diff = TreeDiff::new(old, new);
            assert_eq!(diff.removed_roots, removed_roots);
            assert_eq!(diff.inserted, inserted);
        }
    }
}
