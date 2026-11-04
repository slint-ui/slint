// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Weak;

use crate::protocol::{PreviewToDebuggerMessage, PreviewToLsp};

/// Answers a [`crate::protocol::DebuggerToPreviewMessage::SystemTestRequest`]
/// with a [`PreviewToDebuggerMessage::SystemTestResponse`] through `to_editor`.
/// The subscriptions the request creates push their changes through `to_editor` too.
/// Call it on the thread that runs the Slint event loop.
pub fn answer_request(id: u64, request: Vec<u8>, to_editor: Weak<dyn PreviewToLsp>) {
    if let Err(error) = spawn_answer(id, request, to_editor.clone()) {
        send(&to_editor, id, Err(error));
    }
}

#[cfg(feature = "system-testing")]
fn spawn_answer(
    id: u64,
    request: Vec<u8>,
    to_editor: Weak<dyn PreviewToLsp>,
) -> Result<(), String> {
    i_slint_backend_selector::with_global_context(|context| {
        context.spawn_local(async move {
            let sink = notification_sink(to_editor.clone());
            let response =
                i_slint_backend_testing::systest::handle_encoded_request(&request, sink).await;
            send(&to_editor, id, Ok(response));
        })
    })
    .map_err(|error| error.to_string())?
    .map(drop)
    .map_err(|error| error.to_string())
}

#[cfg(feature = "system-testing")]
fn notification_sink(
    to_editor: Weak<dyn PreviewToLsp>,
) -> i_slint_backend_testing::systest::NotificationSink {
    use i_slint_backend_testing::systest::SubscriptionNotification;
    std::rc::Rc::new(move |notification| {
        let Some(to_editor) = to_editor.upgrade() else { return };
        let message = match notification {
            SubscriptionNotification::PropertiesChanged { subscription, changed } => {
                PreviewToDebuggerMessage::PropertiesChanged { subscription, changed }
            }
            SubscriptionNotification::ElementTreeChanged { subscription, changed } => {
                PreviewToDebuggerMessage::ElementTreeChanged { subscription, changed }
            }
            SubscriptionNotification::Ended { subscription } => {
                PreviewToDebuggerMessage::SubscriptionsEnded { subscriptions: vec![subscription] }
            }
        };
        to_editor.send_to_debugger(&message).ok();
    })
}

/// Ends the subscriptions of the debugger, and tells it so through `to_editor`.
/// Call it when the preview replaces or tears down the instance they observe.
pub fn clear_subscriptions(to_editor: &Weak<dyn PreviewToLsp>) {
    #[cfg(feature = "system-testing")]
    {
        let subscriptions = i_slint_backend_testing::systest::clear_subscriptions();
        if !subscriptions.is_empty()
            && let Some(to_editor) = to_editor.upgrade()
        {
            to_editor
                .send_to_debugger(&PreviewToDebuggerMessage::SubscriptionsEnded { subscriptions })
                .ok();
        }
    }
    #[cfg(not(feature = "system-testing"))]
    let _ = to_editor;
}

#[cfg(not(feature = "system-testing"))]
fn spawn_answer(
    _id: u64,
    _request: Vec<u8>,
    _to_editor: Weak<dyn PreviewToLsp>,
) -> Result<(), String> {
    Err("This preview was built without the 'system-testing' feature".into())
}

