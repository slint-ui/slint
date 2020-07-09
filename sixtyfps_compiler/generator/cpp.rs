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
        fn cpp_type(&self) -> Option<String>;
    }
}

use crate::diagnostics::{CompilerDiagnostic, Diagnostics};
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
                    []([[maybe_unused]] const sixtyfps::EvaluationContext *context) {{
                        [[maybe_unused]] auto self = reinterpret_cast<const {ty}*>(context->component.instance);
                        {code};
                    }});",
                signal_accessor_prefix = signal_accessor_prefix,
                prop = s,
                ty = main_struct.name,
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
                let setter = property_set_value_code(&component, item, s,  init);
                format!(
                    "{accessor_prefix}{cpp_prop}.{setter};",
                    accessor_prefix = accessor_prefix,
                    cpp_prop = s,
                    setter = setter
                )
            } else {
                let binding_code = format!(
                    "[]([[maybe_unused]] const sixtyfps::EvaluationContext *context) {{
                            [[maybe_unused]] auto self = reinterpret_cast<const {ty}*>(context->component.instance);
                            return {init};
                        }}",
                    ty = main_struct.name,
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
    let repeater_id = format!("repeater_{}", repeater_count);

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
            "\n        case {i}: self->repeater_{i}.visit(visitor); break;",
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
                        self->repeater_{i}.update_model({model}, self);
                    }});
                }}
                self->repeater_{i}.visit(visitor);
                break;
            }}",
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
    diag: &mut Diagnostics,
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

fn generate_component(file: &mut File, component: &Rc<Component>, diag: &mut Diagnostics) {
    let component_id = component_id(component);
    let mut component_struct = Struct { name: component_id.clone(), ..Default::default() };

    let is_root = component.parent_element.upgrade().is_none();

    for (cpp_name, property_decl) in component.root_element.borrow().property_declarations.iter() {
        let ty = if property_decl.property_type == Type::Signal {
            if property_decl.expose_in_public_api && is_root {
                let signal_emitter: Vec<String> = vec![
                    "[[maybe_unused]] auto context = sixtyfps::evaluation_context_for_root_component(this);".into(),
                    format!("{}.emit(&context);", cpp_name)
                    ];

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
                    span: property_decl.type_location.clone(),
                };

                diag.push_compiler_error(err);
                "".into()
            });

            if property_decl.expose_in_public_api && is_root {
                let prop_getter: Vec<String> = vec![
                    "[[maybe_unused]] auto context = sixtyfps::evaluation_context_for_root_component(this);".into(),
                    format!("return {}.get(&context);", cpp_name)
                ];

                component_struct.members.push(Declaration::Function(Function {
                    name: format!("get_{}", cpp_name),
                    signature: format!("() -> {}", cpp_type),
                    statements: Some(prop_getter),
                    ..Default::default()
                }));

                let prop_setter: Vec<String> = vec![format!("{}.set(value);", cpp_name)];

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
                diag.push_compiler_error(CompilerDiagnostic {
                    message: "Cannot map property type to C++".into(),
                    span: parent_element
                        .borrow()
                        .node
                        .as_ref()
                        .map(|n| n.span())
                        .unwrap_or_default(),
                });
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
    }

    let mut init = vec!["[[maybe_unused]] auto self = this;".into()];
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
            let base_component = match &item.base_type {
                Type::Component(c) => c,
                _ => panic!("should be a component because of the repeater_component pass"),
            };
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
        signature:
            "(sixtyfps::ComponentRef component, [[maybe_unused]] const sixtyfps::EvaluationContext *context) -> void"
                .into(),
        is_static: true,
        statements: Some(compute_layout(component)),
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
            "auto context_ = sixtyfps::evaluation_context_for_root_component(self);".into(),
            "[[maybe_unused]] auto context = &context_;".into(),
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
    file.declarations = declarations;
}

fn component_id(component: &Rc<Component>) -> String {
    if component.id.is_empty() {
        format!("Component_{}", component.root_element.borrow().id)
    } else {
        component.id.clone()
    }
}

/// Returns the code that can access the given property or signal from the context (but without the set or get)
///
/// to be used like:
/// ```ignore
/// let (access, context) = access_member(...)
/// format!("context.{access}.get({context})", ...)
/// ```
fn access_member(
    element: &ElementRc,
    name: &str,
    component: &Rc<Component>,
    context: &str,
    component_cpp: &str,
) -> (String, String) {
    let e = element.borrow();
    let enclosing_component = e.enclosing_component.upgrade().unwrap();
    if Rc::ptr_eq(component, &enclosing_component) {
        let e = element.borrow();
        if e.property_declarations.contains_key(name) {
            (format!("{}->{}", component_cpp, name), context.into())
        } else {
            (format!("{}->{}.{}", component_cpp, e.id.as_str(), name), context.into())
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
            &format!("{}->parent_context", context),
            &format!("{}->parent", component_cpp),
        )
    }
}

