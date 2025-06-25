// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*! module for the C++ code generator
*/

// cSpell:ignore cmath constexpr cstdlib decltype intptr itertools nullptr prepended struc subcomponent uintptr vals

use std::collections::HashSet;
use std::sync::OnceLock;

use smol_str::{format_smolstr, SmolStr, StrExt};

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

fn ident(ident: &str) -> SmolStr {
    let mut new_ident = SmolStr::from(ident);
    if ident.contains('-') {
        new_ident = ident.replace_smolstr("-", "_");
    }
    if is_python_keyword(new_ident.as_str()) {
        new_ident = format_smolstr!("{}_", new_ident);
    }
    new_ident
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
            writeln!(f, "")?;
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
                writeln!(f, "")?;
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
        pub ty: PyType,
        pub default_value: Option<SmolStr>,
    }

    impl Display for Field {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}: {}", self.name, self.ty)?;
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

use crate::langtype::Type;

use crate::llr;
use crate::object_tree::Document;
use crate::CompilerConfiguration;
use itertools::{Either, Itertools};
use python_ast::*;

/// Returns the text of the C++ code produced by the given root component
pub fn generate(
    doc: &Document,
    compiler_config: &CompilerConfiguration,
    destination_path: Option<&std::path::Path>,
) -> std::io::Result<impl std::fmt::Display> {
    let mut file = File { ..Default::default() };
    file.imports.push(SmolStr::new_static("slint"));
    file.imports.push(SmolStr::new_static("typing"));

    for ty in &doc.used_types.borrow().structs_and_enums {
        match ty {
            Type::Struct(s) => {
                if let Some(name) = &s.name {
                    let fields = s
                        .fields
                        .iter()
                        .map(|(name, ty)| Field {
                            name: ident(name),
                            ty: PyType { name: python_type_name(ty), optional: false },
                            default_value: None,
                        })
                        .collect::<Vec<_>>();

                    let ctor = FunctionDeclaration {
                        name: SmolStr::new_static("__init__"),
                        positional_parameters: Vec::default(),
                        keyword_parameters: fields
                            .iter()
                            .map(|field| {
                                let mut kw_field = field.clone();
                                kw_field.ty.optional = true;
                                kw_field.default_value = Some(SmolStr::new_static("None"));
                                kw_field
                            })
                            .collect(),
                        return_type: None,
                    };

                    let struct_class = Class {
                        name: name.clone(),
                        fields,
                        function_declarations: vec![ctor],
                        ..Default::default()
                    };
                    file.declarations.push(Declaration::Class(struct_class));
                }
            }
            Type::Enumeration(_) => {}
            _ => {}
        }
    }

    let llr = llr::lower_to_item_tree::lower_to_item_tree(doc, compiler_config)?;

    let globals = llr.globals.iter().filter(|glob| glob.exported && glob.must_generate());

    for global in globals.clone() {
        generate_global(global, &mut file);
    }

    for public_component in &llr.public_components {
        generate_public_component(&public_component, globals.clone(), &mut file);
    }

    file.declarations.extend(generate_named_exports(&doc.exports));

    let main_file = std::path::absolute(
        doc.node
            .as_ref()
            .ok_or_else(|| std::io::Error::other("Cannot determine path of the main file"))?
            .source_file
            .path(),
    )
    .unwrap();

    let destination_path = destination_path.and_then(|maybe_relative_destination_path| {
        std::path::absolute(maybe_relative_destination_path)
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
        file.imports.push(SmolStr::new_static("os"));
        file.trailing_code.push(format_smolstr!(
            "globals().update(vars(slint.load_file(os.path.join(os.path.dirname(__file__), '{}'))))",
            relative_path_from_destination_to_main_file.join(main_file.file_name().unwrap()).to_string_lossy()
        ));
    }

    Ok(file)
}

fn generate_global(global: &llr::GlobalComponent, file: &mut File) {
    let global_name = ident(&global.name);

    let mut class = Class { name: global_name.clone(), ..Default::default() };

    class.fields = generate_fields_for_public_properties(&global.public_properties).collect();

    file.declarations.push(Declaration::Class(class));

    file.declarations.extend(global.aliases.iter().map(|exported_name| {
        Declaration::Variable(Variable { name: ident(&exported_name), value: global_name.clone() })
    }))
}

fn generate_public_component<'a>(
    component: &'a llr::PublicComponent,
    globals: impl Iterator<Item = &'a llr::GlobalComponent>,
    file: &mut File,
) {
    let mut class = Class {
        name: ident(&component.name),
        super_class: Some(SmolStr::new_static("slint.Component")),
        ..Default::default()
    };

    class.fields = generate_fields_for_public_properties(&component.public_properties)
        .chain(globals.map(|glob| {
            let glob_name = ident(&glob.name);
            Field {
                name: glob_name.clone(),
                ty: PyType { name: glob_name, optional: false },
                default_value: None,
            }
        }))
        .collect();

    file.declarations.push(Declaration::Class(class));
}

fn generate_fields_for_public_properties(
    public_properties: &llr::PublicProperties,
) -> impl Iterator<Item = Field> + '_ {
    public_properties.iter().map(|property| Field {
        name: ident(&property.name),
        ty: PyType { name: python_type_name(&property.ty), optional: false },
        default_value: None,
    })
}

pub fn generate_named_exports(
    exports: &crate::object_tree::Exports,
) -> impl Iterator<Item = Declaration> + '_ {
    exports
        .iter()
        .filter_map(|export| match &export.1 {
            Either::Left(component) if !component.is_global() => {
                Some((&export.0.name, &component.id))
            }
            Either::Right(ty) => match &ty {
                Type::Struct(s) if s.name.is_some() && s.node.is_some() => {
                    Some((&export.0.name, s.name.as_ref().unwrap()))
                }
                Type::Enumeration(en) => Some((&export.0.name, &en.name)),
                _ => None,
            },
            _ => None,
        })
        .filter(|(export_name, type_name)| export_name != type_name)
        .map(|(export_name, type_name)| {
            let type_id = ident(type_name);
            let export_id = ident(export_name);
            Declaration::Variable(Variable { name: export_id, value: type_id })
        })
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
        | Type::UnitProduct(_) => SmolStr::new_static("float"),
        Type::Image => SmolStr::new_static("slint.Image"),
        Type::Bool => SmolStr::new_static("bool"),
        Type::Brush => SmolStr::new_static("Brush"),
        Type::Array(elem_type) => format_smolstr!("slint.Model[{}]", python_type_name(elem_type)),
        Type::Struct(s) => match (&s.name, &s.node) {
            (Some(name), Some(_)) => ident(name),
            (Some(name), None) => todo!(),
            _ => {
                let tuple_types =
                    s.fields.values().map(|ty| python_type_name(ty)).collect::<Vec<_>>();
                format_smolstr!("typing.Tuple[{}]", tuple_types.join(", "))
            }
        },
        Type::Enumeration(enumeration) => todo!(),
        Type::Callback(function) | Type::Function(function) => {
            format_smolstr!(
                "typing.Callable[[{}], {}]",
                function.args.iter().map(|arg_ty| python_type_name(arg_ty)).join(", "),
                python_type_name(&function.return_type)
            )
        }
        ty @ _ => unimplemented!("implemented type conversion {:#?}", ty),
    }
}
