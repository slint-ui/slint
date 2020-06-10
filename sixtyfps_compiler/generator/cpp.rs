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

    pub trait CppType {
        fn cpp_type(&self) -> Option<&str>;
    }
}

use crate::diagnostics::{CompilerDiagnostic, Diagnostics};
use crate::object_tree::{Component, Element};
use crate::typeregister::Type;
use cpp_ast::*;
use std::cell::RefCell;
use std::rc::Rc;

impl CppType for Type {
    fn cpp_type(&self) -> Option<&str> {
        match self {
            Type::Float32 => Some("float"),
            Type::Int32 => Some("int"),
            Type::String => Some("sixtyfps::SharedString"),
            Type::Color => Some("uint32_t"),
            Type::Bool => Some("bool"),
            _ => None,
        }
    }
}

fn handle_item(item: &Element, main_struct: &mut Struct, init: &mut Vec<String>) {
    main_struct.members.push(Declaration::Var(Var {
        ty: format!("sixtyfps::{}", item.base_type.as_builtin().class_name),
        name: item.id.clone(),
        ..Default::default()
    }));

    let id = &item.id;
    init.extend(item.bindings.iter().map(|(s, i)| {
        if matches!(item.lookup_property(s.as_str()), Type::Signal) {
            let signal_accessor_prefix = if item.property_declarations.contains_key(s) {
                String::new()
            } else {
                format!("{id}.", id = id.clone())
            };

            format!(
                "{signal_accessor_prefix}{prop}.set_handler(
                    [](const sixtyfps::EvaluationContext *context) {{
                        auto self = reinterpret_cast<const {ty}*>(context->component.instance);
                        {code};
                    }});",
                signal_accessor_prefix = signal_accessor_prefix,
                prop = s,
                ty = main_struct.name,
                code = compile_expression(i)
            )
        } else {
            let accessor_prefix = if item.property_declarations.contains_key(s) {
                String::new()
            } else {
                format!("{id}.", id = id.clone())
            };

            let init = compile_expression(i);
            if i.is_constant() {
                format!(
                    "{accessor_prefix}{cpp_prop}.set({init});",
                    accessor_prefix = accessor_prefix,
                    cpp_prop = s,
                    init = init
                )
            } else {
                format!(
                    "{accessor_prefix}{cpp_prop}.set_binding(
                        [](const sixtyfps::EvaluationContext *context) {{
                            auto self = reinterpret_cast<const {ty}*>(context->component.instance);
                            return {init};
                        }}
                    );",
                    accessor_prefix = accessor_prefix,
                    cpp_prop = s,
                    ty = main_struct.name,
                    init = init
                )
            }
        }
    }));

    for i in &item.children {
        handle_item(&i.borrow(), main_struct, init)
    }
}

