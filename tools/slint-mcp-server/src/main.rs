// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! MCP (Model Context Protocol) server for introspecting Slint applications at runtime.
//!
//! This server acts as a bridge between the Slint systest protobuf protocol and MCP,
//! allowing LLMs and other MCP clients to inspect and interact with running Slint applications.
//!
//! # Usage
//!
//! 1. Start the MCP server: `slint-mcp-server --port 4242`
//! 2. Launch your Slint app with: `SLINT_TEST_SERVER=localhost:4242 ./your-app`
//! 3. Connect an MCP client (e.g. Claude) to the server via stdio
//!
//! The server listens on a TCP port for the Slint app to connect (via `SLINT_TEST_SERVER`),
//! and exposes MCP tools over stdio for the LLM to use.

use base64::Engine;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use quick_protobuf::{MessageRead, MessageWrite};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

#[allow(non_snake_case, unused_imports, non_camel_case_types)]
mod proto {
    include!(concat!(env!("OUT_DIR"), "/proto.rs"));
}

// ============================================================================
// Protobuf TCP Bridge
// ============================================================================

struct SlintConnection {
    stream: tokio::net::TcpStream,
}

impl SlintConnection {
    async fn send_request(
        &mut self,
        request: proto::RequestToAUT,
    ) -> Result<proto::AUTResponse, String> {
        // Encode request
        let mut buf = Vec::new();
        request
            .write_message(&mut quick_protobuf::Writer::new(&mut buf))
            .map_err(|e| format!("Failed to encode request: {e}"))?;

        // Send with length prefix
        let len = u32::try_from(buf.len())
            .map_err(|_| "Request too large to encode as protobuf".to_string())?;
        let mut len_prefix = [0u8; 4];
        WriteBytesExt::write_u32::<BigEndian>(&mut len_prefix.as_mut_slice(), len).unwrap();
        self.stream
            .write_all(&len_prefix)
            .await
            .map_err(|e| format!("Failed to send request: {e}"))?;
        self.stream
            .write_all(&buf)
            .await
            .map_err(|e| format!("Failed to send request: {e}"))?;

        // Read response length
        let mut len_buf = [0u8; 4];
        self.stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| format!("Failed to read response length: {e}"))?;
        let response_len_u32 =
            ReadBytesExt::read_u32::<BigEndian>(&mut Cursor::new(&len_buf[..])).unwrap();
        let response_len = usize::try_from(response_len_u32)
            .map_err(|_| "Response length exceeds platform usize".to_string())?;

        const MAX_RESPONSE_SIZE: usize = 64 * 1024 * 1024; // 64 MB
        if response_len > MAX_RESPONSE_SIZE {
            return Err(format!(
                "Response too large: {response_len} bytes (max {MAX_RESPONSE_SIZE})"
            ));
        }

        // Read response body
        let mut response_buf = vec![0u8; response_len];
        self.stream
            .read_exact(&mut response_buf)
            .await
            .map_err(|e| format!("Failed to read response body: {e}"))?;

        // Decode response
        let response = proto::AUTResponse::from_reader(
            &mut quick_protobuf::reader::BytesReader::from_bytes(&response_buf),
            &mut response_buf,
        )
        .map_err(|e| format!("Failed to decode response: {e}"))?;

        Ok(response)
    }
}

// ============================================================================
// MCP Protocol Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: serde_json::Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message }),
        }
    }
}

// ============================================================================
// Server State (single mutex to avoid nested lock deadlocks)
// ============================================================================

struct ServerState {
    listener: Option<TcpListener>,
    connection: Option<SlintConnection>,
}

// ============================================================================
// JSON types for element/window representation
// ============================================================================

#[derive(Debug, Serialize)]
struct HandleJson {
    index: u64,
    generation: u64,
}

impl From<&proto::Handle> for HandleJson {
    fn from(h: &proto::Handle) -> Self {
        Self {
            index: h.index,
            generation: h.generation,
        }
    }
}

fn handle_from_json(v: &serde_json::Value) -> Result<proto::Handle, String> {
    let obj = v.as_object().ok_or("handle must be an object")?;
    Ok(proto::Handle {
        index: obj
            .get("index")
            .and_then(|v| v.as_u64())
            .ok_or("handle missing index")?,
        generation: obj
            .get("generation")
            .and_then(|v| v.as_u64())
            .ok_or("handle missing generation")?,
    })
}

fn accessible_role_to_string(role: proto::AccessibleRole) -> &'static str {
    match role {
        proto::AccessibleRole::Unknown => "unknown",
        proto::AccessibleRole::Button => "button",
        proto::AccessibleRole::Checkbox => "checkbox",
        proto::AccessibleRole::Combobox => "combobox",
        proto::AccessibleRole::List => "list",
        proto::AccessibleRole::Slider => "slider",
        proto::AccessibleRole::Spinbox => "spinbox",
        proto::AccessibleRole::Tab => "tab",
        proto::AccessibleRole::TabList => "tab-list",
        proto::AccessibleRole::Text => "text",
        proto::AccessibleRole::Table => "table",
        proto::AccessibleRole::Tree => "tree",
        proto::AccessibleRole::ProgressIndicator => "progress-indicator",
        proto::AccessibleRole::TextInput => "text-input",
        proto::AccessibleRole::Switch => "switch",
        proto::AccessibleRole::ListItem => "list-item",
        proto::AccessibleRole::TabPanel => "tab-panel",
        proto::AccessibleRole::Groupbox => "groupbox",
        proto::AccessibleRole::Image => "image",
        proto::AccessibleRole::RadioButton => "radio-button",
    }
}

fn string_to_accessible_role(s: &str) -> Option<proto::AccessibleRole> {
    Some(match s {
        "unknown" => proto::AccessibleRole::Unknown,
        "button" => proto::AccessibleRole::Button,
        "checkbox" => proto::AccessibleRole::Checkbox,
        "combobox" => proto::AccessibleRole::Combobox,
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
        "groupbox" => proto::AccessibleRole::Groupbox,
        "image" => proto::AccessibleRole::Image,
        "radio-button" => proto::AccessibleRole::RadioButton,
        _ => return None,
    })
}

