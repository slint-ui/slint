// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::{collections::HashMap, iter::once, rc::Rc};

use i_slint_compiler::parser::TextRange;
use i_slint_compiler::{expression_tree, langtype};

use itertools::Itertools;
use slint::{Model, ModelRc, SharedString, ToSharedString, VecModel};
use slint_interpreter::{DiagnosticLevel, PlatformError};
use smol_str::SmolStr;

use crate::common::{self, ComponentInformation};
use crate::preview::{self, preview_data, properties, SelectionNotification};

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

mod brushes;
pub mod log_messages;
pub mod palette;
mod property_view;
mod recent_colors;

slint::include_modules!();

pub type PropertyDeclarations = HashMap<SmolStr, PropertyDeclaration>;

pub fn create_ui(
    to_lsp: &Rc<dyn common::PreviewToLsp>,
    style: &str,
    experimental: bool,
) -> Result<PreviewUi, PlatformError> {
    #[cfg(all(target_vendor = "apple", not(target_arch = "wasm32")))]
    crate::preview::connector::native::init_apple_platform()?;

    let ui = PreviewUi::new()?;

    // styles:
    let known_styles = once(&"native")
        .chain(i_slint_compiler::fileaccess::styles().iter())
        .filter(|s| s != &&"qt" || i_slint_backend_selector::HAS_NATIVE_STYLE)
        .cloned()
        .sorted()
        .collect::<Vec<_>>();
    let style = if known_styles.contains(&style) {
        style.to_string()
    } else {
        known_styles
            .iter()
            .find(|x| **x == "native")
            .or_else(|| known_styles.first())
            .map(|s| s.to_string())
            .unwrap_or_default()
    };

    let style_model = Rc::new({
        let model = VecModel::default();
        model.extend(known_styles.iter().map(|s| SharedString::from(*s)));
        assert!(model.row_count() > 1);
        model
    });

    let api = ui.global::<Api>();

    api.set_current_style(style.clone().into());
    api.set_experimental(experimental);
    api.set_known_styles(style_model.into());

    api.on_add_new_component(super::add_new_component);
    api.on_rename_component(super::rename_component);
    api.on_style_changed(super::change_style);
    api.on_show_component(super::show_component);

    let lsp = to_lsp.clone();
    api.on_show_document(move |file, line, column| {
        use lsp_types::{Position, Range};
        let pos = Position::new((line as u32).saturating_sub(1), (column as u32).saturating_sub(1));
        lsp.ask_editor_to_show_document(&file, Range::new(pos, pos), false).unwrap();
    });
    api.on_show_document_offset_range(super::show_document_offset_range);
    api.on_show_preview_for(super::show_preview_for);
    api.on_reload_preview(super::reload_preview);
    api.on_unselect(super::element_selection::unselect_element);
    api.on_reselect(super::element_selection::reselect_element);
    api.on_select_at(super::element_selection::select_element_at);
    api.on_selection_stack_at(super::element_selection::selection_stack_at);
    api.on_filter_sort_selection_stack(super::element_selection::filter_sort_selection_stack);
    api.on_find_selected_selection_stack_frame(|stack| {
        stack.iter().find(|frame| frame.is_selected).unwrap_or_default()
    });
    api.on_select_element(|path, offset, x, y| {
        super::element_selection::select_element_at_source_code_position(
            PathBuf::from(path.to_string()),
            preview::TextSize::from(offset as u32),
            Some(i_slint_core::lengths::LogicalPoint::new(x, y)),
            SelectionNotification::Now,
        );
    });
    api.on_select_behind(super::element_selection::select_element_behind);
    api.on_highlight_positions(super::element_selection::highlight_positions);
    let lsp = to_lsp.clone();
    api.on_can_drop(super::can_drop_component);
    api.on_drop(move |component_index: i32, x: f32, y: f32| {
        lsp.send_telemetry(&mut [(
            "type".to_string(),
            serde_json::to_value("component_dropped").unwrap(),
        )])
        .unwrap();
        super::drop_component(component_index, x, y)
    });
    api.on_selected_element_resize(super::resize_selected_element);
    api.on_selected_element_can_move_to(super::can_move_selected_element);
    api.on_selected_element_move(super::move_selected_element);
    api.on_selected_element_delete(super::delete_selected_element);

    api.on_test_code_binding(super::test_code_binding);
    api.on_set_code_binding(super::set_code_binding);
    api.on_set_color_binding(super::set_color_binding);
    api.on_property_declaration_ranges(super::property_declaration_ranges);

    api.on_get_property_value(get_property_value);
    api.on_get_property_value_table(get_property_value_table);
    api.on_set_property_value_table(set_property_value_table);
    api.on_insert_row_into_value_table(insert_row_into_value_table);
    api.on_remove_row_from_value_table(remove_row_from_value_table);

    let lsp = to_lsp.clone();
    api.on_set_json_preview_data(move |container, property_name, json_string, send_telemetry| {
        if send_telemetry {
            lsp.send_telemetry(&mut [(
                "type".to_string(),
                serde_json::to_value("data_json_changed").unwrap(),
            )])
            .unwrap();
        }
        set_json_preview_data(container, property_name, json_string)
    });

    api.on_string_to_code(string_to_code);

    brushes::setup(&ui);
    log_messages::setup(&ui);
    palette::setup(&ui);
    recent_colors::setup(&ui);
    super::outline::setup(&ui);
    super::undo_redo::setup(&ui);

    #[cfg(target_vendor = "apple")]
    api.set_control_key_name("command".into());

    #[cfg(target_family = "wasm")]
    if web_sys::window()
        .and_then(|window| window.navigator().platform().ok())
        .map_or(false, |platform| platform.to_ascii_lowercase().contains("mac"))
    {
        api.set_control_key_name("command".into());
    }

    Ok(ui)
}

fn extract_definition_location(ci: &ComponentInformation) -> (SharedString, SharedString) {
    let Some(url) = ci.defined_at.as_ref().map(|da| da.url()) else {
        return (Default::default(), Default::default());
    };

    let path = url.to_file_path().unwrap_or_default();
    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

    (url.to_string().into(), file_name.into())
}

pub fn ui_set_uses_widgets(ui: &PreviewUi, uses_widgets: bool) {
    let api = ui.global::<Api>();
    api.set_uses_widgets(uses_widgets);
}

pub fn set_diagnostics(ui: &PreviewUi, diagnostics: &[slint_interpreter::Diagnostic]) {
    let summary = diagnostics
        .iter()
        .inspect(|d| {
            let location = d.source_file().map(|p| {
                let (line, column) = d.line_column();
                (p.to_string_lossy().to_string().into(), line, column)
            });

            let level = match d.level() {
                DiagnosticLevel::Error => LogMessageLevel::Error,
                DiagnosticLevel::Warning => LogMessageLevel::Warning,
                _ => LogMessageLevel::Debug,
            };

            log_messages::append_log_message(ui, level, location, d.message());
        })
        .fold(DiagnosticSummary::NothingDetected, |acc, d| {
            match (acc, d.level()) {
                (_, DiagnosticLevel::Error) => DiagnosticSummary::Errors,
                (DiagnosticSummary::Errors, DiagnosticLevel::Warning) => DiagnosticSummary::Errors,
                (_, DiagnosticLevel::Warning) => DiagnosticSummary::Warnings,
                // DiagnosticLevel is non-exhaustive:
                (acc, _) => acc,
            }
        });

    let api = ui.global::<Api>();
    api.set_diagnostic_summary(summary);
}

