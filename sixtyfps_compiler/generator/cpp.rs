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
            // all functions are inlines because we are in a header
            write!(f, "inline ")?;
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
        fn cpp_type(&self) -> Option<String>;
    }
}

use crate::diagnostics::{BuildDiagnostics, CompilerDiagnostic};
use crate::expression_tree::Expression;
use crate::object_tree::{Component, Element, ElementRc, RepeatedElementInfo};
use crate::parser::Spanned;
use crate::typeregister::Type;
use cpp_ast::*;
use std::collections::HashMap;
use std::rc::Rc;

impl CppType for Type {
    fn cpp_type(&self) -> Option<String> {
        match self {
            Type::Float32 => Some("float".to_owned()),
            Type::Int32 => Some("int".to_owned()),
            Type::String => Some("sixtyfps::SharedString".to_owned()),
            Type::Color => Some("sixtyfps::Color".to_owned()),
            Type::Duration => Some("std::int64_t".to_owned()),
            Type::Length => Some("float".to_owned()),
            Type::LogicalLength => Some("float".to_owned()),
            Type::Bool => Some("bool".to_owned()),
            Type::Model => Some("std::shared_ptr<sixtyfps::Model>".to_owned()),
            Type::Object(o) => {
                let elem = o.values().map(|v| v.cpp_type()).collect::<Option<Vec<_>>>()?;
                // This will produce a tuple
                Some(format!("std::tuple<{}>", elem.join(", ")))
            }
            Type::Builtin(elem) => elem.cpp_type.clone(),
            _ => None,
        }
    }
}

fn new_struct_with_bindings(
    type_name: &str,
    bindings: &HashMap<String, Expression>,
    component: &Rc<Component>,
) -> String {
    let bindings_initialization: Vec<String> = bindings
        .iter()
        .map(|(prop, initializer)| {
            let initializer = compile_expression(initializer, component);
            format!("var.{} = {};", prop, initializer)
        })
        .collect();

    format!(
        r#"[&](){{
            {} var{{}};
            {}
            return var;
        }}()"#,
        type_name,
        bindings_initialization.join("\n")
    )
}

fn property_animation_code(
    component: &Rc<Component>,
    element: &Element,
    property_name: &str,
) -> Option<String> {
    if let Some(animation) = element.property_animations.get(property_name) {
        Some(new_struct_with_bindings(
            "sixtyfps::internal::PropertyAnimation",
            &animation.borrow().bindings,
            component,
        ))
    } else {
        None
    }
}
fn property_set_value_code(
    component: &Rc<Component>,
    element: &Element,
    property_name: &str,
    value_expr: String,
) -> String {
    if let Some(animation_code) = property_animation_code(component, element, property_name) {
        format!(
            "set_animated_value({value}, {animation})",
            value = value_expr,
            animation = animation_code
        )
    } else {
        format!("set({})", value_expr)
    }
}

fn property_set_binding_code(
    component: &Rc<Component>,
    element: &Element,
    property_name: &str,
    binding_expr: String,
) -> String {
    if let Some(animation_code) = property_animation_code(component, element, property_name) {
        format!(
            "set_animated_binding({binding}, {animation})",
            binding = binding_expr,
            animation = animation_code
        )
    } else {
        format!("set_binding({})", binding_expr)
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
                    [this]() {{
                        [[maybe_unused]] auto self = this;
                        {code};
                    }});",
                signal_accessor_prefix = signal_accessor_prefix,
                prop = s,
                code = compile_expression(i, &item.enclosing_component.upgrade().unwrap())
            )
        } else {
            let accessor_prefix = if item.property_declarations.contains_key(s) {
                String::new()
            } else {
                format!("{id}.", id = id.clone())
            };

            let component = &item.enclosing_component.upgrade().unwrap();

            let init = compile_expression(i, component);
            if i.is_constant() {
                let setter = property_set_value_code(&component, item, s, init);
                format!(
                    "{accessor_prefix}{cpp_prop}.{setter};",
                    accessor_prefix = accessor_prefix,
                    cpp_prop = s,
                    setter = setter
                )
            } else {
                let binding_code = format!(
                    "[this]() {{
                            [[maybe_unused]] auto self = this;
                            return {init};
                        }}",
                    init = init
                );

                let binding_setter = property_set_binding_code(component, item, s, binding_code);

                format!(
                    "{accessor_prefix}{cpp_prop}.{binding_setter};",
                    accessor_prefix = accessor_prefix,
                    cpp_prop = s,
                    binding_setter = binding_setter,
                )
            }
        }
    }));
}

