// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore Nagle
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use futures_lite::AsyncReadExt;
use futures_lite::AsyncWriteExt;
use i_slint_core::api::EventLoopError;
use i_slint_core::debug_log;
use prost::Message;
use std::io::Cursor;
use std::rc::Rc;

use crate::introspection::{self, IntrospectionState, dispatch, proto};
use introspection::handle_to_index;

struct TestingClient {
    state: Rc<IntrospectionState>,
    message_loop_future: std::cell::OnceCell<i_slint_core::future::JoinHandle<()>>,
    server_addr: String,
}

impl TestingClient {
    fn new(state: Rc<IntrospectionState>) -> Option<Rc<Self>> {
        let Ok(server_addr) = std::env::var("SLINT_TEST_SERVER") else {
            return None;
        };

        Some(Rc::new(Self { state, message_loop_future: Default::default(), server_addr }))
    }

    fn start_if_needed(self: &Rc<Self>) {
        let this = self.clone();
        self.message_loop_future.get_or_init(|| {
            i_slint_core::with_existing_context(|context| {
                let this = this.clone();
                context
                    .spawn_local(async move {
                        message_loop(&this.server_addr, |request| {
                            let this = this.clone();
                            Box::pin(async move { this.handle_request(request).await })
                        })
                        .await;
                    })
                    .unwrap()
            })
            .expect("uninitialized platform")
        });
    }

