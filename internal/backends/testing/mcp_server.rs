// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Embedded MCP (Model Context Protocol) server for introspecting Slint applications.
//!
//! When enabled via the `SLINT_MCP_PORT` environment variable, the application starts
//! an HTTP server implementing MCP's Streamable HTTP transport, allowing MCP clients
//! (e.g. Claude) to inspect and interact with the running UI.
//!
//! `SLINT_MCP_HOST` sets the address to bind to, defaulting to `127.0.0.1`.
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

use crate::introspection::{self, IntrospectionState, dispatch, proto};
use i_slint_core::SharedString;
use i_slint_core::item_tree::{ItemRc, ItemTreeRc};
use i_slint_core::items::AccessibleRole;
use introspection::{handle_to_index, index_to_handle};

// ============================================================================
// Tool definitions (schema driven by proto messages)
// ============================================================================

mod mcp_schemas {
    include!(concat!(env!("OUT_DIR"), "/mcp_schemas.rs"));
}

/// Metadata for each MCP tool, mapping it to its proto request message.
struct ToolDef {
    name: &'static str,
    description: &'static str,
    /// Name of the proto Request* message whose fields define the input schema.
    request_type: &'static str,
    /// camelCase field names that are optional (everything else is required).
    optional_fields: &'static [&'static str],
    /// See "Snapshot" in docs/development/mcp-server.md.
    offers_snapshot: bool,
}

const TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "list_windows",
        description: "List all open windows. Returns an array of window handles. Call this first to discover available windows.",
        request_type: "RequestWindowListMessage",
        optional_fields: &[],
        offers_snapshot: false,
    },
    ToolDef {
        name: "get_window_properties",
        description: "Get a window's physical size (pixels), position, scale factor, fullscreen/maximized/minimized state, and rootElementHandle — the entry point for element tree traversal.",
        request_type: "RequestWindowProperties",
        optional_fields: &[],
        offers_snapshot: false,
    },
    ToolDef {
        name: "get_element_tree",
        description: "Get a flat list of elements in the subtree rooted at the given element. Each entry includes type names, IDs, accessibility properties, geometry, and a handle for further queries. Each entry also has a depth (0 for the given element) and, except for the given element, a parentHandle: the handle of its nearest ancestor in the list. Use maxElements to control the result size (default: 200, max: 1000). If truncated is true, there are more elements — use query_element_descendants for targeted searches instead. To explore a UI, get_element_snapshot is far more compact.",
        request_type: "RequestGetElementTree",
        optional_fields: &["maxElements"],
        offers_snapshot: false,
    },
    ToolDef {
        name: "get_element_snapshot",
        description: "Get a compact text listing of every window, or of the subtree rooted at elementHandle, with one line per element on screen worth acting on or checking: elements with an accessible role or @testable properties, and input handlers such as a TouchArea or Flickable that no element with a role contains. Use it to explore a UI and to check the effect of an action; it costs far fewer tokens than get_element_tree. A line reads: [index:generation] Type role=Role id=Component::id \"label\" value=\"...\" placeholder=\"...\" description=\"...\" accessible-id=\"...\", then states (checked, unchecked, expanded, collapsed, selected, disabled, read-only, opacity=0) and @testable properties as @name=value. Empty parts are left out, and role only appears when it differs from the type name. A Text that repeats the label of the element it's listed under is left out. [index:generation] is the element handle {\"index\": \"index\", \"generation\": \"generation\"}. Texts longer than 80 characters end in …. Lines are indented under their nearest listed ancestor. maxElements limits the number of lines per window (default: 200, max: 1000).",
        request_type: "RequestGetElementTree",
        optional_fields: &["elementHandle", "maxElements"],
        offers_snapshot: false,
    },
    ToolDef {
        name: "get_element_properties",
        description: "Get full details of a single element: type names and IDs (including inherited bases), all accessible properties (role, label, value, description, checked, enabled, read-only, placeholder, value min/max/step), logical size and position, computed opacity, layout kind, and testableProperties: the element's @testable properties with name, type and current value (booleans and numbers typed; length in logical px, duration in ms, angle in deg; colors as #rrggbbaa; enums as their .slint spelling; non-finite floats stay strings such as \"inf\"). Prefer reading testableProperties over screenshot diffing to verify state.",
        request_type: "RequestElementProperties",
        optional_fields: &[],
        offers_snapshot: false,
    },
    ToolDef {
        name: "find_elements_by_id",
        description: "Find elements by qualified ID (format: 'ComponentName::element-id', e.g. 'App::my-button'). Returns element handles. Use get_element_tree first to discover available IDs.",
        request_type: "RequestFindElementsById",
        optional_fields: &[],
        offers_snapshot: false,
    },
    ToolDef {
        name: "query_element_descendants",
        description: "Search the descendants of an element, at any depth, using a query pipeline: queryStack, an array of instructions applied in order. More efficient than get_element_tree for targeted lookups. Only elements on screen match. With scrollIntoView, the search also covers content scrolled out of view and ListView rows that don't exist yet, by paging through each Flickable at or below the element that holds repeated elements, such as a ListView. It then scrolls the first match into view and returns only that match, or nothing if scrolling can't reveal it.",
        request_type: "RequestQueryElementDescendants",
        optional_fields: &["findAll", "scrollIntoView"],
        offers_snapshot: true,
    },
    ToolDef {
        name: "take_screenshot",
        description: "Capture a PNG screenshot of a window. Returns an MCP image content block rendered inline by the client. Use after interactions to verify visual results.",
        request_type: "RequestTakeSnapshot",
        optional_fields: &["imageMimeType"],
        offers_snapshot: false,
    },
    ToolDef {
        name: "click_element",
        description: "Simulate a mouse click at the center of an element. Omit action/button for a left single-click (the most common case).",
        request_type: "RequestElementClick",
        optional_fields: &["action", "button", "modifiers"],
        offers_snapshot: true,
    },
    ToolDef {
        name: "drag_element",
        description: "Simulate a drag gesture from the element's center to a target position (logical coordinates). The pointer is pressed at the element center, moved in interpolated steps to the target, then released. Use for sliders, scrollable areas, drag handles, or any element that responds to pointer movement while pressed.",
        request_type: "RequestElementDrag",
        optional_fields: &["button", "modifiers"],
        offers_snapshot: true,
    },
    ToolDef {
        name: "hover_element",
        description: "Move the mouse pointer to the center of an element without pressing any button. Use this for hover states, tooltips and menu highlighting: click_element and drag_element both press, so they cannot produce a hover on their own. The pointer stays where it is left; move it away with move_pointer to check that a hover is cleared.",
        request_type: "RequestHoverElement",
        optional_fields: &["modifiers"],
        offers_snapshot: true,
    },
    ToolDef {
        name: "move_pointer",
        description: "Move the mouse pointer to a position in the window, in logical coordinates, without pressing any button. Use hover_element to hover an element by handle; use this to move to an arbitrary point, such as away from an element to check that its hover state is cleared.",
        request_type: "RequestMovePointer",
        optional_fields: &["modifiers"],
        offers_snapshot: true,
    },
    ToolDef {
        name: "scroll_element",
        description: "Send a mouse wheel event over the center of an element. deltaX and deltaY are logical pixels: a deltaY of -50 moves a Flickable's viewport up by 50, revealing content further down, which is what a wheel-down does. Use this to scroll a Flickable or ScrollView, and for wheel-driven gestures such as zooming a canvas.",
        request_type: "RequestScrollElement",
        optional_fields: &["deltaX", "deltaY", "modifiers"],
        offers_snapshot: true,
    },
    ToolDef {
        name: "dispatch_pointer_scroll",
        description: "Send a mouse wheel event at a position in the window, in logical coordinates. Use scroll_element to scroll an element by handle; use this for an arbitrary point.",
        request_type: "RequestPointerScroll",
        optional_fields: &["deltaX", "deltaY", "modifiers"],
        offers_snapshot: true,
    },
    ToolDef {
        name: "scroll_element_into_view",
        description: "Scroll the Flickables that contain an element, such as a ScrollView or ListView, as far as they can to make the element visible. Call it before clicking an element that is partially scrolled out of view. To find an element that is scrolled out of view, or a ListView row that doesn't exist yet, use query_element_descendants with scrollIntoView instead.",
        request_type: "RequestScrollElementIntoView",
        optional_fields: &[],
        offers_snapshot: true,
    },
    ToolDef {
        name: "invoke_accessibility_action",
        description: "Invoke an accessibility action: 'Default_' (activate buttons, toggle checkboxes), 'Increment'/'Decrement' (sliders, spinboxes), 'Expand' (combo boxes). Preferred over click_element when the element's role suggests a semantic action.",
        request_type: "RequestInvokeElementAccessibilityAction",
        optional_fields: &[],
        offers_snapshot: true,
    },
    ToolDef {
        name: "set_element_value",
        description: "Set the accessible value of an element. For text inputs: sets the text content. For sliders: pass the numeric value as a string (e.g. '42'). For other elements: sets whatever the element exposes as its accessible value.",
        request_type: "RequestSetElementAccessibleValue",
        optional_fields: &[],
        offers_snapshot: true,
    },
    ToolDef {
        name: "dispatch_key_event",
        description: "Send a keyboard event to a window. Use 'PressAndRelease' (default) for typing characters. Use 'Press'/'Release' separately for modifier keys or key combinations.",
        request_type: "RequestDispatchKeyEvent",
        optional_fields: &["eventType"],
        offers_snapshot: true,
    },
    ToolDef {
        name: "start_event_recording",
        description: "Clear the event log and begin recording window/input events. Call this before the interaction you want to observe, then call stop_event_recording when done.",
        request_type: "RequestStartEventRecording",
        optional_fields: &[],
        offers_snapshot: false,
    },
    ToolDef {
        name: "stop_event_recording",
        description: "Stop recording and return all events collected since the last start_event_recording call. The response includes an events array, droppedCount (events evicted when the 1024-entry cap was reached), and unknownEventCount (non-zero indicates a Slint bug: an event variant has no proto mapping). Use to verify that Slint received and processed pointer, key, resize, scale, close, and active-state events.",
        request_type: "RequestStopEventRecording",
        optional_fields: &[],
        offers_snapshot: false,
    },
];

const SNAPSHOT_ARGUMENT: &str = "snapshot";
const RECTANGLES_ARGUMENT: &str = "rectangles";

