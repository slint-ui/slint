// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use futures_lite::AsyncReadExt;
use futures_lite::AsyncWriteExt;
use i_slint_core::api::EventLoopError;
use i_slint_core::debug_log;
use i_slint_core::item_tree::ItemTreeRc;
use i_slint_core::window::WindowAdapter;
use i_slint_core::window::WindowInner;
use quick_protobuf::{MessageRead, MessageWrite};
use std::cell::RefCell;
use std::io::Cursor;
use std::rc::{Rc, Weak};

use crate::{ElementHandle, ElementRoot};

struct RootWrapper<'a>(&'a ItemTreeRc);

impl ElementRoot for RootWrapper<'_> {
    fn item_tree(&self) -> ItemTreeRc {
        self.0.clone()
    }
}

impl super::Sealed for RootWrapper<'_> {}

#[allow(non_snake_case, unused_imports, non_camel_case_types)]
mod proto {
    include!(concat!(env!("OUT_DIR"), "/proto.rs"));
}

struct TestedWindow {
    window_adapter: Weak<dyn WindowAdapter>,
    root_element_handle: proto::Handle,
}

struct TestingClient {
    windows: RefCell<generational_arena::Arena<TestedWindow>>,
    element_handles: RefCell<generational_arena::Arena<ElementHandle>>,
    message_loop_future: std::cell::OnceCell<i_slint_core::future::JoinHandle<()>>,
    server_addr: String,
}

impl TestingClient {
    fn new() -> Option<Rc<Self>> {
        let Ok(server_addr) = std::env::var("SLINT_TEST_SERVER") else {
            return None;
        };

        Some(Rc::new(Self {
            windows: Default::default(),
            element_handles: Default::default(),
            message_loop_future: Default::default(),
            server_addr,
        }))
    }

