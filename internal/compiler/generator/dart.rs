// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Dart wrapper generator.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::io;
use std::sync::Arc;

use itertools::Either;

use crate::CompilerConfiguration;
use crate::langtype::{Function, Struct, StructName, Type};
use crate::llr;
use crate::object_tree::{Document, PropertyVisibility};

const DART_KEYWORDS: &[&str] = &[
    "Function",
    "abstract",
    "as",
    "assert",
    "async",
    "augment",
    "await",
    "base",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "covariant",
    "default",
    "deferred",
    "do",
    "dynamic",
    "else",
    "enum",
    "export",
    "extends",
    "extension",
    "external",
    "factory",
    "false",
    "final",
    "finally",
    "for",
    "get",
    "hide",
    "if",
    "implements",
    "import",
    "in",
    "inout",
    "interface",
    "is",
    "late",
    "library",
    "mixin",
    "native",
    "new",
    "null",
    "of",
    "on",
    "operator",
    "out",
    "part",
    "patch",
    "required",
    "rethrow",
    "return",
    "sealed",
    "set",
    "show",
    "source",
    "static",
    "super",
    "switch",
    "sync",
    "this",
    "throw",
    "true",
    "try",
    "typedef",
    "var",
    "void",
    "when",
    "while",
    "with",
    "yield",
];

const COMPONENT_RESERVED_NAMES: &[&str] = &[
    "dispose",
    "global",
    "hashCode",
    "hide",
    "instance",
    "invoke",
    "load",
    "loadSource",
    "noSuchMethod",
    "run",
    "runtimeType",
    "setCallback",
    "show",
    "toString",
];

const STRUCT_RESERVED_NAMES: &[&str] =
    &["fromSlint", "hashCode", "noSuchMethod", "runtimeType", "toSlint", "toString"];

const ENUM_RESERVED_NAMES: &[&str] = &[
    "fromSlint",
    "hashCode",
    "index",
    "name",
    "noSuchMethod",
    "runtimeType",
    "slintValue",
    "toString",
    "values",
];

const TYPE_RESERVED_NAMES: &[&str] = &[
    "BigInt",
    "Comparable",
    "DateTime",
    "Deprecated",
    "Duration",
    "Enum",
    "Error",
    "Exception",
    "Expando",
    "Future",
    "FutureOr",
    "Function",
    "Invocation",
    "Iterable",
    "Iterator",
    "List",
    "Map",
    "Match",
    "Never",
    "Null",
    "Object",
    "Pattern",
    "Record",
    "RegExp",
    "RuneIterator",
    "Runes",
    "Set",
    "StackTrace",
    "Stopwatch",
    "Stream",
    "String",
    "StringBuffer",
    "StringSink",
    "Symbol",
    "Type",
    "Uri",
    "WeakReference",
];

#[derive(Clone, Copy)]
enum Case {
    LowerCamel,
    UpperCamel,
}

fn dart_identifier(raw: &str, case: Case, reserved: &[&str]) -> io::Result<String> {
    if !raw.is_ascii() {
        return Err(error(format!(
            "Dart code generation doesn't support the non-ASCII public identifier {raw:?}"
        )));
    }

    let leading_private = raw.starts_with('_');
    let words = raw.split(['-', '_']).filter(|word| !word.is_empty()).collect::<Vec<_>>();
    if words.is_empty()
        || !words[0].chars().next().is_some_and(|character| character.is_ascii_alphabetic())
    {
        return Err(error(format!("{raw:?} can't be represented as a public Dart identifier")));
    }

    let mut result = String::new();
    for (index, word) in words.iter().enumerate() {
        let uppercase = index > 0 || matches!(case, Case::UpperCamel);
        let mut characters = word.chars();
        let first = characters.next().expect("empty words are filtered");
        result.push(if uppercase {
            first.to_ascii_uppercase()
        } else {
            first.to_ascii_lowercase()
        });
        result.extend(characters);
    }

    if leading_private
        || DART_KEYWORDS.contains(&result.as_str())
        || reserved.contains(&result.as_str())
    {
        result.push('_');
    }
    Ok(result)
}