fn take_flag(args: &mut Value, name: &str) -> bool {
    args.as_object_mut()
        .and_then(|args| args.remove(name))
        .is_some_and(|value| value == Value::Bool(true))
}

/// Human-readable description for a handle-typed input field. Window and element
/// handles are structurally identical (`{index, generation}` objects), so without
/// these descriptions their input schemas would be byte-for-byte identical and
/// agents routinely confuse the two. The text spells out where each kind of
/// handle comes from and that they are not interchangeable.
fn handle_field_description(field_name: &str) -> Option<&'static str> {
    match field_name {
        "windowHandle" => Some(
            "A window handle (NOT an element handle). Obtain it from list_windows. \
             Window and element handles look identical but are not interchangeable: \
             passing an element handle here is incorrect.",
        ),
        "elementHandle" => Some(
            "An element handle (NOT a window handle). Obtain it from get_element_snapshot, \
             get_element_tree, find_elements_by_id, query_element_descendants, or a window's rootElementHandle \
             (from get_window_properties). Window and element handles look identical but are \
             not interchangeable: passing a window handle here is incorrect.",
        ),
        _ => None,
    }
}

/// Annotate handle-typed properties of an input schema with a `description` so
/// `windowHandle` and `elementHandle` read differently to the client.
fn annotate_handle_fields(schema: &mut Value) {
    let Some(props) = schema.get_mut("properties").and_then(|p| p.as_object_mut()) else {
        return;
    };
    for (name, prop) in props.iter_mut() {
        if let (Some(desc), Some(obj)) = (handle_field_description(name), prop.as_object_mut()) {
            obj.insert("description".to_string(), Value::String(desc.to_string()));
        }
    }
}

/// Keyed by JSON pointer below the `queryStack` property.
const QUERY_STACK_FIELD_DESCRIPTIONS: &[(&str, &str)] = &[
    ("", "Each instruction is an object with exactly one field."),
    (
        "/items/properties/matchDescendants",
        "Continue the search below the elements matched so far, as in \
         [{\"matchElementId\": \"App::list\"}, {\"matchDescendants\": true}, \
         {\"matchElementAccessibleRole\": \"Checkbox\"}]. \
         Don't start with it: the search already covers all descendants, \
         and a second pass repeats results and misses direct children.",
    ),
    ("/items/properties/matchElementId", "A qualified id, such as \"App::ok-button\"."),
    ("/items/properties/matchElementAccessibleLabel", "An exact accessible label."),
    (
        "/items/properties/matchElementAccessibleProperty",
        "Compared as a string, also for a slider's Value.",
    ),
    (
        "/items/properties/matchElementTestableProperty",
        "The value is typed as get_element_properties reports it, such as true or 42.",
    ),
];

fn tool_definitions() -> Value {
    let tools: Vec<Value> = TOOLS
        .iter()
        .map(|def| {
            let mut schema =
                mcp_schemas::proto_input_schema(def.request_type).unwrap_or_else(|| {
                    panic!("no proto schema for {}", def.request_type);
                });

            annotate_handle_fields(&mut schema);

            if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
                if def.offers_snapshot {
                    properties.insert(
                        SNAPSHOT_ARGUMENT.into(),
                        serde_json::json!({
                            "type": "boolean",
                            "description": "Also return a get_element_snapshot listing of every window, taken after the action."
                        }),
                    );
                }
                if def.name == "get_element_snapshot" {
                    properties.insert(
                        RECTANGLES_ARGUMENT.into(),
                        serde_json::json!({
                            "type": "boolean",
                            "description": "End each line in the element's rectangle as (x,y widthxheight), in the window's logical pixels, as move_pointer takes positions."
                        }),
                    );
                }
            }

            // See stringify_testable_property_matches.
            if let Some(value) = schema.pointer_mut(
                "/properties/queryStack/items/properties/matchElementTestableProperty/properties/value",
            ) {
                *value = serde_json::json!({ "type": ["string", "number", "boolean"] });
            }

            for (pointer, description) in QUERY_STACK_FIELD_DESCRIPTIONS {
                if let Some(field) = schema
                    .pointer_mut(&format!("/properties/queryStack{pointer}"))
                    .and_then(Value::as_object_mut)
                {
                    field.insert("description".into(), (*description).into());
                }
            }

            // Add "required" array: all fields except those listed as optional
            if let Some(all_fields) = mcp_schemas::proto_field_names(def.request_type) {
                let required: Vec<&str> = all_fields
                    .iter()
                    .filter(|f| !def.optional_fields.contains(f))
                    .copied()
                    .collect();
                if !required.is_empty() {
                    schema.as_object_mut().unwrap().insert(
                        "required".to_string(),
                        Value::Array(
                            required.into_iter().map(|s| Value::String(s.into())).collect(),
                        ),
                    );
                }
            }

            serde_json::json!({
                "name": def.name,
                "description": def.description,
                "inputSchema": schema,
            })
        })
        .collect();

    serde_json::json!({ "tools": tools })
}

// ============================================================================
// Tool dispatch
// ============================================================================

fn deserialize_params<T: serde::de::DeserializeOwned>(args: &Value) -> Result<T, String> {
    T::deserialize(args).map_err(|e| format!("Invalid parameters: {e}"))
}

/// Tool call result: a JSON value (rendered as text), plain text, or an image with optional metadata.
enum ToolResult {
    Json(Value),
    Text(String),
    Image { png_data: Vec<u8>, meta: Value },
}

const DEFAULT_MAX_ELEMENTS: usize = 200;

fn max_elements(requested: u32) -> usize {
    if requested == 0 { DEFAULT_MAX_ELEMENTS } else { (requested as usize).clamp(1, 1000) }
}

/// Types a `@testable` value by the property's declared type:
/// booleans become JSON booleans, numeric types JSON numbers.
/// Non-finite numbers and every other type stay strings.
fn typed_testable_value(type_name: &str, value: &str) -> Value {
    let typed = match type_name {
        "bool" => match value {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        "int" | "duration" => value.parse::<i64>().ok().map(Value::from),
        "float" | "length" | "angle" | "percent" | "relative-font-size" | "physical-length" => {
            value.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map(Value::Number)
        }
        _ => None,
    };
    typed.unwrap_or_else(|| Value::String(value.into()))
}

/// Rewrites each `testableProperties` entry's string `value` with [`typed_testable_value`].
/// Applied to every response that embeds an `ElementPropertiesResponse`,
/// so `get_element_properties` and `get_element_tree` report one encoding.
fn convert_testable_property_values(element_node: &mut Value) {
    let Some(obj) = element_node.as_object_mut() else { return };
    // The availability flag is protobuf-facing; this layer reports unavailability
    // through a note on the get_element_properties response instead.
    obj.remove("testablePropertiesAvailable");
    let Some(props) = obj.get_mut("testableProperties").and_then(Value::as_array_mut) else {
        return;
    };
    for prop in props {
        let Some(obj) = prop.as_object_mut() else { continue };
        let Some(type_name) = obj.get("typeName").and_then(Value::as_str) else { continue };
        let Some(value) = obj.get("value").and_then(Value::as_str) else { continue };
        let typed = typed_testable_value(type_name, value);
        obj.insert("value".into(), typed);
    }
}

/// The inverse of [`convert_testable_property_values`], for `matchElementTestableProperty` values.
fn stringify_testable_property_matches(query_stack: &mut Value) {
    let Some(instructions) = query_stack.as_array_mut() else { return };
    for instruction in instructions {
        let Some(value) = ["matchElementTestableProperty", "match_element_testable_property"]
            .into_iter()
            .find(|key| instruction.get(key).is_some())
            .and_then(|key| instruction[key].get_mut("value"))
        else {
            continue;
        };
        let encoded = match value {
            Value::Bool(b) => i_slint_core::debug_info::format_bool(*b),
            Value::Number(n) => match n.as_i64() {
                Some(n) => i_slint_core::debug_info::format_integer(n),
                None => match n.as_f64() {
                    Some(n) => i_slint_core::debug_info::format_float(n as f32),
                    None => continue,
                },
            },
            _ => continue,
        };
        *value = Value::String(encoded.into());
    }
}

fn ancestor_positions(
    elements: &[crate::ElementHandle],
    popups: &[(ItemRc, ItemTreeRc)],
) -> Vec<Option<usize>> {
    let mut positions: std::collections::HashMap<_, Vec<usize>> = Default::default();
    for (position, element) in elements.iter().enumerate() {
        if let Some(hint) = element.identity_hint() {
            positions.entry(hint).or_default().push(position);
        }
    }
    let position_of = |element: &crate::ElementHandle| {
        positions
            .get(&element.identity_hint()?)?
            .iter()
            .copied()
            .find(|position| elements[*position].is_same_element(element))
    };
    elements
        .iter()
        .map(|element| ancestors(element, popups).find_map(|ancestor| position_of(&ancestor)))
        .collect()
}

fn ancestors<'a>(
    element: &crate::ElementHandle,
    popups: &'a [(ItemRc, ItemTreeRc)],
) -> impl Iterator<Item = crate::ElementHandle> + 'a {
    std::iter::successors(element.parent_element(popups), |e| e.parent_element(popups))
}

fn accessible_role_of(element: &crate::ElementHandle) -> Option<AccessibleRole> {
    element.accessible_role().filter(|role| *role != AccessibleRole::None)
}

const INPUT_HANDLER_TYPES: &[&str] =
    &["TouchArea", "Flickable", "SwipeGestureHandler", "ScaleRotateGestureHandler"];

fn handles_input(element: &crate::ElementHandle) -> bool {
    element
        .type_name()
        .into_iter()
        .chain(element.bases().into_iter().flatten())
        .any(|type_name| INPUT_HANDLER_TYPES.contains(&type_name.as_str()))
}

struct SnapshotEntry {
    element: crate::ElementHandle,
    role: Option<AccessibleRole>,
    testable_properties: Vec<(SharedString, SharedString)>,
}

/// See docs/development/mcp-server.md for which elements are listed.
fn snapshot_entry(
    element: crate::ElementHandle,
    popups: &[(ItemRc, ItemTreeRc)],
) -> Option<SnapshotEntry> {
    let role = accessible_role_of(&element);
    let testable_properties = element.testable_properties().unwrap_or_default();
    let listed = role.is_some()
        || !testable_properties.is_empty()
        || (handles_input(&element)
            && !ancestors(&element, popups)
                .any(|ancestor| accessible_role_of(&ancestor).is_some()));
    if !listed
        || (role == Some(AccessibleRole::Text)
            && testable_properties.is_empty()
            && repeats_parent_label(&element, popups))
    {
        return None;
    }
    Some(SnapshotEntry { element, role, testable_properties })
}

