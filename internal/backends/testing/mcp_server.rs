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
use i_slint_core::debug_log;
use serde_json::Value;
use std::rc::Rc;

use crate::introspection::{
    self, AccessibilityAction, IntrospectionState, QueryInstruction,
};

// ============================================================================
// Handle serialization
// ============================================================================

fn handle_to_json(index: generational_arena::Index) -> Value {
    let (idx, generation) = introspection::index_to_parts(index);
    serde_json::json!({ "index": idx, "generation": generation })
}

fn handle_from_json(v: &Value) -> Result<generational_arena::Index, String> {
    let index = v.get("index").and_then(|v| v.as_u64()).ok_or("missing index in handle")?;
    let generation =
        v.get("generation").and_then(|v| v.as_u64()).ok_or("missing generation in handle")?;
    Ok(introspection::parts_to_index(index, generation))
}

// ============================================================================
// MCP Tool Macro
// ============================================================================

/// Macro to declare MCP tools with automatic schema generation and dispatch.
///
/// Each tool declaration produces:
/// - An entry in the `tools/list` response (name, description, input schema)
/// - A match arm in the `tools/call` dispatch
/// - Parameter deserialization from JSON arguments
macro_rules! mcp_tools {
    (
        state: $state:ident, args: $args:ident;
        $(
            $(#[doc = $doc:literal])*
            tool $name:ident (
                $( $(#[doc = $param_doc:literal])* $param:ident : $param_ty:tt ),*
                $(,)?
            ) -> $ret:ty $body:block
        )*
    ) => {
        fn tool_definitions() -> Value {
            serde_json::json!({
                "tools": [
                    $(
                        {
                            "name": stringify!($name),
                            "description": concat!($($doc, " "),*),
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    $(
                                        stringify!($param): mcp_tools!(@schema $param_ty $(, $param_doc)*)
                                    ),*
                                },
                                "required": mcp_tools!(@required $($param: $param_ty),*)
                            }
                        }
                    ),*
                ]
            })
        }

        async fn handle_tool_call(
            $state: &IntrospectionState,
            name: &str,
            $args: &Value,
        ) -> Result<Value, String> {
            match name {
                $(
                    stringify!($name) => {
                        $(
                            let $param = mcp_tools!(@extract $args, $param, $param_ty);
                        )*
                        let result: $ret = $body;
                        result
                    }
                )*
                _ => Err(format!("Unknown tool: {name}")),
            }
        }
    };

    // Schema generation for parameter types
    (@schema Handle $(, $doc:literal)*) => {
        serde_json::json!({
            "type": "object",
            "description": concat!($($doc, " "),*),
            "properties": {
                "index": { "type": "integer" },
                "generation": { "type": "integer" }
            },
            "required": ["index", "generation"]
        })
    };
    (@schema String $(, $doc:literal)*) => {
        serde_json::json!({ "type": "string", "description": concat!($($doc, " "),*) })
    };
    (@schema OptionalString $(, $doc:literal)*) => {
        serde_json::json!({ "type": "string", "description": concat!($($doc, " "),*) })
    };
    (@schema QueryArray $(, $doc:literal)*) => {
        serde_json::json!({
            "type": "array",
            "description": concat!($($doc, " "),*),
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
        })
    };
    (@schema Bool $(, $doc:literal)*) => {
        serde_json::json!({ "type": "boolean", "description": concat!($($doc, " "),*) })
    };
    (@schema OptionalBool $(, $doc:literal)*) => {
        serde_json::json!({ "type": "boolean", "description": concat!($($doc, " "),*) })
    };
    (@schema OptionalInt $(, $doc:literal)*) => {
        serde_json::json!({ "type": "integer", "description": concat!($($doc, " "),*) })
    };

    // Required fields: Handle and String are required, Optional* are not
    (@required $($param:ident: $ty:tt),*) => {
        {
            #[allow(unused_mut)]
            let mut required = Vec::<&str>::new();
            $(
                mcp_tools!(@required_check required, $param, $ty);
            )*
            required
        }
    };
    (@required_check $vec:ident, $param:ident, Handle) => { $vec.push(stringify!($param)); };
    (@required_check $vec:ident, $param:ident, String) => { $vec.push(stringify!($param)); };
    (@required_check $vec:ident, $param:ident, QueryArray) => { $vec.push(stringify!($param)); };
    (@required_check $vec:ident, $param:ident, Bool) => { $vec.push(stringify!($param)); };
    (@required_check $vec:ident, $param:ident, $_ty:tt) => { };

    // Parameter extraction from JSON args
    (@extract $args:ident, $param:ident, Handle) => {
        handle_from_json(
            $args.get(stringify!($param))
                .ok_or_else(|| format!("missing {}", stringify!($param)))?
        )?
    };
    (@extract $args:ident, $param:ident, String) => {
        $args.get(stringify!($param))
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("missing {}", stringify!($param)))?
            .to_string()
    };
    (@extract $args:ident, $param:ident, OptionalString) => {
        $args.get(stringify!($param)).and_then(|v| v.as_str()).unwrap_or("").to_string()
    };
    (@extract $args:ident, $param:ident, QueryArray) => {
        $args.get(stringify!($param))
            .and_then(|v| v.as_array())
            .ok_or_else(|| format!("missing {}", stringify!($param)))?
            .clone()
    };
    (@extract $args:ident, $param:ident, Bool) => {
        $args.get(stringify!($param))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    };
    (@extract $args:ident, $param:ident, OptionalBool) => {
        $args.get(stringify!($param)).and_then(|v| v.as_bool())
    };
    (@extract $args:ident, $param:ident, OptionalInt) => {
        $args.get(stringify!($param)).and_then(|v| v.as_u64())
    };
}

