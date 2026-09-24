// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore subcomponent structty enumty

use std::collections::HashMap;
use std::sync::Arc;

use smol_str::{SmolStr, StrExt, format_smolstr};

use std::sync::OnceLock;

use std::collections::HashSet;

fn is_typescript_keyword(word: &str) -> bool {
    static TS_KEYWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    #[rustfmt::skip]
    let keywords = TS_KEYWORDS.get_or_init(|| HashSet::from([
        "abstract", "as", "async", "await", "break", "case", "catch", "class", "const",
        "constructor", "continue", "debugger", "declare", "default", "delete", "do", "else",
        "enum", "export", "extends", "false", "finally", "for", "from", "function", "get",
        "if", "implements", "import", "in", "instanceof", "interface", "is", "let", "module",
        "namespace", "new", "null", "of", "package", "private", "protected", "public",
        "require", "return", "set", "static", "super", "switch", "this", "throw", "true",
        "try", "type", "typeof", "var", "void", "while", "with", "yield",
    ]));
    keywords.contains(word)
}

/// The built-in enums the Node API exposes as `slint.language.X`. The `pub` ones, which
/// is the same set `slint::language` re-exports in Rust; the rest are not public API in
/// any language binding.
fn is_public_builtin_enum(name: &str) -> bool {
    static PUBLIC: OnceLock<HashSet<&'static str>> = OnceLock::new();
    PUBLIC
        .get_or_init(|| {
            let mut names = HashSet::new();
            macro_rules! collect_public {
                ($(
                    $(#[doc = $enum_doc:literal])*
                    $(#[non_exhaustive])?
                    $vis:vis enum $Name:ident { $( $(#[doc = $value_doc:literal])* $Value:ident, )* }
                )*) => {
                    $( if stringify!($vis) == "pub" { names.insert(stringify!($Name)); } )*
                };
            }
            i_slint_common::for_each_enums!(collect_public);
            names
        })
        .contains(name)
}

/// The name of a member: a property, a struct field, or an enum variant.
/// A keyword is a valid member name, so only the dashes need replacing.
fn member(name: &str) -> SmolStr {
    if name.contains('-') { name.replace_smolstr("-", "_") } else { SmolStr::from(name) }
}

/// The name of a declaration: a type or a binding. Those can't be keywords.
pub fn ident(ident: &str) -> SmolStr {
    let normalized = member(ident);
    if is_typescript_keyword(normalized.as_str()) {
        format_smolstr!("{}_", normalized)
    } else {
        normalized
    }
}

struct TsProperty {
    name: SmolStr,
    ty: SmolStr,
    read_only: bool,
}

impl From<(&SmolStr, &llr::PublicProperty)> for TsProperty {
    fn from((name, llr_prop): (&SmolStr, &llr::PublicProperty)) -> Self {
        Self { name: member(name), ty: ts_type_name(&llr_prop.ty), read_only: llr_prop.read_only() }
    }
}

enum ComponentType<'a> {
    Global,
    Component { associated_globals: &'a [TsComponent] },
}

struct TsComponent {
    name: SmolStr,
    properties: Vec<TsProperty>,
    aliases: Vec<SmolStr>,
}

impl TsComponent {
    fn generate(&self, ty: ComponentType<'_>, file: &mut typescript_ast::File) {
        let mut interface = typescript_ast::Interface {
            name: self.name.clone(),
            extends: None,
            ..Default::default()
        };

        interface.fields = self
            .properties
            .iter()
            .map(|prop| typescript_ast::Field {
                name: prop.name.clone(),
                ty: prop.ty.clone(),
                read_only: prop.read_only,
            })
            .chain(
                match ty {
                    ComponentType::Global => None,
                    ComponentType::Component { associated_globals } => Some(associated_globals),
                }
                .into_iter()
                .flat_map(|globals| globals.iter())
                .flat_map(|glob| {
                    std::iter::once(&glob.name).chain(glob.aliases.iter()).map(|exported_name| {
                        typescript_ast::Field {
                            name: member(exported_name),
                            ty: glob.name.clone(),
                            read_only: true,
                        }
                    })
                }),
            )
            .collect();

        file.declarations.push(typescript_ast::Declaration::Interface(interface));

        file.declarations.extend(type_aliases(&self.name, &self.aliases));
    }
}

impl From<&llr::PublicComponent> for TsComponent {
    fn from(llr_compo: &llr::PublicComponent) -> Self {
        Self {
            name: ident(&llr_compo.name),
            properties: llr_compo.public_properties.iter().map(From::from).collect(),
            aliases: Vec::new(),
        }
    }
}

impl From<&llr::GlobalComponent> for TsComponent {
    fn from(llr_global: &llr::GlobalComponent) -> Self {
        Self {
            name: ident(&llr_global.name),
            properties: llr_global.public_properties.iter().map(From::from).collect(),
            aliases: llr_global.aliases.iter().map(|exported_name| ident(exported_name)).collect(),
        }
    }
}

struct TsStructField {
    name: SmolStr,
    ty: SmolStr,
}

struct TsStruct {
    name: SmolStr,
    fields: Vec<TsStructField>,
    aliases: Vec<SmolStr>,
}

struct AnonymousStruct;

impl TryFrom<&Arc<crate::langtype::Struct>> for TsStruct {
    type Error = AnonymousStruct;

    fn try_from(structty: &Arc<crate::langtype::Struct>) -> Result<Self, Self::Error> {
        let StructName::User { name, .. } = &structty.name else {
            return Err(AnonymousStruct);
        };
        Ok(Self {
            name: ident(name),
            fields: structty
                .fields
                .iter()
                .map(|(name, ty)| TsStructField { name: member(name), ty: ts_type_name(ty) })
                .collect(),
            aliases: Vec::new(),
        })
    }
}

impl From<&TsStruct> for typescript_ast::Declaration {
    fn from(ts_struct: &TsStruct) -> Self {
        typescript_ast::Declaration::Interface(typescript_ast::Interface {
            name: ts_struct.name.clone(),
            fields: ts_struct
                .fields
                .iter()
                .map(|field| typescript_ast::Field {
                    name: field.name.clone(),
                    ty: field.ty.clone(),
                    read_only: false,
                })
                .collect(),
            ..Default::default()
        })
    }
}

fn type_aliases<'a>(
    name: &'a SmolStr,
    aliases: &'a [SmolStr],
) -> impl ExactSizeIterator<Item = typescript_ast::Declaration> + 'a {
    aliases.iter().map(|alias| {
        typescript_ast::Declaration::TypeAlias(typescript_ast::TypeAlias {
            name: ident(alias),
            value: name.clone(),
        })
    })
}

struct TsEnum {
    name: SmolStr,
    variants: Vec<typescript_ast::EnumVariant>,
    aliases: Vec<SmolStr>,
}

impl From<&Arc<crate::langtype::Enumeration>> for TsEnum {
    fn from(enumty: &Arc<crate::langtype::Enumeration>) -> Self {
        Self {
            name: ident(&enumty.name),
            variants: enumty
                .values
                .iter()
                .map(|val| typescript_ast::EnumVariant { name: member(val), value: val.clone() })
                .collect(),
            aliases: Vec::new(),
        }
    }
}

impl From<&TsEnum> for typescript_ast::Declaration {
    fn from(ts_enum: &TsEnum) -> Self {
        typescript_ast::Declaration::Enum(typescript_ast::Enum {
            name: ts_enum.name.clone(),
            variants: ts_enum.variants.clone(),
        })
    }
}

enum TsStructOrEnum {
    Struct(TsStruct),
    Enum(TsEnum),
}

impl TsStructOrEnum {
    fn declaration(&self) -> typescript_ast::Declaration {
        match self {
            TsStructOrEnum::Struct(ts_struct) => ts_struct.into(),
            TsStructOrEnum::Enum(ts_enum) => ts_enum.into(),
        }
    }
}

impl TsStructOrEnum {
    fn generate_aliases(&self, file: &mut typescript_ast::File) {
        let (name, aliases) = match self {
            TsStructOrEnum::Struct(s) => (&s.name, &s.aliases),
            TsStructOrEnum::Enum(e) => (&e.name, &e.aliases),
        };
        file.declarations.extend(type_aliases(name, aliases));
    }
}

struct TsModule {
    globals: Vec<TsComponent>,
    components: Vec<TsComponent>,
    structs_and_enums: Vec<TsStructOrEnum>,
}

/// This module contains data structures that represent a TypeScript file.
/// It is rendered into TypeScript code using the Display trait.
mod typescript_ast {
    use std::fmt::{Display, Error, Formatter};

    use smol_str::SmolStr;

    /// A full TypeScript file
    #[derive(Default, Debug)]
    pub struct File {
        pub imports: Vec<SmolStr>,
        pub declarations: Vec<Declaration>,
        pub trailing_code: Vec<SmolStr>,
    }

    impl Display for File {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            writeln!(f, "// This file is auto-generated\n")?;
            for import in &self.imports {
                writeln!(f, "{}", import)?;
            }
            if !self.imports.is_empty() {
                writeln!(f)?;
            }
            for decl in &self.declarations {
                writeln!(f, "{}", decl)?;
            }
            for code in &self.trailing_code {
                writeln!(f, "{}", code)?;
            }
            Ok(())
        }
    }

    #[derive(Debug)]
    pub enum Declaration {
        Interface(Interface),
        Enum(Enum),
        TypeAlias(TypeAlias),
    }

    impl Display for Declaration {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                Declaration::Interface(interface) => write!(f, "{}", interface),
                Declaration::Enum(en) => write!(f, "{}", en),
                Declaration::TypeAlias(alias) => write!(f, "{}", alias),
            }
        }
    }

    #[derive(Debug, Default)]
    pub struct Interface {
        pub name: SmolStr,
        pub extends: Option<SmolStr>,
        pub fields: Vec<Field>,
    }

    impl Display for Interface {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            if let Some(extends) = self.extends.as_ref() {
                writeln!(f, "export interface {} extends {} {{", self.name, extends)?;
            } else {
                writeln!(f, "export interface {} {{", self.name)?;
            }
            for field in &self.fields {
                writeln!(f, "    {};", field)?;
            }
            writeln!(f, "}}")?;
            Ok(())
        }
    }

    #[derive(Debug)]
    pub struct Field {
        pub name: SmolStr,
        pub ty: SmolStr,
        pub read_only: bool,
    }

    impl Display for Field {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            if self.read_only {
                write!(f, "readonly {}: {}", self.name, self.ty)
            } else {
                write!(f, "{}: {}", self.name, self.ty)
            }
        }
    }

    #[derive(Debug)]
    pub struct Enum {
        pub name: SmolStr,
        pub variants: Vec<EnumVariant>,
    }

    impl Display for Enum {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            // A union of the string literals the runtime uses, rather than a TypeScript
            // enum: a plain string stays assignable, and two files that both declare the
            // same built-in enum still describe the same type.
            let union = self
                .variants
                .iter()
                .map(|variant| format!("\"{}\"", variant.value))
                .collect::<Vec<_>>()
                .join(" | ");
            let union = if union.is_empty() { "never".into() } else { union };
            writeln!(f, "export type {} = {};", self.name, union)?;

            writeln!(f, "export declare const {}: {{", self.name)?;
            for variant in &self.variants {
                writeln!(f, "    readonly {}: \"{}\";", variant.name, variant.value)?;
            }
            writeln!(f, "}};")?;
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    pub struct EnumVariant {
        pub name: SmolStr,
        pub value: SmolStr,
    }

    #[derive(Debug)]
    pub struct TypeAlias {
        pub name: SmolStr,
        pub value: SmolStr,
    }

    impl Display for TypeAlias {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            writeln!(f, "export type {} = {};", self.name, self.value)
        }
    }
}