fn repeats_parent_label(element: &crate::ElementHandle, popups: &[(ItemRc, ItemTreeRc)]) -> bool {
    let Some(label) = element.accessible_label().filter(|label| !label.is_empty()) else {
        return false;
    };
    ancestors(element, popups)
        .find_map(|ancestor| snapshot_entry(ancestor, popups))
        .is_some_and(|parent| parent.element.accessible_label() == Some(label))
}

#[derive(Clone, Copy)]
struct SnapshotOptions {
    max_lines: usize,
    rectangles: bool,
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        Self { max_lines: DEFAULT_MAX_ELEMENTS, rectangles: false }
    }
}

const SNAPSHOT_TEXT_MAX_CHARS: usize = 80;

fn snapshot_text(text: &str) -> String {
    let text = match text.char_indices().nth(SNAPSHOT_TEXT_MAX_CHARS) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.into(),
    };
    serde_json::to_string(&text).unwrap()
}

fn write_snapshot_line(
    out: &mut String,
    state: &IntrospectionState,
    entry: &SnapshotEntry,
    depth: usize,
    rectangles: bool,
) {
    use std::fmt::Write;
    let element = &entry.element;
    let handle = index_to_handle(state.element_to_handle(element.clone()));
    let _ = write!(out, "{}[{}:{}]", "  ".repeat(depth), handle.index, handle.generation);

    let type_name = element.type_name().unwrap_or_default();
    if !type_name.is_empty() {
        let _ = write!(out, " {type_name}");
    }
    if let Some(role) = entry
        .role
        .and_then(introspection::convert_to_proto_accessible_role)
        .map(|role| role.as_str_name())
        && !role.eq_ignore_ascii_case(&type_name)
    {
        let _ = write!(out, " role={role}");
    }
    if let Some(id) = element.id().filter(|id| !id.is_empty()) {
        let _ = write!(out, " id={id}");
    }
    if let Some(label) = element.accessible_label().filter(|label| !label.is_empty()) {
        let _ = write!(out, " {}", snapshot_text(&label));
    }
    for (key, text) in [
        ("value", element.accessible_value()),
        ("placeholder", element.accessible_placeholder_text()),
        ("description", element.accessible_description()),
        ("accessible-id", element.accessible_id()),
    ] {
        if let Some(text) = text.filter(|text| !text.is_empty()) {
            let _ = write!(out, " {key}={}", snapshot_text(&text));
        }
    }

    if element.accessible_checkable() == Some(true) {
        let checked = element.accessible_checked() == Some(true);
        out.push_str(if checked { " checked" } else { " unchecked" });
    }
    if element.accessible_expandable() == Some(true) {
        let expanded = element.accessible_expanded() == Some(true);
        out.push_str(if expanded { " expanded" } else { " collapsed" });
    }
    if element.accessible_item_selected() == Some(true) {
        out.push_str(" selected");
    }
    if element.accessible_enabled() == Some(false) || element.input_handler_disabled() {
        out.push_str(" disabled");
    }
    if element.accessible_read_only() == Some(true) {
        out.push_str(" read-only");
    }
    if element.computed_opacity() == 0. {
        out.push_str(" opacity=0");
    }

    for (name, type_name) in &entry.testable_properties {
        let _ = write!(out, " @{name}");
        if let Some(value) = element.testable_property_value(name) {
            match typed_testable_value(type_name, &value) {
                Value::String(text) => {
                    let _ = write!(out, "={}", snapshot_text(&text));
                }
                typed => {
                    let _ = write!(out, "={typed}");
                }
            }
        }
    }

    if rectangles {
        // Adding 0 turns -0 into 0.
        let rounded = |value: f32| value.round() + 0.;
        let position = element.absolute_origin();
        let size = element.size();
        let _ = write!(
            out,
            " ({},{} {}x{})",
            rounded(position.x),
            rounded(position.y),
            rounded(size.width),
            rounded(size.height)
        );
    }
    out.push('\n');
}

fn element_snapshot(
    state: &IntrospectionState,
    root: &crate::ElementHandle,
    options: SnapshotOptions,
) -> String {
    use std::fmt::Write;
    use std::ops::ControlFlow;
    let SnapshotOptions { max_lines, rectangles } = options;

    if !root.has_debug_info() {
        return format!(
            "# No type names, ids or descendants are available. {}\n",
            crate::search_api::MISSING_DEBUG_INFO_MESSAGE
        );
    }

    let popups = root.active_popups();
    let mut on_screen = 0;
    let mut listed = Vec::new();
    let mut truncated = false;
    let mut visit = |element: crate::ElementHandle| {
        on_screen += 1;
        if let Some(entry) = snapshot_entry(element, &popups) {
            if listed.len() == max_lines {
                truncated = true;
                return ControlFlow::Break(());
            }
            listed.push(entry);
        }
        ControlFlow::Continue(())
    };
    if visit(root.clone()).is_continue() {
        root.visit_descendants(&mut visit);
    }

    let mut out = String::new();
    if truncated {
        let _ = write!(
            out,
            "# The first {max_lines} listed elements; more are on screen. \
             Pass a deeper elementHandle or a larger maxElements."
        );
    } else {
        let _ = write!(out, "# {} of {on_screen} elements on screen.", listed.len());
    }
    out.push_str(" [i:g] is the element handle {\"index\": \"i\", \"generation\": \"g\"}.\n");

    let mut children = vec![Vec::new(); listed.len()];
    let mut top_level = Vec::new();
    let elements: Vec<_> = listed.iter().map(|entry| entry.element.clone()).collect();
    for (position, parent) in ancestor_positions(&elements, &popups).into_iter().enumerate() {
        match parent {
            Some(parent) => children[parent].push(position),
            None => top_level.push(position),
        }
    }
    let mut stack: Vec<_> = top_level.into_iter().rev().map(|position| (position, 0)).collect();
    while let Some((position, depth)) = stack.pop() {
        write_snapshot_line(&mut out, state, &listed[position], depth, rectangles);
        stack.extend(children[position].iter().rev().map(|child| (*child, depth + 1)));
    }
    out
}

fn windows_snapshot(state: &IntrospectionState, options: SnapshotOptions) -> String {
    let mut out = String::new();
    for window in state.window_handles() {
        let handle = index_to_handle(window);
        out.push_str(&format!("# Window handle [{}:{}]\n", handle.index, handle.generation));
        match state
            .root_element_handle(window)
            .and_then(|root| state.element("get_element_snapshot", root))
        {
            Ok(root) => out.push_str(&element_snapshot(state, &root, options)),
            Err(error) => out.push_str(&format!("# {error}\n")),
        }
    }
    out
}

fn tree_depth(position: usize, parents: &[Option<usize>], depths: &mut [Option<usize>]) -> usize {
    if let Some(depth) = depths[position] {
        return depth;
    }
    let depth = parents[position].map_or(0, |parent| tree_depth(parent, parents, depths) + 1);
    depths[position] = Some(depth);
    depth
}

/// Assembles the `get_element_tree` response.
///
/// Without debug info the walk yields no descendants and no names, so the response
/// says so rather than looking like a UI with one unnamed element (#13225).
fn element_tree_response(elements: Vec<Value>, truncated: bool, has_debug_info: bool) -> Value {
    let total_count = elements.len();
    let mut response = serde_json::json!({
        "elements": elements,
        "totalCount": total_count,
        "truncated": truncated
    });
    if !has_debug_info && let Some(object) = response.as_object_mut() {
        object.insert("debugInfoMissing".into(), Value::Bool(true));
        object.insert(
            "note".into(),
            Value::String(format!(
                "No descendants, type names or ids are available. {}",
                crate::search_api::MISSING_DEBUG_INFO_MESSAGE
            )),
        );
    }
    response
}