// ============================================================================
// MCP Tool Definitions
// ============================================================================

fn handle_schema(description: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "description": description,
        "properties": {
            "index": { "type": "integer" },
            "generation": { "type": "integer" }
        },
        "required": ["index", "generation"]
    })
}

fn tool_definitions() -> serde_json::Value {
    let wh = handle_schema("Window handle from list_windows");
    let wh_short = handle_schema("Window handle");
    let eh = handle_schema("Element handle");

    serde_json::json!({
        "tools": [
            {
                "name": "list_windows",
                "description": "List all windows in the connected Slint application. Returns window handles that can be used with other tools. This is typically the first tool to call.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_window_properties",
                "description": "Get properties of a window (size, position, fullscreen/maximized/minimized state, root element handle). The returned root_element_handle is the entry point for get_element_tree, get_element_properties, and query_element_descendants.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "window_handle": wh },
                    "required": ["window_handle"]
                }
            },
            {
                "name": "find_elements_by_id",
                "description": "Find elements by their qualified ID (e.g., 'App::mybutton'). Returns element handles. Use get_element_tree first to discover available element IDs.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "window_handle": wh_short.clone(),
                        "element_id": {
                            "type": "string",
                            "description": "Qualified element ID (e.g., 'App::mybutton')"
                        }
                    },
                    "required": ["window_handle", "element_id"]
                }
            },
            {
                "name": "get_element_properties",
                "description": "Get all properties of an element: type info, accessible properties (label, value, description, role, checked, enabled, etc.), geometry (position, size), and opacity.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "element_handle": eh.clone() },
                    "required": ["element_handle"]
                }
            },
            {
                "name": "query_element_descendants",
                "description": "Query descendants of an element using a chain of match instructions. Each instruction narrows the search. Use match_descendants to search recursively, then filter by id, type_name, or accessible_role. More efficient than get_element_tree for targeted searches.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "element_handle": handle_schema("Element handle to start the query from"),
                        "query": {
                            "type": "array",
                            "description": "Array of query instructions, applied in order",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "match_descendants": { "type": "boolean", "description": "If true, search recursively through all descendants" },
                                    "match_id": { "type": "string", "description": "Match elements by ID" },
                                    "match_type_name": { "type": "string", "description": "Match elements by type name (e.g., 'Button')" },
                                    "match_type_name_or_base": { "type": "string", "description": "Match elements by type name or inherited base type" },
                                    "match_accessible_role": { "type": "string", "description": "Match by accessible role (e.g., 'button', 'text', 'slider')" }
                                }
                            }
                        },
                        "find_all": {
                            "type": "boolean",
                            "description": "If true, return all matches. If false, return only the first match.",
                            "default": true
                        }
                    },
                    "required": ["element_handle", "query"]
                }
            },
            {
                "name": "take_screenshot",
                "description": "Take a screenshot of a window. Returns an MCP image content block that clients can render inline.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "window_handle": wh_short.clone() },
                    "required": ["window_handle"]
                }
            },
            {
                "name": "click_element",
                "description": "Simulate a mouse click on an element.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "element_handle": eh.clone(),
                        "action": {
                            "type": "string",
                            "enum": ["single_click", "double_click"],
                            "default": "single_click"
                        },
                        "button": {
                            "type": "string",
                            "enum": ["left", "right", "middle"],
                            "default": "left"
                        }
                    },
                    "required": ["element_handle"]
                }
            },
            {
                "name": "invoke_accessibility_action",
                "description": "Invoke an accessibility action on an element (e.g., default action for buttons, increment/decrement for sliders).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "element_handle": eh.clone(),
                        "action": {
                            "type": "string",
                            "enum": ["default", "increment", "decrement", "expand"],
                            "description": "The accessibility action to invoke"
                        }
                    },
                    "required": ["element_handle", "action"]
                }
            },
            {
                "name": "set_element_value",
                "description": "Set the accessible value of an element (e.g., text input content, slider value).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "element_handle": eh,
                        "value": {
                            "type": "string",
                            "description": "The value to set"
                        }
                    },
                    "required": ["element_handle", "value"]
                }
            },
            {
                "name": "get_element_tree",
                "description": "Get the full element tree starting from a root element. Returns a hierarchical JSON structure with all element properties. Start with max_depth=2 or 3 for an overview, then drill deeper as needed.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "element_handle": handle_schema("Root element handle (typically from get_window_properties root_element_handle)"),
                        "max_depth": {
                            "type": "integer",
                            "description": "Maximum depth to traverse (default: 10)",
                            "default": 10
                        }
                    },
                    "required": ["element_handle"]
                }
            },
            {
                "name": "dispatch_key_event",
                "description": "Dispatch a keyboard event to a window.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "window_handle": wh_short,
                        "text": {
                            "type": "string",
                            "description": "The key text to send"
                        },
                        "event_type": {
                            "type": "string",
                            "enum": ["press", "release", "press_and_release"],
                            "default": "press_and_release",
                            "description": "Type of key event"
                        }
                    },
                    "required": ["window_handle", "text"]
                }
            }
        ]
    })
}

// ============================================================================
// MCP Request Handler
// ============================================================================

struct McpServer {
    state: Mutex<ServerState>,
    port: u16,
}

impl McpServer {
    fn new(port: u16) -> Self {
        Self {
            state: Mutex::new(ServerState {
                listener: None,
                connection: None,
            }),
            port,
        }
    }

    async fn ensure_listening(&self) -> Result<(), String> {
        let mut state = self.state.lock().await;
        if state.listener.is_none() {
            let addr = format!("127.0.0.1:{}", self.port);
            let listener = TcpListener::bind(&addr)
                .await
                .map_err(|e| format!("Failed to bind to {addr}: {e}"))?;
            eprintln!("Listening on {addr} for Slint application connection...");
            eprintln!(
                "Launch your Slint app with: SLINT_TEST_SERVER={addr} ./your-app"
            );
            state.listener = Some(listener);
        }
        Ok(())
    }

