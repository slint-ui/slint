// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*! module for the C++ code generator
*/

// cSpell:ignore cmath constexpr cstdlib decltype intptr itertools nullptr prepended struc subcomponent uintptr vals

use std::collections::HashMap;
use std::sync::OnceLock;
use std::{collections::HashSet, rc::Rc};

use smol_str::{SmolStr, StrExt, format_smolstr};

use serde::{Deserialize, Serialize};

mod diff;

// Check if word is one of Python keywords
// (https://docs.python.org/3/reference/lexical_analysis.html#keywords)
fn is_python_keyword(word: &str) -> bool {
    static PYTHON_KEYWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    let keywords = PYTHON_KEYWORDS.get_or_init(|| {
        let keywords: HashSet<&str> = HashSet::from([
            "False", "await", "else", "import", "pass", "None", "break", "except", "in", "raise",
            "True", "class", "finally", "is", "return", "and", "continue", "for", "lambda", "try",
            "as", "def", "from", "nonlocal", "while", "assert", "del", "global", "not", "with",
            "async", "elif", "if", "or", "yield",
        ]);
        keywords
    });
    keywords.contains(word)
}

pub fn ident(ident: &str) -> SmolStr {
    let mut new_ident = SmolStr::from(ident);
    if ident.contains('-') {
        new_ident = ident.replace_smolstr("-", "_");
    }
    if is_python_keyword(new_ident.as_str()) {
        new_ident = format_smolstr!("{}_", new_ident);
    }
    new_ident
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct PyProperty {
    name: SmolStr,
    ty: SmolStr,
}

impl From<&PyProperty> for python_ast::Field {
    fn from(prop: &PyProperty) -> Self {
        Field {
            name: prop.name.clone(),
            ty: Some(PyType { name: prop.ty.clone(), optional: false }),
            default_value: None,
        }
    }
}

impl From<&llr::PublicProperty> for PyProperty {
    fn from(llr_prop: &llr::PublicProperty) -> Self {
        Self { name: ident(&llr_prop.name), ty: python_type_name(&llr_prop.ty) }
    }
}

enum ComponentType<'a> {
    Global,
    Component { associated_globals: &'a [PyComponent] },
}

#[derive(Serialize, Deserialize)]
pub struct PyComponent {
    name: SmolStr,
    properties: Vec<PyProperty>,
    aliases: Vec<SmolStr>,
}

impl PyComponent {
    fn generate(&self, ty: ComponentType<'_>, file: &mut File) {
        let mut class = Class {
            name: self.name.clone(),
            super_class: if matches!(ty, ComponentType::Global) {
                None
            } else {
                Some(SmolStr::new_static("slint.Component"))
            },
            ..Default::default()
        };

        class.fields = self
            .properties
            .iter()
            .map(From::from)
            .chain(
                match ty {
                    ComponentType::Global => None,
                    ComponentType::Component { associated_globals } => Some(associated_globals),
                }
                .into_iter()
                .flat_map(|globals| globals.iter())
                .map(|glob| Field {
                    name: glob.name.clone(),
                    ty: Some(PyType { name: glob.name.clone(), optional: false }),
                    default_value: None,
                }),
            )
            .collect();

        file.declarations.push(python_ast::Declaration::Class(class));

        file.declarations.extend(self.aliases.iter().map(|exported_name| {
            python_ast::Declaration::Variable(Variable {
                name: ident(&exported_name),
                value: self.name.clone(),
            })
        }))
    }
}

impl From<&llr::PublicComponent> for PyComponent {
    fn from(llr_compo: &llr::PublicComponent) -> Self {
        Self {
            name: ident(&llr_compo.name),
            properties: llr_compo.public_properties.iter().map(From::from).collect(),
            aliases: Vec::new(),
        }
    }
}

impl From<&llr::GlobalComponent> for PyComponent {
    fn from(llr_global: &llr::GlobalComponent) -> Self {
        Self {
            name: ident(&llr_global.name),
            properties: llr_global.public_properties.iter().map(From::from).collect(),
            aliases: llr_global.aliases.iter().map(|exported_name| ident(&exported_name)).collect(),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct PyStructField {
    name: SmolStr,
    ty: SmolStr,
}

#[derive(Serialize, Deserialize)]
pub struct PyStruct {
    name: SmolStr,
    fields: Vec<PyStructField>,
    aliases: Vec<SmolStr>,
}

pub struct AnonymousStruct;

impl TryFrom<&Rc<crate::langtype::Struct>> for PyStruct {
    type Error = AnonymousStruct;

    fn try_from(structty: &Rc<crate::langtype::Struct>) -> Result<Self, Self::Error> {
        let StructName::User { name, .. } = &structty.name else {
            return Err(AnonymousStruct);
        };
        Ok(Self {
            name: ident(&name),
            fields: structty
                .fields
                .iter()
                .map(|(name, ty)| PyStructField { name: ident(&name), ty: python_type_name(ty) })
                .collect(),
            aliases: Vec::new(),
        })
    }
}

impl From<&PyStruct> for python_ast::Declaration {
    fn from(py_struct: &PyStruct) -> Self {
        let py_fields = py_struct
            .fields
            .iter()
            .map(|field| Field {
                name: field.name.clone(),
                ty: Some(PyType { name: field.ty.clone(), optional: false }),
                default_value: None,
            })
            .collect::<Vec<_>>();

        let ctor = FunctionDeclaration {
            name: SmolStr::new_static("__init__"),
            positional_parameters: Vec::default(),
            keyword_parameters: py_fields
                .iter()
                .cloned()
                .map(|field| {
                    let mut kw_field = field.clone();
                    kw_field.ty.as_mut().unwrap().optional = true;
                    kw_field.default_value = Some(SmolStr::new_static("None"));
                    kw_field
                })
                .collect(),
            return_type: None,
        };

        let struct_class = Class {
            name: py_struct.name.clone(),
            fields: py_fields,
            function_declarations: vec![ctor],
            ..Default::default()
        };
        python_ast::Declaration::Class(struct_class)
    }
}

impl PyStruct {
    fn generate_aliases(&self) -> impl ExactSizeIterator<Item = python_ast::Declaration> + use<'_> {
        self.aliases.iter().map(|alias| {
            python_ast::Declaration::Variable(Variable {
                name: alias.clone(),
                value: self.name.clone(),
            })
        })
    }
}

#[derive(Serialize, Deserialize)]
pub struct PyEnumVariant {
    name: SmolStr,
    strvalue: SmolStr,
}

#[derive(Serialize, Deserialize)]
pub struct PyEnum {
    name: SmolStr,
    variants: Vec<PyEnumVariant>,
    aliases: Vec<SmolStr>,
}

impl From<&Rc<crate::langtype::Enumeration>> for PyEnum {
    fn from(enumty: &Rc<crate::langtype::Enumeration>) -> Self {
        Self {
            name: ident(&enumty.name),
            variants: enumty
                .values
                .iter()
                .map(|val| PyEnumVariant { name: ident(&val), strvalue: val.clone() })
                .collect(),
            aliases: Vec::new(),
        }
    }
}

impl From<&PyEnum> for python_ast::Declaration {
    fn from(py_enum: &PyEnum) -> Self {
        python_ast::Declaration::Class(Class {
            name: py_enum.name.clone(),
            super_class: Some(SmolStr::new_static("enum.StrEnum")),
            fields: py_enum
                .variants
                .iter()
                .map(|variant| Field {
                    name: variant.name.clone(),
                    ty: None,
                    default_value: Some(format_smolstr!("\"{}\"", variant.strvalue)),
                })
                .collect(),
            function_declarations: Vec::new(),
        })
    }
}

impl PyEnum {
    fn generate_aliases(&self) -> impl ExactSizeIterator<Item = python_ast::Declaration> + use<'_> {
        self.aliases.iter().map(|alias| {
            python_ast::Declaration::Variable(Variable {
                name: alias.clone(),
                value: self.name.clone(),
            })
        })
    }
}

#[derive(Serialize, Deserialize)]
pub enum PyStructOrEnum {
    Struct(PyStruct),
    Enum(PyEnum),
}

impl From<&PyStructOrEnum> for python_ast::Declaration {
    fn from(struct_or_enum: &PyStructOrEnum) -> Self {
        match struct_or_enum {
            PyStructOrEnum::Struct(py_struct) => py_struct.into(),
            PyStructOrEnum::Enum(py_enum) => py_enum.into(),
        }
    }
}

impl PyStructOrEnum {
    fn generate_aliases(&self, file: &mut File) {
        match self {
            PyStructOrEnum::Struct(py_struct) => {
                file.declarations.extend(py_struct.generate_aliases())
            }
            PyStructOrEnum::Enum(py_enum) => file.declarations.extend(py_enum.generate_aliases()),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PyModule {
    version: SmolStr,
    globals: Vec<PyComponent>,
    components: Vec<PyComponent>,
    structs_and_enums: Vec<PyStructOrEnum>,
}

impl Default for PyModule {
    fn default() -> Self {
        Self {
            version: SmolStr::new_static("1.0"),
            globals: Default::default(),
            components: Default::default(),
            structs_and_enums: Default::default(),
        }
    }
}

impl PyModule {
    pub fn load_from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("{}", e))
    }
}

pub fn generate_py_module(
    doc: &Document,
    compiler_config: &CompilerConfiguration,
) -> std::io::Result<PyModule> {
    let mut module = PyModule::default();

    let mut compo_aliases: HashMap<SmolStr, Vec<SmolStr>> = Default::default();
    let mut struct_aliases: HashMap<SmolStr, Vec<SmolStr>> = Default::default();
    let mut enum_aliases: HashMap<SmolStr, Vec<SmolStr>> = Default::default();

    for export in doc.exports.iter() {
        match &export.1 {
            Either::Left(component) if !component.is_global() => {
                if export.0.name != component.id {
                    compo_aliases
                        .entry(component.id.clone())
                        .or_default()
                        .push(export.0.name.clone());
                }
            }
            Either::Right(ty) => match &ty {
                Type::Struct(s) if s.node().is_some() => {
                    if let StructName::User { name: orig_name, .. } = &s.name {
                        if export.0.name != *orig_name {
                            struct_aliases
                                .entry(orig_name.clone())
                                .or_default()
                                .push(export.0.name.clone());
                        }
                    }
                }
                Type::Enumeration(en) => {
                    if export.0.name != en.name {
                        enum_aliases
                            .entry(en.name.clone())
                            .or_default()
                            .push(export.0.name.clone());
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    for ty in &doc.used_types.borrow().structs_and_enums {
        match ty {
            Type::Struct(s) => module.structs_and_enums.extend(
                PyStruct::try_from(s).ok().and_then(|mut pystruct| {
                    let StructName::User { name, .. } = &s.name else {
                        return None;
                    };
                    pystruct.aliases = struct_aliases.remove(name).unwrap_or_default();
                    Some(PyStructOrEnum::Struct(pystruct))
                }),
            ),
            Type::Enumeration(en) => {
                module.structs_and_enums.push({
                    let mut pyenum = PyEnum::from(en);
                    pyenum.aliases = enum_aliases.remove(&en.name).unwrap_or_default();
                    PyStructOrEnum::Enum(pyenum)
                });
            }
            _ => {}
        }
    }

    let llr = llr::lower_to_item_tree::lower_to_item_tree(doc, compiler_config);

    let globals = llr.globals.iter().filter(|glob| glob.exported && glob.must_generate());

    module.globals.extend(globals.clone().map(PyComponent::from));
    module.components.extend(llr.public_components.iter().map(|llr_compo| {
        let mut pycompo = PyComponent::from(llr_compo);
        pycompo.aliases = compo_aliases.remove(&llr_compo.name).unwrap_or_default();
        pycompo
    }));

    Ok(module)
}

/// This module contains some data structures that helps represent a Python file.
/// It is then rendered into an actual Python code using the Display trait
mod python_ast {

    use std::fmt::{Display, Error, Formatter};

    use smol_str::SmolStr;

    ///A full Python file
    #[derive(Default, Debug)]
    pub struct File {
        pub imports: Vec<SmolStr>,
        pub declarations: Vec<Declaration>,
        pub trailing_code: Vec<SmolStr>,
    }

    impl Display for File {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            writeln!(f, "# This file is auto-generated\n")?;
            for import in &self.imports {
                writeln!(f, "import {}", import)?;
            }
            writeln!(f)?;
            for decl in &self.declarations {
                writeln!(f, "{}", decl)?;
            }
            for code in &self.trailing_code {
                writeln!(f, "{}", code)?;
            }
            Ok(())
        }
    }

    #[derive(Debug, derive_more::Display)]
    pub enum Declaration {
        Class(Class),
        Variable(Variable),
    }

    #[derive(Debug, Default)]
    pub struct Class {
        pub name: SmolStr,
        pub super_class: Option<SmolStr>,
        pub fields: Vec<Field>,
        pub function_declarations: Vec<FunctionDeclaration>,
    }

    impl Display for Class {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            if let Some(super_class) = self.super_class.as_ref() {
                writeln!(f, "class {}({}):", self.name, super_class)?;
            } else {
                writeln!(f, "class {}:", self.name)?;
            }
            if self.fields.is_empty() && self.function_declarations.is_empty() {
                writeln!(f, "    pass")?;
                return Ok(());
            }

            for field in &self.fields {
                writeln!(f, "    {}", field)?;
            }

            if !self.fields.is_empty() {
                writeln!(f)?;
            }

            for fundecl in &self.function_declarations {
                writeln!(f, "    {}", fundecl)?;
            }

            Ok(())
        }
    }

    #[derive(Debug)]
    pub struct Variable {
        pub name: SmolStr,
        pub value: SmolStr,
    }

    impl Display for Variable {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            writeln!(f, "{} = {}", self.name, self.value)
        }
    }

    #[derive(Debug, Clone)]
    pub struct PyType {
        pub name: SmolStr,
        pub optional: bool,
    }

    impl Display for PyType {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            if self.optional {
                write!(f, "typing.Optional[{}]", self.name)
            } else {
                write!(f, "{}", self.name)
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct Field {
        pub name: SmolStr,
        pub ty: Option<PyType>,
        pub default_value: Option<SmolStr>,
    }

    impl Display for Field {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.name)?;
            if let Some(ty) = &self.ty {
                write!(f, ": {}", ty)?;
            }
            if let Some(default_value) = &self.default_value {
                write!(f, " = {}", default_value)?
            }
            Ok(())
        }
    }

    #[derive(Debug)]
    pub struct FunctionDeclaration {
        pub name: SmolStr,
        pub positional_parameters: Vec<SmolStr>,
        pub keyword_parameters: Vec<Field>,
        pub return_type: Option<PyType>,
    }

    impl Display for FunctionDeclaration {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "def {}(self", self.name)?;

            if !self.positional_parameters.is_empty() {
                write!(f, ", {}", self.positional_parameters.join(","))?;
            }

            if !self.keyword_parameters.is_empty() {
                write!(f, ", *")?;
                write!(
                    f,
                    ", {}",
                    self.keyword_parameters
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )?;
            }
            writeln!(
                f,
                ") -> {}: ...",
                self.return_type.as_ref().map_or(std::borrow::Cow::Borrowed("None"), |ty| {
                    std::borrow::Cow::Owned(ty.to_string())
                })
            )?;
            Ok(())
        }
    }
}

use crate::langtype::{StructName, Type};

use crate::CompilerConfiguration;
use crate::llr;
use crate::object_tree::Document;
use itertools::{Either, Itertools};
use python_ast::*;

/// Returns the text of the Python code produced by the given root component
pub fn generate(
    doc: &Document,
    compiler_config: &CompilerConfiguration,
    destination_path: Option<&std::path::Path>,
) -> std::io::Result<File> {
    let mut file = File { ..Default::default() };
    file.imports.push(SmolStr::new_static("slint"));
    file.imports.push(SmolStr::new_static("typing"));

    let pymodule = generate_py_module(doc, compiler_config)?;

    if pymodule.structs_and_enums.iter().any(|se| matches!(se, PyStructOrEnum::Enum(_))) {
        file.imports.push(SmolStr::new_static("enum"));
    }

    file.declarations.extend(pymodule.structs_and_enums.iter().map(From::from));

    for global in &pymodule.globals {
        global.generate(ComponentType::Global, &mut file);
    }

    for public_component in &pymodule.components {
        public_component.generate(
            ComponentType::Component { associated_globals: &pymodule.globals },
            &mut file,
        );
    }

    for struct_or_enum in &pymodule.structs_and_enums {
        struct_or_enum.generate_aliases(&mut file);
    }

    let main_file = std::path::absolute(
        doc.node
            .as_ref()
            .ok_or_else(|| std::io::Error::other("Cannot determine path of the main file"))?
            .source_file
            .path(),
    )
    .unwrap();

    let destination_path = destination_path.and_then(|maybe_relative_destination_path| {
        std::fs::canonicalize(maybe_relative_destination_path)
            .ok()
            .and_then(|p| p.parent().map(std::path::PathBuf::from))
    });

    let relative_path_from_destination_to_main_file =
        destination_path.and_then(|destination_path| {
            pathdiff::diff_paths(main_file.parent().unwrap(), destination_path)
        });

    if let Some(relative_path_from_destination_to_main_file) =
        relative_path_from_destination_to_main_file
    {
        use base64::engine::Engine;
        use std::io::Write;

        let mut api_str_compressor =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        api_str_compressor.write_all(serde_json::to_string(&pymodule).unwrap().as_bytes())?;
        let compressed_api_str = api_str_compressor.finish()?;
        let base64_api_str = base64::engine::general_purpose::STANDARD.encode(&compressed_api_str);

        file.imports.push(SmolStr::new_static("os"));
        file.trailing_code.push(format_smolstr!(
            "globals().update(vars(slint._load_file_checked(path=os.path.join(os.path.dirname(__file__), r'{}'), expected_api_base64_compressed=r'{}', generated_file=__file__)))",
            relative_path_from_destination_to_main_file.join(main_file.file_name().unwrap()).to_string_lossy(),
            base64_api_str
        ));
    }

    Ok(file)
}

fn python_type_name(ty: &Type) -> SmolStr {
    match ty {
        Type::Invalid => panic!("Invalid type encountered in llr output"),
        Type::Void => SmolStr::new_static("None"),
        Type::String => SmolStr::new_static("str"),
        Type::Color => SmolStr::new_static("slint.Color"),
        Type::Float32
        | Type::Int32
        | Type::Duration
        | Type::Angle
        | Type::PhysicalLength
        | Type::LogicalLength
        | Type::Percent
        | Type::Rem
        | Type::UnitProduct(_) => SmolStr::new_static("float"),
        Type::Image => SmolStr::new_static("slint.Image"),
        Type::Bool => SmolStr::new_static("bool"),
        Type::Brush => SmolStr::new_static("slint.Brush"),
        Type::Array(elem_type) => format_smolstr!("slint.Model[{}]", python_type_name(elem_type)),
        Type::Struct(s) => match &s.name {
            StructName::User { name, .. } => ident(name),
            StructName::BuiltinPrivate(_) => SmolStr::new_static("None"),
            StructName::BuiltinPublic(_) | StructName::None => {
                let tuple_types =
                    s.fields.values().map(|ty| python_type_name(ty)).collect::<Vec<_>>();
                format_smolstr!("typing.Tuple[{}]", tuple_types.join(", "))
            }
        },
        Type::Enumeration(enumeration) => {
            if enumeration.node.is_some() {
                ident(&enumeration.name)
            } else {
                SmolStr::new_static("None")
            }
        }
        Type::Callback(function) | Type::Function(function) => {
            format_smolstr!(
                "typing.Callable[[{}], {}]",
                function.args.iter().map(|arg_ty| python_type_name(arg_ty)).join(", "),
                python_type_name(&function.return_type)
            )
        }
        Type::KeyboardShortcutType => SmolStr::new_static("str"),
        ty @ _ => unimplemented!("implemented type conversion {:#?}", ty),
    }
}