/// Returns the text of the C++ code produced by the given root component
pub fn generate(component: &Component, diag: &mut Diagnostics) -> Option<impl std::fmt::Display> {
    let mut x = File::default();

    x.includes.push("<array>".into());
    x.includes.push("<limits>".into());
    x.includes.push("<sixtyfps.h>".into());

    let mut main_struct = Struct { name: component.id.clone(), ..Default::default() };

    main_struct.members.extend(component.root_element.borrow().property_declarations.iter().map(
        |(cpp_name, property_decl)| {
            let ty = if property_decl.property_type == Type::Signal {
                "sixtyfps::Signal".into()
            } else {
                let cpp_type = property_decl.property_type.cpp_type().unwrap_or_else(|| {
                    let err = CompilerDiagnostic {
                        message: "Cannot map property type to C++".into(),
                        span: property_decl.type_location.clone(),
                    };

                    diag.push_compiler_error(err);
                    "".into()
                });
                format!("sixtyfps::Property<{}>", cpp_type)
            };
            Declaration::Var(Var { ty, name: cpp_name.clone(), init: None })
        },
    ));

    let mut init = vec!["auto self = this;".into()];
    handle_item(&component.root_element.borrow(), &mut main_struct, &mut init);

    main_struct.members.push(Declaration::Function(Function {
        name: component.id.clone(),
        signature: "()".to_owned(),
        is_constructor: true,
        statements: Some(init),
        ..Default::default()
    }));

    main_struct.members.push(Declaration::Function(Function {
        name: "tree_fn".into(),
        signature: "(sixtyfps::ComponentRef) -> const sixtyfps::ItemTreeNode* ".into(),
        is_static: true,
        ..Default::default()
    }));

    main_struct.members.push(Declaration::Function(Function {
        name: "compute_layout".into(),
        signature: "(sixtyfps::ComponentRef component) -> void".into(),
        is_static: true,
        statements: Some(compute_layout(component)),
        ..Default::default()
    }));

    main_struct.members.push(Declaration::Var(Var {
        ty: "static const sixtyfps::ComponentVTable".to_owned(),
        name: "component_type".to_owned(),
        init: None,
    }));

    x.declarations.push(Declaration::Struct(main_struct));

    let mut tree_array = String::new();
    super::build_array_helper(component, |item, children_offset| {
        let item = item.borrow();
        tree_array = format!(
            "{}{}sixtyfps::make_item_node(offsetof({}, {}), &sixtyfps::{}, {}, {})",
            tree_array,
            if tree_array.is_empty() { "" } else { ", " },
            &component.id,
            item.id,
            item.base_type.as_builtin().vtable_symbol,
            item.children.len(),
            children_offset,
        )
    });

    x.declarations.push(Declaration::Function(Function {
        name: format!("{}::tree_fn", component.id),
        signature: "(sixtyfps::ComponentRef) -> const sixtyfps::ItemTreeNode* ".into(),
        statements: Some(vec![
            "static const sixtyfps::ItemTreeNode children[] {".to_owned(),
            format!("    {} }};", tree_array),
            "return children;".to_owned(),
        ]),
        ..Default::default()
    }));

    x.declarations.push(Declaration::Var(Var {
        ty: "const sixtyfps::ComponentVTable".to_owned(),
        name: format!("{}::component_type", component.id),
        init: Some(
            "{ nullptr, sixtyfps::dummy_destory, tree_fn, nullptr, compute_layout }".to_owned(),
        ),
    }));

    if diag.has_error() {
        None
    } else {
        Some(x)
    }
}

fn access_member(element: &Rc<RefCell<Element>>, name: &str) -> String {
    let e = element.borrow();
    if e.property_declarations.contains_key(name) {
        name.into()
    } else {
        format!("{}.{}", e.id.as_str(), name)
    }
}

