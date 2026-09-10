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
    preserve_popup: bool,
    write_completed: bool,
    submitted_edit: lsp_types::WorkspaceEdit,
    color: slint::Color,
    previous_history: undo_redo::UndoRedoStack,
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
        if let Some(refresh) = state.color_refresh.as_mut() {
            refresh.preserve_popup = false;
        }
    });
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
    dismiss_color();
    let api = PREVIEW_STATE.with_borrow(|state| state.api.upgrade());
    if let Some(api) = api {
        api.set_inspector_color_generation(api.get_inspector_color_generation().wrapping_add(1));
    }
}

pub(super) fn preserve_color_popup() -> bool {
    PREVIEW_STATE.with_borrow(|state| {
        state
            .color_refresh
            .as_ref()
            .is_some_and(|refresh| refresh.received && refresh.preserve_popup)
    })
}

pub(super) fn workspace_edit_finished(edit: lsp_types::WorkspaceEdit, applied: bool) {
    let result = PREVIEW_STATE.with_borrow_mut(|state| {
        let refresh = state.color_refresh.as_mut()?;
        if refresh.submitted_edit != edit {
            return None;
        }
        let color = refresh.color;
        refresh.write_completed = true;
        let preserve_popup = refresh.preserve_popup;
        if !applied {
            let mut previous = refresh.previous_history.clone();
            for (url, source) in &state.source_code {
                previous.check_set_contents_valid(url, &source.code);
            }
            state.undo_redo_stack = previous;
            state.workspace_edit_sent = false;
        } else {
            state.inspector_edit.take();
        }
        Some((state.api.upgrade(), color, preserve_popup))
    });
    let Some((api, color, preserve_popup)) = result else { return };
    if applied {
        if let Some(api) = api {
            api.invoke_add_recent_color(color);
        }
        if !preserve_popup {
            clear_color_refresh();
        }
    } else {
        cancel();
        clear_color_refresh();
        invalidate_color();
        PREVIEW_STATE.with_borrow(undo_redo::set_undo_redo_enabled);
        undo_redo::apply_pending();
    }
}

pub(super) fn finish_refresh() {
    let (pending, applied, write_completed) = PREVIEW_STATE.with_borrow(|state| {
        let Some(refresh) = state.color_refresh.as_ref() else { return (false, false, false) };
        let applied = refresh.received
            && document_cache_from(state)
                .and_then(|cache| {
                    cache
                        .get_document(&refresh.expected.url)
                        .and_then(|document| document.node.as_ref())
                        .map(|node| node.text() == refresh.expected.contents.as_str())
                })
                .unwrap_or(false);
        (true, applied, refresh.write_completed)
    });
    if pending && applied {
        refresh();
        clear_color_refresh();
    } else if pending && !write_completed {
        refresh();
    } else {
        clear_color_refresh();
        invalidate();
    }
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
        (
            ElementSelection {
                path: url.to_file_path().ok()?,
                offset: (element.offset as u32).into(),
                instance_index: 0,
            },
            -1,
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
    target_with_root(base_key, property_name == "background")
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

pub(super) fn preview_color(key: SharedString, name: SharedString, value: slint::Color) -> bool {
    let Some((node, _, _)) = color_target(&key, &name) else {
        cancel();
        return false;
    };
    let value = slint_interpreter::Value::from(value);
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

pub(super) fn commit_color(key: SharedString, name: SharedString, value: slint::Color) -> bool {
    let Some((node, url, version)) = color_target(&key, &name) else {
        cancel();
        return false;
    };
    let color = ui::color_to_string(value);
    let Some(cache) = document_cache() else {
        cancel();
        return false;
    };
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
    let expected = text_edit::apply_workspace_edit(&cache, &edit)
        .ok()
        .and_then(|mut documents| (documents.len() == 1).then(|| documents.remove(0)));
    let Some(expected) = expected else {
        cancel();
        return false;
    };
    let changed = PREVIEW_STATE.with_borrow(|state| {
        state.source_code.get(&expected.url).is_none_or(|source| source.code != expected.contents)
    });
    if !changed {
        cancel();
        return true;
    }
    let previous_history = PREVIEW_STATE.with_borrow(|state| state.undo_redo_stack.clone());
    let accepted = send_workspace_edit("Editing color".into(), edit.clone(), true);
    if accepted {
        let api = PREVIEW_STATE.with_borrow_mut(|state| {
            state.color_refresh = Some(ColorRefresh {
                expected,
                received: false,
                preserve_popup: true,
                write_completed: false,
                submitted_edit: edit,
                color: value,
                previous_history,
            });
            state.api.upgrade()
        });
        if let Some(api) = api {
            api.set_inspector_color_refresh_pending(true);
        }
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