    async fn send_request(
        &self,
        request: proto::RequestToAUT,
    ) -> Result<proto::AUTResponse, String> {
        // Ensure we have a connection, accepting one if needed.
        // We take the listener out of the mutex before calling accept()
        // so the lock is not held across the await point.
        loop {
            let mut state = self.state.lock().await;
            if state.connection.is_some() {
                break;
            }
            if state.listener.is_none() {
                drop(state);
                self.ensure_listening().await?;
                continue;
            }
            // Take the listener so we can call accept() without holding the lock.
            // If another task already took it, loop and re-check.
            let Some(listener) = state.listener.take() else {
                drop(state);
                continue;
            };
            drop(state);

            eprintln!("Waiting for Slint application to connect...");
            let accept_result: std::io::Result<(tokio::net::TcpStream, std::net::SocketAddr)> =
                listener.accept().await;

            // Put the listener back regardless of accept outcome
            let mut state = self.state.lock().await;
            state.listener = Some(listener);

            let (stream, addr) =
                accept_result.map_err(|e| format!("Failed to accept connection: {e}"))?;
            stream
                .set_nodelay(true)
                .map_err(|e| format!("Failed to set nodelay: {e}"))?;
            eprintln!("Slint application connected from {addr}");
            state.connection = Some(SlintConnection { stream });
            break;
        }

        let mut state = self.state.lock().await;
        let conn = state.connection.as_mut().unwrap();
        match conn.send_request(request).await {
            Ok(resp) => Ok(resp),
            Err(e) => {
                state.connection = None;
                Err(format!("Connection lost: {e}"))
            }
        }
    }

    async fn handle_tool_call(
        &self,
        name: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match name {
            "list_windows" => self.tool_list_windows().await,
            "get_window_properties" => self.tool_get_window_properties(args).await,
            "find_elements_by_id" => self.tool_find_elements_by_id(args).await,
            "get_element_properties" => self.tool_get_element_properties(args).await,
            "query_element_descendants" => self.tool_query_element_descendants(args).await,
            "take_screenshot" => self.tool_take_screenshot(args).await,
            "click_element" => self.tool_click_element(args).await,
            "invoke_accessibility_action" => self.tool_invoke_accessibility_action(args).await,
            "set_element_value" => self.tool_set_element_value(args).await,
            "get_element_tree" => self.tool_get_element_tree(args).await,
            "dispatch_key_event" => self.tool_dispatch_key_event(args).await,
            _ => Err(format!("Unknown tool: {name}")),
        }
    }

    // --- Tool implementations ---

    async fn tool_list_windows(&self) -> Result<serde_json::Value, String> {
        let request = proto::RequestToAUT {
            msg: proto::mod_RequestToAUT::OneOfmsg::request_window_list(
                proto::RequestWindowListMessage {},
            ),
        };
        let response = self.send_request(request).await?;
        check_error(&response)?;

        match response.msg {
            proto::mod_AUTResponse::OneOfmsg::window_list(wl) => {
                let handles: Vec<HandleJson> =
                    wl.window_handles.iter().map(HandleJson::from).collect();
                Ok(serde_json::json!({ "windows": handles }))
            }
            _ => Err("Unexpected response type".into()),
        }
    }

    async fn tool_get_window_properties(
        &self,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let wh = handle_from_json(
            args.get("window_handle")
                .ok_or("missing window_handle")?,
        )?;
        let request = proto::RequestToAUT {
            msg: proto::mod_RequestToAUT::OneOfmsg::request_window_properties(
                proto::RequestWindowProperties {
                    window_handle: Some(wh),
                },
            ),
        };
        let response = self.send_request(request).await?;
        check_error(&response)?;

        match response.msg {
            proto::mod_AUTResponse::OneOfmsg::window_properties(wp) => {
                let size = wp.size.unwrap_or_default();
                let pos = wp.position.unwrap_or_default();
                let root = wp.root_element_handle.as_ref().map(HandleJson::from);
                Ok(serde_json::json!({
                    "is_fullscreen": wp.is_fullscreen,
                    "is_maximized": wp.is_maximized,
                    "is_minimized": wp.is_minimized,
                    "size": { "width": size.width, "height": size.height },
                    "position": { "x": pos.x, "y": pos.y },
                    "root_element_handle": root
                }))
            }
            _ => Err("Unexpected response type".into()),
        }
    }

    async fn tool_find_elements_by_id(
        &self,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let wh = handle_from_json(
            args.get("window_handle")
                .ok_or("missing window_handle")?,
        )?;
        let element_id = args
            .get("element_id")
            .and_then(|v| v.as_str())
            .ok_or("missing element_id")?;
        let request = proto::RequestToAUT {
            msg: proto::mod_RequestToAUT::OneOfmsg::request_find_elements_by_id(
                proto::RequestFindElementsById {
                    window_handle: Some(wh),
                    elements_id: element_id.to_string(),
                },
            ),
        };
        let response = self.send_request(request).await?;
        check_error(&response)?;

        match response.msg {
            proto::mod_AUTResponse::OneOfmsg::elements(el) => {
                let handles: Vec<HandleJson> =
                    el.element_handles.iter().map(HandleJson::from).collect();
                Ok(serde_json::json!({ "elements": handles }))
            }
            _ => Err("Unexpected response type".into()),
        }
    }

    async fn tool_get_element_properties(
        &self,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let eh = handle_from_json(
            args.get("element_handle")
                .ok_or("missing element_handle")?,
        )?;
        let request = proto::RequestToAUT {
            msg: proto::mod_RequestToAUT::OneOfmsg::request_element_properties(
                proto::RequestElementProperties {
                    element_handle: Some(eh),
                },
            ),
        };
        let response = self.send_request(request).await?;
        check_error(&response)?;

        match response.msg {
            proto::mod_AUTResponse::OneOfmsg::element_properties(ep) => {
                Ok(element_properties_to_json(&ep))
            }
            _ => Err("Unexpected response type".into()),
        }
    }

