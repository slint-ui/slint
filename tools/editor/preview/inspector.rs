// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use slint::Model as _;

const CORNERS: [&str; 4] = [
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-left-radius",
    "border-bottom-right-radius",
];

pub(super) struct Edit {
    key: String,
    overrides: Vec<(SmolStr, Option<slint_interpreter::Value>)>,
    changes: Vec<(String, String)>,
    fills: Vec<(String, ui::FillData)>,
}

pub(super) fn invalidate_fill() {
    let properties_api = PREVIEW_STATE.with_borrow(|state| state.properties.upgrade());
    if let Some(properties_api) = properties_api {
        properties_api.set_inspector_fill_generation(
            properties_api.get_inspector_fill_generation().wrapping_add(1),
        );
    }
}

fn target(key: &str) -> Option<(ElementRcNode, Url, SourceFileVersion)> {
    let (node, url, version, expected) = current_target()?;
    (key == expected).then_some((node, url, version))
}

fn current_target() -> Option<(ElementRcNode, Url, SourceFileVersion, String)> {
    let (selected, instance_index) = if let Some(selected) = selected_element() {
        let index = selected.instance_index as i32;
        (selected, index)
    } else {
        let (element, current_url) = PREVIEW_STATE.with_borrow(|state| {
            Some((state.api.upgrade()?.get_current_element(), state.current_component()?.url))
        })?;
        let url = Url::parse(&element.source_uri).ok()?;
        if url != current_url || element.offset < 0 {
            return None;
        }
        let no_selected_instance = -1;
        (
            ElementSelection {
                path: i_slint_compiler::source_path::SourcePath::from(url),
                offset: (element.offset as u32).into(),
                instance_index: 0,
                editing_text: false,
            },
            no_selected_instance,
        )
    };
    let node = selected.as_element_node()?;
    let (path, offset) = node.path_and_offset();
    let url = path.to_url()?;
    let version = document_cache()?.document_version(&url);
    let generation = PREVIEW_STATE.with_borrow(|state| {
        state.properties.upgrade().map(|properties_api| properties_api.get_inspector_generation())
    })?;
    let expected = format!(
        "{url}:{}:{}:{}:{generation}",
        version.unwrap_or(i32::MIN),
        u32::from(offset),
        instance_index
    );
    Some((node, url, version, expected))
}

pub(super) fn cancel() {
    let (edit, overrides) = PREVIEW_STATE
        .with_borrow_mut(|state| (state.inspector_edit.take(), state.debug_hook_overrides.clone()));
    if let Some(edit) = edit {
        let overrides = (*overrides).borrow();
        for (id, previous) in edit.overrides {
            if let Some(property) = overrides.get(&id) {
                property.as_ref().set(previous);
            }
        }
    }
    if let Some(instance) = component_instance() {
        instance.window().request_redraw();
    }
}

fn preview_values<'a>(
    key: SharedString,
    node: ElementRcNode,
    values: impl IntoIterator<Item = (&'a str, slint_interpreter::Value)>,
) -> bool {
    let hash = node.with_element_debug(|debug| debug.element_hash);
    if PREVIEW_STATE.with_borrow(|state| {
        state.inspector_edit.as_ref().is_some_and(|edit| edit.key != key.as_str())
    }) {
        cancel();
    }
    let overrides = PREVIEW_STATE.with_borrow(|state| state.debug_hook_overrides.clone());
    let mut overrides = (*overrides).borrow_mut();
    for (name, value) in values {
        let id = i_slint_compiler::passes::property_id(hash, &SmolStr::from(name));
        let property = overrides
            .entry(id.clone())
            .or_insert_with(|| Box::pin(i_slint_core::Property::new(None)));
        let previous = property.as_ref().get();
        PREVIEW_STATE.with_borrow_mut(|state| {
            let edit = state.inspector_edit.get_or_insert_with(|| Edit {
                key: key.to_string(),
                overrides: Vec::new(),
                changes: Vec::new(),
                fills: Vec::new(),
            });
            if !edit.overrides.iter().any(|(saved, _)| saved == &id) {
                edit.overrides.push((id, previous));
            }
        });
        property.as_ref().set(Some(value));
    }
    drop(overrides);
    if let Some(instance) = component_instance() {
        instance.window().request_redraw();
    }
    true
}

fn number_value(
    number: f32,
    unit: &str,
    property_type: &i_slint_compiler::langtype::Type,
) -> Option<f64> {
    use i_slint_compiler::expression_types::{Unit, WrittenUnit};
    use i_slint_compiler::langtype::Type;
    if !number.is_finite() {
        return None;
    }
    let (number, unit) = unit.parse::<WrittenUnit>().ok()?.normalize(number as f64);
    if unit.ty() == *property_type {
        return Some(number);
    }
    if unit == Unit::Percent && *property_type == Type::Float32 {
        return Some(number / 100.);
    }
    if !matches!(property_type, Type::LogicalLength | Type::PhysicalLength | Type::Rem)
        || !matches!(unit, Unit::Px | Unit::Phx | Unit::Rem)
    {
        return None;
    }
    let instance = component_instance()?;
    let scale = instance.window().scale_factor() as f64;
    let font_size = i_slint_core::items::WindowItem::resolved_default_font_size(
        instance.as_item_tree(i_slint_core::InternalToken),
    )
    .get() as f64;
    let logical = match unit {
        Unit::Phx => number / scale,
        Unit::Rem => number * font_size,
        _ => number,
    };
    Some(match property_type {
        Type::PhysicalLength => logical * scale,
        Type::Rem => logical / font_size,
        _ => logical,
    })
}

