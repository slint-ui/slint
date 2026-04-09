// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*! module for the Go code generator */

use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::sync::OnceLock;

use itertools::Either;
use smol_str::{SmolStr, StrExt, format_smolstr};

use crate::CompilerConfiguration;
use crate::langtype::{StructName, Type};
use crate::llr;
use crate::object_tree::Document;

fn is_go_keyword(word: &str) -> bool {
    static GO_KEYWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    GO_KEYWORDS
        .get_or_init(|| {
            HashSet::from([
                "break",
                "case",
                "chan",
                "const",
                "continue",
                "default",
                "defer",
                "else",
                "fallthrough",
                "for",
                "func",
                "go",
                "goto",
                "if",
                "import",
                "interface",
                "map",
                "package",
                "range",
                "return",
                "select",
                "struct",
                "switch",
                "type",
                "var",
            ])
        })
        .contains(word)
}

fn ident(ident: &str) -> SmolStr {
    let mut new_ident = SmolStr::from(ident);
    if ident.contains('-') {
        new_ident = ident.replace_smolstr("-", "_");
    }
    if is_go_keyword(new_ident.as_str()) {
        new_ident = format_smolstr!("{}_", new_ident);
    }
    new_ident
}

fn exported_ident(name: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if uppercase_next {
                out.extend(ch.to_uppercase());
                uppercase_next = false;
            } else {
                out.push(ch);
            }
        } else {
            uppercase_next = true;
        }
    }
    if out.is_empty() {
        out.push_str("Generated");
    }
    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        out.insert(0, 'X');
    }
    out
}

fn package_name(destination_path: Option<&std::path::Path>) -> SmolStr {
    let fallback = SmolStr::new_static("main");
    let Some(parent) = destination_path.and_then(|path| path.parent()) else {
        return fallback;
    };
    let Some(dir_name) = parent.file_name().and_then(|name| name.to_str()) else {
        return fallback;
    };
    let sanitized = ident(&dir_name.to_ascii_lowercase());
    if sanitized.is_empty() { fallback } else { sanitized }
}

#[derive(Clone)]
struct GoField {
    name: SmolStr,
    ty: Type,
}

#[derive(Clone)]
struct GoStruct {
    name: SmolStr,
    fields: Vec<GoField>,
    aliases: Vec<SmolStr>,
}

#[derive(Clone)]
struct GoEnum {
    name: SmolStr,
    variants: Vec<SmolStr>,
    aliases: Vec<SmolStr>,
}

#[derive(Clone)]
struct GoProperty {
    slint_name: SmolStr,
    go_name: String,
    ty: Type,
    read_only: bool,
}

#[derive(Clone)]
struct GoCallable {
    slint_name: SmolStr,
    go_name: String,
    args: Vec<Type>,
    return_type: Type,
}

#[derive(Clone)]
struct GoGlobal {
    name: SmolStr,
    aliases: Vec<SmolStr>,
    properties: Vec<GoProperty>,
    callbacks: Vec<GoCallable>,
    functions: Vec<GoCallable>,
}

#[derive(Clone)]
struct GoComponent {
    name: SmolStr,
    aliases: Vec<SmolStr>,
    properties: Vec<GoProperty>,
    callbacks: Vec<GoCallable>,
    functions: Vec<GoCallable>,
}

fn split_members(
    properties: &[llr::PublicProperty],
) -> (Vec<GoProperty>, Vec<GoCallable>, Vec<GoCallable>) {
    let mut go_properties = Vec::new();
    let mut callbacks = Vec::new();
    let mut functions = Vec::new();

    for property in properties {
        match &property.ty {
            Type::Callback(function) => callbacks.push(GoCallable {
                slint_name: property.name.clone(),
                go_name: exported_ident(&property.name),
                args: function.args.to_vec(),
                return_type: function.return_type.clone(),
            }),
            Type::Function(function) => functions.push(GoCallable {
                slint_name: property.name.clone(),
                go_name: exported_ident(&property.name),
                args: function.args.to_vec(),
                return_type: function.return_type.clone(),
            }),
            _ => go_properties.push(GoProperty {
                slint_name: property.name.clone(),
                go_name: exported_ident(&property.name),
                ty: property.ty.clone(),
                read_only: property.read_only,
            }),
        }
    }

    (go_properties, callbacks, functions)
}