pub fn ui_set_known_components(
    ui: &PreviewUi,
    known_components: &[crate::common::ComponentInformation],
    current_component_index: usize,
) {
    let mut builtins_map: HashMap<String, Vec<ComponentItem>> = Default::default();
    let mut std_widgets_map: HashMap<String, Vec<ComponentItem>> = Default::default();
    let mut path_map: HashMap<PathBuf, (SharedString, Vec<ComponentItem>)> = Default::default();
    let mut library_map: HashMap<String, Vec<ComponentItem>> = Default::default();
    let mut longest_path_prefix = PathBuf::new();

    for (idx, ci) in known_components.iter().enumerate() {
        if ci.is_global {
            continue;
        }
        let (url, pretty_location) = extract_definition_location(ci);
        let item = ComponentItem {
            name: ci.name.clone().into(),
            index: idx.try_into().unwrap(),
            defined_at: url.clone(),
            pretty_location,
            is_user_defined: !(ci.is_builtin || ci.is_std_widget),
            is_currently_shown: idx == current_component_index,
            is_exported: ci.is_exported,
        };

        if let Some(position) = &ci.defined_at {
            if let Some(library) = position.url().path().strip_prefix("/@") {
                library_map.entry(format!("@{library}")).or_default().push(item);
            } else {
                let path = i_slint_compiler::pathutils::clean_path(
                    &(position.url().to_file_path().unwrap_or_default()),
                );
                if path != PathBuf::new() {
                    if longest_path_prefix == PathBuf::new() {
                        longest_path_prefix = path.clone();
                    } else {
                        longest_path_prefix =
                            std::iter::zip(longest_path_prefix.components(), path.components())
                                .take_while(|(l, p)| l == p)
                                .map(|(l, _)| l)
                                .collect();
                    }
                }
                path_map.entry(path).or_insert((url, Vec::new())).1.push(item);
            }
        } else if ci.is_builtin {
            builtins_map.entry(ci.category.clone()).or_default().push(item);
        } else {
            std_widgets_map.entry(ci.category.clone()).or_default().push(item);
        }
    }

    fn sort_subset(mut input: HashMap<String, Vec<ComponentItem>>) -> Vec<ComponentListItem> {
        let mut output = input
            .drain()
            .map(|(k, mut v)| {
                v.sort_by_key(|i| i.name.clone());
                let model = Rc::new(VecModel::from(v));
                ComponentListItem {
                    category: k.into(),
                    file_url: SharedString::new(),
                    components: model.into(),
                }
            })
            .collect::<Vec<_>>();
        output.sort_by_key(|k| k.category.clone());
        output
    }

    let builtin_components = sort_subset(builtins_map);
    let std_widgets_components = sort_subset(std_widgets_map);
    let library_components = sort_subset(library_map);
    let mut file_components = path_map
        .drain()
        .map(|(p, (file_url, mut v))| {
            v.sort_by_key(|i| i.name.clone());
            let model = Rc::new(VecModel::from(v));
            let name = if p == longest_path_prefix {
                p.file_name().unwrap_or_default().to_string_lossy().to_string()
            } else {
                p.strip_prefix(&longest_path_prefix).unwrap_or(&p).to_string_lossy().to_string()
            };
            ComponentListItem { category: name.into(), file_url, components: model.into() }
        })
        .collect::<Vec<_>>();
    file_components.sort_by_key(|k| PathBuf::from(k.category.to_string()));

    let mut all_components = Vec::with_capacity(
        builtin_components.len() + library_components.len() + file_components.len(),
    );
    all_components.extend_from_slice(&builtin_components[..]);
    all_components.extend_from_slice(&std_widgets_components[..]);
    all_components.extend_from_slice(&library_components[..]);
    all_components.extend_from_slice(&file_components[..]);

    let result = Rc::new(VecModel::from(all_components));
    let api = ui.global::<Api>();
    api.set_known_components(result.into());
}

fn to_ui_range(r: TextRange) -> Option<Range> {
    Some(Range {
        start: i32::try_from(u32::from(r.start())).ok()?,
        end: i32::try_from(u32::from(r.end())).ok()?,
    })
}

fn convert_simple_string(input: SharedString) -> SharedString {
    slint::format!("\"{}\"", str::escape_debug(input.as_ref()))
}

fn string_to_code(
    input: SharedString,
    is_translatable: bool,
    tr_context: SharedString,
    tr_plural: SharedString,
    tr_plural_expression: SharedString,
) -> SharedString {
    let input = convert_simple_string(input);
    if !is_translatable {
        input
    } else {
        let context = if tr_context.is_empty() {
            SharedString::new()
        } else {
            slint::format!("{} => ", convert_simple_string(tr_context))
        };
        let plural = if tr_plural.is_empty() {
            SharedString::new()
        } else {
            slint::format!(" | {} % {}", convert_simple_string(tr_plural), tr_plural_expression)
        };
        slint::format!("@tr({context}{input}{plural})")
    }
}

fn unit_model(units: &[expression_tree::Unit]) -> ModelRc<SharedString> {
    Rc::new(VecModel::from(
        units.iter().map(|u| u.to_string().into()).collect::<Vec<SharedString>>(),
    ))
    .into()
}

fn is_equal_value(c: &PropertyValue, n: &PropertyValue) -> bool {
    c.code == n.code
}

fn is_equal_property(c: &PropertyInformation, n: &PropertyInformation) -> bool {
    c.name == n.name && c.type_name == n.type_name && is_equal_value(&c.value, &n.value)
}

fn is_equal_element(c: &ElementInformation, n: &ElementInformation) -> bool {
    c.id == n.id
        && c.type_name == n.type_name
        && c.source_uri == n.source_uri
        && c.offset == n.offset
}

pub type PropertyGroupModel = ModelRc<PropertyGroup>;

fn update_grouped_properties(
    cvg: &VecModel<PropertyInformation>,
    nvg: &VecModel<PropertyInformation>,
) {
    enum Op {
        Insert((usize, usize)),
        Copy((usize, usize)),
        PushBack(usize),
        Remove(usize),
    }

    let mut to_do = Vec::new();

    let mut c_it = cvg.iter();
    let mut n_it = nvg.iter();

    let mut cp = c_it.next();
    let mut np = n_it.next();

    let mut c_index = 0_usize;
    let mut n_index = 0_usize;

    loop {
        match (cp.as_ref(), np.as_ref()) {
            (None, None) => break,
            (Some(_), None) => {
                to_do.push(Op::Remove(c_index));
                cp = c_it.next();
            }
            (Some(c), Some(n)) => match c.name.cmp(&n.name) {
                std::cmp::Ordering::Less => {
                    to_do.push(Op::Remove(c_index));
                    cp = c_it.next();
                }
                std::cmp::Ordering::Equal => {
                    if !is_equal_property(c, n) {
                        to_do.push(Op::Copy((c_index, n_index)));
                    }
                    c_index += 1;
                    n_index += 1;
                    cp = c_it.next();
                    np = n_it.next();
                }
                std::cmp::Ordering::Greater => {
                    to_do.push(Op::Insert((c_index, n_index)));
                    c_index += 1;
                    n_index += 1;
                    np = n_it.next();
                }
            },
            (None, Some(_)) => {
                to_do.push(Op::PushBack(n_index));
                n_index += 1;
                np = n_it.next();
            }
        }
    }

    for op in &to_do {
        match op {
            Op::Insert((c, n)) => {
                cvg.insert(*c, nvg.row_data(*n).unwrap());
            }
            Op::Copy((c, n)) => {
                cvg.set_row_data(*c, nvg.row_data(*n).unwrap());
            }
            Op::PushBack(n) => {
                cvg.push(nvg.row_data(*n).unwrap());
            }
            Op::Remove(c) => {
                cvg.remove(*c);
            }
        }
    }
}

fn get_value<T: Sized + std::convert::TryFrom<slint_interpreter::Value> + std::default::Default>(
    v: &Option<slint_interpreter::Value>,
) -> T {
    v.clone().and_then(|v| v.try_into().ok()).unwrap_or_default()
}

fn get_code(v: &Option<slint_interpreter::Value>) -> SharedString {
    v.as_ref()
        .and_then(|v| slint_interpreter::json::value_to_json(v).ok())
        .and_then(|j| serde_json::to_string_pretty(&j).ok())
        .unwrap_or_default()
        .into()
}