    async fn handle_request(
        &self,
        request: Option<proto::request_to_aut::Msg>,
    ) -> Result<proto::aut_response::Msg, String> {
        use proto::aut_response::Msg as Resp;
        use proto::request_to_aut::Msg as Req;

        let request = request.ok_or_else(|| "Empty request".to_string())?;

        Ok(match request {
            Req::RequestWindowList(..) => Resp::WindowList(dispatch::list_windows(&self.state)),
            Req::RequestWindowProperties(proto::RequestWindowProperties { window_handle }) => {
                let window_index = handle_to_index(window_handle.ok_or_else(|| {
                    "window properties request missing window handle".to_string()
                })?)?;
                Resp::WindowProperties(dispatch::window_properties(&self.state, window_index)?)
            }
            Req::RequestFindElementsById(proto::RequestFindElementsById {
                window_handle,
                elements_id,
            }) => {
                let window_index = handle_to_index(window_handle.ok_or_else(|| {
                    "find elements by id request missing window handle".to_string()
                })?)?;
                Resp::Elements(dispatch::find_elements_by_id(
                    &self.state,
                    window_index,
                    &elements_id,
                )?)
            }
            Req::RequestElementProperties(proto::RequestElementProperties { element_handle }) => {
                let element_index = handle_to_index(element_handle.ok_or_else(|| {
                    "element properties request missing element handle".to_string()
                })?)?;
                Resp::ElementProperties(dispatch::element_properties(&self.state, element_index)?)
            }
            Req::RequestInvokeElementAccessibilityAction(
                msg @ proto::RequestInvokeElementAccessibilityAction { element_handle, .. },
            ) => {
                let action =
                    proto::ElementAccessibilityAction::try_from(msg.action).map_err(|_| {
                        format!("invalid ElementAccessibilityAction value: {}", msg.action)
                    })?;
                let element_index = handle_to_index(element_handle.ok_or_else(|| {
                    "invoke element accessibility action request missing element handle".to_string()
                })?)?;
                dispatch::invoke_accessibility_action(&self.state, element_index, action)?;
                Resp::InvokeElementAccessibilityActionResponse(
                    proto::InvokeElementAccessibilityActionResponse {},
                )
            }
            Req::RequestSetElementAccessibleValue(proto::RequestSetElementAccessibleValue {
                element_handle,
                value,
            }) => {
                let element_index = handle_to_index(element_handle.ok_or_else(|| {
                    "set element accessible value request missing element handle".to_string()
                })?)?;
                dispatch::set_accessible_value(&self.state, element_index, value)?;
                Resp::SetElementAccessibleValueResponse(proto::SetElementAccessibleValueResponse {})
            }
            Req::RequestTakeSnapshot(proto::RequestTakeSnapshot {
                window_handle,
                image_mime_type,
            }) => {
                let window_index = handle_to_index(
                    window_handle
                        .ok_or_else(|| "grab window request missing window handle".to_string())?,
                )?;
                Resp::TakeSnapshotResponse(dispatch::take_snapshot(
                    &self.state,
                    window_index,
                    &image_mime_type,
                )?)
            }
            Req::RequestElementClick(proto::RequestElementClick {
                element_handle,
                action,
                button,
                modifiers,
            }) => {
                let element_index =
                    handle_to_index(element_handle.ok_or_else(|| {
                        "element click request missing element handle".to_string()
                    })?)?;
                let button = proto::PointerEventButton::try_from(button)
                    .map_err(|_| format!("invalid PointerEventButton value: {button}"))?;
                let action = proto::ClickAction::try_from(action)
                    .map_err(|_| format!("invalid ClickAction value: {action}"))?;
                dispatch::click(&self.state, element_index, action, button, modifiers).await?;
                Resp::ElementClickResponse(proto::ElementClickResponse {})
            }
            Req::RequestElementDrag(proto::RequestElementDrag {
                element_handle,
                target,
                button,
                modifiers,
            }) => {
                let element_index =
                    handle_to_index(element_handle.ok_or_else(|| {
                        "element drag request missing element handle".to_string()
                    })?)?;
                let button = proto::PointerEventButton::try_from(button)
                    .map_err(|_| format!("invalid PointerEventButton value: {button}"))?;
                let target =
                    target.ok_or_else(|| "element drag request missing target".to_string())?;
                dispatch::drag(&self.state, element_index, target, button, modifiers).await?;
                Resp::ElementDragResponse(proto::ElementDragResponse {})
            }
            Req::RequestDispatchWindowEvent(proto::RequestDispatchWindowEvent {
                window_handle,
                event,
            }) => {
                let window_index = handle_to_index(window_handle.ok_or_else(|| {
                    "window event dispatch request missing window handle".to_string()
                })?)?;
                self.state.dispatch_window_event(
                    window_index,
                    convert_window_event(event.ok_or_else(|| {
                        "window event dispatch request missing event".to_string()
                    })?)?,
                )?;
                Resp::DispatchWindowEventResponse(proto::DispatchWindowEventResponse {})
            }
            Req::RequestQueryElementDescendants(proto::RequestQueryElementDescendants {
                element_handle,
                query_stack,
                find_all,
            }) => {
                let element_index = handle_to_index(element_handle.ok_or_else(|| {
                    "run element query request missing element handle".to_string()
                })?)?;
                Resp::ElementQueryResponse(dispatch::query_element_descendants(
                    &self.state,
                    element_index,
                    query_stack,
                    find_all,
                )?)
            }
            Req::RequestEventLog(proto::RequestEventLog {
                window_handle,
                since_sequence,
                max_events,
                clear_after_read,
            }) => {
                let window_index = window_handle.map(handle_to_index).transpose()?;
                Resp::EventLogResponse(dispatch::event_log(
                    &self.state,
                    window_index,
                    since_sequence,
                    max_events,
                    clear_after_read,
                ))
            }
            Req::RequestClearEventLog(..) => {
                Resp::ClearEventLogResponse(dispatch::clear_event_log(&self.state))
            }
            Req::RequestStartEventRecording(..) => {
                Resp::StartEventRecordingResponse(dispatch::start_event_recording(&self.state))
            }
            Req::RequestStopEventRecording(..) => {
                Resp::StopEventRecordingResponse(dispatch::stop_event_recording(&self.state))
            }
            // MCP-only tools — not supported over the binary system-testing transport
            Req::RequestDispatchKeyEvent(..) | Req::RequestGetElementTree(..) => {
                return Err("this request is only supported via the MCP transport".into());
            }
        })
    }
}

pub fn init() -> Result<(), EventLoopError> {
    introspection::ensure_window_tracking()?;
    let state = introspection::shared_state();

    let Some(client) = TestingClient::new(state) else {
        return Ok(());
    };

    // Chain a hook that starts the TCP message loop on first window shown.
    let previous_hook = i_slint_core::context::set_window_shown_hook(None)
        .map_err(|_| EventLoopError::NoEventLoopProvider)?;
    let previous_hook = std::cell::RefCell::new(previous_hook);

    i_slint_core::context::set_window_shown_hook(Some(Box::new(move |adapter| {
        if let Some(prev) = previous_hook.borrow_mut().as_mut() {
            prev(adapter);
        }
        client.start_if_needed();
    })))
    .map_err(|_| EventLoopError::NoEventLoopProvider)?;

    Ok(())
}

