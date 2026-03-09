// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Embedded MCP (Model Context Protocol) server for introspecting Slint applications.
//!
//! When enabled via the `SLINT_MCP_PORT` environment variable, the application starts
//! an HTTP server implementing MCP's Streamable HTTP transport, allowing MCP clients
//! (e.g. Claude) to inspect and interact with the running UI.
//!
//! # Usage
//!
//! Build with the `mcp` feature and set the environment variable:
//! ```sh
//! SLINT_MCP_PORT=8080 ./your-slint-app
//! ```

use base64::Engine;
use futures_lite::{AsyncReadExt, AsyncWriteExt};
use i_slint_core::api::EventLoopError;
use serde::Deserialize;
use serde_json::Value;
use std::rc::Rc;

use crate::introspection::{self, AccessibilityAction, IntrospectionState, QueryInstruction};

// ============================================================================
// Handle serialization
// ============================================================================

/// An arena handle as sent over JSON: `{ "index": u64, "generation": u64 }`.
#[derive(Deserialize)]
struct Handle {
    index: u64,
    generation: u64,
}

impl Handle {
    fn to_index(&self) -> generational_arena::Index {
        introspection::parts_to_index(self.index, self.generation)
    }
}

fn handle_to_json(index: generational_arena::Index) -> Value {
    let (idx, generation) = introspection::index_to_parts(index);
    serde_json::json!({ "index": idx, "generation": generation })
}

// ============================================================================
// Tool parameter structs
// ============================================================================

#[derive(Deserialize)]
struct WindowHandleParam {
    window_handle: Handle,
}

#[derive(Deserialize)]
struct ElementHandleParam {
    element_handle: Handle,
}

#[derive(Deserialize)]
struct FindElementsByIdParams {
    window_handle: Handle,
    element_id: String,
}

#[derive(Deserialize)]
struct QueryElementDescendantsParams {
    element_handle: Handle,
    query: Vec<Value>,
    #[serde(default)]
    find_all: Option<bool>,
}

#[derive(Deserialize)]
struct GetElementTreeParams {
    element_handle: Handle,
    #[serde(default)]
    max_elements: Option<u64>,
}

#[derive(Deserialize)]
struct ClickElementParams {
    element_handle: Handle,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    button: Option<String>,
}

#[derive(Deserialize)]
struct InvokeAccessibilityActionParams {
    element_handle: Handle,
    action: String,
}

#[derive(Deserialize)]
struct SetElementValueParams {
    element_handle: Handle,
    value: String,
}

#[derive(Deserialize)]
struct DispatchKeyEventParams {
    window_handle: Handle,
    text: String,
    #[serde(default)]
    event_type: Option<String>,
}

// ============================================================================
// Tool definitions (schema for tools/list)
// ============================================================================

const HANDLE_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "index": { "type": "integer" },
        "generation": { "type": "integer" }
    },
    "required": ["index", "generation"]
}"#;

fn handle_schema(description: &str) -> Value {
    let mut schema: Value = serde_json::from_str(HANDLE_SCHEMA).unwrap();
    schema["description"] = Value::String(description.into());
    schema
}

fn tool_definitions() -> Value {
    serde_json::json!({
        "tools": [
            {
                "name": "list_windows",
                "description": "List all windows in the application. Returns window handles for use with other tools. This is typically the first tool to call.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "get_window_properties",
                "description": "Get properties of a window (size, position, fullscreen/maximized/minimized state, root element handle). The root_element_handle is the entry point for element tree traversal.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "window_handle": handle_schema("Window handle from list_windows.")
                    },
                    "required": ["window_handle"]
                }
            },
            {
                "name": "find_elements_by_id",
                "description": "Find elements by their qualified ID (e.g. 'App::mybutton'). Returns element handles. Use get_element_tree first to discover available element IDs.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "window_handle": handle_schema("Window handle."),
                        "element_id": { "type": "string", "description": "Qualified element ID (e.g. 'App::mybutton')." }
                    },
                    "required": ["window_handle", "element_id"]
                }
            },
            {
                "name": "get_element_properties",
                "description": "Get all properties of an element: type info, accessible properties (label, value, description, role, checked, enabled, etc.), geometry (position, size), and opacity.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "element_handle": handle_schema("Element handle.")
                    },
                    "required": ["element_handle"]
                }
            },
            {
                "name": "query_element_descendants",
                "description": "Query descendants of an element using a chain of match instructions. Each instruction narrows the search. Use match_descendants to search recursively, then filter by id, type_name, or accessible_role. More efficient than get_element_tree for targeted searches.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "element_handle": handle_schema("Element handle to start the query from."),
                        "query": {
                            "type": "array",
                            "description": "Array of query instructions applied in order.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "match_descendants": { "type": "boolean", "description": "If true, search recursively through all descendants" },
                                    "match_id": { "type": "string", "description": "Match elements by ID" },
                                    "match_type_name": { "type": "string", "description": "Match elements by type name" },
                                    "match_type_name_or_base": { "type": "string", "description": "Match elements by type name or inherited base type" },
                                    "match_accessible_role": { "type": "string", "description": "Match by accessible role (e.g., 'button', 'text', 'slider')" }
                                }
                            }
                        },
                        "find_all": { "type": "boolean", "description": "If true (default), return all matches. If false, return only the first match." }
                    },
                    "required": ["element_handle", "query"]
                }
            },
            {
                "name": "get_element_tree",
                "description": "Get all elements in the subtree starting from a root element. Returns a flat list of elements with their properties and handles. Use this to get an overview of the UI, then use get_element_properties or query_element_descendants for targeted exploration. Limit the result count with max_elements (default: 200, max: 1000).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "element_handle": handle_schema("Root element handle (typically from get_window_properties root_element_handle)."),
                        "max_elements": { "type": "integer", "description": "Maximum number of elements to return (default: 200, max: 1000)." }
                    },
                    "required": ["element_handle"]
                }
            },
            {
                "name": "take_screenshot",
                "description": "Take a screenshot of a window. Returns an MCP image content block.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "window_handle": handle_schema("Window handle.")
                    },
                    "required": ["window_handle"]
                }
            },
            {
                "name": "click_element",
                "description": "Simulate a mouse click on an element.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "element_handle": handle_schema("Element handle."),
                        "action": { "type": "string", "description": "Click action: \"single_click\" (default) or \"double_click\"." },
                        "button": { "type": "string", "description": "Mouse button: \"left\" (default), \"right\", or \"middle\"." }
                    },
                    "required": ["element_handle"]
                }
            },
            {
                "name": "invoke_accessibility_action",
                "description": "Invoke an accessibility action on an element (e.g. default action for buttons, increment/decrement for sliders, expand for combo boxes).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "element_handle": handle_schema("Element handle."),
                        "action": { "type": "string", "description": "The action: \"default\", \"increment\", \"decrement\", or \"expand\"." }
                    },
                    "required": ["element_handle", "action"]
                }
            },
            {
                "name": "set_element_value",
                "description": "Set the accessible value of an element (e.g. text input content, slider value).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "element_handle": handle_schema("Element handle."),
                        "value": { "type": "string", "description": "The value to set." }
                    },
                    "required": ["element_handle", "value"]
                }
            },
            {
                "name": "dispatch_key_event",
                "description": "Dispatch a keyboard event to a window.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "window_handle": handle_schema("Window handle."),
                        "text": { "type": "string", "description": "The key text to send." },
                        "event_type": { "type": "string", "description": "Event type: \"press\", \"release\", or \"press_and_release\" (default)." }
                    },
                    "required": ["window_handle", "text"]
                }
            }
        ]
    })
}

