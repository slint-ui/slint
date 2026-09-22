// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{
    catalog,
    model::{Node, Scene, model, type_name, update_model},
};
use i_slint_core::DataTransfer;
use slint::{ComponentHandle, Model, SharedString};
use slint_editor::{
    component_support::{brushes, element_catalog, element_library, recent_fills},
    ui::*,
};
use std::{cell::RefCell, rc::Rc};

type State = Rc<RefCell<Scene>>;

pub fn record(window: &GalleryWindow, message: impl Into<SharedString>) {
    let g = window.global::<Gallery>();
    let mut events = vec![message.into()];
    events.extend(g.get_events().iter().take(49));
    g.set_events(model(events));
}

fn sync(window: &GalleryWindow, state: &State) {
    let s = state.borrow().clone();
    let g = window.global::<Gallery>();
    g.set_nodes(update_model(
        g.get_nodes(),
        s.nodes.iter().filter(|n| n.parent.is_some()).map(|n| n.view.clone()).collect(),
    ));
    g.set_highlights(update_model(
        g.get_highlights(),
        s.selected()
            .filter(|n| n.parent.is_some())
            .map(|n| geometry(&n.view))
            .into_iter()
            .collect(),
    ));
    g.set_selected(s.selected);
    g.set_generation(g.get_generation().wrapping_add(1));
    let api = window.global::<Api>();
    api.set_outline(update_model(api.get_outline(), s.outline()));
    api.set_undo_enabled(!s.history.is_empty());
    api.set_redo_enabled(!s.future.is_empty());
    if let Some(n) = s.selected() {
        api.set_current_element(ElementInformation {
            id: n.view.label.clone(),
            component_name: "Main".into(),
            type_name: type_name(n.view.kind).into(),
            source_uri: "gallery".into(),
            source_version: 1,
            offset: n.view.id,
        });
        api.set_selection(Selection {
            highlight_index: 0,
            is_interactive: true,
            is_moveable: !g.get_disabled(),
            is_resizable: !g.get_disabled(),
            ..Default::default()
        });
    } else {
        api.set_current_element(Default::default());
        api.set_selection(Selection { highlight_index: -1, ..Default::default() });
    }
}

fn geometry(n: &GalleryNode) -> SelectionRectangle {
    SelectionRectangle {
        x: n.x,
        y: n.y,
        width: n.width,
        height: n.height,
        angle: n.rotation,
        describes_element: true,
        top_left_radius: n.radius,
        top_right_radius: n.radius,
        bottom_left_radius: n.radius,
        bottom_right_radius: n.radius,
    }
}

fn property(window: &GalleryWindow, state: &State, name: &str) -> PropertyValue {
    let g = window.global::<Gallery>();
    let _ = g.get_generation();
    let s = state.borrow();
    let Some(n) = s.selected() else { return Default::default() };
    let mut p = PropertyValue {
        value_resolved: !matches!(g.get_scenario().as_str(), "Read only" | "Unsupported"),
        ..Default::default()
    };
    let numeric = match name {
        "x" => Some(n.view.x),
        "y" => Some(n.view.y),
        "width" => Some(n.view.width),
        "height" => Some(n.view.height),
        "transform-rotation" => Some(n.view.rotation),
        "border-radius" => Some(n.view.radius),
        "opacity" => Some(1.),
        "font-size" => Some(22.),
        _ if name.ends_with("radius") => Some(n.view.radius),
        _ => None,
    };
    if let Some(value) = numeric {
        p.kind = PropertyValueKind::Float;
        p.value_kind = p.kind;
        p.value_float = value;
        p.code = format!(
            "{value}{}",
            if name == "transform-rotation" {
                "deg"
            } else if name == "opacity" {
                ""
            } else {
                "px"
            }
        )
        .into();
    } else if matches!(name, "background" | "color") || name.ends_with("color") {
        p.kind = PropertyValueKind::Brush;
        p.value_kind = p.kind;
        p.fill = n.view.fill.clone();
        p.value_brush = brushes::fill_brush(p.fill.clone());
        p.brush_kind = p.fill.kind;
        p.gradient_stops = p.fill.stops.clone();
        p.code = if p.value_resolved {
            brushes::fill_expression(p.fill.clone())
        } else {
            "Theme.custom-fill".into()
        };
    } else {
        p.kind = PropertyValueKind::String;
        p.value_kind = p.kind;
        p.code = n
            .properties
            .get(name)
            .cloned()
            .unwrap_or_else(|| match name {
                "text" => format!("{:?}", n.view.text.as_str()),
                "image-fit" => "contain".into(),
                "font-weight" => "400".into(),
                "source" => "@image-url(\"checker.svg\")".into(),
                _ => String::new(),
            })
            .into();
        p.value_string = if name == "text" { n.view.text.clone() } else { p.code.clone() };
    }
    p.display_string = p.code.clone();
    p
}

