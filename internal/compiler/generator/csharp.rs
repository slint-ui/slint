// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The C# code generator: typed classes over the .NET interpreter API in `api/dotnet/Slint`.
//! The generated code loads the `.slint` file at run time and checks it against the API it was generated for.

// cSpell:ignore structty enumty

use std::collections::{BTreeSet, HashSet};
use std::fmt::Write;
use std::sync::OnceLock;

use itertools::Itertools;

use crate::CompilerConfiguration;
use crate::langtype::{Function, StructName, Type};
use crate::llr;
use crate::names::{normalize_identifier, to_pascal_case};
use crate::object_tree::Document;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Config {
    /// The namespace of the generated types; none puts them in the global namespace.
    pub namespace: Option<String>,
    /// The path of the `.slint` file at run time, relative to the application directory.
    /// Defaults to the file name of the main file.
    pub runtime_path: Option<String>,
}

const CONVERSION: &str = "global::Slint.Generated.Conversion";

fn is_csharp_keyword(word: &str) -> bool {
    static KEYWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    #[rustfmt::skip]
    let keywords = KEYWORDS.get_or_init(|| HashSet::from([
        "abstract", "as", "base", "bool", "break", "byte", "case", "catch", "char", "checked",
        "class", "const", "continue", "decimal", "default", "delegate", "do", "double", "else",
        "enum", "event", "explicit", "extern", "false", "finally", "fixed", "float", "for",
        "foreach", "goto", "if", "implicit", "in", "int", "interface", "internal", "is", "lock",
        "long", "namespace", "new", "null", "object", "operator", "out", "override", "params",
        "private", "protected", "public", "readonly", "ref", "return", "sbyte", "sealed",
        "short", "sizeof", "stackalloc", "static", "string", "struct", "switch", "this", "throw",
        "true", "try", "typeof", "uint", "ulong", "unchecked", "unsafe", "ushort", "using",
        "virtual", "void", "volatile", "while",
    ]));
    keywords.contains(word)
}

/// The C# name of a Slint identifier, such as `first-name` → `FirstName`.
pub fn ident(name: &str) -> String {
    let normalized = normalize_identifier(name);
    let pascal = match normalized.strip_prefix('_') {
        Some(rest) => format!("_{}", to_pascal_case(rest)),
        None => to_pascal_case(&normalized),
    };
    if is_csharp_keyword(&pascal) { format!("@{pascal}") } else { pascal }
}

/// The member names already used in a C# type.
#[derive(Default)]
struct Members(HashSet<String>);

impl Members {
    fn new(reserved: &[&str]) -> Self {
        Self(reserved.iter().map(|s| s.to_string()).collect())
    }

    /// Returns `name`, with `_` suffixes until it doesn't collide with an earlier member.
    fn claim(&mut self, name: String) -> String {
        let mut name = name;
        while self.0.contains(&name) {
            name.push('_');
        }
        self.0.insert(name.clone());
        name
    }
}

fn string_literal(s: &str) -> String {
    let mut literal = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => literal.push_str("\\\""),
            '\\' => literal.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(literal, "\\u{:04x}", c as u32);
            }
            c => literal.push(c),
        }
    }
    literal.push('"');
    literal
}

/// The C# type of a Slint type. Types without a typed .NET equivalent yet are `object?`.
fn csharp_type(ty: &Type) -> String {
    match ty {
        Type::Void => "void".into(),
        Type::String => "string".into(),
        Type::Bool => "bool".into(),
        Type::Int32 => "int".into(),
        Type::Float32
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Angle
        | Type::Percent
        | Type::Rem
        | Type::UnitProduct(_) => "float".into(),
        Type::Duration => "global::System.TimeSpan".into(),
        Type::Color => "global::Slint.Color".into(),
        Type::Image => "global::Slint.Image".into(),
        Type::Array(element) => format!("global::Slint.Model<{}>", csharp_type(element)),
        Type::Struct(s) => match &s.name {
            StructName::User { name, .. } => ident(name),
            _ => "global::Slint.Struct".into(),
        },
        Type::Enumeration(e) if e.node.is_some() => ident(&e.name),
        Type::Enumeration(_) => "global::Slint.EnumValue".into(),
        _ => "object?".into(),
    }
}

