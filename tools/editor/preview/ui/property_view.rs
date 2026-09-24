// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{collections::HashMap, rc::Rc};

use itertools::Itertools;
use lsp_types::Url;
use smol_str::SmolStr;

use i_slint_compiler::{
    expression_types, langtype, literals, object_tree,
    parser::{SyntaxKind, TextRange, syntax_nodes},
};

use slint::{Model as _, SharedString, VecModel};

use crate::preview::{properties, ui};

pub fn setup(editor: &ui::EditorUi, api: &ui::Api<'_>) {
    use slint::ComponentHandle as _;
    let api_weak = <ui::Api as slint::Global<'_, ui::EditorUi>>::as_weak(api);
    let selection_api_weak = api_weak.clone();
    let editor_weak = editor.as_weak();
    let selected_item = selected_item_cache(move || {
        let editor = editor_weak.upgrade()?;
        let _ = editor.global::<ui::Hover>().get_preview_generation();
        let api = selection_api_weak.upgrade()?;
        let element = api.get_current_element();
        let instance_index = usize::try_from(api.get_selection().highlight_index).ok()?;
        let instance = crate::preview::component_instance()?;
        let source_uri = lsp_types::Url::parse(element.source_uri.as_str()).ok()?;
        let path = i_slint_compiler::source_path::SourcePath::from_url(&source_uri);
        let offset = u32::try_from(element.offset).ok()?;
        instance.component_item(&path, offset, instance_index, i_slint_core::InternalToken)
    });
    api.on_source_property_value_data(move |property_name, fallback| {
        api_weak
            .upgrade()
            .and_then(|api| ui::source_property_value_data(&api, property_name))
            .unwrap_or(fallback)
    });
    api.on_runtime_property_value_data(move |property_name, fallback| {
        let runtime = selected_item
            .as_ref()
            .get()
            .upgrade()
            .and_then(|item| runtime_property(&item, property_name.as_str()));
        runtime_property_value(runtime, fallback)
    });
}

fn selected_item_cache(
    lookup: impl Fn() -> Option<i_slint_core::items::ItemRc> + 'static,
) -> std::pin::Pin<Rc<i_slint_core::Property<i_slint_core::item_tree::ItemWeak>>> {
    let selected_item = Rc::pin(i_slint_core::Property::default());
    selected_item.set_binding(move || lookup().map(|item| item.downgrade()).unwrap_or_default());
    selected_item
}

fn runtime_property(
    item: &i_slint_core::items::ItemRc,
    property_name: &str,
) -> Option<slint_interpreter::Value> {
    if matches!(property_name, "font-family" | "font-size" | "font-weight") {
        let font = font_request(item)?;
        return match property_name {
            "font-family" => font.family.map(slint_interpreter::Value::String),
            "font-size" => {
                font.pixel_size.map(|size| slint_interpreter::Value::Number(size.get() as f64))
            }
            "font-weight" => {
                font.weight.map(|weight| slint_interpreter::Value::Number(weight as f64))
            }
            _ => unreachable!(),
        };
    }
    if matches!(property_name, "width" | "height") {
        let size = item.geometry().size;
        return Some(slint_interpreter::Value::Number(match property_name {
            "width" => size.width as f64,
            _ => size.height as f64,
        }));
    }
    slint_interpreter::ComponentInstance::item_property(
        item,
        property_name,
        i_slint_core::InternalToken,
    )
}

fn font_request(item: &i_slint_core::items::ItemRc) -> Option<i_slint_core::graphics::FontRequest> {
    use i_slint_core::item_rendering::HasFont;
    use i_slint_core::items::{ComplexText, SimpleText, StyledTextItem, TextInput};
    macro_rules! resolve_font {
        ($($item_type:ty),*) => {
            $(if let Some(text) = item.downcast::<$item_type>() {
                return Some(text.as_pin_ref().font_request(item));
            })*
        };
    }
    resolve_font!(ComplexText, SimpleText, StyledTextItem, TextInput);
    None
}

fn runtime_property_value(
    runtime: Option<slint_interpreter::Value>,
    fallback: ui::PropertyValue,
) -> ui::PropertyValue {
    use slint_interpreter::Value;
    let Some(value) = runtime else { return fallback };
    let mut property = ui::PropertyValue { value_resolved: true, ..Default::default() };
    property.value_kind = match value {
        Value::Number(number) => {
            property.value_float = number as f32;
            property.value_int = number as i32;
            ui::PropertyValueKind::Float
        }
        Value::String(text) => {
            property.value_string = text;
            ui::PropertyValueKind::String
        }
        Value::Bool(value) => {
            property.value_bool = value;
            ui::PropertyValueKind::Boolean
        }
        Value::Brush(brush) => {
            property.value_brush = brush;
            ui::PropertyValueKind::Brush
        }
        Value::EnumerationValue(enumeration_name, value) => {
            let enumeration_type = i_slint_compiler::typeregister::TypeRegister::builtin()
                .borrow()
                .lookup(&enumeration_name);
            let mapped = ui::map_value_and_type_to_property_value(
                &enumeration_type,
                &Some(Value::EnumerationValue(enumeration_name, value)),
                "",
            );
            property.visual_items = mapped.visual_items;
            property.value_int = mapped.value_int;
            property.value_string = mapped.value_string;
            ui::PropertyValueKind::Enum
        }
        _ => return fallback,
    };
    property.kind = property.value_kind;
    property
}

fn is_complex(expression: Option<syntax_nodes::Expression>, ty: &langtype::Type) -> bool {
    fn handle_node(node: i_slint_compiler::parser::SyntaxNode, ty: &langtype::Type) -> bool {
        match node.kind() {
            SyntaxKind::Expression
            | SyntaxKind::QualifiedName
            | SyntaxKind::AtGradient
            | SyntaxKind::AtTr
            | SyntaxKind::TrContext
            | SyntaxKind::TrPlural => {}
            SyntaxKind::UnaryOpExpression if *ty != langtype::Type::Bool => {}
            _ => return true,
        }

        for n in node.children() {
            if handle_node(n, ty) {
                return true;
            }
        }
        false
    }

    let Some(expression) = expression else {
        return false;
    };

    handle_node(expression.into(), ty)
}