async fn message_loop(
    server_addr: &str,
    mut message_callback: impl FnMut(
        Option<proto::request_to_aut::Msg>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<proto::aut_response::Msg, String>>>,
    >,
) {
    debug_log!("Attempting to connect to testing server at {server_addr}");

    let mut stream = match async_net::TcpStream::connect(server_addr).await {
        Ok(stream) => stream,
        Err(err) => {
            eprintln!("Error connecting to Slint test server at {server_addr}: {}", err);
            return;
        }
    };
    // Attempt to disable the Nagle algorithm to favor faster packet exchange (latency)
    // over throughput.
    stream.set_nodelay(true).ok();
    debug_log!("Connected to test server");

    // Note: Handling communication errors gracefully (without panic) to avoid
    // triggering any crash reporter from the OS.
    let err_msg = loop {
        let mut message_size_buf = vec![0; 4];
        if stream.read_exact(&mut message_size_buf).await.is_err() {
            break "Unable to read request header from AUT connection";
        }

        let message_size: usize =
            Cursor::new(message_size_buf).read_u32::<BigEndian>().unwrap() as usize;
        let mut message_buf = vec![0; message_size];
        if stream.read_exact(&mut message_buf).await.is_err() {
            break "Unable to read request data from AUT connection";
        }

        let message = match proto::RequestToAut::decode(&message_buf[..]) {
            Ok(msg) => msg,
            Err(_) => {
                break "Error de-serializing AUT request message";
            }
        };
        let response = message_callback(message.msg).await.unwrap_or_else(|message| {
            proto::aut_response::Msg::Error(proto::ErrorResponse { message })
        });
        let response = proto::AutResponse { msg: Some(response) };
        let mut binary_message = Vec::new();
        binary_message.write_u32::<BigEndian>(response.encoded_len() as u32).unwrap();
        response.encode(&mut binary_message).unwrap();
        if stream.write_all(&binary_message).await.is_err() {
            break "Unable to write AUT response body";
        }
        // `read_exact` doesn't yield when the next request is already buffered,
        // so the event loop wouldn't run pending timers and change handlers before it.
        futures_lite::future::yield_now().await;
    };
    eprintln!("{}, closing connection to test server", err_msg);

    // Close connection explicitly to notify the server if it is still connected.
    stream.shutdown(std::net::Shutdown::Both).ok();
}

fn convert_logical_position(pos: proto::LogicalPosition) -> i_slint_core::api::LogicalPosition {
    i_slint_core::api::LogicalPosition { x: pos.x, y: pos.y }
}

fn convert_window_event(
    event: proto::WindowEvent,
) -> Result<i_slint_core::platform::WindowEvent, String> {
    use proto::window_event::Event;
    let event = event.event.ok_or_else(|| "empty window event".to_string())?;
    Ok(match event {
        Event::PointerPressed(proto::PointerPressEvent { position, button }) => {
            i_slint_core::platform::WindowEvent::PointerPressed {
                position: convert_logical_position(position.ok_or_else(|| {
                    "Missing logical position in pointer press event".to_string()
                })?),
                button: introspection::convert_pointer_event_button(
                    proto::PointerEventButton::try_from(button)
                        .map_err(|_| format!("invalid PointerEventButton value: {button}"))?,
                ),
            }
        }
        Event::PointerReleased(proto::PointerReleaseEvent { position, button }) => {
            i_slint_core::platform::WindowEvent::PointerReleased {
                position: convert_logical_position(position.ok_or_else(|| {
                    "Missing logical position in pointer release event".to_string()
                })?),
                button: introspection::convert_pointer_event_button(
                    proto::PointerEventButton::try_from(button)
                        .map_err(|_| format!("invalid PointerEventButton value: {button}"))?,
                ),
            }
        }
        Event::PointerMoved(proto::PointerMoveEvent { position }) => {
            i_slint_core::platform::WindowEvent::PointerMoved {
                position: convert_logical_position(
                    position.ok_or_else(|| {
                        "Missing logical position in pointer move event".to_string()
                    })?,
                ),
            }
        }
        Event::PointerScrolled(proto::PointerScrolledEvent { position, delta_x, delta_y }) => {
            i_slint_core::platform::WindowEvent::PointerScrolled {
                position: convert_logical_position(position.ok_or_else(|| {
                    "Missing logical position in pointer scroll event".to_string()
                })?),
                delta_x,
                delta_y,
            }
        }
        Event::PointerExited(proto::PointerExitedEvent {}) => {
            i_slint_core::platform::WindowEvent::PointerExited {}
        }
        Event::KeyPressed(proto::KeyPressedEvent { text }) => {
            i_slint_core::platform::WindowEvent::KeyPressed { text: text.into() }
        }
        Event::KeyPressRepeated(proto::KeyPressRepeatedEvent { text }) => {
            i_slint_core::platform::WindowEvent::KeyPressRepeated { text: text.into() }
        }
        Event::KeyReleased(proto::KeyReleasedEvent { text }) => {
            i_slint_core::platform::WindowEvent::KeyReleased { text: text.into() }
        }
        Event::ScaleFactorChanged(proto::ScaleFactorChangedEvent { scale_factor }) => {
            i_slint_core::platform::WindowEvent::ScaleFactorChanged { scale_factor }
        }
        Event::Resized(proto::ResizedEvent { size }) => {
            i_slint_core::platform::WindowEvent::Resized {
                size: {
                    let size =
                        size.ok_or_else(|| "Missing logical size in resize event".to_string())?;
                    i_slint_core::api::LogicalSize { width: size.width, height: size.height }
                },
            }
        }
        Event::CloseRequested(proto::CloseRequestedEvent {}) => {
            i_slint_core::platform::WindowEvent::CloseRequested
        }
        Event::WindowActiveChanged(proto::WindowActiveChangedEvent { active }) => {
            i_slint_core::platform::WindowEvent::WindowActiveChanged(active)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffered_request_observes_pending_timers_and_change_handlers() {
        use std::io::{Read, Write};

        crate::init_integration_test_with_system_time();
        slint::slint! {
            #[edition = "1.19"]
            export component App inherits Window {
                in-out property <int> input;
                out property <int> observed;
                callback bump();
                changed input => { observed = input; }
                accessible-role: region;
                accessible-label: "value " + observed;
                accessible-action-default => { bump(); }
            }
        }
        let app = App::new().unwrap();
        let weak = app.as_weak();
        app.on_bump(move || {
            let weak = weak.clone();
            slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                let app = weak.unwrap();
                app.set_input(app.get_input() + 1);
            });
        });
        let state = Rc::new(IntrospectionState::new());
        state.add_window(
            &i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter(),
        );
        let root = Some(introspection::index_to_handle(
            state.windows.borrow().values().next().unwrap().root_element_handle,
        ));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let client = Rc::new(TestingClient {
            state,
            message_loop_future: Default::default(),
            server_addr: listener.local_addr().unwrap().to_string(),
        });
        client.start_if_needed();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut requests = Vec::new();
            for msg in [
                proto::request_to_aut::Msg::RequestInvokeElementAccessibilityAction(
                    proto::RequestInvokeElementAccessibilityAction {
                        element_handle: root,
                        action: proto::ElementAccessibilityAction::Default as i32,
                    },
                ),
                proto::request_to_aut::Msg::RequestElementProperties(
                    proto::RequestElementProperties { element_handle: root },
                ),
            ] {
                let request = proto::RequestToAut { msg: Some(msg) };
                requests.write_u32::<BigEndian>(request.encoded_len() as u32).unwrap();
                request.encode(&mut requests).unwrap();
            }
            stream.write_all(&requests).unwrap();
            let mut response = None;
            for _ in 0..2 {
                let mut body = vec![0; stream.read_u32::<BigEndian>().unwrap() as usize];
                stream.read_exact(&mut body).unwrap();
                response = proto::AutResponse::decode(&body[..]).unwrap().msg;
            }
            response.unwrap()
        });
        let quit = std::thread::spawn(move || {
            let response = server.join();
            slint::invoke_from_event_loop(|| slint::quit_event_loop().unwrap()).unwrap();
            response.unwrap()
        });
        slint::run_event_loop().unwrap();

        let proto::aut_response::Msg::ElementProperties(properties) = quit.join().unwrap() else {
            panic!("expected element properties");
        };
        assert_eq!(properties.accessible_label, "value 1");
    }
}