fn send(to_editor: &Weak<dyn PreviewToLsp>, id: u64, response: Result<Vec<u8>, String>) {
    let Some(to_editor) = to_editor.upgrade() else { return };
    to_editor.send_to_debugger(&PreviewToDebuggerMessage::SystemTestResponse { id, response }).ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct CaptureToDebugger(RefCell<Vec<PreviewToDebuggerMessage>>);

    impl PreviewToLsp for CaptureToDebugger {
        fn send(&self, _: &crate::protocol::PreviewToLspMessage) -> crate::protocol::Result<()> {
            unreachable!("System-test responses go to the debugger")
        }

        fn send_to_debugger(
            &self,
            message: &PreviewToDebuggerMessage,
        ) -> crate::protocol::Result<()> {
            self.0.borrow_mut().push(message.clone());
            Ok(())
        }
    }

    fn answer(request: Vec<u8>, run: impl FnOnce()) -> Vec<PreviewToDebuggerMessage> {
        let capture = Rc::new(CaptureToDebugger::default());
        let to_editor: Rc<dyn PreviewToLsp> = capture.clone();
        answer_request(7, request, Rc::downgrade(&to_editor));
        run();
        capture.0.take()
    }

    #[cfg(not(feature = "system-testing"))]
    #[test]
    fn answers_with_error_when_disabled() {
        assert!(matches!(
            answer(vec![], || {}).as_slice(),
            [PreviewToDebuggerMessage::SystemTestResponse { id: 7, response: Err(_) }]
        ));
    }

    #[cfg(all(feature = "system-testing", feature = "preview-session"))]
    #[test]
    fn answers_on_the_event_loop() {
        i_slint_backend_testing::init_integration_test_with_system_time();
        let messages = answer(vec![0xff], || {
            i_slint_backend_selector::with_global_context(|context| {
                context
                    .spawn_local(async { i_slint_core::api::quit_event_loop().unwrap() })
                    .unwrap();
                context.run_event_loop().unwrap();
            })
            .unwrap();
        });
        assert!(matches!(
            messages.as_slice(),
            [PreviewToDebuggerMessage::SystemTestResponse { id: 7, response: Ok(response) }]
                if !response.is_empty()
        ));
    }

    #[cfg(all(feature = "system-testing", feature = "preview-session"))]
    mod subscriptions {
        use super::*;
        use i_slint_backend_testing::proto::{
            self, aut_response::Msg as Resp, request_to_aut::Msg as Req,
        };
        use prost::Message;
        use slint_interpreter::{ComponentHandle, ComponentInstance, Value};

        const APP: &str = r#"
            export component App inherits Window {
                in-out property <int> value: 1;
                in-out property <int> base: 2;
                out property <int> derived: base * 10;
                label := Text { text: value; }
            }
        "#;

        /// More elements than one response can hold.
        const LARGE: &str = r#"
            export component App inherits Window {
                for _ in 9000: Rectangle { }
            }
        "#;

        struct Debugger {
            capture: Rc<CaptureToDebugger>,
            instances: RefCell<Vec<ComponentInstance>>,
        }

        impl Debugger {
            fn new() -> Self {
                i_slint_backend_testing::init_no_event_loop();
                i_slint_backend_testing::systest::init().unwrap();
                Self { capture: Default::default(), instances: Default::default() }
            }

            fn request(&self, msg: Req) -> Resp {
                let request = proto::RequestToAut { msg: Some(msg) }.encode_to_vec();
                let to_editor = Rc::downgrade(&self.capture);
                let sink = notification_sink(to_editor);
                let response = block_on(i_slint_backend_testing::systest::handle_encoded_request(
                    &request, sink,
                ));
                proto::AutResponse::decode(&response[..]).unwrap().msg.unwrap()
            }

            fn windows(&self) -> Vec<proto::Handle> {
                let Resp::WindowList(list) =
                    self.request(Req::RequestWindowList(proto::RequestWindowListMessage {}))
                else {
                    panic!("expected a window list");
                };
                list.window_handles
            }

            /// Shows `source` and returns the instance and the handle of its window.
            fn show(&self, source: &str) -> (ComponentInstance, proto::Handle) {
                let result = block_on(
                    slint_interpreter::Compiler::default()
                        .build_from_source(source.into(), "/app.slint".into()),
                );
                let instance = result.component("App").unwrap().create().unwrap();
                let before = self.windows();
                instance.show().unwrap();
                let window = self.windows().into_iter().find(|w| !before.contains(w)).unwrap();
                self.instances.borrow_mut().push(instance.clone_strong());
                (instance, window)
            }

            fn root(&self, window: proto::Handle) -> proto::Handle {
                let Resp::WindowProperties(properties) =
                    self.request(Req::RequestWindowProperties(proto::RequestWindowProperties {
                        window_handle: Some(window),
                    }))
                else {
                    panic!("expected window properties");
                };
                properties.root_element_handle.unwrap()
            }

            fn find(&self, window: proto::Handle, id: &str) -> Vec<proto::Handle> {
                let Resp::Elements(elements) =
                    self.request(Req::RequestFindElementsById(proto::RequestFindElementsById {
                        window_handle: Some(window),
                        elements_id: id.into(),
                    }))
                else {
                    panic!("expected elements");
                };
                elements.element_handles
            }

            fn label(&self, window: proto::Handle) -> proto::Handle {
                self.find(window, "App::label")[0]
            }

            fn subscribe(&self, element: proto::Handle) -> u64 {
                let Resp::SubscribeElementPropertiesResponse(response) =
                    self.request(Req::RequestSubscribeElementProperties(
                        proto::RequestSubscribeElementProperties { element_handle: Some(element) },
                    ))
                else {
                    panic!("expected a subscription");
                };
                assert!(response.available);
                response.subscription_id
            }

            fn is_valid(&self, element: proto::Handle) -> bool {
                !matches!(
                    self.request(Req::RequestElementInspectableProperties(
                        proto::RequestElementInspectableProperties {
                            element_handle: Some(element),
                        },
                    )),
                    Resp::Error(_)
                )
            }

            /// Mints more handles than the arena keeps, which evicts every older unpinned one.
            fn evict(&self) {
                for _ in 0..2 {
                    let (_, window) = self.show(LARGE);
                    let root = self.root(window);
                    self.request(Req::RequestGetElementTree(proto::RequestGetElementTree {
                        element_handle: Some(root),
                        max_elements: 0,
                    }));
                }
            }

            /// Runs the change handlers and returns the property changes they pushed,
            /// as subscription, name and value.
            fn pushes(&self) -> Vec<(u64, Vec<(String, String)>)> {
                i_slint_core::properties::ChangeTracker::run_change_handlers();
                self.capture
                    .0
                    .take()
                    .into_iter()
                    .map(|message| {
                        let PreviewToDebuggerMessage::PropertiesChanged { subscription, changed } =
                            message
                        else {
                            panic!("unexpected message {message:?}");
                        };
                        let changed = proto::ElementPropertiesChanged::decode(&changed[..])
                            .unwrap()
                            .changed
                            .into_iter()
                            .map(|property| (property.name, property.value.unwrap_or_default()))
                            .collect();
                        (subscription, changed)
                    })
                    .collect()
            }
        }

        fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
            tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(future)
        }

        fn changed(name: &str, value: &str) -> (String, String) {
            (name.into(), value.into())
        }

        #[test]
        fn setting_a_property_pushes_it_once_the_change_handlers_run() {
            let debugger = Debugger::new();
            let (app, window) = debugger.show(APP);
            let subscription = debugger.subscribe(debugger.root(window));
            app.set_property("value", Value::Number(5.)).unwrap();
            assert!(debugger.capture.0.borrow().is_empty());
            assert_eq!(debugger.pushes(), [(subscription, vec![changed("value", "5")])]);
            assert!(debugger.pushes().is_empty());
        }

        #[test]
        fn several_writes_push_the_final_value_once() {
            let debugger = Debugger::new();
            let (app, window) = debugger.show(APP);
            let subscription = debugger.subscribe(debugger.root(window));
            for value in [5., 6., 7.] {
                app.set_property("value", Value::Number(value)).unwrap();
            }
            assert_eq!(debugger.pushes(), [(subscription, vec![changed("value", "7")])]);
        }

        #[test]
        fn restoring_the_previous_value_pushes_nothing() {
            let debugger = Debugger::new();
            let (app, window) = debugger.show(APP);
            debugger.subscribe(debugger.root(window));
            app.set_property("value", Value::Number(5.)).unwrap();
            app.set_property("value", Value::Number(1.)).unwrap();
            assert!(debugger.pushes().is_empty());
        }

        #[test]
        fn changing_a_dependency_pushes_the_binding() {
            let debugger = Debugger::new();
            let (app, window) = debugger.show(APP);
            let subscription = debugger.subscribe(debugger.root(window));
            app.set_property("base", Value::Number(3.)).unwrap();
            assert_eq!(
                debugger.pushes(),
                [(subscription, vec![changed("base", "3"), changed("derived", "30")])]
            );
        }

        #[test]
        fn unsubscribing_stops_the_pushes() {
            let debugger = Debugger::new();
            let (app, window) = debugger.show(APP);
            let subscription_id = debugger.subscribe(debugger.root(window));
            assert!(matches!(
                debugger.request(Req::RequestUnsubscribe(proto::RequestUnsubscribe {
                    subscription_id
                })),
                Resp::UnsubscribeResponse(_)
            ));
            app.set_property("value", Value::Number(5.)).unwrap();
            assert!(debugger.pushes().is_empty());
        }

        #[test]
        fn evicting_the_handle_keeps_the_subscription() {
            let debugger = Debugger::new();
            let (app, window) = debugger.show(APP);
            let label = debugger.label(window);
            let subscription = debugger.subscribe(label);
            debugger.evict();
            assert!(!debugger.is_valid(label));
            app.set_property("value", Value::Number(5.)).unwrap();
            let [(pushed_to, pushed)] = debugger.pushes().try_into().unwrap();
            assert_eq!(pushed_to, subscription);
            assert!(pushed.contains(&changed("text", "5")), "{pushed:?}");
        }

        #[test]
        fn destroying_the_element_ends_its_property_subscription() {
            let debugger = Debugger::new();
            let (app, window) = debugger.show(
                r#"
                export component App inherits Window {
                    in property <[int]> model;
                    in property <bool> condition: true;
                    for value in model: row := Text { text: value; }
                    if condition: conditional := Text { }
                }
            "#,
            );
            let rows = Rc::new(i_slint_core::model::VecModel::from(vec![
                Value::Number(1.),
                Value::Number(2.),
            ]));
            app.set_property("model", Value::Model(rows.clone().into())).unwrap();
            let [first_row, second_row] = debugger.find(window, "App::row").try_into().unwrap();
            let first_row = debugger.subscribe(first_row);
            let second_row = debugger.subscribe(second_row);
            let [conditional] = debugger.find(window, "App::conditional").try_into().unwrap();
            let conditional = debugger.subscribe(conditional);

            rows.remove(1);
            i_slint_core::properties::ChangeTracker::run_change_handlers();
            assert_eq!(
                debugger.capture.0.take(),
                [PreviewToDebuggerMessage::SubscriptionsEnded { subscriptions: vec![second_row] }]
            );

            app.set_property("condition", Value::Bool(false)).unwrap();
            i_slint_core::properties::ChangeTracker::run_change_handlers();
            assert_eq!(
                debugger.capture.0.take(),
                [PreviewToDebuggerMessage::SubscriptionsEnded { subscriptions: vec![conditional] }]
            );

            let to_editor: Weak<dyn PreviewToLsp> = Rc::downgrade(&debugger.capture) as _;
            clear_subscriptions(&to_editor);
            assert_eq!(
                debugger.capture.0.take(),
                [PreviewToDebuggerMessage::SubscriptionsEnded { subscriptions: vec![first_row] }]
            );
        }

        #[test]
        fn toggling_a_condition_pushes_the_tree_change() {
            let debugger = Debugger::new();
            let (app, window) = debugger.show(
                r#"
                export component App inherits Window {
                    in property <bool> condition;
                    if condition: conditional := Rectangle { }
                }
            "#,
            );
            let Resp::SubscribeElementTreeResponse(response) = debugger.request(
                Req::RequestSubscribeElementTree(proto::RequestSubscribeElementTree {
                    element_handle: Some(debugger.root(window)),
                }),
            ) else {
                panic!("expected a tree subscription");
            };

            app.set_property("condition", Value::Bool(true)).unwrap();
            i_slint_core::properties::ChangeTracker::run_change_handlers();
            let messages = debugger.capture.0.take();
            let [PreviewToDebuggerMessage::ElementTreeChanged { subscription, changed }] =
                messages.as_slice()
            else {
                panic!("expected a tree change, got {messages:?}");
            };
            assert_eq!(*subscription, response.subscription_id);
            let changed = proto::ElementTreeChanged::decode(&changed[..]).unwrap();
            assert!(changed.removed.is_empty());
            let [inserted] = changed.inserted.try_into().unwrap();
            let [node] = inserted.elements.try_into().unwrap();
            assert!(node.type_names_and_ids.iter().any(|t| t.id == "App::conditional"));
        }

        /// The live preview shows each recompiled instance in the window of the one it replaces.
        #[test]
        fn the_tree_of_every_window_follows_a_replaced_instance() {
            let debugger = Debugger::new();
            let (old, window) = debugger.show(APP);
            let result = block_on(
                slint_interpreter::Compiler::default()
                    .build_from_source(APP.replace("label :=", "renamed :="), "/app.slint".into()),
            );
            let new =
                result.component("App").unwrap().create_with_existing_window(old.window()).unwrap();
            new.show().unwrap();
            debugger.instances.borrow_mut().clear();
            drop(old);

            assert_eq!(debugger.windows(), [window]);
            let Resp::GetElementTreeResponse(tree) =
                debugger.request(Req::RequestGetElementTree(proto::RequestGetElementTree {
                    element_handle: None,
                    max_elements: 0,
                }))
            else {
                panic!("expected an element tree");
            };
            let ids: Vec<_> = tree
                .elements
                .iter()
                .flat_map(|node| &node.type_names_and_ids)
                .map(|t| t.id.as_str())
                .collect();
            assert!(ids.contains(&"App::renamed") && !ids.contains(&"App::label"), "{ids:?}");
        }

        #[test]
        fn clearing_drops_every_subscription_and_unpins_its_handles() {
            let debugger = Debugger::new();
            let (app, window) = debugger.show(APP);
            let properties = debugger.subscribe(debugger.root(window));
            let label = debugger.label(window);
            let Resp::SubscribeElementTreeResponse(tree) =
                debugger.request(Req::RequestSubscribeElementTree(
                    proto::RequestSubscribeElementTree { element_handle: Some(label) },
                ))
            else {
                panic!("expected a tree subscription");
            };

            let to_editor: Weak<dyn PreviewToLsp> = Rc::downgrade(&debugger.capture) as _;
            clear_subscriptions(&to_editor);
            assert_eq!(
                debugger.capture.0.take(),
                [PreviewToDebuggerMessage::SubscriptionsEnded {
                    subscriptions: vec![properties, tree.subscription_id]
                }]
            );
            clear_subscriptions(&to_editor);
            assert!(debugger.capture.0.take().is_empty());
            app.set_property("value", Value::Number(5.)).unwrap();
            assert!(debugger.pushes().is_empty());
            debugger.evict();
            assert!(!debugger.is_valid(label));
        }
    }
}
