// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Implements the `@from-json(...)` compile-time macro: reads a JSON file at compile time
//! and converts a value from it into an [`Expression`] matching the target property's type.

use crate::diagnostics::Spanned;
use crate::expression_tree::{Expression, ImageReference, Unit};
use crate::langtype::Type;
use crate::lookup::LookupCtx;
use crate::parser::{SyntaxKind, SyntaxNode, SyntaxToken, syntax_nodes};
use smol_str::SmolStr;
use std::path::PathBuf;

/// One `.member` or `[index]` applied to the value of a `@from-json(...)` call.
pub enum Step {
    Member(SyntaxToken),
    Index(syntax_nodes::Expression),
}

/// If `node` is a chain of `.member` and `[index]` on a `@from-json(...)` call,
/// returns the call and the steps in source order.
pub fn as_chain(node: &SyntaxNode) -> Option<(syntax_nodes::AtFromJson, Vec<Step>)> {
    let mut steps = Vec::new();
    let mut current = node.clone();
    loop {
        current = match current.kind() {
            SyntaxKind::AtFromJson => break,
            SyntaxKind::MemberAccess => {
                let member = syntax_nodes::MemberAccess::from(current);
                steps.push(Step::Member(member.child_token(SyntaxKind::Identifier)?));
                member.Expression().into()
            }
            SyntaxKind::IndexExpression => {
                let (array, index) = syntax_nodes::IndexExpression::from(current).Expression();
                steps.push(Step::Index(index));
                array.into()
            }
            SyntaxKind::Expression => current.first_child()?,
            _ => return None,
        };
    }
    steps.reverse();
    Some((current.into(), steps))
}

/// Lowers `node`, followed by `steps`, to a literal of the expected type.
/// `whole` is the complete expression, used to report a mismatching value.
pub fn lower(
    node: &syntax_nodes::AtFromJson,
    steps: &[Step],
    whole: &SyntaxNode,
    ctx: &mut LookupCtx,
) -> Expression {
    let Some(path_token) = node.child_token(SyntaxKind::StringLiteral) else {
        debug_assert!(ctx.diag.has_errors());
        return Expression::Invalid;
    };
    let Some(path_str) =
        crate::literals::unescape_string_reporting(Some(&path_token), ctx.diag, node)
    else {
        return Expression::Invalid;
    };

    let target_ty = target_type(ctx);
    if matches!(
        target_ty,
        Type::Invalid | Type::Void | Type::InferredProperty | Type::InferredCallback
    ) {
        ctx.diag.push_error(
            "Cannot use @from-json() here: the type of the value is not known".into(),
            whole,
        );
        return Expression::Invalid;
    }

    let resolved = resolve_path(&path_str, node, ctx);

    let Some(file) = crate::fileaccess::load_file(&resolved) else {
        ctx.diag.push_error(format!("Cannot find JSON file {}", resolved.display()), &path_token);
        return Expression::Invalid;
    };
    ctx.diag.all_loaded_files.insert(file.canon_path.clone());

    let bytes = match file.try_read() {
        Ok(bytes) => bytes,
        Err(err) => {
            ctx.diag.push_error(
                format!("Cannot read JSON file {}: {err}", resolved.display()),
                &path_token,
            );
            return Expression::Invalid;
        }
    };
    // serde_json rejects the UTF-8 BOM that some editors write.
    let bytes = bytes.strip_prefix(&[0xEFu8, 0xBB, 0xBF]).unwrap_or(&bytes);

    let json: serde_json::Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(err) => {
            ctx.diag.push_error(
                format!("Cannot parse JSON file {}: {err}", resolved.display()),
                &path_token,
            );
            return Expression::Invalid;
        }
    };

    let mut current = &json;
    let mut path = JsonPath::default();
    for step in steps {
        let next = match step {
            Step::Member(token) => member(current, token, &path, &resolved, ctx),
            Step::Index(index) => element(current, index, &path, &resolved, ctx),
        };
        let Some((segment, value)) = next else {
            return Expression::Invalid;
        };
        path.0.push(segment);
        current = value;
    }

    json_to_expression(current, &target_ty, &mut path, &resolved, whole, ctx)
}

/// Looks up the key named by `token` in `value`.
fn member<'a>(
    value: &'a serde_json::Value,
    token: &SyntaxToken,
    path: &JsonPath,
    file: &std::path::Path,
    ctx: &mut LookupCtx,
) -> Option<(Segment, &'a serde_json::Value)> {
    let name = token.text();
    let Some(object) = value.as_object() else {
        ctx.diag.push_error(path.not_a(file, "an object"), token);
        return None;
    };
    let Some((key, value)) = lookup_key(object, name) else {
        let message = if path.0.is_empty() {
            format!("No key '{name}' at the root of the JSON file {}", file.display())
        } else {
            format!("No key '{name}' in '{path}' of the JSON file {}", file.display())
        };
        ctx.diag.push_error(message, token);
        return None;
    };
    Some((Segment::Key(key.into()), value))
}

