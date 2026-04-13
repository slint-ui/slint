// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    #[cfg(any(feature = "system-testing", feature = "mcp"))]
    {
        use prost::Message;
        let manifest_dir =
            std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let proto_file = manifest_dir.join("slint_systest.proto");
        let fds = protox::compile([&proto_file], [&manifest_dir])
            .expect("failed to compile slint_systest.proto");
        let descriptor_bytes = fds.encode_to_vec();

        #[cfg(feature = "mcp")]
        generate_mcp_schemas(&fds, &out_dir);

        prost_build::Config::new()
            .compile_fds(fds)
            .expect("failed to generate Rust code from proto descriptors");

        pbjson_build::Builder::new()
            .register_descriptors(&descriptor_bytes)
            .expect("failed to register proto descriptors for pbjson")
            .out_dir(&out_dir)
            .build(&[".proto"])
            .expect("failed to generate serde impls from proto descriptors");
    }
}

/// Generate JSON schemas for proto Request* messages, used by the MCP server
/// to build tool definitions from the proto source of truth.
#[cfg(any(feature = "system-testing", feature = "mcp"))]
fn generate_mcp_schemas(
    fds: &prost_types::FileDescriptorSet,
    out_dir: &std::path::Path,
) {
    use std::fmt::Write;

    let file_desc = fds
        .file
        .iter()
        .find(|f| f.name.as_deref() == Some("slint_systest.proto"))
        .expect("slint_systest.proto descriptor not found");

    // Collect enum definitions: name -> list of variant names
    let mut enums = std::collections::HashMap::new();
    for e in &file_desc.enum_type {
        let name = e.name.as_deref().unwrap_or_default();
        let variants: Vec<String> =
            e.value.iter().filter_map(|v| v.name.clone()).collect();
        enums.insert(name.to_string(), variants);
    }

    // Collect message definitions for nested lookups
    let messages: std::collections::HashMap<String, &prost_types::DescriptorProto> = file_desc
        .message_type
        .iter()
        .filter_map(|m: &prost_types::DescriptorProto| m.name.as_ref().map(|n| (n.clone(), m)))
        .collect();

    // Generate JSON schema strings for each Request* message
    let mut code = String::new();
    writeln!(
        &mut code,
        "/// Auto-generated from slint_systest.proto — do not edit."
    )
    .unwrap();
    writeln!(
        &mut code,
        "pub fn proto_input_schema(message_name: &str) -> Option<serde_json::Value> {{"
    )
    .unwrap();
    writeln!(&mut code, "    match message_name {{").unwrap();

    for msg in &file_desc.message_type {
        let msg_name = msg.name.as_deref().unwrap_or_default();
        if !msg_name.starts_with("Request") {
            continue;
        }

        let schema = message_to_json_schema(msg, &messages, &enums);
        writeln!(
            &mut code,
            "        {msg_name:?} => Some(serde_json::json!({schema})),"
        )
        .unwrap();
    }

    writeln!(&mut code, "        _ => None,").unwrap();
    writeln!(&mut code, "    }}").unwrap();
    writeln!(&mut code, "}}").unwrap();

    // Also generate a list of field names (camelCase) for each message
    writeln!(&mut code).unwrap();
    writeln!(
        &mut code,
        "/// Returns the camelCase field names of a proto request message."
    )
    .unwrap();
    writeln!(
        &mut code,
        "pub fn proto_field_names(message_name: &str) -> Option<&'static [&'static str]> {{"
    )
    .unwrap();
    writeln!(&mut code, "    match message_name {{").unwrap();

    for msg in &file_desc.message_type {
        let msg_name = msg.name.as_deref().unwrap_or_default();
        if !msg_name.starts_with("Request") {
            continue;
        }
        let fields: Vec<String> = msg
            .field
            .iter()
            .filter_map(|f| f.name.as_ref().map(|n| snake_to_camel(n)))
            .collect();
        let fields_str: Vec<String> = fields.iter().map(|f| format!("{f:?}")).collect();
        writeln!(
            &mut code,
            "        {msg_name:?} => Some(&[{}]),",
            fields_str.join(", ")
        )
        .unwrap();
    }

    writeln!(&mut code, "        _ => None,").unwrap();
    writeln!(&mut code, "    }}").unwrap();
    writeln!(&mut code, "}}").unwrap();

    std::fs::write(out_dir.join("mcp_schemas.rs"), code)
        .expect("failed to write mcp_schemas.rs");
}

