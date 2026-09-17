// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Implements the `@from-json(...)` compile-time macro: reads a JSON file at compile time
//! and converts a value from it into an [`Expression`] matching the target property's type.

use crate::diagnostics::Spanned;
use crate::expression_tree::{Expression, Unit};
use crate::langtype::Type;
use crate::lookup::LookupCtx;
use crate::parser::{SyntaxKind, SyntaxToken, syntax_nodes};
use smol_str::SmolStr;
use std::path::PathBuf;

pub fn lower(node: &syntax_nodes::AtFromJson, ctx: &mut LookupCtx) -> Expression {
    let string_tokens: Vec<SyntaxToken> = node
        .children_with_tokens()
        .filter_map(|n| n.into_token())
        .filter(|t| t.kind() == SyntaxKind::StringLiteral)
        .collect();

    let Some(path_token) = string_tokens.first() else {
        debug_assert!(ctx.diag.has_errors());
        return Expression::Invalid;
    };
    let Some(path_str) =
        crate::literals::unescape_string_reporting(Some(path_token), ctx.diag, node)
    else {
        return Expression::Invalid;
    };

    let pointer_token = string_tokens.get(1);
    let pointer_str = match pointer_token {
        Some(tok) => match crate::literals::unescape_string_reporting(Some(tok), ctx.diag, node) {
            Some(s) => Some(s),
            None => return Expression::Invalid,
        },
        None => None,
    };
    // Where to point path/key-not-found diagnostics: the JSON-path argument if there is
    // one, otherwise the whole macro call (the 1-arg form has no second token to point at).
    let path_error_source: &dyn Spanned = match pointer_token {
        Some(t) => t,
        None => node,
    };

    let target_ty = target_type(ctx);
    if matches!(
        target_ty,
        Type::Invalid | Type::Void | Type::InferredProperty | Type::InferredCallback
    ) {
        ctx.diag.push_error(
            "Cannot use @from-json() here: the type of the value is not known".into(),
            node,
        );
        return Expression::Invalid;
    }

    let resolved = resolve_path(&path_str, node, ctx);

    let Some(file) = crate::fileaccess::load_file(&resolved) else {
        ctx.diag.push_error(format!("Cannot find JSON file {}", resolved.display()), path_token);
        return Expression::Invalid;
    };
    ctx.diag.all_loaded_files.insert(file.canon_path.clone());

    let bytes = match file.try_read() {
        Ok(bytes) => bytes,
        Err(err) => {
            ctx.diag.push_error(
                format!("Cannot read JSON file {}: {err}", resolved.display()),
                path_token,
            );
            return Expression::Invalid;
        }
    };
    // Strip a UTF-8 BOM if present; some editors add one and serde_json rejects it.
    let bytes = bytes.strip_prefix(&[0xEFu8, 0xBB, 0xBF]).unwrap_or(&bytes);

    let json: serde_json::Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(err) => {
            ctx.diag.push_error(
                format!("Cannot parse JSON file {}: {err}", resolved.display()),
                path_token,
            );
            return Expression::Invalid;
        }
    };

    let segments: Vec<SmolStr> = if let Some(pointer_str) = &pointer_str {
        let mut segments = Vec::new();
        for segment in pointer_str.split('/') {
            if segment.is_empty() {
                ctx.diag.push_error(
                    format!("Empty segment in the JSON path '{pointer_str}'"),
                    path_error_source,
                );
                return Expression::Invalid;
            }
            segments.push(SmolStr::from(segment));
        }
        segments
    } else {
        let Some(name) = ctx.property_name else {
            ctx.diag.push_error(
                "@from-json() without a second argument can only be used in a property binding; \
                 pass the JSON path as the second argument"
                    .into(),
                node,
            );
            return Expression::Invalid;
        };
        vec![SmolStr::from(name)]
    };

    let mut current = &json;
    let mut consumed: Vec<SmolStr> = Vec::new();
    for segment in &segments {
        let Some(obj) = current.as_object() else {
            ctx.diag.push_error(
                format!(
                    "'{}' in the JSON file {} is not an object",
                    breadcrumb(&consumed),
                    resolved.display()
                ),
                path_error_source,
            );
            return Expression::Invalid;
        };
        let Some(value) = lookup_key(obj, segment) else {
            ctx.diag.push_error(
                format!(
                    "No key '{segment}' in '{}' of the JSON file {}",
                    breadcrumb(&consumed),
                    resolved.display()
                ),
                path_error_source,
            );
            return Expression::Invalid;
        };
        current = value;
        consumed.push(segment.clone());
    }

    json_to_expression(current, &target_ty, &mut consumed, &resolved, node, ctx)
}