fn compile_expression(e: &crate::expression_tree::Expression, component: &Rc<Component>) -> String {
    use crate::expression_tree::Expression::*;
    use crate::expression_tree::NamedReference;
    match e {
        StringLiteral(s) => format!(r#"sixtyfps::SharedString("{}")"#, s.escape_default()),
        NumberLiteral(n) => n.to_string(),
        BoolLiteral(b) => b.to_string(),
        PropertyReference(NamedReference { element, name }) => {
            let (access, context) = access_member(
                &element.upgrade().unwrap(),
                name.as_str(),
                component,
                "context",
                "self",
            );
            format!(r#"{}.get({})"#, access, context)
        }
        SignalReference(NamedReference { element, name }) => {
            let (access, context) = access_member(
                &element.upgrade().unwrap(),
                name.as_str(),
                component,
                "context",
                "self",
            );
            format!(r#"{}.emit({})"#, access, context)
        }
        RepeaterIndexReference { element } => {
            if element.upgrade().unwrap().borrow().base_type == Type::Component(component.clone()) {
                "self->index.get(context)".to_owned()
            } else {
                todo!();
            }
        }
        RepeaterModelReference { element } => {
            if element.upgrade().unwrap().borrow().base_type == Type::Component(component.clone()) {
                "self->model_data.get(context)".to_owned()
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
                let (access, context) = access_member(
                    &element.upgrade().unwrap(),
                    name.as_str(),
                    component,
                    "context",
                    "self",
                );
                format!(
                    r#"{lhs}.set({lhs}.get({context}) {op} {rhs})"#,
                    lhs = access,
                    rhs = compile_expression(&*rhs, component),
                    op = op,
                    context = context
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
        PathElements { elements } => match elements {
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
                    return sixtyfps::PathElements(&elements[0], sizeof(elements) / sizeof(elements[0]));
                }}()"#,
                    converted_elements.len(),
                    converted_elements.join(",")
                )
            }
            crate::expression_tree::Path::Events(events) => {
                let converted_elements: Vec<String> = compile_path_events(events);
                format!(
                    r#"[&](){{
                    sixtyfps::PathEvent events[{}] = {{
                        {}
                    }};
                    return sixtyfps::PathElements(&events[0], sizeof(events) / sizeof(events[0]));
                }}()"#,
                    converted_elements.len(),
                    converted_elements.join(",")
                )
            }
        },
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
        res.push(format!("        self->{}.width.get(context),", grid.within.borrow().id));
        res.push(format!("        self->{}.height.get(context),", grid.within.borrow().id));
        res.push("        0., 0.,".to_owned());
        res.push("        {cells, std::size(cells)}".to_owned());
        res.push("    };".to_owned());
        res.push("    sixtyfps::solve_grid_layout(&grid);".to_owned());
        res.push("}".to_owned());
    }
    res
}

fn new_path_event(event_name: &str, fields: &[(&str, String)]) -> String {
    let fields_initialization: Vec<String> = fields
        .iter()
        .map(|(prop, initializer)| format!("var.{} = {};", prop, initializer))
        .collect();

    format!(
        r#"[&](){{
            {} var{{}};
            {}
            return var;
        }}()"#,
        event_name,
        fields_initialization.join("\n")
    )
}

fn compile_path_events(events: &crate::expression_tree::PathEvents) -> Vec<String> {
    use lyon::path::Event;

    events
        .iter()
        .map(|event| match event {
            Event::Begin { at } => format!(
                "sixtyfps::PathEvent::Begin({})",
                new_path_event(
                    "sixtyfps::PathEventBegin",
                    &[("x", at.x.to_string()), ("y", at.y.to_string())]
                )
            ),
            Event::Line { from, to } => format!(
                "sixtyfps::PathEvent::Line({})",
                new_path_event(
                    "sixtyfps::PathEventLine",
                    &[
                        ("from_x", from.x.to_string()),
                        ("from_y", from.y.to_string()),
                        ("to_x", to.x.to_string()),
                        ("to_y", to.y.to_string())
                    ]
                )
            ),
            Event::Quadratic { from, ctrl, to } => format!(
                "sixtyfps::PathEvent::Quadratic({})",
                new_path_event(
                    "sixtyfps::PathEventQuadratic",
                    &[
                        ("from_x", from.x.to_string()),
                        ("from_y", from.y.to_string()),
                        ("control_x", ctrl.x.to_string()),
                        ("control_y", ctrl.y.to_string()),
                        ("to_x", to.x.to_string()),
                        ("to_y", to.y.to_string())
                    ]
                )
            ),
            Event::Cubic { from, ctrl1, ctrl2, to } => format!(
                "sixtyfps::PathEvent::Cubic({})",
                new_path_event(
                    "sixtyfps::PathEventCubic",
                    &[
                        ("from_x", from.x.to_string()),
                        ("from_y", from.y.to_string()),
                        ("control1_x", ctrl1.x.to_string()),
                        ("control1_y", ctrl1.y.to_string()),
                        ("control2_x", ctrl2.x.to_string()),
                        ("control2_y", ctrl2.y.to_string()),
                        ("to_x", to.x.to_string()),
                        ("to_y", to.y.to_string())
                    ]
                )
            ),
            Event::End { last, first, close } => format!(
                "sixtyfps::PathEvent::End({})",
                new_path_event(
                    "sixtyfps::PathEventEnd",
                    &[
                        ("first_x", first.x.to_string()),
                        ("first_y", first.y.to_string()),
                        ("last_x", last.x.to_string()),
                        ("last_y", last.y.to_string()),
                        ("close", close.to_string())
                    ]
                )
            ),
        })
        .collect()
}