/// The value of a new, default instance of a struct field of type `ty`, if `default` doesn't do.
fn default_value(ty: &Type) -> Option<String> {
    match ty {
        Type::String => Some("\"\"".into()),
        Type::Image => Some("new global::Slint.Image()".into()),
        Type::Array(element) => {
            Some(format!("new global::Slint.ListModel<{}>()", csharp_type(element)))
        }
        Type::Struct(s) => Some(match &s.name {
            StructName::User { name, .. } => format!("new {}()", ident(name)),
            _ => "new global::Slint.Struct()".into(),
        }),
        _ => None,
    }
}

/// The `Slint.ValueType` that `ComponentDefinition` reports for `ty`; see `From<LangType> for ValueType`.
fn value_type(ty: &Type) -> &'static str {
    match ty {
        Type::Float32
        | Type::Int32
        | Type::Duration
        | Type::Angle
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Percent
        | Type::UnitProduct(_) => "Number",
        Type::String => "String",
        Type::Color | Type::Brush => "Brush",
        Type::Array(_) => "Model",
        Type::Bool => "Bool",
        Type::Struct { .. } => "Struct",
        Type::Void => "Void",
        Type::Image => "Image",
        _ => "Other",
    }
}

fn from_slint(ty: &Type, expr: &str) -> String {
    format!("{CONVERSION}.FromSlint<{}>({expr})", csharp_type(ty))
}

fn to_slint(expr: &str) -> String {
    format!("{CONVERSION}.ToSlint({expr})")
}

/// Collects the element types of every array in `ty`, to register their model conversions.
fn collect_models(ty: &Type, models: &mut BTreeSet<String>) {
    match ty {
        Type::Array(element) => {
            models.insert(csharp_type(element));
            collect_models(element, models);
        }
        Type::Struct(s) => s.fields.values().for_each(|f| collect_models(f, models)),
        Type::Callback(f) | Type::Function(f) => f
            .args
            .iter()
            .chain(std::iter::once(&f.return_type))
            .for_each(|t| collect_models(t, models)),
        _ => {}
    }
}

fn delegate_type(function: &Function) -> String {
    let mut types = function.args.iter().map(csharp_type).collect::<Vec<_>>();
    if function.return_type != Type::Void {
        types.push(csharp_type(&function.return_type));
        format!("global::System.Func<{}>", types.join(", "))
    } else if types.is_empty() {
        "global::System.Action".into()
    } else {
        format!("global::System.Action<{}>", types.join(", "))
    }
}

/// Where the members of a component or a global are: the instance, and the global name for a global.
struct Target<'a> {
    instance: &'a str,
    global: Option<&'a str>,
}

impl Target<'_> {
    /// Calls `method` of `ComponentInstance`, or its global variant for a global.
    fn call(&self, method: &str, rest: &[String]) -> String {
        let (method, global) = match self.global {
            Some(global) => {
                let method = match method {
                    "GetProperty" => "GetGlobalProperty",
                    "SetProperty" => "SetGlobalProperty",
                    "SetCallback" => "SetGlobalCallback",
                    "Invoke" => "InvokeGlobal",
                    _ => unreachable!("{method} has no global variant"),
                };
                (method, Some(string_literal(global)))
            }
            None => (method, None),
        };
        let args = global.into_iter().chain(rest.iter().cloned()).join(", ");
        format!("{}.{method}({args})", self.instance)
    }
}