// ============================================================================
// Tool Definitions
// ============================================================================

mcp_tools! {
    state: state, args: args;

    /// List all windows in the application. Returns window handles for use with other tools.
    /// This is typically the first tool to call.
    tool list_windows() -> Result<Value, String> {
        let handles: Vec<Value> = state.window_handles().into_iter().map(handle_to_json).collect();
        Ok(serde_json::json!({ "windows": handles }))
    }

    /// Get properties of a window (size, position, fullscreen/maximized/minimized state,
    /// root element handle). The root_element_handle is the entry point for element tree traversal.
    tool get_window_properties(
        /// Window handle from list_windows.
        window_handle: Handle,
    ) -> Result<Value, String> {
        let wp = state.window_properties(window_handle)?;
        Ok(serde_json::json!({
            "is_fullscreen": wp.is_fullscreen,
            "is_maximized": wp.is_maximized,
            "is_minimized": wp.is_minimized,
            "size": { "width": wp.size.width, "height": wp.size.height },
            "position": { "x": wp.position.x, "y": wp.position.y },
            "root_element_handle": handle_to_json(wp.root_element_handle)
        }))
    }

    /// Find elements by their qualified ID (e.g. 'App::mybutton'). Returns element handles.
    /// Use get_element_tree first to discover available element IDs.
    tool find_elements_by_id(
        /// Window handle.
        window_handle: Handle,
        /// Qualified element ID (e.g. 'App::mybutton').
        element_id: String,
    ) -> Result<Value, String> {
        let elements = state.find_elements_by_id(window_handle, &element_id)?;
        let handles: Vec<Value> = elements
            .into_iter()
            .map(|e| handle_to_json(state.element_to_handle(e)))
            .collect();
        Ok(serde_json::json!({ "elements": handles }))
    }

    /// Get all properties of an element: type info, accessible properties (label, value,
    /// description, role, checked, enabled, etc.), geometry (position, size), and opacity.
    tool get_element_properties(
        /// Element handle.
        element_handle: Handle,
    ) -> Result<Value, String> {
        let element = state.element("get_element_properties", element_handle)?;
        Ok(element_properties_to_json(&state.element_properties(&element)))
    }

    /// Query descendants of an element using a chain of match instructions. Each instruction
    /// narrows the search. Use match_descendants to search recursively, then filter by id,
    /// type_name, or accessible_role. More efficient than get_element_tree for targeted searches.
    tool query_element_descendants(
        /// Element handle to start the query from.
        element_handle: Handle,
        /// Array of query instructions applied in order.
        query: QueryArray,
        /// If true, return all matches. If false, return only the first match.
        find_all: OptionalBool,
    ) -> Result<Value, String> {
        let element = state.element("query_element_descendants", element_handle)?;
        let instructions = parse_query_instructions(&query)?;
        let find_all = find_all.unwrap_or(true);
        let results = state.query_element_descendants(element, instructions, find_all)?;
        let handles: Vec<Value> = results
            .into_iter()
            .map(|e| handle_to_json(state.element_to_handle(e)))
            .collect();
        Ok(serde_json::json!({ "elements": handles }))
    }

    /// Get all elements in the subtree starting from a root element. Returns a flat list of
    /// elements with their properties and handles. Use this to get an overview of the UI, then
    /// use get_element_properties or query_element_descendants for targeted exploration.
    /// Limit the result count with max_elements (default: 200, max: 1000).
    tool get_element_tree(
        /// Root element handle (typically from get_window_properties root_element_handle).
        element_handle: Handle,
        /// Maximum number of elements to return (default: 200, max: 1000).
        max_elements: OptionalInt,
    ) -> Result<Value, String> {
        let max_elements = max_elements.unwrap_or(200).min(1000) as usize;
        collect_element_list(state, element_handle, max_elements)
    }

    /// Take a screenshot of a window. Returns an MCP image content block.
    tool take_screenshot(
        /// Window handle.
        window_handle: Handle,
    ) -> Result<Value, String> {
        let png_data = state.take_snapshot(window_handle, "image/png")?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
        Ok(serde_json::json!({
            "_mcp_image": {
                "data": b64,
                "mimeType": "image/png"
            },
            "size_bytes": png_data.len()
        }))
    }

    /// Simulate a mouse click on an element.
    tool click_element(
        /// Element handle.
        element_handle: Handle,
        /// Click action: "single_click" (default) or "double_click".
        action: OptionalString,
        /// Mouse button: "left" (default), "right", or "middle".
        button: OptionalString,
    ) -> Result<Value, String> {
        let element = state.element("click_element", element_handle)?;
        let button = match if button.is_empty() { "left" } else { &button } {
            "left" => i_slint_core::platform::PointerEventButton::Left,
            "right" => i_slint_core::platform::PointerEventButton::Right,
            "middle" => i_slint_core::platform::PointerEventButton::Middle,
            other => return Err(format!("Unknown button: {other}")),
        };
        match if action.is_empty() { "single_click" } else { &action } {
            "single_click" => element.single_click(button).await,
            "double_click" => element.double_click(button).await,
            other => return Err(format!("Unknown click action: {other}")),
        };
        Ok(serde_json::json!({ "success": true }))
    }

    /// Invoke an accessibility action on an element (e.g. default action for buttons,
    /// increment/decrement for sliders, expand for combo boxes).
    tool invoke_accessibility_action(
        /// Element handle.
        element_handle: Handle,
        /// The action: "default", "increment", "decrement", or "expand".
        action: String,
    ) -> Result<Value, String> {
        let element = state.element("invoke_accessibility_action", element_handle)?;
        let action = match action.as_str() {
            "default" => AccessibilityAction::Default,
            "increment" => AccessibilityAction::Increment,
            "decrement" => AccessibilityAction::Decrement,
            "expand" => AccessibilityAction::Expand,
            other => return Err(format!("Unknown action: {other}")),
        };
        state.invoke_element_accessibility_action(&element, action)?;
        Ok(serde_json::json!({ "success": true }))
    }

    /// Set the accessible value of an element (e.g. text input content, slider value).
    tool set_element_value(
        /// Element handle.
        element_handle: Handle,
        /// The value to set.
        value: String,
    ) -> Result<Value, String> {
        let element = state.element("set_element_value", element_handle)?;
        element.set_accessible_value(value);
        Ok(serde_json::json!({ "success": true }))
    }

    /// Dispatch a keyboard event to a window.
    tool dispatch_key_event(
        /// Window handle.
        window_handle: Handle,
        /// The key text to send.
        text: String,
        /// Event type: "press", "release", or "press_and_release" (default).
        event_type: OptionalString,
    ) -> Result<Value, String> {
        let event_type = if event_type.is_empty() { "press_and_release".to_string() } else { event_type };
        let events: Vec<i_slint_core::platform::WindowEvent> = match event_type.as_str() {
            "press" => vec![
                i_slint_core::platform::WindowEvent::KeyPressed { text: text.clone().into() },
            ],
            "release" => vec![
                i_slint_core::platform::WindowEvent::KeyReleased { text: text.clone().into() },
            ],
            "press_and_release" => vec![
                i_slint_core::platform::WindowEvent::KeyPressed { text: text.clone().into() },
                i_slint_core::platform::WindowEvent::KeyReleased { text: text.into() },
            ],
            other => return Err(format!("Unknown event_type: {other}")),
        };
        for event in events {
            state.dispatch_window_event(window_handle, event)?;
        }
        Ok(serde_json::json!({ "success": true }))
    }
}

