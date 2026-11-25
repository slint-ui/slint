// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Subscriptions that push changes of an element's properties, or of its children,
//! once per event-loop pass, when the change handlers run.

use i_slint_core::properties::ChangeTracker;
use prost::Message;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

use super::{ArenaIndex, ElementPin, IntrospectionState, index_to_handle, proto};
use crate::ElementHandle;
use crate::systest::NotificationSink;
use proto::subscription_notification::Msg as Notification;

/// The property and children subscriptions, which share one ID space.
#[derive(Default)]
pub(crate) struct Subscriptions {
    last_id: Cell<u64>,
    trackers: RefCell<HashMap<u64, ChangeTracker>>,
    /// The children subscription of each parent, `None` for the window roots.
    children_of: RefCell<HashMap<Option<ArenaIndex>, u64>>,
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
        self.children_of.borrow_mut().retain(|_, subscription| *subscription != id);
        self.trackers.borrow_mut().remove(&id);
    }

    /// Ends the children subscription of `parent`, if there is one.
    fn remove_children_of(&self, parent: Option<ArenaIndex>) {
        let id = self.children_of.borrow_mut().remove(&parent);
        if let Some(id) = id {
            self.remove(id);
        }
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

fn remove(state: &Weak<IntrospectionState>, id: u64) {
    if let Some(state) = state.upgrade() {
        state.subscriptions.remove(id);
    }
}

/// Pushes `notification`, and ends the subscription `id` if `sink` fails.
fn push(
    state: &Weak<IntrospectionState>,
    sink: &NotificationSink,
    id: u64,
    notification: Notification,
) {
    let encoded = proto::SubscriptionNotification { subscription_id: id, msg: Some(notification) }
        .encode_to_vec();
    if sink(encoded).is_err() {
        remove(state, id);
    }
}

/// Removes the subscription `id`, and tells the debugger through `sink`.
fn end(state: &Weak<IntrospectionState>, sink: &NotificationSink, id: u64) {
    push(state, sink, id, Notification::Ended(proto::SubscriptionEnded {}));
    remove(state, id);
}

/// Makes the calling binding re-evaluate when `element` might have been destroyed,
/// and returns whether it still exists.
fn track_existence(state: &IntrospectionState, element: &ElementHandle) -> bool {
    state.track_windows();
    element.track_existence();
    element.is_valid()
}

fn destroyed(request: &str) -> String {
    format!("Element handle for {request} refers to element that was destroyed")
}

struct PropertiesSubscription {
    id: u64,
    element: Rc<ElementPin>,
    state: Weak<IntrospectionState>,
    sink: NotificationSink,
    last_pushed: RefCell<Vec<proto::TestableProperty>>,
}

impl PropertiesSubscription {
    /// Returns None once the element is destroyed, or if the application has no debug info.
    fn properties(&self) -> Option<Vec<proto::TestableProperty>> {
        let state = self.state.upgrade()?;
        track_existence(&state, &self.element)
            .then(|| super::testable_properties(&self.element))
            .flatten()
    }

    fn push_changes(&self, properties: Option<&[proto::TestableProperty]>) {
        let Some(properties) = properties else {
            return end(&self.state, &self.sink, self.id);
        };
        let changed: Vec<_> = {
            let last_pushed = self.last_pushed.borrow();
            properties.iter().filter(|property| !last_pushed.contains(property)).cloned().collect()
        };
        *self.last_pushed.borrow_mut() = properties.to_vec();
        if !changed.is_empty() {
            push(
                &self.state,
                &self.sink,
                self.id,
                Notification::PropertiesChanged(proto::ElementPropertiesChanged { changed }),
            );
        }
    }
}

pub(crate) fn subscribe_element_properties(
    state: &Rc<IntrospectionState>,
    element: ArenaIndex,
    sink: &NotificationSink,
) -> Result<proto::SubscribeElementPropertiesResponse, String> {
    const REQUEST: &str = "subscribe_element_properties";
    let element = state.pin_element(state.element(REQUEST, element)?).1;
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
        return Err(destroyed(REQUEST));
    };
    *subscription.last_pushed.borrow_mut() = properties.clone();
    state.subscriptions.insert(subscription_id, tracker);
    Ok(proto::SubscribeElementPropertiesResponse {
        subscription_id,
        properties: properties.clone(),
        available: true,
    })
}