fn write_members(
    out: &mut String,
    properties: &llr::PublicProperties,
    target: &Target,
    members: &mut Members,
    models: &mut BTreeSet<String>,
) {
    // Properties and functions keep their natural names; the `On` and `Invoke` methods of callbacks yield.
    let mut callbacks = Vec::new();
    for (name, prop) in properties {
        collect_models(&prop.ty, models);
        let slint_name = string_literal(&normalize_identifier(name));
        match &prop.ty {
            Type::Callback(function) => callbacks.push((name, slint_name, function)),
            Type::Function(function) => {
                write_invoke(out, &members.claim(ident(name)), &slint_name, function, target);
            }
            ty => {
                let member = members.claim(ident(name));
                let getter =
                    from_slint(ty, &target.call("GetProperty", std::slice::from_ref(&slint_name)));
                if prop.read_only() {
                    let _ = writeln!(out, "    public {} {member} => {getter};", csharp_type(ty));
                } else {
                    let setter =
                        target.call("SetProperty", &[slint_name.clone(), to_slint("value")]);
                    let _ = writeln!(
                        out,
                        "    public {} {member} {{ get => {getter}; set => {setter}; }}",
                        csharp_type(ty)
                    );
                }
            }
        }
    }
    for (name, slint_name, function) in callbacks {
        let converted_args = function
            .args
            .iter()
            .enumerate()
            .map(|(i, ty)| from_slint(ty, &format!("args[{i}]")))
            .join(", ");
        let call = format!("handler({converted_args})");
        let body = if function.return_type == Type::Void {
            format!("{{ {call}; return null; }}")
        } else {
            to_slint(&call)
        };
        let _ = writeln!(
            out,
            "    public void {}({} handler) => {};",
            members.claim(format!("On{}", ident(name).trim_start_matches('@'))),
            delegate_type(function),
            target.call("SetCallback", &[slint_name.clone(), format!("args => {body}")])
        );
        let invoke = members.claim(format!("Invoke{}", ident(name).trim_start_matches('@')));
        write_invoke(out, &invoke, &slint_name, function, target);
    }
}

fn write_invoke(
    out: &mut String,
    member: &str,
    slint_name: &str,
    function: &Function,
    target: &Target,
) {
    let params = function
        .args
        .iter()
        .enumerate()
        .map(|(i, ty)| format!("{} arg{i}", csharp_type(ty)))
        .join(", ");
    let args = std::iter::once(slint_name.to_string())
        .chain((0..function.args.len()).map(|i| to_slint(&format!("arg{i}"))))
        .collect::<Vec<_>>();
    let call = target.call("Invoke", &args);
    if function.return_type == Type::Void {
        let _ = writeln!(out, "    public void {member}({params}) => {call};");
    } else {
        let _ = writeln!(
            out,
            "    public {} {member}({params}) => {};",
            csharp_type(&function.return_type),
            from_slint(&function.return_type, &call)
        );
    }
}

/// The `ExpectedApi` of a component or global, for the check at load time.
fn expected_api(properties: &llr::PublicProperties) -> String {
    let mut props = Vec::new();
    let mut callbacks = Vec::new();
    let mut functions = Vec::new();
    for (name, prop) in properties {
        let name = string_literal(&normalize_identifier(name));
        match &prop.ty {
            Type::Callback(_) => callbacks.push(name),
            Type::Function(_) => functions.push(name),
            ty => props.push(format!("[{name}] = global::Slint.ValueType.{}", value_type(ty))),
        }
    }
    format!(
        "new(new global::System.Collections.Generic.Dictionary<string, global::Slint.ValueType> {{ {} }}, [{}], [{}])",
        props.join(", "),
        callbacks.join(", "),
        functions.join(", ")
    )
}

fn write_struct(
    out: &mut String,
    structty: &crate::langtype::Struct,
    name: &str,
    registrations: &mut String,
    models: &mut BTreeSet<String>,
) {
    let class = ident(name);
    let mut field_names = Members::new(&[class.as_str()]);
    let fields = structty
        .fields
        .iter()
        .map(|(field, ty)| {
            collect_models(ty, models);
            (normalize_identifier(field), field_names.claim(ident(field)), ty)
        })
        .collect::<Vec<_>>();
    let _ = writeln!(out, "public sealed record {class}\n{{");
    for (_, member, ty) in &fields {
        let init = default_value(ty).map(|v| format!(" = {v};")).unwrap_or_default();
        let _ = writeln!(out, "    public {} {member} {{ get; init; }}{init}", csharp_type(ty));
    }
    let _ = writeln!(out, "}}\n");

    let to = fields
        .iter()
        .map(|(slint, member, _)| {
            format!("[{}] = {}", string_literal(slint), to_slint(&format!("v.{member}")))
        })
        .join(", ");
    let from = fields
        .iter()
        .map(|(slint, member, ty)| {
            format!(
                "{member} = {}",
                from_slint(ty, &format!("{CONVERSION}.Field(s, {})", string_literal(slint)))
            )
        })
        .join(", ");
    let _ = writeln!(
        registrations,
        "        {CONVERSION}.Register<{class}>(\n            v => new global::Slint.Struct {{ {to} }},\n            o => o is global::Slint.Struct s ? new {class} {{ {from} }} : throw new global::System.InvalidCastException(\"expected a struct for {class}\"));"
    );
}

