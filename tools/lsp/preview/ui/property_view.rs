// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{collections::HashMap, rc::Rc};

use itertools::Itertools;
use lsp_types::Url;
use smol_str::SmolStr;

use i_slint_compiler::{
    expression_tree, langtype, literals, object_tree,
    parser::{syntax_nodes, SyntaxKind, TextRange},
};

use slint::{SharedString, VecModel};

use crate::{
    common,
    preview::{properties, ui},
    util,
};

pub fn map_properties_to_ui(
    document_cache: &common::DocumentCache,
    properties: Option<properties::QueryPropertyResponse>,
) -> Option<(
    ui::ElementInformation,
    HashMap<SmolStr, ui::PropertyDeclaration>,
    ui::PropertyGroupModel,
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
        let defined_at =
            map_property_definition(&pi.defined_at).unwrap_or(ui::PropertyDefinition {
                definition_range: ui::Range { start: 0, end: 0 },
                selection_range: ui::Range { start: 0, end: 0 },
                expression_range: ui::Range { start: 0, end: 0 },
                expression_value: String::new().into(),
            });
        let declared_at =
            map_property_declaration(document_cache, &pi.declared_at, defined_at.clone())
                .unwrap_or(ui::PropertyDeclaration {
                    defined_at,
                    source_path: String::new().into(),
                    source_version: -1,
                    range: ui::Range { start: 0, end: 0 },
                });

        declarations.insert(pi.name.clone(), declared_at);

        let value = simplify_value(document_cache, pi);

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

    Some((
        ui::ElementInformation {
            id: element.id.as_str().into(),
            type_name: element.type_name.as_str().into(),
            source_uri,
            source_version,
            range: ui::to_ui_range(element.range)?,
        },
        declarations,
        Rc::new(VecModel::from(
            keys.iter()
                .map(|k| ui::PropertyGroup {
                    group_name: k.0.as_str().into(),
                    properties: Rc::new(VecModel::from({
                        let mut v = property_groups.remove(k).unwrap();
                        v.sort_by(|a, b| match a.display_priority.cmp(&b.display_priority) {
                            Ordering::Less => Ordering::Less,
                            Ordering::Equal => a.name.cmp(&b.name),
                            Ordering::Greater => Ordering::Greater,
                        });
                        v
                    }))
                    .into(),
                })
                .collect::<Vec<_>>(),
        ))
        .into(),
    ))
}

