// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use futures_lite::AsyncReadExt;
use futures_lite::AsyncWriteExt;
use i_slint_core::api::EventLoopError;
use i_slint_core::debug_log;
use quick_protobuf::{MessageRead, MessageWrite};
use std::io::Cursor;
use std::rc::Rc;

use crate::LayoutKind;
use crate::introspection::{AccessibilityAction, IntrospectionState, QueryInstruction};

#[allow(non_snake_case, unused_imports, non_camel_case_types, clippy::all)]
mod proto {
    include!(concat!(env!("OUT_DIR"), "/proto.rs"));
}

struct TestingClient {
    state: Rc<IntrospectionState>,
    message_loop_future: std::cell::OnceCell<i_slint_core::future::JoinHandle<()>>,
    server_addr: String,
}

impl TestingClient {
    fn new() -> Option<Rc<Self>> {
        let Ok(server_addr) = std::env::var("SLINT_TEST_SERVER") else {
            return None;
        };

        Some(Rc::new(Self {
            state: crate::introspection::shared_state(),
            message_loop_future: Default::default(),
            server_addr,
        }))
    }

    fn start_message_loop(self: Rc<Self>) {
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
        request: proto::mod_RequestToAUT::OneOfmsg,
    ) -> Result<proto::mod_AUTResponse::OneOfmsg, String> {
        Ok(match request {
            proto::mod_RequestToAUT::OneOfmsg::request_window_list(..) => {
                proto::mod_AUTResponse::OneOfmsg::window_list(proto::WindowListResponse {
                    window_handles: self
                        .state
                        .window_handles()
                        .into_iter()
                        .map(index_to_handle)
                        .collect(),
                })
            }
            proto::mod_RequestToAUT::OneOfmsg::request_window_properties(
                proto::RequestWindowProperties { window_handle },
            ) => {
                let window_index = handle_to_index(window_handle.ok_or_else(|| {
                    "window properties request missing window handle".to_string()
                })?);
                let wp = self.state.window_properties(window_index)?;
                proto::mod_AUTResponse::OneOfmsg::window_properties(
                    proto::WindowPropertiesResponse {
                        is_fullscreen: wp.is_fullscreen,
                        is_maximized: wp.is_maximized,
                        is_minimized: wp.is_minimized,
                        size: send_physical_size(wp.size).into(),
                        position: send_physical_position(wp.position).into(),
                        root_element_handle: index_to_handle(wp.root_element_handle).into(),
                    },
                )
            }
            proto::mod_RequestToAUT::OneOfmsg::request_find_elements_by_id(
                proto::RequestFindElementsById { window_handle, elements_id },
            ) => {
                let window_index = handle_to_index(window_handle.ok_or_else(|| {
                    "find elements by id request missing window handle".to_string()
                })?);
                let elements = self.state.find_elements_by_id(window_index, &elements_id)?;
                proto::mod_AUTResponse::OneOfmsg::elements(proto::ElementsResponse {
                    element_handles: elements
                        .into_iter()
                        .map(|elem| self.state.element_to_handle(elem))
                        .map(index_to_handle)
                        .collect(),
                })
            }
            proto::mod_RequestToAUT::OneOfmsg::request_element_properties(
                proto::RequestElementProperties { element_handle },
            ) => {
                let index = handle_to_index(element_handle.ok_or_else(|| {
                    "element properties request missing element handle".to_string()
                })?);
                let element = self.state.element("element properties request", index)?;
                let ep = self.state.element_properties(&element);
                proto::mod_AUTResponse::OneOfmsg::element_properties(
                    proto::ElementPropertiesResponse {
                        type_names_and_ids: ep
                            .type_names_and_ids
                            .into_iter()
                            .map(|(type_name, id)| proto::ElementTypeNameAndId { type_name, id })
                            .collect(),
                        accessible_label: ep.accessible_label.unwrap_or_default(),
                        accessible_value: ep.accessible_value.unwrap_or_default(),
                        accessible_value_maximum: ep.accessible_value_maximum,
                        accessible_value_minimum: ep.accessible_value_minimum,
                        accessible_value_step: ep.accessible_value_step,
                        accessible_description: ep.accessible_description.unwrap_or_default(),
                        accessible_checked: ep.accessible_checked,
                        accessible_checkable: ep.accessible_checkable,
                        size: send_logical_size(ep.size).into(),
                        absolute_position: send_logical_position(ep.absolute_position).into(),
                        accessible_role: convert_accessible_role_str_to_proto(ep.accessible_role)
                            .unwrap_or_default(),
                        computed_opacity: ep.computed_opacity,
                        accessible_placeholder_text: ep
                            .accessible_placeholder_text
                            .unwrap_or_default(),
                        accessible_enabled: ep.accessible_enabled,
                        accessible_read_only: ep.accessible_read_only,
                        layout_kind: match ep.layout_kind {
                            Some(LayoutKind::HorizontalLayout) => {
                                proto::LayoutKind::HorizontalLayout.into()
                            }
                            Some(LayoutKind::VerticalLayout) => {
                                proto::LayoutKind::VerticalLayout.into()
                            }
                            Some(LayoutKind::GridLayout) => proto::LayoutKind::GridLayout.into(),
                            Some(LayoutKind::FlexBox) => proto::LayoutKind::FlexBox.into(),
                            None => proto::LayoutKind::NotALayout.into(),
                        },
                    },
                )
            }
            proto::mod_RequestToAUT::OneOfmsg::request_invoke_element_accessibility_action(
                proto::RequestInvokeElementAccessibilityAction { element_handle, action },
            ) => {
                let index = handle_to_index(element_handle.ok_or_else(|| {
                    "invoke element accessibility action request missing element handle".to_string()
                })?);
                let element =
                    self.state.element("invoke element accessibility action request", index)?;
                let action = match action {
                    proto::ElementAccessibilityAction::Default_ => AccessibilityAction::Default,
                    proto::ElementAccessibilityAction::Increment => AccessibilityAction::Increment,
                    proto::ElementAccessibilityAction::Decrement => AccessibilityAction::Decrement,
                    proto::ElementAccessibilityAction::Expand => AccessibilityAction::Expand,
                };
                self.state.invoke_element_accessibility_action(&element, action)?;
                proto::mod_AUTResponse::OneOfmsg::invoke_element_accessibility_action_response(
                    proto::InvokeElementAccessibilityActionResponse {},
                )
            }
            proto::mod_RequestToAUT::OneOfmsg::request_set_element_accessible_value(
                proto::RequestSetElementAccessibleValue { element_handle, value },
            ) => {
                let index = handle_to_index(element_handle.ok_or_else(|| {
                    "set element accessible value request missing element handle".to_string()
                })?);
                let element = self.state.element("set element accessible value request", index)?;
                element.set_accessible_value(value);
                proto::mod_AUTResponse::OneOfmsg::set_element_accessible_value_response(
                    proto::SetElementAccessibleValueResponse {},
                )
            }
            proto::mod_RequestToAUT::OneOfmsg::request_take_snapshot(
                proto::RequestTakeSnapshot { window_handle, image_mime_type },
            ) => {
                let window_index = handle_to_index(
                    window_handle
                        .ok_or_else(|| "grab window request missing window handle".to_string())?,
                );
                let window_contents_as_encoded_image =
                    self.state.take_snapshot(window_index, &image_mime_type)?;
                proto::mod_AUTResponse::OneOfmsg::take_snapshot_response(
                    proto::TakeSnapshotResponse { window_contents_as_encoded_image },
                )
            }
            proto::mod_RequestToAUT::OneOfmsg::request_element_click(
                proto::RequestElementClick { element_handle, action, button },
            ) => {
                let index =
                    handle_to_index(element_handle.ok_or_else(|| {
                        "element click request missing element handle".to_string()
                    })?);
                let element = self.state.element("element click request", index)?;
                let button = convert_pointer_event_button(button);
                match action {
                    proto::ClickAction::SingleClick => element.single_click(button).await,
                    proto::ClickAction::DoubleClick => element.double_click(button).await,
                }
                proto::mod_AUTResponse::OneOfmsg::element_click_response(
                    proto::ElementClickResponse {},
                )
            }
            proto::mod_RequestToAUT::OneOfmsg::request_dispatch_window_event(
                proto::RequestDispatchWindowEvent { window_handle, event },
            ) => {
                let window_index = handle_to_index(window_handle.ok_or_else(|| {
                    "window event dispatch request missing window handle".to_string()
                })?);
                let event =
                    convert_window_event(event.ok_or_else(|| {
                        "window event dispatch request missing event".to_string()
                    })?)?;
                self.state.dispatch_window_event(window_index, event)?;
                proto::mod_AUTResponse::OneOfmsg::dispatch_window_event_response(
                    proto::DispatchWindowEventResponse {},
                )
            }
            proto::mod_RequestToAUT::OneOfmsg::request_query_element_descendants(
                proto::RequestQueryElementDescendants { element_handle, query_stack, find_all },
            ) => {
                let index = handle_to_index(element_handle.ok_or_else(|| {
                    "run element query request missing element handle".to_string()
                })?);
                let element = self.state.element("run element query request", index)?;
                let instructions = convert_query_instructions(query_stack)?;
                let elements =
                    self.state.query_element_descendants(element, instructions, find_all)?;
                proto::mod_AUTResponse::OneOfmsg::element_query_response(
                    proto::ElementQueryResponse {
                        element_handles: elements
                            .into_iter()
                            .map(|elem| self.state.element_to_handle(elem))
                            .map(index_to_handle)
                            .collect(),
                    },
                )
            }
            proto::mod_RequestToAUT::OneOfmsg::None => return Err("Unknown request".into()),
        })
    }
}

