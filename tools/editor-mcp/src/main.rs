// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use serde::Deserialize;
use serde_json::{Value, json};
use slint_editor_mcp::{
    ChatProvider, ChatRegistration, EditorRequest, EditorResponse, discover_editors, editor_rpc,
    select_editor,
};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

const DISCOVER_TOOL: &str = "discover_visual_editors";
const SCREENSHOT_TOOL: &str = "screenshot_visual_editor_canvas";
const REGISTER_TOOL: &str = "register_visual_editor_chat";

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        if let Some(response) = handle_request(&line?) {
            serde_json::to_writer(&mut stdout, &response).map_err(io::Error::other)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn handle_request(request_text: &str) -> Option<Value> {
    let request: Value = match serde_json::from_str::<Value>(request_text) {
        Ok(request) if request.is_object() => request,
        Ok(_) => return Some(error_response(Value::Null, -32600, "Invalid request")),
        Err(error) => {
            return Some(error_response(Value::Null, -32700, &format!("Parse error: {error}")));
        }
    };
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let is_notification = request.get("id").is_none();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "slint-editor-mcp", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Discover an editor within the current working directory, then register this chat to receive annotations sent from the editor."
        })),
        "ping" => Ok(json!({})),
        "tools/list" => {
            Ok(json!({ "tools": [discover_tool(), register_tool(), screenshot_tool()] }))
        }
        "tools/call" => call_tool(request.get("params")),
        "notifications/initialized" => return None,
        _ => {
            if is_notification {
                return None;
            }
            return Some(error_response(id, -32601, &format!("Method not found: {method}")));
        }
    };
    if is_notification {
        return None;
    }
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(message) => error_response(id, -32602, &message),
    })
}

fn discover_tool() -> Value {
    json!({
        "name": DISCOVER_TOOL,
        "description": "Discover running Slint visual editors whose project roots are equal to or beneath the current chat's working directory, including editors with no annotations.",
        "inputSchema": {
            "type": "object",
            "properties": { "workingDirectory": { "type": "string", "description": "The absolute working directory of the current chat." } },
            "required": ["workingDirectory"],
            "additionalProperties": false
        },
        "annotations": { "readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false }
    })
}

fn register_tool() -> Value {
    json!({
        "name": REGISTER_TOOL,
        "description": "Register this Codex chat as a destination for annotations sent from a running visual editor within workingDirectory. Supply instanceId when multiple editors match.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "workingDirectory": { "type": "string", "description": "The absolute working directory of the current chat." },
                "instanceId": { "type": "string", "description": "The instanceId returned by discover_visual_editors." },
                "provider": { "type": "string", "enum": ["codex"] },
                "threadId": { "type": "string" },
                "displayName": { "type": "string" },
                "cliPath": { "type": "string", "description": "The absolute path to the Codex CLI executable." }
            },
            "required": ["workingDirectory", "provider", "threadId", "displayName", "cliPath"],
            "additionalProperties": false
        },
        "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false }
    })
}