fn accepted(window: &GalleryWindow) -> bool {
    let g = window.global::<Gallery>();
    !g.get_disabled()
        && !g.get_reject_edits()
        && !matches!(g.get_scenario().as_str(), "Read only" | "Unsupported")
}

fn apply_number(n: &mut Node, name: &str, value: f32) -> bool {
    if !value.is_finite() || (matches!(name, "width" | "height") && value <= 0.) {
        return false;
    }
    match name {
        "x" => n.view.x = value,
        "y" => n.view.y = value,
        "width" => n.view.width = value,
        "height" => n.view.height = value,
        "transform-rotation" => n.view.rotation = value,
        "all-corners" | "border-radius" => n.view.radius = value.max(0.),
        name if name.ends_with("radius") => n.view.radius = value.max(0.),
        _ => {
            n.properties.insert(name.into(), value.to_string());
        }
    }
    true
}

fn apply_code(n: &mut Node, name: &str, value: &str) -> bool {
    if let Ok(number) = value.trim().trim_end_matches("px").trim_end_matches("deg").parse::<f32>() {
        return apply_number(n, name, number);
    }
    if name == "background" || name == "color" {
        let Some(color) = brushes::string_to_color(value) else { return false };
        n.view.fill.kind = BrushKind::Solid;
        n.view.fill.color = color;
    } else if name == "text" {
        let Ok(text) = serde_json::from_str::<String>(value) else { return false };
        n.view.text = text.into();
    } else {
        n.properties.insert(name.into(), value.into());
    }
    true
}

fn edit_number(
    window: &GalleryWindow,
    state: &State,
    name: &str,
    value: f32,
    commit: bool,
) -> bool {
    if !accepted(window) {
        return false;
    }
    {
        let mut s = state.borrow_mut();
        let Some(n) = s.selected_mut() else { return false };
        if !apply_number(n, name, value) {
            return false;
        }
        if commit {
            s.commit();
        }
    }
    sync(window, state);
    if commit {
        record(window, format!("Committed {name}: {value}"));
    }
    true
}

fn edit_codes(window: &GalleryWindow, state: &State, bindings: &[(String, String)]) -> bool {
    if !accepted(window) {
        record(window, "Rejected property edit");
        return false;
    }
    {
        let mut s = state.borrow_mut();
        let Some(n) = s.selected_mut() else { return false };
        let mut candidate = n.clone();
        if !bindings.iter().all(|(name, value)| apply_code(&mut candidate, name, value)) {
            return false;
        }
        *n = candidate;
        s.commit();
    }
    sync(window, state);
    record(window, format!("Committed {} properties", bindings.len()));
    true
}

fn payload(data: &DataTransfer) -> Option<(Option<i32>, ElementKind)> {
    let text = data.plain_text().ok()?;
    if let Some(id) = text.strip_prefix("gallery:move:").and_then(|s| s.parse::<i32>().ok()) {
        return Some((Some(id), ElementKind::None));
    }
    let kind = element_catalog::kind_for_type(text.strip_prefix("gallery:new:")?);
    element_catalog::primitive(kind).map(|_| (None, kind))
}