pub fn init() -> Result<(), EventLoopError> {
    let Some(client) = TestingClient::new() else {
        return Ok(());
    };

    // Use the shared introspection state (shared with mcp_server when both features are enabled).
    crate::introspection::ensure_window_tracking()?;

    // Chain with the existing hook (from ensure_window_tracking) to start the
    // message loop on first window show.
    let previous_hook = i_slint_core::context::set_window_shown_hook(None)
        .map_err(|_| EventLoopError::NoEventLoopProvider)?;
    let previous_hook = std::cell::RefCell::new(previous_hook);

    i_slint_core::context::set_window_shown_hook(Some(Box::new(move |adapter| {
        if let Some(prev) = previous_hook.borrow_mut().as_mut() {
            prev(adapter);
        }
        client.clone().start_message_loop()
    })))
    .map_err(|_| EventLoopError::NoEventLoopProvider)?;

    Ok(())
}

async fn message_loop(
    server_addr: &str,
    mut message_callback: impl FnMut(
        proto::mod_RequestToAUT::OneOfmsg,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<proto::mod_AUTResponse::OneOfmsg, String>>>,
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

        let message = match proto::RequestToAUT::from_reader(
            &mut quick_protobuf::reader::BytesReader::from_bytes(&message_buf),
            &message_buf,
        ) {
            Ok(msg) => msg,
            Err(_) => {
                break "Error de-serializing AUT request message";
            }
        };
        let response = message_callback(message.msg).await.unwrap_or_else(|message| {
            proto::mod_AUTResponse::OneOfmsg::error(proto::ErrorResponse { message })
        });
        let response = proto::AUTResponse { msg: response };
        let mut binary_message = Vec::new();
        binary_message.write_u32::<BigEndian>(response.get_size() as u32).unwrap();
        response.write_message(&mut quick_protobuf::Writer::new(&mut binary_message)).unwrap();
        if stream.write_all(&binary_message).await.is_err() {
            break "Unable to write AUT response body";
        }
    };
    eprintln!("{}, closing connection to test server", err_msg);

    // Close connection explicitly to notify the server if it is still connected.
    stream.shutdown(std::net::Shutdown::Both).ok();
}

