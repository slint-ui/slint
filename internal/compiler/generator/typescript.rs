// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*! module for the TypeScript code generator
*/

use std::collections::BTreeMap;
use std::{collections::HashSet, sync::OnceLock};

use crate::langtype::{Enumeration, EnumerationValue, Type};
use crate::object_tree::Document;
use crate::CompilerConfiguration;
use smol_str::{format_smolstr, SmolStr, StrExt};
use typescript_ast::*;

// Check if word is one of C++ keywords
fn is_typescript_keyword(word: &str) -> bool {
    static TS_KEYWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    let keywords = TS_KEYWORDS.get_or_init(|| {
        #[rustfmt::skip]
        let keywords: HashSet<&str> = HashSet::from(["import", "export", "enum"]);
        keywords
    });
    keywords.contains(word)
}

fn ident(ident: &str) -> SmolStr {
    let mut new_ident = SmolStr::from(ident);
    if ident.contains('-') {
        new_ident = ident.replace_smolstr("-", "_");
    }
    if is_typescript_keyword(new_ident.as_str()) {
        new_ident = format_smolstr!("{}_", new_ident);
    }
    new_ident
}

/// This module contains some data structure that helps represent a TypeScript code.
/// It is then rendered into an actual TypeScript text using the Display trait
mod typescript_ast {
    use smol_str::SmolStr;
    use std::{
        cell::Cell,
        fmt::{Display, Error, Formatter},
    };

    thread_local!(static INDENTATION : Cell<u32> = Cell::new(0));
    fn indent(f: &mut Formatter<'_>) -> Result<(), Error> {
        INDENTATION.with(|i| {
            for _ in 0..(i.get()) {
                write!(f, "    ")?;
            }
            Ok(())
        })
    }

    /// A full TypeScript file
    #[derive(Default, Debug)]
    pub struct File {
        pub imports: Vec<Import>,
        pub declarations: Vec<Declaration>,
    }

    impl Display for File {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            writeln!(f, "// This file is auto-generated")?;
            for i in &self.imports {
                writeln!(f, "import {} from '{}';", i.exports, i.module)?;
            }
            for d in &self.declarations {
                write!(f, "\n{}", d)?;
            }
            Ok(())
        }
    }

    #[derive(Default, Debug)]
    pub struct Import {
        pub exports: SmolStr,
        pub module: SmolStr,
    }

    /// Declarations  (top level, or within a struct)
    #[derive(Debug, derive_more::Display)]
    pub enum Declaration {
        Class(Class),
        Enum(Enum),
    }

    /// Member declaration
    #[derive(Default, Debug)]
    pub struct Field {
        pub name: SmolStr,
        pub ty: SmolStr,
    }

    impl Display for Field {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            indent(f)?;
            writeln!(f, "{}: {};", self.ty, self.name)
        }
    }

    #[derive(Default, Debug)]
    pub struct Class {
        pub name: SmolStr,
        pub members: Vec<Field>,
    }

    impl Display for Class {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            indent(f)?;
            writeln!(f, "export class {} {{", self.name)?;
            INDENTATION.with(|x| x.set(x.get() + 1));

            // todo: content
            for m in &self.members {
                write!(f, "{}", m)?;
            }

            INDENTATION.with(|x| x.set(x.get() - 1));
            indent(f)?;
            writeln!(f, "}};")
        }
    }

    #[derive(Default, Debug)]
    pub struct Enum {
        pub name: SmolStr,
        pub values: Vec<SmolStr>,
    }

    impl Display for Enum {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            indent(f)?;
            writeln!(f, "export enum {} {{", self.name)?;
            INDENTATION.with(|x| x.set(x.get() + 1));
            for value in &self.values {
                write!(f, "{value},")?;
            }
            INDENTATION.with(|x| x.set(x.get() - 1));
            indent(f)?;
            writeln!(f, "}};")
        }
    }

    pub trait TypeScriptType {
        fn ts_type(&self) -> Option<SmolStr>;
    }
}

impl TypeScriptType for Type {
    fn ts_type(&self) -> Option<SmolStr> {
        match self {
            Type::Void => Some("void".into()),
            Type::Float32 => Some("number".into()),
            Type::Int32 => Some("number".into()),
            Type::String => Some("string".into()),
            Type::Color => Some("slint.RgbaColor".into()),
            Type::Duration => Some("number".into()),
            Type::PhysicalLength => Some("number".into()),
            Type::LogicalLength => Some("number".into()),
            Type::Rem => Some("number".into()),
            Type::Angle => Some("number".into()),
            Type::Percent => Some("number".into()),
            Type::Image => Some("slint.ImageData".into()),
            Type::Bool => Some("boolean".into()),
            Type::Brush => Some("slint.Brush".into()),
            Type::Array(i) => Some(format_smolstr!("slint.Model<{}>", i.ts_type()?)),
            Type::Struct(s) => match (&s.name, &s.node) {
                (Some(name), Some(_)) => Some(ident(name)),
                (Some(name), None) => {
                    if name.starts_with("slint.") {
                        Some(name.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            },
            Type::Enumeration(enumeration) => {
                if enumeration.node.is_some() {
                    Some(ident(&enumeration.name))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

pub fn generate(
    doc: &Document,
    compiler_config: &CompilerConfiguration,
) -> std::io::Result<impl std::fmt::Display> {
    let mut file = File { ..Default::default() };

    file.imports.push(Import { exports: "* as slint".into(), module: "slint-ui".into() });

    for ty in doc.used_types.borrow().structs_and_enums.iter() {
        match ty {
            Type::Struct(s) if s.name.is_some() && s.node.is_some() => {
                generate_class(
                    &mut file,
                    s.name.as_ref().unwrap(),
                    &s.fields,
                    // s.node.as_ref().unwrap(),
                );
            }
            Type::Enumeration(en) => {
                generate_enum(&mut file, en);
            }
            _ => (),
        }
    }

    Ok(file)
}

fn generate_class(file: &mut File, name: &str, fields: &BTreeMap<SmolStr, Type>) {
    let name = ident(name);

    file.declarations.push(Declaration::Class(Class { name, ..Default::default() }));
}

fn generate_enum(file: &mut File, en: &std::rc::Rc<Enumeration>) {
    file.declarations.push(Declaration::Enum(Enum {
        name: ident(&en.name),
        values: (0..en.values.len())
            .map(|value| {
                ident(&EnumerationValue { value, enumeration: en.clone() }.to_pascal_case())
            })
            .collect(),
    }))
}
