// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{
    settings::{PreviewDisplay, PreviewState},
    ui,
};
use slint::{ComponentHandle, Model};

const LIBRARY: &[(&str, i32, i32, &str)] = &[
    ("iPhone", 390, 844, "mobile"),
    ("iPhone Pro", 393, 852, "mobile"),
    ("iPhone Pro Max", 430, 932, "mobile"),
    ("iPhone SE", 375, 667, "mobile"),
    ("iPad", 820, 1180, "tablet"),
    ("iPad mini", 744, 1133, "tablet"),
    ("iPad Pro", 1024, 1366, "tablet"),
    ("Android", 412, 915, "mobile"),
    ("Android tablet", 800, 1280, "tablet"),
    ("Desktop", 1280, 720, "desktop"),
    ("Desktop 1440p", 2560, 1440, "desktop"),
    ("Desktop 4K", 3840, 2160, "desktop"),
    ("Laptop", 1440, 900, "desktop"),
    ("Embedded 320 × 240", 320, 240, "embedded"),
    ("Embedded 800 × 480", 800, 480, "embedded"),
];

fn to_ui(display: &PreviewDisplay) -> ui::PreviewDisplay {
    ui::PreviewDisplay {
        name: display.name.as_str().into(),
        width: display.width as f32,
        height: display.height as f32,
        kind: display.kind.as_str().into(),
        rotatable: display.can_rotate(),
    }
}

fn model(displays: impl IntoIterator<Item = PreviewDisplay>) -> slint::ModelRc<ui::PreviewDisplay> {
    std::rc::Rc::new(slint::VecModel::from(
        displays.into_iter().map(|d| to_ui(&d)).collect::<Vec<_>>(),
    ))
    .into()
}

pub(super) fn setup(editor: &ui::EditorUi) {
    let displays = editor.global::<ui::PreviewDisplays>();
    displays.on_search(|query| {
        let query = query.trim().to_lowercase();
        model(LIBRARY.iter().filter(|(name, ..)| name.to_lowercase().contains(&query)).map(
            |&(name, width, height, kind)| PreviewDisplay {
                name: name.into(),
                width,
                height,
                kind: kind.into(),
                rotatable: matches!(kind, "mobile" | "tablet"),
            },
        ))
    });
    displays.on_select_project_size(|| {
        let update = super::PREVIEW_STATE.with_borrow_mut(|state| {
            let root = state.current_project_root.as_ref()?.to_file_path().ok()?;
            state.settings.clear_preview_state(&root).then(|| state.settings.serialize())
        });
        save(update);
    });
    displays.on_delete_display(|index| {
        let update = super::PREVIEW_STATE.with_borrow_mut(|state| {
            let index =
                usize::try_from(index).ok().filter(|&i| i < state.settings.displays.len())?;
            let deleted = state.settings.displays.remove(index);
            state.settings.preview_states.retain(|_, saved| saved.display != deleted);
            Some(state.settings.serialize())
        });
        save(update);
    });
    displays.on_select_display(|index| {
        let display = super::PREVIEW_STATE.with_borrow(|state| {
            usize::try_from(index)
                .ok()
                .and_then(|index| state.settings.displays.get(index).cloned())
        });
        if let Some(display) = display {
            select(display);
        }
    });
    displays.on_add_display(|display| {
        let valid = |value: f32| {
            value.is_finite()
                && value > 0.
                && value.fract() == 0.
                && f64::from(value) <= f64::from(i32::MAX)
        };
        if !valid(display.width) || !valid(display.height) || display.name.is_empty() {
            return false;
        }
        select(PreviewDisplay {
            name: display.name.into(),
            width: display.width as i32,
            height: display.height as i32,
            kind: display.kind.into(),
            rotatable: display.rotatable,
        });
        true
    });
    displays.on_add_custom_display(|width, height, rotatable| {
        let result = (|| {
            let width = dimension(&width)?;
            let height = dimension(&height)?;
            select(PreviewDisplay {
                name: format!("{width} × {height}"),
                width,
                height,
                kind: "custom".into(),
                rotatable,
            });
            Ok::<_, String>(())
        })();
        super::PREVIEW_STATE.with_borrow(|state| {
            if let Some(editor) = &state.editor_ui {
                editor
                    .global::<ui::PreviewDisplays>()
                    .set_error(result.as_ref().err().map_or("", String::as_str).into());
            }
        });
        result.is_ok()
    });
    displays.on_set_landscape(|landscape| {
        let update = super::PREVIEW_STATE.with_borrow_mut(|state| {
            let root = state.current_project_root.as_ref()?.to_file_path().ok()?;
            let saved = state.settings.preview_state(&root)?;
            if !saved.display.can_rotate() || saved.landscape == landscape {
                return None;
            }
            let display = saved.display.clone();
            state
                .settings
                .set_preview_state(&root, PreviewState { display, landscape })
                .then(|| state.settings.serialize())
        });
        save(update);
    });
    refresh();
}

pub(super) fn dimension(value: &str) -> Result<i32, String> {
    value
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| "Width and height must be positive integers".into())
}

fn select(display: PreviewDisplay) {
    let update = super::PREVIEW_STATE.with_borrow_mut(|state| {
        let root = state.current_project_root.as_ref()?.to_file_path().ok()?;
        let mut changed = false;
        if !state.settings.displays.contains(&display) {
            state.settings.displays.push(display.clone());
            changed = true;
        }
        let landscape = display.width > display.height;
        changed |= state.settings.set_preview_state(&root, PreviewState { display, landscape });
        changed.then(|| state.settings.serialize())
    });
    save(update);
}

fn save(update: Option<String>) {
    refresh();
    if let Some(contents) = update {
        super::send_settings_update(contents);
    }
}

pub(super) fn refresh() {
    super::PREVIEW_STATE.with_borrow(|state| {
        let Some(editor) = &state.editor_ui else { return };
        let displays = editor.global::<ui::PreviewDisplays>();
        let saved = state
            .current_project_root
            .as_ref()
            .and_then(|url| url.to_file_path().ok())
            .and_then(|root| state.settings.preview_state(&root))
            .filter(|saved| state.settings.displays.contains(&saved.display));
        let (size, landscape, index) = if let Some(saved) = saved {
            let display = &saved.display;
            let (short, long) =
                (display.width.min(display.height), display.width.max(display.height));
            let (width, height) = if !display.can_rotate() {
                (display.width, display.height)
            } else if saved.landscape {
                (long, short)
            } else {
                (short, long)
            };
            (
                slint::LogicalSize::new(width as f32, height as f32),
                saved.landscape,
                state.settings.displays.iter().position(|d| d == display).map_or(-1, |i| i as i32),
            )
        } else {
            let size = editor.global::<ui::Project>().get_window_size();
            (size, size.width > size.height, -1)
        };
        if !displays.get_displays().iter().eq(state.settings.displays.iter().map(to_ui)) {
            displays.set_displays(model(state.settings.displays.clone()));
        }
        displays.set_selected_index(index);
        displays.set_landscape(landscape);
        displays.set_size(size);
    });
}
