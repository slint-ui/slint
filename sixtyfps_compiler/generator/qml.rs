/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*! Generate QML
*/

/// This module contains some datastructure that helps represent a QML code.
/// It is then rendered into an actual QML text using the Display trait
mod qml_ast {
    use std::fmt::{Display, Formatter, Result};

    #[derive(Default, Debug, Clone)]
    pub struct File {
        pub imports: Vec<String>,
        pub object: Object,
    }

    #[derive(Default, Debug, Clone)]
    pub struct Object {
        pub base: String,
        pub children: Vec<Object>,
        pub signal_decl: Vec<String>,
        pub bindings: Vec<Binding>,
    }

    #[derive(Default, Debug, Clone)]
    pub struct Binding {
        pub property_name: String,
        /// None for normal binding, Some(type) for "property type" declarations
        pub property_decl_type: Option<String>,
        pub code: Option<String>,
    }

    impl Display for File {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            for x in &self.imports {
                writeln!(f, "import {};", x)?;
            }
            display_object(&self.object, f, 0)
        }
    }

    fn indent(f: &mut Formatter<'_>, i: u32) -> Result {
        for _ in 0..i {
            write!(f, "    ")?;
        }
        Ok(())
    }

    fn display_object(object: &Object, f: &mut Formatter<'_>, indent: u32) -> Result {
        self::indent(f, indent)?;
        writeln!(f, "{} {{", object.base)?;
        for sig in &object.signal_decl {
            self::indent(f, indent + 1)?;
            writeln!(f, "signal {};", sig)?;
        }
        for bind in &object.bindings {
            self::indent(f, indent + 1)?;
            if let Some(ty) = &bind.property_decl_type {
                write!(f, "property {} ", ty)?;
            }
            write!(f, "{}", bind.property_name)?;
            if let Some(code) = &bind.code {
                writeln!(f, ": {};", code)?;
            } else {
                writeln!(f, ";")?;
            }
        }
        for c in &object.children {
            display_object(c, f, indent + 1)?;
        }
        self::indent(f, indent)?;
        writeln!(f, "}}")?;
        Ok(())
    }
}

use qml_ast::*;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BuiltinFunction, Expression, NamedReference};
use crate::langtype::{EnumerationValue, Type};
use crate::object_tree::{Document, ElementRc};
use itertools::Itertools;

fn qml_type(ty: &Type) -> String {
    match ty {
        Type::Float32 => "real".to_owned(),
        Type::Int32 => "int".to_owned(),
        Type::String => "string".to_owned(),
        Type::Color => "color".to_owned(),
        Type::Duration => "int".to_owned(),
        Type::Length => "real".to_owned(),
        Type::LogicalLength => "real".to_owned(),
        Type::Percent => "real".to_owned(),
        Type::Bool => "bool".to_owned(),
        _ => "var".to_owned(),
    }
}

pub fn generate(doc: &Document, _diag: &mut BuildDiagnostics) -> Option<impl std::fmt::Display> {
    let mut file = File::default();

    file.imports.push("QtQuick 2.15".into());
    file.imports.push("QtQuick.Window 2.15".into());
    file.object = generate_object(&doc.root_component.root_element);
    for x in doc.root_component.used_global.borrow().iter() {
        let mut global = Object::default();
        global.base = "QtObject".into();
        global.bindings.push(Binding {
            property_name: "id".to_owned(),
            property_decl_type: None,
            code: Some(format!("global_{}", x.id)),
        });
        generate_object_properties(&mut global, &x.root_element);
        file.object.children.push(global);
    }

    Some(file)
}

fn generate_object(element: &ElementRc) -> Object {
    let mut object = Object::default();

    object.bindings.push(Binding {
        property_name: "id".to_owned(),
        property_decl_type: None,
        code: Some(element.borrow().id.clone()),
    });

    generate_object_properties(&mut object, element);

    if let Some(r) = &element.borrow().repeated {
        object.base = "Repeater".to_owned();
        object.bindings.push(Binding {
            property_name: "model".to_owned(),
            property_decl_type: None,
            code: Some(compile_expression(&r.model)),
        });
        if let Type::Component(c) = &element.borrow().base_type {
            object.children.push(generate_object(&c.root_element))
        } else {
            panic!("Repeater must have a component base")
        }
        debug_assert!(element.borrow().children.is_empty());
    } else if let Type::Native(n) = &element.borrow().base_type {
        match n.class_name.as_str() {
            "Rectangle" | "BorderRectangle" => {
                object.base = "Rectangle".to_owned();
                copy_binding(&mut object, element, "color", "color");
                copy_binding(&mut object, element, "border_color", "border.color");
                copy_binding(&mut object, element, "border_radius", "radius");
                copy_binding(&mut object, element, "border_width", "border.width");
            }
            "Image" | "ClippedImage" => {
                object.base = "Image".to_owned();
                copy_binding(&mut object, element, "source", "source");
                copy_binding(&mut object, element, "image_fit", "fillMode");
            }
            "Text" => {
                object.base = "Text".to_owned();
                copy_binding(&mut object, element, "color", "color");
                copy_binding(&mut object, element, "text", "text");
                copy_binding(&mut object, element, "font_family", "font.family");
                copy_binding(&mut object, element, "font_size", "font.pixelSize");
                copy_binding(&mut object, element, "font_weight", "font.weight");
                copy_binding(&mut object, element, "horizontal_alignment", "horizontalAlignment");
                copy_binding(&mut object, element, "vertical_alignment", "verticalAlignment");
            }
            "Window" => {
                object.base = "Window".to_owned();
            }
            "TouchArea" => {
                object.base = "MouseArea".to_owned();
                copy_binding(&mut object, element, "clicked", "onClicked");
            }
            "Flickable" => {
                object.base = "Flickable".to_owned();
                // FIXME: properties
            }
            _ => panic!("Unknown native type {:?}", n),
        };
        copy_binding(&mut object, element, "width", "width");
        copy_binding(&mut object, element, "height", "height");
        copy_binding(&mut object, element, "x", "x");
        copy_binding(&mut object, element, "y", "y");
    }
    for c in &element.borrow().children {
        object.children.push(generate_object(c))
    }
    object
}