// ============================================================================
// Tool dispatch
// ============================================================================

fn deserialize_params<T: serde::de::DeserializeOwned>(args: &Value) -> Result<T, String> {
    serde_json::from_value(args.clone()).map_err(|e| format!("Invalid parameters: {e}"))
}

/// Tool call result: either a JSON value (rendered as text) or an image with optional metadata.
enum ToolResult {
    Json(Value),
    Image { png_data: Vec<u8>, meta: Value },
}

async fn handle_tool_call(
    state: &IntrospectionState,
    name: &str,
    args: &Value,
) -> Result<ToolResult, String> {
    match name {
        "list_windows" => {
            let handles: Vec<Value> =
                state.window_handles().into_iter().map(handle_to_json).collect();
            Ok(ToolResult::Json(serde_json::json!({ "windows": handles })))
        }
        "get_window_properties" => {
            let p: WindowHandleParam = deserialize_params(args)?;
            let wp = state.window_properties(p.window_handle.to_index())?;
            Ok(ToolResult::Json(serde_json::json!({
                "is_fullscreen": wp.is_fullscreen,
                "is_maximized": wp.is_maximized,
                "is_minimized": wp.is_minimized,
                "size": { "width": wp.size.width, "height": wp.size.height },
                "position": { "x": wp.position.x, "y": wp.position.y },
                "root_element_handle": handle_to_json(wp.root_element_handle)
            })))
        }
        "find_elements_by_id" => {
            let p: FindElementsByIdParams = deserialize_params(args)?;
            let elements = state.find_elements_by_id(p.window_handle.to_index(), &p.element_id)?;
            let handles: Vec<Value> =
                elements.into_iter().map(|e| handle_to_json(state.element_to_handle(e))).collect();
            Ok(ToolResult::Json(serde_json::json!({ "elements": handles })))
        }
        "get_element_properties" => {
            let p: ElementHandleParam = deserialize_params(args)?;
            let element = state.element("get_element_properties", p.element_handle.to_index())?;
            Ok(ToolResult::Json(element_properties_to_json(&state.element_properties(&element))))
        }
        "query_element_descendants" => {
            let p: QueryElementDescendantsParams = deserialize_params(args)?;
            let element =
                state.element("query_element_descendants", p.element_handle.to_index())?;
            let instructions = parse_query_instructions(&p.query)?;
            let find_all = p.find_all.unwrap_or(true);
            let results = state.query_element_descendants(element, instructions, find_all)?;
            let handles: Vec<Value> =
                results.into_iter().map(|e| handle_to_json(state.element_to_handle(e))).collect();
            Ok(ToolResult::Json(serde_json::json!({ "elements": handles })))
        }
        "get_element_tree" => {
            let p: GetElementTreeParams = deserialize_params(args)?;
            let max_elements = p.max_elements.unwrap_or(200).clamp(1, 1000) as usize;
            collect_element_list(state, p.element_handle.to_index(), max_elements)
                .map(ToolResult::Json)
        }
        "take_screenshot" => {
            let p: WindowHandleParam = deserialize_params(args)?;
            let png_data = state.take_snapshot(p.window_handle.to_index(), "image/png")?;
            Ok(ToolResult::Image {
                meta: serde_json::json!({ "size_bytes": png_data.len() }),
                png_data,
            })
        }
        "click_element" => {
            let p: ClickElementParams = deserialize_params(args)?;
            let element = state.element("click_element", p.element_handle.to_index())?;
            let button = match p.button.as_deref().unwrap_or("left") {
                "left" => i_slint_core::platform::PointerEventButton::Left,
                "right" => i_slint_core::platform::PointerEventButton::Right,
                "middle" => i_slint_core::platform::PointerEventButton::Middle,
                other => return Err(format!("Unknown button: {other}")),
            };
            match p.action.as_deref().unwrap_or("single_click") {
                "single_click" => element.single_click(button).await,
                "double_click" => element.double_click(button).await,
                other => return Err(format!("Unknown click action: {other}")),
            };
            Ok(ToolResult::Json(serde_json::json!({ "success": true })))
        }
        "invoke_accessibility_action" => {
            let p: InvokeAccessibilityActionParams = deserialize_params(args)?;
            let element =
                state.element("invoke_accessibility_action", p.element_handle.to_index())?;
            let action = match p.action.as_str() {
                "default" => AccessibilityAction::Default,
                "increment" => AccessibilityAction::Increment,
                "decrement" => AccessibilityAction::Decrement,
                "expand" => AccessibilityAction::Expand,
                other => return Err(format!("Unknown action: {other}")),
            };
            state.invoke_element_accessibility_action(&element, action)?;
            Ok(ToolResult::Json(serde_json::json!({ "success": true })))
        }
        "set_element_value" => {
            let p: SetElementValueParams = deserialize_params(args)?;
            let element = state.element("set_element_value", p.element_handle.to_index())?;
            element.set_accessible_value(p.value);
            Ok(ToolResult::Json(serde_json::json!({ "success": true })))
        }
        "dispatch_key_event" => {
            let p: DispatchKeyEventParams = deserialize_params(args)?;
            let window_handle = p.window_handle.to_index();
            let event_type = p.event_type.as_deref().unwrap_or("press_and_release");
            let events: Vec<i_slint_core::platform::WindowEvent> = match event_type {
                "press" => vec![i_slint_core::platform::WindowEvent::KeyPressed {
                    text: p.text.clone().into(),
                }],
                "release" => vec![i_slint_core::platform::WindowEvent::KeyReleased {
                    text: p.text.clone().into(),
                }],
                "press_and_release" => vec![
                    i_slint_core::platform::WindowEvent::KeyPressed { text: p.text.clone().into() },
                    i_slint_core::platform::WindowEvent::KeyReleased { text: p.text.into() },
                ],
                other => return Err(format!("Unknown event_type: {other}")),
            };
            for event in events {
                state.dispatch_window_event(window_handle, event)?;
            }
            Ok(ToolResult::Json(serde_json::json!({ "success": true })))
        }
        _ => Err(format!("Unknown tool: {name}")),
    }
}

// ============================================================================
// Helper functions
// ============================================================================