    async fn tool_query_element_descendants(
        &self,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let eh = handle_from_json(
            args.get("element_handle")
                .ok_or("missing element_handle")?,
        )?;
        let query_arr = args
            .get("query")
            .and_then(|v| v.as_array())
            .ok_or("missing query array")?;
        let find_all = args
            .get("find_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let mut query_stack = Vec::new();
        for instruction in query_arr {
            let instr = if instruction
                .get("match_descendants")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                proto::mod_ElementQueryInstruction::OneOfinstruction::match_descendants(true)
            } else if let Some(id) = instruction.get("match_id").and_then(|v| v.as_str()) {
                proto::mod_ElementQueryInstruction::OneOfinstruction::match_element_id(
                    id.to_string(),
                )
            } else if let Some(tn) =
                instruction.get("match_type_name").and_then(|v| v.as_str())
            {
                proto::mod_ElementQueryInstruction::OneOfinstruction::match_element_type_name(
                    tn.to_string(),
                )
            } else if let Some(tn) = instruction
                .get("match_type_name_or_base")
                .and_then(|v| v.as_str())
            {
                proto::mod_ElementQueryInstruction::OneOfinstruction::match_element_type_name_or_base(
                    tn.to_string(),
                )
            } else if let Some(role_str) = instruction
                .get("match_accessible_role")
                .and_then(|v| v.as_str())
            {
                let role = string_to_accessible_role(role_str)
                    .ok_or_else(|| format!("Unknown accessible role: {role_str}"))?;
                proto::mod_ElementQueryInstruction::OneOfinstruction::match_element_accessible_role(
                    role,
                )
            } else {
                return Err("Invalid query instruction".into());
            };
            query_stack.push(proto::ElementQueryInstruction { instruction: instr });
        }

        let request = proto::RequestToAUT {
            msg: proto::mod_RequestToAUT::OneOfmsg::request_query_element_descendants(
                proto::RequestQueryElementDescendants {
                    element_handle: Some(eh),
                    query_stack,
                    find_all,
                },
            ),
        };
        let response = self.send_request(request).await?;
        check_error(&response)?;

        match response.msg {
            proto::mod_AUTResponse::OneOfmsg::element_query_response(eq) => {
                let handles: Vec<HandleJson> =
                    eq.element_handles.iter().map(HandleJson::from).collect();
                Ok(serde_json::json!({ "elements": handles }))
            }
            _ => Err("Unexpected response type".into()),
        }
    }

    async fn tool_take_screenshot(
        &self,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let wh = handle_from_json(
            args.get("window_handle")
                .ok_or("missing window_handle")?,
        )?;
        let request = proto::RequestToAUT {
            msg: proto::mod_RequestToAUT::OneOfmsg::request_take_snapshot(
                proto::RequestTakeSnapshot {
                    window_handle: Some(wh),
                    image_mime_type: String::new(), // defaults to PNG
                },
            ),
        };
        let response = self.send_request(request).await?;
        check_error(&response)?;

        match response.msg {
            proto::mod_AUTResponse::OneOfmsg::take_snapshot_response(snap) => {
                let b64 = base64::engine::general_purpose::STANDARD
                    .encode(&snap.window_contents_as_encoded_image);
                // Return as MCP image content type so clients can render inline
                Ok(serde_json::json!({
                    "_mcp_image": {
                        "data": b64,
                        "mimeType": "image/png"
                    },
                    "size_bytes": snap.window_contents_as_encoded_image.len()
                }))
            }
            _ => Err("Unexpected response type".into()),
        }
    }

    async fn tool_click_element(
        &self,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let eh = handle_from_json(
            args.get("element_handle")
                .ok_or("missing element_handle")?,
        )?;
        let action_str = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("single_click");
        let button_str = args
            .get("button")
            .and_then(|v| v.as_str())
            .unwrap_or("left");

        let action = match action_str {
            "single_click" => proto::ClickAction::SingleClick,
            "double_click" => proto::ClickAction::DoubleClick,
            _ => return Err(format!("Unknown click action: {action_str}")),
        };
        let button = match button_str {
            "left" => proto::PointerEventButton::Left,
            "right" => proto::PointerEventButton::Right,
            "middle" => proto::PointerEventButton::Middle,
            _ => return Err(format!("Unknown button: {button_str}")),
        };

        let request = proto::RequestToAUT {
            msg: proto::mod_RequestToAUT::OneOfmsg::request_element_click(
                proto::RequestElementClick {
                    element_handle: Some(eh),
                    action,
                    button,
                },
            ),
        };
        let response = self.send_request(request).await?;
        check_error(&response)?;
        Ok(serde_json::json!({ "success": true }))
    }

    async fn tool_invoke_accessibility_action(
        &self,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let eh = handle_from_json(
            args.get("element_handle")
                .ok_or("missing element_handle")?,
        )?;
        let action_str = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("missing action")?;

        let action = match action_str {
            "default" => proto::ElementAccessibilityAction::Default_,
            "increment" => proto::ElementAccessibilityAction::Increment,
            "decrement" => proto::ElementAccessibilityAction::Decrement,
            "expand" => proto::ElementAccessibilityAction::Expand,
            _ => return Err(format!("Unknown action: {action_str}")),
        };

        let request = proto::RequestToAUT {
            msg: proto::mod_RequestToAUT::OneOfmsg::request_invoke_element_accessibility_action(
                proto::RequestInvokeElementAccessibilityAction {
                    element_handle: Some(eh),
                    action,
                },
            ),
        };
        let response = self.send_request(request).await?;
        check_error(&response)?;
        Ok(serde_json::json!({ "success": true }))
    }