#[derive(Default, Debug)]
struct ValueMapping {
    name_prefix: SharedString,
    is_too_complex: bool,
    is_array: bool,
    headers: Vec<SharedString>,
    current_values: Vec<PropertyValue>,
    array_values: Vec<Vec<PropertyValue>>,
    code_value: PropertyValue,
}

fn map_value_and_type(
    ty: &langtype::Type,
    value: &Option<slint_interpreter::Value>,
    mapping: &mut ValueMapping,
) {
    fn map_color(
        mapping: &mut ValueMapping,
        color: slint::Color,
        kind: PropertyValueKind,
        code: SharedString,
    ) {
        let color_string = brushes::color_to_string(color);
        mapping.headers.push(mapping.name_prefix.clone());
        mapping.current_values.push(PropertyValue {
            value_kind: kind,
            kind,
            display_string: color_string.clone(),
            brush_kind: BrushKind::Solid,
            value_brush: slint::Brush::SolidColor(color),
            gradient_stops: Rc::new(VecModel::from(vec![GradientStop { color, position: 0.5 }]))
                .into(),
            code,
            accessor_path: mapping.name_prefix.clone(),
            ..Default::default()
        });
    }
    use i_slint_compiler::expression_tree::Unit;
    use langtype::Type;

    match ty {
        Type::Float32 => {
            mapping.headers.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                display_string: get_value::<f32>(value).to_shared_string(),
                kind: PropertyValueKind::Float,
                value_float: get_value::<f32>(value),
                code: get_code(value),
                accessor_path: mapping.name_prefix.clone(),
                ..Default::default()
            });
        }

        Type::Int32 => {
            mapping.headers.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                display_string: get_value::<i32>(value).to_shared_string(),
                kind: PropertyValueKind::Integer,
                value_kind: PropertyValueKind::Integer,
                value_int: get_value::<i32>(value),
                code: get_code(value),
                accessor_path: mapping.name_prefix.clone(),
                ..Default::default()
            });
        }
        Type::Duration => {
            mapping.headers.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                display_string: slint::format!("{}{}", get_value::<f32>(value), Unit::Ms),
                kind: PropertyValueKind::Float,
                value_kind: PropertyValueKind::Float,
                value_float: get_value::<f32>(value),
                visual_items: unit_model(&[Unit::S, Unit::Ms]),
                value_int: 1,
                code: get_code(value),
                default_selection: 1,
                accessor_path: mapping.name_prefix.clone(),
                ..Default::default()
            });
        }
        Type::PhysicalLength => {
            mapping.headers.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                display_string: slint::format!("{}{}", get_value::<f32>(value), Unit::Phx),
                kind: PropertyValueKind::Float,
                value_kind: PropertyValueKind::Float,
                value_float: get_value::<f32>(value),
                visual_items: unit_model(&[
                    Unit::Px,
                    Unit::Cm,
                    Unit::Mm,
                    Unit::In,
                    Unit::Pt,
                    Unit::Phx,
                    Unit::Rem,
                ]),
                value_int: 5,
                code: get_code(value),
                default_selection: 5,
                accessor_path: mapping.name_prefix.clone(),
                ..Default::default()
            });
        }
        Type::LogicalLength => {
            mapping.headers.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                display_string: slint::format!("{}{}", get_value::<f32>(value), Unit::Px),
                kind: PropertyValueKind::Float,
                value_kind: PropertyValueKind::Float,
                value_float: get_value::<f32>(value),
                visual_items: unit_model(&[
                    Unit::Px,
                    Unit::Cm,
                    Unit::Mm,
                    Unit::In,
                    Unit::Pt,
                    Unit::Phx,
                    Unit::Rem,
                ]),
                value_int: 0,
                code: get_code(value),
                default_selection: 0,
                accessor_path: mapping.name_prefix.clone(),
                ..Default::default()
            });
        }
        Type::Rem => {
            mapping.headers.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                display_string: slint::format!("{}{}", get_value::<f32>(value), Unit::Rem),
                kind: PropertyValueKind::Float,
                value_kind: PropertyValueKind::Float,
                value_float: get_value::<f32>(value),
                visual_items: unit_model(&[
                    Unit::Px,
                    Unit::Cm,
                    Unit::Mm,
                    Unit::In,
                    Unit::Pt,
                    Unit::Phx,
                    Unit::Rem,
                ]),
                value_int: 6,
                code: get_code(value),
                default_selection: 6,
                accessor_path: mapping.name_prefix.clone(),
                ..Default::default()
            });
        }
        Type::Angle => {
            mapping.headers.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                display_string: slint::format!("{}{}", get_value::<f32>(value), Unit::Deg),
                kind: PropertyValueKind::Float,
                value_kind: PropertyValueKind::Float,
                value_float: get_value::<f32>(value),
                visual_items: unit_model(&[Unit::Deg, Unit::Grad, Unit::Turn, Unit::Rad]),
                value_int: 0,
                code: get_code(value),
                default_selection: 0,
                accessor_path: mapping.name_prefix.clone(),
                ..Default::default()
            });
        }
        Type::Percent => {
            mapping.headers.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                display_string: slint::format!("{}{}", get_value::<f32>(value), Unit::Percent),
                kind: PropertyValueKind::Float,
                value_kind: PropertyValueKind::Float,
                value_float: get_value::<f32>(value),
                visual_items: unit_model(&[Unit::Percent]),
                value_int: 0,
                code: get_code(value),
                default_selection: 0,
                accessor_path: mapping.name_prefix.clone(),
                ..Default::default()
            });
        }
        Type::String => {
            mapping.headers.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                display_string: slint::format!("\"{}\"", get_value::<SharedString>(value)),
                kind: PropertyValueKind::String,
                value_kind: PropertyValueKind::String,
                value_string: get_value::<SharedString>(value),
                code: get_code(value),
                accessor_path: mapping.name_prefix.clone(),
                ..Default::default()
            });
        }
        Type::Color => {
            map_color(
                mapping,
                get_value::<slint::Color>(value),
                PropertyValueKind::Color,
                get_code(value),
            );
        }
        Type::Brush => {
            let brush = get_value::<slint::Brush>(value);
            match brush {
                slint::Brush::SolidColor(c) => {
                    map_color(mapping, c, PropertyValueKind::Brush, get_code(value))
                }
                slint::Brush::LinearGradient(lg) => {
                    mapping.headers.push(mapping.name_prefix.clone());
                    mapping.current_values.push(PropertyValue {
                        display_string: SharedString::from("Linear Gradient"),
                        kind: PropertyValueKind::Brush,
                        value_kind: PropertyValueKind::Brush,
                        brush_kind: BrushKind::Linear,
                        value_float: lg.angle(),
                        value_brush: slint::Brush::LinearGradient(lg.clone()),
                        gradient_stops: Rc::new(VecModel::from(
                            lg.stops()
                                .map(|gs| GradientStop { color: gs.color, position: gs.position })
                                .collect::<Vec<_>>(),
                        ))
                        .into(),
                        accessor_path: mapping.name_prefix.clone(),
                        code: get_code(value),
                        ..Default::default()
                    });
                }
                slint::Brush::RadialGradient(rg) => {
                    mapping.headers.push(mapping.name_prefix.clone());
                    mapping.current_values.push(PropertyValue {
                        display_string: SharedString::from("Radial Gradient"),
                        kind: PropertyValueKind::Brush,
                        value_kind: PropertyValueKind::Brush,
                        brush_kind: BrushKind::Radial,
                        value_brush: slint::Brush::RadialGradient(rg.clone()),
                        gradient_stops: Rc::new(VecModel::from(
                            rg.stops()
                                .map(|gs| GradientStop { color: gs.color, position: gs.position })
                                .collect::<Vec<_>>(),
                        ))
                        .into(),
                        accessor_path: mapping.name_prefix.clone(),
                        code: get_code(value),
                        ..Default::default()
                    });
                }
                _ => {
                    mapping.headers.push(mapping.name_prefix.clone());
                    mapping.current_values.push(PropertyValue {
                        display_string: SharedString::from("Unknown Brush"),
                        kind: PropertyValueKind::Code,
                        value_kind: PropertyValueKind::Code,
                        value_string: SharedString::from("???"),
                        accessor_path: mapping.name_prefix.clone(),
                        code: get_code(value),
                        ..Default::default()
                    });
                }
            }
        }
        Type::Bool => {
            mapping.headers.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                display_string: get_value::<bool>(value).to_shared_string(),
                kind: PropertyValueKind::Boolean,
                value_kind: PropertyValueKind::Boolean,
                value_bool: get_value::<bool>(value),
                accessor_path: mapping.name_prefix.clone(),
                code: get_code(value),
                ..Default::default()
            });
        }
        Type::Enumeration(enumeration) => {
            let selected_value = match &value {
                Some(slint_interpreter::Value::EnumerationValue(_, k)) => enumeration
                    .values
                    .iter()
                    .position(|v| v.as_str() == k)
                    .unwrap_or(enumeration.default_value),
                _ => enumeration.default_value,
            };

            mapping.headers.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                display_string: slint::format!(
                    "{}.{}",
                    enumeration.name,
                    enumeration.values[selected_value]
                ),
                kind: PropertyValueKind::Enum,
                value_kind: PropertyValueKind::Enum,
                value_string: enumeration.name.as_str().into(),
                default_selection: i32::try_from(enumeration.default_value).unwrap_or_default(),
                value_int: i32::try_from(selected_value).unwrap_or_default(),
                visual_items: Rc::new(VecModel::from(
                    enumeration
                        .values
                        .iter()
                        .map(|s| SharedString::from(s.as_str()))
                        .collect::<Vec<_>>(),
                ))
                .into(),
                accessor_path: mapping.name_prefix.clone(),
                code: get_code(value),
                ..Default::default()
            });
        }
        Type::Array(array_ty) => {
            mapping.is_array = true;
            let model = get_value::<ModelRc<slint_interpreter::Value>>(value);

            for (idx, sub_value) in model.iter().enumerate() {
                let mut sub_mapping =
                    ValueMapping { name_prefix: mapping.name_prefix.clone(), ..Default::default() };
                map_value_and_type(array_ty, &Some(sub_value), &mut sub_mapping);

                let sub_mapping_too_complex = sub_mapping.is_array || sub_mapping.is_too_complex;
                mapping.is_too_complex = mapping.is_too_complex || sub_mapping_too_complex;

                if sub_mapping_too_complex {
                    if idx == 0 {
                        mapping.headers.push(mapping.name_prefix.clone());
                    }
                    mapping.array_values.push(vec![std::mem::take(&mut sub_mapping.code_value)]);
                } else {
                    if idx == 0 {
                        mapping.headers.extend_from_slice(&sub_mapping.headers);
                    }
                    mapping.array_values.push(std::mem::take(&mut sub_mapping.array_values[0]));
                }
            }
        }
        Type::Struct(s) => {
            mapping.is_array = false;

            let struct_data = get_value::<slint_interpreter::Struct>(value);

            for (field, field_ty) in s.fields.iter() {
                let field = field.to_string();
                let mut sub_mapping = ValueMapping::default();
                let header_name = if mapping.name_prefix.is_empty() {
                    field.clone().into()
                } else {
                    slint::format!("{}.{field}", mapping.name_prefix)
                };
                sub_mapping.name_prefix = header_name.clone();

                map_value_and_type(
                    field_ty,
                    &struct_data.get_field(&field).cloned(),
                    &mut sub_mapping,
                );

                let sub_mapping_too_complex = sub_mapping.is_array || sub_mapping.is_too_complex;

                mapping.is_too_complex = mapping.is_too_complex || sub_mapping_too_complex;

                if sub_mapping_too_complex {
                    mapping.headers.push(mapping.name_prefix.clone());
                    mapping.current_values.push(std::mem::take(&mut sub_mapping.code_value));
                } else {
                    mapping.headers.extend_from_slice(&sub_mapping.headers);
                    mapping.current_values.extend_from_slice(&sub_mapping.array_values[0]);
                }
            }
        }
        Type::Image | Type::Model | Type::PathData | Type::Easing | Type::UnitProduct(_) => {
            mapping.headers.push(mapping.name_prefix.clone());
            mapping.is_too_complex = true;
        }
        _ => {
            mapping.headers.push(mapping.name_prefix.clone());
            mapping.current_values.push(PropertyValue {
                display_string: "Unsupported type".into(),
                kind: PropertyValueKind::Code,
                value_kind: PropertyValueKind::Code,
                value_string: "???".into(),
                accessor_path: mapping.name_prefix.clone(),
                code: get_code(value),
                ..Default::default()
            });
        }
    }

    if mapping.array_values.is_empty() {
        mapping.array_values = vec![std::mem::take(&mut mapping.current_values)];
    }

    mapping.code_value = PropertyValue {
        kind: PropertyValueKind::Code,
        value_kind: PropertyValueKind::Code,
        code: get_code(value),
        ..Default::default()
    };
}