/// The type this `@from-json(...)` call must produce: the contextual expected type, falling
/// back to the whole binding's return type when the former hasn't been set (e.g. when
/// resolving a struct field's default value, which never sets `expected_type`).
fn target_type(ctx: &LookupCtx) -> Type {
    match &ctx.expected_type {
        Type::Invalid | Type::Void | Type::InferredProperty | Type::InferredCallback => {
            ctx.return_type().clone()
        }
        ty => ty.clone(),
    }
}

/// Resolve the file path argument the same way `@image-url(...)` resolves its path: absolute
/// paths are used as-is, otherwise the path is looked up through the same include/library-path
/// machinery as `import` statements, falling back to a path relative to the current `.slint`
/// file's directory.
fn resolve_path(s: &str, node: &syntax_nodes::AtFromJson, ctx: &LookupCtx) -> PathBuf {
    let path = std::path::Path::new(s);
    if crate::pathutils::is_absolute(path) {
        return path.to_path_buf();
    }
    ctx.type_loader
        .and_then(|loader| loader.resolve_import_path(Some(&(**node).clone().into()), s))
        .map(|(p, _)| p)
        .unwrap_or_else(|| {
            crate::pathutils::join(&crate::pathutils::dirname(node.source_file.path()), path)
                .unwrap_or_else(|| path.to_path_buf())
        })
}

/// Look up `key` in `map`: try an exact match first, then fall back to a scan comparing
/// `normalize_identifier`-normalized (underscore/dash-insensitive) keys.
fn lookup_key<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a serde_json::Value> {
    if let Some(v) = map.get(key) {
        return Some(v);
    }
    let normalized_key = crate::parser::normalize_identifier(key);
    map.iter()
        .find(|(k, _)| crate::parser::normalize_identifier(k) == normalized_key)
        .map(|(_, v)| v)
}

fn breadcrumb(consumed: &[SmolStr]) -> String {
    if consumed.is_empty() { "the root".into() } else { consumed.join("/") }
}

fn json_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Report that `value` cannot be used for `target`, and return `Expression::Invalid`.
/// A plain function (not a closure) so it can be called from anywhere in `json_to_expression`
/// without holding a long-lived borrow of `breadcrumb_segments` across the whole match.
fn mismatch(
    value: &serde_json::Value,
    target: &Type,
    breadcrumb_segments: &[SmolStr],
    file: &std::path::Path,
    node: &syntax_nodes::AtFromJson,
    ctx: &mut LookupCtx,
) -> Expression {
    if matches!(value, serde_json::Value::Null) {
        ctx.diag.push_error(
            format!(
                "The JSON value at '{}' in {} is null, which cannot be used for a value of type {target}",
                breadcrumb(breadcrumb_segments),
                file.display()
            ),
            node,
        );
    } else {
        ctx.diag.push_error(
            format!(
                "Cannot use the JSON {} at '{}' in {} for a value of type {target}",
                json_kind(value),
                breadcrumb(breadcrumb_segments),
                file.display()
            ),
            node,
        );
    }
    Expression::Invalid
}