    async fn tool_set_element_value(
        &self,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let eh = handle_from_json(
            args.get("element_handle")
                .ok_or("missing element_handle")?,
        )?;
        let value = args
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or("missing value")?;

        let request = proto::RequestToAUT {
            msg: proto::mod_RequestToAUT::OneOfmsg::request_set_element_accessible_value(
                proto::RequestSetElementAccessibleValue {
                    element_handle: Some(eh),
                    value: value.to_string(),
                },
            ),
        };
        let response = self.send_request(request).await?;
        check_error(&response)?;
        Ok(serde_json::json!({ "success": true }))
    }

    async fn tool_get_element_tree(
        &self,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let eh = handle_from_json(
            args.get("element_handle")
                .ok_or("missing element_handle")?,
        )?;
        let max_depth = args
            .get("max_depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .min(50) as usize;

        self.build_element_tree(eh, max_depth).await
    }

    async fn build_element_tree(
        &self,
        root: proto::Handle,
        max_depth: usize,
    ) -> Result<serde_json::Value, String> {
        use std::collections::VecDeque;

        // Each entry: (handle, depth, node_index). node_index is the position
        // in `nodes` where the finished JSON will be stored.
        let mut queue: VecDeque<(proto::Handle, usize, usize)> = VecDeque::new();
        // Parallel vectors: nodes[i] holds the JSON value, parent_of[i] holds
        // the index of its parent (None for the root).
        let mut nodes: Vec<serde_json::Value> = Vec::new();
        let mut parent_of: Vec<Option<usize>> = Vec::new();

        // Seed with the root element
        let root_idx = 0;
        nodes.push(serde_json::Value::Null); // placeholder
        parent_of.push(None);
        queue.push_back((root, 0, root_idx));

        while let Some((handle, depth, node_idx)) = queue.pop_front() {
            // Fetch properties
            let props_request = proto::RequestToAUT {
                msg: proto::mod_RequestToAUT::OneOfmsg::request_element_properties(
                    proto::RequestElementProperties {
                        element_handle: Some(handle.clone()),
                    },
                ),
            };
            let props_response = self.send_request(props_request).await?;
            check_error(&props_response)?;

            let mut node = match props_response.msg {
                proto::mod_AUTResponse::OneOfmsg::element_properties(ep) => {
                    element_properties_to_json(&ep)
                }
                _ => return Err("Unexpected response type for element properties".into()),
            };

            node.as_object_mut().unwrap().insert(
                "handle".to_string(),
                serde_json::to_value(HandleJson::from(&handle)).unwrap(),
            );

            nodes[node_idx] = node;

            // Fetch children if within depth limit
            if depth < max_depth {
                let children_request = proto::RequestToAUT {
                    msg: proto::mod_RequestToAUT::OneOfmsg::request_query_element_descendants(
                        proto::RequestQueryElementDescendants {
                            element_handle: Some(handle),
                            query_stack: vec![],
                            find_all: true,
                        },
                    ),
                };
                let children_response = self.send_request(children_request).await?;
                check_error(&children_response)?;

                if let proto::mod_AUTResponse::OneOfmsg::element_query_response(eq) =
                    children_response.msg
                {
                    for child_handle in eq.element_handles {
                        let child_idx = nodes.len();
                        nodes.push(serde_json::Value::Null); // placeholder
                        parent_of.push(Some(node_idx));
                        queue.push_back((child_handle, depth + 1, child_idx));
                    }
                }
            }
        }

        // Assemble tree bottom-up: attach each node to its parent's "children" array
        for i in (1..nodes.len()).rev() {
            let child = nodes[i].take();
            if child.is_null() {
                continue; // skipped node
            }
            if let Some(parent_idx) = parent_of[i] {
                let parent = nodes[parent_idx].as_object_mut().unwrap();
                parent
                    .entry("children")
                    .or_insert_with(|| serde_json::Value::Array(Vec::new()))
                    .as_array_mut()
                    .unwrap()
                    .push(child);
            }
        }

        // Reverse children arrays since we assembled bottom-up (last child first)
        for node in &mut nodes {
            if let Some(obj) = node.as_object_mut() {
                if let Some(children) = obj.get_mut("children") {
                    if let Some(arr) = children.as_array_mut() {
                        arr.reverse();
                    }
                }
            }
        }

        Ok(nodes.swap_remove(0))
    }

    async fn tool_dispatch_key_event(
        &self,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let wh = handle_from_json(
            args.get("window_handle")
                .ok_or("missing window_handle")?,
        )?;
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or("missing text")?
            .to_string();
        let event_type = args
            .get("event_type")
            .and_then(|v| v.as_str())
            .unwrap_or("press_and_release");

        let events: Vec<proto::WindowEvent> = match event_type {
            "press" => vec![proto::WindowEvent {
                event: proto::mod_WindowEvent::OneOfevent::key_pressed(proto::KeyPressedEvent {
                    text: text.clone(),
                }),
            }],
            "release" => vec![proto::WindowEvent {
                event: proto::mod_WindowEvent::OneOfevent::key_released(proto::KeyReleasedEvent {
                    text: text.clone(),
                }),
            }],
            "press_and_release" => vec![
                proto::WindowEvent {
                    event: proto::mod_WindowEvent::OneOfevent::key_pressed(
                        proto::KeyPressedEvent {
                            text: text.clone(),
                        },
                    ),
                },
                proto::WindowEvent {
                    event: proto::mod_WindowEvent::OneOfevent::key_released(
                        proto::KeyReleasedEvent {
                            text: text.clone(),
                        },
                    ),
                },
            ],
            _ => return Err(format!("Unknown event_type: {event_type}")),
        };

        for event in events {
            let request = proto::RequestToAUT {
                msg: proto::mod_RequestToAUT::OneOfmsg::request_dispatch_window_event(
                    proto::RequestDispatchWindowEvent {
                        window_handle: Some(wh.clone()),
                        event: Some(event),
                    },
                ),
            };
            let response = self.send_request(request).await?;
            check_error(&response)?;
        }

        Ok(serde_json::json!({ "success": true }))
    }

    async fn handle_mcp_request(
        &self,
        request: &JsonRpcRequest,
    ) -> JsonRpcResponse {
        let id = request.id.clone().unwrap_or(serde_json::Value::Null);

        match request.method.as_str() {
            "initialize" => JsonRpcResponse::success(
                id,
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "slint-mcp-server",
                        "version": "0.1.0"
                    },
                    "instructions": concat!(
                        "This server connects to a running Slint application and lets you inspect and interact with its UI. ",
                        "The app must be built with `--features system-testing` and `SLINT_EMIT_DEBUG_INFO=1`, ",
                        "then launched with `SLINT_TEST_SERVER=localhost:<port>` so it connects to this server.\n\n",
                        "Recommended workflow:\n",
                        "1. list_windows — get window handles\n",
                        "2. get_window_properties — get the root_element_handle\n",
                        "3. get_element_tree (max_depth=2-3) — explore the UI hierarchy\n",
                        "4. Use find_elements_by_id or query_element_descendants for targeted lookups\n",
                        "5. get_element_properties — inspect specific elements\n",
                        "6. take_screenshot — see the current visual state\n\n",
                        "Handles (window_handle, element_handle) are {index, generation} objects returned by the tools above. ",
                        "They remain valid as long as the app is connected and the UI element exists."
                    )
                }),
            ),
            "notifications/initialized" => {
                JsonRpcResponse::success(id, serde_json::json!({}))
            }
            "tools/list" => {
                let tools = tool_definitions();
                JsonRpcResponse::success(id, tools)
            }
            "tools/call" => {
                let tool_name = request
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let args = request
                    .params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                match self.handle_tool_call(tool_name, &args).await {
                    Ok(result) => {
                        // If the result contains an MCP image, return it as an
                        // image content block so clients can render it inline.
                        let content = if let Some(img) = result.get("_mcp_image") {
                            let mut blocks = vec![serde_json::json!({
                                "type": "image",
                                "data": img.get("data").and_then(|v| v.as_str()).unwrap_or(""),
                                "mimeType": img.get("mimeType").and_then(|v| v.as_str()).unwrap_or("image/png")
                            })];
                            // Include non-image metadata as a text block
                            let mut meta = result.clone();
                            meta.as_object_mut().unwrap().remove("_mcp_image");
                            if !meta.as_object().unwrap().is_empty() {
                                blocks.push(serde_json::json!({
                                    "type": "text",
                                    "text": serde_json::to_string_pretty(&meta).unwrap()
                                }));
                            }
                            blocks
                        } else {
                            vec![serde_json::json!({
                                "type": "text",
                                "text": serde_json::to_string_pretty(&result).unwrap()
                            })]
                        };
                        JsonRpcResponse::success(
                            id,
                            serde_json::json!({ "content": content }),
                        )
                    }
                    Err(e) => JsonRpcResponse::success(
                        id,
                        serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": format!("Error: {e}")
                            }],
                            "isError": true
                        }),
                    ),
                }
            }
            _ => JsonRpcResponse::error(id, -32601, format!("Method not found: {}", request.method)),
        }
    }
}