/// Looks up the element at the integer literal `index` in `value`.
fn element<'a>(
    value: &'a serde_json::Value,
    index: &syntax_nodes::Expression,
    path: &JsonPath,
    file: &std::path::Path,
    ctx: &mut LookupCtx,
) -> Option<(Segment, &'a serde_json::Value)> {
    let Some(i) = integer_literal(index) else {
        ctx.diag.push_error(
            "The index into a @from-json() value must be an integer literal, such as '[0]'".into(),
            index,
        );
        return None;
    };
    let Some(array) = value.as_array() else {
        ctx.diag.push_error(path.not_a(file, "an array"), index);
        return None;
    };
    let Some(element) = usize::try_from(i).ok().and_then(|i| array.get(i)) else {
        ctx.diag.push_error(
            format!(
                "Index {i} is out of range for the JSON array {}, which has {} elements",
                path.location(file),
                array.len()
            ),
            index,
        );
        return None;
    };
    Some((Segment::Index(i as usize), element))
}

/// The value of `expr` if it's an integer literal, optionally negated.
fn integer_literal(expr: &syntax_nodes::Expression) -> Option<i64> {
    match expr.first_child() {
        None => expr.child_token(SyntaxKind::NumberLiteral)?.text().parse().ok(),
        Some(node) if node.kind() == SyntaxKind::UnaryOpExpression => {
            let unary = syntax_nodes::UnaryOpExpression::from(node);
            unary.child_token(SyntaxKind::Minus)?;
            integer_literal(&unary.Expression()).map(|i| -i)
        }
        Some(_) => None,
    }
}

/// A position inside a JSON document, as the sequence of keys and indices that leads there.
#[derive(Default)]
struct JsonPath(Vec<Segment>);

enum Segment {
    Key(SmolStr),
    Index(usize),
}

impl std::fmt::Display for JsonPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, segment) in self.0.iter().enumerate() {
            match segment {
                Segment::Key(key) if i == 0 => write!(f, "{key}")?,
                Segment::Key(key) => write!(f, ".{key}")?,
                Segment::Index(index) => write!(f, "[{index}]")?,
            }
        }
        Ok(())
    }
}

impl JsonPath {
    /// "at 'a.b' in FILE", or "at the root of FILE".
    fn location(&self, file: &std::path::Path) -> String {
        if self.0.is_empty() {
            format!("at the root of {}", file.display())
        } else {
            format!("at '{self}' in {}", file.display())
        }
    }