fn export_aliases(
    doc: &Document,
) -> (
    HashMap<SmolStr, Vec<SmolStr>>,
    HashMap<SmolStr, Vec<SmolStr>>,
    HashMap<SmolStr, Vec<SmolStr>>,
    HashMap<SmolStr, Vec<SmolStr>>,
) {
    let mut component_aliases: HashMap<SmolStr, Vec<SmolStr>> = Default::default();
    let mut global_aliases: HashMap<SmolStr, Vec<SmolStr>> = Default::default();
    let mut struct_aliases: HashMap<SmolStr, Vec<SmolStr>> = Default::default();
    let mut enum_aliases: HashMap<SmolStr, Vec<SmolStr>> = Default::default();

    for export in doc.exports.iter() {
        match &export.1 {
            Either::Left(component) => {
                if export.0.name != component.id {
                    let aliases = if component.is_global() {
                        &mut global_aliases
                    } else {
                        &mut component_aliases
                    };
                    aliases.entry(component.id.clone()).or_default().push(export.0.name.clone());
                }
            }
            Either::Right(ty) => match ty {
                Type::Struct(s) if s.node().is_some() => {
                    if let StructName::User { name, .. } = &s.name
                        && export.0.name != *name
                    {
                        struct_aliases.entry(name.clone()).or_default().push(export.0.name.clone());
                    }
                }
                Type::Enumeration(en) if export.0.name != en.name => {
                    enum_aliases.entry(en.name.clone()).or_default().push(export.0.name.clone());
                }
                _ => {}
            },
        }
    }

    (component_aliases, global_aliases, struct_aliases, enum_aliases)
}

fn go_type_name(ty: &Type) -> String {
    match ty {
        Type::String => "string".into(),
        Type::Bool => "bool".into(),
        Type::Float32
        | Type::Int32
        | Type::Duration
        | Type::Angle
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Percent
        | Type::Rem
        | Type::UnitProduct(_) => "float64".into(),
        Type::Enumeration(en) if en.node.is_some() => exported_ident(&en.name),
        Type::Void => "slint.Value".into(),
        _ => "slint.Value".into(),
    }
}

fn to_value_expr(var_name: &str, ty: &Type) -> String {
    match ty {
        Type::String => format!("slint.StringValue({var_name})"),
        Type::Bool => format!("slint.BoolValue({var_name})"),
        Type::Float32
        | Type::Int32
        | Type::Duration
        | Type::Angle
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Percent
        | Type::Rem
        | Type::UnitProduct(_) => format!("slint.NumberValue({var_name})"),
        Type::Enumeration(_) => format!("slint.StringValue(string({var_name}))"),
        _ => var_name.into(),
    }
}

fn emit_value_from_expr(
    out: &mut String,
    value_expr: &str,
    ty: &Type,
    indent: &str,
) -> std::fmt::Result {
    match ty {
        Type::String => {
            writeln!(out, "{indent}return {value_expr}.String()")?;
        }
        Type::Bool => {
            writeln!(out, "{indent}return {value_expr}.Bool()")?;
        }
        Type::Float32
        | Type::Int32
        | Type::Duration
        | Type::Angle
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Percent
        | Type::Rem
        | Type::UnitProduct(_) => {
            writeln!(out, "{indent}return {value_expr}.Number()")?;
        }
        Type::Enumeration(en) if en.node.is_some() => {
            let enum_name = exported_ident(&en.name);
            writeln!(out, "{indent}s, err := {value_expr}.String()")?;
            writeln!(out, "{indent}if err != nil {{")?;
            writeln!(out, "{indent}\treturn \"\", err")?;
            writeln!(out, "{indent}}}")?;
            writeln!(out, "{indent}return {enum_name}(s), nil")?;
        }
        _ => {
            writeln!(out, "{indent}return {value_expr}, nil")?;
        }
    }
    Ok(())
}