fn source_node(node: &ElementRcNode) -> Option<ElementRcNode> {
    let (path, offset) = node.path_and_offset();
    document_cache()?.element_at_offset(&path.to_url()?, offset)
}

fn string_expression(value: &str) -> String {
    let mut expression = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => expression.push_str("\\\""),
            '\\' => expression.push_str("\\\\"),
            '\n' => expression.push_str("\\n"),
            character if character.is_control() => expression.extend(character.escape_unicode()),
            character => expression.push(character),
        }
    }
    expression.push('"');
    expression
}

fn edit_value(
    node: &ElementRcNode,
    name: &str,
    value: &ui::PropertyEditValue,
) -> Option<(String, slint_interpreter::Value)> {
    use i_slint_compiler::element_type::PropertyLookupMode;
    use i_slint_compiler::langtype::Type;
    use slint_interpreter::Value;
    let node = source_node(node)?;
    let property_type = node
        .as_element()
        .borrow()
        .lookup_property(name, PropertyLookupMode::ComponentLocal)
        .property_type;
    let (expression, runtime) = match value.kind {
        ui::PropertyEditKind::Number => {
            let runtime = number_value(value.number, &value.unit, &property_type)?;
            let nonnegative = CORNERS.contains(&name)
                || matches!(
                    name,
                    "border-radius"
                        | "border-width"
                        | "backdrop-blur"
                        | "padding"
                        | "spacing"
                        | "drop-shadow-blur"
                        | "inner-shadow-blur"
                );
            if nonnegative && value.number < 0. {
                return None;
            }
            (format!("{}{}", value.number, value.unit), Value::Number(runtime))
        }
        ui::PropertyEditKind::Integer if property_type == Type::Int32 => {
            (value.integer.to_string(), Value::Number(value.integer as f64))
        }
        ui::PropertyEditKind::Boolean if property_type == Type::Bool => {
            (value.boolean.to_string(), Value::Bool(value.boolean))
        }
        ui::PropertyEditKind::String if property_type == Type::String => {
            (string_expression(&value.text), Value::String(value.text.clone()))
        }
        ui::PropertyEditKind::Enumeration if property_type == Type::MouseCursor => {
            let cursor = value.text.parse::<i_slint_core::items::BuiltInMouseCursor>().ok()?;
            (
                value.text.to_string(),
                Value::MouseCursorInner(i_slint_core::cursor::MouseCursorInner::BuiltIn(cursor)),
            )
        }
        ui::PropertyEditKind::Enumeration => {
            let Type::Enumeration(enumeration) = property_type else { return None };
            if !enumeration.values.iter().any(|member| member.as_str() == value.text.as_str()) {
                return None;
            }
            (
                value.text.to_string(),
                Value::EnumerationValue(enumeration.name.to_string(), value.text.to_string()),
            )
        }
        ui::PropertyEditKind::Fill => (
            ui::fill_expression(value.fill.clone()).to_string(),
            fill_value(&node, name, &value.fill)?,
        ),
        _ => return None,
    };
    Some((expression, runtime))
}

pub(super) fn preview_property(name: SharedString, value: ui::PropertyEditValue) -> bool {
    let Some((node, _, _, key)) = current_target() else {
        cancel();
        return false;
    };
    if PREVIEW_STATE
        .with_borrow(|state| state.inspector_edit.as_ref().is_some_and(|edit| edit.key != key))
    {
        cancel();
        return false;
    }
    let Some((expression, runtime)) = edit_value(&node, &name, &value) else { return false };
    if file_edit_pending() {
        return false;
    }
    preview_values(key.into(), node, [(name.as_str(), runtime)]);
    PREVIEW_STATE.with_borrow_mut(|state| {
        let edit = state.inspector_edit.as_mut().unwrap();
        edit.fills.retain(|(staged_name, _)| staged_name != name.as_str());
        if value.kind == ui::PropertyEditKind::Fill {
            edit.fills.push((name.to_string(), value.fill.clone()));
        }
        if let Some((_, staged)) =
            edit.changes.iter_mut().find(|(staged_name, _)| staged_name == name.as_str())
        {
            *staged = expression;
        } else {
            edit.changes.push((name.to_string(), expression));
        }
    });
    true
}