/// The children of an element, each with whether it has children of its own.
#[derive(Default)]
struct Children {
    children: Vec<(ElementHandle, bool)>,
    debug_info_available: bool,
}

impl PartialEq for Children {
    fn eq(&self, other: &Self) -> bool {
        self.debug_info_available == other.debug_info_available
            && self.children.len() == other.children.len()
            && self.children.iter().zip(&other.children).all(
                |((element, has_children), (other, other_has_children))| {
                    has_children == other_has_children && element.is_same_element(other)
                },
            )
    }
}

/// A child as the debugger last saw it.
struct PushedChild {
    handle: ArenaIndex,
    element: Rc<ElementPin>,
    has_children: bool,
}

impl PushedChild {
    fn to_proto(&self) -> proto::ElementChild {
        proto::ElementChild {
            handle: Some(index_to_handle(self.handle)),
            type_names_and_ids: super::type_names_and_ids(&self.element),
            has_children: self.has_children,
        }
    }
}

fn pin_children(
    state: &IntrospectionState,
    children: &[(ElementHandle, bool)],
) -> Vec<PushedChild> {
    children
        .iter()
        .map(|(element, has_children)| {
            let (handle, element) = state.pin_element(element.clone());
            PushedChild { handle, element, has_children: *has_children }
        })
        .collect()
}

/// Returns the children of `parent`, or the root elements of every window,
/// or None once `parent` is destroyed.
fn children(state: &IntrospectionState, parent: Option<&ElementHandle>) -> Option<Children> {
    use std::ops::ControlFlow;
    let (children, debug_info_available) = match parent {
        Some(parent) => {
            if !track_existence(state, parent) {
                return None;
            }
            parent.instantiate_item_tree();
            let mut children = Vec::new();
            parent.visit_children(|child| {
                children.push(child);
                ControlFlow::<()>::Continue(())
            });
            (children, parent.has_debug_info())
        }
        None => {
            state.track_windows();
            let roots = state.window_root_elements();
            let debug_info_available = roots.iter().all(ElementHandle::has_debug_info);
            (roots, debug_info_available)
        }
    };
    let children = children
        .into_iter()
        .map(|child| {
            child.instantiate_item_tree();
            let has_children = child.visit_children(|_| ControlFlow::Break(())).is_some();
            (child, has_children)
        })
        .collect();
    Some(Children { children, debug_info_available })
}

struct ChildrenSubscription {
    id: u64,
    state: Weak<IntrospectionState>,
    parent: Option<Rc<ElementPin>>,
    sink: NotificationSink,
    pushed: RefCell<Vec<PushedChild>>,
}

impl ChildrenSubscription {
    fn children(&self) -> Option<Children> {
        children(&*self.state.upgrade()?, self.parent.as_deref().map(|parent| &**parent))
    }

    fn push_changes(&self, children: Option<&Children>) {
        let Some(children) = children else {
            return end(&self.state, &self.sink, self.id);
        };
        let Some(state) = self.state.upgrade() else { return };
        if children.children.len() > state.handles_per_response() {
            return end(&self.state, &self.sink, self.id);
        }
        let new = pin_children(&state, &children.children);
        let changed = diff_children(&self.pushed.borrow(), &new);
        *self.pushed.borrow_mut() = new;
        if !changed.removed.is_empty()
            || !changed.inserted.is_empty()
            || !changed.updated.is_empty()
        {
            push(&self.state, &self.sink, self.id, Notification::ChildrenChanged(changed));
        }
    }
}

