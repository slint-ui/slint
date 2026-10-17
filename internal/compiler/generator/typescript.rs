// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore subcomponent structty enumty

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use itertools::Itertools;
use smol_str::{SmolStr, StrExt, format_smolstr};

use crate::CompilerConfiguration;
use crate::langtype::{BuiltinStruct, StructName, Type};
use crate::llr;
use crate::object_tree::Document;
use typescript_ast::*;

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

/// The name of a member: a property, a struct field, or an enum variant.
/// A keyword is a valid member name, so only the dashes need replacing.
fn member(name: &str) -> SmolStr {
    if name.contains('-') { name.replace_smolstr("-", "_") } else { SmolStr::from(name) }
}

/// The name of a declaration: a type or a binding. Those can't be keywords.
fn ident(ident: &str) -> SmolStr {
    let normalized = member(ident);
    if is_typescript_keyword(normalized.as_str()) {
        format_smolstr!("{}_", normalized)
    } else {
        normalized
    }
}

fn type_aliases<'a>(
    name: &'a SmolStr,
    aliases: &'a [SmolStr],
) -> impl Iterator<Item = Declaration> + 'a {
    aliases
        .iter()
        .map(|alias| Declaration::TypeAlias(TypeAlias { name: ident(alias), value: name.clone() }))
}

/// A component or a global, and the extra names the module exports it under.
struct TsInterface {
    name: SmolStr,
    aliases: Vec<SmolStr>,
    fields: Vec<Field>,
}

impl TsInterface {
    fn new(name: &SmolStr, aliases: Vec<SmolStr>, properties: &llr::PublicProperties) -> Self {
        let fields = properties
            .iter()
            .map(|(name, prop)| match (ts_read_type_name(&prop.ty), prop.read_only()) {
                (Some(read_ty), true) => {
                    Field { name: member(name), ty: read_ty, read_ty: None, read_only: true }
                }
                (read_ty, read_only) => {
                    Field { name: member(name), ty: ts_type_name(&prop.ty), read_ty, read_only }
                }
            })
            .collect();
        Self { name: ident(name), aliases, fields }
    }

    /// Every name the `.slint` file exports this under.
    fn exported_names(&self) -> impl Iterator<Item = &SmolStr> {
        std::iter::once(&self.name).chain(&self.aliases)
    }

    /// The interface itself, then one `export type Alias = Name;` per extra exported name.
    fn declarations(&self, associated_globals: &[TsInterface]) -> Vec<Declaration> {
        let globals: Vec<Field> = associated_globals
            .iter()
            .flat_map(|glob| {
                glob.exported_names().map(|exported| Field {
                    name: member(exported),
                    ty: glob.name.clone(),
                    read_ty: None,
                    read_only: true,
                })
            })
            .collect();
        let unhidden_globals = globals
            .iter()
            .filter(|global| self.fields.iter().all(|field| field.name != global.name))
            .cloned();
        let globals_symbol = (!globals.is_empty()).then(|| Field {
            name: "[slint.globalsSymbol]".into(),
            ty: format_smolstr!("{{ {}; }}", globals.iter().join("; ")),
            read_ty: None,
            read_only: true,
        });
        let interface = Interface {
            name: self.name.clone(),
            fields: self
                .fields
                .iter()
                .cloned()
                .chain(unhidden_globals)
                .chain(globals_symbol)
                .collect(),
        };
        std::iter::once(Declaration::Interface(interface))
            .chain(type_aliases(&self.name, &self.aliases))
            .collect()
    }
}

/// A struct or an enum declared in the `.slint` file, and the names it is exported under.
struct TsType {
    name: SmolStr,
    aliases: Vec<SmolStr>,
    kind: TsTypeKind,
}

enum TsTypeKind {
    Struct(Vec<Field>),
    Enum(Vec<EnumVariant>),
}

impl TsType {
    fn declarations(&self) -> impl Iterator<Item = Declaration> + '_ {
        let declaration = match &self.kind {
            TsTypeKind::Struct(fields) => Declaration::Interface(Interface {
                name: self.name.clone(),
                fields: fields.clone(),
            }),
            TsTypeKind::Enum(variants) => {
                Declaration::Enum(Enum { name: self.name.clone(), variants: variants.clone() })
            }
        };
        std::iter::once(declaration).chain(type_aliases(&self.name, &self.aliases))
    }
}

/// This module contains data structures that represent a TypeScript file.
/// It is rendered into TypeScript code using the Display trait.
mod typescript_ast {
    use std::fmt::{Display, Error, Formatter};

    use itertools::Itertools;
    use smol_str::SmolStr;