fn map_property_to_ui(
    document_cache: &i_slint_editor_preview::DocumentCache,
    element: &object_tree::ElementRc,
    property_info: &properties::PropertyInformation,
    window_adapter: Option<&Rc<dyn slint::platform::WindowAdapter>>,
) -> (ui::PropertyValue, ui::PropertyDeclaration) {
    let mut value = ui::palette::evaluate_property(
        element,
        property_info.name.as_str(),
        &property_info.default_value,
        &property_info.ty,
        window_adapter,
    );

    let code_block_or_expression =
        property_info.defined_at.as_ref().map(|da| da.code_block_or_expression.clone());
    let expression = code_block_or_expression.as_ref().and_then(|cbe| cbe.expression());

    if let Some(expression) = &expression {
        value.code = SharedString::from(expression.text().to_string());

        if let Some(qualified_name) = expression.QualifiedName() {
            let name = SharedString::from(&qualified_name.text().to_string());
            value.display_string = name.clone();
            value.accessor_path = name.clone();
            if property_info.ty == langtype::Type::Color
                || property_info.ty == langtype::Type::Brush
            {
                value.value_string = name;
            }
        }

        fn extract_value_with_unit(
            expression: &syntax_nodes::Expression,
            units: &[expression_types::WrittenUnit],
            value: &mut ui::PropertyValue,
        ) {
            fn extract_value_with_unit_impl(
                expression: &syntax_nodes::Expression,
                units: &[i_slint_compiler::expression_types::WrittenUnit],
            ) -> Option<(ui::PropertyValueKind, f32, i32, String)> {
                let (value, unit) = convert_number_literal(expression)?;

                let index = units.iter().position(|u| u == &unit).or_else(|| {
                    (units.is_empty()
                        && unit == i_slint_compiler::expression_types::WrittenUnit::None)
                        .then_some(0_usize)
                })?;

                Some((ui::PropertyValueKind::Float, value as f32, index as i32, unit.to_string()))
            }

            if let Some((kind, v, index, unit)) = extract_value_with_unit_impl(expression, units) {
                value.display_string = slint::format!("{v}{unit}");
                value.value_kind = kind;
                value.kind = kind;
                value.value_float = v;
                value.visual_items = ui::unit_model(units);
                value.value_int = index
            }
        }

        fn extract_tr_data(tr_node: &syntax_nodes::AtTr, value: &mut ui::PropertyValue) {
            let Some(text) = tr_node
                .child_text(SyntaxKind::StringLiteral)
                .and_then(|s| i_slint_compiler::literals::unescape_string(&s))
            else {
                return;
            };

            let context = tr_node
                .TrContext()
                .and_then(|n| n.child_text(SyntaxKind::StringLiteral))
                .and_then(|s| literals::unescape_string(&s))
                .unwrap_or_default();
            let plural = tr_node
                .TrPlural()
                .and_then(|n| n.child_text(SyntaxKind::StringLiteral))
                .and_then(|s| literals::unescape_string(&s))
                .unwrap_or_default();
            let plural_expression = tr_node
                .TrPlural()
                .and_then(|n| n.child_node(SyntaxKind::Expression))
                .and_then(|e| e.child_node(SyntaxKind::QualifiedName))
                .map(|n| object_tree::QualifiedTypeName::from_node(n.into()))
                .map(|qtn| qtn.to_string());

            // We have expressions -> Edit as code
            if tr_node.Expression().next().is_none()
                && (plural.is_empty() || plural_expression.is_some())
            {
                value.kind = ui::PropertyValueKind::String;
                value.is_translatable = true;
                value.tr_context = context.as_str().into();
                value.tr_plural = plural.as_str().into();
                value.tr_plural_expression = plural_expression.unwrap_or_default().into();
                value.value_string = text.as_str().into();
                value.code = SharedString::from(tr_node.text().to_string());
            }
        }
        if value.value_kind == ui::PropertyValueKind::String
            && let Some(tr) = &expression.AtTr()
        {
            extract_tr_data(tr, &mut value);
        }

        if value.value_kind == ui::PropertyValueKind::Float {
            use expression_types::WrittenUnit;
            use langtype::Type;

            match &property_info.ty {
                Type::Float32 => extract_value_with_unit(expression, &[], &mut value),
                Type::Duration => extract_value_with_unit(
                    expression,
                    &[WrittenUnit::S, WrittenUnit::Ms],
                    &mut value,
                ),
                Type::PhysicalLength | Type::LogicalLength | Type::Rem => extract_value_with_unit(
                    expression,
                    &[
                        WrittenUnit::Px,
                        WrittenUnit::Cm,
                        WrittenUnit::Mm,
                        WrittenUnit::In,
                        WrittenUnit::Pt,
                        WrittenUnit::Phx,
                        WrittenUnit::Rem,
                    ],
                    &mut value,
                ),
                Type::Angle => extract_value_with_unit(
                    expression,
                    &[WrittenUnit::Deg, WrittenUnit::Grad, WrittenUnit::Turn, WrittenUnit::Rad],
                    &mut value,
                ),
                Type::Percent => {
                    extract_value_with_unit(expression, &[WrittenUnit::Percent], &mut value)
                }
                _ => {}
            }
        }
    }

    if is_complex(expression, &property_info.ty) {
        value.kind = ui::PropertyValueKind::Code;
    }

    let defined_at =
        map_property_definition(&property_info.defined_at).unwrap_or(ui::PropertyDefinition {
            definition_range: ui::Range { start: 0, end: 0 },
            selection_range: ui::Range { start: 0, end: 0 },
            expression_range: ui::Range { start: 0, end: 0 },
            expression_value: String::new().into(),
        });
    let declared_at =
        map_property_declaration(document_cache, &property_info.declared_at, defined_at.clone())
            .unwrap_or(ui::PropertyDeclaration {
                defined_at,
                source_path: String::new().into(),
                source_version: -1,
                range: ui::Range { start: 0, end: 0 },
            });

    (value, declared_at)
}

