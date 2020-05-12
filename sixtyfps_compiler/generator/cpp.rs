/*! module for the C++ code generator
*/

/// This module contains some datastructure that helps represent a C++ code.
/// It is then rendered into an actual C++ text using the Display trait
mod cpp_ast {

    use std::cell::Cell;
    use std::fmt::{Display, Error, Formatter};
    thread_local!(static INDETATION : Cell<u32> = Cell::new(0));
    fn indent(f: &mut Formatter<'_>) -> Result<(), Error> {
        INDETATION.with(|i| {
            for _ in 0..(i.get()) {
                write!(f, "    ")?;
            }
            Ok(())
        })
    }

    ///A full C++ file
    #[derive(Default, Debug)]
    pub struct File {
        pub includes: Vec<String>,
        pub declarations: Vec<Declaration>,
    }

    impl Display for File {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            for i in &self.includes {
                writeln!(f, "#include {}", i)?;
            }
            for d in &self.declarations {
                write!(f, "\n{}", d)?;
            }
            Ok(())
        }
    }

    /// Declarations  (top level, or within a struct)
    #[derive(Debug, derive_more::Display)]
    pub enum Declaration {
        Struct(Struct),
        Function(Function),
        Var(Var),
    }

    #[derive(Default, Debug)]
    pub struct Struct {
        pub name: String,
        pub members: Vec<Declaration>,
    }

    impl Display for Struct {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            indent(f)?;
            writeln!(f, "struct {} {{", self.name)?;
            INDETATION.with(|x| x.set(x.get() + 1));
            for m in &self.members {
                // FIXME! identation
                write!(f, "{}", m)?;
            }
            INDETATION.with(|x| x.set(x.get() - 1));
            indent(f)?;
            writeln!(f, "}};")
        }
    }

    /// Function or method
    #[derive(Default, Debug)]
    pub struct Function {
        pub name: String,
        /// "(...) -> ..."
        pub signature: String,
        /// The function does not have return type
        pub is_constructor: bool,
        pub is_static: bool,
        /// The list of statement instead the function.  When None,  this is just a function
        /// declaration without the definition
        pub statements: Option<Vec<String>>,
    }

    impl Display for Function {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            indent(f)?;
            if self.is_static {
                write!(f, "static ")?;
            }
            if !self.is_constructor {
                write!(f, "auto ")?;
            }
            write!(f, "{} {}", self.name, self.signature)?;
            if let Some(st) = &self.statements {
                writeln!(f, "{{")?;
                for s in st {
                    indent(f)?;
                    writeln!(f, "    {}", s)?;
                }
                indent(f)?;
                writeln!(f, "}}")
            } else {
                writeln!(f, ";")
            }
        }
    }

    /// A variable or a member declaration.
    #[derive(Default, Debug)]
    pub struct Var {
        pub ty: String,
        pub name: String,
        pub init: Option<String>,
    }

    impl Display for Var {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            indent(f)?;
            write!(f, "{} {}", self.ty, self.name)?;
            if let Some(i) = &self.init {
                write!(f, " = {}", i)?;
            }
            writeln!(f, ";")
        }
    }
}

use crate::lower::{LoweredComponent, LoweredItem};
use cpp_ast::*;

fn handle_item(item: &LoweredItem, main_struct: &mut Struct, init: &mut Vec<String>) {
    main_struct.members.push(Declaration::Var(Var {
        ty: format!("sixtyfps::{}", item.native_type.class_name),
        name: item.id.clone(),
        ..Default::default()
    }));

    let id = &item.id;
    init.extend(
        item.init_properties
            .iter()
            .map(|(s, i)| format!("{id}.{prop} = {init};", id = id, prop = s, init = i)),
    );

    for i in &item.children {
        handle_item(i, main_struct, init)
    }
}

/// Returns the text of the C++ code produced by the given root component
pub fn generate(component: &LoweredComponent) -> impl std::fmt::Display {
    let mut x = File::default();

    x.includes.push("<sixtyfps.h>".into());

    let mut main_struct = Struct { name: component.id.clone(), ..Default::default() };

    let mut init = Vec::new();
    handle_item(&component.root_item, &mut main_struct, &mut init);

    main_struct.members.push(Declaration::Function(Function {
        name: component.id.clone(),
        signature: "()".to_owned(),
        is_constructor: true,
        statements: Some(init),
        ..Default::default()
    }));

    main_struct.members.push(Declaration::Function(Function {
        name: "tree_fn".into(),
        signature: "(const sixtyfps::ComponentType*) -> const sixtyfps::ItemTreeNode* ".into(),
        is_static: true,
        ..Default::default()
    }));

    main_struct.members.push(Declaration::Var(Var {
        ty: "static const sixtyfps::ComponentType".to_owned(),
        name: "component_type".to_owned(),
        init: None,
    }));

    x.declarations.push(Declaration::Struct(main_struct));

    let mut tree_array = String::new();
    super::build_array_helper(component, |item: &LoweredItem, children_offset: usize| {
        tree_array = format!(
            "{}{}sixtyfps::make_item_node(offsetof({}, {}), &sixtyfps::{}, {}, {})",
            tree_array,
            if tree_array.is_empty() { "" } else { ", " },
            &component.id,
            item.id,
            item.native_type.vtable,
            item.children.len(),
            children_offset,
        )
    });

    x.declarations.push(Declaration::Function(Function {
        name: format!("{}::tree_fn", component.id),
        signature: "(const sixtyfps::ComponentType*) -> const sixtyfps::ItemTreeNode* ".into(),
        statements: Some(vec![
            "static const sixtyfps::ItemTreeNode children[] {".to_owned(),
            format!("    {} }};", tree_array),
            "return children;".to_owned(),
        ]),
        ..Default::default()
    }));

    x.declarations.push(Declaration::Var(Var {
        ty: "const sixtyfps::ComponentType".to_owned(),
        name: format!("{}::component_type", component.id),
        init: Some("{ nullptr, sixtyfps::dummy_destory, tree_fn }".to_owned()),
    }));

    x.declarations.push(Declaration::Function(Function {
        name: "main".into(),
        signature: "() -> int".to_owned(),
        statements: Some(vec![
            format!("static {} component;", component.id),
            format!("sixtyfps::run(&component);"),
        ]),
        ..Default::default()
    }));
    x
}