    #[derive(Default, Debug)]
    pub struct File {
        pub imports: Vec<SmolStr>,
        pub declarations: Vec<Declaration>,
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
                if decl.followed_by_blank_line() {
                    writeln!(f)?;
                }
            }
            Ok(())
        }
    }

    #[derive(Debug, derive_more::Display)]
    pub enum Declaration {
        Interface(Interface),
        Enum(Enum),
        TypeAlias(TypeAlias),
        DeclaredConst(DeclaredConst),
        DeclaredFunction(DeclaredFunction),
    }

    impl Declaration {
        /// Types are set apart from one another; the runtime values at the end of the file
        /// read as one list.
        fn followed_by_blank_line(&self) -> bool {
            !matches!(self, Declaration::DeclaredConst(_) | Declaration::DeclaredFunction(_))
        }
    }

    #[derive(Debug, Default)]
    pub struct Interface {
        pub name: SmolStr,
        pub fields: Vec<Field>,
    }

    impl Display for Interface {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            writeln!(f, "export interface {} {{", self.name)?;
            for field in &self.fields {
                for line in field.to_string().lines() {
                    writeln!(f, "    {line};")?;
                }
            }
            write!(f, "}}")
        }
    }

    #[derive(Debug, Clone)]
    pub struct Field {
        pub name: SmolStr,
        /// The type that a write accepts.
        pub ty: SmolStr,
        /// The type that a read returns, if it isn't `ty`.
        pub read_ty: Option<SmolStr>,
        pub read_only: bool,
    }

    impl Display for Field {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match (&self.read_ty, self.read_only) {
                (Some(read_ty), _) => write!(
                    f,
                    "get {name}(): {read_ty}\nset {name}(value: {ty})",
                    name = self.name,
                    ty = self.ty
                ),
                (None, true) => write!(f, "readonly {}: {}", self.name, self.ty),
                (None, false) => write!(f, "{}: {}", self.name, self.ty),
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
            // enum: a plain string stays assignable, and `enum` is not erasable syntax,
            // which node's type stripping rejects.
            let union =
                self.variants.iter().map(|variant| format!("\"{}\"", variant.value)).join(" | ");
            writeln!(
                f,
                "export type {} = {};",
                self.name,
                if union.is_empty() { "never" } else { &union }
            )?;

            writeln!(f, "export declare const {}: {{", self.name)?;
            for variant in &self.variants {
                writeln!(f, "    readonly {}: \"{}\";", variant.name, variant.value)?;
            }
            write!(f, "}};")
        }
    }

    #[derive(Debug, Clone)]
    pub struct EnumVariant {
        pub name: SmolStr,
        pub value: SmolStr,
    }

    #[derive(Debug, derive_more::Display)]
    #[display("export type {name} = {value};")]
    pub struct TypeAlias {
        pub name: SmolStr,
        pub value: SmolStr,
    }

    #[derive(Debug, derive_more::Display)]
    #[display("export declare const {name}: {ty};")]
    pub struct DeclaredConst {
        pub name: SmolStr,
        pub ty: SmolStr,
    }

    #[derive(Debug, derive_more::Display)]
    #[display("export declare function {name}({parameters}): {return_type};")]
    pub struct DeclaredFunction {
        pub name: SmolStr,
        pub parameters: SmolStr,
        pub return_type: SmolStr,
    }
}

/// Returns the text of the TypeScript code produced by the given root component
pub fn generate(doc: &Document, compiler_config: &CompilerConfiguration) -> std::io::Result<File> {
    let mut file = File::default();
    file.imports.push(SmolStr::new_static("import * as slint from \"slint-ui\";"));

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

    let types: Vec<TsType> = doc
        .used_types
        .borrow()
        .structs_and_enums
        .iter()
        .filter_map(|ty| match ty {
            Type::Struct(s) => {
                let StructName::User { name, .. } = &s.name else { return None };
                let fields = s
                    .fields
                    .iter()
                    .map(|(name, ty)| Field {
                        name: member(name),
                        ty: ts_type_name(ty),
                        read_ty: None,
                        read_only: false,
                    })
                    .collect();
                Some(TsType {
                    name: ident(name),
                    aliases: aliases_of(name),
                    kind: TsTypeKind::Struct(fields),
                })
            }
            // A built-in enum is not declared here: it is `slint.language.X` or nothing.
            Type::Enumeration(en) if en.node.is_some() => {
                let variants = en
                    .values
                    .iter()
                    .map(|value| EnumVariant { name: member(value), value: value.clone() })
                    .collect();
                Some(TsType {
                    name: ident(&en.name),
                    aliases: aliases_of(&en.name),
                    kind: TsTypeKind::Enum(variants),
                })
            }
            _ => None,
        })
        .collect();

    let globals: Vec<TsInterface> = llr
        .globals
        .iter()
        .filter(|glob| glob.exported && glob.must_generate())
        .map(|glob| {
            let aliases = glob.aliases.iter().map(|name| ident(name)).collect();
            TsInterface::new(&glob.name, aliases, &glob.public_properties)
        })
        .collect();

    let components: Vec<TsInterface> = llr
        .public_components
        .iter()
        .map(|compo| {
            TsInterface::new(&compo.name, aliases_of(&compo.name), &compo.public_properties)
        })
        .collect();

    file.declarations.extend(types.iter().flat_map(TsType::declarations));
    file.declarations.extend(globals.iter().flat_map(|glob| glob.declarations(&[])));
    file.declarations.extend(components.iter().flat_map(|compo| compo.declarations(&globals)));

    // Declare the runtime values so TypeScript allows `new MainWindow()` and `Item({ … })`.
    file.declarations.extend(components.iter().map(|compo| {
        Declaration::DeclaredConst(DeclaredConst {
            name: compo.name.clone(),
            ty: format_smolstr!(
                "{{ new(properties?: {properties}): {name} & Omit<slint.ComponentHandle, keyof {name}> }}",
                name = compo.name,
                properties = constructor_properties_type(compo),
            ),
        })
    }));
    file.declarations.extend(
        types.iter().filter(|ty| matches!(ty.kind, TsTypeKind::Struct(_))).map(|ty| {
            Declaration::DeclaredFunction(DeclaredFunction {
                name: ty.name.clone(),
                parameters: format_smolstr!("properties?: Partial<{}>", ty.name),
                return_type: ty.name.clone(),
            })
        }),
    );

    Ok(file)
}