pub fn map_value_and_type_to_property_value(
    ty: &langtype::Type,
    value: &Option<slint_interpreter::Value>,
    name_prefix: &str,
) -> PropertyValue {
    let mut mapping =
        ValueMapping { name_prefix: SharedString::from(name_prefix), ..Default::default() };

    map_value_and_type(ty, value, &mut mapping);

    if mapping.is_too_complex
        || mapping.array_values.len() != 1
        || mapping.array_values[0].len() != 1
    {
        mapping.code_value
    } else {
        mapping
            .array_values
            .first()
            .and_then(|av| av.first())
            .cloned()
            .unwrap_or_else(|| mapping.code_value.clone())
    }
}

fn map_preview_data_to_property_value(preview_data: &preview_data::PreviewData) -> PropertyValue {
    map_value_and_type_to_property_value(&preview_data.ty, &preview_data.value, "")
}

fn map_preview_data_property(
    key: &preview_data::PreviewDataKey,
    value: &preview_data::PreviewData,
) -> Option<PreviewData> {
    if !value.is_property() {
        return None;
    };

    let has_getter = value.has_getter();
    let has_setter = value.has_setter();

    let mut mapping = ValueMapping::default();
    map_value_and_type(&value.ty, &value.value, &mut mapping);

    let is_array = mapping.array_values.len() != 1 || mapping.array_values[0].len() != 1;
    let is_too_complex = mapping.is_too_complex;

    Some(PreviewData {
        name: SharedString::from(&key.property_name),
        has_getter,
        has_setter,
        kind: match (is_array, is_too_complex) {
            (false, false) => PreviewDataKind::Value,
            (true, false) => PreviewDataKind::Table,
            _ => PreviewDataKind::Json,
        },
    })
}

pub fn ui_set_preview_data(
    ui: &PreviewUi,
    preview_data: preview_data::PreviewDataMap,
    previewed_component: Option<String>,
) {
    fn create_container(
        container_name: String,
        it: &mut dyn Iterator<Item = (&preview_data::PreviewDataKey, &preview_data::PreviewData)>,
    ) -> Option<PropertyContainer> {
        let (id, props) = it.filter_map(|(k, v)| Some((k, map_preview_data_property(k, v)?))).fold(
            (None, vec![]),
            move |mut acc, (key, value)| {
                acc.0 = Some(acc.0.unwrap_or_else(|| key.container.clone()));
                acc.1.push(value);
                acc
            },
        );
        Some(PropertyContainer {
            container_name: container_name.into(),
            container_id: id?.to_string().into(),
            properties: Rc::new(VecModel::from(props)).into(),
        })
    }

    let mut result: Vec<PropertyContainer> = vec![];

    if let Some(c) = create_container(
        previewed_component.unwrap_or_else(|| "<MAIN>".to_string()),
        &mut preview_data
            .iter()
            .filter(|(k, _)| k.container == preview_data::PropertyContainer::Main),
    ) {
        result.push(c);
    }

    for (k, mut chunk) in &preview_data
        .iter()
        .filter(|(k, _)| k.container != preview_data::PropertyContainer::Main)
        .chunk_by(|(k, _)| k.container.clone())
    {
        if let Some(c) = create_container(k.to_string(), &mut chunk) {
            result.push(c);
        }
    }

    let api = ui.global::<Api>();

    api.set_preview_data(Rc::new(VecModel::from(result)).into());
}