fn diff_children(old: &[PushedChild], new: &[PushedChild]) -> proto::ElementChildrenChanged {
    let old_handles: HashSet<ArenaIndex> = old.iter().map(|child| child.handle).collect();
    let new_handles: HashSet<ArenaIndex> = new.iter().map(|child| child.handle).collect();
    let survivors = |children: &[PushedChild], other: &HashSet<ArenaIndex>| -> Vec<ArenaIndex> {
        children.iter().map(|child| child.handle).filter(|handle| other.contains(handle)).collect()
    };
    if survivors(old, &new_handles) != survivors(new, &old_handles) {
        return proto::ElementChildrenChanged {
            removed: old.iter().map(|child| index_to_handle(child.handle)).collect(),
            inserted: vec![proto::InsertedChildren {
                after: None,
                children: new.iter().map(PushedChild::to_proto).collect(),
            }],
            updated: Vec::new(),
        };
    }

    let removed = old
        .iter()
        .filter(|child| !new_handles.contains(&child.handle))
        .map(|child| index_to_handle(child.handle))
        .collect();
    let mut inserted: Vec<proto::InsertedChildren> = Vec::new();
    for (index, child) in new.iter().enumerate() {
        if old_handles.contains(&child.handle) {
            continue;
        }
        let previous = index.checked_sub(1).map(|previous| new[previous].handle);
        match inserted.last_mut() {
            Some(run) if previous.is_some_and(|previous| !old_handles.contains(&previous)) => {
                run.children.push(child.to_proto());
            }
            _ => inserted.push(proto::InsertedChildren {
                after: previous.map(index_to_handle),
                children: vec![child.to_proto()],
            }),
        }
    }
    let old_has_children: HashMap<ArenaIndex, bool> =
        old.iter().map(|child| (child.handle, child.has_children)).collect();
    let updated = new
        .iter()
        .filter(|child| {
            old_has_children.get(&child.handle).is_some_and(|had| *had != child.has_children)
        })
        .map(PushedChild::to_proto)
        .collect();
    proto::ElementChildrenChanged { removed, inserted, updated }
}

/// Lists the children of `parent`, or the root elements of every window.
/// With a `sink`, it also subscribes to them, unless the response is truncated,
/// and ends the previous subscription to them.
pub(crate) fn element_children(
    state: &Rc<IntrospectionState>,
    parent_index: Option<ArenaIndex>,
    sink: Option<&NotificationSink>,
) -> Result<proto::GetElementChildrenResponse, String> {
    const REQUEST: &str = "get_element_children";
    state.ensure_windows_instantiated();
    let parent = parent_index.map(|parent| state.element(REQUEST, parent)).transpose()?;
    let Some(sink) = sink else {
        let children = children(state, parent.as_ref()).ok_or_else(|| destroyed(REQUEST))?;
        return Ok(response(state, &children).0);
    };

    state.subscriptions.remove_children_of(parent_index);
    let subscription_id = state.subscriptions.next_id();
    let subscription = Rc::new(ChildrenSubscription {
        id: subscription_id,
        state: Rc::downgrade(state),
        parent: parent.map(|parent| state.pin_element(parent).1),
        sink: sink.clone(),
        pushed: Default::default(),
    });
    let tracker = ChangeTracker::default();
    let children = init_tracker(
        &tracker,
        subscription.clone(),
        |subscription| subscription.children(),
        |subscription, children| subscription.push_changes(children.as_ref()),
    );
    let children = Option::as_ref(&children).ok_or_else(|| destroyed(REQUEST))?;
    let (mut response, pushed) = response(state, children);
    if !response.truncated {
        response.subscription_id = subscription_id;
        *subscription.pushed.borrow_mut() = pushed;
        state.subscriptions.insert(subscription_id, tracker);
        state.subscriptions.children_of.borrow_mut().insert(parent_index, subscription_id);
    }
    Ok(response)
}