fn lower_camel(raw: &str, reserved: &[&str]) -> io::Result<String> {
    dart_identifier(raw, Case::LowerCamel, reserved)
}

fn upper_camel(raw: &str) -> io::Result<String> {
    dart_identifier(raw, Case::UpperCamel, TYPE_RESERVED_NAMES)
}

fn error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn ensure_supported_type(ty: &Type) -> io::Result<()> {
    ensure_supported_type_impl(ty, &mut BTreeSet::new())
}

fn ensure_supported_type_impl(ty: &Type, visited_structs: &mut BTreeSet<String>) -> io::Result<()> {
    match ty {
        Type::Void
        | Type::Int32
        | Type::Float32
        | Type::Duration
        | Type::Angle
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Rem
        | Type::Percent
        | Type::UnitProduct(_)
        | Type::String
        | Type::Color
        | Type::Brush
        | Type::Image
        | Type::Bool
        | Type::Enumeration(_) => Ok(()),
        Type::Array(element) => ensure_supported_type_impl(element, visited_structs),
        Type::Struct(structure) => {
            if let StructName::User { name, node, .. } = &structure.name {
                let identity = format!(
                    "{}:{:?}:{name}",
                    node.source_file().path().display(),
                    node.text_range()
                );
                if !visited_structs.insert(identity) {
                    return Ok(());
                }
            }
            for field in structure.fields.values() {
                ensure_supported_type_impl(field, visited_structs)?;
            }
            Ok(())
        }
        Type::Callback(function) | Type::Function(function) => {
            for argument in &function.args {
                ensure_supported_type_impl(argument, visited_structs)?;
            }
            ensure_supported_type_impl(&function.return_type, visited_structs)
        }
        _ => Err(error(format!("Dart code generation doesn't support the public Slint type {ty}"))),
    }
}

fn dart_string(value: &str) -> String {
    // JSON and Dart use the same double-quoted escapes except that `$` starts
    // interpolation in Dart. Escape it after JSON serialization so paths and
    // raw Slint names always remain literal strings in generated source.
    serde_json::to_string(value).expect("serializing a string can't fail").replace('$', r"\$")
}

#[derive(Default)]
struct Scope {
    names: HashMap<String, String>,
}

impl Scope {
    fn claim(&mut self, generated: &str, raw: &str, context: &str) -> io::Result<()> {
        if let Some(previous) = self.names.get(generated) {
            return Err(error(format!(
                "Slint identifiers {previous:?} and {raw:?} both generate the Dart identifier {generated:?} in {context}"
            )));
        }
        self.names.insert(generated.to_owned(), raw.to_owned());
        Ok(())
    }
}

fn dart_type(ty: &Type) -> io::Result<String> {
    Ok(match ty {
        Type::Void => "void".into(),
        Type::Int32 => "int".into(),
        Type::Float32
        | Type::Duration
        | Type::Angle
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Rem
        | Type::Percent
        | Type::UnitProduct(_) => "double".into(),
        Type::String | Type::Color | Type::Brush | Type::Image => "String".into(),
        Type::Bool => "bool".into(),
        Type::Array(element) => format!("List<{}>", dart_type(element)?),
        Type::Struct(structure) => match &structure.name {
            StructName::User { name, .. } => upper_camel(name)?,
            _ => "Map<String, Object?>".into(),
        },
        Type::Enumeration(enumeration) => upper_camel(&enumeration.name)?,
        _ => "Object?".into(),
    })
}