fn check_error(response: &proto::AUTResponse) -> Result<(), String> {
    if let proto::mod_AUTResponse::OneOfmsg::error(ref e) = response.msg {
        return Err(e.message.clone());
    }
    Ok(())
}

fn element_properties_to_json(ep: &proto::ElementPropertiesResponse) -> serde_json::Value {
    let type_info: Vec<serde_json::Value> = ep
        .type_names_and_ids
        .iter()
        .map(|t| {
            serde_json::json!({
                "type_name": t.type_name,
                "id": t.id
            })
        })
        .collect();

    let size = ep.size.as_ref().map(|s| serde_json::json!({"width": s.width, "height": s.height}));
    let pos = ep
        .absolute_position
        .as_ref()
        .map(|p| serde_json::json!({"x": p.x, "y": p.y}));

    serde_json::json!({
        "type_info": type_info,
        "accessible_role": accessible_role_to_string(ep.accessible_role),
        "accessible_label": if ep.accessible_label.is_empty() { None } else { Some(&ep.accessible_label) },
        "accessible_value": if ep.accessible_value.is_empty() { None } else { Some(&ep.accessible_value) },
        "accessible_description": if ep.accessible_description.is_empty() { None } else { Some(&ep.accessible_description) },
        "accessible_placeholder_text": if ep.accessible_placeholder_text.is_empty() { None } else { Some(&ep.accessible_placeholder_text) },
        "accessible_checked": ep.accessible_checked,
        "accessible_checkable": ep.accessible_checkable,
        "accessible_enabled": ep.accessible_enabled,
        "accessible_read_only": ep.accessible_read_only,
        "accessible_value_minimum": ep.accessible_value_minimum,
        "accessible_value_maximum": ep.accessible_value_maximum,
        "accessible_value_step": ep.accessible_value_step,
        "size": size,
        "absolute_position": pos,
        "computed_opacity": ep.computed_opacity
    })
}

