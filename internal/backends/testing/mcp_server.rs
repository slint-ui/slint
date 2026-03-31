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
use serde_json::Value;
use std::rc::Rc;

use crate::introspection::{self, IntrospectionState, proto};

fn handle_to_index(handle: proto::Handle) -> generational_arena::Index {
    introspection::handle_to_index(handle)
}

fn index_to_handle(index: generational_arena::Index) -> proto::Handle {
    introspection::index_to_handle(index)
}

// ============================================================================
// Tool definitions (schema for tools/list)
// ============================================================================

fn tool_definitions() -> Value {
    let handle_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "index": { "type": "string", "description": "Arena index (uint64 as string)" },
            "generation": { "type": "string", "description": "Arena generation (uint64 as string)" }
        },
        "required": ["index", "generation"]
    });

    serde_json::json!({
        "tools": [
            {
                "name": "list_windows",
                "description": "List all open windows. Returns an array of window handles. Call this first to discover available windows.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "get_window_properties",
                "description": "Get a window's physical size (pixels), position, fullscreen/maximized/minimized state, and rootElementHandle — the entry point for element tree traversal.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "windowHandle": handle_schema.clone()
                    },
                    "required": ["windowHandle"]
                }
            },
            {
                "name": "get_element_tree",
                "description": "Get a flat list of elements in the subtree rooted at the given element. Each entry includes type names, IDs, accessibility properties, geometry, and a handle for further queries. Use maxElements to control the result size (default: 200, max: 1000). If truncated is true, there are more elements — use query_element_descendants for targeted searches instead.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "elementHandle": handle_schema.clone(),
                        "maxElements": { "type": "integer", "description": "Maximum elements to return (default: 200, max: 1000)." }
                    },
                    "required": ["elementHandle"]
                }
            },
            {
                "name": "get_element_properties",
                "description": "Get full details of a single element: type names and IDs (including inherited bases), all accessible properties (role, label, value, description, checked, enabled, read-only, placeholder, value min/max/step), logical size and position, computed opacity, and layout kind.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "elementHandle": handle_schema.clone()
                    },
                    "required": ["elementHandle"]
                }
            },
            {
                "name": "find_elements_by_id",
                "description": "Find elements by qualified ID (format: 'ComponentName::element-id', e.g. 'App::my-button'). Returns element handles. Use get_element_tree first to discover available IDs.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "windowHandle": handle_schema.clone(),
                        "elementsId": { "type": "string", "description": "Qualified element ID (e.g. 'App::my-button')." }
                    },
                    "required": ["windowHandle", "elementsId"]
                }
            },
            {
                "name": "query_element_descendants",
                "description": "Search descendants of an element using a query pipeline. Pass an array of instructions applied in order: {\"matchDescendants\": true} to recurse, then filter by {\"matchElementId\": \"...\"}, {\"matchElementTypeName\": \"...\"}, {\"matchElementTypeNameOrBase\": \"...\"}, or {\"matchElementAccessibleRole\": \"Button\"}. More efficient than get_element_tree for targeted lookups.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "elementHandle": handle_schema.clone(),
                        "queryStack": {
                            "type": "array",
                            "description": "Query pipeline: array of objects, each with exactly one instruction field.",
                            "items": { "type": "object" }
                        },
                        "findAll": { "type": "boolean", "description": "Return all matches (default: true). Set to false for first match only." }
                    },
                    "required": ["elementHandle", "queryStack"]
                }
            },
            {
                "name": "take_screenshot",
                "description": "Capture a PNG screenshot of a window. Returns an MCP image content block rendered inline by the client. Use after interactions to verify visual results.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "windowHandle": handle_schema.clone()
                    },
                    "required": ["windowHandle"]
                }
            },
            {
                "name": "click_element",
                "description": "Simulate a mouse click at the center of an element. Omit action/button for a left single-click (the most common case).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "elementHandle": handle_schema.clone(),
                        "action": { "type": "string", "description": "'SingleClick' (default) or 'DoubleClick'." },
                        "button": { "type": "string", "description": "'Left' (default), 'Right', or 'Middle'." }
                    },
                    "required": ["elementHandle"]
                }
            },
            {
                "name": "invoke_accessibility_action",
                "description": "Invoke an accessibility action: 'Default_' (activate buttons, toggle checkboxes), 'Increment'/'Decrement' (sliders, spinboxes), 'Expand' (combo boxes). Preferred over click_element when the element's role suggests a semantic action.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "elementHandle": handle_schema.clone(),
                        "action": { "type": "string", "description": "'Default_', 'Increment', 'Decrement', or 'Expand'." }
                    },
                    "required": ["elementHandle", "action"]
                }
            },
            {
                "name": "set_element_value",
                "description": "Set the accessible value of an element. For text inputs: sets the text content. For sliders: pass the numeric value as a string (e.g. '42'). For other elements: sets whatever the element exposes as its accessible value.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "elementHandle": handle_schema.clone(),
                        "value": { "type": "string", "description": "The value to set (always a string, even for numeric values)." }
                    },
                    "required": ["elementHandle", "value"]
                }
            },
            {
                "name": "dispatch_key_event",
                "description": "Send a keyboard event to a window. Use 'press_and_release' (default) for typing characters. Use 'press'/'release' separately for modifier keys or key combinations.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "windowHandle": handle_schema.clone(),
                        "text": { "type": "string", "description": "The key text (e.g. 'a', 'Enter', '\\t' for tab)." },
                        "eventType": { "type": "string", "description": "'press_and_release' (default), 'press', or 'release'." }
                    },
                    "required": ["windowHandle", "text"]
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
            let response = proto::WindowListResponse {
                window_handles: state.window_handles().into_iter().map(index_to_handle).collect(),
            };
            Ok(ToolResult::Json(
                serde_json::to_value(&response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "get_window_properties" => {
            let p: proto::RequestWindowProperties = deserialize_params(args)?;
            let window_index =
                handle_to_index(p.window_handle.ok_or_else(|| "missing windowHandle".to_string())?);
            let adapter = state.window_adapter(window_index)?;
            let window = adapter.window();
            let response = proto::WindowPropertiesResponse {
                is_fullscreen: window.is_fullscreen(),
                is_maximized: window.is_maximized(),
                is_minimized: window.is_minimized(),
                size: Some(proto::PhysicalSize {
                    width: window.size().width,
                    height: window.size().height,
                }),
                position: Some(proto::PhysicalPosition {
                    x: window.position().x,
                    y: window.position().y,
                }),
                root_element_handle: Some(index_to_handle(
                    state.root_element_handle(window_index)?,
                )),
            };
            Ok(ToolResult::Json(
                serde_json::to_value(&response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "find_elements_by_id" => {
            let p: proto::RequestFindElementsById = deserialize_params(args)?;
            let window_index =
                handle_to_index(p.window_handle.ok_or_else(|| "missing windowHandle".to_string())?);
            let elements = state.find_elements_by_id(window_index, &p.elements_id)?;
            let response = proto::ElementsResponse {
                element_handles: elements
                    .into_iter()
                    .map(|e| index_to_handle(state.element_to_handle(e)))
                    .collect(),
            };
            Ok(ToolResult::Json(
                serde_json::to_value(&response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "get_element_properties" => {
            let p: proto::RequestElementProperties = deserialize_params(args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            );
            let element = state.element("get_element_properties", element_index)?;
            let response = introspection::element_properties(&element);
            Ok(ToolResult::Json(
                serde_json::to_value(&response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "query_element_descendants" => {
            let p: proto::RequestQueryElementDescendants = deserialize_params(args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            );
            let element = state.element("query_element_descendants", element_index)?;
            let results =
                introspection::query_element_descendants(element, p.query_stack, p.find_all)?;
            let response = proto::ElementQueryResponse {
                element_handles: results
                    .into_iter()
                    .map(|e| index_to_handle(state.element_to_handle(e)))
                    .collect(),
            };
            Ok(ToolResult::Json(
                serde_json::to_value(&response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "get_element_tree" => {
            // Custom tool not in proto — returns flat list of element properties with handles.
            let element_handle: proto::Handle = args
                .get("elementHandle")
                .ok_or_else(|| "missing elementHandle".to_string())
                .and_then(|v| {
                    serde_json::from_value(v.clone())
                        .map_err(|e| format!("invalid elementHandle: {e}"))
                })?;
            let max_elements: usize =
                args.get("maxElements").and_then(|v| v.as_u64()).unwrap_or(200).clamp(1, 1000)
                    as usize;

            let root_index = handle_to_index(element_handle);
            let root_element = state.element("get_element_tree", root_index)?;

            let mut elements: Vec<Value> = Vec::new();
            let mut truncated = false;

            // Add root element
            let root_props = introspection::element_properties(&root_element);
            let mut root_node =
                serde_json::to_value(&root_props).map_err(|e| format!("serialize error: {e}"))?;
            if let Some(obj) = root_node.as_object_mut() {
                obj.insert(
                    "handle".to_string(),
                    serde_json::to_value(&index_to_handle(root_index))
                        .map_err(|e| format!("serialize error: {e}"))?,
                );
            }
            elements.push(root_node);

            root_element.visit_descendants(|child| {
                if elements.len() >= max_elements {
                    truncated = true;
                    return std::ops::ControlFlow::Break(());
                }
                let child_handle = state.element_to_handle(child.clone());
                let props = introspection::element_properties(&child);
                if let Ok(mut node) = serde_json::to_value(&props) {
                    if let Some(obj) = node.as_object_mut() {
                        if let Ok(handle_json) =
                            serde_json::to_value(&index_to_handle(child_handle))
                        {
                            obj.insert("handle".to_string(), handle_json);
                        }
                    }
                    elements.push(node);
                }
                std::ops::ControlFlow::<()>::Continue(())
            });

            Ok(ToolResult::Json(serde_json::json!({
                "elements": elements,
                "totalCount": elements.len(),
                "truncated": truncated
            })))
        }
        "take_screenshot" => {
            let p: proto::RequestTakeSnapshot = deserialize_params(args)?;
            let window_index =
                handle_to_index(p.window_handle.ok_or_else(|| "missing windowHandle".to_string())?);
            let png_data = state.take_snapshot(window_index, "image/png")?;
            Ok(ToolResult::Image {
                meta: serde_json::json!({ "sizeBytes": png_data.len() }),
                png_data,
            })
        }
        "click_element" => {
            let p: proto::RequestElementClick = deserialize_params(args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            );
            let element = state.element("click_element", element_index)?;
            let button = proto::PointerEventButton::try_from(p.button)
                .map_err(|_| format!("invalid button value: {}", p.button))?;
            let button = introspection::convert_pointer_event_button(button);
            let action = proto::ClickAction::try_from(p.action)
                .map_err(|_| format!("invalid action value: {}", p.action))?;
            match action {
                proto::ClickAction::SingleClick => element.single_click(button).await,
                proto::ClickAction::DoubleClick => element.double_click(button).await,
            }
            let response = proto::ElementClickResponse {};
            Ok(ToolResult::Json(
                serde_json::to_value(&response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "invoke_accessibility_action" => {
            let p: proto::RequestInvokeElementAccessibilityAction = deserialize_params(args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            );
            let element = state.element("invoke_accessibility_action", element_index)?;
            let action = proto::ElementAccessibilityAction::try_from(p.action)
                .map_err(|_| format!("invalid action value: {}", p.action))?;
            introspection::invoke_element_accessibility_action(&element, action);
            let response = proto::InvokeElementAccessibilityActionResponse {};
            Ok(ToolResult::Json(
                serde_json::to_value(&response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "set_element_value" => {
            let p: proto::RequestSetElementAccessibleValue = deserialize_params(args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            );
            let element = state.element("set_element_value", element_index)?;
            element.set_accessible_value(p.value);
            let response = proto::SetElementAccessibleValueResponse {};
            Ok(ToolResult::Json(
                serde_json::to_value(&response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "dispatch_key_event" => {
            // Custom tool: simplified key event dispatch (not a direct proto mapping).
            let window_handle: proto::Handle = args
                .get("windowHandle")
                .ok_or_else(|| "missing windowHandle".to_string())
                .and_then(|v| {
                    serde_json::from_value(v.clone())
                        .map_err(|e| format!("invalid windowHandle: {e}"))
                })?;
            let text: String = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing text".to_string())?
                .to_string();
            let event_type =
                args.get("eventType").and_then(|v| v.as_str()).unwrap_or("press_and_release");

            let window_index = handle_to_index(window_handle);
            let events: Vec<i_slint_core::platform::WindowEvent> = match event_type {
                "press" => {
                    vec![i_slint_core::platform::WindowEvent::KeyPressed { text: text.into() }]
                }
                "release" => {
                    vec![i_slint_core::platform::WindowEvent::KeyReleased { text: text.into() }]
                }
                "press_and_release" => vec![
                    i_slint_core::platform::WindowEvent::KeyPressed { text: text.clone().into() },
                    i_slint_core::platform::WindowEvent::KeyReleased { text: text.into() },
                ],
                other => return Err(format!("Unknown eventType: {other}")),
            };
            for event in events {
                state.dispatch_window_event(window_index, event)?;
            }
            Ok(ToolResult::Json(serde_json::json!({ "success": true })))
        }
        _ => Err(format!("Unknown tool: {name}")),
    }
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
                    "It lets you inspect and interact with the application's UI in real time.\n\n",

                    "# Workflow\n\n",
                    "1. list_windows → get window handles\n",
                    "2. get_window_properties → get size, position, and the rootElementHandle\n",
                    "3. get_element_tree (start with maxElements=50) → flat list of the UI hierarchy with types, IDs, accessibility info, and handles\n",
                    "4. Drill down: use query_element_descendants to search by type, ID, or accessible role; or find_elements_by_id for known IDs\n",
                    "5. get_element_properties → full details on a specific element\n",
                    "6. take_screenshot → visual snapshot (returned as inline image)\n",
                    "7. Interact: click_element, set_element_value, invoke_accessibility_action, dispatch_key_event\n",
                    "8. take_screenshot again to verify the effect\n\n",

                    "# Handle format\n\n",
                    "All handles are JSON objects with string-valued fields: {\"index\": \"0\", \"generation\": \"0\"}. ",
                    "Values are uint64 encoded as strings (protobuf JSON convention). ",
                    "Zero-valued fields may be omitted by the serializer, so {} means {\"index\": \"0\", \"generation\": \"0\"}. ",
                    "When sending handles back, you may omit zero fields or include them — both work.\n\n",

                    "# Enum values\n\n",
                    "Enum fields accept PascalCase strings:\n",
                    "- AccessibleRole: Unknown, Button, Checkbox, Combobox, List, Slider, Spinbox, Tab, TabList, Text, Table, Tree, ProgressIndicator, TextInput, Switch, ListItem, TabPanel, Groupbox, Image, RadioButton\n",
                    "- PointerEventButton: Left, Right, Middle\n",
                    "- ClickAction: SingleClick, DoubleClick\n",
                    "- ElementAccessibilityAction: Default_, Increment, Decrement, Expand\n",
                    "- LayoutKind: NotALayout, HorizontalLayout, VerticalLayout, GridLayout, FlexboxLayout\n",
                    "Omitted enum fields default to the first value (e.g. Left, SingleClick).\n\n",

                    "# Query instructions\n\n",
                    "query_element_descendants takes a queryStack array. Each entry is an object with exactly one field:\n",
                    "- {\"matchDescendants\": true} — search recursively (without this, only direct children match)\n",
                    "- {\"matchElementId\": \"MyComponent::button1\"} — match by qualified element ID\n",
                    "- {\"matchElementTypeName\": \"Button\"} — match by exact Slint type name\n",
                    "- {\"matchElementTypeNameOrBase\": \"TouchArea\"} — match by type or inherited base\n",
                    "- {\"matchElementAccessibleRole\": \"Button\"} — match by accessible role (PascalCase)\n",
                    "Instructions are applied in order to build a query pipeline.\n\n",

                    "# Tips\n\n",
                    "- Start with get_element_tree to understand the UI structure before making targeted queries.\n",
                    "- Element IDs are qualified: 'ComponentName::element-id'. Use get_element_tree to discover them.\n",
                    "- After clicking or setting values, take a screenshot to verify the visual result.\n",
                    "- For text input: find the TextInput element, then use set_element_value to set its content.\n",
                    "- For buttons: use click_element, or invoke_accessibility_action with 'Default_' for the default action.\n",
                    "- For sliders: use invoke_accessibility_action with 'Increment'/'Decrement', or set_element_value with the numeric value as a string.\n",
                    "- For checkboxes/switches: use click_element or invoke_accessibility_action with 'Default_'.\n"
                )
            }),
        ),
        "notifications/initialized" => {
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
// HTTP Server
// ============================================================================

async fn read_http_request(
    stream: &mut async_net::TcpStream,
    carry: Vec<u8>,
) -> Result<(String, String, Vec<(String, String)>, Vec<u8>, Vec<u8>), String> {
    let mut buf = carry;
    let header_end;
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

    let body_start = header_end + 4;
    let available = buf.len() - body_start;

    let (body, leftover) = if available >= content_length {
        let body = buf[body_start..body_start + content_length].to_vec();
        let leftover = buf[body_start + content_length..].to_vec();
        (body, leftover)
    } else {
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

fn is_localhost_origin(origin: &str) -> bool {
    let host =
        origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://")).unwrap_or("");
    let host_no_port = if host.starts_with('[') {
        host.split_once(']').map(|(h, _)| &h[1..]).unwrap_or(host)
    } else {
        host.split(':').next().unwrap_or(host)
    };
    matches!(host_no_port, "localhost" | "127.0.0.1" | "::1")
}

fn validate_origin(headers: &[(String, String)]) -> Result<Option<&str>, ()> {
    let origin = headers.iter().find(|(k, _)| k == "origin").map(|(_, v)| v.as_str());
    match origin {
        None => Ok(None),
        Some(o) if is_localhost_origin(o.trim()) => Ok(Some(o.trim())),
        Some(_) => Err(()),
    }
}

fn wants_close(headers: &[(String, String)]) -> bool {
    headers.iter().any(|(k, v)| k == "connection" && v.eq_ignore_ascii_case("close"))
}

async fn handle_connection(state: &IntrospectionState, mut stream: async_net::TcpStream) {
    let mut carry = Vec::new();

    loop {
        let (method, path, headers, body, leftover) =
            match read_http_request(&mut stream, carry).await {
                Ok(req) => req,
                Err(_) => return,
            };

        let close_after = wants_close(&headers);
        carry = leftover;

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

        // For non-browser clients (no Origin header), use "*" for CORS.
        // For browser clients, echo back the validated localhost origin.
        // Note: POST requests without Origin are still allowed — these come from
        // non-browser MCP clients (curl, SDKs). Browsers always send Origin on
        // cross-origin POST, so if Origin is present and invalid, we already
        // rejected it above.
        let cors_origin = allowed_origin.unwrap_or("*");

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
            let body_str = match String::from_utf8(body) {
                Ok(s) => s,
                Err(_) => {
                    let _ = write_http_response(
                        &mut stream,
                        400,
                        "Bad Request",
                        &[],
                        b"Request body is not valid UTF-8\n",
                    )
                    .await;
                    if close_after {
                        return;
                    }
                    continue;
                }
            };
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

    introspection::ensure_window_tracking()?;
    let state = introspection::shared_state();

    let server_started =
        Rc::new(std::cell::OnceCell::<i_slint_core::future::JoinHandle<()>>::new());
    let server_started_clone = server_started.clone();
    let state_clone = state.clone();

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

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        futures_lite::future::block_on(f)
    }

    fn make_state() -> IntrospectionState {
        IntrospectionState::new()
    }

    #[test]
    fn test_handle_roundtrip() {
        let index = generational_arena::Index::from_raw_parts(42, 7);
        let handle = index_to_handle(index);
        assert_eq!(handle.index, 42);
        assert_eq!(handle.generation, 7);
        assert_eq!(index, handle_to_index(handle));
    }

    #[test]
    fn test_validate_origin() {
        assert_eq!(validate_origin(&[]), Ok(None));
        assert!(validate_origin(&[("origin".into(), "http://localhost".into())]).is_ok());
        assert!(validate_origin(&[("origin".into(), "http://localhost:3000".into())]).is_ok());
        assert!(validate_origin(&[("origin".into(), "http://127.0.0.1:8080".into())]).is_ok());
        assert!(validate_origin(&[("origin".into(), "http://[::1]:3000".into())]).is_ok());
        assert!(validate_origin(&[("origin".into(), "http://evil.com".into())]).is_err());
        assert!(validate_origin(&[("origin".into(), "http://localhost.evil.com".into())]).is_err());
    }

    #[test]
    fn test_wants_close() {
        assert!(!wants_close(&[]));
        assert!(!wants_close(&[("connection".into(), "keep-alive".into())]));
        assert!(wants_close(&[("connection".into(), "close".into())]));
        assert!(wants_close(&[("connection".into(), "Close".into())]));
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
        assert_eq!(result["error"]["code"], -32600);
    }

    #[test]
    fn test_find_header_end() {
        assert_eq!(find_header_end(b"GET / HTTP/1.1\r\n\r\n"), Some(14));
        assert_eq!(find_header_end(b"no double crlf here"), None);
        assert_eq!(find_header_end(b"\r\n\r\n"), Some(0));
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
        assert!(resp["result"]["protocolVersion"].as_str().is_some());
        assert!(resp["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_mcp_notification_returns_none() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        ));
        assert!(resp.is_none());
    }

    #[test]
    fn test_mcp_tools_list() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        ));
        let resp = resp.unwrap();
        let tools = resp["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"list_windows"));
        assert!(names.contains(&"get_element_tree"));
        assert!(names.contains(&"take_screenshot"));
    }

    #[test]
    fn test_mcp_tools_call_list_windows() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_windows","arguments":{}}}"#,
        ));
        let resp = resp.unwrap();
        let content = &resp["result"]["content"];
        let text = content[0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        // pbjson omits empty repeated fields, so windowHandles may be absent or empty
        let handles = parsed.get("windowHandles").and_then(|v| v.as_array());
        assert!(handles.is_none() || handles.unwrap().is_empty());
    }

    #[test]
    fn test_mcp_tools_call_unknown_tool() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"nonexistent","arguments":{}}}"#,
        ));
        let resp = resp.unwrap();
        assert!(resp["result"]["isError"].as_bool().unwrap_or(false));
    }

    #[test]
    fn test_mcp_unknown_method() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":5,"method":"bogus/method"}"#,
        ));
        let resp = resp.unwrap();
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[test]
    fn test_mcp_malformed_json() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(&state, "not json"));
        let resp = resp.unwrap();
        assert_eq!(resp["error"]["code"], -32700);
    }

    #[test]
    fn test_mcp_batch_request_rejected() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"[{"jsonrpc":"2.0","id":1,"method":"initialize"}]"#,
        ));
        let resp = resp.unwrap();
        assert_eq!(resp["error"]["code"], -32600);
    }

    #[test]
    fn test_tool_definitions_structure() {
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 11);
        for tool in tools {
            assert!(tool.get("name").and_then(|v| v.as_str()).is_some());
            assert!(tool.get("description").and_then(|v| v.as_str()).is_some());
            assert_eq!(tool["inputSchema"]["type"], "object");
        }
    }

    #[test]
    fn test_proto_serde_field_names_match_tool_schemas() {
        // Verify that pbjson field names match what tool_definitions documents.
        // If a proto field is renamed, this test catches the mismatch.
        let req = proto::RequestWindowProperties {
            window_handle: Some(proto::Handle { index: 1, generation: 2 }),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("windowHandle").is_some(), "expected camelCase 'windowHandle'");
        assert_eq!(json["windowHandle"]["index"], "1");

        let req = proto::RequestFindElementsById {
            window_handle: Some(proto::Handle { index: 0, generation: 0 }),
            elements_id: "test".into(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("elementsId").is_some(), "expected camelCase 'elementsId'");

        let req = proto::RequestElementProperties {
            element_handle: Some(proto::Handle { index: 0, generation: 0 }),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("elementHandle").is_some(), "expected camelCase 'elementHandle'");
    }

    #[test]
    fn test_proto_enum_serde_format() {
        // Verify that pbjson enum serialization matches the tool descriptions.
        let json = serde_json::to_value(proto::PointerEventButton::Left).unwrap();
        assert_eq!(json, "Left");

        let json = serde_json::to_value(proto::ClickAction::SingleClick).unwrap();
        assert_eq!(json, "SingleClick");

        let json = serde_json::to_value(proto::ElementAccessibilityAction::Default).unwrap();
        assert_eq!(json, "Default_");

        // Verify round-trip: clients can send these strings back
        let button: proto::PointerEventButton = serde_json::from_value("Left".into()).unwrap();
        assert_eq!(button, proto::PointerEventButton::Left);
    }

    #[test]
    fn test_proto_enum_string_deserialization_in_struct() {
        // This is the actual MCP client path: string enum values inside a parent struct.
        // pbjson must convert "Left" → i32(0) when deserializing the parent struct's i32 field.
        let json = serde_json::json!({
            "elementHandle": { "index": "0", "generation": "0" },
            "action": "SingleClick",
            "button": "Left"
        });
        let req: proto::RequestElementClick = serde_json::from_value(json).unwrap();
        assert_eq!(req.action, proto::ClickAction::SingleClick as i32);
        assert_eq!(req.button, proto::PointerEventButton::Left as i32);

        // Also verify DoubleClick + Right
        let json = serde_json::json!({
            "elementHandle": { "index": "0", "generation": "0" },
            "action": "DoubleClick",
            "button": "Right"
        });
        let req: proto::RequestElementClick = serde_json::from_value(json).unwrap();
        assert_eq!(req.action, proto::ClickAction::DoubleClick as i32);
        assert_eq!(req.button, proto::PointerEventButton::Right as i32);

        // Verify accessibility action deserialization
        let json = serde_json::json!({
            "elementHandle": { "index": "0", "generation": "0" },
            "action": "Increment"
        });
        let req: proto::RequestInvokeElementAccessibilityAction =
            serde_json::from_value(json).unwrap();
        assert_eq!(req.action, proto::ElementAccessibilityAction::Increment as i32);
    }
}