fn from_slint(ty: &Type, expression: &str) -> io::Result<String> {
    Ok(match ty {
        Type::Void => "null".into(),
        Type::Int32 => format!("({expression} as num).toInt()"),
        Type::Float32
        | Type::Duration
        | Type::Angle
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Rem
        | Type::Percent
        | Type::UnitProduct(_) => format!("({expression} as num).toDouble()"),
        Type::String | Type::Color | Type::Brush | Type::Image => {
            format!("{expression} as String")
        }
        Type::Bool => format!("{expression} as bool"),
        Type::Array(element) => format!(
            "({expression} as List<Object?>).map((value) => {}).toList()",
            from_slint(element, "value")?
        ),
        Type::Struct(structure) => match &structure.name {
            StructName::User { name, .. } => {
                format!("{}.fromSlint({expression})", upper_camel(name)?)
            }
            _ => format!("({expression} as Map).cast<String, Object?>()"),
        },
        Type::Enumeration(enumeration) => {
            format!("{}.fromSlint({expression})", upper_camel(&enumeration.name)?)
        }
        _ => expression.into(),
    })
}

fn to_slint(ty: &Type, expression: &str) -> io::Result<String> {
    Ok(match ty {
        Type::Array(element) => {
            format!("{expression}.map((value) => {}).toList()", to_slint(element, "value")?)
        }
        Type::Struct(structure) if matches!(&structure.name, StructName::User { .. }) => {
            format!("{expression}.toSlint()")
        }
        Type::Enumeration(_) => format!("{expression}.slintValue"),
        Type::Void => "null".into(),
        _ => expression.into(),
    })
}

fn argument_names(function: &Function, reserved: &[&str]) -> io::Result<Vec<String>> {
    let mut scope = Scope::default();
    function
        .args
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let raw = function
                .arg_names
                .get(index)
                .filter(|name| !name.is_empty())
                .map_or_else(|| format!("arg{index}"), ToString::to_string);
            let generated = lower_camel(&raw, reserved)?;
            scope.claim(&generated, &raw, "a function argument list")?;
            Ok(generated)
        })
        .collect()
}

fn typed_arguments(function: &Function, reserved: &[&str]) -> io::Result<(Vec<String>, String)> {
    let names = argument_names(function, reserved)?;
    let declaration = function
        .args
        .iter()
        .zip(&names)
        .map(|(ty, name)| Ok(format!("{} {name}", dart_type(ty)?)))
        .collect::<io::Result<Vec<_>>>()?
        .join(", ");
    Ok((names, declaration))
}

fn generate_property(
    output: &mut String,
    raw: &str,
    property: &llr::PublicProperty,
    instance: &str,
    global: Option<&str>,
) -> io::Result<()> {
    let name = lower_camel(raw, COMPONENT_RESERVED_NAMES)?;
    let ty = dart_type(&property.ty)?;
    let raw = dart_string(raw);
    let get = match global {
        Some(global) => format!("{instance}.global({}).getProperty({raw})", dart_string(global)),
        None => format!("{instance}.getProperty({raw})"),
    };
    writeln!(output, "  {ty} get {name} => {};", from_slint(&property.ty, &get)?).unwrap();
    if property.visibility != PropertyVisibility::Output {
        let set = match global {
            Some(global) => format!(
                "{instance}.global({}).setProperty({raw}, {})",
                dart_string(global),
                to_slint(&property.ty, "value")?
            ),
            None => format!("{instance}.setProperty({raw}, {})", to_slint(&property.ty, "value")?),
        };
        writeln!(output, "  set {name}({ty} value) => {set};").unwrap();
    }
    Ok(())
}

fn generate_invoker(
    output: &mut String,
    raw: &str,
    function: &Function,
    instance: &str,
    global: Option<&str>,
) -> io::Result<()> {
    let name = format!("invoke{}", upper_camel(raw)?);
    let return_type = dart_type(&function.return_type)?;
    let (names, declaration) = typed_arguments(function, &["instance"])?;
    let args = function
        .args
        .iter()
        .zip(&names)
        .map(|(ty, name)| to_slint(ty, name))
        .collect::<io::Result<Vec<_>>>()?
        .join(", ");
    let call = match global {
        Some(global) => format!(
            "{instance}.global({}).invoke({}, [{args}])",
            dart_string(global),
            dart_string(raw)
        ),
        None => format!("{instance}.invoke({}, [{args}])", dart_string(raw)),
    };
    if function.return_type == Type::Void {
        writeln!(output, "  void {name}({declaration}) {{ {call}; }}").unwrap();
    } else {
        writeln!(
            output,
            "  {return_type} {name}({declaration}) => {};",
            from_slint(&function.return_type, &call)?
        )
        .unwrap();
    }
    Ok(())
}