fn generate_object_properties(object: &mut Object, element: &ElementRc) {
    for (prop, decl) in &element.borrow().property_declarations {
        if let Type::Callback { args, .. } = &decl.property_type {
            object.signal_decl.push(format!("{} ({})", prop, args.iter().join(", ")));
            if let Some(code) = element.borrow().bindings.get(prop) {
                object.bindings.push(Binding {
                    property_name: format!(
                        "on{}{}",
                        prop.chars().next().unwrap_or_default().to_uppercase(),
                        &prop[prop.char_indices().nth(1).unwrap_or_default().0..]
                    ),
                    property_decl_type: None,
                    code: Some(compile_expression(&code.expression)),
                });
            }
        } else {
            object.bindings.push(Binding {
                property_name: prop.clone(),
                property_decl_type: Some(qml_type(&decl.property_type)),
                code: element
                    .borrow()
                    .bindings
                    .get(prop)
                    .map(|c| compile_expression(&c.expression)),
            });
        };
    }
}

fn copy_binding(object: &mut Object, element: &ElementRc, source_prop: &str, dest_prop: &str) {
    if let Some(x) = element.borrow().bindings.get(source_prop) {
        object.bindings.push(Binding {
            property_name: dest_prop.to_owned(),
            property_decl_type: None,
            code: Some(compile_expression(&x.expression)),
        })
    }
}

