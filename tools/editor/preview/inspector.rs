// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;

const CORNERS: [&str; 4] = [
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-left-radius",
    "border-bottom-right-radius",
];

pub(super) type SavedOverrides = Vec<(SmolStr, Option<slint_interpreter::Value>)>;

pub(super) struct Edit {
    key: String,
    #[cfg(feature = "system-testing")]
    work: test_sync::Work,
    overrides: SavedOverrides,
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
            #[cfg(feature = "system-testing")]
            test_sync::Work::combine(vec![
                edit.work.clone(),
                test_sync::Work::capture("gesture cancellation"),
            ])
            .run(|| test_sync::effect("canceled"));
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

pub(super) fn restore_committed() {
    PREVIEW_STATE.with_borrow_mut(|state| {
        let overrides = state.debug_hook_overrides.borrow();
        for (id, previous) in state.committed_inspector_overrides.drain(..) {
            if let Some(property) = overrides.get(&id) {
                property.as_ref().set(previous);
            }
        }
    });
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
        let edit = state.inspector_edit.get_or_insert_with(|| Edit {
            key: key.to_string(),
            overrides: Vec::new(),
            #[cfg(feature = "system-testing")]
            work: test_sync::Work::capture("inspector gesture"),
        });
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

pub(super) fn commit(key: SharedString, name: SharedString, value: f32) -> bool {
    #[cfg(feature = "system-testing")]
    {
        let gesture = PREVIEW_STATE
            .with_borrow(|s| s.inspector_edit.as_ref().map(|e| e.work.clone()).unwrap_or_default());
        test_sync::Work::combine(vec![gesture, test_sync::Work::capture("inspector commit")])
            .run(|| commit_impl(key, name, value))
    }
    #[cfg(not(feature = "system-testing"))]
    commit_impl(key, name, value)
}

fn commit_impl(key: SharedString, name: SharedString, value: f32) -> bool {
    let Some((node, url, version, names)) = validate(&key, &name, value) else {
        #[cfg(feature = "system-testing")]
        test_sync::effect("rejected");
        return false;
    };
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
            if let Some(edit) = state.inspector_edit.take() {
                state.committed_inspector_overrides = edit.overrides;
            }
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
    cancel();
    PREVIEW_STATE.with_borrow(|state| {
        if let Some(api) = state.api.upgrade() {
            api.set_inspector_generation(api.get_inspector_generation().wrapping_add(1));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_cleanup_preserves_unrelated_overrides() {
        PREVIEW_STATE.with_borrow_mut(|state| {
            let mut overrides = (*state.debug_hook_overrides).borrow_mut();
            for (id, value) in [("owned", 45.), ("unrelated", 12.)] {
                overrides.insert(
                    id.into(),
                    Box::pin(i_slint_core::Property::new(Some(slint_interpreter::Value::Number(
                        value,
                    )))),
                );
            }
            state.committed_inspector_overrides = vec![("owned".into(), None)];
        });
        restore_committed();
        PREVIEW_STATE.with_borrow(|state| {
            let overrides = state.debug_hook_overrides.borrow();
            assert_eq!(overrides["owned"].as_ref().get(), None);
            assert_eq!(
                overrides["unrelated"].as_ref().get(),
                Some(slint_interpreter::Value::Number(12.))
            );
            assert!(state.committed_inspector_overrides.is_empty());
        });
        PREVIEW_STATE.with_borrow_mut(|state| *state = super::super::PreviewState::default());
    }
}