#[cfg(any(feature = "system-testing", feature = "mcp"))]
fn message_to_json_schema(
    msg: &prost_types::DescriptorProto,
    messages: &std::collections::HashMap<String, &prost_types::DescriptorProto>,
    enums: &std::collections::HashMap<String, Vec<String>>,
) -> String {
    use std::fmt::Write;

    let mut props = String::new();
    let mut first = true;
    for field in &msg.field {
        let name = field.name.as_deref().unwrap_or_default();
        let camel_name = snake_to_camel(name);
        let field_type = field.r#type();
        let is_repeated =
            field.label == Some(prost_types::field_descriptor_proto::Label::Repeated as i32);

        let mut type_schema = match field_type {
            prost_types::field_descriptor_proto::Type::Double
            | prost_types::field_descriptor_proto::Type::Float => {
                r#"{"type": "number"}"#.to_string()
            }
            prost_types::field_descriptor_proto::Type::Int32
            | prost_types::field_descriptor_proto::Type::Uint32
            | prost_types::field_descriptor_proto::Type::Sint32
            | prost_types::field_descriptor_proto::Type::Fixed32
            | prost_types::field_descriptor_proto::Type::Sfixed32 => {
                r#"{"type": "integer"}"#.to_string()
            }
            prost_types::field_descriptor_proto::Type::Int64
            | prost_types::field_descriptor_proto::Type::Uint64
            | prost_types::field_descriptor_proto::Type::Sint64
            | prost_types::field_descriptor_proto::Type::Fixed64
            | prost_types::field_descriptor_proto::Type::Sfixed64 => {
                // pbjson serializes 64-bit integers as strings
                r#"{"type": "string"}"#.to_string()
            }
            prost_types::field_descriptor_proto::Type::Bool => {
                r#"{"type": "boolean"}"#.to_string()
            }
            prost_types::field_descriptor_proto::Type::String => {
                r#"{"type": "string"}"#.to_string()
            }
            prost_types::field_descriptor_proto::Type::Bytes => {
                r#"{"type": "string"}"#.to_string()
            }
            prost_types::field_descriptor_proto::Type::Enum => {
                let type_name = field.type_name.as_deref().unwrap_or_default();
                let short_name = type_name.rsplit('.').next().unwrap_or(type_name);
                if let Some(variants) = enums.get(short_name) {
                    let vs: Vec<String> =
                        variants.iter().map(|v| format!("{v:?}")).collect();
                    format!(r#"{{"type": "string", "enum": [{}]}}"#, vs.join(", "))
                } else {
                    r#"{"type": "string"}"#.to_string()
                }
            }
            prost_types::field_descriptor_proto::Type::Message => {
                let type_name = field.type_name.as_deref().unwrap_or_default();
                let short_name = type_name.rsplit('.').next().unwrap_or(type_name);
                if let Some(nested_msg) = messages.get(short_name) {
                    message_to_json_schema(nested_msg, messages, enums)
                } else {
                    r#"{"type": "object"}"#.to_string()
                }
            }
            _ => r#"{"type": "string"}"#.to_string(),
        };

        if is_repeated {
            type_schema = format!(r#"{{"type": "array", "items": {type_schema}}}"#);
        }

        if !first {
            write!(&mut props, ", ").unwrap();
        }
        first = false;
        write!(&mut props, "{camel_name:?}: {type_schema}").unwrap();
    }

    // Required fields are handled at runtime by the tool metadata table.
    format!(r#"{{"type": "object", "properties": {{{props}}}}}"#)
}

#[cfg(any(feature = "system-testing", feature = "mcp"))]
fn snake_to_camel(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}