/// Returns the response listing `children`, without a subscription,
/// and the children it lists, pinned.
fn response(
    state: &IntrospectionState,
    children: &Children,
) -> (proto::GetElementChildrenResponse, Vec<PushedChild>) {
    let limit = state.handles_per_response();
    let listed = pin_children(state, &children.children[..children.children.len().min(limit)]);
    let response = proto::GetElementChildrenResponse {
        children: listed.iter().map(PushedChild::to_proto).collect(),
        truncated: children.children.len() > limit,
        debug_info_available: children.debug_info_available,
        subscription_id: 0,
    };
    (response, listed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::{
        ELEMENT_HANDLE_CAP, TrackedElement, handle_to_index, window_adapter_for_test,
    };
    use crate::systest::Disconnected;
    use slint::{Model, VecModel};

    slint::slint! {
        #[edition = "1.20"]
        export component App inherits Window {
            width: 100px;
            height: 100px;
            in property <[int]> model;
            in property <bool> condition;
            @testable in-out property <int> counter: 1;
            @testable in-out property <string> label: "a";
            list := Rectangle {
                for value in model: row := Text { text: value; }
            }
            if condition: conditional := Rectangle {
                @testable in property <int> answer: 42;
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
        pushed: Rc<RefCell<Vec<proto::SubscriptionNotification>>>,
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

        fn element(&self, id: &str) -> ArenaIndex {
            if id == "App::root" {
                return self.state.root_element_handle(self.state.window_handles()[0]).unwrap();
            }
            self.state.ensure_windows_instantiated();
            let element = ElementHandle::find_by_element_id(&self.app, id).next().unwrap();
            self.state.element_to_handle(element)
        }

        fn subscribe_children(&self, id: &str) -> proto::GetElementChildrenResponse {
            element_children(&self.state, Some(self.element(id)), Some(&self.sink)).unwrap()
        }

        /// Runs the change handlers and returns what they pushed.
        fn notifications(&self) -> Vec<Notification> {
            notifications(&self.pushed)
        }

        fn children_changes(&self) -> Vec<proto::ElementChildrenChanged> {
            self.notifications()
                .into_iter()
                .map(|notification| match notification {
                    Notification::ChildrenChanged(changed) => changed,
                    _ => panic!("expected a children change"),
                })
                .collect()
        }
    }

    fn notifications(pushed: &RefCell<Vec<proto::SubscriptionNotification>>) -> Vec<Notification> {
        ChangeTracker::run_change_handlers();
        pushed.take().into_iter().map(|notification| notification.msg.unwrap()).collect()
    }

    fn capture() -> (NotificationSink, Rc<RefCell<Vec<proto::SubscriptionNotification>>>) {
        let pushed = Rc::new(RefCell::new(Vec::new()));
        let sink_pushed = pushed.clone();
        let sink: NotificationSink = Rc::new(move |encoded| {
            let notification = proto::SubscriptionNotification::decode(&encoded[..]).unwrap();
            sink_pushed.borrow_mut().push(notification);
            Ok(())
        });
        (sink, pushed)
    }

    fn ids(children: &[proto::ElementChild]) -> Vec<&str> {
        children.iter().map(|child| child.type_names_and_ids[0].id.as_str()).collect()
    }

    fn is_pinned(state: &IntrospectionState, handle: &proto::Handle) -> bool {
        let index = handle_to_index(*handle).unwrap();
        state.element_handles.borrow().get(index).is_some_and(TrackedElement::is_pinned)
    }

    fn assert_ended(notifications: &[Notification]) {
        assert!(matches!(notifications, [Notification::Ended(_)]), "got: {notifications:?}");
    }

    #[test]
    fn unsubscribing_an_ended_subscription_succeeds() {
        let fixture = Fixture::new();
        let id = fixture.subscribe_children("App::root").subscription_id;
        fixture.state.subscriptions.remove(id);
        fixture.state.subscriptions.remove(id);
        fixture.state.subscriptions.remove(42);
    }

    #[test]
    fn subscribing_to_a_stale_handle_errors() {
        let fixture = Fixture::new();
        let stale = ArenaIndex::default();
        let err = subscribe_element_properties(&fixture.state, stale, &fixture.sink).unwrap_err();
        assert!(err.contains("Invalid element handle"), "got: {err}");
        let err = element_children(&fixture.state, Some(stale), Some(&fixture.sink)).unwrap_err();
        assert!(err.contains("Invalid element handle"), "got: {err}");
        assert!(fixture.state.subscriptions.trackers.borrow().is_empty());
    }

    #[test]
    fn property_changes_push_only_the_changed_properties() {
        let fixture = Fixture::new();
        let root = fixture.element("App::root");
        let initial = subscribe_element_properties(&fixture.state, root, &fixture.sink).unwrap();
        assert!(initial.available);
        let values: Vec<_> = initial
            .properties
            .iter()
            .map(|property| (property.name.as_str(), property.value.as_deref()))
            .collect();
        assert_eq!(values, [("counter", Some("1")), ("label", Some("a"))]);

        fixture.app.set_counter(2);
        let [Notification::PropertiesChanged(changed)] =
            <[_; 1]>::try_from(fixture.notifications()).unwrap()
        else {
            panic!("expected a property change");
        };
        let [property] = <[_; 1]>::try_from(changed.changed).unwrap();
        assert_eq!((property.name.as_str(), property.value.as_deref()), ("counter", Some("2")));

        fixture.app.set_counter(2);
        assert!(fixture.notifications().is_empty());
    }

    #[test]
    fn destroying_the_element_ends_its_property_subscription() {
        let fixture = Fixture::new();
        fixture.app.set_condition(true);
        let conditional = fixture.element("App::conditional");
        let id = subscribe_element_properties(&fixture.state, conditional, &fixture.sink)
            .unwrap()
            .subscription_id;

        fixture.app.set_condition(false);
        assert_ended(&fixture.notifications());
        assert!(!fixture.state.subscriptions.trackers.borrow().contains_key(&id));
    }

    #[test]
    fn dropping_a_component_pushes_nothing_until_a_window_shows() {
        let fixture = Fixture::new();
        let root = fixture.element("App::root");
        subscribe_element_properties(&fixture.state, root, &fixture.sink).unwrap();
        fixture.subscribe_children("App::list");
        let roots = element_children(&fixture.state, None, Some(&fixture.sink)).unwrap();

        drop(fixture.app);
        assert!(notifications(&fixture.pushed).is_empty());

        let next = App::new().unwrap();
        fixture.state.add_window(&window_adapter_for_test(&next));
        ChangeTracker::run_change_handlers();
        let (roots_changed, ended): (Vec<_>, Vec<_>) = fixture
            .pushed
            .take()
            .into_iter()
            .partition(|notification| notification.subscription_id == roots.subscription_id);
        assert!(
            ended
                .iter()
                .all(|notification| matches!(notification.msg, Some(Notification::Ended(_))))
                && ended.len() == 2,
            "got: {ended:?}"
        );
        let [roots_changed] = <[_; 1]>::try_from(roots_changed).unwrap();
        let Some(Notification::ChildrenChanged(changed)) = roots_changed.msg else {
            panic!("expected a children change, got: {roots_changed:?}");
        };
        assert_eq!(changed.removed, [roots.children[0].handle.unwrap()]);
        let trackers = fixture.state.subscriptions.trackers.borrow();
        assert_eq!(trackers.keys().collect::<Vec<_>>(), [&roots.subscription_id]);
    }

    #[test]
    fn model_rows_push_inserted_and_removed_runs() {
        let fixture = Fixture::new();
        let initial = fixture.subscribe_children("App::list");
        assert_eq!(ids(&initial.children), ["App::row", "App::row"]);
        let rows: Vec<_> = initial.children.iter().map(|child| child.handle.unwrap()).collect();

        fixture.rows.push(3);
        fixture.rows.push(4);
        let [changed] = fixture.children_changes().try_into().unwrap();
        assert!(changed.removed.is_empty());
        let [run] = <[_; 1]>::try_from(changed.inserted).unwrap();
        assert_eq!(run.after, Some(rows[1]));
        assert_eq!(ids(&run.children), ["App::row", "App::row"]);

        fixture.rows.remove(0);
        let [changed] = fixture.children_changes().try_into().unwrap();
        assert_eq!(changed.removed, [rows[0]]);
        assert!(changed.inserted.is_empty());
    }

    #[test]
    fn changing_row_data_pushes_nothing() {
        let fixture = Fixture::new();
        fixture.subscribe_children("App::list");
        fixture.rows.set_row_data(0, 10);
        assert!(fixture.children_changes().is_empty());
        assert_eq!(fixture.rows.row_count(), 2);
    }

    #[test]
    fn toggling_a_condition_pushes_the_child() {
        let fixture = Fixture::new();
        let initial = fixture.subscribe_children("App::root");
        assert_eq!(ids(&initial.children), ["App::list"]);

        fixture.app.set_condition(true);
        let [changed] = fixture.children_changes().try_into().unwrap();
        assert!(changed.removed.is_empty());
        let [run] = <[_; 1]>::try_from(changed.inserted).unwrap();
        assert_eq!(run.after, initial.children[0].handle);
        assert_eq!(ids(&run.children), ["App::conditional"]);
        assert!(run.children[0].has_children);
        let conditional = run.children[0].handle.unwrap();
        assert!(is_pinned(&fixture.state, &conditional));

        fixture.app.set_condition(false);
        let [changed] = fixture.children_changes().try_into().unwrap();
        assert_eq!(changed.removed, [conditional]);
        assert!(changed.inserted.is_empty());
        assert!(!is_pinned(&fixture.state, &conditional));
    }

    #[test]
    fn a_child_losing_its_children_pushes_an_update() {
        let fixture = Fixture::new();
        let initial = fixture.subscribe_children("App::root");
        assert!(initial.children[0].has_children);

        fixture.rows.set_vec(Vec::new());
        let [changed] = fixture.children_changes().try_into().unwrap();
        assert!(changed.removed.is_empty() && changed.inserted.is_empty());
        let [updated] = <[_; 1]>::try_from(changed.updated).unwrap();
        assert_eq!(updated.handle, initial.children[0].handle);
        assert!(!updated.has_children);
    }

    #[test]
    fn destroying_the_parent_ends_its_children_subscription() {
        let fixture = Fixture::new();
        fixture.app.set_condition(true);
        let id = fixture.subscribe_children("App::conditional").subscription_id;

        fixture.app.set_condition(false);
        assert_ended(&fixture.notifications());
        assert!(!fixture.state.subscriptions.trackers.borrow().contains_key(&id));
    }

    #[test]
    fn opening_and_closing_a_popup_pushes_it_under_its_parent() {
        let fixture = Fixture::new();
        fixture.subscribe_children("App::root");

        fixture.app.invoke_show_popup();
        let [changed] = fixture.children_changes().try_into().unwrap();
        let [run] = <[_; 1]>::try_from(changed.inserted).unwrap();
        assert_eq!(ids(&run.children), ["App::popup"]);
        let popup = run.children[0].handle.unwrap();

        fixture.app.invoke_close_popup();
        let [changed] = fixture.children_changes().try_into().unwrap();
        assert_eq!(changed.removed, [popup]);
        assert!(changed.inserted.is_empty());
    }

    #[test]
    fn subscription_without_parent_pushes_a_window_shown_later() {
        crate::init_no_event_loop();
        let state = Rc::new(IntrospectionState::new());
        let (sink, pushed) = capture();
        let initial = element_children(&state, None, Some(&sink)).unwrap();
        assert!(initial.children.is_empty());

        let app = App::new().unwrap();
        state.add_window(&window_adapter_for_test(&app));
        let [Notification::ChildrenChanged(changed)] =
            <[_; 1]>::try_from(notifications(&pushed)).unwrap()
        else {
            panic!("expected a children change");
        };
        let [run] = <[_; 1]>::try_from(changed.inserted).unwrap();
        assert_eq!(run.after, None);
        assert_eq!(ids(&run.children), ["App::root"]);
    }

    #[test]
    fn pinned_handles_survive_eviction() {
        let fixture = Fixture::new();
        let initial = fixture.subscribe_children("App::list");
        let others: Vec<_> = (0..ELEMENT_HANDLE_CAP / 100 + 10)
            .map(|_| {
                let app = super::super::hundred_rectangles::App::new().unwrap();
                super::super::mention_window(&fixture.state, &app);
                app
            })
            .collect();
        for child in &initial.children {
            let handle = handle_to_index(child.handle.unwrap()).unwrap();
            fixture.state.element("test", handle).unwrap();
        }
        assert!(fixture.state.element_handles.borrow().len() <= ELEMENT_HANDLE_CAP + 2);
        drop(others);
    }

    #[test]
    fn subscriptions_pin_the_observed_element() {
        let fixture = Fixture::new();
        let list = fixture.element("App::list");
        let root = fixture.element("App::root");
        subscribe_element_properties(&fixture.state, root, &fixture.sink).unwrap();
        fixture.subscribe_children("App::list");
        let arena = fixture.state.element_handles.borrow();
        assert!(arena[list].is_pinned());
        assert!(arena[root].is_pinned());
    }

    #[test]
    fn unsubscribing_unpins_the_children() {
        let fixture = Fixture::new();
        let initial = fixture.subscribe_children("App::list");
        assert!(
            initial.children.iter().all(|child| is_pinned(&fixture.state, &child.handle.unwrap()))
        );

        fixture.state.subscriptions.remove(initial.subscription_id);
        assert!(!fixture.state.element_handles.borrow().values().any(TrackedElement::is_pinned));
        assert_eq!(fixture.state.pinned_count.get(), 0);

        fixture.rows.push(3);
        assert!(fixture.notifications().is_empty());
    }

    #[test]
    fn subscribing_to_the_same_children_again_replaces_the_subscription() {
        let fixture = Fixture::new();
        let initial = fixture.subscribe_children("App::list");
        let again = fixture.subscribe_children("App::list");
        assert_ne!(initial.subscription_id, again.subscription_id);
        let trackers = fixture.state.subscriptions.trackers.borrow();
        assert_eq!(trackers.keys().collect::<Vec<_>>(), [&again.subscription_id]);
        drop(trackers);
        assert!(
            again.children.iter().all(|child| is_pinned(&fixture.state, &child.handle.unwrap()))
        );

        fixture.rows.push(3);
        ChangeTracker::run_change_handlers();
        let ids: Vec<_> = fixture.pushed.take().iter().map(|n| n.subscription_id).collect();
        assert_eq!(ids, [again.subscription_id]);
    }

    #[test]
    fn a_failing_sink_ends_the_subscription() {
        let fixture = Fixture::new();
        let sink: NotificationSink = Rc::new(|_| Err(Disconnected));
        let list = fixture.element("App::list");
        element_children(&fixture.state, Some(list), Some(&sink)).unwrap();
        let root = fixture.element("App::root");
        subscribe_element_properties(&fixture.state, root, &sink).unwrap();

        fixture.rows.push(3);
        fixture.app.set_counter(2);
        ChangeTracker::run_change_handlers();
        assert!(fixture.state.subscriptions.trackers.borrow().is_empty());
        assert!(fixture.state.subscriptions.children_of.borrow().is_empty());
        assert_eq!(fixture.state.pinned_count.get(), 0);
    }

    slint::slint! {
        #[edition = "1.20"]
        export component Large inherits Window {
            in property <int> count;
            for _ in count: Rectangle { }
        }
    }

    /// More children than one response can hold.
    const LARGE_COUNT: i32 = 9000;

    fn large(count: i32) -> (Large, Rc<IntrospectionState>, ArenaIndex) {
        crate::init_no_event_loop();
        let state = Rc::new(IntrospectionState::new());
        let app = Large::new().unwrap();
        app.set_count(count);
        state.add_window(&window_adapter_for_test(&app));
        assert!(LARGE_COUNT as usize > state.handles_per_response());
        let root = state.root_element_handle(state.window_handles()[0]).unwrap();
        (app, state, root)
    }

    #[test]
    fn a_truncated_response_subscribes_to_nothing() {
        let (_app, state, root) = large(LARGE_COUNT);
        let (sink, _) = capture();
        let response = element_children(&state, Some(root), Some(&sink)).unwrap();
        assert!(response.truncated);
        assert_eq!(response.subscription_id, 0);
        assert!(state.subscriptions.trackers.borrow().is_empty());
        assert!(!state.element_handles.borrow().values().any(TrackedElement::is_pinned));
    }

    #[test]
    fn growing_more_children_than_a_response_holds_ends_the_subscription() {
        let (app, state, root) = large(1);
        let (sink, pushed) = capture();
        element_children(&state, Some(root), Some(&sink)).unwrap();
        app.set_count(LARGE_COUNT);
        assert_ended(&notifications(&pushed));
        assert!(state.subscriptions.trackers.borrow().is_empty());
        assert!(!state.element_handles.borrow().values().any(TrackedElement::is_pinned));
    }

    #[test]
    fn diff_replaces_all_children_when_survivors_reorder() {
        crate::init_no_event_loop();
        let app = App::new().unwrap();
        let state = IntrospectionState::new();
        let element = ElementHandle::find_by_element_id(&app, "App::list").next().unwrap();
        let element = state.pin_element(element).1;
        let mut arena = slotmap::SlotMap::<ArenaIndex, ()>::with_key();
        let child = |handle| PushedChild { handle, element: element.clone(), has_children: false };
        let [a, b] = std::array::from_fn(|_| arena.insert(()));
        let changed = diff_children(&[child(a), child(b)], &[child(b), child(a)]);
        assert_eq!(changed.removed, [index_to_handle(a), index_to_handle(b)]);
        let [run] = <[_; 1]>::try_from(changed.inserted).unwrap();
        assert_eq!(run.after, None);
        assert_eq!(run.children.len(), 2);
    }
}