fn parse_query_instructions(arr: &[Value]) -> Result<Vec<QueryInstruction>, String> {
    let mut instructions = Vec::new();
    for item in arr {
        if item.get("match_descendants").is_some() {
            // Treat {match_descendants: false} as a no-op; only true adds the instruction.
            if item.get("match_descendants").and_then(|v| v.as_bool()).unwrap_or(false) {
                instructions.push(QueryInstruction::MatchDescendants);
            }
        } else if let Some(id) = item.get("match_id").and_then(|v| v.as_str()) {
            instructions.push(QueryInstruction::MatchId(id.to_string()));
        } else if let Some(tn) = item.get("match_type_name").and_then(|v| v.as_str()) {
            instructions.push(QueryInstruction::MatchTypeName(tn.to_string()));
        } else if let Some(tn) = item.get("match_type_name_or_base").and_then(|v| v.as_str()) {
            instructions.push(QueryInstruction::MatchTypeNameOrBase(tn.to_string()));
        } else if let Some(role_str) = item.get("match_accessible_role").and_then(|v| v.as_str()) {
            let role = introspection::string_to_accessible_role(role_str)
                .ok_or_else(|| format!("Unknown accessible role: {role_str}"))?;
            instructions.push(QueryInstruction::MatchAccessibleRole(role));
        } else {
            return Err("Invalid query instruction".into());
        }
    }
    Ok(instructions)
}

fn element_properties_to_json(ep: &introspection::ElementProperties) -> Value {
    let type_info: Vec<Value> = ep
        .type_names_and_ids
        .iter()
        .map(|(type_name, id)| serde_json::json!({ "type_name": type_name, "id": id }))
        .collect();

    let layout_kind = ep.layout_kind.as_ref().map(introspection::layout_kind_to_string);

    serde_json::json!({
        "type_info": type_info,
        "accessible_role": ep.accessible_role,
        "accessible_label": ep.accessible_label,
        "accessible_value": ep.accessible_value,
        "accessible_description": ep.accessible_description,
        "accessible_placeholder_text": ep.accessible_placeholder_text,
        "accessible_checked": ep.accessible_checked,
        "accessible_checkable": ep.accessible_checkable,
        "accessible_enabled": ep.accessible_enabled,
        "accessible_read_only": ep.accessible_read_only,
        "accessible_value_minimum": ep.accessible_value_minimum,
        "accessible_value_maximum": ep.accessible_value_maximum,
        "accessible_value_step": ep.accessible_value_step,
        "size": { "width": ep.size.width, "height": ep.size.height },
        "absolute_position": { "x": ep.absolute_position.x, "y": ep.absolute_position.y },
        "computed_opacity": ep.computed_opacity,
        "layout_kind": layout_kind
    })
}

fn collect_element_list(
    state: &IntrospectionState,
    root_handle: generational_arena::Index,
    max_elements: usize,
) -> Result<Value, String> {
    let root_element = state.element("get_element_tree", root_handle)?;

    // Collect all descendants via visit_descendants (DFS order).
    let mut elements: Vec<Value> = Vec::new();
    let mut truncated = false;

    // Add root element first
    let root_props = state.element_properties(&root_element);
    let mut root_node = element_properties_to_json(&root_props);
    root_node.as_object_mut().unwrap().insert("handle".to_string(), handle_to_json(root_handle));
    elements.push(root_node);

    root_element.visit_descendants(|child| {
        if elements.len() >= max_elements {
            truncated = true;
            return std::ops::ControlFlow::Break(());
        }
        let child_handle = state.element_to_handle(child.clone());
        let props = state.element_properties(&child);
        let mut node = element_properties_to_json(&props);
        node.as_object_mut().unwrap().insert("handle".to_string(), handle_to_json(child_handle));
        elements.push(node);
        std::ops::ControlFlow::<()>::Continue(())
    });

    Ok(serde_json::json!({
        "elements": elements,
        "total_count": elements.len(),
        "truncated": truncated
    }))
}

// ============================================================================
// JSON-RPC 2.0
// ============================================================================

fn json_rpc_success(id: &Value, result: Value) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn json_rpc_error(id: &Value, code: i32, message: String) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

async fn handle_mcp_request(state: &IntrospectionState, body: &str) -> Option<Value> {
    let request: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return Some(json_rpc_error(&Value::Null, -32700, format!("Parse error: {e}"))),
    };

    // Batch requests (JSON arrays) are not supported.
    if request.is_array() {
        return Some(json_rpc_error(
            &Value::Null,
            -32600,
            "Batch requests are not supported".into(),
        ));
    }

    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let is_notification = request.get("id").is_none();

    let response = match method {
        "initialize" => json_rpc_success(
            &id,
            serde_json::json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "slint-mcp-embedded",
                    "version": "0.1.0"
                },
                "instructions": concat!(
                    "This is an embedded MCP server in a running Slint application. ",
                    "It lets you inspect and interact with the application's UI.\n\n",
                    "Recommended workflow:\n",
                    "1. list_windows — get window handles\n",
                    "2. get_window_properties — get the root_element_handle\n",
                    "3. get_element_tree (max_elements=200) — explore the UI hierarchy\n",
                    "4. Use find_elements_by_id or query_element_descendants for targeted lookups\n",
                    "5. get_element_properties — inspect specific elements\n",
                    "6. take_screenshot — see the current visual state\n\n",
                    "Handles (window_handle, element_handle) are {index, generation} objects. ",
                    "They remain valid as long as the UI element exists."
                )
            }),
        ),
        "notifications/initialized" => {
            // Notification — no response needed
            return None;
        }
        "tools/list" => json_rpc_success(&id, tool_definitions()),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(serde_json::json!({}));
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let tool_args = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

            match handle_tool_call(state, tool_name, &tool_args).await {
                Ok(result) => {
                    let content = match result {
                        ToolResult::Image { png_data, meta } => {
                            let b64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
                            let mut blocks = vec![serde_json::json!({
                                "type": "image",
                                "data": b64,
                                "mimeType": "image/png"
                            })];
                            if !meta.as_object().map_or(true, |o| o.is_empty()) {
                                blocks.push(serde_json::json!({
                                    "type": "text",
                                    "text": serde_json::to_string_pretty(&meta).unwrap()
                                }));
                            }
                            blocks
                        }
                        ToolResult::Json(value) => {
                            vec![serde_json::json!({
                                "type": "text",
                                "text": serde_json::to_string_pretty(&value).unwrap()
                            })]
                        }
                    };
                    json_rpc_success(&id, serde_json::json!({ "content": content }))
                }
                Err(e) => json_rpc_success(
                    &id,
                    serde_json::json!({
                        "content": [{ "type": "text", "text": format!("Error: {e}") }],
                        "isError": true
                    }),
                ),
            }
        }
        _ => {
            if is_notification {
                return None;
            }
            json_rpc_error(&id, -32601, format!("Method not found: {method}"))
        }
    };

    if is_notification { None } else { Some(response) }
}

// ============================================================================
// HTTP Server (using httparse + async-net)
// ============================================================================