// ============================================================================
// Proto conversion helpers
// ============================================================================

fn index_to_handle(index: generational_arena::Index) -> proto::Handle {
    let (index, generation) = crate::introspection::index_to_parts(index);
    proto::Handle { index, generation }
}

fn handle_to_index(handle: proto::Handle) -> generational_arena::Index {
    crate::introspection::parts_to_index(handle.index, handle.generation)
}

fn send_physical_size(sz: i_slint_core::api::PhysicalSize) -> proto::PhysicalSize {
    proto::PhysicalSize { width: sz.width, height: sz.height }
}

fn send_physical_position(pos: i_slint_core::api::PhysicalPosition) -> proto::PhysicalPosition {
    proto::PhysicalPosition { x: pos.x, y: pos.y }
}

fn send_logical_size(sz: i_slint_core::api::LogicalSize) -> proto::LogicalSize {
    proto::LogicalSize { width: sz.width, height: sz.height }
}

fn send_logical_position(pos: i_slint_core::api::LogicalPosition) -> proto::LogicalPosition {
    proto::LogicalPosition { x: pos.x, y: pos.y }
}

fn convert_logical_position(pos: proto::LogicalPosition) -> i_slint_core::api::LogicalPosition {
    i_slint_core::api::LogicalPosition { x: pos.x, y: pos.y }
}