async fn handle_tool_call(
    state: &IntrospectionState,
    name: &str,
    args: &Value,
) -> Result<ToolResult, String> {
    // Report on the current tree: see `IntrospectionState::ensure_windows_instantiated`.
    state.ensure_windows_instantiated();

    match name {
        "list_windows" => {
            let response = dispatch::list_windows(state);
            Ok(ToolResult::Json(
                serde_json::to_value(response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "get_window_properties" => {
            let p: proto::RequestWindowProperties = deserialize_params(args)?;
            let window_index = handle_to_index(
                p.window_handle.ok_or_else(|| "missing windowHandle".to_string())?,
            )?;
            let response = dispatch::window_properties(state, window_index)?;
            Ok(ToolResult::Json(
                serde_json::to_value(response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "find_elements_by_id" => {
            let p: proto::RequestFindElementsById = deserialize_params(args)?;
            let window_index = handle_to_index(
                p.window_handle.ok_or_else(|| "missing windowHandle".to_string())?,
            )?;
            let response = dispatch::find_elements_by_id(state, window_index, &p.elements_id)?;
            Ok(ToolResult::Json(
                serde_json::to_value(response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "get_element_properties" => {
            let p: proto::RequestElementProperties = deserialize_params(args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            )?;
            let response = dispatch::element_properties(state, element_index)?;
            let available = response.testable_properties_available;
            let mut response =
                serde_json::to_value(response).map_err(|e| format!("serialize error: {e}"))?;
            convert_testable_property_values(&mut response);
            if !available && let Some(object) = response.as_object_mut() {
                let element = state.element("get_element_properties", element_index)?;
                if element.has_debug_info() {
                    object.insert(
                        "note".into(),
                        Value::String(
                            "Testable property values are not available: this application \
                             was generated without testable-property introspection."
                                .into(),
                        ),
                    );
                } else {
                    object.insert("debugInfoMissing".into(), Value::Bool(true));
                    object.insert(
                        "note".into(),
                        Value::String(format!(
                            "No testable properties, type names or ids are available. {}",
                            crate::search_api::MISSING_DEBUG_INFO_MESSAGE
                        )),
                    );
                }
            }
            Ok(ToolResult::Json(response))
        }
        "query_element_descendants" => {
            let mut args = args.clone();
            if let Some(query_stack) = args.get_mut("queryStack") {
                stringify_testable_property_matches(query_stack);
            }
            let p: proto::RequestQueryElementDescendants = deserialize_params(&args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            )?;
            let response = dispatch::query_element_descendants(
                state,
                element_index,
                p.query_stack,
                p.find_all,
                p.scroll_into_view,
            )?;
            Ok(ToolResult::Json(
                serde_json::to_value(response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "get_element_tree" => {
            let p: proto::RequestGetElementTree = deserialize_params(args)?;
            let element_handle =
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?;
            let max_elements = max_elements(p.max_elements);

            let root_index = handle_to_index(element_handle)?;
            let root_element = state.element("get_element_tree", root_index)?;

            let mut walked = vec![root_element.clone()];
            let mut truncated = false;
            root_element.visit_descendants(|child| {
                if walked.len() >= max_elements {
                    truncated = true;
                    return std::ops::ControlFlow::Break(());
                }
                walked.push(child);
                std::ops::ControlFlow::<()>::Continue(())
            });

            let handles: Vec<_> =
                walked.iter().map(|e| state.element_to_handle(e.clone())).collect();
            let parents = ancestor_positions(&walked, &root_element.active_popups());
            let mut depths = vec![None; walked.len()];

            let mut elements = Vec::with_capacity(walked.len());
            for (position, element) in walked.iter().enumerate() {
                let mut node = serde_json::to_value(introspection::element_properties(element))
                    .map_err(|e| format!("serialize error: {e}"))?;
                convert_testable_property_values(&mut node);
                if let Some(obj) = node.as_object_mut() {
                    let to_json = |index| {
                        serde_json::to_value(index_to_handle(index))
                            .map_err(|e| format!("serialize error: {e}"))
                    };
                    obj.insert("handle".into(), to_json(handles[position])?);
                    if let Some(parent) = parents[position] {
                        obj.insert("parentHandle".into(), to_json(handles[parent])?);
                    }
                    obj.insert("depth".into(), tree_depth(position, &parents, &mut depths).into());
                }
                elements.push(node);
            }

            Ok(ToolResult::Json(element_tree_response(
                elements,
                truncated,
                root_element.has_debug_info(),
            )))
        }
        "get_element_snapshot" => {
            let mut args = args.clone();
            let rectangles = take_flag(&mut args, RECTANGLES_ARGUMENT);
            let p: proto::RequestGetElementTree = deserialize_params(&args)?;
            let options = SnapshotOptions { max_lines: max_elements(p.max_elements), rectangles };
            let Some(element_handle) = p.element_handle else {
                return Ok(ToolResult::Text(windows_snapshot(state, options)));
            };
            let root_element =
                state.element("get_element_snapshot", handle_to_index(element_handle)?)?;
            Ok(ToolResult::Text(element_snapshot(state, &root_element, options)))
        }
        "take_screenshot" => {
            let p: proto::RequestTakeSnapshot = deserialize_params(args)?;
            let window_index = handle_to_index(
                p.window_handle.ok_or_else(|| "missing windowHandle".to_string())?,
            )?;
            let response = dispatch::take_snapshot(state, window_index, "image/png")?;
            let png_data = response.window_contents_as_encoded_image;
            Ok(ToolResult::Image {
                meta: serde_json::json!({ "sizeBytes": png_data.len() }),
                png_data,
            })
        }
        "click_element" => {
            let p: proto::RequestElementClick = deserialize_params(args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            )?;
            let button = proto::PointerEventButton::try_from(p.button)
                .map_err(|_| format!("invalid button value: {}", p.button))?;
            let action = proto::ClickAction::try_from(p.action)
                .map_err(|_| format!("invalid action value: {}", p.action))?;
            dispatch::click(state, element_index, action, button, p.modifiers).await?;
            let response = proto::ElementClickResponse {};
            Ok(ToolResult::Json(
                serde_json::to_value(response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "hover_element" => {
            let p: proto::RequestHoverElement = deserialize_params(args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            )?;
            dispatch::move_pointer_to_element(state, element_index, p.modifiers.as_ref())?;
            Ok(ToolResult::Json(serde_json::json!({})))
        }
        "move_pointer" => {
            let p: proto::RequestMovePointer = deserialize_params(args)?;
            let window_index = handle_to_index(
                p.window_handle.ok_or_else(|| "missing windowHandle".to_string())?,
            )?;
            let position = p.position.ok_or_else(|| "missing position".to_string())?;
            let _modifiers = introspection::HeldModifiers::hold(
                state.window_adapter(window_index)?,
                p.modifiers.as_ref(),
            );
            state.dispatch_window_event(
                window_index,
                i_slint_core::platform::WindowEvent::PointerMoved {
                    position: i_slint_core::api::LogicalPosition::new(position.x, position.y),
                },
            )?;
            Ok(ToolResult::Json(serde_json::json!({})))
        }
        "scroll_element" => {
            let p: proto::RequestScrollElement = deserialize_params(args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            )?;
            dispatch::scroll_element(
                state,
                element_index,
                p.delta_x,
                p.delta_y,
                p.modifiers.as_ref(),
            )?;
            Ok(ToolResult::Json(serde_json::json!({})))
        }
        "dispatch_pointer_scroll" => {
            let p: proto::RequestPointerScroll = deserialize_params(args)?;
            let window_index = handle_to_index(
                p.window_handle.ok_or_else(|| "missing windowHandle".to_string())?,
            )?;
            let position = p.position.ok_or_else(|| "missing position".to_string())?;
            let _modifiers = introspection::HeldModifiers::hold(
                state.window_adapter(window_index)?,
                p.modifiers.as_ref(),
            );
            state.dispatch_window_event(
                window_index,
                i_slint_core::platform::WindowEvent::PointerScrolled {
                    position: i_slint_core::api::LogicalPosition::new(position.x, position.y),
                    delta_x: p.delta_x,
                    delta_y: p.delta_y,
                },
            )?;
            Ok(ToolResult::Json(serde_json::json!({})))
        }
        "scroll_element_into_view" => {
            let p: proto::RequestScrollElementIntoView = deserialize_params(args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            )?;
            let response = dispatch::scroll_element_into_view(state, element_index)?;
            Ok(ToolResult::Json(
                serde_json::to_value(response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "drag_element" => {
            let p: proto::RequestElementDrag = deserialize_params(args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            )?;
            let target = p.target.ok_or_else(|| "missing target position".to_string())?;
            let button = proto::PointerEventButton::try_from(p.button)
                .map_err(|_| format!("invalid button value: {}", p.button))?;
            dispatch::drag(state, element_index, target, button, p.modifiers).await?;
            let response = proto::ElementDragResponse {};
            Ok(ToolResult::Json(
                serde_json::to_value(response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "invoke_accessibility_action" => {
            let p: proto::RequestInvokeElementAccessibilityAction = deserialize_params(args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            )?;
            let action = proto::ElementAccessibilityAction::try_from(p.action)
                .map_err(|_| format!("invalid action value: {}", p.action))?;
            dispatch::invoke_accessibility_action(state, element_index, action)?;
            let response = proto::InvokeElementAccessibilityActionResponse {};
            Ok(ToolResult::Json(
                serde_json::to_value(response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "set_element_value" => {
            let p: proto::RequestSetElementAccessibleValue = deserialize_params(args)?;
            let element_index = handle_to_index(
                p.element_handle.ok_or_else(|| "missing elementHandle".to_string())?,
            )?;
            dispatch::set_accessible_value(state, element_index, p.value)?;
            let response = proto::SetElementAccessibleValueResponse {};
            Ok(ToolResult::Json(
                serde_json::to_value(response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "dispatch_key_event" => {
            let p: proto::RequestDispatchKeyEvent = deserialize_params(args)?;
            let window_index = handle_to_index(
                p.window_handle.ok_or_else(|| "missing windowHandle".to_string())?,
            )?;
            let event_type = proto::KeyEventType::try_from(p.event_type)
                .map_err(|_| format!("invalid eventType value: {}", p.event_type))?;
            let events: Vec<i_slint_core::platform::WindowEvent> = match event_type {
                proto::KeyEventType::Press => {
                    vec![i_slint_core::platform::WindowEvent::KeyPressed { text: p.text.into() }]
                }
                proto::KeyEventType::Release => {
                    vec![i_slint_core::platform::WindowEvent::KeyReleased { text: p.text.into() }]
                }
                proto::KeyEventType::PressAndRelease => vec![
                    i_slint_core::platform::WindowEvent::KeyPressed { text: p.text.clone().into() },
                    i_slint_core::platform::WindowEvent::KeyReleased { text: p.text.into() },
                ],
            };
            for event in events {
                state.dispatch_window_event(window_index, event)?;
            }
            let response = proto::DispatchKeyEventResponse {};
            Ok(ToolResult::Json(
                serde_json::to_value(response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "start_event_recording" => {
            let response = dispatch::start_event_recording(state);
            Ok(ToolResult::Json(
                serde_json::to_value(response).map_err(|e| format!("serialize error: {e}"))?,
            ))
        }
        "stop_event_recording" => {
            let response = dispatch::stop_event_recording(state);
            Ok(ToolResult::Json(
                serde_json::to_value(response).map_err(|e| format!("serialize error: {e}"))?,
            ))
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
                    "1. get_element_snapshot without arguments → compact text listing of every window, each headed by its window handle, with one line per element worth acting on or checking: handle, type, id, label, value, state and @testable properties. ",
                    "Pass rectangles: true to also get each element's rectangle, or an elementHandle to list only its subtree. ",
                    "Use get_element_tree (start with maxElements=50) only when you need every field of every element; it costs far more tokens\n",
                    "2. Drill down: use query_element_descendants to search by type, ID, accessible role or property, or @testable property value; or find_elements_by_id for known IDs\n",
                    "3. get_element_properties → full details on a specific element\n",
                    "4. list_windows and get_window_properties → window handles, and a window's size, position, scale factor and rootElementHandle\n",
                    "5. take_screenshot → visual snapshot (returned as inline image)\n",
                    "6. Interact: click_element, drag_element, set_element_value, invoke_accessibility_action, dispatch_key_event\n",
                    "   Pass snapshot: true to any of these, or to the hover, pointer and scroll tools and query_element_descendants, to also get a get_element_snapshot listing of every window after the action.\n",
                    "   Wheel: scroll_element (an element's center) or dispatch_pointer_scroll (a position).\n",
                    "   Hover without pressing: hover_element (an element's center) or move_pointer (a position). ",
                    "click_element and drag_element both press, so neither can produce a hover on its own. ",
                    "The pointer stays where it is left, so clear a hover with move_pointer to a point away from the element.\n",
                    "   Scrolling into view: query_element_descendants with scrollIntoView finds an element scrolled out of view, or a ListView row that doesn't exist yet, and scrolls to it. ",
                    "scroll_element_into_view scrolls to an element you already have a handle for.\n",
                    "7. start_event_recording → then interact → stop_event_recording to verify the runtime received and processed expected input/window events\n",
                    "8. take_screenshot again to verify the visual effect\n\n",

                    "# Requirements\n\n",
                    "Element type names, ids and testable properties need the application to be built with `SLINT_EMIT_DEBUG_INFO=1`, ",
                    "or with `with_debug_info` in `slint_build`'s `CompilerConfiguration`. ",
                    "Without it get_element_tree returns the root element alone, unnamed, and find_elements_by_id fails.\n\n",

                    "# Modifiers\n\n",
                    "click_element, drag_element, hover_element, scroll_element and dispatch_pointer_scroll accept a `modifiers` object with `shift`, `control`, `alt` and `meta`. ",
                    "The keys are held for the gesture and released after it, which is how a shift-click or a ctrl+wheel zoom is expressed as one call.\n\n",

                    "# Handle format\n\n",
                    "All handles are JSON objects with string-valued fields: {\"index\": \"0\", \"generation\": \"0\"}. ",
                    "Values are uint64 encoded as strings (protobuf JSON convention). ",
                    "Zero-valued fields may be omitted by the serializer, so {} means {\"index\": \"0\", \"generation\": \"0\"}. ",
                    "When sending handles back, you may omit zero fields or include them — both work. ",
                    "get_element_snapshot writes a handle as [index:generation]; send it back as {\"index\": \"index\", \"generation\": \"generation\"}. ",
                    "A handle stays the same for as long as the element lives, so the same element keeps its handle across queries. ",
                    "Re-query when a call reports an invalid or destroyed handle.\n\n",

                    "IMPORTANT: window handles and element handles are SEPARATE, NON-INTERCHANGEABLE kinds, ",
                    "even though they share this {index, generation} shape. ",
                    "Window handles come from list_windows, or from the \"# Window handle\" lines of get_element_snapshot (use them for windowHandle parameters). ",
                    "Element handles come from get_element_snapshot, get_element_tree, find_elements_by_id, query_element_descendants, ",
                    "or a window's rootElementHandle (use them for elementHandle parameters). ",
                    "Do not reuse a window handle as an element handle or vice versa — they are not interchangeable.\n\n",

                    "# Enum values\n\n",
                    "Enum fields accept PascalCase strings:\n",
                    "- AccessibleRole: Unknown, Button, Checkbox, Combobox, List, Slider, Spinbox, Tab, TabList, Text, Table, Tree, ProgressIndicator, TextInput, Switch, ListItem, TabPanel, Groupbox, Image, RadioButton, RadioGroup, Banner, Complementary, ContentInfo, Form, Main, Navigation, Region, Search\n",
                    "- PointerEventButton: Left, Right, Middle, Back, Forward, Other\n",
                    "- ClickAction: SingleClick, DoubleClick\n",
                    "- ElementAccessibilityAction: Default_, Increment, Decrement, Expand\n",
                    "- KeyEventType: PressAndRelease, Press, Release\n",
                    "- RecordedEventResult: Unspecified, Accepted, Rejected (Unspecified appears only on malformed data)\n",
                    "- LayoutKind: NotALayout, HorizontalLayout, VerticalLayout, GridLayout, FlexboxLayout\n",
                    "- AccessibleStringProperty: Label, Value, Description, PlaceholderText, Id\n",
                    "- TextMatchMode: Exact, ContainsIgnoringCase\n",
                    "Omitted enum fields default to the first value (e.g. Left, SingleClick, PressAndRelease).\n\n",

                    "# Query instructions\n\n",
                    "query_element_descendants takes a queryStack array of instructions, applied in order to build a query pipeline. ",
                    "Its input schema describes each instruction.\n\n",

                    "# Tips\n\n",
                    "- Start with get_element_snapshot to understand the UI structure before making targeted queries.\n",
                    "- Element IDs are qualified: 'ComponentName::element-id'. Use get_element_snapshot to discover them.\n",
                    "- To verify state after an interaction, read the @testable properties in a snapshot, or testableProperties via get_element_properties, instead of diffing screenshots. ",
                    "Only properties declared `@testable` in the .slint source are listed, and only in builds with debug info. ",
                    "`@testable` is experimental: don't add it to a project that doesn't enable the `testable` experimental feature.\n",
                    "- After clicking or setting values, take a screenshot to verify the visual result.\n",
                    "- For text input: find the TextInput element, then use set_element_value to set its content.\n",
                    "- For buttons: use click_element, or invoke_accessibility_action with 'Default_' for the default action.\n",
                    "- For sliders: use invoke_accessibility_action with 'Increment'/'Decrement', set_element_value with the numeric value as a string, or drag_element to drag the thumb to a position.\n",
                    "- For drag gestures (scrollable areas, drag handles, custom sliders): use drag_element with the target position in logical coordinates.\n",
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
            let mut tool_args = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));
            let snapshot_after =
                TOOLS.iter().any(|def| def.name == tool_name && def.offers_snapshot)
                    && take_flag(&mut tool_args, SNAPSHOT_ARGUMENT);

            match handle_tool_call(state, tool_name, &tool_args).await {
                Ok(result) => {
                    let mut content = match result {
                        ToolResult::Image { png_data, meta } => {
                            let b64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
                            let mut blocks = vec![serde_json::json!({
                                "type": "image",
                                "data": b64,
                                "mimeType": "image/png"
                            })];
                            if !meta.as_object().is_none_or(|o| o.is_empty()) {
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
                        ToolResult::Text(text) => {
                            vec![serde_json::json!({ "type": "text", "text": text })]
                        }
                    };
                    if snapshot_after {
                        state.ensure_windows_instantiated();
                        content.push(serde_json::json!({
                            "type": "text",
                            "text": windows_snapshot(state, SnapshotOptions::default())
                        }));
                    }
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
    buf.array_windows().position(|w| w == b"\r\n\r\n")
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

// IPv6 hosts need brackets in the socket address so the port isn't parsed as part of the address.
fn bind_address(host: &str, port: u16) -> String {
    if host.starts_with('[') || !host.contains(':') {
        format!("{host}:{port}")
    } else {
        format!("[{host}]:{port}")
    }
}

async fn run_server(
    state: Rc<IntrospectionState>,
    host: std::borrow::Cow<'static, str>,
    port: u16,
) {
    let addr = bind_address(&host, port);
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
                i_slint_core::with_existing_context(|context| {
                    let _ = context.spawn_local(async move {
                        handle_connection(&state, stream).await;
                    });
                })
                .expect("uninitialized platform");
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

thread_local! {
    static INIT_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

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
    let host = std::env::var("SLINT_MCP_HOST")
        .map(std::borrow::Cow::Owned)
        .unwrap_or(std::borrow::Cow::Borrowed("127.0.0.1"));

    if INIT_INSTALLED.with(|installed| installed.get()) {
        return Ok(());
    }

    introspection::ensure_window_tracking()?;
    let state = introspection::shared_state();

    // The JoinHandle is kept alive inside the OnceCell so the server task is not dropped.
    let server_started =
        Rc::new(std::cell::OnceCell::<i_slint_core::future::JoinHandle<()>>::new());
    let server_started_clone = server_started.clone();
    let state_clone = state.clone();

    // Mark as installed before registering the hook so re-entrant calls to init() are rejected.
    INIT_INSTALLED.with(|installed| installed.set(true));

    let previous_hook = i_slint_core::context::set_window_shown_hook(None)
        .map_err(|_| EventLoopError::NoEventLoopProvider)?;
    let previous_hook = std::cell::RefCell::new(previous_hook);

    if i_slint_core::context::set_window_shown_hook(Some(Box::new(move |adapter| {
        if let Some(prev) = previous_hook.borrow_mut().as_mut() {
            prev(adapter);
        }

        // OnceCell ensures we only spawn once even if the hook fires multiple times.
        if server_started_clone.get().is_some() {
            return;
        }

        let host = host.clone();
        let state = state_clone.clone();
        let spawn_result = i_slint_core::with_existing_context(|context| {
            context.spawn_local(async move { run_server(state, host, port).await })
        })
        .expect("uninitialized platform");
        match spawn_result {
            Ok(join_handle) => {
                let _ = server_started_clone.set(join_handle);
            }
            // spawn_local fails when no event-loop proxy is available yet. The hook
            // will fire again on the next window-show, so this is non-fatal.
            Err(e) => {
                i_slint_core::debug_log!("MCP server failed to start: {e:?}");
            }
        }
    })))
    .is_err()
    {
        INIT_INSTALLED.with(|installed| installed.set(false));
        return Err(EventLoopError::NoEventLoopProvider);
    }

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
        let index = handle_to_index(proto::Handle { index: 42, generation: 7 }).unwrap();
        let handle = index_to_handle(index);
        assert_eq!(handle.index, 42);
        assert_eq!(handle.generation, 7);
        assert_eq!(index, handle_to_index(handle).unwrap());
    }

    #[test]
    fn test_mcp_rejects_noncanonical_handle() {
        let state = make_state();
        let resp = block_on(handle_mcp_request(
            &state,
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"get_window_properties","arguments":{"windowHandle":{"index":"42","generation":"6"}}}}"#,
        ));
        let resp = resp.unwrap();
        assert!(resp["result"]["isError"].as_bool().unwrap_or(false));
        assert!(resp["result"]["content"][0]["text"].as_str().unwrap().contains("Invalid handle"));
    }

    #[test]
    fn test_bind_address() {
        assert_eq!(bind_address("127.0.0.1", 8080), "127.0.0.1:8080");
        assert_eq!(bind_address("0.0.0.0", 8080), "0.0.0.0:8080");
        assert_eq!(bind_address("::1", 8080), "[::1]:8080");
        assert_eq!(bind_address("[::1]", 8080), "[::1]:8080");
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
    fn test_tool_call_instantiates_pending_repeaters() {
        use slint::ComponentHandle;

        crate::init_no_event_loop();
        slint::slint! {
            #[slint_edition = "1.19"]
            export component App inherits Window {
                in property <int> count: 3;
                probe := VerticalLayout {
                    for _ in root.count: Rectangle { height: 10px; }
                }
                measured := Rectangle {
                    height: probe.preferred-height;
                }
            }
        }

        let app = App::new().unwrap();
        let adapter = i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter();
        let state = make_state();
        state.add_window(&adapter);

        let measured = crate::ElementHandle::find_by_element_id(&app, "App::measured")
            .next()
            .expect("measured element");
        let element_handle = index_to_handle(state.element_to_handle(measured));

        // The model changes between events,
        // so the repeater instances stay stale until the next instantiation pass.
        app.set_count(5);

        let result = block_on(handle_tool_call(
            &state,
            "get_element_properties",
            &serde_json::json!({
                "elementHandle": serde_json::to_value(element_handle).unwrap()
            }),
        ))
        .expect("get_element_properties failed");
        let ToolResult::Json(value) = result else { panic!("expected a json result") };
        assert_eq!(value["size"]["height"], 50.0);
    }

    #[test]
    fn test_scroll_element_scrolls_a_flickable() {
        use slint::ComponentHandle;

        crate::init_no_event_loop();
        slint::slint! {
            #[slint_edition = "1.19"]
            export component App inherits Window {
                width: 100px;
                height: 100px;
                out property <length> scrolled: flick.content-y;
                flick := Flickable {
                    width: 100%;
                    height: 100%;
                    content-width: 100px;
                    content-height: 400px;
                    Rectangle { background: #abc; }
                }
            }
        }

        let app = App::new().unwrap();
        let adapter = i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter();
        let state = make_state();
        state.add_window(&adapter);
        let window_handle = index_to_handle(state.window_handles()[0]);
        let root = index_to_handle(
            state.root_element_handle(handle_to_index(window_handle).unwrap()).unwrap(),
        );

        assert_eq!(app.get_scrolled(), 0.);

        block_on(handle_tool_call(
            &state,
            "scroll_element",
            &serde_json::json!({
                "elementHandle": serde_json::to_value(root).unwrap(),
                "deltaY": -50.0
            }),
        ))
        .expect("scroll_element failed");
        let by_element = app.get_scrolled();
        assert_eq!(by_element, -50., "the wheel event didn't reach the Flickable");

        block_on(handle_tool_call(
            &state,
            "dispatch_pointer_scroll",
            &serde_json::json!({
                "windowHandle": serde_json::to_value(window_handle).unwrap(),
                "position": { "x": 50.0, "y": 50.0 },
                "deltaY": -50.0
            }),
        ))
        .expect("dispatch_pointer_scroll failed");
        assert_eq!(app.get_scrolled(), -100., "the second wheel event had no effect");
    }

    #[test]
    fn test_query_scroll_into_view_creates_list_view_row() {
        use slint::ComponentHandle;

        crate::init_no_event_loop();
        slint::slint! {
            #[slint_edition = "1.19"]
            import { ListView } from "std-widgets.slint";
            export component App inherits Window {
                width: 100px;
                height: 100px;
                ListView {
                    for row in 1000: Text { text: "Row " + row; height: 20px; }
                }
            }
        }

        let app = App::new().unwrap();
        let adapter = i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter();
        let state = make_state();
        state.add_window(&adapter);
        let root = index_to_handle(state.root_element_handle(state.window_handles()[0]).unwrap());
        let query = |scroll_into_view: bool| {
            let result = block_on(handle_tool_call(
                &state,
                "query_element_descendants",
                &serde_json::json!({
                    "elementHandle": serde_json::to_value(root).unwrap(),
                    "queryStack": [{ "matchElementAccessibleLabel": "Row 500" }],
                    "findAll": true,
                    "scrollIntoView": scroll_into_view,
                }),
            ))
            .expect("query_element_descendants failed");
            let ToolResult::Json(value) = result else { panic!("expected a json result") };
            value["elementHandles"].as_array().cloned().unwrap_or_default()
        };

        assert!(query(false).is_empty());
        let found = query(true);
        assert_eq!(found.len(), 1, "paging didn't create the row");
        assert_eq!(query(false), found, "the row isn't on screen after scrolling into view");
    }

    #[test]
    fn test_scroll_element_into_view() {
        use slint::ComponentHandle;

        crate::init_no_event_loop();
        slint::slint! {
            #[slint_edition = "1.19"]
            export component App inherits Window {
                width: 100px;
                height: 100px;
                in-out property <length> content-y <=> flick.content-y;
                flick := Flickable {
                    content-height: 2000px;
                    deep := Text { y: 1000px; height: 20px; text: "Deep"; }
                }
            }
        }

        let app = App::new().unwrap();
        let adapter = i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter();
        let state = make_state();
        state.add_window(&adapter);
        app.set_content_y(-1000.);
        let deep = crate::ElementHandle::find_by_element_id(&app, "App::deep").next().unwrap();
        let deep = index_to_handle(state.element_to_handle(deep));
        app.set_content_y(0.);

        block_on(handle_tool_call(
            &state,
            "scroll_element_into_view",
            &serde_json::json!({ "elementHandle": serde_json::to_value(deep).unwrap() }),
        ))
        .expect("scroll_element_into_view failed");
        assert_eq!(app.get_content_y(), -920.);
    }

    #[test]
    fn test_get_element_tree_links_parents() {
        use slint::ComponentHandle;

        crate::init_no_event_loop();
        slint::slint! {
            #[slint_edition = "1.19"]
            export component App inherits Window {
                width: 100px;
                height: 100px;
                public function show-popup() { popup.show(); }
                outer := Rectangle {
                    background: red;
                    inner := Rectangle { background: blue; }
                    for _ in 2: repeated := Rectangle { background: green; }
                    popup := PopupWindow {
                        in-popup := Rectangle { background: yellow; }
                    }
                }
                clipper := Rectangle {
                    clip: true;
                    clipped := Rectangle {
                        x: 200px;
                        width: 10px;
                        height: 10px;
                        shown := Rectangle { x: -150px; width: 10px; height: 10px; }
                    }
                }
            }
        }

        for native_popup in [false, true] {
            let app = App::new().unwrap();
            let adapter =
                i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter();
            adapter
                .internal(i_slint_core::InternalToken)
                .and_then(|wa| {
                    (wa as &dyn core::any::Any)
                        .downcast_ref::<crate::testing_backend::TestingWindow>()
                })
                .expect("testing window")
                .use_native_popup(native_popup);
            app.invoke_show_popup();
            let state = make_state();
            state.add_window(&adapter);
            let window_handle = index_to_handle(state.window_handles()[0]);
            let root = index_to_handle(
                state.root_element_handle(handle_to_index(window_handle).unwrap()).unwrap(),
            );

            let result = block_on(handle_tool_call(
                &state,
                "get_element_tree",
                &serde_json::json!({ "elementHandle": serde_json::to_value(root).unwrap() }),
            ))
            .expect("get_element_tree failed");
            let ToolResult::Json(tree) = result else { panic!("expected a json result") };
            let elements = tree["elements"].as_array().expect("elements");

            let with_id = |id: &str| -> Vec<&Value> {
                elements
                    .iter()
                    .filter(|e| {
                        e["typeNamesAndIds"].as_array().unwrap().iter().any(|t| t["id"] == id)
                    })
                    .collect()
            };
            let one = |id: &str| {
                let found = with_id(id);
                assert_eq!(found.len(), 1, "expected one element with id {id}");
                found[0]
            };
            let parent_of = |element: &Value| -> Option<&Value> {
                let handle = element.get("parentHandle")?;
                Some(elements.iter().find(|e| e["handle"] == *handle).expect("parent is listed"))
            };

            let root_node = &elements[0];
            assert!(parent_of(root_node).is_none());
            assert_eq!(root_node["depth"], 0);
            for element in &elements[1..] {
                let parent = parent_of(element).unwrap_or_else(|| {
                    panic!("no parentHandle with native_popup {native_popup}: {element}")
                });
                assert_eq!(
                    element["depth"].as_u64().unwrap(),
                    parent["depth"].as_u64().unwrap() + 1
                );
            }

            let outer = one("App::outer");
            assert_eq!(parent_of(outer), Some(root_node));
            assert_eq!(parent_of(one("App::inner")), Some(outer));
            let repeated = with_id("App::repeated");
            assert_eq!(repeated.len(), 2);
            for r in repeated {
                assert_eq!(parent_of(r), Some(outer));
            }
            assert!(with_id("App::popup").is_empty());
            assert_eq!(parent_of(one("App::in-popup")), Some(outer), "native_popup {native_popup}");
            assert!(with_id("App::clipped").is_empty());
            let clipper = one("App::clipper");
            assert!(
                std::iter::successors(Some(one("App::shown")), |e| parent_of(e))
                    .any(|e| e == clipper)
            );
        }
    }

    #[test]
    fn test_get_element_snapshot() {
        use slint::ComponentHandle;

        crate::init_no_event_loop();
        slint::slint! {
            #[slint_edition = "1.19"]
            component Toggle inherits Rectangle {
                @testable in-out property <bool> on;
                @testable in property <string> caption;
                Text { width: 40px; height: 10px; text: root.caption; }
                TouchArea { clicked => { root.on = !root.on; } }
            }
            export component App inherits Window {
                width: 200px;
                height: 200px;
                public function show-popup() { popup.show(); }
                ok := Rectangle {
                    x: 10px;
                    y: 10px;
                    width: 50px;
                    height: 20px;
                    accessible-role: button;
                    accessible-label: "OK";
                    accessible-enabled: false;
                    TouchArea { }
                }
                run := TouchArea {
                    x: 70px;
                    y: 10px;
                    width: 20px;
                    height: 20px;
                    accessible-role: button;
                    accessible-label: "Run";
                    enabled: false;
                    Text { text: "Run"; }
                }
                Toggle {
                    x: 10px;
                    y: 40px;
                    width: 50px;
                    height: 20px;
                    caption: "a \"quoted\" caption";
                    popup := PopupWindow {
                        x: 0px;
                        y: 20px;
                        Text { x: 0; y: 0; width: 30px; height: 10px; text: "In popup"; }
                    }
                }
                Rectangle {
                    x: 10px;
                    y: 70px;
                    width: 20px;
                    height: 20px;
                    accessible-role: checkbox;
                    accessible-checkable: true;
                    accessible-checked: true;
                    accessible-label: "Check";
                }
                Text {
                    x: 10px;
                    y: 100px;
                    width: 180px;
                    height: 10px;
                    text: "This text is longer than eighty characters, so the snapshot cuts it off after them.";
                }
                Flickable {
                    x: 10px;
                    y: 120px;
                    width: 50px;
                    height: 50px;
                    content-height: 100px;
                }
                Rectangle {
                    x: 100px;
                    y: 10px;
                    width: 20px;
                    height: 20px;
                    clip: true;
                    Rectangle {
                        x: 50px;
                        accessible-role: button;
                        accessible-label: "Clipped";
                    }
                }
            }
        }

        let app = App::new().unwrap();
        let adapter = i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter();
        let state = make_state();
        state.add_window(&adapter);
        let root = index_to_handle(state.root_element_handle(state.window_handles()[0]).unwrap());
        let snapshot = |rectangles: bool| {
            let result = block_on(handle_tool_call(
                &state,
                "get_element_snapshot",
                &serde_json::json!({
                    "elementHandle": serde_json::to_value(root).unwrap(),
                    "rectangles": rectangles,
                }),
            ))
            .expect("get_element_snapshot failed");
            let ToolResult::Text(snapshot) = result else { panic!("expected a text result") };
            snapshot
        };

        assert_eq!(
            snapshot(false),
            r#"# 8 of 14 elements on screen. [i:g] is the element handle {"index": "i", "generation": "g"}.
[2:1] Rectangle role=Button id=App::ok "OK" disabled
[3:1] TouchArea role=Button id=App::run "Run" disabled
[4:1] Toggle @caption="a \"quoted\" caption" @on=false
  [5:1] Text "a \"quoted\" caption"
  [6:1] TouchArea
[7:1] Rectangle role=Checkbox "Check" checked
[8:1] Text "This text is longer than eighty characters, so the snapshot cuts it off after th…"
[9:1] Flickable
"#
        );
        assert_eq!(
            snapshot(true),
            r#"# 8 of 14 elements on screen. [i:g] is the element handle {"index": "i", "generation": "g"}.
[2:1] Rectangle role=Button id=App::ok "OK" disabled (10,10 50x20)
[3:1] TouchArea role=Button id=App::run "Run" disabled (70,10 20x20)
[4:1] Toggle @caption="a \"quoted\" caption" @on=false (10,40 50x20)
  [5:1] Text "a \"quoted\" caption" (15,45 40x10)
  [6:1] TouchArea (10,40 50x20)
[7:1] Rectangle role=Checkbox "Check" checked (10,70 20x20)
[8:1] Text "This text is longer than eighty characters, so the snapshot cuts it off after th…" (10,100 180x10)
[9:1] Flickable (10,120 50x50)
"#
        );

        // The walk visits a popup's contents before the element it's attached to.
        app.invoke_show_popup();
        let snapshot = snapshot(true);
        let lines: Vec<&str> = snapshot.lines().collect();
        let toggle = lines.iter().position(|line| line.contains(" Toggle ")).unwrap();
        assert!(lines[toggle + 1].starts_with("  [") && lines[toggle + 1].contains("\"In popup\""));
        assert!(lines[toggle + 1].ends_with(" (10,60 30x10)"), "{snapshot}");
    }

    #[test]
    fn test_snapshot_argument_and_truncation() {
        use slint::ComponentHandle;

        crate::init_no_event_loop();
        slint::slint! {
            #[slint_edition = "1.19"]
            export component App inherits Window {
                width: 100px;
                height: 100px;
                @testable out property <bool> hovered: area.has-hover;
                for i in 3: Text { y: i * 10px; height: 10px; text: "Row " + i; }
                area := TouchArea { }
            }
        }

        let app = App::new().unwrap();
        let adapter = i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter();
        let state = make_state();
        state.add_window(&adapter);
        let root = serde_json::to_value(index_to_handle(
            state.root_element_handle(state.window_handles()[0]).unwrap(),
        ))
        .unwrap();
        let call = |name: &str, arguments: Value| {
            let request = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": { "name": name, "arguments": arguments }
            });
            block_on(handle_mcp_request(&state, &request.to_string())).unwrap()["result"].clone()
        };

        let result = call("hover_element", serde_json::json!({ "elementHandle": root }));
        assert_eq!(result["content"].as_array().unwrap().len(), 1);

        let result =
            call("hover_element", serde_json::json!({ "elementHandle": root, "snapshot": true }));
        let content = result["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        let snapshot = content[1]["text"].as_str().unwrap();
        assert!(snapshot.starts_with("# Window handle ["), "{snapshot}");
        assert!(snapshot.contains("@hovered=true"), "{snapshot}");

        let result = call(
            "get_element_properties",
            serde_json::json!({ "elementHandle": root, "snapshot": true }),
        );
        assert_eq!(result["isError"], true);

        let result = call(
            "get_element_snapshot",
            serde_json::json!({ "elementHandle": root, "maxElements": 2 }),
        );
        let snapshot = result["content"][0]["text"].as_str().unwrap();
        assert!(snapshot.starts_with("# The first 2 listed elements;"), "{snapshot}");
        assert_eq!(snapshot.lines().count(), 3, "{snapshot}");

        let result = call("get_element_snapshot", serde_json::json!({ "maxElements": 2 }));
        let snapshot = result["content"][0]["text"].as_str().unwrap();
        let lines: Vec<&str> = snapshot.lines().collect();
        assert!(lines[0].starts_with("# Window handle ["), "{snapshot}");
        assert!(lines[1].starts_with("# The first 2 listed elements;"), "{snapshot}");
        assert_eq!(lines.len(), 4, "{snapshot}");
    }

    #[test]
    fn test_snapshot_argument_schema() {
        let defs = tool_definitions();
        let properties = |name: &str| {
            defs["tools"].as_array().unwrap().iter().find(|t| t["name"] == name).unwrap()
                ["inputSchema"]["properties"]
                .clone()
        };
        assert_eq!(properties("click_element")[SNAPSHOT_ARGUMENT]["type"], "boolean");
        assert_eq!(properties("query_element_descendants")[SNAPSHOT_ARGUMENT]["type"], "boolean");
        assert!(properties("get_element_tree").get(SNAPSHOT_ARGUMENT).is_none());
        assert_eq!(properties("get_element_snapshot")[RECTANGLES_ARGUMENT]["type"], "boolean");
        assert!(properties("get_element_tree").get(RECTANGLES_ARGUMENT).is_none());
    }

    #[test]
    fn test_element_tree_response_reports_missing_debug_info() {
        let response = element_tree_response(vec![], false, false);
        assert_eq!(response["debugInfoMissing"], true);
        assert!(response["note"].as_str().unwrap().contains("SLINT_EMIT_DEBUG_INFO"));
        assert_eq!(response["totalCount"], 0);
    }

    #[test]
    fn test_element_tree_response_omits_the_note_with_debug_info() {
        let response = element_tree_response(vec![serde_json::json!({})], true, true);
        assert!(response.get("debugInfoMissing").is_none());
        assert!(response.get("note").is_none());
        assert_eq!(response["totalCount"], 1);
        assert_eq!(response["truncated"], true);
    }

    #[test]
    fn test_get_element_properties_reports_testable_properties() {
        use slint::ComponentHandle;

        crate::init_no_event_loop();
        slint::slint! {
            #[slint_edition = "1.19"]
            export component App inherits Window {
                width: 100px;
                height: 100px;
                @testable in-out property <bool> checked: true;
                @testable in-out property <float> amount: 12.5;
                @testable in-out property <string> label: "hi";
                in-out property <int> unmarked: 1;
            }
        }

        let app = App::new().unwrap();
        let adapter = i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter();
        let state = make_state();
        state.add_window(&adapter);
        let window_handle = index_to_handle(state.window_handles()[0]);
        let root = index_to_handle(
            state.root_element_handle(handle_to_index(window_handle).unwrap()).unwrap(),
        );

        let result = block_on(handle_tool_call(
            &state,
            "get_element_properties",
            &serde_json::json!({
                "elementHandle": serde_json::to_value(root).unwrap()
            }),
        ))
        .expect("get_element_properties failed");
        let ToolResult::Json(value) = result else { panic!("expected a json result") };

        // The protobuf-facing availability flag is dropped from the JSON encoding.
        assert!(value.get("testablePropertiesAvailable").is_none());
        assert!(value.get("note").is_none());

        let props = value["testableProperties"].as_array().expect("testableProperties");
        let by_name = |name: &str| {
            props
                .iter()
                .find(|p| p["name"] == name)
                .unwrap_or_else(|| panic!("property {name} not listed"))
        };
        // Values arrive typed: booleans and numbers as JSON values, not strings.
        assert_eq!(by_name("checked")["typeName"], "bool");
        assert_eq!(by_name("checked")["value"], serde_json::json!(true));
        assert_eq!(by_name("amount")["typeName"], "float");
        assert_eq!(by_name("amount")["value"], serde_json::json!(12.5));
        assert_eq!(by_name("label")["typeName"], "string");
        assert_eq!(by_name("label")["value"], serde_json::json!("hi"));
        // Only `@testable` declarations are listed.
        assert!(!props.iter().any(|p| p["name"] == "unmarked"));

        // The same conversion applies to elements in the tree response.
        let result = block_on(handle_tool_call(
            &state,
            "get_element_tree",
            &serde_json::json!({
                "elementHandle": serde_json::to_value(root).unwrap()
            }),
        ))
        .expect("get_element_tree failed");
        let ToolResult::Json(tree) = result else { panic!("expected a json result") };
        let root_node = &tree["elements"][0];
        assert!(root_node.get("testablePropertiesAvailable").is_none());
        let props = root_node["testableProperties"].as_array().expect("testableProperties");
        assert!(props.iter().any(|p| p["name"] == "checked" && p["value"] == true));
    }

    #[test]
    fn test_convert_testable_property_values_typing() {
        let mut node = serde_json::json!({
            "testablePropertiesAvailable": true,
            "testableProperties": [
                { "name": "b", "typeName": "bool", "value": "false" },
                { "name": "n", "typeName": "int", "value": "42" },
                { "name": "d", "typeName": "duration", "value": "250" },
                { "name": "l", "typeName": "length", "value": "12.5" },
                { "name": "inf", "typeName": "float", "value": "inf" },
                { "name": "e", "typeName": "Mood", "value": "grumpy" },
                { "name": "s", "typeName": "{ a: int,}" },
            ]
        });
        convert_testable_property_values(&mut node);
        assert!(node.get("testablePropertiesAvailable").is_none());
        let props = node["testableProperties"].as_array().unwrap();
        assert_eq!(props[0]["value"], serde_json::json!(false));
        assert_eq!(props[1]["value"], serde_json::json!(42));
        assert_eq!(props[2]["value"], serde_json::json!(250));
        assert_eq!(props[3]["value"], serde_json::json!(12.5));
        // Non-finite numbers and enum values stay strings; a value-less entry stays value-less.
        assert_eq!(props[4]["value"], serde_json::json!("inf"));
        assert_eq!(props[5]["value"], serde_json::json!("grumpy"));
        assert!(props[6].get("value").is_none());
    }

    #[test]
    fn test_stringify_testable_property_matches() {
        let mut query_stack = serde_json::json!([
            { "matchDescendants": true },
            { "matchElementTestableProperty": { "name": "b", "value": true } },
            { "matchElementTestableProperty": { "name": "n", "value": 42 } },
            { "matchElementTestableProperty": { "name": "f", "value": 12.5 } },
            { "matchElementTestableProperty": { "name": "w", "value": 3.0 } },
            { "matchElementTestableProperty": { "name": "s", "value": "text", "mode": "ContainsIgnoringCase" } },
            { "match_element_testable_property": { "name": "b", "value": false } },
        ]);
        stringify_testable_property_matches(&mut query_stack);
        let value = |index: usize| &query_stack[index]["matchElementTestableProperty"]["value"];
        assert_eq!(query_stack[0], serde_json::json!({ "matchDescendants": true }));
        assert_eq!(value(1), "true");
        assert_eq!(value(2), "42");
        assert_eq!(value(3), "12.5");
        assert_eq!(value(4), "3");
        assert_eq!(value(5), "text");
        assert_eq!(query_stack[6]["match_element_testable_property"]["value"], "false");
    }

    #[test]
    fn test_query_element_descendants_by_property_values() {
        use slint::ComponentHandle;

        crate::init_no_event_loop();
        slint::slint! {
            #[slint_edition = "1.19"]
            component Item {
                @testable in property <bool> selected;
                @testable in property <float> amount;
            }
            export component App inherits Window {
                VerticalLayout {
                    Item { selected: true; amount: 2.5; }
                    Item { }
                    TextInput { accessible-placeholder-text: "Search files"; }
                }
            }
        }

        let app = App::new().unwrap();
        let adapter = i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter();
        let state = make_state();
        state.add_window(&adapter);
        let root = index_to_handle(
            state.root_element_handle(state.window_handles()[0]).expect("root element handle"),
        );

        let query = |instruction: Value| {
            let result = block_on(handle_tool_call(
                &state,
                "query_element_descendants",
                &serde_json::json!({
                    "elementHandle": serde_json::to_value(root).unwrap(),
                    "queryStack": [instruction],
                    "findAll": true,
                }),
            ))
            .expect("query_element_descendants failed");
            let ToolResult::Json(value) = result else { panic!("expected a json result") };
            value["elementHandles"].as_array().map_or(0, Vec::len)
        };

        let testable = |name: &str, value: Value| {
            query(serde_json::json!({
                "matchElementTestableProperty": { "name": name, "value": value }
            }))
        };
        assert_eq!(testable("selected", serde_json::json!(true)), 1);
        assert_eq!(testable("selected", serde_json::json!("false")), 1);
        assert_eq!(testable("amount", serde_json::json!(2.5)), 1);
        assert_eq!(testable("amount", serde_json::json!(0)), 1);
        assert_eq!(
            query(serde_json::json!({
                "match_element_testable_property": { "name": "selected", "value": true }
            })),
            1
        );

        assert_eq!(
            query(serde_json::json!({
                "matchElementAccessibleProperty": {
                    "property": "PlaceholderText",
                    "value": "FILES",
                    "mode": "ContainsIgnoringCase"
                }
            })),
            1
        );
    }

    #[test]
    fn test_pointer_tools_hold_modifiers() {
        use slint::ComponentHandle;

        crate::init_no_event_loop();
        slint::slint! {
            #[slint_edition = "1.19"]
            export component App inherits Window {
                width: 100px;
                height: 100px;
                out property <bool> shift-on-move;
                out property <bool> control-on-scroll;
                TouchArea {
                    width: 100%;
                    height: 100%;
                    pointer-event(event) => {
                        if (event.kind == PointerEventKind.move) {
                            root.shift-on-move = event.modifiers.shift;
                        }
                    }
                    scroll-event(event) => {
                        root.control-on-scroll = event.modifiers.control;
                        accept
                    }
                }
            }
        }

        let app = App::new().unwrap();
        let adapter = i_slint_core::window::WindowInner::from_pub(app.window()).window_adapter();
        let state = make_state();
        state.add_window(&adapter);
        let window = serde_json::to_value(index_to_handle(state.window_handles()[0])).unwrap();
        let root = serde_json::to_value(index_to_handle(
            state.root_element_handle(state.window_handles()[0]).unwrap(),
        ))
        .unwrap();

        block_on(handle_tool_call(
            &state,
            "hover_element",
            &serde_json::json!({ "elementHandle": root, "modifiers": { "shift": true } }),
        ))
        .expect("hover_element failed");
        assert!(app.get_shift_on_move(), "the pointer event didn't carry Shift");

        block_on(handle_tool_call(
            &state,
            "scroll_element",
            &serde_json::json!({
                "elementHandle": root,
                "deltaY": -10.0,
                "modifiers": { "control": true }
            }),
        ))
        .expect("scroll_element failed");
        assert!(app.get_control_on_scroll(), "the wheel event didn't carry Control");

        // The keys are released with the gesture, so the next one is unmodified.
        block_on(handle_tool_call(
            &state,
            "hover_element",
            &serde_json::json!({ "elementHandle": root }),
        ))
        .expect("hover_element failed");
        assert!(!app.get_shift_on_move(), "Shift stayed down after the gesture");

        block_on(handle_tool_call(
            &state,
            "move_pointer",
            &serde_json::json!({
                "windowHandle": window,
                "position": { "x": 10.0, "y": 10.0 },
                "modifiers": { "shift": true }
            }),
        ))
        .expect("move_pointer failed");
        assert!(app.get_shift_on_move(), "move_pointer's own pointer event didn't carry Shift");

        block_on(handle_tool_call(
            &state,
            "move_pointer",
            &serde_json::json!({ "windowHandle": window, "position": { "x": 20.0, "y": 20.0 } }),
        ))
        .expect("move_pointer failed");
        assert!(!app.get_shift_on_move(), "Shift stayed down after move_pointer's gesture");
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
    fn test_handle_field_schemas_are_distinct() {
        // The window and element handle schemas would otherwise be byte-identical
        // {index, generation} objects. Verify the disambiguating descriptions are
        // present and differ, so clients can tell the two kinds apart.
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().unwrap();
        let find = |name: &str| tools.iter().find(|t| t["name"] == name).unwrap().clone();

        let window_tool = find("get_window_properties");
        let window_desc = window_tool["inputSchema"]["properties"]["windowHandle"]["description"]
            .as_str()
            .expect("windowHandle should have a description");
        let element_tool = find("get_element_properties");
        let element_desc =
            element_tool["inputSchema"]["properties"]["elementHandle"]["description"]
                .as_str()
                .expect("elementHandle should have a description");

        assert!(window_desc.contains("window handle"));
        assert!(element_desc.contains("element handle"));
        assert_ne!(window_desc, element_desc);
    }

    #[test]
    fn test_tool_definitions_structure() {
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().unwrap();
        assert_eq!(tools.len(), TOOLS.len());
        for tool in tools {
            assert!(tool.get("name").and_then(|v| v.as_str()).is_some());
            assert!(tool.get("description").and_then(|v| v.as_str()).is_some());
            assert_eq!(tool["inputSchema"]["type"], "object");
        }
    }

    #[test]
    fn test_query_schema_types_match_values() {
        let defs = tool_definitions();
        let query = defs["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "query_element_descendants")
            .unwrap();
        let instruction = &query["inputSchema"]["properties"]["queryStack"]["items"]["properties"];
        assert_eq!(
            instruction["matchElementTestableProperty"]["properties"]["value"]["type"],
            serde_json::json!(["string", "number", "boolean"])
        );
        assert_eq!(
            instruction["matchElementAccessibleProperty"]["properties"]["value"]["type"],
            "string"
        );
    }

    #[test]
    fn test_query_stack_field_descriptions_apply() {
        let defs = tool_definitions();
        let query = defs["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "query_element_descendants")
            .unwrap();
        let query_stack = &query["inputSchema"]["properties"]["queryStack"];
        for (pointer, description) in QUERY_STACK_FIELD_DESCRIPTIONS {
            assert_eq!(
                query_stack.pointer(&format!("{pointer}/description")),
                Some(&Value::from(*description)),
                "no field at {pointer:?}"
            );
        }
    }

    #[test]
    fn test_all_tools_have_proto_schemas() {
        for def in TOOLS {
            assert!(
                mcp_schemas::proto_input_schema(def.request_type).is_some(),
                "tool {:?} references unknown proto message {:?}",
                def.name,
                def.request_type,
            );
            assert!(
                mcp_schemas::proto_field_names(def.request_type).is_some(),
                "tool {:?} has no field names for {:?}",
                def.name,
                def.request_type,
            );
            // Verify optional_fields are actual fields of the message
            let field_names = mcp_schemas::proto_field_names(def.request_type).unwrap();
            for opt in def.optional_fields {
                assert!(
                    field_names.contains(opt),
                    "tool {:?} lists optional field {:?} not in proto message {:?} (fields: {:?})",
                    def.name,
                    opt,
                    def.request_type,
                    field_names,
                );
            }
        }
    }

    #[test]
    fn test_proto_serde_field_names_match_tool_schemas() {
        // Verify that pbjson field names match what tool_definitions documents.
        // If a proto field is renamed, this test catches the mismatch.
        let req = proto::RequestWindowProperties {
            window_handle: Some(proto::Handle { index: 1, generation: 2 }),
        };
        let json = serde_json::to_value(req).unwrap();
        assert!(json.get("windowHandle").is_some(), "expected camelCase 'windowHandle'");
        assert_eq!(json["windowHandle"]["index"], "1");

        let req = proto::RequestFindElementsById {
            window_handle: Some(proto::Handle { index: 0, generation: 0 }),
            elements_id: "test".into(),
        };
        let json = serde_json::to_value(req).unwrap();
        assert!(json.get("elementsId").is_some(), "expected camelCase 'elementsId'");

        let req = proto::RequestElementProperties {
            element_handle: Some(proto::Handle { index: 0, generation: 0 }),
        };
        let json = serde_json::to_value(req).unwrap();
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