fn handle_repeater(
    repeated: &RepeatedElementInfo,
    base_component: &Rc<Component>,
    parent_component: &Rc<Component>,
    repeater_count: i32,
    component_struct: &mut Struct,
    init: &mut Vec<String>,
    children_repeater_cases: &mut Vec<String>,
) {
    let repeater_id =
        format!("repeater_{}", base_component.parent_element.upgrade().unwrap().borrow().id);

    let model = compile_expression(&repeated.model, parent_component);
    let model = if !repeated.is_conditional_element {
        format!("{}.get()", model)
    } else {
        // bool converts to int
        // FIXME: don't do a heap allocation here
        format!("std::make_shared<sixtyfps::IntModel>({}).get()", model)
    };

    if repeated.model.is_constant() {
        children_repeater_cases.push(format!(
            "\n        case {i}: self->{id}.visit(visitor); break;",
            id = repeater_id,
            i = repeater_count
        ));
        init.push(format!(
            "self->{repeater_id}.update_model({model}, self);",
            repeater_id = repeater_id,
            model = model,
        ));
    } else {
        let model_id = format!("model_{}", repeater_count);
        component_struct.members.push(Declaration::Var(Var {
            ty: "sixtyfps::PropertyListenerScope".to_owned(),
            name: model_id,
            init: None,
        }));
        children_repeater_cases.push(format!(
            "\n        case {i}: {{
                if (self->model_{i}.is_dirty()) {{
                    self->model_{i}.evaluate([&] {{
                        self->{id}.update_model({model}, self);
                    }});
                }}
                self->{id}.visit(visitor);
                break;
            }}",
            id = repeater_id,
            i = repeater_count,
            model = model,
        ));
    }

    component_struct.members.push(Declaration::Var(Var {
        ty: format!("sixtyfps::Repeater<struct {}>", component_id(base_component)),
        name: repeater_id,
        init: None,
    }));
}

/// Returns the text of the C++ code produced by the given root component
pub fn generate(
    component: &Rc<Component>,
    diag: &mut BuildDiagnostics,
) -> Option<impl std::fmt::Display> {
    let mut file = File::default();

    file.includes.push("<array>".into());
    file.includes.push("<limits>".into());
    file.includes.push("<sixtyfps.h>".into());

    generate_component(&mut file, component, diag);

    if diag.has_error() {
        None
    } else {
        Some(file)
    }
}