fn generate_handler(
    output: &mut String,
    raw: &str,
    function: &Function,
    instance: &str,
    global: Option<&str>,
) -> io::Result<()> {
    let name = format!("on{}", upper_camel(raw)?);
    let return_type = dart_type(&function.return_type)?;
    let (_names, declaration) = typed_arguments(function, &["arguments", "handler"])?;
    let handler_type = if function.return_type == Type::Void {
        format!("void Function({declaration})")
    } else {
        format!("{return_type} Function({declaration})")
    };
    let arguments = function
        .args
        .iter()
        .enumerate()
        .map(|(index, ty)| from_slint(ty, &format!("arguments[{index}]")))
        .collect::<io::Result<Vec<_>>>()?
        .join(", ");
    let set_callback = match global {
        Some(global) => format!(
            "{instance}.global({}).setCallback({}, callback)",
            dart_string(global),
            dart_string(raw)
        ),
        None => format!("{instance}.setCallback({}, callback)", dart_string(raw)),
    };
    writeln!(output, "  void {name}({handler_type} handler) {{").unwrap();
    writeln!(output, "    Object? callback(List<Object?> arguments) {{").unwrap();
    if function.return_type == Type::Void {
        writeln!(output, "      handler({arguments});").unwrap();
        writeln!(output, "      return null;").unwrap();
    } else {
        writeln!(
            output,
            "      return {};",
            to_slint(&function.return_type, &format!("handler({arguments})"))?
        )
        .unwrap();
    }
    writeln!(output, "    }}").unwrap();
    writeln!(output, "    {set_callback};").unwrap();
    writeln!(output, "  }}").unwrap();
    Ok(())
}

fn generate_members(
    output: &mut String,
    properties: &llr::PublicProperties,
    instance: &str,
    global: Option<&str>,
    context: &str,
) -> io::Result<Scope> {
    let mut scope = Scope::default();
    for property in properties.values() {
        let raw = property.display_name.as_str();
        ensure_supported_type(&property.ty).map_err(|unsupported| {
            error(format!("{unsupported} in public member {raw:?} of {context}"))
        })?;
        match &property.ty {
            Type::Callback(function) => {
                let invoke = format!("invoke{}", upper_camel(raw)?);
                let handler = format!("on{}", upper_camel(raw)?);
                scope.claim(&invoke, raw, context)?;
                scope.claim(&handler, raw, context)?;
                generate_invoker(output, raw, function, instance, global)?;
                generate_handler(output, raw, function, instance, global)?;
            }
            Type::Function(function) => {
                let invoke = format!("invoke{}", upper_camel(raw)?);
                scope.claim(&invoke, raw, context)?;
                generate_invoker(output, raw, function, instance, global)?;
            }
            _ => {
                let name = lower_camel(raw, COMPONENT_RESERVED_NAMES)?;
                scope.claim(&name, raw, context)?;
                generate_property(output, raw, property, instance, global)?;
            }
        }
    }
    Ok(scope)
}