fn write_enum(out: &mut String, enumty: &crate::langtype::Enumeration, registrations: &mut String) {
    let name = ident(&enumty.name);
    let variants = enumty.values.iter().map(|v| (v, ident(v))).collect::<Vec<_>>();
    let _ = writeln!(out, "public enum {name}\n{{");
    for (_, variant) in &variants {
        let _ = writeln!(out, "    {variant},");
    }
    let _ = writeln!(out, "}}\n");

    let slint_enum = string_literal(&enumty.name);
    let to = variants
        .iter()
        .map(|(value, variant)| {
            format!(
                "{name}.{variant} => new global::Slint.EnumValue({slint_enum}, {})",
                string_literal(value)
            )
        })
        .join(", ");
    let from = variants
        .iter()
        .map(|(value, variant)| format!("{} => {name}.{variant}", string_literal(value)))
        .join(", ");
    let _ = writeln!(
        registrations,
        "        {CONVERSION}.Register<{name}>(\n            v => v switch {{ {to}, _ => throw new global::System.ArgumentOutOfRangeException(nameof(v)) }},\n            o => o is global::Slint.EnumValue e ? e.Value switch {{ {from}, var other => throw new global::Slint.SlintException($\"unknown {name} value '{{other}}'\") }} : throw new global::System.InvalidCastException(\"expected an enum value for {name}\"));"
    );
}