use crate::langtype::{StructName, Type};

use crate::CompilerConfiguration;
use crate::llr;
use crate::object_tree::Document;
use typescript_ast::*;

/// Returns the text of the TypeScript code produced by the given root component
pub fn generate(
    doc: &Document,
    compiler_config: &CompilerConfiguration,
    destination_path: Option<&std::path::Path>,
) -> std::io::Result<File> {
    let mut file = File { ..Default::default() };
    file.imports.push(SmolStr::new_static("import * as slint from \"slint-ui\";"));

    // The output describes the module that `slint-ui/register` makes of the `.slint` file,
    // so it is a declaration file. A destination named otherwise would silently be one too.
    if let Some(name) = destination_path.and_then(|p| p.file_name()).and_then(|n| n.to_str())
        && !name.ends_with(".d.ts")
    {
        return Err(std::io::Error::other(format!(
            "The TypeScript output is a declaration file, so '{name}' should be named '.d.ts'"
        )));
    }

    let llr = llr::lower_to_item_tree::lower_to_item_tree(doc, compiler_config);

    // A name kept only for compatibility is not exposed: TypeScript support is new, so a name
    // the .slint never exported was never reachable from it.
    let mut aliases: HashMap<&str, Vec<SmolStr>> = Default::default();
    for export in llr.type_exports.iter().filter(|e| e.is_alias() && !e.deprecated) {
        aliases
            .entry(export.internal_name.as_str())
            .or_default()
            .push(export.exported_name.clone());
    }
    let aliases_of = |name: &str| aliases.get(name).cloned().unwrap_or_default();

    let mut module =
        TsModule { globals: Vec::new(), components: Vec::new(), structs_and_enums: Vec::new() };

    for ty in &doc.used_types.borrow().structs_and_enums {
        match ty {
            Type::Struct(s) => module.structs_and_enums.extend(
                TsStruct::try_from(s).ok().and_then(|mut ts_struct| {
                    let StructName::User { name, .. } = &s.name else {
                        return None;
                    };
                    ts_struct.aliases = aliases_of(name);
                    Some(TsStructOrEnum::Struct(ts_struct))
                }),
            ),
            // A built-in enum is not declared here: it is `slint.language.X` or nothing.
            Type::Enumeration(en) if en.node.is_some() => {
                module.structs_and_enums.push({
                    let mut ts_enum = TsEnum::from(en);
                    ts_enum.aliases = aliases_of(&en.name);
                    TsStructOrEnum::Enum(ts_enum)
                });
            }
            _ => {}
        }
    }

    let globals = llr.globals.iter().filter(|glob| glob.exported && glob.must_generate());

    module.globals.extend(globals.clone().map(TsComponent::from));
    module.components.extend(llr.public_components.iter().map(|llr_compo| {
        let mut ts_compo = TsComponent::from(llr_compo);
        ts_compo.aliases = aliases_of(&llr_compo.name);
        ts_compo
    }));

    file.declarations.extend(module.structs_and_enums.iter().map(TsStructOrEnum::declaration));

    for global in &module.globals {
        global.generate(ComponentType::Global, &mut file);
    }

    for public_component in &module.components {
        public_component
            .generate(ComponentType::Component { associated_globals: &module.globals }, &mut file);
    }

    for struct_or_enum in &module.structs_and_enums {
        struct_or_enum.generate_aliases(&mut file);
    }

    // Declare runtime values so TypeScript allows `new MainWindow()` etc.
    {
        for compo in &module.components {
            file.trailing_code.push(format_smolstr!(
                "export declare const {name}: {{ new(properties?: Partial<{name}>): {name} & slint.ComponentHandle }};",
                name = compo.name
            ));
        }
        for se in &module.structs_and_enums {
            if let TsStructOrEnum::Struct(s) = se {
                file.trailing_code.push(format_smolstr!(
                    "export declare function {name}(properties?: Partial<{name}>): {name};",
                    name = s.name
                ));
            }
        }
    }

    Ok(file)
}

