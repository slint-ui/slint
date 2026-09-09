// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;

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
    expected: text_edit::EditedText,
    received: bool,
}

pub(super) fn color_contents_changed(url: &Url, content: &str) -> bool {
    PREVIEW_STATE.with_borrow_mut(|state| {
        let Some(refresh) = state.color_refresh.as_mut() else { return false };
        if refresh.expected.url != *url || refresh.expected.contents != content {
            return false;
        }
        refresh.received = true;
        true
    })
}

pub(super) fn dismiss_color() {
    PREVIEW_STATE.with_borrow_mut(|state| {
        state.color_refresh = None;
        if let Some(api) = state.api.upgrade() {
            api.set_inspector_color_refresh_pending(false);
        }
    });
}

pub(super) fn invalidate_color() {
    dismiss_color();
    PREVIEW_STATE.with_borrow(|state| {
        if let Some(api) = state.api.upgrade() {
            api.set_inspector_color_generation(
                api.get_inspector_color_generation().wrapping_add(1),
            );
        }
    });
}

pub(super) fn preserve_color_popup() -> bool {
    PREVIEW_STATE
        .with_borrow(|state| state.color_refresh.as_ref().is_some_and(|refresh| refresh.received))
}

pub(super) fn finish_refresh() {
    let (pending, applied) = PREVIEW_STATE.with_borrow(|state| {
        let Some(refresh) = state.color_refresh.as_ref() else { return (false, false) };
        let applied = refresh.received
            && document_cache_from(state)
                .and_then(|cache| {
                    cache
                        .get_document(&refresh.expected.url)
                        .and_then(|document| document.node.as_ref())
                        .map(|node| node.text() == refresh.expected.contents.as_str())
                })
                .unwrap_or(false);
        (true, applied)
    });
    if pending {
        refresh();
        if applied {
            dismiss_color();
        }
    } else {
        invalidate();
    }
}

fn target(key: &str) -> Option<(ElementRcNode, Url, SourceFileVersion)> {
    let selected = selected_element()?;
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
        selected.instance_index
    );
    (key == expected).then_some((node, url, version))
}

fn color_target(key: &str, property_name: &str) -> Option<(ElementRcNode, Url, SourceFileVersion)> {
    let suffix = format!(":{property_name}");
    let base_key = key.strip_suffix(&suffix)?;
    target(base_key)
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
    PREVIEW_STATE.with_borrow_mut(|state| {
        if let Some(edit) = state.inspector_edit.take() {
            let overrides = state.debug_hook_overrides.borrow();
            for (id, previous) in edit.overrides {
                if let Some(property) = overrides.get(&id) {
                    property.as_ref().set(previous);
                }
            }
        }
    });
    if let Some(instance) = component_instance() {
        instance.window().request_redraw();
    }
}

pub(super) fn preview(key: SharedString, name: SharedString, value: f32) -> bool {
    let Some((node, _, _, names)) = validate(&key, &name, value) else { return false };
    let hash = node.with_element_debug(|debug| debug.element_hash);
    let ids: Vec<_> = names
        .iter()
        .map(|name| i_slint_compiler::passes::property_id(hash, &SmolStr::from(*name)))
        .collect();
    if PREVIEW_STATE.with_borrow(|state| {
        state.inspector_edit.as_ref().is_some_and(|edit| edit.key != key.as_str())
    }) {
        cancel();
    }
    PREVIEW_STATE.with_borrow_mut(|state| {
        let mut overrides = (*state.debug_hook_overrides).borrow_mut();
        let edit = state
            .inspector_edit
            .get_or_insert_with(|| Edit { key: key.to_string(), overrides: Vec::new() });
        for id in ids {
            let property = overrides
                .entry(id.clone())
                .or_insert_with(|| Box::pin(i_slint_core::Property::new(None)));
            if !edit.overrides.iter().any(|(saved, _)| saved == &id) {
                edit.overrides.push((id, property.as_ref().get()));
            }
            property.as_ref().set(Some(slint_interpreter::Value::Number(value as f64)));
        }
    });
    if let Some(instance) = component_instance() {
        instance.window().request_redraw();
    }
    true
}

