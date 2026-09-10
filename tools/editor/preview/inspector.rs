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
}

pub(super) struct ColorRefresh {
    pub(super) expected: text_edit::EditedText,
    pub(super) submitted_edit: lsp_types::WorkspaceEdit,
    pub(super) color: ui::FillData,
    pub(super) undo: Option<undo_redo::EditItem>,
}

pub(super) fn color_contents_changed(url: &Url, content: &str) -> bool {
    PREVIEW_STATE.with_borrow_mut(|state| {
        let changed = state.source_code.get(url).is_none_or(|source| source.code != content);
        let Some(refresh) = state.color_refresh.as_mut() else { return false };
        if refresh.expected.url != *url {
            return false;
        }
        if refresh.expected.contents != content {
            if changed {
                refresh.undo = None;
            }
            return false;
        }
        true
    })
}

fn clear_color_refresh() {
    let api = PREVIEW_STATE.with_borrow_mut(|state| {
        state.color_refresh = None;
        state.api.upgrade()
    });
    if let Some(api) = api {
        api.set_inspector_color_refresh_pending(false);
    }
}

pub(super) fn invalidate_color() {
    let api = PREVIEW_STATE.with_borrow(|state| state.api.upgrade());
    if let Some(api) = api {
        api.set_inspector_color_generation(api.get_inspector_color_generation().wrapping_add(1));
    }
}

pub(super) fn workspace_edit_finished(edit: lsp_types::WorkspaceEdit, applied: bool) {
    let result = PREVIEW_STATE.with_borrow_mut(|state| {
        let refresh = state.color_refresh.as_mut()?;
        if refresh.submitted_edit != edit {
            return None;
        }
        let color = refresh.color.clone();
        if !applied {
            state.workspace_edit_sent = false;
        } else {
            if let Some(undo) = refresh.undo.take() {
                state.undo_redo_stack.push_item(undo);
            }
            state.inspector_edit.take();
        }
        Some((state.api.upgrade(), color))
    });
    let Some((api, color)) = result else { return };
    if applied {
        if let Some(api) = api {
            if color.kind == ui::BrushKind::Solid { api.invoke_add_recent_color(color.color); }
        }
    } else {
        cancel();
        invalidate_color();
    }
    clear_color_refresh();
    PREVIEW_STATE.with_borrow(undo_redo::set_undo_redo_enabled);
    undo_redo::apply_pending();
}

fn target(key: &str) -> Option<(ElementRcNode, Url, SourceFileVersion)> {
    target_with_root(key, false)
}

fn target_with_root(
    key: &str,
    allow_root: bool,
) -> Option<(ElementRcNode, Url, SourceFileVersion)> {
    let (selected, instance_index) = if let Some(selected) = selected_element() {
        let index = selected.instance_index as i32;
        (selected, index)
    } else if allow_root {
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
                path: url.to_file_path().ok()?,
                offset: (element.offset as u32).into(),
                instance_index: 0,
            },
            no_selected_instance,
        )
    } else {
        return None;
    };
    let node = selected.as_element_node()?;
    let (path, offset) = node.path_and_offset();
    let url = Url::from_file_path(path).ok()?;
    let version = document_cache()?.document_version(&url);
    let generation = PREVIEW_STATE
        .with_borrow(|state| state.api.upgrade().map(|api| api.get_inspector_generation()))?;
    let expected = format!(
        "{url}:{}:{}:{}:{generation}",
        version.unwrap_or(i32::MIN),
        u32::from(offset),
        instance_index
    );
    (key == expected).then_some((node, url, version))
}

fn color_target(key: &str, property_name: &str) -> Option<(ElementRcNode, Url, SourceFileVersion)> {
    let suffix = format!(":{property_name}");
    let base_key = key.strip_suffix(&suffix)?;
    let allow_root_background = property_name == "background";
    target_with_root(base_key, allow_root_background)
}

fn names(name: &str) -> Option<Vec<&str>> {
    match name {
        "all-corners" => Some(CORNERS.to_vec()),
        "transform-rotation" => Some(vec![name]),
        name if CORNERS.contains(&name) => Some(vec![name]),
        _ => None,
    }
}