fn screenshot_tool() -> Value {
    json!({
        "name": SCREENSHOT_TOOL,
        "description": "Wait for the latest source files and imports to compile and install. Capture the canvas viewport as a PNG with zoom, pan, selection, and annotation popovers. Compilation failures return current diagnostics. Supply instanceId when multiple editors match.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "workingDirectory": { "type": "string", "description": "The absolute working directory of the current chat." },
                "instanceId": { "type": "string", "description": "The instanceId returned by discover_visual_editors." }
            },
            "required": ["workingDirectory"],
            "additionalProperties": false
        },
        "annotations": { "readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false }
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EditorArguments {
    working_directory: PathBuf,
    instance_id: Option<String>,
}

fn canvas_screenshot(arguments: &Value) -> Result<Value, String> {
    let arguments: EditorArguments =
        serde_json::from_value(arguments.clone()).map_err(|error| error.to_string())?;
    let editors = discover_editors(&arguments.working_directory)?;
    let editor = select_editor(&editors, arguments.instance_id.as_deref())?;
    let response = editor_rpc(
        &editor,
        &arguments.working_directory,
        EditorRequest::CanvasScreenshot { project_root: editor.project_root.clone() },
    )?;
    let EditorResponse::CanvasScreenshot { png_base64 } = response else {
        return Err("Unexpected editor screenshot response".into());
    };
    Ok(json!({ "content": [{ "type": "image", "mimeType": "image/png", "data": png_base64 }] }))
}

fn call_tool(params: Option<&Value>) -> Result<Value, String> {
    let params = params.ok_or("Missing tool parameters")?;
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let result = match name {
        DISCOVER_TOOL => discover(&arguments),
        REGISTER_TOOL => register(&arguments),
        SCREENSHOT_TOOL => canvas_screenshot(&arguments),
        _ => return Err(format!("Unknown tool: {name}")),
    };
    Ok(result.unwrap_or_else(
        |message| json!({ "content": [{ "type": "text", "text": message }], "isError": true }),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DiscoveryArguments {
    working_directory: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistrationArguments {
    working_directory: PathBuf,
    instance_id: Option<String>,
    provider: ChatProvider,
    thread_id: String,
    display_name: String,
    cli_path: PathBuf,
}

fn discover(arguments: &Value) -> Result<Value, String> {
    let arguments: DiscoveryArguments =
        serde_json::from_value(arguments.clone()).map_err(|error| error.to_string())?;
    let editors = discover_editors(&arguments.working_directory)?;
    let editors = editors.iter().map(|editor| {
        json!({ "instanceId": editor.instance_id, "projectRoot": editor.project_root })
    }).collect::<Vec<_>>();
    Ok(json!({
        "content": [{ "type": "text", "text": format!("Found {} running visual editor(s) within workingDirectory.", editors.len()) }],
        "structuredContent": { "editors": editors }
    }))
}

fn register(arguments: &Value) -> Result<Value, String> {
    let arguments: RegistrationArguments =
        serde_json::from_value(arguments.clone()).map_err(|error| error.to_string())?;
    let chat = ChatRegistration {
        provider: arguments.provider,
        thread_id: arguments.thread_id,
        display_name: arguments.display_name,
        cli_path: arguments.cli_path,
    };
    chat.validate()?;
    let editors = discover_editors(&arguments.working_directory)?;
    let editor = select_editor(&editors, arguments.instance_id.as_deref())?;
    let response = editor_rpc(
        &editor,
        &arguments.working_directory,
        EditorRequest::RegisterChat { project_root: editor.project_root.clone(), chat },
    )?;
    let EditorResponse::ChatRegistered { chat } = response else {
        return Err("Unexpected editor registration response".into());
    };
    Ok(json!({
        "content": [{ "type": "text", "text": format!("Registered {} with the visual editor for {}.", chat.display_name, editor.project_root.display()) }],
        "structuredContent": { "instanceId": editor.instance_id, "projectRoot": editor.project_root, "chat": chat }
    }))
}

fn error_response(id: Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertises_discovery_registration_and_screenshot() {
        let initialized =
            handle_request(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#).unwrap();
        assert_eq!(initialized["result"]["capabilities"], json!({ "tools": {} }));
        let response = handle_request(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        let tools = response["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0]["name"], DISCOVER_TOOL);
        assert_eq!(tools[1]["name"], REGISTER_TOOL);
        assert_eq!(tools[2]["name"], SCREENSHOT_TOOL);
        assert!(tools.iter().all(|tool| tool.get("_meta").is_none()));
        for method in ["resources/list", "resources/read"] {
            let response = handle_request(&json!({"id":3,"method":method}).to_string()).unwrap();
            assert_eq!(response["error"]["code"], -32601);
        }
    }

    #[test]
    fn reports_bad_scope_and_unsupported_provider_as_tool_errors() {
        for (name, arguments) in [
            (DISCOVER_TOOL, json!({ "workingDirectory": "relative" })),
            (
                REGISTER_TOOL,
                json!({ "workingDirectory": "/", "provider": "other", "threadId": "one", "displayName": "Chat", "cliPath": "/bin/codex" }),
            ),
        ] {
            let response =
                call_tool(Some(&json!({ "name": name, "arguments": arguments }))).unwrap();
            assert_eq!(response["isError"], true);
        }
    }

    #[test]
    fn handles_ping_notifications_and_invalid_json() {
        let response = handle_request(r#"{"id":"ping","method":"ping"}"#).unwrap();
        assert_eq!(response["result"], json!({}));
        assert!(handle_request(r#"{"method":"notifications/initialized"}"#).is_none());
        assert_eq!(handle_request("invalid").unwrap()["error"]["code"], -32700);
    }
}