fn convert_query_instructions(
    query_stack: Vec<proto::ElementQueryInstruction>,
) -> Result<Vec<QueryInstruction>, String> {
    let mut instructions = Vec::new();
    for instruction in query_stack {
        let qi = match instruction.instruction {
            proto::mod_ElementQueryInstruction::OneOfinstruction::match_descendants(_) => {
                QueryInstruction::MatchDescendants
            }
            proto::mod_ElementQueryInstruction::OneOfinstruction::match_element_id(id) => {
                QueryInstruction::MatchId(id)
            }
            proto::mod_ElementQueryInstruction::OneOfinstruction::match_element_type_name(
                type_name,
            ) => QueryInstruction::MatchTypeName(type_name),
            proto::mod_ElementQueryInstruction::OneOfinstruction::match_element_type_name_or_base(
                type_name_or_base,
            ) => QueryInstruction::MatchTypeNameOrBase(type_name_or_base),
            proto::mod_ElementQueryInstruction::OneOfinstruction::match_element_accessible_role(
                role,
            ) => {
                let role = convert_from_proto_accessible_role(role).ok_or_else(|| {
                    "Unknown accessibility role used in element query".to_string()
                })?;
                QueryInstruction::MatchAccessibleRole(role)
            }
            proto::mod_ElementQueryInstruction::OneOfinstruction::None => {
                return Err("unknown element query instruction".into());
            }
        };
        instructions.push(qi);
    }
    Ok(instructions)
}

/// Convert an accessible role string (from introspection) to a proto enum.
fn convert_accessible_role_str_to_proto(role: &str) -> Option<proto::AccessibleRole> {
    Some(match role {
        "unknown" => proto::AccessibleRole::Unknown,
        "button" => proto::AccessibleRole::Button,
        "checkbox" => proto::AccessibleRole::Checkbox,
        "combobox" => proto::AccessibleRole::Combobox,
        "groupbox" => proto::AccessibleRole::Groupbox,
        "list" => proto::AccessibleRole::List,
        "slider" => proto::AccessibleRole::Slider,
        "spinbox" => proto::AccessibleRole::Spinbox,
        "tab" => proto::AccessibleRole::Tab,
        "tab-list" => proto::AccessibleRole::TabList,
        "text" => proto::AccessibleRole::Text,
        "table" => proto::AccessibleRole::Table,
        "tree" => proto::AccessibleRole::Tree,
        "progress-indicator" => proto::AccessibleRole::ProgressIndicator,
        "text-input" => proto::AccessibleRole::TextInput,
        "switch" => proto::AccessibleRole::Switch,
        "list-item" => proto::AccessibleRole::ListItem,
        "tab-panel" => proto::AccessibleRole::TabPanel,
        "image" => proto::AccessibleRole::Image,
        "radio-button" => proto::AccessibleRole::RadioButton,
        _ => return None,
    })
}

fn convert_from_proto_accessible_role(
    role: proto::AccessibleRole,
) -> Option<i_slint_core::items::AccessibleRole> {
    Some(match role {
        proto::AccessibleRole::Unknown => i_slint_core::items::AccessibleRole::None,
        proto::AccessibleRole::Button => i_slint_core::items::AccessibleRole::Button,
        proto::AccessibleRole::Checkbox => i_slint_core::items::AccessibleRole::Checkbox,
        proto::AccessibleRole::Combobox => i_slint_core::items::AccessibleRole::Combobox,
        proto::AccessibleRole::Groupbox => i_slint_core::items::AccessibleRole::Groupbox,
        proto::AccessibleRole::List => i_slint_core::items::AccessibleRole::List,
        proto::AccessibleRole::Slider => i_slint_core::items::AccessibleRole::Slider,
        proto::AccessibleRole::Spinbox => i_slint_core::items::AccessibleRole::Spinbox,
        proto::AccessibleRole::Tab => i_slint_core::items::AccessibleRole::Tab,
        proto::AccessibleRole::TabList => i_slint_core::items::AccessibleRole::TabList,
        proto::AccessibleRole::Text => i_slint_core::items::AccessibleRole::Text,
        proto::AccessibleRole::Table => i_slint_core::items::AccessibleRole::Table,
        proto::AccessibleRole::Tree => i_slint_core::items::AccessibleRole::Tree,
        proto::AccessibleRole::ProgressIndicator => {
            i_slint_core::items::AccessibleRole::ProgressIndicator
        }
        proto::AccessibleRole::TextInput => i_slint_core::items::AccessibleRole::TextInput,
        proto::AccessibleRole::Switch => i_slint_core::items::AccessibleRole::Switch,
        proto::AccessibleRole::ListItem => i_slint_core::items::AccessibleRole::ListItem,
        proto::AccessibleRole::TabPanel => i_slint_core::items::AccessibleRole::TabPanel,
        proto::AccessibleRole::Image => i_slint_core::items::AccessibleRole::Image,
        proto::AccessibleRole::RadioButton => i_slint_core::items::AccessibleRole::RadioButton,
    })
}