fn to_property_container(container: SharedString) -> preview_data::PropertyContainer {
    if container.is_empty() || container == "<MAIN>" {
        preview_data::PropertyContainer::Main
    } else {
        preview_data::PropertyContainer::Global(container.to_string())
    }
}

fn get_property_value(container: SharedString, property_name: SharedString) -> PropertyValue {
    preview::component_instance()
        .and_then(|component_instance| {
            preview_data::get_preview_data(
                &component_instance,
                &to_property_container(container),
                property_name.as_str(),
            )
        })
        .map(|pd| map_preview_data_to_property_value(&pd))
        .unwrap_or_default()
}

fn map_preview_data_to_property_value_table(
    preview_data: &preview_data::PreviewData,
) -> (bool, Vec<SharedString>, Vec<Vec<PropertyValue>>) {
    let mut mapping = ValueMapping::default();
    map_value_and_type(&preview_data.ty, &preview_data.value, &mut mapping);

    let is_array = mapping.is_array;
    let headers = std::mem::take(&mut mapping.headers);
    let values = std::mem::take(&mut mapping.array_values);

    (is_array, headers, values)
}

fn get_property_value_table(
    container: SharedString,
    property_name: SharedString,
) -> PropertyValueTable {
    let (is_array, headers, mut values) = preview::component_instance()
        .and_then(|component_instance| {
            preview_data::get_preview_data(
                &component_instance,
                &to_property_container(container),
                property_name.as_str(),
            )
        })
        .map(|pd| map_preview_data_to_property_value_table(&pd))
        .unwrap_or_else(|| (false, Default::default(), Default::default()));

    let headers = Rc::new(VecModel::from(headers)).into();
    let values = Rc::new(VecModel::from(
        values.drain(..).map(|cv| Rc::new(VecModel::from(cv)).into()).collect::<Vec<_>>(),
    ))
    .into();

    PropertyValueTable { is_array, headers, values }
}

fn table_to_array(table: ModelRc<ModelRc<PropertyValue>>) -> Option<String> {
    let mut result = "[\n".to_string();

    for (row_number, row) in table.iter().enumerate() {
        if row_number != 0 {
            result += ",\n";
        }

        result += &table_row_to_struct(row, 1)?;
    }

    result += "\n]";

    Some(result)
}

fn table_row_to_struct(row: ModelRc<PropertyValue>, indent_level: usize) -> Option<String> {
    enum NodeKind {
        Leaf(String),
        Inner(BTreeMap<String, NodeKind>),
    }

    if row.row_count() == 1 {
        if let Some(v) = row.row_data(0) {
            if v.accessor_path.is_empty() {
                // bare value!
                return Some(format!("{}{}", "  ".repeat(indent_level), v.code));
            }
        }
    }

    fn structurize(row: ModelRc<PropertyValue>) -> Option<BTreeMap<String, NodeKind>> {
        let mut result = BTreeMap::default();

        fn insert(
            map: &mut BTreeMap<String, NodeKind>,
            accessor_path: &[&str],
            value: String,
        ) -> Option<()> {
            match accessor_path.len() {
                0 => None,
                1 => {
                    let prev = map
                        .insert(accessor_path.first().unwrap().to_string(), NodeKind::Leaf(value));
                    prev.is_none().then_some(())
                }
                _ => {
                    let n = map
                        .entry(accessor_path.first().unwrap().to_string())
                        .or_insert_with(|| NodeKind::Inner(BTreeMap::default()));

                    match n {
                        NodeKind::Leaf(_) => None,
                        NodeKind::Inner(m) => insert(m, &accessor_path[1..], value),
                    }
                }
            }
        }

        for col in row.iter() {
            let ap = col.accessor_path.split('.').collect::<Vec<_>>();
            let value = if col.was_edited { col.edited_value.clone() } else { col.code.clone() };

            insert(&mut result, &ap[..], value.to_string())?;
        }

        Some(result)
    }

    let structure = structurize(row)?;

    fn structure_to_string(
        structure: &BTreeMap<String, NodeKind>,
        indent_level: usize,
        prefix: &str,
    ) -> Option<String> {
        let indent_step = "  ";
        let mut result = format!("{}{prefix}{{\n", indent_step.repeat(indent_level));

        let last_index = structure.len() - 1;

        for (index, (k, v)) in structure.iter().enumerate() {
            let comma = if index == last_index { "" } else { "," };
            match v {
                NodeKind::Leaf(v) => {
                    result +=
                        &format!("{}\"{k}\": {v}{comma}\n", indent_step.repeat(indent_level + 1))
                }
                NodeKind::Inner(m) => {
                    result += &structure_to_string(m, indent_level + 1, &format!("\"{k}\": "))?;
                    result += &format!("{comma}\n");
                }
            }
        }

        result += &format!("{}}}", indent_step.repeat(indent_level));

        Some(result)
    }

    structure_to_string(&structure, indent_level, "")
}

fn set_property_value_table(
    container: SharedString,
    property_name: SharedString,
    table: ModelRc<ModelRc<PropertyValue>>,
    is_array: bool,
) -> SharedString {
    let json_string = if is_array {
        table_to_array(table)
    } else {
        if table.row_count() != 1 {
            // A struct must have exactly one row!
            return "Malformed table".into();
        }

        table_row_to_struct(table.row_data(0).unwrap(), 0)
    };

    let Some(json_string) = json_string else {
        return "Could not process input values".into();
    };

    set_json_preview_data(container, property_name, json_string.into())
}

fn default_property_value(source: &PropertyValue) -> PropertyValue {
    let mut pv = PropertyValue {
        kind: source.kind,
        accessor_path: source.accessor_path.clone(),
        ..Default::default()
    };
    match source.kind {
        PropertyValueKind::Boolean => {
            pv.display_string = "false".into();
            pv.code = "false".into();
        }
        PropertyValueKind::Brush => {
            pv.display_string = "#00000000".into();
            pv.brush_kind = BrushKind::Solid;
            pv.value_brush = slint::Color::default().into();
            pv.code = "#00000000".into();
        }
        PropertyValueKind::Code => {
            pv.display_string = "Code".into();
        }
        PropertyValueKind::Color => {
            pv.display_string = "#00000000".into();
            pv.brush_kind = BrushKind::Solid;
            pv.value_brush = slint::Color::default().into();
            pv.code = "#00000000".into();
        }
        PropertyValueKind::Enum => {
            let enum_selection: SharedString = format!(
                "{}.{}",
                source.value_string,
                source
                    .visual_items
                    .row_data(source.default_selection.try_into().unwrap_or_default())
                    .unwrap_or_default(),
            )
            .into();

            pv.display_string = enum_selection.clone();
            pv.value_int = source.default_selection;
            pv.value_string = source.value_string.clone();
            pv.default_selection = source.default_selection;
            pv.visual_items = source.visual_items.clone();
            pv.code = enum_selection;
        }
        PropertyValueKind::Float => {
            pv.display_string = "0.0".into();
            pv.code = "0.0".into();
        }
        PropertyValueKind::Integer => {
            pv.display_string = "0".into();
            pv.code = "0".into();
        }
        PropertyValueKind::String => {
            pv.display_string = "".into();
            pv.code = "\"\"".into();
        }
    }

    pv
}