fn json_to_expression(
    value: &serde_json::Value,
    target: &Type,
    breadcrumb_segments: &mut Vec<SmolStr>,
    file: &std::path::Path,
    node: &syntax_nodes::AtFromJson,
    ctx: &mut LookupCtx,
) -> Expression {
    match target {
        Type::Bool => match value {
            serde_json::Value::Bool(b) => Expression::BoolLiteral(*b),
            _ => mismatch(value, target, breadcrumb_segments, file, node, ctx),
        },
        Type::String => match value {
            serde_json::Value::String(s) => Expression::StringLiteral(s.as_str().into()),
            _ => mismatch(value, target, breadcrumb_segments, file, node, ctx),
        },
        Type::Int32 => match value.as_f64() {
            Some(n) if n.fract() == 0.0 => Expression::Cast {
                from: Box::new(Expression::NumberLiteral(n, Unit::None)),
                to: Type::Int32,
            },
            Some(n) => {
                ctx.diag.push_error(
                    format!(
                        "The JSON number {n} at '{}' in {} is not an integer",
                        breadcrumb(breadcrumb_segments),
                        file.display()
                    ),
                    node,
                );
                Expression::Invalid
            }
            None => mismatch(value, target, breadcrumb_segments, file, node, ctx),
        },
        Type::Float32 => match value.as_f64() {
            Some(n) => Expression::NumberLiteral(n, Unit::None),
            None => mismatch(value, target, breadcrumb_segments, file, node, ctx),
        },
        Type::Percent => match value.as_f64() {
            Some(n) => Expression::NumberLiteral(n, Unit::Percent),
            None => mismatch(value, target, breadcrumb_segments, file, node, ctx),
        },
        Type::Duration | Type::PhysicalLength | Type::LogicalLength | Type::Rem | Type::Angle => {
            match value.as_f64() {
                Some(n) => Expression::NumberLiteral(n, target.default_unit().unwrap()),
                None => mismatch(value, target, breadcrumb_segments, file, node, ctx),
            }
        }
        Type::Color => match value {
            serde_json::Value::String(s) => match color_from_str(s) {
                Some(argb) => Expression::Cast {
                    from: Box::new(Expression::NumberLiteral(argb as f64, Unit::None)),
                    to: Type::Color,
                },
                None => {
                    ctx.diag.push_error(
                        format!(
                            "Cannot parse '{s}' as a color, at '{}' in {}",
                            breadcrumb(breadcrumb_segments),
                            file.display()
                        ),
                        node,
                    );
                    Expression::Invalid
                }
            },
            _ => mismatch(value, target, breadcrumb_segments, file, node, ctx),
        },
        Type::Enumeration(e) => match value {
            serde_json::Value::String(s) => {
                let s = s.strip_prefix(&format!("{}.", e.name)).unwrap_or(s.as_str());
                let normalized = crate::parser::normalize_identifier(s);
                match e.values.iter().position(|v| *v == normalized) {
                    Some(idx) => Expression::EnumerationValue(crate::langtype::EnumerationValue {
                        value: idx,
                        enumeration: e.clone(),
                    }),
                    None => {
                        ctx.diag.push_error(
                            format!(
                                "'{s}' is not a value of the enum '{}'. Expected one of: {}",
                                e.name,
                                e.values.join(", ")
                            ),
                            node,
                        );
                        Expression::Invalid
                    }
                }
            }
            _ => mismatch(value, target, breadcrumb_segments, file, node, ctx),
        },
        Type::Array(elem_ty) => match value.as_array() {
            Some(arr) => {
                let elem_ty = (**elem_ty).clone();
                let mut values = Vec::with_capacity(arr.len());
                for (i, v) in arr.iter().enumerate() {
                    breadcrumb_segments.push(SmolStr::from(format!("[{i}]")));
                    values.push(json_to_expression(
                        v,
                        &elem_ty,
                        breadcrumb_segments,
                        file,
                        node,
                        ctx,
                    ));
                    breadcrumb_segments.pop();
                }
                Expression::Array { element_ty: elem_ty, values }
            }
            None => mismatch(value, target, breadcrumb_segments, file, node, ctx),
        },
        Type::Struct(st) => match value.as_object() {
            Some(obj) => {
                let mut values = std::collections::BTreeMap::new();
                for (field_name, field_ty) in &st.fields {
                    let expr = match lookup_key(obj, field_name) {
                        Some(v) => {
                            breadcrumb_segments.push(field_name.clone());
                            let e = json_to_expression(
                                v,
                                field_ty,
                                breadcrumb_segments,
                                file,
                                node,
                                ctx,
                            );
                            breadcrumb_segments.pop();
                            e
                        }
                        None => {
                            ctx.diag.push_warning(
                                format!(
                                    "The JSON object at '{}' in {} has no key for the struct field '{field_name}'; using its default value",
                                    breadcrumb(breadcrumb_segments),
                                    file.display()
                                ),
                                node,
                            );
                            st.default_value_for_field(field_name)
                        }
                    };
                    values.insert(field_name.clone(), expr);
                }
                Expression::Struct { ty: st.clone(), values }
            }
            None => mismatch(value, target, breadcrumb_segments, file, node, ctx),
        },
        _ => mismatch(value, target, breadcrumb_segments, file, node, ctx),
    }
}

fn color_from_str(s: &str) -> Option<u32> {
    i_slint_common::color_parsing::parse_color_literal(s)
        .or_else(|| i_slint_common::color_parsing::named_colors().get(s).copied())
}