pub fn map_properties_to_ui(
    document_cache: &i_slint_editor_preview::DocumentCache,
    properties: Option<properties::QueryPropertyResponse>,
    window_adapter: &Rc<dyn slint::platform::WindowAdapter>,
) -> Option<(
    ui::ElementInformation,
    HashMap<SmolStr, ui::PropertyDeclaration>,
    Rc<ui::search_model::SearchModel<ui::PropertyGroup>>,
)> {
    use std::cmp::Ordering;

    let properties = &properties?;
    let element = properties.element.as_ref()?;

    let raw_source_uri = Url::parse(&properties.source_uri).ok()?;
    let source_uri: SharedString = raw_source_uri.to_string().into();
    let source_version = properties.source_version;

    let mut property_groups: HashMap<(SmolStr, u32), Vec<ui::PropertyInformation>> = HashMap::new();

    let mut declarations = HashMap::new();

    fn property_group_from(
        groups: &mut HashMap<(SmolStr, u32), Vec<ui::PropertyInformation>>,
        name: SmolStr,
        group_priority: u32,
        property: ui::PropertyInformation,
    ) {
        let entry = groups.entry((name.clone(), group_priority));
        entry.and_modify(|e| e.push(property.clone())).or_insert(vec![property]);
    }

    for pi in &properties.properties {
        let (value, declared_at) = map_property_to_ui(
            document_cache,
            &properties.element_rc_node.element,
            pi,
            Some(window_adapter),
        );

        declarations.insert(pi.name.clone(), declared_at);

        property_group_from(
            &mut property_groups,
            pi.group.clone(),
            pi.group_priority,
            ui::PropertyInformation {
                name: pi.name.as_str().into(),
                type_name: pi.ty.to_string().into(),
                value,
                display_priority: i32::try_from(pi.priority).unwrap(),
            },
        );
    }

    let keys = property_groups
        .keys()
        .sorted_by(|a, b| match a.1.cmp(&b.1) {
            Ordering::Less => Ordering::Less,
            Ordering::Equal => a.0.cmp(&b.0),
            Ordering::Greater => Ordering::Greater,
        })
        .cloned()
        .collect::<Vec<_>>();

    type InnerGroupModel = ui::search_model::SearchModel<ui::PropertyInformation>;

    Some((
        ui::ElementInformation {
            id: element.id.as_str().into(),
            component_name: element.component_name.as_str().into(),
            type_name: element.type_name.as_str().into(),
            source_uri,
            source_version,
            offset: u32::from(element.offset) as i32,
        },
        declarations,
        Rc::new(ui::search_model::SearchModel::new(
            VecModel::from(
                keys.iter()
                    .map(|k| ui::PropertyGroup {
                        group_name: k.0.as_str().into(),
                        properties: Rc::new(InnerGroupModel::new(
                            VecModel::from({
                                let mut v = property_groups.remove(k).unwrap();
                                v.sort_by(|a, b| {
                                    match a.display_priority.cmp(&b.display_priority) {
                                        Ordering::Less => Ordering::Less,
                                        Ordering::Equal => a.name.cmp(&b.name),
                                        Ordering::Greater => Ordering::Greater,
                                    }
                                });
                                v
                            }),
                            |i, search_str| ui::search_model::contains(&i.name, search_str),
                        ))
                        .into(),
                    })
                    .collect::<Vec<_>>(),
            ),
            |group, search_str| {
                let yes = search_str.is_empty()
                    || ui::search_model::contains(&group.group_name, search_str);
                if let Some(sub_filter) =
                    group.properties.as_any().downcast_ref::<InnerGroupModel>()
                {
                    if yes {
                        sub_filter.set_search_text(Default::default());
                    } else {
                        sub_filter.set_search_text(search_str.clone());
                        return sub_filter.row_count() > 0;
                    }
                }
                yes
            },
        )),
    ))
}

fn map_property_declaration(
    document_cache: &i_slint_editor_preview::DocumentCache,
    declared_at: &Option<properties::DeclarationInformation>,
    defined_at: ui::PropertyDefinition,
) -> Option<ui::PropertyDeclaration> {
    let da = declared_at.as_ref()?;
    let source_version = document_cache.document_version_by_path(&da.path).unwrap_or(-1);
    let pos = TextRange::new(da.start_position, da.start_position);

    Some(ui::PropertyDeclaration {
        defined_at,
        source_path: da.path.to_string().into(),
        source_version,
        range: ui::to_ui_range(pos)?,
    })
}

fn map_property_definition(
    defined_at: &Option<properties::DefinitionInformation>,
) -> Option<ui::PropertyDefinition> {
    let da = defined_at.as_ref()?;

    Some(ui::PropertyDefinition {
        definition_range: ui::to_ui_range(da.property_definition_range)?,
        selection_range: ui::to_ui_range(da.selection_range)?,
        expression_range: ui::to_ui_range(da.code_block_or_expression.text_range())?,
        expression_value: da.code_block_or_expression.text().to_string().into(),
    })
}

fn convert_number_literal(
    node: &syntax_nodes::Expression,
) -> Option<(f64, i_slint_compiler::expression_types::WrittenUnit)> {
    if let Some(unary) = &node.UnaryOpExpression() {
        let factor = match unary.first_token().unwrap().text() {
            "-" => -1.0,
            "+" => 1.0,
            _ => return None,
        };
        convert_number_literal(&unary.Expression()).map(|(v, u)| (factor * v, u))
    } else {
        let literal = node.child_text(SyntaxKind::NumberLiteral)?;
        literals::parse_number_literal(literal).ok()
    }
}

#[cfg(test)]
mod tests {
    use slint::{Model, SharedString};
    use std::rc::Rc;

    use crate::preview::{properties, ui};
    use i_slint_editor_preview::test::loaded_document_cache;

    fn properties_at_position(
        source: &str,
        line: u32,
        character: u32,
    ) -> Option<(
        i_slint_editor_preview::ElementRcNode,
        Vec<properties::PropertyInformation>,
        i_slint_editor_preview::DocumentCache,
        lsp_types::Url,
    )> {
        let (dc, url, diag) = loaded_document_cache(source.to_string());
        for (u, diag) in diag.iter() {
            if diag.is_empty() {
                continue;
            }
            tracing::debug!("Diags for {u}");
            for d in diag {
                tracing::debug!("{d:#?}");
            }
        }
        if let Some((e, p)) =
            properties::tests::properties_at_position_in_cache(line, character, &dc, &url)
        {
            Some((e, p, dc, url))
        } else {
            None
        }
    }

    fn property_conversion_test(contents: &str, property_line: u32) -> ui::PropertyValue {
        tracing::debug!("\n\n\n{contents}:");
        let (e, pi, dc, _) = properties_at_position(contents, property_line, 30).unwrap();
        let test1 = pi.iter().find(|pi| pi.name == "test1").unwrap();
        super::map_property_to_ui(&dc, &e.element, test1, None).0
    }

    #[test]
    fn runtime_property_values_are_independent_of_source_and_preserve_fallbacks() {
        use slint_interpreter::Value;
        let source = property_conversion_test(
            "export component Test { in property <length> test1: 2rem; }",
            0,
        );
        let resolved = super::runtime_property_value(Some(Value::Number(40.0)), source.clone());
        assert_eq!(source.value_float, 2.0);
        assert_eq!(resolved.value_float, 40.0);
        assert!(resolved.code.is_empty());
        assert_eq!(resolved.visual_items.row_count(), 0);
        let fallback = ui::PropertyValue {
            value_float: 12.0,
            value_int: 400,
            value_string: "Fallback".into(),
            value_bool: true,
            ..Default::default()
        };
        assert_eq!(super::runtime_property_value(None, fallback.clone()), fallback);
        for value in [Value::Number(0.0), Value::String("".into()), Value::Bool(false)] {
            let result = super::runtime_property_value(Some(value.clone()), fallback.clone());
            assert!(result.value_resolved);
            match value {
                Value::Number(_) => assert_eq!(result.value_float, 0.0),
                Value::String(_) => assert!(result.value_string.is_empty()),
                Value::Bool(_) => assert!(!result.value_bool),
                _ => unreachable!(),
            }
        }
        let alignment = super::runtime_property_value(
            Some(Value::EnumerationValue("TextHorizontalAlignment".into(), "right".into())),
            fallback,
        );
        assert_eq!(alignment.value_int, 4);
    }