fn ts_type_name(ty: &Type) -> SmolStr {
    match ty {
        Type::Invalid => panic!("Invalid type encountered in llr output"),
        Type::Void => SmolStr::new_static("void"),
        Type::String => SmolStr::new_static("string"),
        // Reading gives a brush; the other forms are what a write accepts.
        Type::Color => SmolStr::new_static("slint.Brush | slint.RgbaColor | string"),
        Type::Int32 => SmolStr::new_static("number"),
        Type::Float32
        | Type::Duration
        | Type::Angle
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Percent
        | Type::Rem
        | Type::UnitProduct(_) => SmolStr::new_static("number"),
        Type::Image => SmolStr::new_static("slint.ImageData"),
        Type::Bool => SmolStr::new_static("boolean"),
        Type::Brush => SmolStr::new_static("slint.Brush | string"),
        Type::StyledText => SmolStr::new_static("slint.StyledText"),
        Type::Array(elem_type) => format_smolstr!("slint.Model<{}>", ts_type_name(elem_type)),
        Type::Struct(s) => match &s.name {
            StructName::User { name, .. } => ident(name),
            StructName::Builtin(builtin_struct) if !builtin_struct.is_public() => {
                SmolStr::new_static("void")
            }
            StructName::Builtin(_) | StructName::None => {
                let fields = s
                    .fields
                    .iter()
                    .map(|(name, ty)| format!("{}: {}", member(name), ts_type_name(ty)))
                    .collect::<Vec<_>>();
                format_smolstr!("{{ {} }}", fields.join("; "))
            }
        },
        // An enum declared in the .slint file is generated here. A built-in one comes from
        // `slint.language` when it is public, and is otherwise unreachable, like in Rust.
        Type::Enumeration(enumeration) if enumeration.node.is_some() => ident(&enumeration.name),
        Type::Enumeration(enumeration) if is_public_builtin_enum(&enumeration.name) => {
            format_smolstr!("slint.language.{}", enumeration.name)
        }
        Type::Enumeration(_) => SmolStr::new_static("void"),
        Type::Callback(function) | Type::Function(function) => {
            let args = function
                .args
                .iter()
                .enumerate()
                .map(|(i, ty)| format!("arg_{}: {}", i, ts_type_name(ty)))
                .collect::<Vec<_>>();
            format_smolstr!("({}) => {}", args.join(", "), ts_type_name(&function.return_type))
        }
        Type::Keys => SmolStr::new_static("slint.Keys"),
        Type::DataTransfer => SmolStr::new_static("slint.DataTransfer"),
        Type::ComponentFactory => SmolStr::new_static("any"),
        // The Node.js bindings have no JavaScript representation for a mouse cursor value.
        Type::MouseCursor => SmolStr::new_static("void"),
        ty => unimplemented!("unimplemented type conversion {:#?}", ty),
    }
}