pub fn navigate(window: &GalleryWindow, state: &State, id: &str, scenario: &str) {
    let Some(page) = catalog::page(id) else { return };
    window.invoke_clear_transients();
    window.set_mounted(false);
    state.borrow_mut().reset(scenario);
    let g = window.global::<Gallery>();
    g.set_page(id.into());
    g.set_title(page.title.into());
    g.set_description(page.description.into());
    g.set_scenarios(model(page.scenarios.iter().map(|s| (*s).into()).collect()));
    g.set_scenario(scenario.into());
    g.set_scenario_index(page.scenarios.iter().position(|s| *s == scenario).unwrap_or(0) as i32);
    g.set_disabled(scenario == "Disabled");
    g.set_reject_edits(scenario == "Rejected edits");
    g.set_value(24.);
    g.set_text(
        if scenario == "Long names" {
            "An intentionally long component label for checking truncation and editing"
        } else {
            "Hello Slint"
        }
        .into(),
    );
    g.set_events(Default::default());
    let api = window.global::<Api>();
    api.set_editor_surface_mode(if scenario == "Unavailable" {
        EditorSurfaceMode::Image
    } else {
        EditorSurfaceMode::Component
    });
    api.set_known_components(if scenario == "Dragging disabled" {
        Default::default()
    } else {
        model(vec![ComponentListItem { category: "Gallery".into(), ..Default::default() }])
    });
    api.set_inspector_generation(api.get_inspector_generation().wrapping_add(1));
    api.set_inspector_fill_generation(api.get_inspector_fill_generation().wrapping_add(1));
    api.set_inspector_fill_refresh_pending(false);
    api.set_drop_mark(DropMark { x1: -1., y1: -1., x2: -1., y2: -1. });
    api.set_recent_fills(Default::default());
    api.set_image_nine_slice_top(8);
    api.set_image_nine_slice_right(8);
    api.set_image_nine_slice_bottom(8);
    api.set_image_nine_slice_left(8);
    api.set_selected_image_asset(ImageAssetPreview {
        path: if scenario == "Raster" { "gallery/thumbsup.png" } else { "gallery/checker.svg" }
            .into(),
        relative_path: if scenario == "Raster" {
            "assets/thumbsup.png"
        } else {
            "assets/checker.svg"
        }
        .into(),
        format: if scenario == "Raster" { "PNG" } else { "SVG" }.into(),
        image: match scenario {
            "Missing image" => Default::default(),
            "Raster" => g.get_sample_raster(),
            _ => g.get_sample_image(),
        },
        error: if scenario == "Missing image" {
            "This fixture simulates an unavailable image.".into()
        } else {
            "".into()
        },
    });
    api.set_update_state(match scenario {
        "Update available" => UpdateState::Available,
        "Downloading" => UpdateState::Downloading,
        "Ready to install" => UpdateState::ReadyToInstall,
        "Installing" => UpdateState::Installing,
        "Restart required" => UpdateState::RestartRequired,
        "Update failed" => UpdateState::Error,
        _ => UpdateState::UpToDate,
    });
    api.set_update_download_progress(0.45);
    api.set_update_error("Simulated connection failure".into());
    api.set_update_version("1.20".into());
    let project = window.global::<Project>();
    project.set_selected_project_file("Gallery / Main.slint".into());
    project.set_file_tree(model(if scenario == "Empty" {
        vec![]
    } else {
        vec![
            FileTreeNode {
                label: "gallery".into(),
                path: "gallery".into(),
                has_children: true,
                is_expanded: scenario != "Collapsed",
                kind: FileTreeNodeKind::Folder,
                ..Default::default()
            },
            FileTreeNode {
                label: if scenario == "Long names" {
                    "A-very-long-component-file-name.slint"
                } else {
                    "Main.slint"
                }
                .into(),
                path: "gallery/Main.slint".into(),
                parent_path: "gallery".into(),
                indent_level: 1,
                is_slint_file: true,
                kind: FileTreeNodeKind::File,
                rename_selection_end: 4,
                ..Default::default()
            },
            FileTreeNode {
                label: "checker.svg".into(),
                path: "gallery/checker.svg".into(),
                parent_path: "gallery".into(),
                indent_level: 1,
                kind: FileTreeNodeKind::Image,
                ..Default::default()
            },
        ]
    }));
    state.borrow_mut().files = project.get_file_tree().iter().collect();
    sync_files(window, &state.borrow().files);
    project.set_recent(model(if scenario == "Empty" {
        vec![]
    } else {
        vec![RecentProject {
            name: "Component playground".into(),
            component: "Main".into(),
            root_path: "gallery".into(),
            path: "gallery/Main.slint".into(),
        }]
    }));
    window.global::<Preview>().set_can_run(true);
    window.global::<Preview>().set_is_running(false);
    sync(window, state);
    record(window, format!("Loaded {} / {scenario}", page.title));
    let weak = window.as_weak();
    slint::Timer::single_shot(std::time::Duration::ZERO, move || {
        if let Some(w) = weak.upgrade() {
            w.set_mounted(true);
        }
    });
}