fn map_property_declaration(
    document_cache: &common::DocumentCache,
    declared_at: &Option<properties::DeclarationInformation>,
    defined_at: ui::PropertyDefinition,
) -> Option<ui::PropertyDeclaration> {
    let da = declared_at.as_ref()?;
    let source_version = document_cache.document_version_by_path(&da.path).unwrap_or(-1);
    let pos = TextRange::new(da.start_position, da.start_position);

    Some(ui::PropertyDeclaration {
        defined_at,
        source_path: da.path.to_string_lossy().to_string().into(),
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
) -> Option<(f64, i_slint_compiler::expression_tree::Unit)> {
    if let Some(unary) = &node.UnaryOpExpression() {
        let factor = match unary.first_token().unwrap().text() {
            "-" => -1.0,
            "+" => 1.0,
            _ => return None,
        };
        convert_number_literal(&unary.Expression()).map(|(v, u)| (factor * v, u))
    } else {
        let literal = node.child_text(SyntaxKind::NumberLiteral)?;
        let expr = literals::parse_number_literal(literal).ok()?;

        match expr {
            i_slint_compiler::expression_tree::Expression::NumberLiteral(value, unit) => {
                Some((value, unit))
            }
            _ => None,
        }
    }
}

fn extract_color(
    expression: &syntax_nodes::Expression,
    kind: ui::PropertyValueKind,
    value: &mut ui::PropertyValue,
) -> bool {
    if let Some(text) = expression.child_text(SyntaxKind::ColorLiteral) {
        if let Some(color) = ui::string_to_color(&text) {
            value.display_string = text.as_str().into();
            value.kind = kind;
            value.value_brush = slint::Brush::SolidColor(color);
            value.gradient_stops =
                Rc::new(VecModel::from(vec![ui::GradientStop { color, position: 0.5 }])).into();
            return true;
        }
    }
    false
}

fn set_default_brush(
    kind: ui::PropertyValueKind,
    def_val: Option<&expression_tree::Expression>,
    value: &mut ui::PropertyValue,
) {
    use expression_tree::Expression;
    value.kind = kind;
    if let Some(mut def_val) = def_val {
        if let Expression::Cast { from, .. } = def_val {
            def_val = from;
        }
        if let Expression::NumberLiteral(v, _) = def_val {
            value.value_brush = slint::Brush::SolidColor(slint::Color::from_argb_encoded(*v as _));
            return;
        }
    }
    value.brush_kind = ui::BrushKind::Solid;
    let text = "#00000000";
    let color = ui::string_to_color(text).unwrap();
    value.gradient_stops =
        Rc::new(VecModel::from(vec![ui::GradientStop { color, position: 0.5 }])).into();
    value.display_string = text.into();
    value.value_brush = slint::Brush::SolidColor(color);
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
    if tr_node.Expression().next().is_none() && (plural.is_empty() || plural_expression.is_some()) {
        value.kind = ui::PropertyValueKind::String;
        value.is_translatable = true;
        value.tr_context = context.as_str().into();
        value.tr_plural = plural.as_str().into();
        value.tr_plural_expression = plural_expression.unwrap_or_default().into();
        value.value_string = text.as_str().into();
    }
}

fn is_gradient_too_complex(expression: &syntax_nodes::AtGradient) -> bool {
    let is_radial = expression
        .child_token(SyntaxKind::Identifier)
        .is_some_and(|t| t.text().trim().replace('_', "-") == "radial-gradient");

    expression.Expression().any(|e| {
        match e
            .children_with_tokens()
            .next()
            .expect("An expression needs to contain at least one node or token")
        {
            i_slint_compiler::parser::NodeOrToken::Node(syntax_node) => match syntax_node.kind() {
                SyntaxKind::QualifiedName => {
                    if is_radial && syntax_node.text().to_string().trim() == "circle" {
                        false
                    } else if let Some(first_identifier) =
                        syntax_node.child_token(SyntaxKind::Identifier)
                    {
                        first_identifier.text().trim() != "Colors"
                    } else {
                        true
                    }
                }
                _ => true,
            },
            i_slint_compiler::parser::NodeOrToken::Token(syntax_token) => {
                !matches!(syntax_token.kind(), SyntaxKind::NumberLiteral | SyntaxKind::ColorLiteral)
            }
        }
    })
}

#[derive(Default)]
struct NumberValue {
    value: f64,
    unit: expression_tree::Unit,
    normalized_value: f64,
}

fn extract_number_from_expression_tree(
    expression: &expression_tree::Expression,
) -> Option<NumberValue> {
    match expression {
        expression_tree::Expression::NumberLiteral(v, u) => {
            Some(NumberValue { value: *v, unit: *u, normalized_value: u.normalize(*v) })
        }
        expression_tree::Expression::Cast { from, to } => {
            if *to == langtype::Type::Float32 {
                extract_number_from_expression_tree(from.as_ref())
            } else {
                None
            }
        }
        expression_tree::Expression::BinaryExpression { lhs, rhs, op } => {
            let lhs = extract_number_from_expression_tree(lhs.as_ref())?;
            let rhs = extract_number_from_expression_tree(rhs.as_ref())?;

            if lhs.unit != expression_tree::Unit::None
                && rhs.unit == expression_tree::Unit::None
                && *op == '*'
                && (lhs.value * rhs.value == lhs.normalized_value
                    || lhs.unit == expression_tree::Unit::Percent && rhs.value == 0.01)
            {
                // This is just a unit conversion
                Some(NumberValue {
                    value: lhs.value,
                    unit: lhs.unit,
                    normalized_value: lhs.value * rhs.value,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

fn extract_color_from_expression_tree(
    expression: &expression_tree::Expression,
) -> Option<slint::Color> {
    match expression {
        expression_tree::Expression::NumberLiteral(v, _) => {
            Some(slint::Color::from_argb_encoded(*v as u32))
        }
        expression_tree::Expression::Cast { from, to } => {
            if *to == langtype::Type::Color {
                extract_color_from_expression_tree(from.as_ref())
            } else {
                None
            }
        }
        _ => None,
    }
}

fn extract_gradient(
    document_cache: &common::DocumentCache,
    expression: &syntax_nodes::AtGradient,
    value: &mut ui::PropertyValue,
) {
    if is_gradient_too_complex(expression) {
        return;
    }

    let Some(resolved_expression) =
        util::with_lookup_ctx(document_cache, expression.clone().into(), |ctx| {
            expression_tree::Expression::from_at_gradient(expression.clone(), ctx)
        })
    else {
        return;
    };

    fn parse_stops(
        stops: &[(expression_tree::Expression, expression_tree::Expression)],
    ) -> Vec<ui::GradientStop> {
        stops
            .iter()
            .filter_map(|(c, p)| {
                Some(ui::GradientStop {
                    color: extract_color_from_expression_tree(c)?,
                    position: extract_number_from_expression_tree(p)?.normalized_value as f32,
                })
            })
            .collect::<Vec<_>>()
    }

    let stops = match resolved_expression {
        expression_tree::Expression::LinearGradient { angle, stops } => {
            let angle = extract_number_from_expression_tree(angle.as_ref());
            value.value_float = angle.unwrap_or_default().normalized_value as f32;
            value.display_string = "Linear Gradient".into();
            value.brush_kind = ui::BrushKind::Linear;

            parse_stops(&stops)
        }
        expression_tree::Expression::RadialGradient { stops } => {
            value.display_string = "Radial Gradient".into();
            value.brush_kind = ui::BrushKind::Radial;
            parse_stops(&stops)
        }

        _ => vec![],
    };

    value.gradient_stops = Rc::new(VecModel::from(stops)).into();
    value.value_brush = super::create_brush(
        value.brush_kind,
        value.value_float,
        slint::Color::default(),
        value.gradient_stops.clone(),
    );
    value.kind = ui::PropertyValueKind::Brush;
}

fn simplify_value(
    document_cache: &common::DocumentCache,
    prop_info: &properties::PropertyInformation,
) -> ui::PropertyValue {
    use i_slint_compiler::expression_tree::Unit;
    use langtype::Type;

    let code_block_or_expression =
        prop_info.defined_at.as_ref().map(|da| da.code_block_or_expression.clone());
    let expression = code_block_or_expression.as_ref().and_then(|cbe| cbe.expression());

    let mut value = ui::PropertyValue {
        code: code_block_or_expression
            .as_ref()
            .map(|cbe| cbe.text().to_string())
            .unwrap_or_default()
            .into(),
        kind: ui::PropertyValueKind::Code,
        ..Default::default()
    };

    let def_val = prop_info.default_value.as_ref();

    match &prop_info.ty {
        Type::Float32 => extract_value_with_unit(&expression, def_val, &[], &mut value),
        Type::Duration => {
            extract_value_with_unit(&expression, def_val, &[Unit::S, Unit::Ms], &mut value)
        }
        Type::PhysicalLength | Type::LogicalLength | Type::Rem => extract_value_with_unit(
            &expression,
            def_val,
            &[Unit::Px, Unit::Cm, Unit::Mm, Unit::In, Unit::Pt, Unit::Phx, Unit::Rem],
            &mut value,
        ),
        Type::Angle => extract_value_with_unit(
            &expression,
            def_val,
            &[Unit::Deg, Unit::Grad, Unit::Turn, Unit::Rad],
            &mut value,
        ),
        Type::Percent => {
            extract_value_with_unit(&expression, def_val, &[Unit::Percent], &mut value)
        }
        Type::Int32 => {
            if let Some(expression) = expression {
                if let Some((v, unit)) = convert_number_literal(&expression) {
                    if unit == i_slint_compiler::expression_tree::Unit::None {
                        value.display_string = slint::format!("{v}");
                        value.kind = ui::PropertyValueKind::Integer;
                        value.value_int = v as i32;
                    }
                }
            } else if value.code.is_empty() {
                value.kind = ui::PropertyValueKind::Integer;
            }
        }
        Type::Color => {
            if let Some(expression) = expression {
                extract_color(&expression, ui::PropertyValueKind::Color, &mut value);
                // TODO: Extract `Foo.bar` as Palette `Foo`, entry `bar`.
                // This makes no sense right now, as we have no way to get any
                // information on the palettes.
            } else if value.code.is_empty() {
                set_default_brush(ui::PropertyValueKind::Color, def_val, &mut value);
            }
        }
        Type::Brush => {
            if let Some(expression) = expression {
                if let Some(gradient) = expression.AtGradient() {
                    extract_gradient(document_cache, &gradient, &mut value);
                } else {
                    extract_color(&expression, ui::PropertyValueKind::Brush, &mut value);
                }
            } else if value.code.is_empty() {
                set_default_brush(ui::PropertyValueKind::Brush, def_val, &mut value);
            }
        }
        Type::Bool => {
            if let Some(expression) = expression {
                let qualified_name =
                    expression.QualifiedName().map(|qn| qn.text().to_string()).unwrap_or_default();
                if ["true", "false"].contains(&qualified_name.as_str()) {
                    value.kind = ui::PropertyValueKind::Boolean;
                    value.value_bool = &qualified_name == "true";
                }
            } else if value.code.is_empty() {
                if let Some(expression_tree::Expression::BoolLiteral(v)) = def_val {
                    value.value_bool = *v;
                }
                value.kind = ui::PropertyValueKind::Boolean;
            }
        }
        Type::String => {
            if let Some(expression) = &expression {
                if let Some(text) = expression
                    .child_text(SyntaxKind::StringLiteral)
                    .and_then(|s| i_slint_compiler::literals::unescape_string(&s))
                {
                    value.kind = ui::PropertyValueKind::String;
                    value.value_string = text.as_str().into();
                } else if let Some(tr_node) = &expression.AtTr() {
                    extract_tr_data(tr_node, &mut value)
                }
            } else if value.code.is_empty() {
                if let Some(expression_tree::Expression::StringLiteral(v)) = def_val {
                    value.value_string = v.as_str().into();
                }
                value.kind = ui::PropertyValueKind::String;
            }
        }
        Type::Enumeration(enumeration) => {
            value.kind = ui::PropertyValueKind::Enum;
            value.value_string = enumeration.name.as_str().into();
            value.default_selection = i32::try_from(enumeration.default_value).unwrap_or_default();
            value.visual_items = Rc::new(VecModel::from(
                enumeration
                    .values
                    .iter()
                    .map(|s| SharedString::from(s.as_str()))
                    .collect::<Vec<_>>(),
            ))
            .into();

            if let Some(expression) = expression {
                if let Some(text) = expression
                    .child_node(SyntaxKind::QualifiedName)
                    .map(|n| i_slint_compiler::object_tree::QualifiedTypeName::from_node(n.into()))
                    .map(|n| {
                        let n_str = n.to_string();
                        n_str
                            .strip_prefix(&format!("{}.", enumeration.name))
                            .map(|s| s.to_string())
                            .unwrap_or(n_str)
                    })
                    .map(|s| s.to_string())
                {
                    value.value_int = enumeration
                        .values
                        .iter()
                        .position(|v| v == &text)
                        .and_then(|v| i32::try_from(v).ok())
                        .unwrap_or_default();
                }
            } else if let Some(expression_tree::Expression::EnumerationValue(v)) = def_val {
                value.value_int = v.value as i32
            }
        }
        _ => {}
    }

    value
}

fn extract_value_with_unit_impl(
    expression: &Option<syntax_nodes::Expression>,
    def_val: Option<&expression_tree::Expression>,
    code: &str,
    units: &[i_slint_compiler::expression_tree::Unit],
) -> Option<(ui::PropertyValueKind, f32, i32, String)> {
    if let Some(expression) = expression {
        if let Some((value, unit)) = convert_number_literal(expression) {
            let index = units.iter().position(|u| u == &unit).or_else(|| {
                (units.is_empty() && unit == i_slint_compiler::expression_tree::Unit::None)
                    .then_some(0_usize)
            })?;

            return Some((
                ui::PropertyValueKind::Float,
                value as f32,
                index as i32,
                unit.to_string(),
            ));
        }
    } else if code.is_empty() {
        if let Some(expression_tree::Expression::NumberLiteral(value, unit)) = def_val {
            let index = units.iter().position(|u| u == unit).unwrap_or(0);
            return Some((
                ui::PropertyValueKind::Float,
                *value as f32,
                index as i32,
                String::new(),
            ));
        } else {
            // FIXME: if def_vale is Some but not a NumberLiteral, we should not show "0"
            return Some((ui::PropertyValueKind::Float, 0.0, 0, String::new()));
        }
    }

    None
}

fn extract_value_with_unit(
    expression: &Option<syntax_nodes::Expression>,
    def_val: Option<&expression_tree::Expression>,
    units: &[expression_tree::Unit],
    value: &mut ui::PropertyValue,
) {
    let Some((kind, v, index, unit)) =
        extract_value_with_unit_impl(expression, def_val, &value.code, units)
    else {
        return;
    };

    value.display_string = slint::format!("{v}{unit}");
    value.kind = kind;
    value.value_float = v;
    value.visual_items = ui::unit_model(units);
    value.value_int = index
}

#[cfg(test)]
mod tests {
    use slint::{Model, SharedString};

    use crate::{
        common,
        preview::{properties, ui},
        test::loaded_document_cache,
    };

    fn properties_at_position(
        source: &str,
        line: u32,
        character: u32,
    ) -> Option<(
        common::ElementRcNode,
        Vec<properties::PropertyInformation>,
        common::DocumentCache,
        lsp_types::Url,
    )> {
        let (dc, url, _) = loaded_document_cache(source.to_string());
        if let Some((e, p)) =
            properties::tests::properties_at_position_in_cache(line, character, &dc, &url)
        {
            Some((e, p, dc, url))
        } else {
            None
        }
    }

    fn property_conversion_test(contents: &str, property_line: u32) -> ui::PropertyValue {
        let (_, pi, dc, _) = properties_at_position(contents, property_line, 30).unwrap();
        let test1 = pi.iter().find(|pi| pi.name == "test1").unwrap();
        super::simplify_value(&dc, test1)
    }

    #[test]
    fn test_property_bool() {
        let result =
            property_conversion_test(r#"export component Test { in property <bool> test1; }"#, 0);
        assert_eq!(result.kind, ui::PropertyValueKind::Boolean);
        assert!(!result.value_bool);
        assert!(result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <bool> test1: true; }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Boolean);
        assert!(result.value_bool);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <bool> test1: false; }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Boolean);
        assert!(!result.value_bool);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <bool> test1: 1.1.round() == 1.1.floor(); }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert!(!result.value_bool);
        assert!(!result.code.is_empty());
    }

    #[test]
    fn test_property_string() {
        let result =
            property_conversion_test(r#"export component Test { in property <string> test1; }"#, 0);
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
        assert_eq!(result.kind, ui::PropertyValueKind::String);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert!(!result.value_bool);
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: "" + "test"); }"#,
            0,
        );
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
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_string, "");
        assert!(!result.code.is_empty());

        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: @tr("" + "test"); }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_string, "");
        assert!(!result.code.is_empty());
        let result = property_conversion_test(
            r#"export component Test { in property <string> test1: @tr("width {}", self.width()); }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert!(!result.is_translatable);
        assert_eq!(result.tr_context, "");
        assert_eq!(result.tr_plural, "");
        assert_eq!(result.value_string, "");
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
        assert_eq!(result.value_float, 0.0);
    }

    #[test]
    fn test_property_integer() {
        let result =
            property_conversion_test(r#"export component Test { in property <int> test1; }"#, 0);
        assert_eq!(result.kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.value_int, 0);

        let result = property_conversion_test(
            r#"export component Test { in property <int> test1: 42; }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.value_int, 42);

        let result = property_conversion_test(
            r#"export component Test { in property <int> test1: +42; }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.value_int, 42);

        let result = property_conversion_test(
            r#"export component Test { in property <int> test1: -42; }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Integer);
        assert_eq!(result.value_int, -42);

        let result = property_conversion_test(
            r#"export component Test { in property <int> test1: 42 * 23; }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
        assert_eq!(result.value_int, 0);
    }

    #[test]
    fn test_property_color() {
        let result =
            property_conversion_test(r#"export component Test { in property <color> test1; }"#, 0);
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_brush.color().red(), 0);
        assert_eq!(result.value_brush.color().green(), 0);
        assert_eq!(result.value_brush.color().blue(), 0);
        assert_eq!(result.value_brush.color().alpha(), 0);

        let result = property_conversion_test(
            r#"export component Test { in property <color> test1: #10203040; }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Color);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_brush.color().red(), 0x10);
        assert_eq!(result.value_brush.color().green(), 0x20);
        assert_eq!(result.value_brush.color().blue(), 0x30);
        assert_eq!(result.value_brush.color().alpha(), 0x40);

        let result = property_conversion_test(
            r#"export component Test { in property <color> test1: #10203040.darker(0.5); }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);

        let result = property_conversion_test(
            r#"export component Test { in property <color> test1: Colors.red; }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
    }

    #[test]
    fn test_property_brush() {
        let result =
            property_conversion_test(r#"export component Test { in property <brush> test1; }"#, 0);
        assert_eq!(result.kind, ui::PropertyValueKind::Brush);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_brush.color().red(), 0);
        assert_eq!(result.value_brush.color().green(), 0);
        assert_eq!(result.value_brush.color().blue(), 0);
        assert_eq!(result.value_brush.color().alpha(), 0);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: #10203040; }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Brush);
        assert!(matches!(result.value_brush, slint::Brush::SolidColor(_)));
        assert_eq!(result.value_brush.color().red(), 0x10);
        assert_eq!(result.value_brush.color().green(), 0x20);
        assert_eq!(result.value_brush.color().blue(), 0x30);
        assert_eq!(result.value_brush.color().alpha(), 0x40);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: #10203040.darker(0.5); }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: Colors.red; }"#,
            0,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @linear-gradient(90deg, #3f87a6 0%, #ebf8e1 50%, #f69d3c 100%); }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Brush);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @radial-gradient(circle, #f00 0%, #0f0 50%, #00f 100%); }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Brush);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @linear-gradient(90deg, #3f87a6 0%, #ebf8e1 50% - 10%, #f69d3c 100%); }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);

        let result = property_conversion_test(
            r#"export component Test { in property <brush> test1: @radial-gradient(circle, #f00 0%, #0f0 50% - 10%, #00f 100%); }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Code);
    }

    #[test]
    fn test_property_units() {
        let result =
            property_conversion_test(r#"export component Test { in property <length> test1; }"#, 0);
        assert_eq!(result.kind, ui::PropertyValueKind::Float);
        assert_eq!(result.default_selection, 0);
        assert_eq!(result.value_int, 0);
        assert_eq!(result.visual_items.row_data(result.value_int as usize), Some("px".into()));
        let length_row_count = result.visual_items.row_count();
        assert!(length_row_count > 2);

        let result = property_conversion_test(
            r#"export component Test { in property <duration> test1: 25s; }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Float);
        assert_eq!(result.value_float, 25.0);
        assert_eq!(result.default_selection, 0);
        assert_eq!(result.visual_items.row_data(result.value_int as usize), Some("s".into()));
        assert_eq!(result.visual_items.row_count(), 2); // ms, s

        let result = property_conversion_test(
            r#"export component Test { in property <physical-length> test1: 1.5phx; }"#,
            1,
        );
        assert_eq!(result.kind, ui::PropertyValueKind::Float);
        assert_eq!(result.value_float, 1.5);
        assert_eq!(result.default_selection, 0);
        assert_eq!(result.visual_items.row_data(result.value_int as usize), Some("phx".into()));
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
        let result = super::simplify_value(&dc, prop);
        assert_eq!(result.kind, ui::PropertyValueKind::Boolean);
        assert!(result.value_bool);

        let prop = pi.iter().find(|pi| pi.name == "enabled").unwrap();
        let result = super::simplify_value(&dc, prop);
        assert_eq!(result.kind, ui::PropertyValueKind::Boolean);
        assert!(result.value_bool);

        let prop = pi.iter().find(|pi| pi.name == "text").unwrap();
        let result = super::simplify_value(&dc, prop);
        assert_eq!(result.kind, ui::PropertyValueKind::String);
        assert_eq!(result.value_string, "Ok");

        let prop = pi.iter().find(|pi| pi.name == "alias").unwrap();
        let result = super::simplify_value(&dc, prop);
        assert_eq!(result.kind, ui::PropertyValueKind::Float);
        assert_eq!(result.value_float, 45.);
        assert_eq!(result.visual_items.row_data(result.value_int as usize).unwrap(), "cm");

        let prop = pi.iter().find(|pi| pi.name == "color").unwrap();
        let result = super::simplify_value(&dc, prop);
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
        let result = super::simplify_value(&dc, prop);
        assert_eq!(result.kind, ui::PropertyValueKind::Boolean);
        assert!(result.value_bool);
    }
}