fn generate_component(file: &mut File, component: &Rc<Component>, diag: &mut BuildDiagnostics) {
    let component_id = component_id(component);
    let mut component_struct = Struct { name: component_id.clone(), ..Default::default() };

    let is_root = component.parent_element.upgrade().is_none();
    let mut init = vec!["[[maybe_unused]] auto self = this;".into()];

    for (cpp_name, property_decl) in component.root_element.borrow().property_declarations.iter() {
        let ty = if property_decl.property_type == Type::Signal {
            if property_decl.expose_in_public_api && is_root {
                let signal_emitter: Vec<String> = vec![format!("{}.emit();", cpp_name)];

                component_struct.members.push(Declaration::Function(Function {
                    name: format!("emit_{}", cpp_name),
                    signature: "()".into(),
                    statements: Some(signal_emitter),
                    ..Default::default()
                }));
            }

            "sixtyfps::Signal".into()
        } else {
            let cpp_type = property_decl.property_type.cpp_type().unwrap_or_else(|| {
                let err = CompilerDiagnostic {
                    message: "Cannot map property type to C++".into(),
                    span: property_decl.type_node.span(),
                };

                diag.push_internal_error(err.into());
                "".into()
            });

            if property_decl.expose_in_public_api && is_root {
                let prop_getter: Vec<String> = vec![format!("return {}.get();", cpp_name)];

                component_struct.members.push(Declaration::Function(Function {
                    name: format!("get_{}", cpp_name),
                    signature: format!("() -> {}", cpp_type),
                    statements: Some(prop_getter),
                    ..Default::default()
                }));

                let prop_setter: Vec<String> = vec![format!("this->{}.set(value);", cpp_name)];
                component_struct.members.push(Declaration::Function(Function {
                    name: format!("set_{}", cpp_name),
                    signature: format!("(const {} &value)", cpp_type),
                    statements: Some(prop_setter),
                    ..Default::default()
                }));
            }

            format!("sixtyfps::Property<{}>", cpp_type)
        };
        component_struct.members.push(Declaration::Var(Var {
            ty,
            name: cpp_name.clone(),
            init: None,
        }));
    }

    if !is_root {
        let parent_element = component.parent_element.upgrade().unwrap();

        let mut update_statements = vec![];

        if !parent_element.borrow().repeated.as_ref().map_or(false, |r| r.is_conditional_element) {
            component_struct.members.push(Declaration::Var(Var {
                ty: "sixtyfps::Property<int>".into(),
                name: "index".into(),
                init: None,
            }));
            let cpp_model_data_type = crate::expression_tree::Expression::RepeaterModelReference {
                element: component.parent_element.clone(),
            }
            .ty()
            .cpp_type()
            .unwrap_or_else(|| {
                diag.push_internal_error(
                    CompilerDiagnostic {
                        message: "Cannot map property type to C++".into(),
                        span: parent_element
                            .borrow()
                            .node
                            .as_ref()
                            .map(|n| n.span())
                            .unwrap_or_default(),
                    }
                    .into(),
                );
                String::default()
            })
            .to_owned();
            component_struct.members.push(Declaration::Var(Var {
                ty: format!("sixtyfps::Property<{}>", cpp_model_data_type),
                name: "model_data".into(),
                init: None,
            }));

            update_statements = vec![
                "index.set(i);".into(),
                format!("model_data.set(*reinterpret_cast<{} const*>(data));", cpp_model_data_type),
            ];
        }
        component_struct.members.push(Declaration::Var(Var {
            ty: format!(
                "{} const *",
                self::component_id(
                    &component
                        .parent_element
                        .upgrade()
                        .unwrap()
                        .borrow()
                        .enclosing_component
                        .upgrade()
                        .unwrap()
                )
            ),
            name: "parent".into(),
            init: Some("nullptr".to_owned()),
        }));
        component_struct.members.push(Declaration::Function(Function {
            name: "update_data".into(),
            signature: "([[maybe_unused]] int i, [[maybe_unused]] const void *data) -> void".into(),
            statements: Some(update_statements),
            ..Function::default()
        }));
    } else {
        component_struct.members.push(Declaration::Var(Var {
            ty: "sixtyfps::Property<float>".into(),
            name: "dpi".into(),
            ..Var::default()
        }));
        init.push("self->dpi.set(1.);".to_owned());

        let window_props = |name| {
            let root_elem = component.root_element.borrow();

            if root_elem.lookup_property(name) == Type::Length {
                format!("&this->{}.{}", root_elem.id, name)
            } else {
                "nullptr".to_owned()
            }
        };
        component_struct.members.push(Declaration::Function(Function {
            name: "window_properties".into(),
            signature: "() -> sixtyfps::WindowProperties".into(),
            statements: Some(vec![format!(
                "return {{ {} , {}, &this->dpi }};",
                window_props("width"),
                window_props("height")
            )]),
            ..Default::default()
        }));
    }

    let mut children_visitor_case = vec![];
    let mut tree_array = String::new();
    let mut repeater_count = 0;
    super::build_array_helper(component, |item, children_offset| {
        let item = item.borrow();
        if let Some(repeated) = &item.repeated {
            tree_array = format!(
                "{}{}sixtyfps::make_dyn_node({})",
                tree_array,
                if tree_array.is_empty() { "" } else { ", " },
                repeater_count,
            );
            let base_component = item.base_type.as_component();
            generate_component(file, base_component, diag);
            handle_repeater(
                repeated,
                base_component,
                component,
                repeater_count,
                &mut component_struct,
                &mut init,
                &mut children_visitor_case,
            );
            repeater_count += 1;
        } else {
            tree_array = format!(
                "{}{}sixtyfps::make_item_node(offsetof({}, {}), &sixtyfps::{}, {}, {})",
                tree_array,
                if tree_array.is_empty() { "" } else { ", " },
                &component_id,
                item.id,
                item.base_type.as_builtin().vtable_symbol,
                item.children.len(),
                children_offset,
            );
            handle_item(&*item, &mut component_struct, &mut init);
        }
    });

    component_struct.members.push(Declaration::Function(Function {
        name: component_id.clone(),
        signature: "()".to_owned(),
        is_constructor: true,
        statements: None,
        ..Default::default()
    }));

    component_struct.members.push(Declaration::Function(Function {
        name: "visit_children".into(),
        signature: "(sixtyfps::ComponentRef, intptr_t, sixtyfps::ItemVisitorRefMut) -> void".into(),
        is_static: true,
        ..Default::default()
    }));

    component_struct.members.push(Declaration::Function(Function {
        name: "compute_layout".into(),
        signature: "(sixtyfps::ComponentRef component) -> void".into(),
        is_static: true,
        ..Default::default()
    }));

    component_struct.members.push(Declaration::Var(Var {
        ty: "static const sixtyfps::ComponentVTable".to_owned(),
        name: "component_type".to_owned(),
        init: None,
    }));

    let mut declarations = vec![];
    declarations.push(Declaration::Struct(component_struct));

    declarations.push(Declaration::Function(Function {
        name: format!("{0}::{0}", component_id),
        signature: "()".to_owned(),
        is_constructor: true,
        statements: Some(init),
        ..Default::default()
    }));

    declarations.push(Declaration::Function(Function {
        name: format!("{}::visit_children", component_id),
        signature: "(sixtyfps::ComponentRef component, intptr_t index, sixtyfps::ItemVisitorRefMut visitor) -> void".into(),
        statements: Some(vec![
            "static const sixtyfps::ItemTreeNode<uint8_t> children[] {".to_owned(),
            format!("    {} }};", tree_array),
            "static const auto dyn_visit = [] (const uint8_t *base, [[maybe_unused]] sixtyfps::ItemVisitorRefMut visitor, uintptr_t dyn_index) {".to_owned(),
            format!("    [[maybe_unused]] auto self = reinterpret_cast<const {}*>(base);", component_id),
            // Fixme: this is not the root component
            format!("    switch(dyn_index) {{ {} }}\n  }};", children_visitor_case.join("")),
            "return sixtyfps::sixtyfps_visit_item_tree(component, { const_cast<sixtyfps::ItemTreeNode<uint8_t>*>(children), std::size(children)}, index, visitor, dyn_visit);".to_owned(),
        ]),
        ..Default::default()
    }));

    declarations.push(Declaration::Var(Var {
        ty: "const sixtyfps::ComponentVTable".to_owned(),
        name: format!("{}::component_type", component_id),
        init: Some("{ visit_children, nullptr, compute_layout }".to_owned()),
    }));

    declarations.append(&mut file.declarations);

    declarations.push(Declaration::Function(Function {
        name: format!("{}::compute_layout", component_id),
        signature: "(sixtyfps::ComponentRef component) -> void".into(),
        statements: Some(compute_layout(component)),
        ..Default::default()
    }));

    file.declarations = declarations;
}