/// Read a complete HTTP request from a stream.
/// `carry` contains leftover bytes from a previous read on this connection.
/// Returns (method, path, headers, body, leftover) where leftover is any
/// data past the current request's body (start of the next pipelined request).
async fn read_http_request(
    stream: &mut async_net::TcpStream,
    carry: Vec<u8>,
) -> Result<(String, String, Vec<(String, String)>, Vec<u8>, Vec<u8>), String> {
    // Read headers into a buffer. We read in chunks until we find \r\n\r\n.
    let mut buf = carry;
    let header_end;
    // Check if the carry buffer already contains a complete header.
    if let Some(pos) = find_header_end(&buf) {
        header_end = pos;
    } else {
        loop {
            let mut chunk = [0u8; 1024];
            let n = stream.read(&mut chunk).await.map_err(|e| format!("read error: {e}"))?;
            if n == 0 {
                return Err("connection closed".into());
            }
            buf.extend_from_slice(&chunk[..n]);

            if let Some(pos) = find_header_end(&buf) {
                header_end = pos;
                break;
            }
            if buf.len() > 64 * 1024 {
                return Err("headers too large".into());
            }
        }
    }

    // Parse headers with httparse
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Request::new(&mut headers);
    let status = req.parse(&buf[..header_end + 4]).map_err(|e| format!("parse error: {e}"))?;
    if status.is_partial() {
        return Err("incomplete HTTP request".into());
    }

    let method = req.method.unwrap_or("").to_string();
    let path = req.path.unwrap_or("").to_string();
    let parsed_headers: Vec<(String, String)> = req
        .headers
        .iter()
        .map(|h| (h.name.to_ascii_lowercase(), String::from_utf8_lossy(h.value).to_string()))
        .collect();

    // Read body based on Content-Length (capped at 4 MB to prevent OOM).
    // Reject conflicting Content-Length values per RFC 7230 §3.3.2.
    const MAX_BODY_SIZE: usize = 4 * 1024 * 1024;
    let cl_values: Vec<&str> = parsed_headers
        .iter()
        .filter(|(k, _)| k == "content-length")
        .map(|(_, v)| v.as_str())
        .collect();
    if cl_values.len() > 1 && !cl_values.iter().all(|v| *v == cl_values[0]) {
        return Err("conflicting Content-Length headers".into());
    }
    let content_length: usize = cl_values.first().and_then(|v| v.parse().ok()).unwrap_or(0);
    if content_length > MAX_BODY_SIZE {
        return Err(format!("body too large: {content_length} bytes (max {MAX_BODY_SIZE})"));
    }

    // Body bytes we already have (after the header end)
    let body_start = header_end + 4; // skip \r\n\r\n
    let available = buf.len() - body_start;

    let (body, leftover) = if available >= content_length {
        // We have the full body (and possibly the start of the next request)
        let body = buf[body_start..body_start + content_length].to_vec();
        let leftover = buf[body_start + content_length..].to_vec();
        (body, leftover)
    } else {
        // Need to read more body bytes from the stream
        let mut body = buf[body_start..].to_vec();
        let remaining = content_length - available;
        let mut rest = vec![0u8; remaining];
        stream.read_exact(&mut rest).await.map_err(|e| format!("body read error: {e}"))?;
        body.extend_from_slice(&rest);
        (body, Vec::new())
    };

    Ok((method, path, parsed_headers, body, leftover))
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

async fn write_http_response(
    stream: &mut async_net::TcpStream,
    status: u16,
    status_text: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<(), String> {
    let mut response = format!("HTTP/1.1 {status} {status_text}\r\n");
    for (k, v) in headers {
        response.push_str(&format!("{k}: {v}\r\n"));
    }
    response.push_str(&format!("Content-Length: {}\r\n", body.len()));
    response.push_str("\r\n");

    stream.write_all(response.as_bytes()).await.map_err(|e| format!("write error: {e}"))?;
    stream.write_all(body).await.map_err(|e| format!("write error: {e}"))?;
    stream.flush().await.map_err(|e| format!("flush error: {e}"))?;
    Ok(())
}

/// Validates the Origin header to protect against DNS rebinding attacks.
/// Allows requests with no Origin (e.g. curl, MCP CLI clients), or with
/// localhost-like origins.
/// Checks whether an origin string refers to a localhost address.
/// The origin after the scheme must be exactly "localhost", "127.0.0.1", or "[::1]",
/// optionally followed by a port (":1234").
fn is_localhost_origin(origin: &str) -> bool {
    let host =
        origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://")).unwrap_or("");
    // Strip optional port
    let host_no_port = if host.starts_with('[') {
        // IPv6: [::1]:port
        host.split_once(']').map(|(h, _)| &h[1..]).unwrap_or(host)
    } else {
        host.split(':').next().unwrap_or(host)
    };
    matches!(host_no_port, "localhost" | "127.0.0.1" | "::1")
}

/// Returns the validated origin if present and allowed, or `None` if no origin was sent.
/// Returns `Err` if the origin is present but not a localhost address.
fn validate_origin(headers: &[(String, String)]) -> Result<Option<&str>, ()> {
    let origin = headers.iter().find(|(k, _)| k == "origin").map(|(_, v)| v.as_str());
    match origin {
        None => Ok(None), // No Origin header — non-browser client (curl, MCP SDK, etc.)
        Some(o) if is_localhost_origin(o.trim()) => Ok(Some(o.trim())),
        Some(_) => Err(()),
    }
}

/// Returns true if the client requested connection close.
fn wants_close(headers: &[(String, String)]) -> bool {
    headers.iter().any(|(k, v)| k == "connection" && v.eq_ignore_ascii_case("close"))
}

async fn handle_connection(state: &IntrospectionState, mut stream: async_net::TcpStream) {
    let mut carry = Vec::new();

    loop {
        let (method, path, headers, body, leftover) =
            match read_http_request(&mut stream, carry).await {
                Ok(req) => req,
                Err(_) => return, // Connection closed or read error — done.
            };

        let close_after = wants_close(&headers);
        carry = leftover;

        // Validate Origin header to prevent DNS rebinding attacks.
        let allowed_origin = match validate_origin(&headers) {
            Ok(origin) => origin,
            Err(()) => {
                let _ = write_http_response(
                    &mut stream,
                    403,
                    "Forbidden",
                    &[],
                    b"Origin not allowed\n",
                )
                .await;
                return;
            }
        };

        // The CORS origin to echo back: the validated origin, or "*" for non-browser clients.
        let cors_origin = allowed_origin.unwrap_or("*");

        // Handle CORS preflight for browser-based MCP clients.
        if method == "OPTIONS" {
            let cors_headers = [
                ("Access-Control-Allow-Origin", cors_origin),
                ("Access-Control-Allow-Methods", "POST, OPTIONS"),
                (
                    "Access-Control-Allow-Headers",
                    "Content-Type, MCP-Protocol-Version, Mcp-Session-Id",
                ),
                ("Access-Control-Max-Age", "86400"),
                ("Vary", "Origin"),
            ];
            let _ = write_http_response(&mut stream, 204, "No Content", &cors_headers, b"").await;
        } else if method != "POST" || (path != "/mcp" && path != "/") {
            let _ =
                write_http_response(&mut stream, 404, "Not Found", &[], b"404 Not Found\n").await;
        } else if !headers
            .iter()
            .find(|(k, _)| k == "content-type")
            .map(|(_, v)| v.as_str())
            .is_some_and(|ct| ct.starts_with("application/json"))
        {
            let _ = write_http_response(
                &mut stream,
                415,
                "Unsupported Media Type",
                &[],
                b"Content-Type must be application/json\n",
            )
            .await;
        } else {
            let body_str = String::from_utf8_lossy(&body);
            let response = handle_mcp_request(state, &body_str).await;

            let resp_headers = [
                ("Content-Type", "application/json"),
                ("Access-Control-Allow-Origin", cors_origin),
                ("Vary", "Origin"),
            ];

            match response {
                Some(resp) => {
                    let json = serde_json::to_string(&resp).unwrap();
                    let _ =
                        write_http_response(&mut stream, 200, "OK", &resp_headers, json.as_bytes())
                            .await;
                }
                None => {
                    let _ =
                        write_http_response(&mut stream, 202, "Accepted", &resp_headers, b"").await;
                }
            }
        }

        if close_after {
            return;
        }
    }
}

async fn run_server(state: Rc<IntrospectionState>, port: u16) {
    let addr = format!("127.0.0.1:{port}");
    let listener = match async_net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("MCP server: failed to bind to {addr}: {e}");
            return;
        }
    };
    eprintln!("Slint MCP server listening on http://{addr}/mcp");

    loop {
        match listener.accept().await {
            Ok((stream, _peer)) => {
                stream.set_nodelay(true).ok();
                // Spawn each connection handler so slow clients don't block others.
                let state = state.clone();
                let _ = i_slint_core::with_global_context(
                    || panic!("uninitialized platform"),
                    |context| {
                        let _ = context.spawn_local(async move {
                            handle_connection(&state, stream).await;
                        });
                    },
                );
            }
            Err(e) => {
                eprintln!("MCP server: accept error: {e}");
            }
        }
    }
}

