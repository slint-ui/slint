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

struct TestingClient {
    windows: RefCell<generational_arena::Arena<Weak<dyn WindowAdapter>>>,
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
        self.windows.borrow_mut().insert(Rc::downgrade(adapter));

        let this = self.clone();
        self.message_loop_future.get_or_init(|| {
            i_slint_core::future::spawn_local({
                let this = this.clone();
                async move {
                    message_loop(&this.server_addr, |request| this.handle_request(request)).await;
                }
            })
            .unwrap()
        });
    }

    fn handle_request(
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
                    element_handles: elements
                        .map(|elem| index_to_handle(self.element_handles.borrow_mut().insert(elem)))
                        .collect(),
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
        })
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
            accessible_role: convert_accessible_role(element.accessible_role().unwrap())
                .unwrap_or_default(),
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
        }
        Ok(())
    }

    fn window_adapter(
        &self,
        window_index: generational_arena::Index,
    ) -> Result<Rc<dyn WindowAdapter>, String> {
        self.windows
            .borrow()
            .get(window_index)
            .ok_or_else(|| "Invalid window handle".to_string())?
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
    ) -> Result<proto::mod_AUTResponse::OneOfmsg, String>,
) {
    debug_log!("Attempting to connect to testing server at {server_addr}");

    let mut stream = match async_net::TcpStream::connect(server_addr).await {
        Ok(stream) => stream,
        Err(err) => {
            eprintln!("Error connecting to Slint test server at {server_addr}: {}", err);
            return;
        }
    };
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
        let response = message_callback(message.msg).unwrap_or_else(|message| {
            proto::mod_AUTResponse::OneOfmsg::error(proto::ErrorResponse { message })
        });
        let response = proto::AUTResponse { msg: response };
        let mut size_header = Vec::new();
        size_header.write_u32::<BigEndian>(response.get_size() as u32).unwrap();
        stream.write_all(&size_header).await.expect("Unable to write AUT response header");
        let mut message_body = Vec::new();
        response.write_message(&mut quick_protobuf::Writer::new(&mut message_body)).unwrap();
        stream.write_all(&message_body).await.expect("Unable to write AUT response body");
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

fn convert_accessible_role(
    role: i_slint_core::items::AccessibleRole,
) -> Option<proto::AccessibleRole> {
    Some(match role {
        i_slint_core::items::AccessibleRole::None => proto::AccessibleRole::Unknown,
        i_slint_core::items::AccessibleRole::Button => proto::AccessibleRole::Button,
        i_slint_core::items::AccessibleRole::Checkbox => proto::AccessibleRole::Checkbox,
        i_slint_core::items::AccessibleRole::Combobox => proto::AccessibleRole::Combobox,
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
        _ => return None,
    })
}

#[test]
fn test_accessibility_role_mapping_complete() {
    use strum::IntoEnumIterator;
    for role in i_slint_core::items::AccessibleRole::iter() {
        assert!(convert_accessible_role(role).is_some());
    }
}