fn validate<'a>(
    key: &str,
    name: &'a str,
    value: f32,
) -> Option<(ElementRcNode, Url, SourceFileVersion, Vec<&'a str>)> {
    let Some((node, url, version)) = target(key) else {
        cancel();
        return None;
    };
    let names = names(name)?;
    (value.is_finite() && (name == "transform-rotation" || value >= 0.))
        .then_some((node, url, version, names))
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

fn preview_value(
    key: SharedString,
    node: ElementRcNode,
    names: &[&str],
    value: slint_interpreter::Value,
) -> bool {
    let hash = node.with_element_debug(|debug| debug.element_hash);
    if PREVIEW_STATE.with_borrow(|state| {
        state.inspector_edit.as_ref().is_some_and(|edit| edit.key != key.as_str())
    }) {
        cancel();
    }
    let overrides = PREVIEW_STATE.with_borrow(|state| state.debug_hook_overrides.clone());
    let mut overrides = (*overrides).borrow_mut();
    for name in names {
        let id = i_slint_compiler::passes::property_id(hash, &SmolStr::from(*name));
        let property = overrides
            .entry(id.clone())
            .or_insert_with(|| Box::pin(i_slint_core::Property::new(None)));
        let previous = property.as_ref().get();
        PREVIEW_STATE.with_borrow_mut(|state| {
            let edit = state
                .inspector_edit
                .get_or_insert_with(|| Edit { key: key.to_string(), overrides: Vec::new() });
            if !edit.overrides.iter().any(|(saved, _)| saved == &id) {
                edit.overrides.push((id, previous));
            }
        });
        property.as_ref().set(Some(value.clone()));
    }
    drop(overrides);
    if let Some(instance) = component_instance() {
        instance.window().request_redraw();
    }
    true
}

pub(super) fn preview(key: SharedString, name: SharedString, value: f32) -> bool {
    let Some((node, _, _, names)) = validate(&key, &name, value) else { return false };
    preview_value(key, node, &names, slint_interpreter::Value::Number(value as f64))
}

fn fill_value(node: &ElementRcNode, name: &str, fill: &ui::FillData) -> Option<slint_interpreter::Value> {
    use i_slint_compiler::langtype::{Type, PropertyLookupMode};
    if !fill.angle.is_finite() || !fill.center_x.is_finite() || !fill.center_y.is_finite()
        || !fill.radius.is_finite() || (fill.custom_radius && fill.radius <= 0.)
        || fill.stops.iter().any(|s| !s.position.is_finite()) {
        return None;
    }
    match node.as_element().borrow().lookup_property(name, PropertyLookupMode::ComponentLocal).property_type {
        Type::Color if fill.kind == ui::BrushKind::Solid => Some(fill.color.into()),
        Type::Brush => Some(ui::fill_brush(fill.clone()).into()),
        _ => None,
    }
}

pub(super) fn preview_color(key: SharedString, name: SharedString, value: ui::FillData) -> bool {
    let Some((node, _, _)) = color_target(&key, &name) else {
        cancel();
        return false;
    };
    let Some(value) = fill_value(&node, &name, &value) else { cancel(); return false; };
    preview_value(key, node, &[name.as_str()], value)
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

pub(super) fn commit_color(key: SharedString, name: SharedString, value: ui::FillData) -> bool {
    let Some((node, url, version)) = color_target(&key, &name) else {
        cancel();
        return false;
    };
    if fill_value(&node, &name, &value).is_none() { cancel(); return false; }
    let color = ui::fill_expression(value.clone());
    let edit = property_edit(
        node,
        url,
        version,
        vec![i_slint_editor_preview::editing::PropertyChange::new(
            name.as_str(),
            color.to_string(),
        )],
    );
    let Some(edit) = edit else {
        cancel();
        return false;
    };
    let accepted = submit_workspace_edit("Editing color".into(), edit, true, Some(value));
    if !accepted {
        cancel();
    }
    accepted
}

pub(super) fn commit(key: SharedString, name: SharedString, value: f32) -> bool {
    let Some((node, url, version, names)) = validate(&key, &name, value) else { return false };
    let unit = if name == "transform-rotation" { "deg" } else { "px" };
    let changes = names
        .iter()
        .map(|name| {
            i_slint_editor_preview::editing::PropertyChange::new(name, format!("{value}{unit}"))
        })
        .collect::<Vec<_>>();
    let edit = property_edit(node, url, version, changes);
    let accepted = edit.is_some_and(|edit| {
        send_workspace_edit(
            if name == "transform-rotation" {
                "Rotating element"
            } else {
                "Changing border radius"
            }
            .into(),
            edit,
            true,
        )
    });
    if accepted {
        PREVIEW_STATE.with_borrow_mut(|state| {
            state.inspector_edit.take();
        });
    } else {
        cancel();
    }
    accepted
}

pub(super) fn values(key: SharedString) -> slint::ModelRc<f32> {
    let values = (|| {
        let (node, _, _) = target(&key)?;
        let selected = selected_element()?;
        let instance = component_instance()?;
        let geometry =
            instance.element_positions(&node.element).get(selected.instance_index).copied()?;
        let radii = geometry.corner_radii;
        Some(vec![
            geometry.transform_rotation,
            radii.top_left,
            radii.top_right,
            radii.bottom_left,
            radii.bottom_right,
        ])
    })()
    .unwrap_or_else(|| vec![0.; 5]);
    slint::ModelRc::new(slint::VecModel::from(values))
}

pub(super) fn invalidate() {
    invalidate_color();
    refresh();
}

pub(super) fn refresh() {
    cancel();
    let api = PREVIEW_STATE.with_borrow(|state| state.api.upgrade());
    if let Some(api) = api {
        api.set_inspector_generation(api.get_inspector_generation().wrapping_add(1));
    }
}