pub fn install(window: &GalleryWindow) -> State {
    let state = Rc::new(RefCell::new(Scene::default()));
    let api = window.global::<Api>();
    let gallery = window.global::<Gallery>();
    gallery.on_matches(|text, query| {
        text.to_lowercase().contains(query.trim().to_lowercase().as_str())
    });
    gallery.set_pages(model(
        catalog::PAGES
            .iter()
            .map(|p| GalleryPage {
                id: p.id.into(),
                category: p.category.into(),
                title: p.title.into(),
                description: p.description.into(),
            })
            .collect(),
    ));
    slint_editor::component_support::cursors::setup(&window.global::<EditorCursors>());
    brushes::setup(&api);
    element_library::setup(&api);
    recent_fills::setup(&api, <Api as slint::Global<'_, GalleryWindow>>::as_weak(&api));
    macro_rules! wire {
        ($owner:ident, $callback:ident, |$w:ident, $s:ident $(, $arg:ident)*| $body:block) => {{
            let weak = window.as_weak(); let state = state.clone();
            $owner.$callback(move |$($arg),*| {
                let Some($w) = weak.upgrade() else { return Default::default() };
                let $s = &state;
                $body
            });
        }};
    }
    wire!(gallery, on_record, |w, _s, message| {
        record(&w, message);
    });
    wire!(gallery, on_navigate, |w, s, page, scenario| {
        navigate(&w, s, &page, &scenario);
    });
    wire!(gallery, on_reset, |w, s| {
        let g = w.global::<Gallery>();
        navigate(&w, s, &g.get_page(), &g.get_scenario());
    });
    wire!(gallery, on_select, |w, s, id| {
        w.invoke_clear_transients();
        s.borrow_mut().cancel();
        s.borrow_mut().selected = id;
        sync(&w, s);
    });
    wire!(gallery, on_open_picker, |w, s, anchor| {
        let Some(n) = s.borrow().selected().map(|n| n.view.clone()) else { return };
        let api = w.global::<Api>();
        w.global::<FillSession>().invoke_begin(FillSessionRequest {
            fill: n.fill.clone(),
            target: FillSessionTarget {
                key: format!("gallery:1:{}:0:{}", n.id, api.get_inspector_generation()).into(),
                session_key: format!(
                    "gallery:{}:0:{}:background",
                    n.id,
                    api.get_inspector_fill_generation()
                )
                .into(),
                property_name: "background".into(),
                size: slint::LogicalSize::new(n.width, n.height),
                canvas: n.kind == ElementKind::Rectangle,
            },
            expression: brushes::fill_expression(n.fill.clone()),
            unsupported: matches!(
                w.global::<Gallery>().get_scenario().as_str(),
                "Unsupported" | "Read only"
            ),
            allow_gradient: true,
            anchor_position: anchor,
            anchor_width: 24.,
        });
        w.global::<FillSession>().set_selection(geometry(&n));
    });
    wire!(api, on_current_property_value_data, |w, s, name| { property(&w, s, &name) });
    wire!(api, on_current_property_value, |w, s, name, fallback| {
        let p = property(&w, s, &name);
        if p.code.is_empty() { fallback } else { p.code }
    });
    wire!(api, on_highlight_positions, |w, _s, _uri, id| {
        let g = w.global::<Gallery>();
        if id == g.get_selected() {
            return g.get_highlights();
        }
        let _ = g.get_generation();
        model(g.get_nodes().iter().filter(|n| n.id == id).map(|n| geometry(&n)).collect())
    });
    wire!(api, on_inspector_values, |w, _s, _key| {
        let g = w.global::<Gallery>();
        let _ = g.get_generation();
        let n = g.get_nodes().iter().find(|n| n.id == g.get_selected()).unwrap_or_default();
        model(vec![n.rotation, n.radius, n.radius, n.radius, n.radius])
    });
    wire!(api, on_inspector_preview, |w, s, _key, name, value| {
        edit_number(&w, s, &name, value, false)
    });
    wire!(api, on_inspector_commit, |w, s, _key, name, value| {
        edit_number(&w, s, &name, value, true)
    });
    wire!(api, on_inspector_cancel, |w, s| {
        s.borrow_mut().cancel();
        sync(&w, s);
        record(&w, "Canceled edit");
    });
    wire!(api, on_inspector_fill_preview, |w, s, _key, _name, fill| {
        if !accepted(&w) {
            record(&w, "Rejected fill preview");
            return false;
        }
        {
            let mut scene = s.borrow_mut();
            let Some(n) = scene.selected_mut() else { return false };
            n.view.fill = fill;
        }
        sync(&w, s);
        true
    });
    wire!(api, on_inspector_fill_commit, |w, s, _key, _name, fill| {
        if !accepted(&w) {
            return false;
        }
        {
            let mut scene = s.borrow_mut();
            let Some(n) = scene.selected_mut() else { return false };
            n.view.fill = fill;
            scene.commit();
        }
        sync(&w, s);
        record(&w, "Committed fill");
        true
    });
    wire!(api, on_undo, |w, s| {
        s.borrow_mut().undo(false);
        sync(&w, s);
        record(&w, "Undo");
    });
    wire!(api, on_redo, |w, s| {
        s.borrow_mut().undo(true);
        sync(&w, s);
        record(&w, "Redo");
    });
    wire!(api, on_set_code_binding, |w, s, _uri, _version, _offset, name, value| {
        edit_codes(&w, s, &[(name.to_string(), value.to_string())])
    });
    wire!(api, on_test_code_binding, |w, _s, _uri, _version, _offset, _name, value| {
        accepted(&w) && !value.trim().is_empty()
    });
    wire!(api, on_set_code_bindings, |w, s, _uri, _version, _offset, bindings| {
        edit_codes(
            &w,
            s,
            &bindings.iter().map(|b| (b.name.to_string(), b.value.to_string())).collect::<Vec<_>>(),
        )
    });
    wire!(api, on_set_element_id, |w, s, _uri, _version, _offset, id| {
        if accepted(&w) {
            if let Some(n) = s.borrow_mut().selected_mut() {
                n.view.label = id;
            }
            s.borrow_mut().commit();
            sync(&w, s);
        }
    });
    wire!(api, on_outline_select_element, |w, s, _uri, id, _notify| {
        w.invoke_clear_transients();
        s.borrow_mut().cancel();
        s.borrow_mut().selected = id;
        sync(&w, s);
    });
    wire!(api, on_outline_set_expanded, |w, s, row, expanded| {
        let id = w.global::<Api>().get_outline().row_data(row as usize).map(|n| n.offset);
        if let Some(n) = s.borrow_mut().nodes.iter_mut().find(|n| Some(n.view.id) == id) {
            n.expanded = expanded;
        }
        sync(&w, s);
    });
    api.on_new_component_data_for_kind(|kind| {
        DataTransfer::from(SharedString::from(format!("gallery:new:{}", type_name(kind))))
    });
    api.on_move_element_instance_data(|_, id| {
        DataTransfer::from(SharedString::from(format!("gallery:move:{id}")))
    });
    wire!(api, on_outline_can_drop, |w, s, data, _uri, target, location| {
        accepted(&w)
            && payload(&data)
                .is_some_and(|(source, _)| s.borrow().can_drop(source, target, location))
    });
    wire!(api, on_outline_drop, |w, s, data, _uri, target, location| {
        if !accepted(&w) {
            return;
        }
        if let Some((source, kind)) = payload(&data) {
            let changed = s.borrow_mut().drop_node(source, kind, target, location);
            if changed {
                sync(&w, s);
                record(&w, "Dropped element");
            }
        }
    });
    wire!(api, on_can_drop, |w, s, data, _x, _y, _on_area| {
        accepted(&w)
            && payload(&data)
                .is_some_and(|(source, _)| s.borrow().can_drop(source, 1, DropLocation::Onto))
    });
    wire!(api, on_drop, |w, s, data, x, y| {
        drop_on_canvas(&w, s, data, x, y, None);
    });
    wire!(api, on_drop_with_geometry, |w, s, data, _hit, position, size| {
        drop_on_canvas(&w, s, data, position.x, position.y, Some(size));
    });
    wire!(api, on_selected_element_resize, |w, s, x, y, width, height| {
        if !accepted(&w) {
            return;
        }
        {
            let mut scene = s.borrow_mut();
            if let Some(n) = scene.selected_mut() {
                n.view.x = x;
                n.view.y = y;
                n.view.width = width;
                n.view.height = height;
            }
            scene.commit();
        }
        sync(&w, s);
    });
    wire!(api, on_override_selected_element_geometry, |w, s, x, y, width, height| {
        if !accepted(&w) {
            return;
        }
        {
            let mut scene = s.borrow_mut();
            if let Some(n) = scene.selected_mut() {
                n.view.x = x;
                n.view.y = y;
                n.view.width = width;
                n.view.height = height;
            }
        }
        sync(&w, s);
    });
    wire!(api, on_persist_selected_element_geometry, |w, s| {
        s.borrow_mut().commit();
        sync(&w, s);
        record(&w, "Committed geometry");
    });
    wire!(api, on_override_selected_element_rotation, |w, s, value| {
        edit_number(&w, s, "transform-rotation", value, false);
    });
    wire!(api, on_selected_element_rotate, |w, s, value| {
        edit_number(&w, s, "transform-rotation", value, true);
    });
    wire!(api, on_override_selected_element_border_radius, |w, s, _corner, value, _single| {
        edit_number(&w, s, "border-radius", value, false);
    });
    wire!(api, on_persist_selected_element_border_radius, |w, s| {
        s.borrow_mut().commit();
        sync(&w, s);
        record(&w, "Committed radius");
    });
    wire!(api, on_unselect, |w, s| {
        s.borrow_mut().selected = -1;
        sync(&w, s);
    });
    wire!(api, on_select_at, |w, s, x, y, _enter| {
        let id = hit_test(&s.borrow(), x, y).map(|n| n.id).unwrap_or(-1);
        s.borrow_mut().selected = id;
        sync(&w, s);
    });
    wire!(api, on_select_element, |w, s, _uri, id, _x, _y| {
        s.borrow_mut().selected = id;
        sync(&w, s);
    });
    let hover = window.global::<Hover>();
    wire!(hover, on_element_at, |w, s, x, y, _enter| {
        let _ = w.global::<Gallery>().get_generation();
        let scene = s.borrow();
        hit_test(&scene, x, y)
            .map(|n| HoveredElement {
                valid: true,
                is_selected: n.id == scene.selected,
                is_over_selected_element: n.id == scene.selected,
                element_path: "gallery".into(),
                source_uri: "gallery".into(),
                element_offset: n.id,
                type_name: type_name(n.kind).into(),
                geometry: model(vec![geometry(&n)]),
            })
            .unwrap_or_default()
    });
    wire!(api, on_override_element_text, |w, s, id, text| {
        if !accepted(&w) {
            return SharedString::default();
        }
        if id.is_empty() {
            return "gallery-text".into();
        }
        if let Some(n) = s.borrow_mut().selected_mut() {
            n.view.text = text;
        }
        sync(&w, s);
        "gallery-text".into()
    });
    wire!(api, on_file_tree_select, |w, s, path| {
        let mut scene = s.borrow_mut();
        for n in &mut scene.files {
            n.is_selected = n.path == path;
        }
        w.global::<Project>().set_selected_project_file(path.clone());
        sync_files(&w, &scene.files);
        record(&w, format!("Selected {path}"));
    });
    wire!(api, on_file_tree_toggle, |w, s, path| {
        let mut scene = s.borrow_mut();
        if let Some(n) = scene.files.iter_mut().find(|n| n.path == path) {
            n.is_expanded = !n.is_expanded;
        }
        sync_files(&w, &scene.files);
        record(&w, "Toggled folder");
    });
    wire!(api, on_file_tree_rename, |w, s, path, name| {
        if name.trim().is_empty()
            || name.contains(['/', '\\'])
            || matches!(name.as_str(), "." | "..")
        {
            return SharedString::from("Enter a file name without slashes.");
        }
        let mut scene = s.borrow_mut();
        if scene.files.iter().any(|n| n.label == name && n.path != path) {
            return "This file name already exists.".into();
        }
        if let Some(n) = scene.files.iter_mut().find(|n| n.path == path) {
            if n.kind == FileTreeNodeKind::Folder {
                return "Choose a file to rename.".into();
            }
            n.label = name.clone();
            n.path = format!("{}/{}", n.parent_path, name).into();
        }
        sync_files(&w, &scene.files);
        record(&w, format!("Renamed fixture to {name}"));
        SharedString::default()
    });
    api.on_image_nine_slice_expression(
        slint_editor::component_support::image::format_nine_slice_expression,
    );
    api.on_image_nine_slice_preview(
        slint_editor::component_support::image::nine_slice_preview_image,
    );
    wire!(api, on_copy_nine_slice_expression, |w, _s, value| {
        record(&w, format!("Generated expression: {value}"));
    });
    api.on_string_is_single_line(|s| !s.contains(['\r', '\n']));
    api.on_string_to_code(|s, _, _, _, _| {
        serde_json::to_string(s.as_str()).unwrap_or_default().into()
    });
    wire!(api, on_check_for_update, |w, _s| {
        record(&w, "Simulated update action");
    });
    let project = window.global::<Project>();
    wire!(project, on_create_new_project, |w, _s| {
        record(&w, "Simulated create project");
        true
    });
    wire!(project, on_open_existing_project, |w, _s| {
        record(&w, "Simulated open project");
        true
    });
    wire!(project, on_open_recent_project, |w, _s, p| {
        record(&w, format!("Simulated open {}", p.name));
        true
    });
    wire!(project, on_create_new_slint_file, |w, _s| {
        record(&w, "Simulated new Slint file");
        SharedString::default()
    });
    let preview = window.global::<Preview>();
    wire!(preview, on_run, |w, _s| {
        let p = w.global::<Preview>();
        p.set_is_running(!p.get_is_running());
        record(&w, "Simulated run toggle");
    });
    state
}

fn sync_files(window: &GalleryWindow, files: &[FileTreeNode]) {
    window.global::<Project>().set_file_tree(update_model(
        window.global::<Project>().get_file_tree(),
        files
            .iter()
            .filter(|n| {
                n.parent_path.is_empty()
                    || files.iter().any(|p| p.path == n.parent_path && p.is_expanded)
            })
            .cloned()
            .collect(),
    ));
}

fn hit_test(scene: &Scene, x: f32, y: f32) -> Option<GalleryNode> {
    scene
        .nodes
        .iter()
        .rev()
        .filter(|n| n.parent.is_some())
        .find(|n| {
            let n = &n.view;
            let angle = (-n.rotation).to_radians();
            let dx = x - n.x - n.width / 2.;
            let dy = y - n.y - n.height / 2.;
            let px = dx * angle.cos() - dy * angle.sin() + n.width / 2.;
            let py = dx * angle.sin() + dy * angle.cos() + n.height / 2.;
            px >= 0. && py >= 0. && px < n.width && py < n.height
        })
        .map(|n| n.view.clone())
}

fn drop_on_canvas(
    window: &GalleryWindow,
    state: &State,
    data: DataTransfer,
    x: f32,
    y: f32,
    size: Option<slint::LogicalSize>,
) {
    if !accepted(window) {
        return;
    }
    if let Some((source, kind)) = payload(&data) {
        let mut scene = state.borrow_mut();
        if !scene.drop_node(source, kind, 1, DropLocation::Onto) {
            return;
        }
        if let Some(n) = scene.selected_mut() {
            n.view.x = x;
            n.view.y = y;
            if let Some(size) = size {
                n.view.width = size.width;
                n.view.height = size.height;
            }
        }
        scene.refresh_committed();
        drop(scene);
        sync(window, state);
        record(window, "Dropped element");
    }
}