// ============================================================================
// Helper functions
// ============================================================================

fn parse_query_instructions(arr: &[Value]) -> Result<Vec<QueryInstruction>, String> {
    let mut instructions = Vec::new();
    for item in arr {
        let instr =
            if item.get("match_descendants").and_then(|v| v.as_bool()).unwrap_or(false) {
                QueryInstruction::MatchDescendants
            } else if let Some(id) = item.get("match_id").and_then(|v| v.as_str()) {
                QueryInstruction::MatchId(id.to_string())
            } else if let Some(tn) = item.get("match_type_name").and_then(|v| v.as_str()) {
                QueryInstruction::MatchTypeName(tn.to_string())
            } else if let Some(tn) =
                item.get("match_type_name_or_base").and_then(|v| v.as_str())
            {
                QueryInstruction::MatchTypeNameOrBase(tn.to_string())
            } else if let Some(role_str) =
                item.get("match_accessible_role").and_then(|v| v.as_str())
            {
                let role = introspection::string_to_accessible_role(role_str)
                    .ok_or_else(|| format!("Unknown accessible role: {role_str}"))?;
                QueryInstruction::MatchAccessibleRole(role)
            } else {
                return Err("Invalid query instruction".into());
            };
        instructions.push(instr);
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
    root_node
        .as_object_mut()
        .unwrap()
        .insert("handle".to_string(), handle_to_json(root_handle));
    elements.push(root_node);

    root_element.visit_descendants(|child| {
        if elements.len() >= max_elements {
            truncated = true;
            return std::ops::ControlFlow::Break(());
        }
        let child_handle = state.element_to_handle(child.clone());
        let props = state.element_properties(&child);
        let mut node = element_properties_to_json(&props);
        node.as_object_mut()
            .unwrap()
            .insert("handle".to_string(), handle_to_json(child_handle));
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
        Err(e) => {
            return Some(json_rpc_error(
                &Value::Null,
                -32700,
                format!("Parse error: {e}"),
            ))
        }
    };

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
                    "3. get_element_tree (max_depth=2-3) — explore the UI hierarchy\n",
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
                    let content = if let Some(img) = result.get("_mcp_image") {
                        let mut blocks = vec![serde_json::json!({
                            "type": "image",
                            "data": img.get("data").and_then(|v| v.as_str()).unwrap_or(""),
                            "mimeType": img.get("mimeType").and_then(|v| v.as_str()).unwrap_or("image/png")
                        })];
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

    if is_notification {
        None
    } else {
        Some(response)
    }
}

// ============================================================================
// HTTP Server (using httparse + async-net)
// ============================================================================

/// Read a complete HTTP request from a stream.
/// Returns (method, path, headers, body).
async fn read_http_request(
    stream: &mut async_net::TcpStream,
) -> Result<(String, String, Vec<(String, String)>, Vec<u8>), String> {
    // Read headers into a buffer. We read in chunks until we find \r\n\r\n.
    let mut buf = Vec::with_capacity(4096);
    let header_end;
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
        .map(|h| {
            (
                h.name.to_ascii_lowercase(),
                String::from_utf8_lossy(h.value).to_string(),
            )
        })
        .collect();

    // Read body based on Content-Length (capped at 4 MB to prevent OOM)
    const MAX_BODY_SIZE: usize = 4 * 1024 * 1024;
    let content_length: usize = parsed_headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    if content_length > MAX_BODY_SIZE {
        return Err(format!("body too large: {content_length} bytes (max {MAX_BODY_SIZE})"));
    }

    // Body bytes we already have (after the header end)
    let body_start = header_end + 4; // skip \r\n\r\n
    let already_read = buf.len() - body_start;
    let mut body = buf[body_start..].to_vec();

    // Read remaining body bytes
    if already_read < content_length {
        let remaining = content_length - already_read;
        let mut rest = vec![0u8; remaining];
        stream.read_exact(&mut rest).await.map_err(|e| format!("body read error: {e}"))?;
        body.extend_from_slice(&rest);
    }

    Ok((method, path, parsed_headers, body))
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
    response.push_str("Connection: close\r\n");
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
    let host = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
        .unwrap_or("");
    // Strip optional port
    let host_no_port = if host.starts_with('[') {
        // IPv6: [::1]:port
        host.split_once(']').map(|(h, _)| &h[1..]).unwrap_or(host)
    } else {
        host.split(':').next().unwrap_or(host)
    };
    matches!(host_no_port, "localhost" | "127.0.0.1" | "::1" | "0.0.0.0")
}

fn validate_origin(headers: &[(String, String)]) -> bool {
    let origin = headers.iter().find(|(k, _)| k == "origin").map(|(_, v)| v.as_str());
    match origin {
        None => true, // No Origin header — non-browser client (curl, MCP SDK, etc.)
        Some(o) => is_localhost_origin(o.trim()),
    }
}

async fn handle_connection(state: &IntrospectionState, mut stream: async_net::TcpStream) {
    let (method, path, headers, body) = match read_http_request(&mut stream).await {
        Ok(req) => req,
        Err(e) => {
            debug_log!("MCP HTTP: failed to read request: {e}");
            return;
        }
    };

    // Validate Origin header to prevent DNS rebinding attacks.
    if !validate_origin(&headers) {
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

    // Handle CORS preflight for browser-based MCP clients.
    if method == "OPTIONS" {
        let origin = headers
            .iter()
            .find(|(k, _)| k == "origin")
            .map(|(_, v)| v.as_str())
            .unwrap_or("http://localhost");
        let cors_headers = [
            ("Access-Control-Allow-Origin", origin),
            ("Access-Control-Allow-Methods", "POST, OPTIONS"),
            ("Access-Control-Allow-Headers", "Content-Type, MCP-Protocol-Version, Mcp-Session-Id"),
            ("Access-Control-Max-Age", "86400"),
            ("Vary", "Origin"),
        ];
        let _ = write_http_response(&mut stream, 204, "No Content", &cors_headers, b"").await;
        return;
    }

    // Only accept POST /mcp (or POST / for flexibility)
    if method != "POST" || (path != "/mcp" && path != "/") {
        let body = b"404 Not Found\n";
        let _ = write_http_response(&mut stream, 404, "Not Found", &[], body).await;
        return;
    }

    let body_str = String::from_utf8_lossy(&body);
    let response = handle_mcp_request(state, &body_str).await;

    // Mirror the request's Origin back (already validated above).
    let origin = headers
        .iter()
        .find(|(k, _)| k == "origin")
        .map(|(_, v)| v.as_str())
        .unwrap_or("http://localhost");
    let resp_headers = [
        ("Content-Type", "application/json"),
        ("Access-Control-Allow-Origin", origin),
        ("Vary", "Origin"),
    ];

    match response {
        Some(resp) => {
            let json = serde_json::to_string(&resp).unwrap();
            let _ =
                write_http_response(&mut stream, 200, "OK", &resp_headers, json.as_bytes()).await;
        }
        None => {
            // Notification — 202 Accepted with no body
            let _ = write_http_response(&mut stream, 202, "Accepted", &resp_headers, b"").await;
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
                debug_log!("MCP server: accept error: {e}");
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
    let server_started = Rc::new(std::cell::OnceCell::<i_slint_core::future::JoinHandle<()>>::new());
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
        let back = handle_from_json(&json).unwrap();
        assert_eq!(index, back);

        // Test with larger values
        let index = generational_arena::Index::from_raw_parts(42, 7);
        let json = handle_to_json(index);
        assert_eq!(json["index"], 42);
        assert_eq!(json["generation"], 7);
        let back = handle_from_json(&json).unwrap();
        assert_eq!(index, back);
    }

    #[test]
    fn test_handle_from_json_missing_fields() {
        let v = serde_json::json!({});
        assert!(handle_from_json(&v).is_err());

        let v = serde_json::json!({"index": 0});
        assert!(handle_from_json(&v).is_err());

        let v = serde_json::json!({"generation": 0});
        assert!(handle_from_json(&v).is_err());
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
        assert!(matches!(&instructions[3], QueryInstruction::MatchTypeNameOrBase(n) if n == "TouchArea"));
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
    fn test_parse_query_instructions_unknown_role() {
        let arr = vec![serde_json::json!({"match_accessible_role": "nonexistent"})];
        assert!(parse_query_instructions(&arr).is_err());
    }

    #[test]
    fn test_validate_origin() {
        // No Origin header — allowed (non-browser client)
        assert!(validate_origin(&[]));

        // Localhost origins — allowed
        assert!(validate_origin(&[("origin".into(), "http://localhost".into())]));
        assert!(validate_origin(&[("origin".into(), "http://localhost:3000".into())]));
        assert!(validate_origin(&[("origin".into(), "https://localhost:8443".into())]));
        assert!(validate_origin(&[("origin".into(), "http://127.0.0.1:8080".into())]));
        assert!(validate_origin(&[("origin".into(), "http://[::1]:3000".into())]));
        assert!(validate_origin(&[("origin".into(), "http://0.0.0.0:8080".into())]));

        // External origins — rejected
        assert!(!validate_origin(&[("origin".into(), "http://evil.com".into())]));
        assert!(!validate_origin(&[("origin".into(), "http://localhost.evil.com".into())]));
        assert!(!validate_origin(&[("origin".into(), "https://attacker.com".into())]));

        // Edge cases
        assert!(!validate_origin(&[("origin".into(), "http://127.0.0.2".into())]));
        assert!(!validate_origin(&[("origin".into(), "".into())]));
        assert!(!validate_origin(&[("origin".into(), "localhost".into())])); // no scheme
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
    fn test_mcp_missing_method() {
        let state = make_state();
        // Valid JSON-RPC but no method field — treated as unknown method ""
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":6}"#,
        ));
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
}