/// Returns the text of the C# code for the main file of `doc`.
pub fn generate(
    doc: &Document,
    config: &Config,
    compiler_config: &CompilerConfiguration,
) -> std::io::Result<String> {
    let unit = llr::lower_to_item_tree::lower_to_item_tree(doc, compiler_config);
    let main_file = doc
        .node
        .as_ref()
        .and_then(|node| node.source_file.path().as_native_path())
        .ok_or_else(|| std::io::Error::other("Cannot determine path of the main file"))?;
    let file_name =
        main_file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let runtime_path = config.runtime_path.clone().unwrap_or_else(|| file_name.clone());

    let mut types = String::new();
    let mut registrations = String::new();
    let mut models = BTreeSet::new();

    for ty in doc.used_types.borrow().structs_and_enums.iter() {
        match ty {
            Type::Struct(s) => {
                if let StructName::User { name, .. } = &s.name {
                    write_struct(&mut types, s, name, &mut registrations, &mut models);
                }
            }
            Type::Enumeration(e) => write_enum(&mut types, e, &mut registrations),
            _ => {}
        }
    }

    let globals =
        unit.globals.iter().filter(|g| g.exported && g.must_generate()).collect::<Vec<_>>();
    for global in &globals {
        let class = ident(&global.name);
        let _ = writeln!(types, "public sealed class {class}\n{{");
        let _ =
            writeln!(types, "    private readonly global::Slint.ComponentInstance _instance;\n");
        let _ = writeln!(
            types,
            "    internal {class}(global::Slint.ComponentInstance instance) => _instance = instance;\n"
        );
        write_members(
            &mut types,
            &global.public_properties,
            &Target { instance: "_instance", global: Some(&global.name) },
            &mut Members::new(&[class.as_str(), "_instance"]),
            &mut models,
        );
        let _ = writeln!(types, "}}\n");
    }

    let expected_globals = globals
        .iter()
        .map(|g| format!("[{}] = {}", string_literal(&g.name), expected_api(&g.public_properties)))
        .join(", ");

    for component in &unit.public_components {
        let class = ident(&component.name);
        let mut members = Members::new(&[
            class.as_str(),
            "Instance",
            "Create",
            "Show",
            "Hide",
            "Run",
            "Dispose",
            "Equals",
            "GetHashCode",
            "GetType",
            "ToString",
        ]);

        let _ =
            writeln!(types, "public sealed partial class {class} : global::System.IDisposable\n{{");
        let _ = writeln!(
            types,
            "    private {class}(global::Slint.ComponentInstance instance) => Instance = instance;\n"
        );
        let _ = writeln!(
            types,
            "    /// <summary>The untyped instance, for members this class doesn't cover.</summary>"
        );
        let _ = writeln!(types, "    public global::Slint.ComponentInstance Instance {{ get; }}\n");
        let _ = writeln!(
            types,
            "    public static {class} Create() => new(SlintFile.Create({}, {}));\n",
            string_literal(&component.name),
            expected_api(&component.public_properties)
        );
        let _ = writeln!(types, "    public void Show() => Instance.Show();");
        let _ = writeln!(types, "    public void Hide() => Instance.Hide();");
        let _ = writeln!(types, "    public void Run() => Instance.Run();");
        let _ = writeln!(types, "    public void Dispose() => Instance.Dispose();\n");
        for global in &globals {
            let class = ident(&global.name);
            let member = members.claim(class.clone());
            let _ = writeln!(types, "    public {class} {member} => new(Instance);");
        }
        write_members(
            &mut types,
            &component.public_properties,
            &Target { instance: "Instance", global: None },
            &mut members,
            &mut models,
        );
        let _ = writeln!(types, "}}\n");
    }

    for element in &models {
        let _ = writeln!(registrations, "        {CONVERSION}.RegisterModel<{element}>();");
    }

    let mut out = String::new();
    let _ = writeln!(out, "// <auto-generated>");
    let _ = writeln!(out, "// slint-compiler generated this file from {file_name}; don't edit it.");
    let _ = writeln!(out, "// </auto-generated>\n");
    let _ = writeln!(out, "#nullable enable");
    // Generated members have no documentation comments.
    let _ = writeln!(out, "#pragma warning disable CS1591\n");
    if let Some(namespace) = &config.namespace {
        let _ = writeln!(out, "namespace {namespace};\n");
    }
    out.push_str(&types);
    let _ = writeln!(out, "file static class SlintFile\n{{");
    let _ = writeln!(out, "    private static global::Slint.CompilationResult? s_result;\n");
    let _ = writeln!(out, "    static SlintFile()\n    {{\n{registrations}    }}\n");
    let _ = writeln!(
        out,
        "    public static global::Slint.ComponentInstance Create(string name, global::Slint.Generated.ExpectedApi api)\n    {{"
    );
    let _ = writeln!(
        out,
        "        s_result ??= global::Slint.Generated.GeneratedApi.Load(global::System.IO.Path.Combine(global::System.AppContext.BaseDirectory, {}), {});",
        string_literal(&runtime_path),
        compiler_config.style.as_deref().map_or("null".into(), string_literal)
    );
    let _ = writeln!(
        out,
        "        return global::Slint.Generated.GeneratedApi.Create(s_result, {}, name, api, new global::System.Collections.Generic.Dictionary<string, global::Slint.Generated.ExpectedApi> {{ {expected_globals} }});",
        string_literal(&format!("the C# code generated from {file_name}"))
    );
    let _ = writeln!(out, "    }}\n}}");
    Ok(out)
}

#[test]
fn csharp_identifiers() {
    assert_eq!(ident("first-name"), "FirstName");
    assert_eq!(ident("first_name"), "FirstName");
    assert_eq!(ident("_private-thing"), "_PrivateThing");
    assert_eq!(ident("value"), "Value");

    let mut members = Members::new(&["Show"]);
    assert_eq!(members.claim(ident("show")), "Show_");
    assert_eq!(members.claim("OnToggle".into()), "OnToggle");
    assert_eq!(members.claim("OnToggle".into()), "OnToggle_");
}

#[test]
fn csharp_string_literals() {
    assert_eq!(string_literal("a\"b\\c"), r#""a\"b\\c""#);
    assert_eq!(string_literal("wörld\n"), "\"wörld\\u000a\"");
}