    fn add_window(self: Rc<Self>, adapter: &Rc<dyn WindowAdapter>) {
        self.windows.borrow_mut().insert(TestedWindow {
            window_adapter: Rc::downgrade(adapter),
            root_element_handle: {
                let window = adapter.window();
                let item_tree = WindowInner::from_pub(window).component();
                let root_wrapper = RootWrapper(&item_tree);
                self.element_to_handle(root_wrapper.root_element())
            },
        });

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
                        .windows
                        .borrow()
                        .iter()
                        .map(|(index, _)| index_to_handle(index))
                        .collect(),
                })
            }
            proto::mod_RequestToAUT::OneOfmsg::request_window_properties(
                proto::RequestWindowProperties { window_handle },
            ) => proto::mod_AUTResponse::OneOfmsg::window_properties(self.window_properties(
                handle_to_index(window_handle.ok_or_else(|| {
                    "window properties request missing window handle".to_string()
                })?),
            )?),
            proto::mod_RequestToAUT::OneOfmsg::request_find_elements_by_id(
                proto::RequestFindElementsById { window_handle, elements_id },
            ) => {
                let elements = self.find_elements_by_id(
                    handle_to_index(window_handle.ok_or_else(|| {
                        "find elements by id request missing window handle".to_string()
                    })?),
                    &elements_id,
                )?;
                proto::mod_AUTResponse::OneOfmsg::elements(proto::ElementsResponse {
                    element_handles: elements.map(|elem| self.element_to_handle(elem)).collect(),
                })
            }
            proto::mod_RequestToAUT::OneOfmsg::request_element_properties(
                proto::RequestElementProperties { element_handle },
            ) => proto::mod_AUTResponse::OneOfmsg::element_properties(
                self.element_properties(element_handle)?,
            ),
            proto::mod_RequestToAUT::OneOfmsg::request_invoke_element_accessibility_action(
                proto::RequestInvokeElementAccessibilityAction { element_handle, action },
            ) => {
                self.invoke_element_accessibility_action(element_handle, action)?;
                proto::mod_AUTResponse::OneOfmsg::invoke_element_accessibility_action_response(
                    proto::InvokeElementAccessibilityActionResponse {},
                )
            }
            proto::mod_RequestToAUT::OneOfmsg::request_set_element_accessible_value(
                proto::RequestSetElementAccessibleValue { element_handle, value },
            ) => {
                let element =
                    self.element("set element accessible value request", element_handle)?;
                element.set_accessible_value(value);
                proto::mod_AUTResponse::OneOfmsg::set_element_accessible_value_response(
                    proto::SetElementAccessibleValueResponse {},
                )
            }
            proto::mod_RequestToAUT::OneOfmsg::request_take_snapshot(
                proto::RequestTakeSnapshot { window_handle, image_mime_type },
            ) => {
                proto::mod_AUTResponse::OneOfmsg::take_snapshot_response(self.take_snapshot(
                    handle_to_index(
                        window_handle.ok_or_else(|| {
                            "grab window request missing window handle".to_string()
                        })?,
                    ),
                    image_mime_type,
                )?)
            }
            proto::mod_RequestToAUT::OneOfmsg::request_element_click(
                proto::RequestElementClick { element_handle, action, button },
            ) => {
                let element = self.element("element click request", element_handle)?;
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
                self.dispatch_window_event(
                    handle_to_index(window_handle.ok_or_else(|| {
                        "window event dispatch request missing window handle".to_string()
                    })?),
                    convert_window_event(event.ok_or_else(|| {
                        "window event dispatch request missing event".to_string()
                    })?)?,
                )?;
                proto::mod_AUTResponse::OneOfmsg::dispatch_window_event_response(
                    proto::DispatchWindowEventResponse {},
                )
            }
            proto::mod_RequestToAUT::OneOfmsg::request_query_element_descendants(
                proto::RequestQueryElementDescendants { element_handle, query_stack, find_all },
            ) => {
                let element = self.element("run element query request", element_handle)?;
                let elements = self.query_element_descendants(element, query_stack, find_all)?;
                proto::mod_AUTResponse::OneOfmsg::element_query_response(
                    proto::ElementQueryResponse {
                        element_handles: elements
                            .into_iter()
                            .map(|elem| self.element_to_handle(elem))
                            .collect(),
                    },
                )
            }
            proto::mod_RequestToAUT::OneOfmsg::None => return Err("Unknown request".into()),
        })
    }

    fn window_properties(
        &self,
        window_index: generational_arena::Index,
    ) -> Result<proto::WindowPropertiesResponse, String> {
        let adapter = self.window_adapter(window_index)?;
        let window = adapter.window();
        Ok(proto::WindowPropertiesResponse {
            is_fullscreen: window.is_fullscreen(),
            is_maximized: window.is_maximized(),
            is_minimized: window.is_minimized(),
            size: send_physical_size(window.size()).into(),
            position: send_physical_position(window.position()).into(),
            root_element_handle: self.root_element_handle(window_index)?.into(),
        })
    }

    fn take_snapshot(
        &self,
        window_index: generational_arena::Index,
        image_mime_type: String,
    ) -> Result<proto::TakeSnapshotResponse, String> {
        let adapter = self.window_adapter(window_index)?;
        let window = adapter.window();
        let buffer =
            window.take_snapshot().map_err(|e| format!("Error grabbing window screenshot: {e}"))?;
        let mut window_contents_as_encoded_image: Vec<u8> = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut window_contents_as_encoded_image);
        let format = if image_mime_type.is_empty() {
            image::ImageFormat::Png
        } else {
            image::ImageFormat::from_mime_type(&image_mime_type).ok_or_else(|| {
                format!(
                    "Unsupported image format {image_mime_type} requested for window snapshotting"
                )
            })?
        };

        image::write_buffer_with_format(
            &mut cursor,
            buffer.as_bytes(),
            buffer.width(),
            buffer.height(),
            image::ExtendedColorType::Rgba8,
            format,
        )
        .map_err(|encode_err| {
            format!("error encoding {image_mime_type} image after screenshot: {encode_err}")
        })?;
        Ok(proto::TakeSnapshotResponse { window_contents_as_encoded_image })
    }

    fn dispatch_window_event(
        &self,
        window_index: generational_arena::Index,
        event: i_slint_core::platform::WindowEvent,
    ) -> Result<(), String> {
        let adapter = self.window_adapter(window_index)?;
        let window = adapter.window();
        window.dispatch_event(event);
        Ok(())
    }

    fn find_elements_by_id(
        &self,
        window_index: generational_arena::Index,
        elements_id: &str,
    ) -> Result<impl Iterator<Item = crate::ElementHandle>, String> {
        let adapter = self.window_adapter(window_index)?;
        let window = adapter.window();
        let item_tree = WindowInner::from_pub(window).component();
        Ok(ElementHandle::find_by_element_id(&RootWrapper(&item_tree), elements_id)
            .collect::<Vec<_>>()
            .into_iter())
    }

    fn query_element_descendants(
        &self,
        element: ElementHandle,
        query_stack: Vec<proto::ElementQueryInstruction>,
        find_all: bool,
    ) -> Result<Vec<crate::ElementHandle>, String> {
        let mut query = element.query_descendants();
        for instruction in query_stack {
            match instruction.instruction {
                proto::mod_ElementQueryInstruction::OneOfinstruction::match_descendants(_) => {
                    query = query.match_descendants();
                }
                proto::mod_ElementQueryInstruction::OneOfinstruction::match_element_id(id) => {
                    query = query.match_id(id)
                }
                proto::mod_ElementQueryInstruction::OneOfinstruction::match_element_type_name(type_name) => {
                    query = query.match_type_name(type_name)
                }
                proto::mod_ElementQueryInstruction::OneOfinstruction::match_element_type_name_or_base(type_name_or_base) => {
                    query = query.match_type_name(type_name_or_base)
                }
                proto::mod_ElementQueryInstruction::OneOfinstruction::match_element_accessible_role(role) => {
                    query = query.match_accessible_role(convert_from_proto_accessible_role(role).ok_or_else(|| "Unknown accessibility role used in element query".to_string())?)
                }
                proto::mod_ElementQueryInstruction::OneOfinstruction::None => {
                    return Err("unknown element query instruction".into());
                }
            }
        }
        Ok(if find_all { query.find_all() } else { query.find_first().into_iter().collect() })
    }

    fn element(
        &self,
        request: &'static str,
        element_handle: Option<proto::Handle>,
    ) -> Result<ElementHandle, String> {
        let index = handle_to_index(
            element_handle.ok_or_else(|| format!("{request} missing element handle"))?,
        );
        let element = self
            .element_handles
            .borrow()
            .get(index)
            .ok_or_else(|| format!("Invalid element handle for {request}"))?
            .clone();
        if !element.is_valid() {
            self.element_handles.borrow_mut().remove(index);
            return Err(format!(
                "Element handle for {request} refers to element that was destroyed"
            ));
        }
        Ok(element)
    }

    fn element_to_handle(&self, element: ElementHandle) -> proto::Handle {
        index_to_handle(self.element_handles.borrow_mut().insert(element))
    }

    fn element_properties(
        &self,
        element_handle: Option<proto::Handle>,
    ) -> Result<proto::ElementPropertiesResponse, String> {
        let element = self.element("element properties request", element_handle)?;
        let type_names_and_ids = core::iter::once(proto::ElementTypeNameAndId {
            type_name: element.type_name().unwrap().into(),
            id: element.id().unwrap().into(),
        })
        .chain(element.bases().unwrap().map(|base_type_name| proto::ElementTypeNameAndId {
            type_name: base_type_name.into(),
            id: "root".into(),
        }))
        .collect();
        Ok(proto::ElementPropertiesResponse {
            type_names_and_ids,
            accessible_label: element
                .accessible_label()
                .map_or(Default::default(), |s| s.to_string()),
            accessible_value: element.accessible_value().unwrap_or_default().to_string(),
            accessible_value_maximum: element.accessible_value_maximum().unwrap_or_default(),
            accessible_value_minimum: element.accessible_value_minimum().unwrap_or_default(),
            accessible_value_step: element.accessible_value_step().unwrap_or_default(),
            accessible_description: element
                .accessible_description()
                .unwrap_or_default()
                .to_string(),
            accessible_checked: element.accessible_checked().unwrap_or_default(),
            accessible_checkable: element.accessible_checkable().unwrap_or_default(),
            size: send_logical_size(element.size()).into(),
            absolute_position: send_logical_position(element.absolute_position()).into(),
            accessible_role: convert_to_proto_accessible_role(element.accessible_role().unwrap())
                .unwrap_or_default(),
            computed_opacity: element.computed_opacity(),
            accessible_placeholder_text: element
                .accessible_placeholder_text()
                .unwrap_or_default()
                .to_string(),
            accessible_enabled: element.accessible_enabled().unwrap_or_default(),
            accessible_read_only: element.accessible_read_only().unwrap_or_default(),
        })
    }

    fn invoke_element_accessibility_action(
        &self,
        element_handle: Option<proto::Handle>,
        action: proto::ElementAccessibilityAction,
    ) -> Result<(), String> {
        let element =
            self.element("invoke element accessibility action request", element_handle)?;
        match action {
            proto::ElementAccessibilityAction::Default_ => {
                element.invoke_accessible_default_action()
            }
            proto::ElementAccessibilityAction::Increment => {
                element.invoke_accessible_increment_action()
            }
            proto::ElementAccessibilityAction::Decrement => {
                element.invoke_accessible_decrement_action()
            }
            proto::ElementAccessibilityAction::Expand => element.invoke_accessible_expand_action(),
        }
        Ok(())
    }

    fn root_element_handle(
        &self,
        window_index: generational_arena::Index,
    ) -> Result<proto::Handle, String> {
        Ok(self
            .windows
            .borrow()
            .get(window_index)
            .ok_or_else(|| "Invalid window handle".to_string())?
            .root_element_handle
            .clone())
    }

    fn window_adapter(
        &self,
        window_index: generational_arena::Index,
    ) -> Result<Rc<dyn WindowAdapter>, String> {
        self.windows
            .borrow()
            .get(window_index)
            .ok_or_else(|| "Invalid window handle".to_string())?
            .window_adapter
            .upgrade()
            .ok_or_else(|| "Attempting to access deleted window".to_string())
    }
}