fn generate_struct(output: &mut String, structure: &Arc<Struct>) -> io::Result<()> {
    let StructName::User { name, .. } = &structure.name else { return Ok(()) };
    let name = upper_camel(name)?;
    let mut scope = Scope::default();
    let fields = structure
        .fields
        .iter()
        .map(|(raw, ty)| {
            let generated = lower_camel(raw, STRUCT_RESERVED_NAMES)?;
            scope.claim(&generated, raw, &format!("struct {name}"))?;
            Ok((raw.as_str(), generated, ty))
        })
        .collect::<io::Result<Vec<_>>>()?;

    writeln!(output, "class {name} {{").unwrap();
    write!(output, "  const {name}({{").unwrap();
    for (_, generated, _) in &fields {
        write!(output, "required this.{generated}, ").unwrap();
    }
    writeln!(output, "}});").unwrap();
    for (_, generated, ty) in &fields {
        writeln!(output, "  final {} {generated};", dart_type(ty)?).unwrap();
    }
    writeln!(output, "  factory {name}.fromSlint(Object? value) {{").unwrap();
    writeln!(output, "    final map = (value as Map).cast<String, Object?>();").unwrap();
    writeln!(output, "    return {name}(").unwrap();
    for (raw, generated, ty) in &fields {
        writeln!(
            output,
            "      {generated}: {},",
            from_slint(ty, &format!("map[{}]", dart_string(raw)))?
        )
        .unwrap();
    }
    writeln!(output, "    );").unwrap();
    writeln!(output, "  }}").unwrap();
    writeln!(output, "  Map<String, Object?> toSlint() => {{").unwrap();
    for (raw, generated, ty) in &fields {
        writeln!(output, "    {}: {},", dart_string(raw), to_slint(ty, generated)?).unwrap();
    }
    writeln!(output, "  }};").unwrap();
    writeln!(output, "}}\n").unwrap();
    Ok(())
}

fn generate_enum(
    output: &mut String,
    enumeration: &Arc<crate::langtype::Enumeration>,
) -> io::Result<()> {
    let name = upper_camel(&enumeration.name)?;
    let mut scope = Scope::default();
    let values = enumeration
        .values
        .iter()
        .map(|raw| {
            let generated = lower_camel(raw, ENUM_RESERVED_NAMES)?;
            scope.claim(&generated, raw, &format!("enum {name}"))?;
            Ok((raw.as_str(), generated))
        })
        .collect::<io::Result<Vec<_>>>()?;

    writeln!(output, "enum {name} {{").unwrap();
    for (index, (raw, generated)) in values.iter().enumerate() {
        let separator = if index + 1 == values.len() { ";" } else { "," };
        writeln!(output, "  {generated}({}){separator}", dart_string(raw)).unwrap();
    }
    writeln!(output, "  const {name}(this.slintValue);").unwrap();
    writeln!(output, "  final String slintValue;").unwrap();
    writeln!(output, "  static {name} fromSlint(Object? value) {{").unwrap();
    writeln!(output, "    final raw = value as String;").unwrap();
    writeln!(output, "    final dot = raw.lastIndexOf('.');").unwrap();
    writeln!(output, "    final suffix = dot < 0 ? raw : raw.substring(dot + 1);").unwrap();
    writeln!(output, "    return values.firstWhere((value) => value.slintValue == suffix);")
        .unwrap();
    writeln!(output, "  }}").unwrap();
    writeln!(output, "}}\n").unwrap();
    Ok(())
}

fn same_declaration(
    left: Option<&crate::langtype::DeclNode>,
    right: Option<&crate::langtype::DeclNode>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.source_file().path() == right.source_file().path()
                && left.text_range() == right.text_range()
        }
        (None, None) => true,
        _ => false,
    }
}

