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

    #[derive(Default, Debug)]
    pub struct Function {
        pub name: String,
        /// (...) -> ...
        pub signature: String,
        pub is_constructor: bool,
        pub is_static: bool,
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

struct ItemTreeArrayBuilder<'a> {
    children_offset: usize,
    class_name: &'a str,
}

impl<'a> ItemTreeArrayBuilder<'a> {
    pub fn build_array(&mut self, component: &LoweredComponent) -> String {
        self.children_offset = 1;
        let s = self.visit_item(&component.root_item, self.children_offset, String::new());
        self.visit_children(&component.root_item, s)
    }

    fn visit_children(&mut self, item: &LoweredItem, mut acc: String) -> String {
        for i in &item.children {
            acc = self.visit_item(i, self.children_offset, acc);
            self.children_offset += i.children.len();
        }

        for i in &item.children {
            acc = self.visit_children(i, acc);
        }

        acc
    }

    /// This is the only function which is language dependent
    /// maybe this should be a callback
    fn visit_item(&self, item: &LoweredItem, children_offset: usize, acc: String) -> String {
        format!(
            "{}{}sixtyfps::ItemTreeNode{{ offsetof({}, {}), &sixtyfps::{}, {}, {}  }}",
            acc,
            if acc.is_empty() { "" } else { ", " },
            self.class_name,
            item.id,
            item.native_type.vtable,
            item.children.len(),
            children_offset,
        )
    }
}

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

    let tree_array = ItemTreeArrayBuilder { children_offset: 0, class_name: &component.id }
        .build_array(&component);
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