    #[test]
    fn runtime_properties_follow_selected_occurrence_and_font_defaults() {
        i_slint_backend_testing::init_no_event_loop();
        let code = r#"
export component Win inherits Window {
    width: 300px;
    height: 200px;
    default-font-family: "Test Family";
    default-font-size: 20px;
    default-font-weight: 600;
    in property <int> count: 2;
    Rectangle { width: 70px; height: 80px; }
    for index in count: Text {
        text: "Row";
        font-size: 2rem;
        color: index == 0 ? red : blue;
    }
}"#;
        let path = i_slint_compiler::source_path::SourcePath::new("/tmp/property-query.slint");
        let mut compiler = slint_interpreter::Compiler::default();
        compiler.compiler_configuration(i_slint_core::InternalToken).debug_hooks =
            Some(std::hash::RandomState::new());
        let compiled = spin_on::spin_on(compiler.build_from_source_path(
            code.into(),
            path.clone(),
            i_slint_core::InternalToken,
        ));
        assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics().collect::<Vec<_>>());
        let instance = compiled.components().next().unwrap().create().unwrap();
        let rectangle = instance
            .component_item(
                &path,
                code.find("Rectangle {").unwrap() as u32,
                0,
                i_slint_core::InternalToken,
            )
            .unwrap();
        for (name, expected) in [("width", 70.0), ("height", 80.0)] {
            assert_eq!(
                super::runtime_property(&rectangle, name),
                Some(slint_interpreter::Value::Number(expected))
            );
        }
        drop(rectangle);
        let offset = code.find("Text {").unwrap() as u32;
        for (index, color) in
            [slint::Color::from_rgb_u8(255, 0, 0), slint::Color::from_rgb_u8(0, 0, 255)]
                .into_iter()
                .enumerate()
        {
            let item =
                instance.component_item(&path, offset, index, i_slint_core::InternalToken).unwrap();
            assert_eq!(
                slint_interpreter::ComponentInstance::item_property(
                    &item,
                    "color",
                    i_slint_core::InternalToken
                ),
                Some(slint_interpreter::Value::Brush(color.into()))
            );
            let font = super::font_request(&item).unwrap();
            assert_eq!(font.family.as_deref(), Some("Test Family"));
            assert_eq!(font.pixel_size.unwrap().get(), 40.0);
            assert_eq!(font.weight, Some(600));
        }
        let current_instance = Rc::new(std::cell::RefCell::new(instance));
        let generation = Rc::pin(i_slint_core::Property::new(0));
        let lookups = Rc::new(std::cell::Cell::new(0));
        let cache = {
            let current_instance = current_instance.clone();
            let generation = generation.clone();
            let lookups = lookups.clone();
            let path = path.clone();
            super::selected_item_cache(move || {
                generation.as_ref().get();
                lookups.set(lookups.get() + 1);
                current_instance.borrow().component_item(
                    &path,
                    offset,
                    1,
                    i_slint_core::InternalToken,
                )
            })
        };
        assert!(cache.as_ref().get().upgrade().is_some());
        assert!(cache.as_ref().get().upgrade().is_some());
        assert_eq!(lookups.get(), 1);
        current_instance
            .borrow()
            .set_property("count", slint_interpreter::Value::Number(1.0))
            .unwrap();
        assert!(cache.as_ref().get().upgrade().is_none());
        current_instance
            .borrow()
            .set_property("count", slint_interpreter::Value::Number(3.0))
            .unwrap();
        let item = cache.as_ref().get().upgrade().unwrap();
        assert_eq!(lookups.get(), 3);
        drop(item);
        let old_item = cache.as_ref().get();
        let compiled = spin_on::spin_on(compiler.build_from_source_path(
            code.replace("Test Family", "Next Family"),
            path,
            i_slint_core::InternalToken,
        ));
        assert!(!compiled.has_errors());
        *current_instance.borrow_mut() = compiled.components().next().unwrap().create().unwrap();
        generation.set(1);
        assert!(old_item.upgrade().is_none());
        let item = cache.as_ref().get().upgrade().unwrap();
        assert_eq!(super::font_request(&item).unwrap().family.as_deref(), Some("Next Family"));
        assert_eq!(lookups.get(), 4);
    }

    #[test]
    fn test_property_bool() {
        let result =
            property_conversion_test(r#"export component Test { in property <bool> test1; }"#, 0);

        assert_eq!(result.value_kind, ui::PropertyValueKind::Boolean);
        assert_eq!(result.kind, ui::PropertyValueKind::Boolean);
        assert!(!result.value_bool);
        assert!(result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <bool> test1: true; }"#,
            0,
        );

        assert_eq!(result.value_kind, ui::PropertyValueKind::Boolean);
        assert_eq!(result.kind, ui::PropertyValueKind::Boolean);
        assert!(result.value_bool);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <bool> test1: false; }"#,
            0,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Boolean);
        assert_eq!(result.kind, ui::PropertyValueKind::Boolean);
        assert!(!result.value_bool);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <bool> test1: 1.1.round() == 1.1.floor(); }"#,
            0,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Boolean);
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert!(result.value_bool);
        assert!(!result.code.is_empty());
    }

    #[test]
    fn test_property_string() {
        let result =
            property_conversion_test(r#"export component Test { in property <string> test1; }"#, 0);
        assert_eq!(result.value_kind, ui::PropertyValueKind::String);
        assert_eq!(result.kind, ui::PropertyValueKind::String);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert!(!result.value_bool);
        assert!(result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: ""; }"#,
            0,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::String);
        assert_eq!(result.kind, ui::PropertyValueKind::String);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert!(!result.value_bool);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: "string"; }"#,
            0,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::String);
        assert_eq!(result.kind, ui::PropertyValueKind::String);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert!(!result.value_bool);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: "" + "test"; }"#,
            0,
        );

        assert_eq!(result.value_kind, ui::PropertyValueKind::String);
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert!(!result.value_bool);
        assert!(!result.code.is_empty());
    }

    #[test]
    fn test_property_tr_string() {
        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: @tr("Context" => "test"); }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::String);
        assert_eq!(result.value_string, "test");
        assert!(result.is_translatable);
        assert_eq!(result.tr_context, "Context");
        assert_eq!(result.tr_plural, "");
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test {
    property <int> test: 42;
    in property <string> test1: @tr("{n} string" | "{n} strings" % test);
}"#,
            2,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::String);
        assert!(result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "{n} strings");
        assert_eq!(result.tr_plural_expression, "test");
        assert_eq!(result.value_string, "{n} string");
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test {
    property <int> test: 42;
    in property <string> test1: @tr("{n} string" | "{n} strings" % self.test);
}"#,
            2,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::String);
        assert_eq!(result.value_kind, ui::PropertyValueKind::String);
        assert!(result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "{n} strings");
        assert_eq!(result.tr_plural_expression, "self.test");
        assert_eq!(result.value_string, "{n} string");
        assert!(!result.code.is_empty());

        // `15` is not a qualified name
        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: @tr("{n} string" | "{n} strings" % 15); }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::String);
        assert_eq!(result.value_kind, ui::PropertyValueKind::String);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_string, "15 strings");
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: @tr("width {}", self.width / 1px); }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_string, "width ");
        assert!(!result.code.is_empty());
    }

    #[test]
    fn test_property_enum() {
        let result = property_conversion_test(
            r#"export component Test { in property <ImageFit> test1: ImageFit.preserve; }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Enum);
        assert_eq!(result.value_string, "ImageFit");
        assert_eq!(result.value_int, 3);
        assert_eq!(result.default_selection, 0);
        assert!(!result.is_translatable);

        assert_eq!(result.visual_items.row_count(), 4);

        let result = property_conversion_test(
            r#"export component Test { in property <ImageFit> test1: ImageFit   .    /* abc */ preserve; }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Enum);
        assert_eq!(result.value_string, "ImageFit");
        assert_eq!(result.value_int, 3);
        assert_eq!(result.default_selection, 0);
        assert!(!result.is_translatable);

        assert_eq!(result.visual_items.row_count(), 4);

        let result = property_conversion_test(
            r#"export component Test { in property <ImageFit> test1: /* abc */ preserve; }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Enum);
        assert_eq!(result.value_string, "ImageFit");
        assert_eq!(result.value_int, 3);
        assert_eq!(result.default_selection, 0);
        assert!(!result.is_translatable);

        assert_eq!(result.visual_items.row_count(), 4);

        let result = property_conversion_test(
            r#"enum Foobar { foo, bar }
export component Test { in property <Foobar> test1: Foobar.bar; }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Enum);
        assert_eq!(result.value_string, "Foobar");
        assert_eq!(result.value_int, 1);
        assert_eq!(result.default_selection, 0);
        assert!(!result.is_translatable);

        assert_eq!(result.visual_items.row_count(), 2);
        assert_eq!(result.visual_items.row_data(0), Some(SharedString::from("foo")));
        assert_eq!(result.visual_items.row_data(1), Some(SharedString::from("bar")));

        let result = property_conversion_test(
            r#"enum Foobar { foo, bar }
export component Test { in property <Foobar> test1; }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Enum);
        assert_eq!(result.value_string, "Foobar");
        assert_eq!(result.value_int, 0); // default
        assert_eq!(result.default_selection, 0);
        assert!(!result.is_translatable);

        assert_eq!(result.visual_items.row_count(), 2);
        assert_eq!(result.visual_items.row_data(0), Some(SharedString::from("foo")));
        assert_eq!(result.visual_items.row_data(1), Some(SharedString::from("bar")));
    }

    #[test]
    fn test_property_float() {
        let result =
            property_conversion_test(r#"export component Test { in property <float> test1; }"#, 0);
        assert_eq!(result.kind, ui::PropertyValueKind::Float);
        assert_eq!(result.value_float, 0.0);

        let result = property_conversion_test(
            r#"export component Test { in property <float> test1: 42.0; }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Float);
        assert_eq!(result.value_float, 42.0);

        let result = property_conversion_test(
            r#"export component Test { in property <float> test1: +42.0; }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Float);
        assert_eq!(result.value_float, 42.0);

        let result = property_conversion_test(
            r#"export component Test { in property <float> test1: -42.0; }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Float);
        assert_eq!(result.value_float, -42.0);

        let result = property_conversion_test(
            r#"export component Test { in property <float> test1: 42.0 * 23.0; }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert_eq!(result.value_float, 966.0);
    }

    #[test]
    fn test_property_integer() {
        let result =
            property_conversion_test(r#"export component Test { in property <int> test1; }"#, 0);
        assert_eq!(result.value_kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.value_int, 0);

        let result = property_conversion_test(
            r#"export component Test { in property <int> test1: 42; }"#,
            1,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.value_int, 42);

        let result = property_conversion_test(
            r#"export component Test { in property <int> test1: +42; }"#,
            1,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.value_int, 42);

        let result = property_conversion_test(
            r#"export component Test { in property <int> test1: -42; }"#,
            1,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.value_int, -42);

        let result = property_conversion_test(
            r#"export component Test { in property <int> test1: 42 * 23; }"#,
            0,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert_eq!(result.value_int, 966);
    }

    #[test]
    fn test_property_color() {
        let result =
            property_conversion_test(r#"export component Test { in property <color> test1; }"#, 0);
        assert_eq!(result.value_kind, ui::PropertyValueKind::Color);
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.display_string, "#00000000");
        assert!(result.value_string.is_empty());
        assert_eq!(result.value_brush.color().red(), 0);
        assert_eq!(result.value_brush.color().green(), 0);
        assert_eq!(result.value_brush.color().blue(), 0);
        assert_eq!(result.value_brush.color().alpha(), 0);

        let result = property_conversion_test(
            r#"export component Test { in property <color> test1: #10203040; }"#,
            1,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Color);
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.display_string, "#10203040");
        assert!(result.value_string.is_empty());
        assert_eq!(result.value_brush.color().red(), 0x10);
        assert_eq!(result.value_brush.color().green(), 0x20);
        assert_eq!(result.value_brush.color().blue(), 0x30);
        assert_eq!(result.value_brush.color().alpha(), 0x40);

        let result = property_conversion_test(
            r#"export component Test { in property <color> test1: #10203040.darker(0.5); }"#,
            1,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Color);
        assert_eq!(result.kind, ui::PropertyValueKind::Code);

        let result = property_conversion_test(
            r#"export component Test { in property <color> test1: Colors.red; }"#,
            0,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Color);
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.display_string, "Colors.red");
        assert_eq!(result.value_string, "Colors.red");
        assert_eq!(result.accessor_path, "Colors.red");
        assert_eq!(result.value_brush.color().red(), 0xff);
        assert_eq!(result.value_brush.color().green(), 0x00);
        assert_eq!(result.value_brush.color().blue(), 0x00);
        assert_eq!(result.value_brush.color().alpha(), 0xff);

        let result = property_conversion_test(
            r#"export component Test { in property <color> test1: red; }"#,
            0,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Color);
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_string, "red");
        assert_eq!(result.accessor_path, "red");
        assert_eq!(result.value_brush.color().red(), 0xff);
        assert_eq!(result.value_brush.color().green(), 0x00);
        assert_eq!(result.value_brush.color().blue(), 0x00);
        assert_eq!(result.value_brush.color().alpha(), 0xff);

        let result = property_conversion_test(
            r#"global Foo {
                out property <color> red: blue;
            }
            export component Test { in property <color> test1: Foo.red; }"#,
            3,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Color);
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_string, "Foo.red");
        assert_eq!(result.accessor_path, "Foo.red");
        assert_eq!(result.value_brush.color().red(), 0x00);
        assert_eq!(result.value_brush.color().green(), 0x00);
        assert_eq!(result.value_brush.color().blue(), 0xff);
        assert_eq!(result.value_brush.color().alpha(), 0xff);

        let result = property_conversion_test(
            r#"struct Bar {
                foo: color,
            }
            global Foo {
                out property <Bar> s: { foo: Colors.blue };
            }
            export component Test { in property <color> test1: Foo.s.foo; }"#,
            6,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Color);
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_string, "Foo.s.foo");
        assert_eq!(result.value_brush.color().red(), 0x00);
        assert_eq!(result.value_brush.color().green(), 0x00);
        assert_eq!(result.value_brush.color().blue(), 0xff);
        assert_eq!(result.value_brush.color().alpha(), 0xff);

        let result = property_conversion_test(
            r#"struct Bar {
                bar: color,
            }
            struct Baz {
                baz: Bar,
            }
            global Foo {
                out property <Baz> s: { baz: { bar: Colors.blue } };
            }
            export component Test { in property <color> test1: Foo.s.baz.bar; }"#,
            9,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Color);
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_string, "Foo.s.baz.bar");
        assert_eq!(result.value_brush.color().red(), 0x00);
        assert_eq!(result.value_brush.color().green(), 0x00);
        assert_eq!(result.value_brush.color().blue(), 0xff);
        assert_eq!(result.value_brush.color().alpha(), 0xff);

        let result = property_conversion_test(
            r#"struct Bar {
                bar: color,
            }
            struct Baz {
                baz: Bar,
            }
            global Foo2 {
                out property <Baz> test: Foo.s;
            }
            global Foo {
                out property <Baz> s: { baz: { bar: Colors.blue } };
            }
            export component Test { in property <color> test1: Foo.s.baz.bar; }"#,
            12,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Color);
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_string, "Foo.s.baz.bar");
        assert_eq!(result.value_brush.color().red(), 0x00);
        assert_eq!(result.value_brush.color().green(), 0x00);
        assert_eq!(result.value_brush.color().blue(), 0xff);
        assert_eq!(result.value_brush.color().alpha(), 0xff);

        let result = property_conversion_test(
            r#"struct Bar {
                bar: color,
            }
            struct Baz {
                baz: Bar,
            }
            global Foo2 {
                in property <int> index;
                out property <Baz> test: index == 0 ? Foo.s : FooBar.s;
            }
            global Foo {
                out property <Baz> s: { baz: { bar: Colors.blue } };
            }
            global FooBar {
                out property <Baz> s: { baz: { bar: Colors.green } };
            }
            export component Test { in property <color> test1: Foo.s.baz.bar; }"#,
            16,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Color);
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_string, "Foo.s.baz.bar");
        assert_eq!(result.value_brush.color().red(), 0x00);
        assert_eq!(result.value_brush.color().green(), 0x00);
        assert_eq!(result.value_brush.color().blue(), 0xff);
        assert_eq!(result.value_brush.color().alpha(), 0xff);

        let result = property_conversion_test(
            r#"export component Test {
            in property <int> foo;
            in property <color> test1: foo == 0 ? red : blue;
            }"#,
            2,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);

        let result = property_conversion_test(
            r#"global Foo {
                in property <int> foo;
                out property <color> red: foo == 0 ? blue : red;
            }
            export component Test { in property <color> test1: Foo.red; }"#,
            4,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Color);
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_string, "Foo.red");
        assert!(!result.value_resolved);
        assert_eq!(result.value_brush.color(), slint::Color::default());

        let result = property_conversion_test(
            r#"struct Bar {
                foo: color,
            }
            global Foo {
                in property <int> foo;
                out property <Bar> s: foo == 0 ? { foo: Colors.blue } : { foo: Colors.red };
            }
            export component Test { in property <color> test1: Foo.s.foo; }"#,
            7,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Color);
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_string, "Foo.s.foo");
        assert!(!result.value_resolved);
        assert_eq!(result.value_brush.color(), slint::Color::default());

        let result = property_conversion_test(
            r#"struct Bar {
                bar: color,
            }
            struct Baz {
                baz: Bar,
            }
            global Foo {
                in property <int> foo;
                out property <Baz> s: foo == 0 ? { baz: { bar: Colors.blue } } : { baz: { bar: Colors.blue } };
            }
            export component Test { in property <color> test1: Foo.s.baz.bar; }"#,
            10,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Color);
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_string, "Foo.s.baz.bar");
        assert!(!result.value_resolved);
        assert_eq!(result.value_brush.color(), slint::Color::default());
    }

    #[test]
    fn test_property_brush() {
        use crate::preview::ui::GradientStop;
        let result =
            property_conversion_test(r#"export component Test { in property <brush> test1; }"#, 0);
        assert_eq!(result.value_kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.kind, ui::PropertyValueKind::Brush);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert!(matches!(result.brush_kind, ui::BrushKind::Solid));
        assert_eq!(result.display_string, "#00000000");
        assert!(result.value_string.is_empty());
        assert_eq!(result.value_brush.color().red(), 0);
        assert_eq!(result.value_brush.color().green(), 0);
        assert_eq!(result.value_brush.color().blue(), 0);
        assert_eq!(result.value_brush.color().alpha(), 0);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: #10203040; }"#,
            1,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.kind, ui::PropertyValueKind::Brush);
        assert!(matches!(result.brush_kind, ui::BrushKind::Solid));
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.display_string, "#10203040");
        assert!(result.value_string.is_empty());
        assert_eq!(result.value_brush.color().red(), 0x10);
        assert_eq!(result.value_brush.color().green(), 0x20);
        assert_eq!(result.value_brush.color().blue(), 0x30);
        assert_eq!(result.value_brush.color().alpha(), 0x40);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: #10203040.darker(0.5); }"#,
            1,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert!(matches!(result.brush_kind, ui::BrushKind::Solid));
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.display_string, "#0b152040");
        assert!(result.value_string.is_empty());
        assert_eq!(result.value_brush.color().red(), 0x0b);
        assert_eq!(result.value_brush.color().green(), 0x15);
        assert_eq!(result.value_brush.color().blue(), 0x20);
        assert_eq!(result.value_brush.color().alpha(), 0x40);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: Colors.red; }"#,
            0,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.kind, ui::PropertyValueKind::Brush);
        assert!(matches!(result.brush_kind, ui::BrushKind::Solid));
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.display_string, "Colors.red");
        assert_eq!(result.value_string, "Colors.red");
        assert_eq!(result.accessor_path, "Colors.red");
        assert_eq!(result.value_brush.color().red(), 0xff);
        assert_eq!(result.value_brush.color().green(), 0);
        assert_eq!(result.value_brush.color().blue(), 0);
        assert_eq!(result.value_brush.color().alpha(), 0xff);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @linear-gradient(90deg, #3f87a6 0%, #ebf8e1 50%, #f69d3c 100%); }"#,
            1,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.kind, ui::PropertyValueKind::Brush);
        assert!(matches!(result.brush_kind, ui::BrushKind::Linear));

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @radial-gradient(circle, #f00 0%, #0f0 50%, #00f 100%); }"#,
            1,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.kind, ui::PropertyValueKind::Brush);
        assert!(matches!(result.brush_kind, ui::BrushKind::Radial));

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @radial-gradient(ellipse 40px 20px, #f00 0%, #00f 100%); }"#,
            1,
        );
        assert!(matches!(result.brush_kind, ui::BrushKind::Radial));
        assert!(!result.value_resolved);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @radial-gradient(ellipse 40px 40px, #f00 0%, #00f 100%); }"#,
            1,
        );
        assert!(matches!(result.brush_kind, ui::BrushKind::Radial));
        assert!(!result.value_resolved);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @radial-gradient(circle 0px, #f00 0%, #00f 100%); }"#,
            1,
        );
        assert!(result.value_resolved);
        assert!(result.fill.custom_radius);
        assert_eq!(result.fill.radius, 0.);

        let result = property_conversion_test(
            r#"global Foo {
                out property <brush> ellipse: @radial-gradient(ellipse, #f00 0%, #00f 100%);
            }
            export component Test { in property <brush> test1: Foo.ellipse; }"#,
            3,
        );
        assert!(matches!(result.brush_kind, ui::BrushKind::Radial));
        assert!(!result.value_resolved);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @linear-gradient(90deg, #3f87a6 0%, #ebf8e1 50% - 10%, #f69d3c 100%); }"#,
            1,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert!(matches!(result.brush_kind, ui::BrushKind::Linear));

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @radial-gradient(circle, #f00 0%, #0f0 50% - 10%, #00f 100%); }"#,
            1,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert!(matches!(result.brush_kind, ui::BrushKind::Radial));

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @conic-gradient(white 36deg, #239 126deg, red 306deg); }"#,
            1,
        );
        assert_eq!(result.value_kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.brush_kind, ui::BrushKind::Conic);
        assert_eq!(
            result.gradient_stops.iter().collect::<Vec<_>>(),
            [
                GradientStop { color: slint::Color::from_rgb_u8(0xff, 0xff, 0xff,), position: 0.0 },
                GradientStop {
                    color: slint::Color::from_rgb_u8(0xff, 0xff, 0xff,),
                    position: 36.0 / 360.0
                },
                GradientStop {
                    color: slint::Color::from_rgb_u8(0x22, 0x33, 0x99),
                    position: 126.0 / 360.0
                },
                GradientStop {
                    color: slint::Color::from_rgb_u8(0xff, 0x00, 0x00),
                    position: 306.0 / 360.0
                },
                GradientStop {
                    color: slint::Color::from_rgb_u8(0xff, 0x00, 0x00),
                    position: 360.0 / 360.0
                },
            ]
        );
        assert_eq!(result.code, "@conic-gradient(white 36deg, #239 126deg, red 306deg)");
    }

    #[test]
    fn radial_shape_follows_source_with_equal_radii() {
        for (gradient, ellipse, radii) in [
            ("@radial-gradient(circle 40px, red, blue)", false, (40., 40.)),
            ("@radial-gradient(ellipse 40px 40px, red, blue)", true, (40., 40.)),
            ("@radial-gradient(ellipse 40px 20px, red, blue)", true, (40., 20.)),
        ] {
            let source =
                format!("export component Test {{ in property <brush> test1: {gradient}; }}");
            let result = property_conversion_test(&source, 0);
            assert!(result.value_resolved);
            assert_eq!(result.fill.kind, ui::BrushKind::Radial);
            assert_eq!(result.fill.radial_ellipse, ellipse);
            assert_eq!((result.fill.radius, result.fill.radius_y), radii);
        }
    }

    #[test]
    fn test_property_units() {
        let result =
            property_conversion_test(r#"export component Test { in property <length> test1; }"#, 0);

        assert_eq!(result.value_kind, ui::PropertyValueKind::Float);
        assert_eq!(result.kind, ui::PropertyValueKind::Float);
        assert_eq!(result.value_int, 0);
        assert_eq!(
            result.visual_items.row_data(result.default_selection as usize),
            Some("px".into())
        );
        assert_eq!(result.visual_items.row_data(result.value_int as usize), Some("px".into()));
        let length_row_count = result.visual_items.row_count();
        assert!(length_row_count > 2);

        let result = property_conversion_test(
            r#"export component Test { in property <duration> test1: 25s; }"#,
            1,
        );

        assert_eq!(result.value_kind, ui::PropertyValueKind::Float);
        assert_eq!(result.kind, ui::PropertyValueKind::Float);
        assert_eq!(result.value_float, 25.0);
        assert_eq!(
            result.visual_items.row_data(result.default_selection as usize),
            Some("ms".into())
        );
        assert_eq!(result.visual_items.row_data(result.value_int as usize), Some("s".into()));
        assert_eq!(result.visual_items.row_count(), 2); // ms, s

        let result = property_conversion_test(
            r#"export component Test { in property <physical-length> test1: 1.5phx; }"#,
            1,
        );

        assert_eq!(result.value_kind, ui::PropertyValueKind::Float);
        assert_eq!(result.kind, ui::PropertyValueKind::Float);
        assert_eq!(result.value_float, 1.5);
        assert_eq!(
            result.visual_items.row_data(result.default_selection as usize),
            Some("phx".into())
        );
        assert_eq!(result.visual_items.row_data(result.value_int as usize), Some("phx".into()));
        assert!(result.visual_items.row_count() > 1); // More than just physical length

        let result = property_conversion_test(
            r#"export component Test { in property <relative-font-size> test1: 1.5rem; }"#,
            1,
        );

        assert_eq!(result.value_kind, ui::PropertyValueKind::Float);
        assert_eq!(result.kind, ui::PropertyValueKind::Float);
        assert_eq!(result.value_float, 1.5);
        assert_eq!(
            result.visual_items.row_data(result.default_selection as usize),
            Some("rem".into())
        );
        assert_eq!(result.visual_items.row_data(result.value_int as usize), Some("rem".into()));
        assert!(result.visual_items.row_count() > 1); // More than just physical length

        let result = property_conversion_test(
            r#"export component Test { in property <angle> test1: 1.5turns + 1.3deg; }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
    }

    #[test]
    fn test_property_with_default_values() {
        let source = r#"
import { Button } from "std-widgets.slint";
component MyButton inherits Button {
    text: "Ok";
    in property <color> color: red;
    in property alias <=> self.xxx;
    property <length> xxx: 45cm;
}
export component X {
    MyButton {
        /*CURSOR*/
    }
}
        "#;

        let (dc, url, _diag) = loaded_document_cache(source.to_string());
        let element = dc
            .element_at_offset(&url, (source.find("/*CURSOR*/").expect("cursor") as u32).into())
            .unwrap();
        let pi = super::properties::get_properties(&element, super::properties::LayoutKind::None);

        let prop = pi.iter().find(|pi| pi.name == "visible").unwrap();
        let result = super::map_property_to_ui(&dc, &element.element, prop, None).0;
        assert_eq!(result.kind, ui::PropertyValueKind::Boolean);
        assert!(result.value_bool);

        let prop = pi.iter().find(|pi| pi.name == "enabled").unwrap();
        let result = super::map_property_to_ui(&dc, &element.element, prop, None).0;
        assert_eq!(result.kind, ui::PropertyValueKind::Boolean);
        assert!(result.value_bool);

        let prop = pi.iter().find(|pi| pi.name == "text").unwrap();
        let result = super::map_property_to_ui(&dc, &element.element, prop, None).0;
        assert_eq!(result.kind, ui::PropertyValueKind::String);
        assert_eq!(result.value_string, "Ok");

        let prop = pi.iter().find(|pi| pi.name == "alias").unwrap();
        let result = super::map_property_to_ui(&dc, &element.element, prop, None).0;
        assert_eq!(result.kind, ui::PropertyValueKind::Float);
        assert!(result.value_float >= 45.);
        assert_eq!(result.visual_items.row_data(result.value_int as usize).unwrap(), "px");

        let prop = pi.iter().find(|pi| pi.name == "color").unwrap();
        let result = super::map_property_to_ui(&dc, &element.element, prop, None).0;
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert_eq!(
            result.value_brush,
            slint::Brush::SolidColor(slint::Color::from_rgb_u8(255, 0, 0))
        );
    }

    #[test]
    fn test_property_with_default_values_loop() {
        let source = r#"
component Abc {
        // This should be an error, not a infinite loop/hang
        in property <length> some_loop <=> r.border-width;
        r:= Rectangle {
            property <length> some_loop <=> root.some_loop;
            border-width <=> some_loop;
        }
}
export component X {
    Abc {
        /*CURSOR*/
    }
}
        "#;

        let (dc, url, _diag) = loaded_document_cache(source.to_string());

        let element = dc
            .element_at_offset(&url, (source.find("/*CURSOR*/").expect("cursor") as u32).into())
            .unwrap();
        let pi = super::properties::get_properties(&element, super::properties::LayoutKind::None);

        let prop = pi.iter().find(|pi| pi.name == "visible").unwrap();
        let result = super::map_property_to_ui(&dc, &element.element, prop, None).0;

        assert_eq!(result.kind, ui::PropertyValueKind::Boolean);
        assert!(result.value_bool);
    }

    #[test]
    fn test_property_referencing_global() {
        let source = r#"
global Other {
    in-out property <brush> test: @linear-gradient(90deg, #0ff, #f0f, #ff0);
}
component Abc {
    in-out property <brush> local-test-brush: @linear-gradient(90deg, #0ff, #f0f, #ff0);
    in-out property <brush> test-brush1 <=> Other.test;
    in-out property <brush> test-brush2: Other.test;
    in-out property <brush> test-brush3: true ? Other.test : self.local-test-brush;
    /*CURSOR*/
}
        "#;

        let (dc, url, _diag) = loaded_document_cache(source.to_string());

        let element = dc
            .element_at_offset(&url, (source.find("/*CURSOR*/").expect("cursor") as u32).into())
            .unwrap();
        let pi = super::properties::get_properties(&element, super::properties::LayoutKind::None);

        let prop = pi.iter().find(|pi| pi.name == "local-test-brush").unwrap();
        let result = super::map_property_to_ui(&dc, &element.element, prop, None).0;
        assert_eq!(result.kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.value_kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.brush_kind, ui::BrushKind::Linear);

        let prop = pi.iter().find(|pi| pi.name == "test-brush1").unwrap();
        let result = super::map_property_to_ui(&dc, &element.element, prop, None).0;
        assert_eq!(result.kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.value_kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.brush_kind, ui::BrushKind::Linear);

        let prop = pi.iter().find(|pi| pi.name == "test-brush2").unwrap();
        let result = super::map_property_to_ui(&dc, &element.element, prop, None).0;
        assert_eq!(result.kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.value_kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.brush_kind, ui::BrushKind::Linear);

        let prop = pi.iter().find(|pi| pi.name == "test-brush3").unwrap();
        let result = super::map_property_to_ui(&dc, &element.element, prop, None).0;
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert_eq!(result.value_kind, ui::PropertyValueKind::Brush);
        assert_eq!(result.brush_kind, ui::BrushKind::Linear);
    }

    #[test]
    fn test_property_recursion() {
        let result = property_conversion_test(
            r#"export component Test { in property <int> test1 : test1 + 1; }"#,
            0,
        );

        assert_eq!(result.value_kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert_eq!(result.value_int, 0);
        assert_eq!(result.code, "test1 + 1");

        let result = property_conversion_test(
            r#"export component Test {
                in property <int> test1: test2;
                in property <float> test2: test1;
            }"#,
            1,
        );

        assert_eq!(result.value_kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.value_int, 0);
        assert_eq!(result.code, "test2");
    }
}