pub fn init() -> Result<(), EventLoopError> {
    let Some(client) = TestingClient::new() else {
        return Ok(());
    };

    i_slint_core::context::set_window_shown_hook(Some(Box::new(move |adapter| {
        client.clone().add_window(adapter)
    })))
    .unwrap();

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

    loop {
        let mut message_size_buf = vec![0; 4];
        stream
            .read_exact(&mut message_size_buf)
            .await
            .expect("Unable to read request header from AUT connection");

        let message_size: usize =
            Cursor::new(message_size_buf).read_u32::<BigEndian>().unwrap() as usize;
        let mut message_buf = Vec::with_capacity(message_size);
        message_buf.resize(message_size, 0);
        stream
            .read_exact(&mut message_buf)
            .await
            .expect("Unable to read request data from AUT connection");

        let message = proto::RequestToAUT::from_reader(
            &mut quick_protobuf::reader::BytesReader::from_bytes(&message_buf),
            &mut message_buf,
        )
        .expect("Unable to de-serialize AUT request message");
        let response = message_callback(message.msg).await.unwrap_or_else(|message| {
            proto::mod_AUTResponse::OneOfmsg::error(proto::ErrorResponse { message })
        });
        let response = proto::AUTResponse { msg: response };
        let mut binary_message = Vec::new();
        binary_message.write_u32::<BigEndian>(response.get_size() as u32).unwrap();
        response.write_message(&mut quick_protobuf::Writer::new(&mut binary_message)).unwrap();
        stream.write_all(&binary_message).await.expect("Unable to write AUT response body");
    }
}