/// Try to parse a JSON-RPC request from a line. On failure, return a -32700
/// parse-error response that should be sent back to the client.
fn try_parse_request(line: &str) -> Result<JsonRpcRequest, JsonRpcResponse> {
    serde_json::from_str(line).map_err(|e| {
        JsonRpcResponse::error(
            serde_json::Value::Null,
            -32700,
            format!("Parse error: {e}"),
        )
    })
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    let port = std::env::args()
        .skip_while(|a| a != "--port")
        .nth(1)
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(4242);

    let server = std::sync::Arc::new(McpServer::new(port));

    // Start listening immediately so the port is ready
    if let Err(e) = server.ensure_listening().await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    // Read MCP requests from stdin, write responses to stdout.
    // Shutdown gracefully on ctrl-c or stdin EOF.
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();
    let mut stdout = stdout;

    loop {
        let line = tokio::select! {
            result = lines.next_line() => {
                match result {
                    Ok(Some(line)) => line,
                    // stdin closed or error — shut down
                    _ => break,
                }
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("Received interrupt, shutting down.");
                break;
            }
        };

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match try_parse_request(&line) {
            Ok(r) => r,
            Err(error_response) => {
                eprintln!("Failed to parse JSON-RPC request");
                let response_json = serde_json::to_string(&error_response).unwrap();
                let output = format!("{response_json}\n");
                let _ = stdout.write_all(output.as_bytes()).await;
                let _ = stdout.flush().await;
                continue;
            }
        };

        // JSON-RPC 2.0: notifications have no "id" field — don't send a response
        if request.id.is_none() {
            server.handle_mcp_request(&request).await;
            continue;
        }

        let response = server.handle_mcp_request(&request).await;
        let response_json = serde_json::to_string(&response).unwrap();
        let output = format!("{response_json}\n");
        if stdout.write_all(output.as_bytes()).await.is_err() {
            break;
        }
        if stdout.flush().await.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ====================================================================
    // handle_from_json
    // ====================================================================

    #[test]
    fn handle_from_json_valid() {
        let v = json!({"index": 3, "generation": 7});
        let h = handle_from_json(&v).unwrap();
        assert_eq!(h.index, 3);
        assert_eq!(h.generation, 7);
    }

    #[test]
    fn handle_from_json_zero_values() {
        let v = json!({"index": 0, "generation": 0});
        let h = handle_from_json(&v).unwrap();
        assert_eq!(h.index, 0);
        assert_eq!(h.generation, 0);
    }

    #[test]
    fn handle_from_json_not_object() {
        let v = json!("hello");
        let err = handle_from_json(&v).unwrap_err();
        assert!(err.contains("object"), "error was: {err}");
    }

    #[test]
    fn handle_from_json_missing_index() {
        let v = json!({"generation": 1});
        let err = handle_from_json(&v).unwrap_err();
        assert!(err.contains("index"), "error was: {err}");
    }

    #[test]
    fn handle_from_json_missing_generation() {
        let v = json!({"index": 1});
        let err = handle_from_json(&v).unwrap_err();
        assert!(err.contains("generation"), "error was: {err}");
    }

    #[test]
    fn handle_from_json_wrong_types() {
        let v = json!({"index": "foo", "generation": 1});
        assert!(handle_from_json(&v).is_err());

        let v = json!({"index": 1, "generation": true});
        assert!(handle_from_json(&v).is_err());
    }

    // ====================================================================
    // accessible_role_to_string / string_to_accessible_role round-trip
    // ====================================================================

    #[test]
    fn role_round_trip_all_variants() {
        let roles = [
            (proto::AccessibleRole::Unknown, "unknown"),
            (proto::AccessibleRole::Button, "button"),
            (proto::AccessibleRole::Checkbox, "checkbox"),
            (proto::AccessibleRole::Combobox, "combobox"),
            (proto::AccessibleRole::List, "list"),
            (proto::AccessibleRole::Slider, "slider"),
            (proto::AccessibleRole::Spinbox, "spinbox"),
            (proto::AccessibleRole::Tab, "tab"),
            (proto::AccessibleRole::TabList, "tab-list"),
            (proto::AccessibleRole::Text, "text"),
            (proto::AccessibleRole::Table, "table"),
            (proto::AccessibleRole::Tree, "tree"),
            (proto::AccessibleRole::ProgressIndicator, "progress-indicator"),
            (proto::AccessibleRole::TextInput, "text-input"),
            (proto::AccessibleRole::Switch, "switch"),
            (proto::AccessibleRole::ListItem, "list-item"),
            (proto::AccessibleRole::TabPanel, "tab-panel"),
            (proto::AccessibleRole::Groupbox, "groupbox"),
            (proto::AccessibleRole::Image, "image"),
            (proto::AccessibleRole::RadioButton, "radio-button"),
        ];

        assert_eq!(roles.len(), 20, "expected 20 accessible roles");

        for (role, expected_str) in &roles {
            let s = accessible_role_to_string(*role);
            assert_eq!(s, *expected_str, "to_string failed for {expected_str}");

            let back = string_to_accessible_role(s).unwrap();
            assert_eq!(
                std::mem::discriminant(&back),
                std::mem::discriminant(role),
                "round-trip failed for {expected_str}"
            );
        }
    }

    #[test]
    fn string_to_accessible_role_unknown_input() {
        assert!(string_to_accessible_role("nonexistent").is_none());
        assert!(string_to_accessible_role("").is_none());
        assert!(string_to_accessible_role("BUTTON").is_none()); // case sensitive
    }

    // ====================================================================
    // element_properties_to_json
    // ====================================================================

    fn default_element_properties() -> proto::ElementPropertiesResponse {
        proto::ElementPropertiesResponse::default()
    }

    #[test]
    fn element_properties_empty_strings_become_null() {
        let ep = default_element_properties();
        let j = element_properties_to_json(&ep);

        assert!(j["accessible_label"].is_null());
        assert!(j["accessible_value"].is_null());
        assert!(j["accessible_description"].is_null());
        assert!(j["accessible_placeholder_text"].is_null());
    }

    #[test]
    fn element_properties_non_empty_strings_preserved() {
        let mut ep = default_element_properties();
        ep.accessible_label = "OK".to_string();
        ep.accessible_value = "42".to_string();
        ep.accessible_description = "confirm".to_string();
        ep.accessible_placeholder_text = "enter text".to_string();

        let j = element_properties_to_json(&ep);
        assert_eq!(j["accessible_label"], "OK");
        assert_eq!(j["accessible_value"], "42");
        assert_eq!(j["accessible_description"], "confirm");
        assert_eq!(j["accessible_placeholder_text"], "enter text");
    }

    #[test]
    fn element_properties_zero_numeric_values() {
        let ep = default_element_properties();
        let j = element_properties_to_json(&ep);

        assert_eq!(j["accessible_value_minimum"], 0.0);
        assert_eq!(j["accessible_value_maximum"], 0.0);
        assert_eq!(j["accessible_value_step"], 0.0);
        assert_eq!(j["computed_opacity"], 0.0);
        assert_eq!(j["accessible_checked"], false);
        assert_eq!(j["accessible_checkable"], false);
    }

    #[test]
    fn element_properties_complete_response() {
        let mut ep = default_element_properties();
        ep.type_names_and_ids = vec![proto::ElementTypeNameAndId {
            type_name: "Button".to_string(),
            id: "App::ok_btn".to_string(),
        }];
        ep.accessible_role = proto::AccessibleRole::Button;
        ep.accessible_label = "OK".to_string();
        ep.accessible_enabled = true;
        ep.size = Some(proto::LogicalSize {
            width: 100.0,
            height: 40.0,
        });
        ep.absolute_position = Some(proto::LogicalPosition { x: 10.0, y: 20.0 });
        ep.computed_opacity = 1.0;

        let j = element_properties_to_json(&ep);
        assert_eq!(j["type_info"][0]["type_name"], "Button");
        assert_eq!(j["type_info"][0]["id"], "App::ok_btn");
        assert_eq!(j["accessible_role"], "button");
        assert_eq!(j["accessible_label"], "OK");
        assert_eq!(j["accessible_enabled"], true);
        assert_eq!(j["size"]["width"], 100.0);
        assert_eq!(j["size"]["height"], 40.0);
        assert_eq!(j["absolute_position"]["x"], 10.0);
        assert_eq!(j["absolute_position"]["y"], 20.0);
        assert_eq!(j["computed_opacity"], 1.0);
    }

    #[test]
    fn element_properties_no_size_or_position() {
        let ep = default_element_properties();
        let j = element_properties_to_json(&ep);

        assert!(j["size"].is_null());
        assert!(j["absolute_position"].is_null());
    }

    // ====================================================================
    // tool_definitions
    // ====================================================================

    #[test]
    fn tool_definitions_has_all_tools() {
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 11, "expected 11 tool definitions");

        let expected_names = [
            "list_windows",
            "get_window_properties",
            "find_elements_by_id",
            "get_element_properties",
            "query_element_descendants",
            "take_screenshot",
            "click_element",
            "invoke_accessibility_action",
            "set_element_value",
            "get_element_tree",
            "dispatch_key_event",
        ];
        let actual_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(actual_names, expected_names);
    }

    #[test]
    fn tool_definitions_have_required_fields() {
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().unwrap();

        for tool in tools {
            assert!(
                tool["name"].is_string(),
                "tool missing name: {tool}"
            );
            assert!(
                tool["description"].is_string(),
                "tool {} missing description",
                tool["name"]
            );
            assert!(
                tool["inputSchema"].is_object(),
                "tool {} missing inputSchema",
                tool["name"]
            );
            assert_eq!(
                tool["inputSchema"]["type"], "object",
                "tool {} inputSchema type must be 'object'",
                tool["name"]
            );
        }
    }

    // ====================================================================
    // MCP request handling (JSON-RPC level)
    // ====================================================================

    #[tokio::test]
    async fn mcp_unknown_method_returns_32601() {
        let server = McpServer::new(0);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(1)),
            method: "nonexistent/method".into(),
            params: json!({}),
        };
        let response = server.handle_mcp_request(&request).await;
        let err = response.error.unwrap();
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("nonexistent/method"));
    }

    #[tokio::test]
    async fn mcp_initialize_returns_capabilities() {
        let server = McpServer::new(0);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(1)),
            method: "initialize".into(),
            params: json!({}),
        };
        let response = server.handle_mcp_request(&request).await;
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert_eq!(result["serverInfo"]["name"], "slint-mcp-server");
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[tokio::test]
    async fn mcp_tools_list_returns_all_tools() {
        let server = McpServer::new(0);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(1)),
            method: "tools/list".into(),
            params: json!({}),
        };
        let response = server.handle_mcp_request(&request).await;
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert_eq!(result["tools"].as_array().unwrap().len(), 11);
    }

    #[tokio::test]
    async fn mcp_unknown_tool_returns_error_content() {
        let server = McpServer::new(0);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(1)),
            method: "tools/call".into(),
            params: json!({"name": "bogus_tool", "arguments": {}}),
        };
        let response = server.handle_mcp_request(&request).await;
        // Unknown tools return a success JSON-RPC response with isError in the content
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Unknown tool"), "error text was: {text}");
    }

    // ====================================================================
    // JSON-RPC parse error handling
    // ====================================================================

    #[test]
    fn parse_error_produces_32700() {
        let err_response = try_parse_request("this is not valid json{{{").unwrap_err();
        let err = err_response.error.as_ref().unwrap();
        assert_eq!(err.code, -32700);
        assert!(err.message.contains("Parse error"));
        assert!(err_response.id.is_null());

        // Verify it serializes correctly
        let json_str = serde_json::to_string(&err_response).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], -32700);
    }

    // ====================================================================
    // HandleJson / From<&proto::Handle>
    // ====================================================================

    #[test]
    fn handle_json_from_proto() {
        let proto_handle = proto::Handle {
            index: 5,
            generation: 42,
        };
        let hj = HandleJson::from(&proto_handle);
        assert_eq!(hj.index, 5);
        assert_eq!(hj.generation, 42);

        // Verify serialization
        let j = serde_json::to_value(&hj).unwrap();
        assert_eq!(j["index"], 5);
        assert_eq!(j["generation"], 42);
    }

    // ====================================================================
    // JsonRpcResponse helpers
    // ====================================================================

    #[test]
    fn jsonrpc_response_success_format() {
        let resp = JsonRpcResponse::success(json!(42), json!({"ok": true}));
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, json!(42));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());

        // error field should be omitted in serialization
        let j = serde_json::to_value(&resp).unwrap();
        assert!(!j.as_object().unwrap().contains_key("error"));
    }

    #[test]
    fn jsonrpc_response_error_format() {
        let resp = JsonRpcResponse::error(json!(1), -32600, "Invalid Request".into());
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_none());
        let err = resp.error.as_ref().unwrap();
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid Request");

        // result field should be omitted in serialization
        let j = serde_json::to_value(&resp).unwrap();
        assert!(!j.as_object().unwrap().contains_key("result"));
    }
}