fn insert_row_into_value_table(table: PropertyValueTable, insert_before: i32) {
    if !table.is_array {
        return;
    }

    let model = table.values.clone();
    let insert_before = (insert_before as usize).clamp(0, model.row_count());

    let Some(vec_model) = model.as_any().downcast_ref::<VecModel<ModelRc<PropertyValue>>>() else {
        return;
    };

    let row_data = {
        let mut result = vec![];
        if let Some(row) = vec_model.row_data(0) {
            result = row.iter().map(|pv| default_property_value(&pv)).collect::<Vec<_>>();
        }
        result
    };

    let row_model = Rc::new(VecModel::from(row_data));
    if vec_model.row_count() == insert_before {
        vec_model.push(row_model.into());
    } else {
        vec_model.insert(insert_before, row_model.into());
    }
}

fn remove_row_from_value_table(table: PropertyValueTable, to_remove: i32) {
    if to_remove < 0 || !table.is_array {
        return;
    }
    let to_remove = to_remove as usize;

    let model = table.values.clone();
    let Some(vec_model) = model.as_any().downcast_ref::<VecModel<ModelRc<PropertyValue>>>() else {
        return;
    };

    if to_remove < vec_model.row_count() {
        vec_model.remove(to_remove);
    }
}

fn set_json_preview_data(
    container: SharedString,
    property_name: SharedString,
    json_string: SharedString,
) -> SharedString {
    let property_name = (!property_name.is_empty()).then_some(property_name.to_string());

    let json = match serde_json::from_str::<serde_json::Value>(json_string.as_ref()) {
        Ok(j) => j,
        Err(e) => {
            return SharedString::from(format!("Input is not valid JSON: {e}"));
        }
    };

    if property_name.is_none() && !json.is_object() {
        return SharedString::from("Input for Slint Element is not a JSON object");
    }

    if let Some(ci) = preview::component_instance() {
        match preview_data::set_json_preview_data(
            &ci,
            to_property_container(container),
            property_name,
            json,
        ) {
            Ok(_) => SharedString::new(),
            Err(v) => v.first().cloned().unwrap_or_default().into(),
        }
    } else {
        SharedString::from("No preview loaded")
    }
}

fn update_properties(
    current_model: PropertyGroupModel,
    next_model: PropertyGroupModel,
) -> PropertyGroupModel {
    if current_model.row_count() != next_model.row_count() {
        return next_model;
    }

    for (c, n) in std::iter::zip(current_model.iter(), next_model.iter()) {
        debug_assert_eq!(c.group_name, n.group_name);

        let cvg = c.properties.as_any().downcast_ref::<VecModel<PropertyInformation>>().unwrap();
        let nvg = n.properties.as_any().downcast_ref::<VecModel<PropertyInformation>>().unwrap();

        update_grouped_properties(cvg, nvg);
    }

    current_model
}

pub fn ui_set_properties(
    ui: &PreviewUi,
    document_cache: &common::DocumentCache,
    properties: Option<properties::QueryPropertyResponse>,
) -> PropertyDeclarations {
    let win = i_slint_core::window::WindowInner::from_pub(ui.window()).window_adapter();
    let (next_element, declarations, next_model) =
        property_view::map_properties_to_ui(document_cache, properties, &win).unwrap_or((
            ElementInformation {
                id: "".into(),
                type_name: "".into(),
                source_uri: "".into(),
                source_version: 0,
                offset: 0,
            },
            HashMap::new(),
            Rc::new(VecModel::from(Vec::<PropertyGroup>::new())).into(),
        ));

    let api = ui.global::<Api>();
    let current_model = api.get_properties();

    let element = api.get_current_element();
    if !is_equal_element(&element, &next_element) {
        api.set_properties(next_model);
    } else if current_model.row_count() > 0 {
        update_properties(current_model, next_model);
    } else {
        api.set_properties(next_model);
    }

    api.set_current_element(next_element);

    declarations
}

#[cfg(test)]
mod tests {
    use crate::preview::preview_data;

    use slint::{Model, SharedString, ToSharedString, VecModel};

    use super::{PropertyInformation, PropertyValue, PropertyValueKind};