fn index_to_handle(index: generational_arena::Index) -> proto::Handle {
    let (index, generation) = index.into_raw_parts();
    proto::Handle { index: index as u64, generation }
}

fn handle_to_index(handle: proto::Handle) -> generational_arena::Index {
    generational_arena::Index::from_raw_parts(handle.index as usize, handle.generation)
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

fn convert_to_proto_accessible_role(
    role: i_slint_core::items::AccessibleRole,
) -> Option<proto::AccessibleRole> {
    Some(match role {
        i_slint_core::items::AccessibleRole::None => proto::AccessibleRole::Unknown,
        i_slint_core::items::AccessibleRole::Button => proto::AccessibleRole::Button,
        i_slint_core::items::AccessibleRole::Checkbox => proto::AccessibleRole::Checkbox,
        i_slint_core::items::AccessibleRole::Combobox => proto::AccessibleRole::Combobox,
        i_slint_core::items::AccessibleRole::Groupbox => proto::AccessibleRole::Groupbox,
        i_slint_core::items::AccessibleRole::List => proto::AccessibleRole::List,
        i_slint_core::items::AccessibleRole::Slider => proto::AccessibleRole::Slider,
        i_slint_core::items::AccessibleRole::Spinbox => proto::AccessibleRole::Spinbox,
        i_slint_core::items::AccessibleRole::Tab => proto::AccessibleRole::Tab,
        i_slint_core::items::AccessibleRole::TabList => proto::AccessibleRole::TabList,
        i_slint_core::items::AccessibleRole::Text => proto::AccessibleRole::Text,
        i_slint_core::items::AccessibleRole::Table => proto::AccessibleRole::Table,
        i_slint_core::items::AccessibleRole::Tree => proto::AccessibleRole::Tree,
        i_slint_core::items::AccessibleRole::ProgressIndicator => {
            proto::AccessibleRole::ProgressIndicator
        }
        i_slint_core::items::AccessibleRole::TextInput => proto::AccessibleRole::TextInput,
        i_slint_core::items::AccessibleRole::Switch => proto::AccessibleRole::Switch,
        i_slint_core::items::AccessibleRole::ListItem => proto::AccessibleRole::ListItem,
        i_slint_core::items::AccessibleRole::TabPanel => proto::AccessibleRole::TabPanel,
        i_slint_core::items::AccessibleRole::Image => proto::AccessibleRole::Image,
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
                    .ok_or_else(|| format!("Missing logical position in pointer press event"))?,
            ),
            button: convert_pointer_event_button(button),
        },
        proto::mod_WindowEvent::OneOfevent::pointer_released(proto::PointerReleaseEvent {
            position,
            button,
        }) => i_slint_core::platform::WindowEvent::PointerReleased {
            position: convert_logical_position(
                position
                    .ok_or_else(|| format!("Missing logical position in pointer press event"))?,
            ),
            button: convert_pointer_event_button(button),
        },
        proto::mod_WindowEvent::OneOfevent::pointer_moved(proto::PointerMoveEvent { position }) => {
            i_slint_core::platform::WindowEvent::PointerMoved {
                position: convert_logical_position(
                    position
                        .ok_or_else(|| format!("Missing logical position in pointer move event"))?,
                ),
            }
        }
        proto::mod_WindowEvent::OneOfevent::pointer_scrolled(proto::PointerScrolledEvent {
            position,
            delta_x,
            delta_y,
        }) => i_slint_core::platform::WindowEvent::PointerScrolled {
            position: convert_logical_position(
                position
                    .ok_or_else(|| format!("Missing logical position in pointer scroll event"))?,
            ),
            delta_x,
            delta_y,
        },
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
            return Err(format!("Unknown window event received in system testing protobuf"))
        }
    })
}

#[test]
fn test_accessibility_role_mapping_complete() {
    macro_rules! test_accessibility_enum_mapping_inner {
        (AccessibleRole, $($Value:ident,)*) => {
            $(assert!(convert_to_proto_accessible_role(i_slint_core::items::AccessibleRole::$Value).is_some());)*
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
