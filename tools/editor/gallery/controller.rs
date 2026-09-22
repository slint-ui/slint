// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::catalog;
use i_slint_core::DataTransfer;
use slint::{Color, ComponentHandle, ModelRc, SharedString, VecModel};
use slint_editor::{
    component_support::{brushes, element_library, recent_fills},
    ui::*,
};

fn model<T: Clone + 'static>(values: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(values))
}

fn outline(window: &GalleryWindow, expanded: bool) {
    let scenario = window.global::<Gallery>().get_scenario();
    let rows = [
        ("Main", ElementKind::Component),
        ("card", ElementKind::Rectangle),
        ("title", ElementKind::Text),
        ("artwork", ElementKind::Image),
    ];
    window.global::<Api>().set_outline(model(
        rows.into_iter()
            .enumerate()
            .take(if scenario == "Empty" {
                0
            } else if expanded {
                4
            } else {
                1
            })
            .map(|(i, (label, kind))| OutlineTreeNode {
                indent_level: i32::from(i > 0),
                has_children: i == 0,
                is_expanded: expanded,
                is_last_child: i == 3,
                icon_kind: kind,
                element_type: format!("{kind:?}").into(),
                element_id: if scenario == "Long names" {
                    format!("{label}-with-a-long-component-name").into()
                } else {
                    label.into()
                },
                uri: "gallery".into(),
                offset: i as i32,
            })
            .collect(),
    ));
}

pub fn navigate(window: &GalleryWindow, id: &str, scenario: &str) {
    let Some(page) = catalog::page(id) else { return };
    window.invoke_clear_transients();
    let g = window.global::<Gallery>();
    g.set_page(id.into());
    g.set_title(page.title.into());
    g.set_description(page.description.into());
    g.set_scenarios(model(page.scenarios.iter().map(|s| (*s).into()).collect()));
    g.set_scenario(scenario.into());
    g.set_scenario_index(page.scenarios.iter().position(|s| *s == scenario).unwrap_or(0) as i32);
    g.set_feedback("Ready".into());
    let api = window.global::<Api>();
    api.set_editor_surface_mode(if scenario == "Unavailable" {
        EditorSurfaceMode::Image
    } else {
        EditorSurfaceMode::Component
    });
    match id {
        "palette" => api.set_known_components(if scenario == "Dragging disabled" {
            Default::default()
        } else {
            model(vec![ComponentListItem { category: "Gallery".into(), ..Default::default() }])
        }),
        "outline" => {
            api.set_current_element(Default::default());
            outline(window, scenario != "Collapsed");
        }
        "picker" => {
            api.set_current_element(ElementInformation {
                source_uri: "gallery".into(),
                ..Default::default()
            });
            api.set_recent_fills(Default::default());
            g.set_fill(FillData {
                kind: match scenario {
                    "Linear" => BrushKind::Linear,
                    "Radial" => BrushKind::Radial,
                    "Conic" => BrushKind::Conic,
                    _ => BrushKind::Solid,
                },
                color: Color::from_argb_u8(
                    if scenario == "Transparent" { 80 } else { 255 },
                    92,
                    105,
                    225,
                ),
                angle: 120.,
                stops: model(vec![
                    GradientStop { color: Color::from_rgb_u8(92, 105, 225), position: 0. },
                    GradientStop { color: Color::from_rgb_u8(238, 134, 172), position: 1. },
                ]),
                ..Default::default()
            });
        }
        _ => {}
    }
}

pub fn install(window: &GalleryWindow) {
    let api = window.global::<Api>();
    let gallery = window.global::<Gallery>();
    gallery.on_matches(|text, query| {
        text.to_lowercase().contains(query.trim().to_lowercase().as_str())
    });
    gallery.set_pages(model(
        catalog::PAGES
            .iter()
            .map(|p| GalleryPage { id: p.id.into(), title: p.title.into() })
            .collect(),
    ));
    brushes::setup(&api);
    element_library::setup(&api);
    recent_fills::setup(&api, <Api as slint::Global<'_, GalleryWindow>>::as_weak(&api));
    let weak = window.as_weak();
    gallery.on_navigate(move |page, scenario| {
        if let Some(w) = weak.upgrade() {
            navigate(&w, &page, &scenario);
        }
    });
    let weak = window.as_weak();
    gallery.on_open_picker(move |anchor| {
        let Some(w) = weak.upgrade() else { return };
        let api = w.global::<Api>();
        let fill = w.global::<Gallery>().get_fill();
        w.global::<FillSession>().invoke_begin(FillSessionRequest {
            expression: brushes::fill_expression(fill.clone()),
            fill,
            target: FillSessionTarget {
                key: "sample".into(),
                session_key: format!(
                    "gallery:0:{}:{}:background",
                    api.get_selection().highlight_index,
                    api.get_inspector_fill_generation()
                )
                .into(),
                property_name: "background".into(),
                size: slint::LogicalSize::new(240., 160.),
                canvas: false,
            },
            unsupported: w.global::<Gallery>().get_scenario() == "Unsupported",
            allow_gradient: true,
            anchor_position: anchor,
            anchor_width: 24.,
        });
    });
    api.on_inspector_fill_preview(|_, _, _| true);
    let weak = window.as_weak();
    api.on_inspector_fill_commit(move |_, _, fill| {
        let Some(w) = weak.upgrade() else { return false };
        w.global::<Gallery>().set_fill(fill);
        true
    });
    let weak = window.as_weak();
    api.on_outline_select_element(move |uri, offset, _| {
        let Some(w) = weak.upgrade() else { return };
        let api = w.global::<Api>();
        api.set_current_element(ElementInformation {
            source_uri: uri,
            offset,
            ..Default::default()
        });
        w.global::<Gallery>().set_feedback(format!("Selected row {offset}").into());
    });
    let weak = window.as_weak();
    api.on_outline_set_expanded(move |_, expanded| {
        if let Some(w) = weak.upgrade() {
            outline(&w, expanded);
        }
    });
    api.on_new_component_data_for_kind(|kind| {
        DataTransfer::from(SharedString::from(format!("{kind:?}")))
    });
    api.on_move_element_instance_data(|_, id| {
        DataTransfer::from(SharedString::from(format!("Row {id}")))
    });
    api.on_outline_can_drop(|_, _, _, _| true);
    let weak = window.as_weak();
    api.on_outline_drop(move |_, _, target, location| {
        if let Some(w) = weak.upgrade() {
            w.global::<Gallery>().set_feedback(format!("Dropped {location:?} row {target}").into());
        }
    });
    let weak = window.as_weak();
    api.on_drop(move |data, _, _| {
        if let Some(w) = weak.upgrade() {
            let label = data.plain_text().unwrap_or_default();
            w.global::<Gallery>().set_feedback(format!("Dropped {label}").into());
        }
    });
}
