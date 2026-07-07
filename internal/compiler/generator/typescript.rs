// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore subcomponent structty enumty

use std::collections::HashMap;
use std::rc::Rc;

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

pub fn ident(ident: &str) -> SmolStr {
    let normalized =
        if ident.contains('-') { ident.replace_smolstr("-", "_") } else { SmolStr::from(ident) };
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

impl From<&llr::PublicProperty> for TsProperty {
    fn from(llr_prop: &llr::PublicProperty) -> Self {
        Self {
            name: ident(&llr_prop.name),
            ty: ts_type_name(&llr_prop.ty),
            read_only: llr_prop.read_only,
        }
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
                .map(|glob| typescript_ast::Field {
                    name: glob.name.clone(),
                    ty: glob.name.clone(),
                    read_only: true,
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

impl TryFrom<&Rc<crate::langtype::Struct>> for TsStruct {
    type Error = AnonymousStruct;

    fn try_from(structty: &Rc<crate::langtype::Struct>) -> Result<Self, Self::Error> {
        let StructName::User { name, .. } = &structty.name else {
            return Err(AnonymousStruct);
        };
        Ok(Self {
            name: ident(name),
            fields: structty
                .fields
                .iter()
                .map(|(name, ty)| TsStructField { name: ident(name), ty: ts_type_name(ty) })
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

impl From<&Rc<crate::langtype::Enumeration>> for TsEnum {
    fn from(enumty: &Rc<crate::langtype::Enumeration>) -> Self {
        Self {
            name: ident(&enumty.name),
            variants: enumty
                .values
                .iter()
                .map(|val| typescript_ast::EnumVariant { name: ident(val), value: val.clone() })
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

impl From<&TsStructOrEnum> for typescript_ast::Declaration {
    fn from(struct_or_enum: &TsStructOrEnum) -> Self {
        match struct_or_enum {
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
            writeln!(f, "export enum {} {{", self.name)?;
            for variant in &self.variants {
                writeln!(f, "    {},", variant)?;
            }
            writeln!(f, "}}")?;
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    pub struct EnumVariant {
        pub name: SmolStr,
        pub value: SmolStr,
    }

    impl Display for EnumVariant {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "{} = \"{}\"", self.name, self.value)
        }
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
use itertools::Either;
use typescript_ast::*;

/// Returns the text of the TypeScript code produced by the given root component
pub fn generate(
    doc: &Document,
    compiler_config: &CompilerConfiguration,
    destination_path: Option<&std::path::Path>,
) -> std::io::Result<File> {
    let mut file = File { ..Default::default() };
    file.imports.push(SmolStr::new_static("import * as slint from \"slint-ui\";"));

    let mut compo_aliases: HashMap<SmolStr, Vec<SmolStr>> = Default::default();
    let mut struct_aliases: HashMap<SmolStr, Vec<SmolStr>> = Default::default();
    let mut enum_aliases: HashMap<SmolStr, Vec<SmolStr>> = Default::default();

    for export in doc.exports.iter() {
        match &export.1 {
            Either::Left(component) if !component.is_global() && export.0.name != component.id => {
                compo_aliases.entry(component.id.clone()).or_default().push(export.0.name.clone());
            }
            Either::Right(ty) => match &ty {
                Type::Struct(s) if s.node().is_some() => {
                    if let StructName::User { name: orig_name, .. } = &s.name
                        && export.0.name != *orig_name
                    {
                        struct_aliases
                            .entry(orig_name.clone())
                            .or_default()
                            .push(export.0.name.clone());
                    }
                }
                Type::Enumeration(en) if export.0.name != en.name => {
                    enum_aliases.entry(en.name.clone()).or_default().push(export.0.name.clone());
                }
                _ => {}
            },
            _ => {}
        }
    }

    let mut module =
        TsModule { globals: Vec::new(), components: Vec::new(), structs_and_enums: Vec::new() };

    for ty in &doc.used_types.borrow().structs_and_enums {
        match ty {
            Type::Struct(s) => module.structs_and_enums.extend(
                TsStruct::try_from(s).ok().and_then(|mut ts_struct| {
                    let StructName::User { name, .. } = &s.name else {
                        return None;
                    };
                    ts_struct.aliases = struct_aliases.remove(name).unwrap_or_default();
                    Some(TsStructOrEnum::Struct(ts_struct))
                }),
            ),
            Type::Enumeration(en) => {
                module.structs_and_enums.push({
                    let mut ts_enum = TsEnum::from(en);
                    ts_enum.aliases = enum_aliases.remove(&en.name).unwrap_or_default();
                    TsStructOrEnum::Enum(ts_enum)
                });
            }
            _ => {}
        }
    }

    let llr = llr::lower_to_item_tree::lower_to_item_tree(doc, compiler_config);

    let globals = llr.globals.iter().filter(|glob| glob.exported && glob.must_generate());

    module.globals.extend(globals.clone().map(TsComponent::from));
    module.components.extend(llr.public_components.iter().map(|llr_compo| {
        let mut ts_compo = TsComponent::from(llr_compo);
        ts_compo.aliases = compo_aliases.remove(&llr_compo.name).unwrap_or_default();
        ts_compo
    }));

    // Collect built-in enums referenced by public properties but not already
    // in used_types (which only contains user-defined types).
    let mut seen_enums: HashSet<SmolStr> = module
        .structs_and_enums
        .iter()
        .filter_map(|se| match se {
            TsStructOrEnum::Enum(e) => Some(e.name.clone()),
            _ => None,
        })
        .collect();

    let all_properties = llr
        .public_components
        .iter()
        .flat_map(|c| c.public_properties.iter())
        .chain(globals.clone().flat_map(|g| g.public_properties.iter()));

    for prop in all_properties {
        collect_builtin_enums(&prop.ty, &mut seen_enums, &mut module.structs_and_enums);
    }

    file.declarations.extend(module.structs_and_enums.iter().map(From::from));

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

    let is_dts = destination_path
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".d.ts"));

    if is_dts {
        // Declare runtime values so TypeScript allows `new MainWindow()` etc.
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

    let relative_path = if is_dts {
        None
    } else {
        let main_file = std::path::absolute(
            doc.node
                .as_ref()
                .ok_or_else(|| std::io::Error::other("Cannot determine path of the main file"))?
                .source_file
                .path(),
        )
        .unwrap();

        let destination_dir = destination_path.and_then(|p| {
            std::path::absolute(p).ok().and_then(|p| p.parent().map(std::path::PathBuf::from))
        });

        destination_dir
            .and_then(|dir| pathdiff::diff_paths(main_file.parent().unwrap(), dir))
            .map(|rel| rel.join(main_file.file_name().unwrap()).to_string_lossy().into_owned())
    };

    if let Some(slint_file_relative) = relative_path {
        let slint_file_relative = slint_file_relative.replace('\\', "/");
        file.trailing_code.push(format_smolstr!(
            "const _module: any = slint.loadFile(new URL(\"./{}\", import.meta.url));",
            slint_file_relative
        ));

        for compo in &module.components {
            file.trailing_code.push(format_smolstr!(
                "export const {name}: {{ new(properties?: Partial<{name}>): {name} & slint.ComponentHandle }} = _module.{name};",
                name = compo.name
            ));
        }

        for se in &module.structs_and_enums {
            if let TsStructOrEnum::Struct(s) = se {
                file.trailing_code.push(format_smolstr!(
                    "export const {name}: (properties?: Partial<{name}>) => {name} = _module.{name};",
                    name = s.name
                ));
            }
        }
    }

    Ok(file)
}

/// Recursively find built-in enums in a type and add them to the output list.
fn collect_builtin_enums(ty: &Type, seen: &mut HashSet<SmolStr>, out: &mut Vec<TsStructOrEnum>) {
    match ty {
        Type::Enumeration(en) if en.node.is_none() => {
            let name = ident(&en.name);
            if seen.insert(name) {
                out.push(TsStructOrEnum::Enum(TsEnum::from(en)));
            }
        }
        Type::Array(elem) => collect_builtin_enums(elem, seen, out),
        Type::Struct(s) => {
            for field_ty in s.fields.values() {
                collect_builtin_enums(field_ty, seen, out);
            }
        }
        Type::Callback(f) | Type::Function(f) => {
            for arg in &f.args {
                collect_builtin_enums(arg, seen, out);
            }
            collect_builtin_enums(&f.return_type, seen, out);
        }
        _ => {}
    }
}

fn ts_type_name(ty: &Type) -> SmolStr {
    match ty {
        Type::Invalid => panic!("Invalid type encountered in llr output"),
        Type::Void => SmolStr::new_static("void"),
        Type::String => SmolStr::new_static("string"),
        Type::Color => SmolStr::new_static("slint.RgbaColor"),
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
        Type::Brush => SmolStr::new_static("slint.Brush"),
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
                    .map(|(name, ty)| format!("{}: {}", ident(name), ts_type_name(ty)))
                    .collect::<Vec<_>>();
                format_smolstr!("{{ {} }}", fields.join("; "))
            }
        },
        Type::Enumeration(enumeration) => ident(&enumeration.name),
        Type::Callback(function) | Type::Function(function) => {
            let args = function
                .args
                .iter()
                .enumerate()
                .map(|(i, ty)| format!("arg_{}: {}", i, ts_type_name(ty)))
                .collect::<Vec<_>>();
            format_smolstr!("({}) => {}", args.join(", "), ts_type_name(&function.return_type))
        }
        Type::Keys => SmolStr::new_static("string"),
        Type::DataTransfer => SmolStr::new_static("slint.DataTransfer"),
        Type::ComponentFactory => SmolStr::new_static("any"),
        ty => unimplemented!("unimplemented type conversion {:#?}", ty),
    }
}