fn component_id(component: &Rc<Component>) -> String {
    if component.id.is_empty() {
        format!("Component_{}", component.root_element.borrow().id)
    } else {
        component.id.clone()
    }
}

/// Returns the code that can access the given property (but without the set or get)
///
/// to be used like:
/// ```ignore
/// let access = access_member(...);
/// format!("{}.get()", access)
/// ```
fn access_member(
    element: &ElementRc,
    name: &str,
    component: &Rc<Component>,
    component_cpp: &str,
) -> String {
    let e = element.borrow();
    let enclosing_component = e.enclosing_component.upgrade().unwrap();
    if Rc::ptr_eq(component, &enclosing_component) {
        let e = element.borrow();
        if e.property_declarations.contains_key(name) {
            format!("{}->{}", component_cpp, name)
        } else {
            format!("{}->{}.{}", component_cpp, e.id.as_str(), name)
        }
    } else {
        access_member(
            element,
            name,
            &component
                .parent_element
                .upgrade()
                .unwrap()
                .borrow()
                .enclosing_component
                .upgrade()
                .unwrap(),
            &format!("{}->parent", component_cpp),
        )
    }
}

/// Return an expression that gets the DPI property
fn dpi_expression(component: &Rc<Component>) -> String {
    let mut root_component = component.clone();
    let mut component_cpp = "self".to_owned();
    while let Some(p) = root_component.parent_element.upgrade() {
        root_component = p.borrow().enclosing_component.upgrade().unwrap();
        component_cpp = format!("{}->parent", component_cpp);
    }
    format!("{}->dpi.get()", component_cpp)
}

