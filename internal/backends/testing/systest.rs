// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use futures_lite::AsyncReadExt;
use futures_lite::AsyncWriteExt;
use i_slint_core::api::EventLoopError;
use i_slint_core::debug_log;
use i_slint_core::window::WindowAdapter;
use quick_protobuf::{MessageRead, MessageWrite};
use std::cell::RefCell;
use std::io::Cursor;
use std::rc::{Rc, Weak};

#[allow(non_snake_case, unused_imports, non_camel_case_types)]
mod proto {
    include!(concat!(env!("OUT_DIR"), "/slint_systest.rs"));
}

struct TestingClient {
    windows: RefCell<generational_arena::Arena<Weak<dyn WindowAdapter>>>,
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
            proto::mod_RequestToAUT::OneOfmsg::None => return Err("Unknown request".into()),
        })
    }

    fn window_properties(
        &self,
        window_index: generational_arena::Index,
    ) -> Result<proto::WindowPropertiesResponse, String> {
        let adapter = self
            .windows
            .borrow()
            .get(window_index)
            .ok_or_else(|| "Invalid window handle".to_string())?
            .upgrade()
            .ok_or_else(|| "Attempting to access deleted window".to_string())?;
        let window = adapter.window();
        Ok(proto::WindowPropertiesResponse {
            is_fullscreen: window.is_fullscreen(),
            is_maximized: window.is_maximized(),
            is_minimized: window.is_minimized(),
            size: send_physical_size(window.size()).into(),
            position: send_physical_position(window.position()).into(),
        })
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