    /// The error for a value that isn't `what` ("an object" or "an array").
    fn not_a(&self, file: &std::path::Path, what: &str) -> String {
        if self.0.is_empty() {
            format!("The root of the JSON file {} is not {what}", file.display())
        } else {
            format!("'{self}' in the JSON file {} is not {what}", file.display())
        }
    }
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

/// Look up `key` in `map`: try an exact match first, then fall back to comparing
/// `normalize_identifier`-normalized (underscore/dash-insensitive) keys.
fn lookup_key<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<(&'a str, &'a serde_json::Value)> {
    if let Some((k, v)) = map.get_key_value(key) {
        return Some((k, v));
    }
    let normalized_key = crate::parser::normalize_identifier(key);
    map.iter()
        .find(|(k, _)| crate::parser::normalize_identifier(k) == normalized_key)
        .map(|(k, v)| (k.as_str(), v))
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
fn mismatch(
    value: &serde_json::Value,
    target: &Type,
    path: &JsonPath,
    file: &std::path::Path,
    node: &SyntaxNode,
    ctx: &mut LookupCtx,
) -> Expression {
    if matches!(value, serde_json::Value::Null) {
        ctx.diag.push_error(
            format!(
                "The JSON value {} is null, which cannot be used for a value of type {target}",
                path.location(file)
            ),
            node,
        );
    } else {
        ctx.diag.push_error(
            format!(
                "Cannot use the JSON {} {} for a value of type {target}",
                json_kind(value),
                path.location(file)
            ),
            node,
        );
    }
    Expression::Invalid
}

fn json_to_expression(
    value: &serde_json::Value,
    target: &Type,
    path: &mut JsonPath,
    file: &std::path::Path,
    node: &SyntaxNode,
    ctx: &mut LookupCtx,
) -> Expression {
    match target {
        Type::Bool => match value {
            serde_json::Value::Bool(b) => Expression::BoolLiteral(*b),
            _ => mismatch(value, target, path, file, node, ctx),
        },
        Type::String => match value {
            serde_json::Value::String(s) => Expression::StringLiteral(s.as_str().into()),
            _ => mismatch(value, target, path, file, node, ctx),
        },
        Type::Int32 => match value.as_f64() {
            Some(n) if n.fract() == 0.0 => Expression::Cast {
                from: Box::new(Expression::NumberLiteral(n, Unit::None)),
                to: Type::Int32,
            },
            Some(n) => {
                ctx.diag.push_error(
                    format!("The JSON number {n} {} is not an integer", path.location(file)),
                    node,
                );
                Expression::Invalid
            }
            None => mismatch(value, target, path, file, node, ctx),
        },
        Type::Float32 => match value.as_f64() {
            Some(n) => Expression::NumberLiteral(n, Unit::None),
            None => mismatch(value, target, path, file, node, ctx),
        },
        Type::Percent => match value.as_f64() {
            Some(n) => Expression::NumberLiteral(n, Unit::Percent),
            None => mismatch(value, target, path, file, node, ctx),
        },
        Type::Duration | Type::PhysicalLength | Type::LogicalLength | Type::Rem | Type::Angle => {
            match value.as_f64() {
                Some(n) => Expression::NumberLiteral(n, target.default_unit().unwrap()),
                None => mismatch(value, target, path, file, node, ctx),
            }
        }
        Type::Color => match value {
            serde_json::Value::String(s) => color_expression(s, path, file, node, ctx),
            _ => mismatch(value, target, path, file, node, ctx),
        },
        Type::Brush => match value {
            serde_json::Value::String(s) => match color_expression(s, path, file, node, ctx) {
                Expression::Invalid => Expression::Invalid,
                color => Expression::Cast { from: Box::new(color), to: Type::Brush },
            },
            _ => mismatch(value, target, path, file, node, ctx),
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
            _ => mismatch(value, target, path, file, node, ctx),
        },
        Type::Array(elem_ty) => match value.as_array() {
            Some(arr) => {
                let elem_ty = (**elem_ty).clone();
                let mut values = Vec::with_capacity(arr.len());
                for (i, v) in arr.iter().enumerate() {
                    path.0.push(Segment::Index(i));
                    values.push(json_to_expression(v, &elem_ty, path, file, node, ctx));
                    path.0.pop();
                }
                Expression::Array { element_ty: elem_ty, values }
            }
            None => mismatch(value, target, path, file, node, ctx),
        },
        Type::Image => match value {
            serde_json::Value::String(s) => image_expression(s, file, node),
            _ => mismatch(value, target, path, file, node, ctx),
        },
        Type::Struct(st) => match value.as_object() {
            Some(obj) => {
                let mut values = std::collections::BTreeMap::new();
                for (field_name, field_ty) in &st.fields {
                    let expr = match lookup_key(obj, field_name) {
                        Some((key, v)) => {
                            path.0.push(Segment::Key(key.into()));
                            let e = json_to_expression(v, field_ty, path, file, node, ctx);
                            path.0.pop();
                            e
                        }
                        None => {
                            ctx.diag.push_warning(
                                format!(
                                    "The JSON object {} has no key for the struct field '{field_name}'; using its default value",
                                    path.location(file)
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
            None => mismatch(value, target, path, file, node, ctx),
        },
        _ => mismatch(value, target, path, file, node, ctx),
    }
}

/// An image for `reference`, which is a `data:` URI, a URL, or a path relative to the JSON `file`.
/// An empty `reference` is no image.
fn image_expression(reference: &str, file: &std::path::Path, node: &SyntaxNode) -> Expression {
    let resource_ref = if reference.is_empty() {
        ImageReference::None
    } else {
        match ImageReference::from_resolved(reference.into()) {
            ImageReference::Path(relative)
                if !crate::pathutils::is_absolute(std::path::Path::new(relative.as_str())) =>
            {
                let path = crate::pathutils::join(
                    &crate::pathutils::dirname(file),
                    std::path::Path::new(relative.as_str()),
                )
                .map_or(relative, |path| path.to_string_lossy().into());
                ImageReference::Path(path)
            }
            resource_ref => resource_ref,
        }
    };
    Expression::ImageReference {
        resource_ref,
        source_location: Some(node.to_source_location()),
        nine_slice: None,
    }
}

fn color_from_str(s: &str) -> Option<u32> {
    i_slint_common::color_parsing::parse_color_literal(s)
        .or_else(|| i_slint_common::color_parsing::named_colors().get(s).copied())
}

/// Parse `s` as a color and return a `Type::Color`-typed expression for it, or report a
/// diagnostic and return `Expression::Invalid` if `s` isn't a valid color.
fn color_expression(
    s: &str,
    path: &JsonPath,
    file: &std::path::Path,
    node: &SyntaxNode,
    ctx: &mut LookupCtx,
) -> Expression {
    match color_from_str(s) {
        Some(argb) => Expression::Cast {
            from: Box::new(Expression::NumberLiteral(argb as f64, Unit::None)),
            to: Type::Color,
        },
        None => {
            ctx.diag.push_error(
                format!("Cannot parse '{s}' as a color, {}", path.location(file)),
                node,
            );
            Expression::Invalid
        }
    }
}