fn compile_expression(e: &crate::expression_tree::Expression) -> String {
    use crate::expression_tree::Expression::*;
    match e {
        StringLiteral(s) => format!(r#"sixtyfps::SharedString("{}")"#, s.escape_default()),
        NumberLiteral(n) => n.to_string(),
        PropertyReference { element, name, .. } => format!(
            r#"self->{}.get(context)"#,
            access_member(&element.upgrade().unwrap(), name.as_str())
        ),
        SignalReference { element, name, .. } => {
            format!(r#"self->{}"#, access_member(&element.upgrade().unwrap(), name.as_str()))
        }
        Cast { from, to } => {
            let f = compile_expression(&*from);
            match (from.ty(), to) {
                (Type::Float32, Type::String) | (Type::Int32, Type::String) => {
                    format!("sixtyfps::SharedString::from_number({})", f)
                }
                _ => f,
            }
        }
        CodeBlock(sub) => {
            let mut x = sub.iter().map(|e| compile_expression(e)).collect::<Vec<_>>();
            x.last_mut().map(|s| *s = format!("return {};", s));

            format!("[&]{{ {} }}()", x.join(";"))
        }
        FunctionCall { function } => {
            if matches!(function.ty(), Type::Signal) {
                format!("{}.emit(context)", compile_expression(&*function))
            } else {
                format!("\n#error the function `{:?}` is not a signal\n", function)
            }
        }
        SelfAssignment { lhs, rhs, op } => match &**lhs {
            PropertyReference { element, name, .. } => format!(
                r#"self->{lhs}.set(self->{lhs}.get(context) {op} {rhs})"#,
                lhs = access_member(&element.upgrade().unwrap(), name.as_str()),
                rhs = compile_expression(&*rhs),
                op = op
            ),
            _ => panic!("typechecking should make sure this was a PropertyReference"),
        },
        ResourceReference { absolute_source_path } => {
            format!(r#"sixtyfps::Resource(sixtyfps::SharedString("{}"))"#, absolute_source_path)
        }
        Condition { condition, true_expr, false_expr } => {
            let cond_code = compile_expression(condition);
            let true_code = compile_expression(true_expr);
            let false_code = compile_expression(false_expr);
            format!(
                r#"[&]() -> {} {{ if ({}) {{ return {}; }} else {{ return {}; }}}}()"#,
                true_expr.ty().cpp_type().unwrap(),
                cond_code,
                true_code,
                false_code
            )
        }
        Uncompiled(_) => panic!(),
        Invalid => format!("\n#error invalid expression\n"),
    }
}

fn compute_layout(component: &Component) -> Vec<String> {
    let mut res = vec![];

    res.push(format!(
        "auto self = reinterpret_cast<const {ty}*>(component.instance);",
        ty = component.id
    ));
    res.push("sixtyfps::EvaluationContext context{component};".to_owned());
    for grid in component.layout_constraints.borrow().0.iter() {
        res.push("{".to_owned());
        res.push(format!("    std::array<sixtyfps::Constraint, {}> row_constr;", grid.row_count()));
        res.push(format!("    std::array<sixtyfps::Constraint, {}> col_constr;", grid.col_count()));
        res.push(
            "    row_constr.fill(sixtyfps::Constraint{0., std::numeric_limits<float>::max()});"
                .to_owned(),
        );
        res.push(
            "    col_constr.fill(sixtyfps::Constraint{0., std::numeric_limits<float>::max()});"
                .to_owned(),
        );
        res.push("    sixtyfps::GridLayoutCellData grid_data[] = {".to_owned());
        let mut row_info = vec![];
        let mut count = 0;
        for row in &grid.elems {
            row_info.push(format!("{{ &grid_data[{}], {} }}", count, row.len()));
            for cell in row {
                if let Some(cell) = cell {
                    let p = |n: &str| {
                        if cell.borrow().lookup_property(n) == Type::Float32 {
                            format!("&self->{}.{}", cell.borrow().id, n)
                        } else {
                            "nullptr".to_owned()
                        }
                    };
                    res.push(format!(
                        "        {{ {}, {}, {}, {} }},",
                        p("x"),
                        p("y"),
                        p("width"),
                        p("height")
                    ));
                } else {
                    res.push("        {},".into());
                }
                count += 1;
            }
        }
        res.push("    };".to_owned());
        res.push("    sixtyfps::Slice<sixtyfps::GridLayoutCellData> cells[] = {".to_owned());
        res.push(format!("        {} }};", row_info.join(", ")));
        res.push("    sixtyfps::GridLayoutData grid { ".into());
        // FIXME: add auto conversion from std::array* to Slice
        res.push("        { row_constr.data(), row_constr.size() },".to_owned());
        res.push("        { col_constr.data(), col_constr.size() },".to_owned());
        res.push(format!("        self->{}.width.get(&context),", grid.within.borrow().id));
        res.push(format!("        self->{}.height.get(&context),", grid.within.borrow().id));
        res.push("        0., 0.,".to_owned());
        res.push("        {cells, std::size(cells)}".to_owned());
        res.push("    };".to_owned());
        res.push("    sixtyfps::solve_grid_layout(&grid);".to_owned());
        res.push("}".to_owned());
    }
    res
}