// ============================================================================
// Initialization
// ============================================================================

pub fn init() -> Result<(), EventLoopError> {
    let Ok(port_str) = std::env::var("SLINT_MCP_PORT") else {
        return Ok(());
    };
    let port: u16 = match port_str.parse() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("SLINT_MCP_PORT: invalid port number '{port_str}'");
            return Ok(());
        }
    };

    // Use the shared introspection state (shared with systest when both features are enabled).
    introspection::ensure_window_tracking()?;
    let state = introspection::shared_state();

    // Start the HTTP server. The future is queued and will execute once the event loop runs.
    let server_started =
        Rc::new(std::cell::OnceCell::<i_slint_core::future::JoinHandle<()>>::new());
    let server_started_clone = server_started.clone();
    let state_clone = state.clone();

    // Use the window-shown hook to trigger server start on first window,
    // since spawn_local requires the event loop context.
    let previous_hook = i_slint_core::context::set_window_shown_hook(None)
        .map_err(|_| EventLoopError::NoEventLoopProvider)?;
    let previous_hook = std::cell::RefCell::new(previous_hook);

    i_slint_core::context::set_window_shown_hook(Some(Box::new(move |adapter| {
        if let Some(prev) = previous_hook.borrow_mut().as_mut() {
            prev(adapter);
        }

        let state = state_clone.clone();
        server_started_clone.get_or_init(|| {
            i_slint_core::with_global_context(
                || panic!("uninitialized platform"),
                |context| {
                    context
                        .spawn_local(async move {
                            run_server(state, port).await;
                        })
                        .unwrap()
                },
            )
            .unwrap()
        });
    })))
    .map_err(|_| EventLoopError::NoEventLoopProvider)?;

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_roundtrip() {
        // Test basic roundtrip
        let index = generational_arena::Index::from_raw_parts(0, 0);
        let json = handle_to_json(index);
        let handle: Handle = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(index, handle.to_index());

        // Test with larger values
        let index = generational_arena::Index::from_raw_parts(42, 7);
        let json = handle_to_json(index);
        assert_eq!(json["index"], 42);
        assert_eq!(json["generation"], 7);
        let handle: Handle = serde_json::from_value(json).unwrap();
        assert_eq!(index, handle.to_index());
    }

    #[test]
    fn test_handle_deserialize_missing_fields() {
        let v = serde_json::json!({});
        assert!(serde_json::from_value::<Handle>(v).is_err());

        let v = serde_json::json!({"index": 0});
        assert!(serde_json::from_value::<Handle>(v).is_err());

        let v = serde_json::json!({"generation": 0});
        assert!(serde_json::from_value::<Handle>(v).is_err());
    }

    #[test]
    fn test_parse_query_instructions_all_types() {
        let arr = vec![
            serde_json::json!({"match_descendants": true}),
            serde_json::json!({"match_id": "my_id"}),
            serde_json::json!({"match_type_name": "Button"}),
            serde_json::json!({"match_type_name_or_base": "TouchArea"}),
            serde_json::json!({"match_accessible_role": "button"}),
        ];
        let instructions = parse_query_instructions(&arr).unwrap();
        assert_eq!(instructions.len(), 5);
        assert!(matches!(instructions[0], QueryInstruction::MatchDescendants));
        assert!(matches!(&instructions[1], QueryInstruction::MatchId(id) if id == "my_id"));
        assert!(matches!(&instructions[2], QueryInstruction::MatchTypeName(n) if n == "Button"));
        assert!(
            matches!(&instructions[3], QueryInstruction::MatchTypeNameOrBase(n) if n == "TouchArea")
        );
        assert!(matches!(instructions[4], QueryInstruction::MatchAccessibleRole(_)));
    }

    #[test]
    fn test_parse_query_instructions_empty() {
        let arr: Vec<Value> = vec![];
        let instructions = parse_query_instructions(&arr).unwrap();
        assert!(instructions.is_empty());
    }

    #[test]
    fn test_parse_query_instructions_invalid() {
        let arr = vec![serde_json::json!({"bogus": 123})];
        assert!(parse_query_instructions(&arr).is_err());
    }

    #[test]
    fn test_parse_query_instructions_match_descendants_false_is_noop() {
        let arr = vec![serde_json::json!({"match_descendants": false})];
        let instructions = parse_query_instructions(&arr).unwrap();
        assert!(instructions.is_empty(), "match_descendants: false should be a no-op");
    }

    #[test]
    fn test_parse_query_instructions_unknown_role() {
        let arr = vec![serde_json::json!({"match_accessible_role": "nonexistent"})];
        assert!(parse_query_instructions(&arr).is_err());
    }

    #[test]
    fn test_validate_origin() {
        // No Origin header — allowed, returns Ok(None)
        assert_eq!(validate_origin(&[]), Ok(None));

        // Localhost origins — allowed, returns Ok(Some(origin))
        assert!(validate_origin(&[("origin".into(), "http://localhost".into())]).is_ok());
        assert_eq!(
            validate_origin(&[("origin".into(), "http://localhost:3000".into())]),
            Ok(Some("http://localhost:3000"))
        );
        assert!(validate_origin(&[("origin".into(), "https://localhost:8443".into())]).is_ok());
        assert!(validate_origin(&[("origin".into(), "http://127.0.0.1:8080".into())]).is_ok());
        assert!(validate_origin(&[("origin".into(), "http://[::1]:3000".into())]).is_ok());

        // External origins — rejected
        assert!(validate_origin(&[("origin".into(), "http://evil.com".into())]).is_err());
        assert!(validate_origin(&[("origin".into(), "http://localhost.evil.com".into())]).is_err());
        assert!(validate_origin(&[("origin".into(), "https://attacker.com".into())]).is_err());

        // Edge cases
        assert!(validate_origin(&[("origin".into(), "http://0.0.0.0:8080".into())]).is_err());
        assert!(validate_origin(&[("origin".into(), "http://127.0.0.2".into())]).is_err());
        assert!(validate_origin(&[("origin".into(), "".into())]).is_err());
        assert!(validate_origin(&[("origin".into(), "localhost".into())]).is_err()); // no scheme
    }

    #[test]
    fn test_wants_close() {
        assert!(!wants_close(&[]));
        assert!(!wants_close(&[("connection".into(), "keep-alive".into())]));
        assert!(wants_close(&[("connection".into(), "close".into())]));
        assert!(wants_close(&[("connection".into(), "Close".into())]));
        assert!(!wants_close(&[("content-type".into(), "close".into())]));
    }

    #[test]
    fn test_json_rpc_success() {
        let result = json_rpc_success(&Value::from(1), serde_json::json!({"ok": true}));
        assert_eq!(result["jsonrpc"], "2.0");
        assert_eq!(result["id"], 1);
        assert_eq!(result["result"]["ok"], true);
    }

    #[test]
    fn test_json_rpc_error() {
        let result = json_rpc_error(&Value::from(2), -32600, "Invalid Request".into());
        assert_eq!(result["jsonrpc"], "2.0");
        assert_eq!(result["id"], 2);
        assert_eq!(result["error"]["code"], -32600);
        assert_eq!(result["error"]["message"], "Invalid Request");
    }

    #[test]
    fn test_find_header_end() {
        assert_eq!(find_header_end(b"GET / HTTP/1.1\r\n\r\n"), Some(14));
        assert_eq!(find_header_end(b"no double crlf here"), None);
        assert_eq!(find_header_end(b"\r\n\r\n"), Some(0));
        assert_eq!(find_header_end(b"a\r\n\r\nb"), Some(1));
    }

    #[test]
    fn test_tool_definitions_structure() {
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().unwrap();
        assert!(!tools.is_empty());

        // Verify all tools have required fields
        for tool in tools {
            assert!(tool.get("name").and_then(|v| v.as_str()).is_some());
            assert!(tool.get("description").and_then(|v| v.as_str()).is_some());
            let schema = tool.get("inputSchema").unwrap();
            assert_eq!(schema["type"], "object");
        }

        // Verify specific tools exist
        let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(tool_names.contains(&"list_windows"));
        assert!(tool_names.contains(&"get_window_properties"));
        assert!(tool_names.contains(&"get_element_tree"));
        assert!(tool_names.contains(&"take_screenshot"));
        assert!(tool_names.contains(&"click_element"));
        assert!(tool_names.contains(&"query_element_descendants"));
    }

    #[test]
    fn test_element_properties_to_json() {
        let ep = introspection::ElementProperties {
            type_names_and_ids: vec![("Button".into(), "my_button".into())],
            accessible_role: "button",
            accessible_label: Some("Click me".into()),
            accessible_value: None,
            accessible_description: None,
            accessible_placeholder_text: None,
            accessible_checked: false,
            accessible_checkable: false,
            accessible_enabled: true,
            accessible_read_only: false,
            accessible_value_minimum: 0.0,
            accessible_value_maximum: 0.0,
            accessible_value_step: 0.0,
            size: i_slint_core::api::LogicalSize { width: 100.0, height: 30.0 },
            absolute_position: i_slint_core::api::LogicalPosition { x: 10.0, y: 20.0 },
            computed_opacity: 1.0,
            layout_kind: None,
        };
        let json = element_properties_to_json(&ep);
        assert_eq!(json["accessible_role"], "button");
        assert_eq!(json["accessible_label"], "Click me");
        assert_eq!(json["size"]["width"], 100.0);
        assert_eq!(json["absolute_position"]["x"], 10.0);
        assert_eq!(json["accessible_enabled"], true);
        assert!(json["layout_kind"].is_null());
        assert_eq!(json["type_info"][0]["type_name"], "Button");
        assert_eq!(json["type_info"][0]["id"], "my_button");
    }

    // ========================================================================
    // handle_mcp_request integration tests
    // ========================================================================

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        futures_lite::future::block_on(f)
    }

    fn make_state() -> IntrospectionState {
        IntrospectionState::new()
    }

    #[test]
    fn test_mcp_initialize() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        ));
        let resp = resp.expect("initialize should return a response");
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 1);
        assert!(resp["result"]["protocolVersion"].as_str().is_some());
        assert!(resp["result"]["serverInfo"]["name"].as_str().is_some());
        assert!(resp["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_mcp_notification_returns_none() {
        let state = make_state();
        // notifications/initialized has no "id" field — it's a notification
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        ));
        assert!(resp.is_none(), "notification should not produce a response");
    }

    #[test]
    fn test_mcp_unknown_notification_returns_none() {
        let state = make_state();
        // Unknown method with no "id" is also a notification
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","method":"some/unknown/notification"}"#,
        ));
        assert!(resp.is_none(), "unknown notification should not produce a response");
    }

    #[test]
    fn test_mcp_tools_list() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        ));
        let resp = resp.unwrap();
        assert_eq!(resp["id"], 2);
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert!(!tools.is_empty());
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"list_windows"));
    }

    #[test]
    fn test_mcp_tools_call_list_windows() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_windows","arguments":{}}}"#,
        ));
        let resp = resp.unwrap();
        assert_eq!(resp["id"], 3);
        // No windows registered, so result should have empty windows array
        let content = &resp["result"]["content"];
        let text = content[0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["windows"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_mcp_tools_call_unknown_tool() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"nonexistent_tool","arguments":{}}}"#,
        ));
        let resp = resp.unwrap();
        assert_eq!(resp["id"], 4);
        assert!(resp["result"]["isError"].as_bool().unwrap_or(false));
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Unknown tool"));
    }

    #[test]
    fn test_mcp_unknown_method() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":5,"method":"bogus/method"}"#,
        ));
        let resp = resp.unwrap();
        assert_eq!(resp["id"], 5);
        assert_eq!(resp["error"]["code"], -32601);
        assert!(resp["error"]["message"].as_str().unwrap().contains("Method not found"));
    }

    #[test]
    fn test_mcp_malformed_json() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(&state, "this is not json"));
        let resp = resp.unwrap();
        assert_eq!(resp["error"]["code"], -32700);
        assert!(resp["error"]["message"].as_str().unwrap().contains("Parse error"));
    }

    #[test]
    fn test_mcp_batch_request_rejected() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"[{"jsonrpc":"2.0","id":1,"method":"initialize"},{"jsonrpc":"2.0","id":2,"method":"tools/list"}]"#,
        ));
        let resp = resp.unwrap();
        assert_eq!(resp["error"]["code"], -32600);
        assert!(resp["error"]["message"].as_str().unwrap().contains("Batch"));
    }

    #[test]
    fn test_mcp_missing_method() {
        let state = make_state();
        // Valid JSON-RPC but no method field — treated as unknown method ""
        let resp = block_on(handle_mcp_request(&state, r#"{"jsonrpc":"2.0","id":6}"#));
        let resp = resp.unwrap();
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[test]
    fn test_mcp_tools_call_missing_required_param() {
        let state = make_state();
        // get_window_properties requires window_handle
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"get_window_properties","arguments":{}}}"#,
        ));
        let resp = resp.unwrap();
        assert!(resp["result"]["isError"].as_bool().unwrap_or(false));
    }

    #[test]
    fn test_element_properties_to_json_with_layout() {
        let ep = introspection::ElementProperties {
            type_names_and_ids: vec![],
            accessible_role: "unknown",
            accessible_label: None,
            accessible_value: None,
            accessible_description: None,
            accessible_placeholder_text: None,
            accessible_checked: false,
            accessible_checkable: false,
            accessible_enabled: false,
            accessible_read_only: false,
            accessible_value_minimum: 0.0,
            accessible_value_maximum: 0.0,
            accessible_value_step: 0.0,
            size: i_slint_core::api::LogicalSize { width: 0.0, height: 0.0 },
            absolute_position: i_slint_core::api::LogicalPosition { x: 0.0, y: 0.0 },
            computed_opacity: 1.0,
            layout_kind: Some(crate::LayoutKind::VerticalLayout),
        };
        let json = element_properties_to_json(&ep);
        assert_eq!(json["layout_kind"], "vertical");
    }

    // ========================================================================
    // Parameter deserialization tests
    // ========================================================================

    #[test]
    fn test_deserialize_window_handle_param() {
        let v = serde_json::json!({"window_handle": {"index": 1, "generation": 2}});
        let p: WindowHandleParam = serde_json::from_value(v).unwrap();
        assert_eq!(p.window_handle.index, 1);
        assert_eq!(p.window_handle.generation, 2);
    }

    #[test]
    fn test_deserialize_window_handle_param_wrong_type() {
        let v = serde_json::json!({"window_handle": "not_an_object"});
        assert!(serde_json::from_value::<WindowHandleParam>(v).is_err());
    }

    #[test]
    fn test_deserialize_find_elements_by_id_params() {
        let v = serde_json::json!({
            "window_handle": {"index": 0, "generation": 0},
            "element_id": "App::button1"
        });
        let p: FindElementsByIdParams = serde_json::from_value(v).unwrap();
        assert_eq!(p.element_id, "App::button1");
    }

    #[test]
    fn test_deserialize_find_elements_by_id_missing_id() {
        let v = serde_json::json!({"window_handle": {"index": 0, "generation": 0}});
        assert!(serde_json::from_value::<FindElementsByIdParams>(v).is_err());
    }

    #[test]
    fn test_deserialize_click_element_defaults() {
        let v = serde_json::json!({"element_handle": {"index": 0, "generation": 0}});
        let p: ClickElementParams = serde_json::from_value(v).unwrap();
        assert!(p.action.is_none());
        assert!(p.button.is_none());
    }

    #[test]
    fn test_deserialize_click_element_with_optionals() {
        let v = serde_json::json!({
            "element_handle": {"index": 0, "generation": 0},
            "action": "double_click",
            "button": "right"
        });
        let p: ClickElementParams = serde_json::from_value(v).unwrap();
        assert_eq!(p.action.as_deref(), Some("double_click"));
        assert_eq!(p.button.as_deref(), Some("right"));
    }

    #[test]
    fn test_deserialize_dispatch_key_event_defaults() {
        let v = serde_json::json!({
            "window_handle": {"index": 0, "generation": 0},
            "text": "a"
        });
        let p: DispatchKeyEventParams = serde_json::from_value(v).unwrap();
        assert_eq!(p.text, "a");
        assert!(p.event_type.is_none());
    }

    #[test]
    fn test_deserialize_get_element_tree_defaults() {
        let v = serde_json::json!({"element_handle": {"index": 0, "generation": 0}});
        let p: GetElementTreeParams = serde_json::from_value(v).unwrap();
        assert!(p.max_elements.is_none());
    }

    #[test]
    fn test_deserialize_query_element_descendants_defaults() {
        let v = serde_json::json!({
            "element_handle": {"index": 0, "generation": 0},
            "query": [{"match_descendants": true}]
        });
        let p: QueryElementDescendantsParams = serde_json::from_value(v).unwrap();
        assert!(p.find_all.is_none());
        assert_eq!(p.query.len(), 1);
    }

    #[test]
    fn test_deserialize_params_wrong_type() {
        // Passing a string where an object is expected
        let result = deserialize_params::<WindowHandleParam>(&serde_json::json!("string"));
        let err = result.err().expect("should fail");
        assert!(err.contains("Invalid parameters"));
    }

    #[test]
    fn test_deserialize_invoke_accessibility_action_params() {
        let v = serde_json::json!({
            "element_handle": {"index": 5, "generation": 3},
            "action": "increment"
        });
        let p: InvokeAccessibilityActionParams = serde_json::from_value(v).unwrap();
        assert_eq!(p.action, "increment");
        assert_eq!(p.element_handle.index, 5);
    }

    #[test]
    fn test_deserialize_invoke_accessibility_action_missing_action() {
        let v = serde_json::json!({"element_handle": {"index": 0, "generation": 0}});
        assert!(serde_json::from_value::<InvokeAccessibilityActionParams>(v).is_err());
    }

    #[test]
    fn test_deserialize_set_element_value_params() {
        let v = serde_json::json!({
            "element_handle": {"index": 0, "generation": 0},
            "value": "hello"
        });
        let p: SetElementValueParams = serde_json::from_value(v).unwrap();
        assert_eq!(p.value, "hello");
    }

    #[test]
    fn test_deserialize_set_element_value_missing_value() {
        let v = serde_json::json!({"element_handle": {"index": 0, "generation": 0}});
        assert!(serde_json::from_value::<SetElementValueParams>(v).is_err());
    }

    // ========================================================================
    // Tool error path tests (via handle_tool_call)
    // ========================================================================

    /// Helper: call a tool and return the result directly.
    fn call_tool(state: &IntrospectionState, name: &str, args: Value) -> Result<Value, String> {
        block_on(handle_tool_call(state, name, &args)).map(|r| match r {
            ToolResult::Json(v) => v,
            ToolResult::Image { png_data, meta } => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
                serde_json::json!({ "image_data": b64, "meta": meta })
            }
        })
    }

    /// Helper: make a handle JSON for an index that doesn't exist in the arena.
    fn bogus_handle() -> Value {
        serde_json::json!({"index": 999, "generation": 999})
    }

    #[test]
    fn test_tool_get_window_properties_invalid_handle() {
        let state = make_state();
        let result = call_tool(
            &state,
            "get_window_properties",
            serde_json::json!({"window_handle": bogus_handle()}),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid window handle"));
    }

    #[test]
    fn test_tool_find_elements_by_id_invalid_window() {
        let state = make_state();
        let result = call_tool(
            &state,
            "find_elements_by_id",
            serde_json::json!({
                "window_handle": bogus_handle(),
                "element_id": "App::foo"
            }),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_get_element_properties_invalid_handle() {
        let state = make_state();
        let result = call_tool(
            &state,
            "get_element_properties",
            serde_json::json!({"element_handle": bogus_handle()}),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid element handle"));
    }

    #[test]
    fn test_tool_query_element_descendants_invalid_handle() {
        let state = make_state();
        let result = call_tool(
            &state,
            "query_element_descendants",
            serde_json::json!({
                "element_handle": bogus_handle(),
                "query": [{"match_descendants": true}]
            }),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_get_element_tree_invalid_handle() {
        let state = make_state();
        let result = call_tool(
            &state,
            "get_element_tree",
            serde_json::json!({"element_handle": bogus_handle()}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_take_screenshot_invalid_window() {
        let state = make_state();
        let result = call_tool(
            &state,
            "take_screenshot",
            serde_json::json!({"window_handle": bogus_handle()}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_click_element_invalid_handle() {
        let state = make_state();
        let result = call_tool(
            &state,
            "click_element",
            serde_json::json!({"element_handle": bogus_handle()}),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_click_element_invalid_button() {
        let state = make_state();
        // Dummy elements fail is_valid(), so we get an element error before
        // reaching button validation. Verify the error path works.
        let handle_idx = state.element_to_handle(crate::ElementHandle::new_test_dummy());
        let handle = handle_to_json(handle_idx);
        let result = call_tool(
            &state,
            "click_element",
            serde_json::json!({
                "element_handle": handle,
                "button": "imaginary"
            }),
        );
        assert!(result.is_err());
        // Element is invalid (test dummy), so we get an element error
        assert!(result.unwrap_err().contains("element"));
    }

    #[test]
    fn test_tool_click_element_missing_handle() {
        let state = make_state();
        let result =
            call_tool(&state, "click_element", serde_json::json!({"action": "triple_click"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid parameters"));
    }

    #[test]
    fn test_tool_invoke_accessibility_action_invalid_handle() {
        let state = make_state();
        let result = call_tool(
            &state,
            "invoke_accessibility_action",
            serde_json::json!({
                "element_handle": bogus_handle(),
                "action": "default"
            }),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_invoke_accessibility_action_unknown_action() {
        let state = make_state();
        // Dummy elements fail is_valid(), so we get an element error
        let handle_idx = state.element_to_handle(crate::ElementHandle::new_test_dummy());
        let handle = handle_to_json(handle_idx);
        let result = call_tool(
            &state,
            "invoke_accessibility_action",
            serde_json::json!({
                "element_handle": handle,
                "action": "fly"
            }),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("element"));
    }

    #[test]
    fn test_tool_invoke_accessibility_action_missing_action() {
        let state = make_state();
        let result = call_tool(
            &state,
            "invoke_accessibility_action",
            serde_json::json!({"element_handle": {"index": 0, "generation": 0}}),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid parameters"));
    }

    #[test]
    fn test_tool_set_element_value_invalid_handle() {
        let state = make_state();
        let result = call_tool(
            &state,
            "set_element_value",
            serde_json::json!({
                "element_handle": bogus_handle(),
                "value": "test"
            }),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_dispatch_key_event_invalid_window() {
        let state = make_state();
        let result = call_tool(
            &state,
            "dispatch_key_event",
            serde_json::json!({
                "window_handle": bogus_handle(),
                "text": "a"
            }),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_dispatch_key_event_unknown_event_type() {
        let state = make_state();
        let result = call_tool(
            &state,
            "dispatch_key_event",
            serde_json::json!({
                "window_handle": bogus_handle(),
                "text": "a",
                "event_type": "smash"
            }),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown event_type"));
    }

    // ========================================================================
    // Tool error wrapping via MCP request (isError flag)
    // ========================================================================

    #[test]
    fn test_mcp_tool_error_is_wrapped_correctly() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"get_element_properties","arguments":{"element_handle":{"index":999,"generation":999}}}}"#,
        ));
        let resp = resp.unwrap();
        assert_eq!(resp["id"], 10);
        assert!(resp["result"]["isError"].as_bool().unwrap());
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Error:"));
    }

    #[test]
    fn test_mcp_tool_deserialization_error_is_wrapped() {
        let state = make_state();
        // Pass wrong type for window_handle (string instead of object)
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"get_window_properties","arguments":{"window_handle":"bad"}}}"#,
        ));
        let resp = resp.unwrap();
        assert!(resp["result"]["isError"].as_bool().unwrap());
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Invalid parameters"));
    }

    // ========================================================================
    // Tool definitions schema validation
    // ========================================================================

    #[test]
    fn test_tool_definitions_required_fields_match_params() {
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().unwrap();

        for tool in tools {
            let name = tool["name"].as_str().unwrap();
            let schema = &tool["inputSchema"];
            let required = schema.get("required").and_then(|r| r.as_array());
            let properties = schema["properties"].as_object().unwrap();

            // Every required field must exist in properties
            if let Some(req) = required {
                for field in req {
                    let field_name = field.as_str().unwrap();
                    assert!(
                        properties.contains_key(field_name),
                        "Tool '{name}': required field '{field_name}' not in properties"
                    );
                }
            }

            // Every property with a handle schema should be required
            for (prop_name, prop_schema) in properties {
                if prop_schema.get("properties").is_some()
                    && prop_schema["properties"].get("index").is_some()
                    && prop_schema["properties"].get("generation").is_some()
                {
                    let is_required = required
                        .map(|r| r.iter().any(|f| f.as_str() == Some(prop_name)))
                        .unwrap_or(false);
                    assert!(
                        is_required,
                        "Tool '{name}': handle property '{prop_name}' should be required"
                    );
                }
            }
        }
    }

    #[test]
    fn test_tool_definitions_all_tools_present() {
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

        let expected = [
            "list_windows",
            "get_window_properties",
            "find_elements_by_id",
            "get_element_properties",
            "query_element_descendants",
            "get_element_tree",
            "take_screenshot",
            "click_element",
            "invoke_accessibility_action",
            "set_element_value",
            "dispatch_key_event",
        ];
        for name in &expected {
            assert!(names.contains(name), "Missing tool definition: {name}");
        }
        assert_eq!(names.len(), expected.len(), "Unexpected extra tool definitions");
    }
}