fn convert_pointer_event_button(
    button: proto::PointerEventButton,
) -> i_slint_core::platform::PointerEventButton {
    match button {
        proto::PointerEventButton::Left => i_slint_core::platform::PointerEventButton::Left,
        proto::PointerEventButton::Right => i_slint_core::platform::PointerEventButton::Right,
        proto::PointerEventButton::Middle => i_slint_core::platform::PointerEventButton::Middle,
    }
}

fn convert_window_event(
    event: proto::WindowEvent,
) -> Result<i_slint_core::platform::WindowEvent, String> {
    Ok(match event.event {
        proto::mod_WindowEvent::OneOfevent::pointer_pressed(proto::PointerPressEvent {
            position,
            button,
        }) => i_slint_core::platform::WindowEvent::PointerPressed {
            position: convert_logical_position(
                position
                    .ok_or_else(|| "Missing logical position in pointer press event".to_string())?,
            ),
            button: convert_pointer_event_button(button),
        },
        proto::mod_WindowEvent::OneOfevent::pointer_released(proto::PointerReleaseEvent {
            position,
            button,
        }) => i_slint_core::platform::WindowEvent::PointerReleased {
            position: convert_logical_position(
                position
                    .ok_or_else(|| "Missing logical position in pointer press event".to_string())?,
            ),
            button: convert_pointer_event_button(button),
        },
        proto::mod_WindowEvent::OneOfevent::pointer_moved(proto::PointerMoveEvent { position }) => {
            i_slint_core::platform::WindowEvent::PointerMoved {
                position: convert_logical_position(
                    position.ok_or_else(|| {
                        "Missing logical position in pointer move event".to_string()
                    })?,
                ),
            }
        }
        proto::mod_WindowEvent::OneOfevent::pointer_scrolled(proto::PointerScrolledEvent {
            position,
            delta_x,
            delta_y,
        }) => {
            i_slint_core::platform::WindowEvent::PointerScrolled {
                position: convert_logical_position(position.ok_or_else(|| {
                    "Missing logical position in pointer scroll event".to_string()
                })?),
                delta_x,
                delta_y,
            }
        }
        proto::mod_WindowEvent::OneOfevent::pointer_exited(proto::PointerExitedEvent {}) => {
            i_slint_core::platform::WindowEvent::PointerExited {}
        }
        proto::mod_WindowEvent::OneOfevent::key_pressed(proto::KeyPressedEvent { text }) => {
            i_slint_core::platform::WindowEvent::KeyPressed { text: text.into() }
        }
        proto::mod_WindowEvent::OneOfevent::key_press_repeated(proto::KeyPressRepeatedEvent {
            text,
        }) => i_slint_core::platform::WindowEvent::KeyPressRepeated { text: text.into() },
        proto::mod_WindowEvent::OneOfevent::key_released(proto::KeyReleasedEvent { text }) => {
            i_slint_core::platform::WindowEvent::KeyReleased { text: text.into() }
        }
        proto::mod_WindowEvent::OneOfevent::None => {
            return Err("Unknown window event received in system testing protobuf".to_string());
        }
    })
}

#[test]
fn test_accessibility_role_mapping_complete() {
    macro_rules! test_accessibility_enum_mapping_inner {
        (AccessibleRole, $($Value:ident,)*) => {
            $(assert!(crate::introspection::accessible_role_to_string(i_slint_core::items::AccessibleRole::$Value) == "unknown" || convert_accessible_role_str_to_proto(crate::introspection::accessible_role_to_string(i_slint_core::items::AccessibleRole::$Value)).is_some());)*
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