fn collect_types(
    ty: &Type,
    structs: &mut BTreeMap<String, Arc<Struct>>,
    enums: &mut BTreeMap<String, Arc<crate::langtype::Enumeration>>,
) -> io::Result<()> {
    match ty {
        Type::Enumeration(enumeration) => {
            if let Some(previous) = enums.get(enumeration.name.as_str())
                && !same_declaration(previous.node.as_ref(), enumeration.node.as_ref())
            {
                return Err(error(format!(
                    "Distinct Slint enum types named {:?} cannot both be represented in one generated Dart library",
                    enumeration.name
                )));
            }
            enums.entry(enumeration.name.to_string()).or_insert_with(|| enumeration.clone());
        }
        Type::Array(element) => collect_types(element, structs, enums)?,
        Type::Struct(structure) => {
            if let StructName::User { name, .. } = &structure.name {
                if let Some(previous) = structs.get(name.as_str()) {
                    if !same_declaration(previous.node(), structure.node()) {
                        return Err(error(format!(
                            "Distinct Slint struct types named {name:?} cannot both be represented in one generated Dart library"
                        )));
                    }
                    return Ok(());
                }
                structs.insert(name.to_string(), structure.clone());
            }
            for field in structure.fields.values() {
                collect_types(field, structs, enums)?;
            }
        }
        Type::Callback(function) | Type::Function(function) => {
            for argument in &function.args {
                collect_types(argument, structs, enums)?;
            }
            collect_types(&function.return_type, structs, enums)?;
        }
        _ => {}
    }
    Ok(())
}

fn source_path(doc: &Document) -> io::Result<String> {
    let path = std::path::absolute(
        doc.node
            .as_ref()
            .ok_or_else(|| error("Cannot determine the path of the main Slint file"))?
            .source_file
            .path(),
    )?;
    let current_dir = std::env::current_dir()?;
    Ok(pathdiff::diff_paths(&path, current_dir).unwrap_or(path).to_string_lossy().into_owned())
}

fn runtime_include_paths(compiler_config: &CompilerConfiguration) -> io::Result<Vec<String>> {
    let current_dir = std::env::current_dir()?;
    Ok(compiler_config
        .include_paths
        .iter()
        .map(|path| {
            pathdiff::diff_paths(path, &current_dir)
                .unwrap_or_else(|| path.clone())
                .to_string_lossy()
                .into_owned()
        })
        .collect())
}

fn write_runtime_load_options(
    output: &mut String,
    path: &str,
    compiler_config: &CompilerConfiguration,
    include_paths: &str,
) {
    writeln!(output, "    String path = {path},").unwrap();
    if let Some(style) = compiler_config.style.as_deref() {
        writeln!(output, "    String? style = {},", dart_string(style)).unwrap();
    } else {
        writeln!(output, "    String? style,").unwrap();
    }
    writeln!(output, "    List<String> includePaths = const [{include_paths}],").unwrap();
}

fn raw_component_names(doc: &Document) -> HashMap<String, String> {
    doc.exports
        .iter()
        .filter_map(|(export, item)| {
            let component = item.as_ref().left()?;
            (!component.is_global() && !component.is_interface())
                .then(|| (component.id.to_string(), export.name.to_string()))
        })
        .collect()
}