fn compile_expression(expr: &Expression) -> String {
    match expr {
        Expression::StringLiteral(s) => {
            format!(r#""{}""#, s.escape_debug())
        }
        Expression::NumberLiteral(n, unit) => unit.normalize(*n).to_string(),
        Expression::BoolLiteral(b) => b.to_string(),
        Expression::PropertyReference(nr) => {
            access_named_reference(nr)
        }
        Expression::CallbackReference(nr) =>
            access_named_reference(nr)
        ,
        Expression::BuiltinFunctionReference(funcref) => match funcref {
            BuiltinFunction::GetWindowScaleFactor => {
                "(function () { return 1 })".into()
            }
            BuiltinFunction::Debug => {
                "console.log"
                    .into()
            }
            BuiltinFunction::Mod => "(function(a, b){return a % b})".into(),
            BuiltinFunction::Round => "Math.round".into(),
            BuiltinFunction::Ceil => "Math.ceil".into(),
            BuiltinFunction::Floor => "Math.floor".into(),
            BuiltinFunction::SetFocusItem => {
                "(function() { console.log('TODO: set_focus') })".into()
            }
            BuiltinFunction::ShowPopupWindow => {
                "(function() { console.log('TODO: show_popup') })".into()
            }
            BuiltinFunction::StringIsFloat => {
                "(function(x) { return x == parseFloat(x) })".into()
            }
            BuiltinFunction::StringToFloat => {
                "parseFloat"
                    .into()
            }
        },
        Expression::ElementReference(_) => todo!("Element references are only supported in the context of built-in function calls at the moment"),
        Expression::MemberFunction { .. } => panic!("member function expressions must not appear in the code generator anymore"),
        Expression::BuiltinMacroReference { .. } => panic!("macro expressions must not appear in the code generator anymore"),
        Expression::RepeaterIndexReference { element } => {
            // FIXME! access the right repeater
            "index".into()
        }
        Expression::RepeaterModelReference { element } => {
            // FIXME! access the right repeater
            "modelData".into()
        }
        Expression::FunctionParameterReference { index, .. } => format!("arg_{}", index),
        Expression::StoreLocalVariable { name, value } => {
            format!("let {} = {};", name, compile_expression(value))
        }
        Expression::ReadLocalVariable { name, .. } => name.clone(),
        Expression::ObjectAccess { base, name } =>
                format!("{}.{}", compile_expression(base), name)

           ,
        Expression::Cast { from, to } => {
            let f = compile_expression(&*from);
            match (from.ty(), to) {
                (Type::Float32, Type::Color) => {
                    format!("(function (c){{ return Qt.rgba(((c >> 16) & 0xff)/255, ((c >> 8) & 0xff)/255, (c & 0xff)/255, ((c >> 24)&0xff)/255)  }})({})", f)
                }
                _ => f,
            }
        }
        Expression::CodeBlock(sub) => {
            let mut x = sub.iter().map(|e| compile_expression(e)).collect::<Vec<_>>();
            if let Some(s) = x.last_mut() { *s = format!("return {};", s) };
            format!("(function(){{ {} }})()", x.join(";"))
        }
        Expression::FunctionCall { function, arguments } => match &**function {
            Expression::BuiltinFunctionReference(BuiltinFunction::SetFocusItem) => {
                if arguments.len() != 1 {
                    panic!("internal error: incorrect argument count to SetFocusItem call");
                }
                if let Expression::ElementReference(focus_item) = &arguments[0] {
                    "console.log('TODO: set_focus')".into()
                } else {
                    panic!("internal error: argument to SetFocusItem must be an element")
                }
            }
            Expression::BuiltinFunctionReference(BuiltinFunction::ShowPopupWindow) => {
                if arguments.len() != 1 {
                    panic!("internal error: incorrect argument count to SetFocusItem call");
                }
                if let Expression::ElementReference(popup_window) = &arguments[0] {
                    "console.log('TODO: show_popup')".into()
                } else {
                    panic!("internal error: argument to SetFocusItem must be an element")
                }
            }
            _ => {
                let mut args = arguments.iter().map(|e| compile_expression(e));
                format!("{}({})", compile_expression(&function), args.join(", "))
            }
        },
        Expression::SelfAssignment { lhs, rhs, op } => {
            if *op == '=' {
                format!(r#"({lhs} = {rhs})"#, lhs = compile_expression(&*lhs), rhs = compile_expression(&*rhs))
            } else {
                format!(r#"({lhs} {op}= {rhs})"#, lhs = compile_expression(&*lhs), rhs = compile_expression(&*rhs), op= op)
            }
        }
        Expression::BinaryExpression { lhs, rhs, op } => {
            let mut buffer = [0; 3];
            format!(
                "({lhs} {op} {rhs})",
                lhs = compile_expression(&*lhs),
                rhs = compile_expression(&*rhs),
                op = match op {
                    '=' => "==",
                    '!' => "!=",
                    '≤' => "<=",
                    '≥' => ">=",
                    '&' => "&&",
                    '|' => "||",
                    _ => op.encode_utf8(&mut buffer),
                },
            )
        }
        Expression::UnaryOp { sub, op } => {
            format!("({op} {sub})", sub = compile_expression(&*sub), op = op,)
        }
        Expression::ResourceReference(resource_ref)  => {
            match resource_ref {
                crate::expression_tree::ResourceReference::AbsolutePath(path) => format!("'{}'", path),
                crate::expression_tree::ResourceReference::EmbeddedData(_) => unimplemented!("The QML generator does not support resource embedding yet")
            }
        }
        Expression::Condition { condition, true_expr, false_expr } => {
            let cond_code = compile_expression(condition);
            let true_code = compile_expression(true_expr);
            let false_code = compile_expression(false_expr);

                format!(
                    r#"({} ? {} : {})"#,
                    cond_code,
                    true_code,
                    false_code
                )

        }
        Expression::Array { element_ty, values } => {
            format!(
                "[{}]",

                 values
                    .iter()
                    .map(|e| compile_expression(e),
                    )
                    .join(", ")
            )
        }
        Expression::Object { ty, values } => {
            format!("{{{}}}", values.iter().map(|(k, v)| {
                format!("'{}': {}",  k, compile_expression(v))

            }).join(", "))
        }
        Expression::PathElements { elements } => todo!("Path in QML"),
        Expression::EasingCurve(_) => todo!("EasingCurve"),
        Expression::EnumerationValue(value) => {
            match value.to_string().as_str() {
                "align_left" => "Text.AlignLeft".to_owned(),
                "align_right" => "Text.AlignRight".to_owned(),
                "align_top" => "Text.AlignTop".to_owned(),
                "align_bottom" => "Text.AlignBottom".to_owned(),
                "align_center" => if value.enumeration.name == "TextHorizontalAlignment" { "Text.AlignHCenter" } else {"Text.AlignVCenter" }.to_owned(),
                "fit" => "Image.Stretch".to_owned(),
                "contain" => "Image.PreserveAspectFit".to_owned(),
                x => format!("'{}'", x)

            }
        }
        Expression::TwoWayBinding(..) => "console.log('FIXME: two way binding')".into(),
        Expression::Uncompiled(_)   => panic!(),
        Expression::Invalid => "\n#error invalid expression\n".to_string(),
    }
}

fn access_named_reference(nr: &NamedReference) -> String {
    let elem = nr.element.upgrade().unwrap();
    let compo = elem.borrow().enclosing_component.upgrade().unwrap();
    if compo.is_global() {
        format!("global_{}.{}", compo.id, nr.name)
    } else {
        format!("{}.{}", elem.borrow().id, nr.name)
    }
}