    fn create_test_property(name: &str, value: &str) -> PropertyInformation {
        PropertyInformation {
            name: name.into(),
            display_priority: 1000,
            type_name: "Sometype".into(),
            value: PropertyValue {
                kind: PropertyValueKind::String,
                value_string: value.into(),
                code: value.into(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn test_property_date_update() {
        let current = VecModel::from(vec![
            create_test_property("aaa", "AAA"),
            create_test_property("bbb", "BBB"),
            create_test_property("ccc", "CCC"),
            create_test_property("ddd", "DDD"),
            create_test_property("eee", "EEE"),
        ]);
        let next = VecModel::from(vec![
            create_test_property("aaa", "AAA"),
            create_test_property("aab", "AAB"),
            create_test_property("abb", "ABB"),
            create_test_property("bbb", "BBBX"),
            create_test_property("ddd", "DDD"),
        ]);

        super::update_grouped_properties(&current, &next);

        let mut it = current.iter();

        let t = it.next().unwrap();
        assert_eq!(t.name.as_str(), "aaa");
        assert_eq!(t.value.code.as_str(), "AAA");

        let t = it.next().unwrap();
        assert_eq!(t.name.as_str(), "aab");
        assert_eq!(t.value.code.as_str(), "AAB");

        let t = it.next().unwrap();
        assert_eq!(t.name.as_str(), "abb");
        assert_eq!(t.value.code.as_str(), "ABB");

        let t = it.next().unwrap();
        assert_eq!(t.name.as_str(), "bbb");
        assert_eq!(t.value.code.as_str(), "BBBX");

        let t = it.next().unwrap();
        assert_eq!(t.name.as_str(), "ddd");
        assert_eq!(t.value.code.as_str(), "DDD");

        assert!(it.next().is_none());
    }

    fn generate_preview_data(
        visibility: &str,
        type_def: &str,
        type_name: &str,
        code: &str,
    ) -> (crate::preview::preview_data::PreviewDataKey, crate::preview::preview_data::PreviewData)
    {
        let component_instance = crate::preview::test::interpret_test(
            "fluent",
            &format!(
                r#"
{type_def}
export component Tester {{
    {visibility} property <{type_name}> test: {code};
}}
            "#
            ),
        );
        let mut data =
            preview_data::query_preview_data_properties_and_callbacks(&component_instance);
        assert_eq!(data.len(), 1);
        data.pop_first().unwrap()
    }

    fn compare_pv(r: &super::PropertyValue, e: &PropertyValue) {
        eprintln!("Received: {r:?}");
        eprintln!("Expected: {e:?}");

        assert_eq!(r.value_bool, e.value_bool);
        assert_eq!(r.is_translatable, e.is_translatable);
        assert_eq!(r.value_brush, e.value_brush);
        assert_eq!(r.value_float, e.value_float);
        assert_eq!(r.value_int, e.value_int);
        assert_eq!(r.default_selection, e.default_selection);
        assert_eq!(r.value_string, e.value_string);
        assert_eq!(r.tr_context, e.tr_context);
        assert_eq!(r.tr_plural, e.tr_plural);
        assert_eq!(r.tr_plural_expression, e.tr_plural_expression);
        assert_eq!(r.code, e.code);

        assert_eq!(r.visual_items.row_count(), e.visual_items.row_count());
        for (r, e) in r.visual_items.iter().zip(e.visual_items.iter()) {
            assert_eq!(r, e);
        }
    }

    fn validate_rp_impl(
        visibility: &str,
        type_def: &str,
        type_name: &str,
        code: &str,
        expected_data: super::PreviewData,
    ) -> (preview_data::PreviewDataKey, preview_data::PreviewData) {
        let (key, value) = generate_preview_data(visibility, type_def, type_name, code);
        let rp = super::map_preview_data_property(&key, &value).unwrap();

        eprintln!("*** Validating PreviewData: Received: {rp:?}");
        eprintln!("*** Validating PreviewData: Expected: {expected_data:?}");

        assert_eq!(rp.name, expected_data.name);
        assert_eq!(rp.has_getter, expected_data.has_getter);
        assert_eq!(rp.has_setter, expected_data.has_setter);
        assert_eq!(rp.kind, expected_data.kind);

        eprintln!("*** PreviewData is as expected...");

        (key, value)
    }

    fn validate_rp(
        visibility: &str,
        type_def: &str,
        type_name: &str,
        code: &str,
        expected_data: super::PreviewData,
        expected_value: super::PropertyValue,
    ) {
        let (_, value) = validate_rp_impl(visibility, type_def, type_name, code, expected_data);

        let pv = super::map_preview_data_to_property_value(&value);
        compare_pv(&pv, &expected_value);

        let (is_array, headers, values) = super::map_preview_data_to_property_value_table(&value);
        assert!(!is_array);
        assert!(headers.len() == 1);
        assert!(headers[0].is_empty());
        assert_eq!(values.len(), 1);
        assert_eq!(values.first().unwrap().len(), 1);
    }

    fn validate_table_rp(
        visibility: &str,
        type_def: &str,
        type_name: &str,
        code: &str,
        expected_data: super::PreviewData,
        expected_code: &str,
        expected_is_array: bool,
        expected_headers: Vec<String>,
        expected_table: Vec<Vec<super::PropertyValue>>,
    ) {
        let (_, value) = validate_rp_impl(visibility, type_def, type_name, code, expected_data);

        let pv = super::map_preview_data_to_property_value(&value);
        compare_pv(
            &pv,
            &super::PropertyValue {
                kind: super::PropertyValueKind::Code,
                code: expected_code.into(),
                ..Default::default()
            },
        );

        let (is_array, headers, values) = super::map_preview_data_to_property_value_table(&value);

        assert_eq!(is_array, expected_is_array);

        for (idx, h) in headers.iter().enumerate() {
            eprintln!("Header {idx}: \"{h}\"");
        }
        assert_eq!(headers.len(), expected_headers.len());
        assert!(headers.iter().zip(expected_headers.iter()).all(|(rh, eh)| rh == eh));

        assert_eq!(values.len(), expected_table.len());
        for (rr, er) in values.iter().zip(expected_table.iter()) {
            assert!(!rr.is_empty());
            assert_eq!(rr.len(), er.len());
            rr.iter().zip(er.iter()).for_each(|(rv, ev)| compare_pv(rv, ev));
        }
    }

    #[test]
    fn test_map_preview_data_string() {
        validate_rp(
            "in",
            "",
            "string",
            "\"Test\"",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                display_string: "\"Test\"".into(),
                code: "\"Test\"".into(),
                kind: super::PropertyValueKind::String,
                value_string: "Test".into(),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_length_px() {
        validate_rp(
            "in",
            "",
            "length",
            "100px",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                display_string: "100".into(),
                code: "100".into(),
                kind: super::PropertyValueKind::Float,
                value_float: 100.0,
                visual_items: std::rc::Rc::new(VecModel::from(vec![
                    "px".into(),
                    "cm".into(),
                    "mm".into(),
                    "in".into(),
                    "pt".into(),
                    "phx".into(),
                    "rem".into(),
                ]))
                .into(),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_length_cm() {
        validate_rp(
            "in",
            "",
            "length",
            "10cm",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                display_string: "378px".into(),
                code: "378".into(),
                kind: super::PropertyValueKind::Float,
                value_float: 378.0,
                visual_items: std::rc::Rc::new(VecModel::from(vec![
                    "px".into(),
                    "cm".into(),
                    "mm".into(),
                    "in".into(),
                    "pt".into(),
                    "phx".into(),
                    "rem".into(),
                ]))
                .into(),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_duration() {
        validate_rp(
            "in",
            "",
            "duration",
            "100s",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                display_string: "100000ms".into(),
                code: "100000".into(),
                kind: super::PropertyValueKind::Float,
                value_float: 100000.0,
                visual_items: std::rc::Rc::new(VecModel::from(vec!["s".into(), "ms".into()]))
                    .into(),
                default_selection: 1,
                value_int: 1,
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_angle() {
        validate_rp(
            "in",
            "",
            "angle",
            "100turn",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                display_string: "36000deg".into(),
                code: "36000".into(),
                kind: super::PropertyValueKind::Float,
                value_float: 36000.0,
                visual_items: std::rc::Rc::new(VecModel::from(vec![
                    "deg".into(),
                    "grad".into(),
                    "turn".into(),
                    "rad".into(),
                ]))
                .into(),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_percent() {
        validate_rp(
            "in",
            "",
            "percent",
            "10%",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                display_string: "10%".into(),
                code: "10".into(),
                kind: super::PropertyValueKind::Float,
                value_float: 10.0,
                visual_items: std::rc::Rc::new(VecModel::from(vec!["%".into()])).into(),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_color() {
        validate_rp(
            "in",
            "",
            "color",
            "#aabbcc",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                display_string: "#aabbccff".into(),
                code: "\"#aabbccff\"".into(),
                kind: super::PropertyValueKind::Color,
                value_brush: slint::Brush::SolidColor(slint::Color::from_argb_u8(
                    0xff, 0xaa, 0xbb, 0xcc,
                )),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_int() {
        validate_rp(
            "in",
            "",
            "int",
            "12",
            super::PreviewData {
                name: "test".into(),
                has_setter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                display_string: "12".into(),
                code: "12".into(),
                kind: super::PropertyValueKind::Integer,
                value_int: 12,
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_bool_true() {
        validate_rp(
            "out",
            "",
            "bool",
            "true",
            super::PreviewData {
                name: "test".into(),
                has_getter: true,
                kind: super::PreviewDataKind::Value,
                ..Default::default()
            },
            super::PropertyValue {
                display_string: "true".into(),
                code: "true".into(),
                kind: super::PropertyValueKind::Boolean,
                value_bool: true,
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_bool_false() {
        validate_rp(
            "in-out",
            "",
            "bool",
            "false",
            super::PreviewData {
                name: "test".into(),
                has_getter: true,
                has_setter: true,
                kind: super::PreviewDataKind::Value,
            },
            super::PropertyValue {
                display_string: "false".into(),
                code: "false".into(),
                kind: super::PropertyValueKind::Boolean,
                value_bool: false,
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_struct_with_array() {
        validate_rp(
            "in-out",
            r#"
            struct FooStruct { first: [string] }
            "#,
            "FooStruct",
            "{ first: [ \"first of a kind\", \"second of a kind\"] }",
            super::PreviewData {
                name: "test".into(),
                has_getter: true,
                has_setter: true,
                kind: super::PreviewDataKind::Json,
            },
            super::PropertyValue {
                kind: super::PropertyValueKind::Code,
                code:
                    "{\n  \"first\": [\n    \"first of a kind\",\n    \"second of a kind\"\n  ]\n}"
                        .into(),
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_map_preview_data_struct() {
        validate_table_rp(
            "in-out",
            "struct FooStruct { bar: bool, count: int }",
            "FooStruct",
            "{ bar: true, count: 23 }",
            super::PreviewData {
                name: "test".into(),
                has_getter: true,
                has_setter: true,
                kind: super::PreviewDataKind::Table,
            },
            "{\n  \"bar\": true,\n  \"count\": 23\n}",
            false,
            vec!["bar".into(), "count".into()],
            vec![vec![
                super::PropertyValue {
                    display_string: "true".into(),
                    code: "true".into(),
                    kind: super::PropertyValueKind::Boolean,
                    value_bool: true,
                    ..Default::default()
                },
                super::PropertyValue {
                    display_string: "23".into(),
                    code: "23".into(),
                    kind: super::PropertyValueKind::Integer,
                    value_int: 23,
                    ..Default::default()
                },
            ]],
        );
    }

    #[test]
    fn test_map_preview_data_struct_of_structs() {
        validate_table_rp(
            "in-out",
            r#"
            struct C1 { c1_1: string, c1_2: int }
            struct C2 { c2_1: string, c2_2: int }
            struct FooStruct { first: C1, second: C2 }
            "#,
           "FooStruct",
           "{ first: { c1_1: \"first of a kind\", c1_2: 23 }, second: { c2_1: \"second of a kind\", c2_2: 42 } }",
            super::PreviewData {
                name: "test".into(),
                has_getter: true,
                has_setter: true,
                kind: super::PreviewDataKind::Table,
                },
            "{\n  \"first\": {\n    \"c1-1\": \"first of a kind\",\n    \"c1-2\": 23\n  },\n  \"second\": {\n    \"c2-1\": \"second of a kind\",\n    \"c2-2\": 42\n  }\n}",
            false,
                vec![
                    "first.c1-1".into(),
                    "first.c1-2".into(),
                    "second.c2-1".into(),
                   "second.c2-2".into(),
                ],
               vec![
                    vec![super::PropertyValue {
                            display_string: "first of a kind".into(),
                            code: "\"first of a kind\"".into(),
                            kind: super::PropertyValueKind::String,
                            value_string: "first of a kind".into(),
                            ..Default::default()
                        },
                        super::PropertyValue {
                            display_string: "23".into(),
                            code: "23".into(),
                            kind: super::PropertyValueKind::Integer,
                            value_int: 23,
                            ..Default::default()
                        },
                        super::PropertyValue {
                            display_string: "second of a kind".into(),
                            code: "\"second of a kind\"".into(),
                            kind: super::PropertyValueKind::String,
                            value_string: "second of a kind".into(),
                            ..Default::default()
                        },
                        super::PropertyValue {
                            display_string: "42".into(),
                            code: "42".into(),
                            kind: super::PropertyValueKind::Integer,
                            value_int: 42,
                            ..Default::default()
                        },
                        ],
                ]
        );
    }

    #[test]
    fn test_map_preview_data_array_of_struct_of_structs() {
        validate_table_rp(
            "in-out",
            r#"
            struct C1 { c1_1: string, c1_2: int }
            struct C2 { c2_1: string, c2_2: int }
            struct FooStruct { first: C1, second: C2 }
            "#,
           "[FooStruct]",
           "[{ first: { c1_1: \"first of a kind\", c1_2: 23 }, second: { c2_1: \"second of a kind\", c2_2: 42 } }, { first: { c1_1: \"row 2, 1\", c1_2: 3 }, second: { c2_1: \"row 2, 2\", c2_2: 2 } }]",
            super::PreviewData {
                name: "test".into(),
                has_getter: true,
                has_setter: true,
                kind: super::PreviewDataKind::Table,
            },
            "[\n  {\n    \"first\": {\n      \"c1-1\": \"first of a kind\",\n      \"c1-2\": 23\n    },\n    \"second\": {\n      \"c2-1\": \"second of a kind\",\n      \"c2-2\": 42\n    }\n  },\n  {\n    \"first\": {\n      \"c1-1\": \"row 2, 1\",\n      \"c1-2\": 3\n    },\n    \"second\": {\n      \"c2-1\": \"row 2, 2\",\n      \"c2-2\": 2\n    }\n  }\n]",
            true,
                vec![
                    "first.c1-1".into(),
                    "first.c1-2".into(),
                    "second.c2-1".into(),
                   "second.c2-2".into(),
                ],
               vec![
                    vec![super::PropertyValue {
                            display_string: "first of a kind".into(),
                            code: "\"first of a kind\"".into(),
                            kind: super::PropertyValueKind::String,
                            value_string: "first of a kind".into(),
                            ..Default::default()
                        },
                        super::PropertyValue {
                            display_string: "23".into(),
                            code: "23".into(),
                            kind: super::PropertyValueKind::Integer,
                            value_int: 23,
                            ..Default::default()
                        },
                        super::PropertyValue {
                            display_string: "second of a kind".into(),
                            code: "\"second of a kind\"".into(),
                            kind: super::PropertyValueKind::String,
                            value_string: "second of a kind".into(),
                            ..Default::default()
                        },
                        super::PropertyValue {
                            display_string: "42".into(),
                            code: "42".into(),
                            kind: super::PropertyValueKind::Integer,
                            value_int: 42,
                            ..Default::default()
                        },
                    ],
                    vec![super::PropertyValue {
                            display_string: "row 2, 1".into(),
                            code: "\"row 2, 1\"".into(),
                            kind: super::PropertyValueKind::String,
                            value_string: "row 2, 1".into(),
                            ..Default::default()
                        },
                        super::PropertyValue {
                            display_string: "3".into(),
                            code: "3".into(),
                            kind: super::PropertyValueKind::Integer,
                            value_int: 3,
                            ..Default::default()
                        },
                        super::PropertyValue {
                            display_string: "row 2, 2".into(),
                            code: "\"row 2, 2\"".into(),
                            kind: super::PropertyValueKind::String,
                            value_string: "row 2, 2".into(),
                            ..Default::default()
                        },
                        super::PropertyValue {
                            display_string: "2".into(),
                            code: "2".into(),
                            kind: super::PropertyValueKind::Integer,
                            value_int: 2,
                            ..Default::default()
                        },
                    ],
                ]
        );
    }

    #[test]
    fn test_map_preview_data_bool_array() {
        validate_table_rp(
            "in-out",
            "",
            "[bool]",
            "[ true, false ]",
            super::PreviewData {
                name: "test".into(),
                has_getter: true,
                has_setter: true,
                kind: super::PreviewDataKind::Table,
            },
            "[\n  true,\n  false\n]",
            true,
            vec!["".into()],
            vec![
                vec![super::PropertyValue {
                    display_string: "true".into(),
                    code: "true".into(),
                    kind: super::PropertyValueKind::Boolean,
                    value_bool: true,
                    ..Default::default()
                }],
                vec![super::PropertyValue {
                    display_string: "false".into(),
                    code: "false".into(),
                    kind: super::PropertyValueKind::Boolean,
                    value_bool: false,
                    ..Default::default()
                }],
            ],
        );
    }

    #[track_caller]
    fn validate_array_row_to_struct(indent_level: usize, row: Vec<PropertyValue>, expected: &str) {
        let model = std::rc::Rc::new(VecModel::from(row)).into();
        let received = super::table_row_to_struct(model, indent_level).unwrap();

        assert_eq!(received, expected);
    }

    #[test]
    fn test_table_row_to_stuct() {
        fn bool_pv(value: bool, accessor_path: &str) -> PropertyValue {
            PropertyValue {
                accessor_path: SharedString::from(accessor_path),
                display_string: value.to_shared_string(),
                value_bool: value,
                code: value.to_shared_string(),
                ..Default::default()
            }
        }

        validate_array_row_to_struct(0, vec![bool_pv(true, "")], "true");
        validate_array_row_to_struct(1, vec![bool_pv(true, "")], "  true");
        validate_array_row_to_struct(2, vec![bool_pv(true, "")], "    true");
        validate_array_row_to_struct(3, vec![bool_pv(true, "")], "      true");
        validate_array_row_to_struct(
            1,
            vec![bool_pv(true, "test")],
            "  {\n    \"test\": true\n  }",
        );
        validate_array_row_to_struct(
            0,
            vec![bool_pv(true, "l1.l2.l3")],
            "{\n  \"l1\": {\n    \"l2\": {\n      \"l3\": true\n    }\n  }\n}",
        );
        validate_array_row_to_struct(
            0,
            vec![bool_pv(true, "l1.l2.l3"), bool_pv(false, "l1.test")],
            "{\n  \"l1\": {\n    \"l2\": {\n      \"l3\": true\n    },\n    \"test\": false\n  }\n}",
        );
    }
}