/// The type that reading a property of type `ty` returns, if it isn't the one that a write accepts.
fn ts_read_type_name(ty: &Type) -> Option<SmolStr> {
    matches!(ty, Type::Color | Type::Brush).then(|| SmolStr::new_static("Required<slint.Brush>"))
}

/// The type of the properties that the constructor of `compo` takes.
fn constructor_properties_type(compo: &TsInterface) -> SmolStr {
    let differing: Vec<&Field> =
        compo.fields.iter().filter(|field| field.read_ty.is_some()).collect();
    if differing.is_empty() {
        return format_smolstr!("Partial<{}>", compo.name);
    }
    format_smolstr!(
        "Partial<Omit<{name}, {omitted}> & {{ {written} }}>",
        name = compo.name,
        omitted = differing.iter().map(|field| format!("\"{}\"", field.name)).join(" | "),
        written = differing
            .iter()
            .map(|field| format!("{};", Field { read_ty: None, ..(*field).clone() }))
            .join(" "),
    )
}

fn ts_type_name(ty: &Type) -> SmolStr {
    match ty {
        Type::Invalid => panic!("Invalid type encountered in llr output"),
        Type::Void => SmolStr::new_static("void"),
        Type::String => SmolStr::new_static("string"),
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
            // `is_public` also covers two structs that are not in `for_each_builtin_structs`,
            // so they are not under `slint.language`; the Node API spells them itself.
            StructName::Builtin(BuiltinStruct::LogicalPosition) => {
                SmolStr::new_static("slint.Point")
            }
            StructName::Builtin(BuiltinStruct::LogicalSize) => SmolStr::new_static("slint.Size"),
            StructName::Builtin(
                BuiltinStruct::ColorRgba | BuiltinStruct::ColorHsva | BuiltinStruct::ColorOklch,
            ) => {
                let fields = s
                    .fields
                    .iter()
                    .map(|(name, ty)| format!("{}: {}", member(name), ts_type_name(ty)))
                    .join("; ");
                format_smolstr!("{{ {} }}", fields)
            }
            StructName::Builtin(builtin_struct) if builtin_struct.is_public() => {
                let name: &'static str = builtin_struct.into();
                format_smolstr!("slint.language.{}", name)
            }
            StructName::Builtin(_) => SmolStr::new_static("void"),
            StructName::None => {
                let fields = s
                    .fields
                    .iter()
                    .map(|(name, ty)| format!("{}: {}", member(name), ts_type_name(ty)))
                    .join("; ");
                format_smolstr!("{{ {} }}", fields)
            }
        },
        // An enum declared in the .slint file is generated here. A built-in one comes from
        // `slint.language` when it is public, and is otherwise unreachable, like in Rust.
        Type::Enumeration(en) if en.node.is_some() => ident(&en.name),
        Type::Enumeration(en) if en.public => format_smolstr!("slint.language.{}", en.name),
        Type::Enumeration(_) => SmolStr::new_static("void"),
        Type::Callback(function) | Type::Function(function) => {
            // Distinct `.slint` names can become the same one, such as `default` and `default_`.
            let mut used = HashSet::new();
            let args = function
                .args
                .iter()
                .enumerate()
                .map(|(i, ty)| {
                    let mut name = match function.arg_names.get(i) {
                        Some(name) if !name.is_empty() => ident(name),
                        _ => format_smolstr!("arg_{i}"),
                    };
                    if used.contains(&name) {
                        name = format_smolstr!("arg_{i}");
                    }
                    while !used.insert(name.clone()) {
                        name = format_smolstr!("{name}_");
                    }
                    format!("{name}: {}", ts_type_name(ty))
                })
                .join(", ");
            format_smolstr!("({}) => {}", args, ts_type_name(&function.return_type))
        }
        Type::Keys => SmolStr::new_static("slint.Keys"),
        Type::DataTransfer => SmolStr::new_static("slint.DataTransfer"),
        Type::ComponentFactory => SmolStr::new_static("any"),
        // The Node.js bindings have no JavaScript representation for a mouse cursor value.
        Type::MouseCursor => SmolStr::new_static("void"),
        ty => unimplemented!("unimplemented type conversion {:#?}", ty),
    }
}