/// Generate typed Dart wrappers for the public API in `doc`.
pub fn generate(
    doc: &Document,
    compiler_config: &CompilerConfiguration,
    _destination_path: Option<&std::path::Path>,
) -> io::Result<String> {
    let unit = llr::lower_to_item_tree::lower_to_item_tree(doc, compiler_config);
    let raw_component_names = raw_component_names(doc);
    let mut output = String::new();
    writeln!(output, "// This file is auto-generated by Slint.\n").unwrap();
    writeln!(output, "import 'package:slint/slint.dart' as slint;\n").unwrap();

    let mut top_level = Scope::default();
    let mut aliases: HashMap<String, Vec<String>> = HashMap::new();
    for export in doc.exports.iter() {
        let (canonical, alias) = match &export.1 {
            Either::Left(component)
                if !component.is_global()
                    && unit.public_components.iter().any(|public| public.name == component.id)
                    && unit.public_components.iter().any(|public| public.name == export.0.name)
                    && export.0.name != component.id =>
            {
                (component.id.as_str(), export.0.name.as_str())
            }
            Either::Right(Type::Struct(structure)) => match &structure.name {
                StructName::User { name, .. } if export.0.name != *name => {
                    (name.as_str(), export.0.name.as_str())
                }
                _ => continue,
            },
            Either::Right(Type::Enumeration(enumeration)) if export.0.name != enumeration.name => {
                (enumeration.name.as_str(), export.0.name.as_str())
            }
            _ => continue,
        };
        aliases.entry(canonical.into()).or_default().push(alias.into());
    }

    let mut structs = BTreeMap::<String, Arc<Struct>>::new();
    let mut enums = BTreeMap::new();
    for ty in &doc.used_types.borrow().structs_and_enums {
        collect_types(ty, &mut structs, &mut enums)?;
    }
    for component in &unit.public_components {
        for property in component.public_properties.values() {
            collect_types(&property.ty, &mut structs, &mut enums)?;
        }
    }
    for global in &unit.globals {
        for property in global.public_properties.values() {
            collect_types(&property.ty, &mut structs, &mut enums)?;
        }
    }

    for (raw, structure) in &structs {
        let name = upper_camel(raw)?;
        top_level.claim(&name, raw, "the generated Dart library")?;
        generate_struct(&mut output, structure)?;
    }
    for (raw, enumeration) in &enums {
        let name = upper_camel(raw)?;
        top_level.claim(&name, raw, "the generated Dart library")?;
        generate_enum(&mut output, enumeration)?;
    }

    let exported_globals =
        unit.globals.iter().filter(|global| global.exported && !global.is_builtin);
    for global in exported_globals.clone() {
        let name = upper_camel(&global.name)?;
        top_level.claim(&name, &global.name, "the generated Dart library")?;
        writeln!(output, "class {name} {{").unwrap();
        writeln!(output, "  {name}._(this._instance);").unwrap();
        writeln!(output, "  final slint.ComponentInstance _instance;").unwrap();
        generate_members(
            &mut output,
            &global.public_properties,
            "_instance",
            Some(&global.name),
            &format!("global {name}"),
        )?;
        writeln!(output, "}}\n").unwrap();
    }

    let raw_source_path = source_path(doc)?;
    let include_paths = runtime_include_paths(compiler_config)?
        .iter()
        .map(|path| dart_string(path))
        .collect::<Vec<_>>()
        .join(", ");
    let path = dart_string(&raw_source_path);
    for component in &unit.public_components {
        let raw_name = raw_component_names
            .get(component.name.as_str())
            .map_or(component.name.as_str(), String::as_str);
        let name = upper_camel(raw_name)?;
        top_level.claim(&name, raw_name, "the generated Dart library")?;
        writeln!(output, "class {name} implements slint.SlintComponent {{").unwrap();
        writeln!(output, "  {name}._(this.instance);").unwrap();
        writeln!(output, "  factory {name}.load({{").unwrap();
        write_runtime_load_options(&mut output, &path, compiler_config, &include_paths);
        writeln!(output, "  }}) => {name}._(slint.loadFile(").unwrap();
        writeln!(output, "    path,").unwrap();
        writeln!(output, "    component: {},", dart_string(raw_name)).unwrap();
        writeln!(output, "    style: style,").unwrap();
        writeln!(output, "    includePaths: includePaths,").unwrap();
        writeln!(output, "  ));").unwrap();
        writeln!(output, "  factory {name}.loadSource(").unwrap();
        writeln!(output, "    String source, {{").unwrap();
        write_runtime_load_options(&mut output, &path, compiler_config, &include_paths);
        writeln!(output, "  }}) => {name}._(slint.loadSource(").unwrap();
        writeln!(output, "    source,").unwrap();
        writeln!(output, "    path: path,").unwrap();
        writeln!(output, "    component: {},", dart_string(raw_name)).unwrap();
        writeln!(output, "    style: style,").unwrap();
        writeln!(output, "    includePaths: includePaths,").unwrap();
        writeln!(output, "  ));").unwrap();
        writeln!(output, "  @override").unwrap();
        writeln!(output, "  final slint.ComponentInstance instance;").unwrap();
        writeln!(output, "  void show() => instance.show();").unwrap();
        writeln!(output, "  void hide() => instance.hide();").unwrap();
        writeln!(output, "  void run() => instance.run();").unwrap();
        writeln!(output, "  void dispose() => instance.dispose();").unwrap();
        let mut scope = generate_members(
            &mut output,
            &component.public_properties,
            "instance",
            None,
            &format!("component {name}"),
        )?;
        for global in exported_globals.clone() {
            let global_type = upper_camel(&global.name)?;
            for raw in global.aliases.iter().chain(std::iter::once(&global.name)) {
                let getter = lower_camel(raw, COMPONENT_RESERVED_NAMES)?;
                scope.claim(&getter, raw, &format!("component {name}"))?;
                writeln!(output, "  {global_type} get {getter} => {global_type}._(instance);")
                    .unwrap();
            }
        }
        writeln!(output, "}}\n").unwrap();
    }

    let mut emitted_aliases = BTreeSet::new();
    let mut aliases = aliases.into_iter().collect::<Vec<_>>();
    aliases.sort_by(|left, right| left.0.cmp(&right.0));
    for (canonical, mut aliases) in aliases {
        let canonical_name = upper_camel(&canonical)?;
        aliases.sort();
        for alias in aliases {
            let alias_name = upper_camel(&alias)?;
            top_level.claim(&alias_name, &alias, "the generated Dart library")?;
            if emitted_aliases.insert(alias_name.clone()) {
                writeln!(output, "typedef {alias_name} = {canonical_name};").unwrap();
            }
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dart_names_use_camel_case() {
        assert_eq!(lower_camel("todo-model", &[]).unwrap(), "todoModel");
        assert_eq!(
            lower_camel("apply_sorting_and_filtering", &[]).unwrap(),
            "applySortingAndFiltering"
        );
        assert_eq!(lower_camel("statusString", &[]).unwrap(), "statusString");
        assert_eq!(upper_camel("main-window").unwrap(), "MainWindow");
        assert_eq!(upper_camel("List").unwrap(), "List_");
        assert_eq!(lower_camel("class", &[]).unwrap(), "class_");
        assert_eq!(lower_camel("_leading-name", &[]).unwrap(), "leadingName_");
    }

    #[test]
    fn dart_names_reject_non_ascii() {
        assert!(lower_camel("café", &[]).unwrap_err().to_string().contains("non-ASCII"));
    }

    #[test]
    fn scope_rejects_camel_case_collisions() {
        let mut scope = Scope::default();
        scope.claim("fooBar", "foo-bar", "test").unwrap();
        let error = scope.claim("fooBar", "fooBar", "test").unwrap_err();
        assert!(error.to_string().contains("both generate"));
    }

    #[test]
    fn dart_strings_escape_interpolation() {
        assert_eq!(dart_string(r"lib/foo$bar.slint"), r#""lib/foo\$bar.slint""#);
    }

    #[test]
    fn invoker_arguments_do_not_shadow_the_runtime_instance() {
        let function = Function {
            return_type: Type::Int32,
            args: vec![Type::Int32],
            arg_names: vec!["instance".into()],
        };
        assert_eq!(argument_names(&function, &["instance"]).unwrap(), ["instance_"]);
    }

    #[test]
    fn unsupported_public_types_are_rejected() {
        let error = ensure_supported_type(&Type::Keys).unwrap_err();
        assert!(error.to_string().contains("public Slint type keys"));

        let nested = Type::Array(Arc::new(Type::MouseCursor));
        let error = ensure_supported_type(&nested).unwrap_err();
        assert!(error.to_string().contains("MouseCursor"));
    }

    #[test]
    fn runtime_include_paths_are_relative_to_the_runtime_working_directory() {
        let mut compiler_config = CompilerConfiguration::new(crate::generator::OutputFormat::Dart);
        let current_dir = std::env::current_dir().unwrap();
        compiler_config.include_paths = vec![current_dir.join("lib/ui/includes")];
        assert_eq!(
            runtime_include_paths(&compiler_config).unwrap(),
            [std::path::Path::new("lib/ui/includes").to_string_lossy()]
        );
    }
}