fn changed_properties(
    node: &ElementRcNode,
    changes: &[(String, String)],
) -> Vec<i_slint_editor_preview::editing::PropertyChange> {
    let properties = source_node(node)
        .map(|node| properties::get_properties(&node, properties::LayoutKind::None))
        .unwrap_or_default();
    changes
        .iter()
        .filter(|(name, expression)| {
            !properties.iter().find(|property| property.name == name.as_str()).is_some_and(
                |property| {
                    property.defined_at.as_ref().is_some_and(|definition| {
                        definition.code_block_or_expression.to_string().trim() == expression
                    })
                },
            )
        })
        .map(|(name, expression)| {
            i_slint_editor_preview::editing::PropertyChange::new(name, expression.clone())
        })
        .collect()
}

pub(super) fn commit_properties() -> bool {
    let staged = PREVIEW_STATE.with_borrow(|state| {
        state
            .inspector_edit
            .as_ref()
            .map(|edit| (edit.key.clone(), edit.changes.clone(), edit.fills.clone()))
    });
    let Some((key, changes, mut fills)) = staged else { return false };
    let Some((node, url, version)) = target(&key) else {
        cancel();
        return false;
    };
    if file_edit_pending() || changes.is_empty() {
        cancel();
        return false;
    }
    let changes = changed_properties(&node, &changes);
    if changes.is_empty() {
        cancel();
        return true;
    }
    fills.retain(|(name, _)| changes.iter().any(|change| change.name == name.as_str()));
    let accepted = property_edit(node, url, version, changes)
        .is_some_and(|edit| send_workspace_edit("Editing properties".into(), edit, true));
    if accepted && file_edit_pending() {
        notify_property_changed();
        PREVIEW_STATE.with_borrow_mut(|state| {
            state.inspector_edit.take();
        });
        if let Some(properties_api) = PREVIEW_STATE.with_borrow(|state| state.properties.upgrade())
        {
            for (_, fill) in fills {
                properties_api.invoke_add_recent_fill(fill);
            }
        }
    } else {
        cancel();
    }
    accepted
}

fn fill_value(
    node: &ElementRcNode,
    name: &str,
    fill: &ui::FillData,
) -> Option<slint_interpreter::Value> {
    use i_slint_compiler::element_type::PropertyLookupMode;
    use i_slint_compiler::langtype::Type;
    if !fill.angle.is_finite()
        || !fill.center_x.is_finite()
        || !fill.center_y.is_finite()
        || !fill.radius.is_finite()
        || !fill.radius_y.is_finite()
        || (fill.custom_radius && fill.radius <= 0.)
        || (fill.custom_radius && fill.radial_ellipse && fill.radius_y <= 0.)
        || fill.stops.iter().any(|s| !s.position.is_finite())
    {
        return None;
    }
    match node
        .as_element()
        .borrow()
        .lookup_property(name, PropertyLookupMode::ComponentLocal)
        .property_type
    {
        Type::Color if fill.kind == ui::BrushKind::Solid => Some(fill.color.into()),
        Type::Brush => Some(ui::fill_brush(fill.clone()).into()),
        _ => None,
    }
}

fn property_edit(
    node: ElementRcNode,
    url: Url,
    version: SourceFileVersion,
    changes: Vec<i_slint_editor_preview::editing::PropertyChange>,
) -> Option<lsp_types::WorkspaceEdit> {
    let cache = document_cache()?;
    let (_, offset) = node.path_and_offset();
    properties::update_element_properties(
        &cache,
        i_slint_editor_preview::editing::VersionedPosition::new(
            VersionedUrl::new(url, version),
            offset,
        ),
        changes,
    )
}

pub(super) fn invalidate() {
    invalidate_fill();
    refresh();
}

pub(super) fn refresh() {
    cancel();
    let properties_api = PREVIEW_STATE.with_borrow(|state| state.properties.upgrade());
    if let Some(properties_api) = properties_api {
        properties_api
            .set_inspector_generation(properties_api.get_inspector_generation().wrapping_add(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_expressions_roundtrip() {
        for value in [
            "apostrophe's",
            "tab\there",
            "carriage\rreturn",
            "back\\slash",
            "\\{interpolation}",
            "quotes \"here\"\n🦊\0",
        ] {
            assert_eq!(
                i_slint_compiler::literals::unescape_string(&string_expression(value)).as_deref(),
                Some(value)
            );
        }
    }

    #[test]
    fn typed_numbers_normalize_units_and_validate_the_property_type() {
        use i_slint_compiler::langtype::Type;
        for (number, unit, property_type, expected) in [
            (2., "cm", Type::LogicalLength, Some(75.6)),
            (2., "s", Type::Duration, Some(2000.)),
            (0.5, "turn", Type::Angle, Some(180.)),
            (50., "%", Type::Percent, Some(50.)),
            (50., "%", Type::Float32, Some(0.5)),
            (0.5, "", Type::Float32, Some(0.5)),
            (f32::NAN, "px", Type::LogicalLength, None),
            (f32::INFINITY, "deg", Type::Angle, None),
            (2., "px", Type::Angle, None),
            (2., "invalid", Type::LogicalLength, None),
            (2., "", Type::Bool, None),
        ] {
            assert_eq!(number_value(number, unit, &property_type), expected);
        }
    }
}