fn compile_expression(e: &crate::expression_tree::Expression, component: &Rc<Component>) -> String {
    use crate::expression_tree::Expression::*;
    use crate::expression_tree::NamedReference;
    match e {
        StringLiteral(s) => format!(r#"sixtyfps::SharedString("{}")"#, s.escape_default()),
        NumberLiteral(n, unit) => unit.normalize(*n).to_string(),
        BoolLiteral(b) => b.to_string(),
        PropertyReference(NamedReference { element, name }) => {
            let access =
                access_member(&element.upgrade().unwrap(), name.as_str(), component, "self");
            format!(r#"{}.get()"#, access)
        }
        SignalReference(NamedReference { element, name }) => {
            let access =
                access_member(&element.upgrade().unwrap(), name.as_str(), component, "self");
            format!(r#"{}.emit()"#, access)
        }
        RepeaterIndexReference { element } => {
            if element.upgrade().unwrap().borrow().base_type == Type::Component(component.clone()) {
                "self->index.get()".to_owned()
            } else {
                todo!();
            }
        }
        RepeaterModelReference { element } => {
            if element.upgrade().unwrap().borrow().base_type == Type::Component(component.clone()) {
                "self->model_data.get()".to_owned()
            } else {
                todo!();
            }
        }
        ObjectAccess { base, name } => {
            let index = if let Type::Object(ty) = base.ty() {
                ty.keys()
                    .position(|k| k == name)
                    .expect("Expression::ObjectAccess: Cannot find a key in an object")
            } else {
                panic!("Expression::ObjectAccess's base expression is not an Object type")
            };
            format!("std::get<{}>({})", index, compile_expression(base, component))
        }
        Cast { from, to } => {
            let f = compile_expression(&*from, component);
            match (from.ty(), to) {
                (Type::Float32, Type::String) | (Type::Int32, Type::String) => {
                    format!("sixtyfps::SharedString::from_number({})", f)
                }
                (Type::Float32, Type::Model) | (Type::Int32, Type::Model) => {
                    format!("std::make_shared<sixtyfps::IntModel>({})", f)
                }
                (Type::Array(_), Type::Model) => f,
                (Type::Float32, Type::Color) => format!("sixtyfps::Color({})", f),
                (Type::LogicalLength, Type::Length) => {
                    format!("({} * {})", f, dpi_expression(component))
                }
                (Type::Length, Type::LogicalLength) => {
                    format!("({} / {})", f, dpi_expression(component))
                }
                _ => f,
            }
        }
        CodeBlock(sub) => {
            let mut x = sub.iter().map(|e| compile_expression(e, component)).collect::<Vec<_>>();
            x.last_mut().map(|s| *s = format!("return {};", s));

            format!("[&]{{ {} }}()", x.join(";"))
        }
        FunctionCall { function } => {
            if matches!(function.ty(), Type::Signal) {
                compile_expression(&*function, component)
            } else {
                format!("\n#error the function `{:?}` is not a signal\n", function)
            }
        }
        SelfAssignment { lhs, rhs, op } => match &**lhs {
            PropertyReference(NamedReference { element, name }) => {
                let access =
                    access_member(&element.upgrade().unwrap(), name.as_str(), component, "self");
                format!(
                    r#"{lhs}.set({lhs}.get() {op} {rhs})"#,
                    lhs = access,
                    rhs = compile_expression(&*rhs, component),
                    op = op,
                )
            }
            _ => panic!("typechecking should make sure this was a PropertyReference"),
        },
        BinaryExpression { lhs, rhs, op } => {
            let mut buffer = [0; 3];
            format!(
                "({lhs} {op} {rhs})",
                lhs = compile_expression(&*lhs, component),
                rhs = compile_expression(&*rhs, component),
                op = match op {
                    '=' => "==",
                    '!' => "!=",
                    '≤' => "<=",
                    '≥' => ">=",
                    '&' => "&&",
                    '|' => "||",
                    _ => op.encode_utf8(&mut buffer),
                },
            )
        }
        UnaryOp { sub, op } => {
            format!("({op} {sub})", sub = compile_expression(&*sub, component), op = op,)
        }
        ResourceReference { absolute_source_path } => {
            format!(r#"sixtyfps::Resource(sixtyfps::SharedString("{}"))"#, absolute_source_path)
        }
        Condition { condition, true_expr, false_expr } => {
            let cond_code = compile_expression(condition, component);
            let true_code = compile_expression(true_expr, component);
            let false_code = compile_expression(false_expr, component);
            format!(
                r#"[&]() -> {} {{ if ({}) {{ return {}; }} else {{ return {}; }}}}()"#,
                e.ty().cpp_type().unwrap(),
                cond_code,
                true_code,
                false_code
            )
        }
        Array { element_ty, values } => {
            let ty = element_ty.cpp_type().unwrap_or_else(|| "FIXME: report error".to_owned());
            format!(
                "std::make_shared<sixtyfps::ArrayModel<{count},{ty}>>({val})",
                count = values.len(),
                ty = ty,
                val = values
                    .iter()
                    .map(|e| format!(
                        "{ty} ( {expr} )",
                        expr = compile_expression(e, component),
                        ty = ty,
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Object { ty, values } => {
            if let Type::Object(ty) = ty {
                let elem = ty
                    .keys()
                    .map(|k| {
                        values
                            .get(k)
                            .map(|e| compile_expression(e, component))
                            .unwrap_or_else(|| "(Error: missing member in object)".to_owned())
                    })
                    .collect::<Vec<String>>();
                format!("std::make_tuple({})", elem.join(", "))
            } else {
                panic!("Expression::Object is not a Type::Object")
            }
        }
        PathElements { elements } => compile_path(elements, component),
        Uncompiled(_) => panic!(),
        Invalid => format!("\n#error invalid expression\n"),
    }
}

fn compute_layout(component: &Rc<Component>) -> Vec<String> {
    let mut res = vec![];

    res.push(format!(
        "[[maybe_unused]] auto self = reinterpret_cast<const {ty}*>(component.instance);",
        ty = component_id(component)
    ));
    for grid in component.layout_constraints.borrow().grids.iter() {
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
                        if cell.borrow().lookup_property(n) == Type::Length {
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
        res.push(format!("    auto x = {};", compile_expression(&grid.x_reference, component)));
        res.push(format!("    auto y = {};", compile_expression(&grid.y_reference, component)));
        res.push("    sixtyfps::GridLayoutData grid { ".into());
        // FIXME: add auto conversion from std::array* to Slice
        res.push("        { row_constr.data(), row_constr.size() },".to_owned());
        res.push("        { col_constr.data(), col_constr.size() },".to_owned());
        res.push(format!("        self->{}.width.get(),", grid.within.borrow().id));
        res.push(format!("        self->{}.height.get(),", grid.within.borrow().id));
        res.push("        x, y,".to_owned());
        res.push("        {cells, std::size(cells)}".to_owned());
        res.push("    };".to_owned());
        res.push("    sixtyfps::solve_grid_layout(&grid);".to_owned());
        res.push("}".to_owned());
    }

    for path_layout in component.layout_constraints.borrow().paths.iter() {
        res.push("{".to_owned());

        let path_layout_item_data = |elem: &ElementRc, elem_cpp: &str, component_cpp: &str| {
            let prop_ref = |n: &str| {
                if elem.borrow().lookup_property(n) == Type::Length {
                    format!("&{}.{}", elem_cpp, n)
                } else {
                    "nullptr".to_owned()
                }
            };
            let prop_value = |n: &str| {
                if elem.borrow().lookup_property(n) == Type::Length {
                    let value_accessor = access_member(
                        &elem,
                        n,
                        &elem.borrow().enclosing_component.upgrade().unwrap(),
                        component_cpp,
                    );
                    format!("{}.get()", value_accessor)
                } else {
                    "0.".into()
                }
            };
            format!(
                "{{ {}, {}, {}, {} }}",
                prop_ref("x"),
                prop_ref("y"),
                prop_value("width"),
                prop_value("height")
            )
        };
        let path_layout_item_data_for_elem = |elem: &ElementRc| {
            path_layout_item_data(elem, &format!("self->{}", elem.borrow().id), "self")
        };

        let is_static_array =
            path_layout.elements.iter().all(|elem| elem.borrow().repeated.is_none());

        let slice = if is_static_array {
            res.push("    sixtyfps::PathLayoutItemData items[] = {".to_owned());
            for elem in &path_layout.elements {
                res.push(format!("        {},", path_layout_item_data_for_elem(elem)));
            }
            res.push("    };".to_owned());
            "        {items, std::size(items)},".to_owned()
        } else {
            res.push("    std::vector<sixtyfps::PathLayoutItemData> items;".to_owned());
            for elem in &path_layout.elements {
                if elem.borrow().repeated.is_some() {
                    let root_element = elem.borrow().base_type.as_component().root_element.clone();
                    res.push(format!(
                        "    for (auto &&sub_comp : self->repeater_{}.data)",
                        elem.borrow().id
                    ));
                    res.push(format!(
                        "         items.push_back({});",
                        path_layout_item_data(
                            &root_element,
                            &format!("sub_comp->{}", root_element.borrow().id),
                            "sub_comp",
                        )
                    ));
                } else {
                    res.push(format!(
                        "     items.push_back({});",
                        path_layout_item_data_for_elem(elem)
                    ));
                }
            }
            "        {items.data(), std::size(items)},".to_owned()
        };

        res.push(format!("    auto path = {};", compile_path(&path_layout.path, component)));

        res.push(format!(
            "    auto x = {};",
            compile_expression(&path_layout.x_reference, component)
        ));
        res.push(format!(
            "    auto y = {};",
            compile_expression(&path_layout.y_reference, component)
        ));
        res.push(format!(
            "    auto width = {};",
            compile_expression(&path_layout.width_reference, component)
        ));
        res.push(format!(
            "    auto height = {};",
            compile_expression(&path_layout.height_reference, component)
        ));

        res.push(format!(
            "    auto offset = {};",
            compile_expression(&path_layout.offset_reference, component)
        ));

        res.push("    sixtyfps::PathLayoutData pl { ".into());
        res.push("        &path,".to_owned());
        res.push(slice);
        res.push("        x, y, width, height, offset".to_owned());
        res.push("    };".to_owned());
        res.push("    sixtyfps::solve_path_layout(&pl);".to_owned());
        res.push("}".to_owned());
    }

    res
}

fn compile_path(path: &crate::expression_tree::Path, component: &Rc<Component>) -> String {
    match path {
        crate::expression_tree::Path::Elements(elements) => {
            let converted_elements: Vec<String> = elements
                .iter()
                .map(|element| {
                    let element_initializer = element
                        .element_type
                        .cpp_type
                        .as_ref()
                        .map(|cpp_type| {
                            new_struct_with_bindings(&cpp_type, &element.bindings, component)
                        })
                        .unwrap_or_default();
                    format!(
                        "sixtyfps::PathElement::{}({})",
                        element.element_type.class_name, element_initializer
                    )
                })
                .collect();
            format!(
                r#"[&](){{
                sixtyfps::PathElement elements[{}] = {{
                    {}
                }};
                return sixtyfps::PathData(&elements[0], std::size(elements));
            }}()"#,
                converted_elements.len(),
                converted_elements.join(",")
            )
        }
        crate::expression_tree::Path::Events(events) => {
            let (converted_events, converted_coordinates) = compile_path_events(events);
            format!(
                r#"[&](){{
                sixtyfps::PathEvent events[{}] = {{
                    {}
                }};
                sixtyfps::Point coordinates[{}] = {{
                    {}
                }};
                return sixtyfps::PathData(&events[0], std::size(events), &coordinates[0], std::size(coordinates));
            }}()"#,
                converted_events.len(),
                converted_events.join(","),
                converted_coordinates.len(),
                converted_coordinates.join(",")
            )
        }
    }
}

fn compile_path_events(events: &crate::expression_tree::PathEvents) -> (Vec<String>, Vec<String>) {
    use lyon::path::Event;

    let mut coordinates = Vec::new();

    let events = events
        .iter()
        .map(|event| match event {
            Event::Begin { at } => {
                coordinates.push(at);
                "sixtyfps::PathEvent::Begin"
            }
            Event::Line { from, to } => {
                coordinates.push(from);
                coordinates.push(to);
                "sixtyfps::PathEvent::Line"
            }
            Event::Quadratic { from, ctrl, to } => {
                coordinates.push(from);
                coordinates.push(ctrl);
                coordinates.push(to);
                "sixtyfps::PathEvent::Quadratic"
            }
            Event::Cubic { from, ctrl1, ctrl2, to } => {
                coordinates.push(from);
                coordinates.push(ctrl1);
                coordinates.push(ctrl2);
                coordinates.push(to);
                "sixtyfps::PathEvent::Cubic"
            }
            Event::End { last, first, close } => {
                debug_assert_eq!(coordinates.first(), Some(&first));
                debug_assert_eq!(coordinates.last(), Some(&last));
                if *close {
                    "sixtyfps::PathEvent::EndClosed"
                } else {
                    "sixtyfps::PathEvent::EndOpen"
                }
            }
        })
        .map(String::from)
        .collect();

    let coordinates = coordinates
        .into_iter()
        .map(|pt| format!("sixtyfps::Point{{{}, {}}}", pt.x, pt.y))
        .collect();

    (events, coordinates)
}
