// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use futures_lite::AsyncReadExt;
use futures_lite::AsyncWriteExt;
use i_slint_core::api::EventLoopError;
use i_slint_core::debug_log;
use prost::Message;
use std::io::Cursor;
use std::rc::Rc;

use crate::ElementHandle;
use crate::introspection::{self, IntrospectionState, proto};

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
            i_slint_core::with_global_context(
                || panic!("uninitialized platform"),
                |context| {
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
                },
            )
            .unwrap()
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
            Req::RequestWindowList(..) => Resp::WindowList(proto::WindowListResponse {
                window_handles: self
                    .state
                    .window_handles()
                    .into_iter()
                    .map(index_to_handle)
                    .collect(),
            }),
            Req::RequestWindowProperties(proto::RequestWindowProperties { window_handle }) => {
                Resp::WindowProperties(self.window_properties(handle_to_index(
                    window_handle.ok_or_else(|| {
                        "window properties request missing window handle".to_string()
                    })?,
                ))?)
            }
            Req::RequestFindElementsById(proto::RequestFindElementsById {
                window_handle,
                elements_id,
            }) => {
                let elements = self.state.find_elements_by_id(
                    handle_to_index(window_handle.ok_or_else(|| {
                        "find elements by id request missing window handle".to_string()
                    })?),
                    &elements_id,
                )?;
                Resp::Elements(proto::ElementsResponse {
                    element_handles: elements
                        .into_iter()
                        .map(|elem| self.element_to_handle(elem))
                        .collect(),
                })
            }
            Req::RequestElementProperties(proto::RequestElementProperties { element_handle }) => {
                let element = self.element("element properties request", element_handle)?;
                Resp::ElementProperties(introspection::element_properties(&element))
            }
            Req::RequestInvokeElementAccessibilityAction(
                msg @ proto::RequestInvokeElementAccessibilityAction { element_handle, .. },
            ) => {
                let action =
                    proto::ElementAccessibilityAction::try_from(msg.action).map_err(|_| {
                        format!("invalid ElementAccessibilityAction value: {}", msg.action)
                    })?;
                let element =
                    self.element("invoke element accessibility action request", element_handle)?;
                introspection::invoke_element_accessibility_action(&element, action);
                Resp::InvokeElementAccessibilityActionResponse(
                    proto::InvokeElementAccessibilityActionResponse {},
                )
            }
            Req::RequestSetElementAccessibleValue(proto::RequestSetElementAccessibleValue {
                element_handle,
                value,
            }) => {
                let element =
                    self.element("set element accessible value request", element_handle)?;
                element.set_accessible_value(value);
                Resp::SetElementAccessibleValueResponse(proto::SetElementAccessibleValueResponse {})
            }
            Req::RequestTakeSnapshot(proto::RequestTakeSnapshot {
                window_handle,
                image_mime_type,
            }) => {
                Resp::TakeSnapshotResponse(self.take_snapshot(
                    handle_to_index(
                        window_handle.ok_or_else(|| {
                            "grab window request missing window handle".to_string()
                        })?,
                    ),
                    image_mime_type,
                )?)
            }
            Req::RequestElementClick(proto::RequestElementClick {
                element_handle,
                action,
                button,
            }) => {
                let element = self.element("element click request", element_handle)?;
                let button = introspection::convert_pointer_event_button(
                    proto::PointerEventButton::try_from(button)
                        .map_err(|_| format!("invalid PointerEventButton value: {button}"))?,
                );
                match proto::ClickAction::try_from(action)
                    .map_err(|_| format!("invalid ClickAction value: {action}"))?
                {
                    proto::ClickAction::SingleClick => element.single_click(button).await,
                    proto::ClickAction::DoubleClick => element.double_click(button).await,
                }
                Resp::ElementClickResponse(proto::ElementClickResponse {})
            }
            Req::RequestDispatchWindowEvent(proto::RequestDispatchWindowEvent {
                window_handle,
                event,
            }) => {
                self.state.dispatch_window_event(
                    handle_to_index(window_handle.ok_or_else(|| {
                        "window event dispatch request missing window handle".to_string()
                    })?),
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
                let element = self.element("run element query request", element_handle)?;
                let elements =
                    introspection::query_element_descendants(element, query_stack, find_all)?;
                Resp::ElementQueryResponse(proto::ElementQueryResponse {
                    element_handles: elements
                        .into_iter()
                        .map(|elem| self.element_to_handle(elem))
                        .collect(),
                })
            }
        })
    }

    fn window_properties(
        &self,
        window_index: generational_arena::Index,
    ) -> Result<proto::WindowPropertiesResponse, String> {
        let adapter = self.state.window_adapter(window_index)?;
        let window = adapter.window();
        Ok(proto::WindowPropertiesResponse {
            is_fullscreen: window.is_fullscreen(),
            is_maximized: window.is_maximized(),
            is_minimized: window.is_minimized(),
            size: Some(send_physical_size(window.size())),
            position: Some(send_physical_position(window.position())),
            root_element_handle: Some(index_to_handle(
                self.state.root_element_handle(window_index)?,
            )),
        })
    }

    fn take_snapshot(
        &self,
        window_index: generational_arena::Index,
        image_mime_type: String,
    ) -> Result<proto::TakeSnapshotResponse, String> {
        let window_contents_as_encoded_image =
            self.state.take_snapshot(window_index, &image_mime_type)?;
        Ok(proto::TakeSnapshotResponse { window_contents_as_encoded_image })
    }

    fn element(
        &self,
        request: &'static str,
        element_handle: Option<proto::Handle>,
    ) -> Result<ElementHandle, String> {
        let index = handle_to_index(
            element_handle.ok_or_else(|| format!("{request} missing element handle"))?,
        );
        self.state.element(request, index)
    }

    fn element_to_handle(&self, element: ElementHandle) -> proto::Handle {
        index_to_handle(self.state.element_to_handle(element))
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
    };
    eprintln!("{}, closing connection to test server", err_msg);

    // Close connection explicitly to notify the server if it is still connected.
    stream.shutdown(std::net::Shutdown::Both).ok();
}

fn index_to_handle(index: generational_arena::Index) -> proto::Handle {
    introspection::index_to_handle(index)
}

fn handle_to_index(handle: proto::Handle) -> generational_arena::Index {
    introspection::handle_to_index(handle)
}

fn send_physical_size(sz: i_slint_core::api::PhysicalSize) -> proto::PhysicalSize {
    proto::PhysicalSize { width: sz.width, height: sz.height }
}

fn send_physical_position(pos: i_slint_core::api::PhysicalPosition) -> proto::PhysicalPosition {
    proto::PhysicalPosition { x: pos.x, y: pos.y }
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
    })
}

#[test]
fn test_accessibility_role_mapping_complete() {
    macro_rules! test_accessibility_enum_mapping_inner {
        (AccessibleRole, $($Value:ident,)*) => {
            $(assert!(introspection::convert_to_proto_accessible_role(i_slint_core::items::AccessibleRole::$Value).is_some());)*
        };
        ($_:ident, $($Value:ident,)*) => {};
    }

    macro_rules! test_accessibility_enum_mapping {
        ($( $(#[doc = $enum_doc:literal])* $(#[non_exhaustive])? enum $Name:ident { $( $(#[doc = $value_doc:literal])* $Value:ident,)* })*) => {
            $(
                test_accessibility_enum_mapping_inner!($Name, $($Value,)*);
            )*
        };
    }
    i_slint_common::for_each_enums!(test_accessibility_enum_mapping);
}