pub(super) fn preview_color(
    key: SharedString,
    name: SharedString,
    value: slint::Color,
    is_brush: bool,
) -> bool {
    let Some((node, _, _)) = color_target(&key, &name) else {
        cancel();
        return false;
    };
    let hash = node.with_element_debug(|debug| debug.element_hash);
    let id = i_slint_compiler::passes::property_id(hash, &SmolStr::from(name.as_str()));
    if PREVIEW_STATE.with_borrow(|state| {
        state.inspector_edit.as_ref().is_some_and(|edit| edit.key != key.as_str())
    }) {
        cancel();
    }
    // The interpreter represents both `color` and `brush` values as a Brush. The
    // `From<Color>` conversion is the canonical representation for Color, while
    // the explicit Brush form is used for a Brush-typed property.
    let value = if is_brush {
        slint_interpreter::Value::Brush(slint::Brush::SolidColor(value))
    } else {
        slint_interpreter::Value::from(value)
    };
    PREVIEW_STATE.with_borrow_mut(|state| {
        let mut overrides = (*state.debug_hook_overrides).borrow_mut();
        let edit = state
            .inspector_edit
            .get_or_insert_with(|| Edit { key: key.to_string(), overrides: Vec::new() });
        let property = overrides
            .entry(id.clone())
            .or_insert_with(|| Box::pin(i_slint_core::Property::new(None)));
        if !edit.overrides.iter().any(|(saved, _)| saved == &id) {
            edit.overrides.push((id, property.as_ref().get()));
        }
        property.as_ref().set(Some(value));
    });
    if let Some(instance) = component_instance() {
        instance.window().request_redraw();
    }
    true
}

pub(super) fn commit_color(
    key: SharedString,
    name: SharedString,
    value: slint::Color,
    _is_brush: bool,
) -> bool {
    let Some((node, url, version)) = color_target(&key, &name) else {
        cancel();
        return false;
    };
    let color = if value.alpha() == 255 {
        slint::format!("#{:02x}{:02x}{:02x}", value.red(), value.green(), value.blue())
    } else {
        slint::format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            value.red(),
            value.green(),
            value.blue(),
            value.alpha()
        )
    };
    let Some(cache) = document_cache() else {
        cancel();
        return false;
    };
    let (_, offset) = node.path_and_offset();
    let edit = properties::update_element_properties(
        &cache,
        i_slint_editor_preview::editing::VersionedPosition::new(
            VersionedUrl::new(url, version),
            offset,
        ),
        vec![i_slint_editor_preview::editing::PropertyChange::new(
            name.as_str(),
            color.to_string(),
        )],
    );
    let Some(edit) = edit else {
        cancel();
        return false;
    };
    let expected = text_edit::apply_workspace_edit(&cache, &edit)
        .ok()
        .and_then(|mut documents| (documents.len() == 1).then(|| documents.remove(0)));
    let Some(expected) = expected else {
        cancel();
        return false;
    };
    let accepted = send_workspace_edit("Editing color".into(), edit, true);
    if accepted {
        PREVIEW_STATE.with_borrow_mut(|state| {
            state.inspector_edit.take();
            let changed = state
                .source_code
                .get(&expected.url)
                .is_none_or(|source| source.code != expected.contents);
            if changed {
                state.color_refresh = Some(ColorRefresh { expected, received: false });
                if let Some(api) = state.api.upgrade() {
                    api.set_inspector_color_refresh_pending(true);
                }
            }
        });
    } else {
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
    let Some(cache) = document_cache() else {
        cancel();
        return false;
    };
    let (_, offset) = node.path_and_offset();
    let edit = properties::update_element_properties(
        &cache,
        i_slint_editor_preview::editing::VersionedPosition::new(
            VersionedUrl::new(url, version),
            offset,
        ),
        changes,
    );
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
    PREVIEW_STATE.with_borrow(|state| {
        if let Some(api) = state.api.upgrade() {
            api.set_inspector_generation(api.get_inspector_generation().wrapping_add(1));
        }
    });
}