fn source_path_snippet(
    doc: &Document,
    destination_path: Option<&std::path::Path>,
) -> std::io::Result<String> {
    let main_file = std::path::absolute(
        doc.node
            .as_ref()
            .ok_or_else(|| std::io::Error::other("Cannot determine path of the main file"))?
            .source_file
            .path(),
    )?;
    let destination_dir = destination_path
        .and_then(|path| path.parent())
        .and_then(|path| std::fs::canonicalize(path).ok());
    let relative = destination_dir
        .and_then(|dir| pathdiff::diff_paths(&main_file, dir))
        .unwrap_or_else(|| std::path::PathBuf::from(main_file.file_name().unwrap()));

    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn emit_struct(out: &mut String, go_struct: &GoStruct) -> std::fmt::Result {
    writeln!(out, "type {} struct {{", exported_ident(&go_struct.name))?;
    for field in &go_struct.fields {
        writeln!(out, "\t{} {}", exported_ident(&field.name), go_type_name(&field.ty))?;
    }
    writeln!(out, "}}")?;
    writeln!(out)?;
    for alias in &go_struct.aliases {
        writeln!(out, "type {} = {}", exported_ident(alias), exported_ident(&go_struct.name))?;
        writeln!(out)?;
    }
    Ok(())
}

fn emit_enum(out: &mut String, go_enum: &GoEnum) -> std::fmt::Result {
    let enum_name = exported_ident(&go_enum.name);
    writeln!(out, "type {enum_name} string")?;
    writeln!(out)?;
    writeln!(out, "const (")?;
    for variant in &go_enum.variants {
        writeln!(out, "\t{}{} {enum_name} = {:?}", enum_name, exported_ident(variant), variant)?;
    }
    writeln!(out, ")")?;
    writeln!(out)?;
    for alias in &go_enum.aliases {
        writeln!(out, "type {} = {}", exported_ident(alias), enum_name)?;
        writeln!(out)?;
    }
    Ok(())
}

fn emit_global_wrapper(
    out: &mut String,
    component: &GoComponent,
    global: &GoGlobal,
) -> std::fmt::Result {
    let component_name = exported_ident(&component.name);
    let global_type_name = format!("{component_name}{}Global", exported_ident(&global.name));

    writeln!(out, "type {global_type_name} struct {{")?;
    writeln!(out, "\tinner *slint.ComponentInstance")?;
    writeln!(out, "}}")?;
    writeln!(out)?;

    for accessor_name in std::iter::once(&global.name).chain(global.aliases.iter()) {
        writeln!(
            out,
            "func (c *{component_name}) {}() *{global_type_name} {{",
            exported_ident(accessor_name)
        )?;
        writeln!(out, "\treturn &{global_type_name}{{inner: c.inner}}")?;
        writeln!(out, "}}")?;
        writeln!(out)?;
    }

    for property in &global.properties {
        let property_type = go_type_name(&property.ty);
        writeln!(
            out,
            "func (g *{global_type_name}) Get{}() ({property_type}, error) {{",
            property.go_name
        )?;
        writeln!(
            out,
            "\tvalue, err := g.inner.GetGlobalProperty({:?}, {:?})",
            global.name, property.slint_name
        )?;
        writeln!(out, "\tif err != nil {{")?;
        writeln!(out, "\t\tvar zero {property_type}")?;
        writeln!(out, "\t\treturn zero, err")?;
        writeln!(out, "\t}}")?;
        emit_value_from_expr(out, "value", &property.ty, "\t")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        if !property.read_only {
            writeln!(
                out,
                "func (g *{global_type_name}) Set{}(value {property_type}) error {{",
                property.go_name
            )?;
            writeln!(
                out,
                "\treturn g.inner.SetGlobalProperty({:?}, {:?}, {})",
                global.name,
                property.slint_name,
                to_value_expr("value", &property.ty)
            )?;
            writeln!(out, "}}")?;
            writeln!(out)?;
        }
    }

    for callback in &global.callbacks {
        emit_callback_methods(out, &global_type_name, Some(&global.name), callback)?;
    }
    for function in &global.functions {
        emit_invoke_method(out, &global_type_name, Some(&global.name), function)?;
    }

    Ok(())
}

fn emit_callback_methods(
    out: &mut String,
    receiver_type: &str,
    global_name: Option<&str>,
    callback: &GoCallable,
) -> std::fmt::Result {
    let params = callback
        .args
        .iter()
        .enumerate()
        .map(|(index, ty)| format!("arg_{index} {}", go_type_name(ty)))
        .collect::<Vec<_>>();
    let return_type = go_type_name(&callback.return_type);
    let handler_signature = if matches!(callback.return_type, Type::Void) {
        format!("func({})", params.join(", "))
    } else {
        format!("func({}) {return_type}", params.join(", "))
    };
    writeln!(
        out,
        "func (c *{receiver_type}) On{}(handler {handler_signature}) error {{",
        callback.go_name
    )?;
    writeln!(
        out,
        "\treturn c.inner.{}(",
        if global_name.is_some() { "SetGlobalCallback" } else { "SetCallback" }
    )?;
    if let Some(global_name) = global_name {
        writeln!(out, "\t\t{:?},", global_name)?;
    }
    writeln!(out, "\t\t{:?},", callback.slint_name)?;
    writeln!(out, "\t\tfunc(args []slint.Value) slint.Value {{")?;
    for (index, ty) in callback.args.iter().enumerate() {
        writeln!(out, "\t\t\tconvertedArg{index}, err := func() ({}, error) {{", go_type_name(ty))?;
        emit_value_from_expr(out, &format!("args[{index}]"), ty, "\t\t\t\t")?;
        writeln!(out, "\t\t\t}}()")?;
        writeln!(out, "\t\t\tif err != nil {{")?;
        writeln!(out, "\t\t\t\tpanic(err)")?;
        writeln!(out, "\t\t\t}}")?;
    }
    if matches!(callback.return_type, Type::Void) {
        writeln!(
            out,
            "\t\t\thandler({})",
            (0..callback.args.len())
                .map(|index| format!("convertedArg{index}"))
                .collect::<Vec<_>>()
                .join(", ")
        )?;
        writeln!(out, "\t\t\treturn slint.VoidValue()")?;
    } else {
        writeln!(
            out,
            "\t\t\treturn {}",
            to_value_expr(
                &format!(
                    "handler({})",
                    (0..callback.args.len())
                        .map(|index| format!("convertedArg{index}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                &callback.return_type
            )
        )?;
    }
    writeln!(out, "\t\t}},")?;
    writeln!(out, "\t)")?;
    writeln!(out, "}}")?;
    writeln!(out)?;

    emit_invoke_method(out, receiver_type, global_name, callback)
}

fn emit_invoke_method(
    out: &mut String,
    receiver_type: &str,
    global_name: Option<&str>,
    callable: &GoCallable,
) -> std::fmt::Result {
    let params = callable
        .args
        .iter()
        .enumerate()
        .map(|(index, ty)| format!("arg_{index} {}", go_type_name(ty)))
        .collect::<Vec<_>>();

    if matches!(callable.return_type, Type::Void) {
        writeln!(
            out,
            "func (c *{receiver_type}) Invoke{}({}) error {{",
            callable.go_name,
            params.join(", ")
        )?;
    } else {
        writeln!(
            out,
            "func (c *{receiver_type}) Invoke{}({}) ({}, error) {{",
            callable.go_name,
            params.join(", "),
            go_type_name(&callable.return_type)
        )?;
    }

    let args = callable
        .args
        .iter()
        .enumerate()
        .map(|(index, ty)| to_value_expr(&format!("arg_{index}"), ty))
        .collect::<Vec<_>>();

    if let Some(global_name) = global_name {
        if args.is_empty() {
            writeln!(
                out,
                "\tvalue, err := c.inner.InvokeGlobal({:?}, {:?})",
                global_name, callable.slint_name
            )?;
        } else {
            writeln!(
                out,
                "\tvalue, err := c.inner.InvokeGlobal({:?}, {:?}, {})",
                global_name,
                callable.slint_name,
                args.join(", ")
            )?;
        }
    } else {
        if args.is_empty() {
            writeln!(out, "\tvalue, err := c.inner.Invoke({:?})", callable.slint_name)?;
        } else {
            writeln!(
                out,
                "\tvalue, err := c.inner.Invoke({:?}, {})",
                callable.slint_name,
                args.join(", ")
            )?;
        }
    }
    writeln!(out, "\tif err != nil {{")?;
    if matches!(callable.return_type, Type::Void) {
        writeln!(out, "\t\treturn err")?;
    } else {
        writeln!(out, "\t\tvar zero {}", go_type_name(&callable.return_type))?;
        writeln!(out, "\t\treturn zero, err")?;
    }
    writeln!(out, "\t}}")?;
    if matches!(callable.return_type, Type::Void) {
        writeln!(out, "\treturn nil")?;
    } else {
        emit_value_from_expr(out, "value", &callable.return_type, "\t")?;
    }
    writeln!(out, "}}")?;
    writeln!(out)?;
    Ok(())
}

fn emit_component(
    out: &mut String,
    component: &GoComponent,
    globals: &[GoGlobal],
) -> std::fmt::Result {
    let component_name = exported_ident(&component.name);

    writeln!(out, "type {component_name} struct {{")?;
    writeln!(out, "\tinner *slint.ComponentInstance")?;
    writeln!(out, "}}")?;
    writeln!(out)?;

    writeln!(out, "func New{component_name}() (*{component_name}, error) {{")?;
    writeln!(out, "\tdefinition, err := generatedComponentDefinition({:?})", component.name)?;
    writeln!(out, "\tif err != nil {{")?;
    writeln!(out, "\t\treturn nil, err")?;
    writeln!(out, "\t}}")?;
    writeln!(out, "\tinner, err := definition.Create()")?;
    writeln!(out, "\tif err != nil {{")?;
    writeln!(out, "\t\treturn nil, err")?;
    writeln!(out, "\t}}")?;
    writeln!(out, "\treturn &{component_name}{{inner: inner}}, nil")?;
    writeln!(out, "}}")?;
    writeln!(out)?;

    for alias in &component.aliases {
        writeln!(
            out,
            "func New{}() (*{}, error) {{ return New{}() }}",
            exported_ident(alias),
            component_name,
            component_name
        )?;
        writeln!(out)?;
        writeln!(out, "type {} = {}", exported_ident(alias), component_name)?;
        writeln!(out)?;
    }

    writeln!(
        out,
        "func (c *{component_name}) Inner() *slint.ComponentInstance {{ return c.inner }}"
    )?;
    writeln!(out)?;
    writeln!(out, "func (c *{component_name}) Show() error {{ return c.inner.Show() }}")?;
    writeln!(out)?;
    writeln!(out, "func (c *{component_name}) Hide() error {{ return c.inner.Hide() }}")?;
    writeln!(out)?;
    writeln!(out, "func (c *{component_name}) Run() error {{ return c.inner.Run() }}")?;
    writeln!(out)?;

    for property in &component.properties {
        let property_type = go_type_name(&property.ty);
        writeln!(
            out,
            "func (c *{component_name}) Get{}() ({property_type}, error) {{",
            property.go_name
        )?;
        writeln!(out, "\tvalue, err := c.inner.GetProperty({:?})", property.slint_name)?;
        writeln!(out, "\tif err != nil {{")?;
        writeln!(out, "\t\tvar zero {property_type}")?;
        writeln!(out, "\t\treturn zero, err")?;
        writeln!(out, "\t}}")?;
        emit_value_from_expr(out, "value", &property.ty, "\t")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        if !property.read_only {
            writeln!(
                out,
                "func (c *{component_name}) Set{}(value {property_type}) error {{",
                property.go_name
            )?;
            writeln!(
                out,
                "\treturn c.inner.SetProperty({:?}, {})",
                property.slint_name,
                to_value_expr("value", &property.ty)
            )?;
            writeln!(out, "}}")?;
            writeln!(out)?;
        }
    }

    for callback in &component.callbacks {
        emit_callback_methods(out, &component_name, None, callback)?;
    }
    for function in &component.functions {
        emit_invoke_method(out, &component_name, None, function)?;
    }

    for global in globals {
        emit_global_wrapper(out, component, global)?;
    }

    Ok(())
}

pub fn generate(
    doc: &Document,
    compiler_config: &CompilerConfiguration,
    destination_path: Option<&std::path::Path>,
) -> std::io::Result<String> {
    let (mut component_aliases, mut global_aliases, mut struct_aliases, mut enum_aliases) =
        export_aliases(doc);

    let mut go_structs = Vec::new();
    let mut go_enums = Vec::new();
    for ty in &doc.used_types.borrow().structs_and_enums {
        match ty {
            Type::Struct(s) => {
                let StructName::User { name, .. } = &s.name else { continue };
                go_structs.push(GoStruct {
                    name: name.clone(),
                    fields: s
                        .fields
                        .iter()
                        .map(|(field_name, field_ty)| GoField {
                            name: ident(field_name),
                            ty: field_ty.clone(),
                        })
                        .collect(),
                    aliases: struct_aliases.remove(name).unwrap_or_default(),
                });
            }
            Type::Enumeration(en) => go_enums.push(GoEnum {
                name: en.name.clone(),
                variants: en.values.to_vec(),
                aliases: enum_aliases.remove(&en.name).unwrap_or_default(),
            }),
            _ => {}
        }
    }

    let llr = llr::lower_to_item_tree::lower_to_item_tree(doc, compiler_config);

    let globals = llr
        .globals
        .iter()
        .filter(|global| global.exported && global.must_generate())
        .map(|global| {
            let (properties, callbacks, functions) = split_members(&global.public_properties);
            GoGlobal {
                name: global.name.clone(),
                aliases: global_aliases
                    .remove(&global.name)
                    .unwrap_or_else(|| global.aliases.clone()),
                properties,
                callbacks,
                functions,
            }
        })
        .collect::<Vec<_>>();

    let components = llr
        .public_components
        .iter()
        .map(|component| {
            let (properties, callbacks, functions) = split_members(&component.public_properties);
            GoComponent {
                name: component.name.clone(),
                aliases: component_aliases.remove(&component.name).unwrap_or_default(),
                properties,
                callbacks,
                functions,
            }
        })
        .collect::<Vec<_>>();

    let package_name = package_name(destination_path);
    let source =
        doc.node.as_ref().and_then(|node| node.source_file.source()).ok_or_else(|| {
            std::io::Error::other("Cannot determine source code of the main file")
        })?;
    let source_path = source_path_snippet(doc, destination_path)?;

    let mut output = String::new();
    (|| -> std::fmt::Result {
        writeln!(output, "// Code generated by Slint. DO NOT EDIT.")?;
        writeln!(output)?;
        writeln!(output, "package {package_name}")?;
        writeln!(output)?;
        writeln!(output, "import (")?;
        writeln!(output, "\t\"fmt\"")?;
        writeln!(output, "\t\"path/filepath\"")?;
        writeln!(output, "\t\"runtime\"")?;
        writeln!(output, "\t\"sync\"")?;
        writeln!(output)?;
        writeln!(output, "\tslint \"github.com/slint-ui/slint/api/go/slint\"")?;
        writeln!(output, ")")?;
        writeln!(output)?;

        writeln!(output, "var generatedSource = {:?}", source)?;
        writeln!(output)?;
        writeln!(output, "var generatedSourcePathRelative = {:?}", source_path)?;
        writeln!(output)?;
        writeln!(output, "var generatedCompilationOnce sync.Once")?;
        writeln!(output, "var generatedCompilation *slint.CompilationResult")?;
        writeln!(output, "var generatedCompilationErr error")?;
        writeln!(output)?;
        writeln!(output, "func generatedSourcePath() string {{")?;
        writeln!(output, "\t_, file, _, ok := runtime.Caller(0)")?;
        writeln!(output, "\tif !ok {{")?;
        writeln!(output, "\t\treturn generatedSourcePathRelative")?;
        writeln!(output, "\t}}")?;
        writeln!(
            output,
            "\treturn filepath.Clean(filepath.Join(filepath.Dir(file), generatedSourcePathRelative))"
        )?;
        writeln!(output, "}}")?;
        writeln!(output)?;
        writeln!(output, "func generatedCompilationResult() (*slint.CompilationResult, error) {{")?;
        writeln!(output, "\tgeneratedCompilationOnce.Do(func() {{")?;
        writeln!(
            output,
            "\t\tgeneratedCompilation, generatedCompilationErr = slint.CompileSource(generatedSourcePath(), generatedSource)"
        )?;
        writeln!(output, "\t}})")?;
        writeln!(output, "\treturn generatedCompilation, generatedCompilationErr")?;
        writeln!(output, "}}")?;
        writeln!(output)?;
        writeln!(
            output,
            "func generatedComponentDefinition(name string) (*slint.ComponentDefinition, error) {{"
        )?;
        writeln!(output, "\tresult, err := generatedCompilationResult()")?;
        writeln!(output, "\tif err != nil {{")?;
        writeln!(output, "\t\treturn nil, err")?;
        writeln!(output, "\t}}")?;
        writeln!(output, "\tdefinition := result.Component(name)")?;
        writeln!(output, "\tif definition == nil {{")?;
        writeln!(output, "\t\treturn nil, fmt.Errorf(\"slint: component %q not found\", name)")?;
        writeln!(output, "\t}}")?;
        writeln!(output, "\treturn definition, nil")?;
        writeln!(output, "}}")?;
        writeln!(output)?;

        for go_struct in &go_structs {
            emit_struct(&mut output, go_struct)?;
        }
        for go_enum in &go_enums {
            emit_enum(&mut output, go_enum)?;
        }
        for component in &components {
            emit_component(&mut output, component, &globals)?;
        }
        Ok(())
    })()
    .map_err(std::io::Error::other)?;

    Ok(output)
}
