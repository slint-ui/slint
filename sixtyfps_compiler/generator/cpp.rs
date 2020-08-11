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

    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub enum Access {
        Public,
        Private,
        /*Protected,*/
    }

    #[derive(Default, Debug)]
    pub struct Struct {
        pub name: String,
        pub members: Vec<(Access, Declaration)>,
        pub friends: Vec<String>,
    }

    impl Display for Struct {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            indent(f)?;
            writeln!(f, "class {} {{", self.name)?;
            INDETATION.with(|x| x.set(x.get() + 1));
            let mut access = Access::Private;
            for m in &self.members {
                if m.0 != access {
                    access = m.0;
                    indent(f)?;
                    match access {
                        Access::Public => writeln!(f, "public:")?,
                        Access::Private => writeln!(f, "private:")?,
                    }
                }
                write!(f, "{}", m.1)?;
            }
            for friend in &self.friends {
                indent(f)?;
                writeln!(f, "friend class {};", friend)?;
            }
            INDETATION.with(|x| x.set(x.get() - 1));
            indent(f)?;
            writeln!(f, "}};")
        }
    }

    impl Struct {
        pub fn extract_definitions(&mut self) -> impl Iterator<Item = Declaration> + '_ {
            let struct_name = self.name.clone();
            self.members.iter_mut().filter_map(move |x| match &mut x.1 {
                Declaration::Function(f) if f.statements.is_some() => {
                    Some(Declaration::Function(Function {
                        name: format!("{}::{}", struct_name, f.name),
                        signature: f.signature.clone(),
                        is_constructor: f.is_constructor,
                        is_static: false,
                        statements: f.statements.take(),
                        template_parameters: f.template_parameters.clone(),
                    }))
                }
                _ => None,
            })
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
        /// What's inside template<...> if any
        pub template_parameters: Option<String>,
    }

    impl Display for Function {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            indent(f)?;
            if let Some(tpl) = &self.template_parameters {
                write!(f, "template<{}> ", tpl)?;
            }
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

use crate::diagnostics::{BuildDiagnostics, CompilerDiagnostic, Spanned};
use crate::expression_tree::{BuiltinFunction, EasingCurve, Expression, ExpressionSpanned};
use crate::layout::{GridLayout, Layout, LayoutItem, PathLayout};
use crate::object_tree::{Component, Element, ElementRc, RepeatedElementInfo};
use crate::typeregister::Type;
use cpp_ast::*;
use std::collections::HashMap;
use std::rc::Rc;

impl CppType for Type {
    fn cpp_type(&self) -> Option<String> {
        match self {
            Type::Void => Some("void".to_owned()),
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
            Type::Resource => Some("sixtyfps::Resource".to_owned()),
            Type::Builtin(elem) => elem.native_class.cpp_type.clone(),
            Type::Enumeration(enumeration) => Some(format!("sixtyfps::{}", enumeration.name)),
            _ => None,
        }
    }
}

fn new_struct_with_bindings(
    type_name: &str,
    bindings: &HashMap<String, ExpressionSpanned>,
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
    value_expr: &str,
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
    main_struct.members.push((
        Access::Private,
        Declaration::Var(Var {
            ty: format!("sixtyfps::{}", item.base_type.as_native().class_name),
            name: item.id.clone(),
            ..Default::default()
        }),
    ));

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
                format!(
                    "{accessor_prefix}{cpp_prop}.set({init});",
                    accessor_prefix = accessor_prefix,
                    cpp_prop = s,
                    init = init
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
    children_visitor_cases: &mut Vec<String>,
    repeated_input_branch: &mut Vec<String>,
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
        children_visitor_cases.push(format!(
            "\n        case {i}: return self->{id}.visit(order, visitor);",
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
        component_struct.members.push((
            Access::Private,
            Declaration::Var(Var {
                ty: "sixtyfps::PropertyListenerScope".to_owned(),
                name: model_id,
                init: None,
            }),
        ));
        children_visitor_cases.push(format!(
            "\n        case {i}: {{
                if (self->model_{i}.is_dirty()) {{
                    self->model_{i}.evaluate([&] {{
                        self->{id}.update_model({model}, self);
                    }});
                }}
                self->{id}.visit(order, visitor);
                break;
            }}",
            id = repeater_id,
            i = repeater_count,
            model = model,
        ));
    }

    repeated_input_branch.push(format!(
        "\n        case {i}: return self->{id}.item_at(rep_index);",
        i = repeater_count,
        id = repeater_id,
    ));

    component_struct.members.push((
        Access::Private,
        Declaration::Var(Var {
            ty: format!("sixtyfps::Repeater<class {}>", component_id(base_component)),
            name: repeater_id,
            init: None,
        }),
    ));
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

    generate_component(&mut file, component, diag, None);

    file.declarations.push(Declaration::Var(Var{
        ty: format!(
            "constexpr sixtyfps::VersionCheckHelper<{}, {}, {}>",
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH")),
        name: "THE_SAME_VERSION_MUST_BE_USED_FOR_THE_COMPILER_AND_THE_RUNTIME".into(),
        init: Some("sixtyfps::VersionCheckHelper<int(sixtyfps::VersionCheck::Major), int(sixtyfps::VersionCheck::Minor), int(sixtyfps::VersionCheck::Patch)>()".into())
    }));

    if diag.has_error() {
        None
    } else {
        Some(file)
    }
}

/// Generate the component in `file`.
///
/// `sub_components`, if Some, will be filled with all the sub component which needs to be added as friends
fn generate_component(
    file: &mut File,
    component: &Rc<Component>,
    diag: &mut BuildDiagnostics,
    mut sub_components: Option<&mut Vec<String>>,
) {
    let component_id = component_id(component);
    let mut component_struct = Struct { name: component_id.clone(), ..Default::default() };

    let is_root = component.parent_element.upgrade().is_none();
    let mut init = vec!["[[maybe_unused]] auto self = this;".into()];

    for (cpp_name, property_decl) in component.root_element.borrow().property_declarations.iter() {
        let ty = if property_decl.property_type == Type::Signal {
            if property_decl.expose_in_public_api && is_root {
                let signal_emitter = vec![format!("{}.emit();", cpp_name)];
                component_struct.members.push((
                    Access::Public,
                    Declaration::Function(Function {
                        name: format!("emit_{}", cpp_name),
                        signature: "()".into(),
                        statements: Some(signal_emitter),
                        ..Default::default()
                    }),
                ));
                component_struct.members.push((
                    Access::Public,
                    Declaration::Function(Function {
                        name: format!("on_{}", cpp_name),
                        template_parameters: Some("typename Functor".into()),
                        signature: "(Functor && signal_handler)".into(),
                        statements: Some(vec![format!(
                            "{}.set_handler(std::forward<Functor>(signal_handler));",
                            cpp_name
                        )]),
                        ..Default::default()
                    }),
                ));
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

                component_struct.members.push((
                    Access::Public,
                    Declaration::Function(Function {
                        name: format!("get_{}", cpp_name),
                        signature: format!("() -> {}", cpp_type),
                        statements: Some(prop_getter),
                        ..Default::default()
                    }),
                ));

                let prop_setter: Vec<String> = vec![format!(
                    "this->{}.{};",
                    cpp_name,
                    property_set_value_code(
                        &component,
                        &*component.root_element.borrow(),
                        cpp_name,
                        "value"
                    )
                )];
                component_struct.members.push((
                    Access::Public,
                    Declaration::Function(Function {
                        name: format!("set_{}", cpp_name),
                        signature: format!("(const {} &value)", cpp_type),
                        statements: Some(prop_setter),
                        ..Default::default()
                    }),
                ));
            }

            format!("sixtyfps::Property<{}>", cpp_type)
        };
        component_struct.members.push((
            Access::Private,
            Declaration::Var(Var { ty, name: cpp_name.clone(), init: None }),
        ));
    }

    if !is_root {
        let parent_element = component.parent_element.upgrade().unwrap();

        let mut update_statements = vec![];

        if !parent_element.borrow().repeated.as_ref().map_or(false, |r| r.is_conditional_element) {
            component_struct.members.push((
                Access::Private,
                Declaration::Var(Var {
                    ty: "sixtyfps::Property<int>".into(),
                    name: "index".into(),
                    init: None,
                }),
            ));

            let model_data_type = crate::expression_tree::Expression::RepeaterModelReference {
                element: component.parent_element.clone(),
            }
            .ty();
            let cpp_model_data_type = model_data_type
                .cpp_type()
                .unwrap_or_else(|| {
                    diag.push_internal_error(
                        CompilerDiagnostic {
                            message: format!("Cannot map property type {} to C++", model_data_type)
                                .into(),
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
            component_struct.members.push((
                Access::Private,
                Declaration::Var(Var {
                    ty: format!("sixtyfps::Property<{}>", cpp_model_data_type),
                    name: "model_data".into(),
                    init: None,
                }),
            ));

            update_statements = vec![
                "index.set(i);".into(),
                format!("model_data.set(*reinterpret_cast<{} const*>(data));", cpp_model_data_type),
            ];
        }
        let parent_component_id = self::component_id(
            &component
                .parent_element
                .upgrade()
                .unwrap()
                .borrow()
                .enclosing_component
                .upgrade()
                .unwrap(),
        );
        component_struct.members.push((
            Access::Public, // Because Repeater::update_model accesses it
            Declaration::Var(Var {
                ty: format!("{} const *", parent_component_id),
                name: "parent".into(),
                init: Some("nullptr".to_owned()),
            }),
        ));
        component_struct.friends.push(parent_component_id);
        component_struct.members.push((
            Access::Public, // Because Repeater::update_model accesses it
            Declaration::Function(Function {
                name: "update_data".into(),
                signature: "([[maybe_unused]] int i, [[maybe_unused]] const void *data) -> void"
                    .into(),
                statements: Some(update_statements),
                ..Function::default()
            }),
        ));
    } else {
        component_struct.members.push((
            Access::Public, // FIXME: many of the different component bindings need to access this
            Declaration::Var(Var {
                ty: "sixtyfps::Property<float>".into(),
                name: "scale_factor".into(),
                ..Var::default()
            }),
        ));
        init.push("self->scale_factor.set(1.);".to_owned());

        component_struct.members.push((
            Access::Public, // FIXME: many of the different component bindings need to access this
            Declaration::Var(Var {
                ty: "sixtyfps::ComponentWindow".into(),
                name: "window".into(),
                ..Var::default()
            }),
        ));

        let window_props = |name| {
            let root_elem = component.root_element.borrow();

            if root_elem.lookup_property(name) == Type::Length {
                format!("&this->{}.{}", root_elem.id, name)
            } else {
                "nullptr".to_owned()
            }
        };
        component_struct.members.push((
            Access::Public,
            Declaration::Function(Function {
                name: "window_properties".into(),
                signature: "() -> sixtyfps::WindowProperties".into(),
                statements: Some(vec![format!(
                    "return {{ {} , {}, &this->scale_factor }};",
                    window_props("width"),
                    window_props("height")
                )]),
                ..Default::default()
            }),
        ));

        component_struct.members.push((
            Access::Public,
            Declaration::Function(Function {
                name: "run".into(),
                signature: "()".into(),
                statements: Some(vec!["window.run(this);".into()]),
                ..Default::default()
            }),
        ))
    }

    let mut children_visitor_cases = vec![];
    let mut repeated_input_branch = vec![];
    let mut tree_array = vec![];
    let mut repeater_count = 0;
    super::build_array_helper(component, |item_rc, children_offset, is_flickable_rect| {
        let item = item_rc.borrow();
        if is_flickable_rect {
            tree_array.push(format!(
                "sixtyfps::make_item_node(offsetof({}, {}) + offsetof(sixtyfps::Flickable, viewport), &sixtyfps::RectangleVTable, {}, {})",
                &component_id,
                item.id,
                item.children.len(),
                tree_array.len() + 1,
            ));
        } else if let Some(repeated) = &item.repeated {
            tree_array.push(format!("sixtyfps::make_dyn_node({})", repeater_count,));
            let base_component = item.base_type.as_component();
            let mut friends = Vec::new();
            generate_component(file, base_component, diag, Some(&mut friends));
            if let Some(sub_components) = sub_components.as_mut() {
                sub_components.extend_from_slice(friends.as_slice());
                sub_components.push(self::component_id(base_component))
            }
            component_struct.friends.append(&mut friends);
            component_struct.friends.push(self::component_id(base_component));
            handle_repeater(
                repeated,
                base_component,
                component,
                repeater_count,
                &mut component_struct,
                &mut init,
                &mut children_visitor_cases,
                &mut repeated_input_branch,
            );
            repeater_count += 1;
        } else {
            tree_array.push(format!(
                "sixtyfps::make_item_node(offsetof({}, {}), &sixtyfps::{}, {}, {})",
                &component_id,
                item.id,
                item.base_type.as_native().vtable_symbol,
                if super::is_flickable(item_rc) { 1 } else { item.children.len() },
                children_offset,
            ));
            handle_item(&*item, &mut component_struct, &mut init);
        }
    });

    component_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: component_id.clone(),
            signature: "()".to_owned(),
            is_constructor: true,
            statements: Some(init),
            ..Default::default()
        }),
    ));

    component_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "visit_children".into(),
            signature: "(sixtyfps::ComponentRef component, intptr_t index, sixtyfps::TraversalOrder order, sixtyfps::ItemVisitorRefMut visitor) -> int64_t".into(),
            is_static: true,
            statements: Some(vec![
                "static const auto dyn_visit = [] (const uint8_t *base,  [[maybe_unused]] sixtyfps::TraversalOrder order, [[maybe_unused]] sixtyfps::ItemVisitorRefMut visitor, uintptr_t dyn_index) -> int64_t {".to_owned(),
                format!("    [[maybe_unused]] auto self = reinterpret_cast<const {}*>(base);", component_id),
                format!("    switch(dyn_index) {{ {} }};", children_visitor_cases.join("")),
                "    return -1; //should not happen\n};".to_owned(),
                "return sixtyfps::sixtyfps_visit_item_tree(component, item_tree() , index, order, visitor, dyn_visit);".to_owned(),
            ]),
            ..Default::default()
        }),
    ));

    component_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "item_tree".into(),
            signature: "() -> sixtyfps::Slice<sixtyfps::ItemTreeNode>".into(),
            is_static: true,
            statements: Some(vec![
                "static const sixtyfps::ItemTreeNode children[] {".to_owned(),
                format!("    {} }};", tree_array.join(", ")),
                "return { const_cast<sixtyfps::ItemTreeNode*>(children), std::size(children) };"
                    .to_owned(),
            ]),
            ..Default::default()
        }),
    ));

    component_struct.members.push((
        Access::Private,
        Declaration::Var(Var {
            ty: "int64_t".into(),
            name: "mouse_grabber".into(),
            init: Some("-1".into()),
        }),
    ));
    component_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "input_event".into(),
            signature:
                "(sixtyfps::ComponentRef component, sixtyfps::MouseEvent mouse_event) -> sixtyfps::InputEventResult"
                    .into(),
            is_static: true,
            statements: Some(vec![
                format!("    auto self = reinterpret_cast<{}*>(component.instance);", component_id),
                "return sixtyfps::process_input_event(component, self->mouse_grabber, mouse_event, item_tree(), [self](int dyn_index, [[maybe_unused]] int rep_index) {".into(),
                format!("    switch(dyn_index) {{ {} }};", repeated_input_branch.join("")),
                "    return sixtyfps::ComponentRef{nullptr, nullptr};\n});".into(),
            ]),
            ..Default::default()
        }),
    ));

    component_struct.members.push((
        Access::Public, // FIXME: we call this function from tests
        Declaration::Function(Function {
            name: "compute_layout".into(),
            signature: "(sixtyfps::ComponentRef component) -> void".into(),
            is_static: true,
            statements: Some(compute_layout(component)),
            ..Default::default()
        }),
    ));

    component_struct.members.push((
        Access::Public,
        Declaration::Var(Var {
            ty: "static const sixtyfps::ComponentVTable".to_owned(),
            name: "component_type".to_owned(),
            init: None,
        }),
    ));

    let mut definitions = component_struct.extract_definitions().collect::<Vec<_>>();
    let mut declarations = vec![];
    declarations.push(Declaration::Struct(component_struct));

    declarations.push(Declaration::Var(Var {
        ty: "const sixtyfps::ComponentVTable".to_owned(),
        name: format!("{}::component_type", component_id),
        init: Some("{ visit_children, nullptr, compute_layout, input_event }".to_owned()),
    }));

    declarations.append(&mut file.declarations);
    declarations.append(&mut definitions);

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
        if e.property_declarations.contains_key(name) || name == "" {
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

/// Return an expression that gets the window scale factor property
fn window_scale_factor_expression(component: &Rc<Component>) -> String {
    let mut root_component = component.clone();
    let mut component_cpp = "self".to_owned();
    while let Some(p) = root_component.parent_element.upgrade() {
        root_component = p.borrow().enclosing_component.upgrade().unwrap();
        component_cpp = format!("{}->parent", component_cpp);
    }
    format!("{}->scale_factor.get()", component_cpp)
}

fn compile_expression(e: &crate::expression_tree::Expression, component: &Rc<Component>) -> String {
    use crate::expression_tree::NamedReference;
    match e {
        Expression::StringLiteral(s) => {
            format!(r#"sixtyfps::SharedString("{}")"#, s.escape_debug())
        }
        Expression::NumberLiteral(n, unit) => unit.normalize(*n).to_string(),
        Expression::BoolLiteral(b) => b.to_string(),
        Expression::PropertyReference(NamedReference { element, name }) => {
            let access =
                access_member(&element.upgrade().unwrap(), name.as_str(), component, "self");
            format!(r#"{}.get()"#, access)
        }
        Expression::SignalReference(NamedReference { element, name }) => {
            let access =
                access_member(&element.upgrade().unwrap(), name.as_str(), component, "self");
            format!(r#"{}.emit()"#, access)
        }
        Expression::BuiltinFunctionReference(funcref) => match funcref {
            BuiltinFunction::GetWindowScaleFactor => window_scale_factor_expression(component),
        },
        Expression::RepeaterIndexReference { element } => {
            let access = access_member(
                &element.upgrade().unwrap().borrow().base_type.as_component().root_element,
                "",
                component,
                "self",
            );
            format!(r#"{}index.get()"#, access)
        }
        Expression::RepeaterModelReference { element } => {
            let access = access_member(
                &element.upgrade().unwrap().borrow().base_type.as_component().root_element,
                "",
                component,
                "self",
            );
            format!(r#"{}model_data.get()"#, access)
        }
        Expression::StoreLocalVariable { name, value } => {
            format!("auto {} = {};", name, compile_expression(value, component))
        }
        Expression::ReadLocalVariable { name, .. } => name.clone(),
        Expression::ObjectAccess { base, name } => {
            let index = if let Type::Object(ty) = base.ty() {
                ty.keys()
                    .position(|k| k == name)
                    .expect("Expression::ObjectAccess: Cannot find a key in an object")
            } else {
                panic!("Expression::ObjectAccess's base expression is not an Object type")
            };
            format!("std::get<{}>({})", index, compile_expression(base, component))
        }
        Expression::Cast { from, to } => {
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
        Expression::CodeBlock(sub) => {
            let mut x = sub.iter().map(|e| compile_expression(e, component)).collect::<Vec<_>>();
            x.last_mut().map(|s| *s = format!("return {};", s));

            format!("[&]{{ {} }}()", x.join(";"))
        }
        Expression::FunctionCall { function } => {
            if matches!(function.ty(), Type::Signal | Type::Function{..}) {
                compile_expression(&*function, component)
            } else {
                format!("\n#error the function `{:?}` is not a signal\n", function)
            }
        }
        Expression::SelfAssignment { lhs, rhs, op } => match &**lhs {
            Expression::PropertyReference(NamedReference { element, name }) => {
                let access =
                    access_member(&element.upgrade().unwrap(), name.as_str(), component, "self");
                let rhs = compile_expression(&*rhs, component);
                if *op == '=' {
                    format!(r#"{lhs}.set({rhs})"#, lhs = access, rhs = rhs)
                } else {
                    format!(
                        r#"{lhs}.set({lhs}.get() {op} {rhs})"#,
                        lhs = access,
                        rhs = rhs,
                        op = op,
                    )
                }
            }
            _ => panic!("typechecking should make sure this was a PropertyReference"),
        },
        Expression::BinaryExpression { lhs, rhs, op } => {
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
        Expression::UnaryOp { sub, op } => {
            format!("({op} {sub})", sub = compile_expression(&*sub, component), op = op,)
        }
        Expression::ResourceReference { absolute_source_path } => {
            format!(r#"sixtyfps::Resource(sixtyfps::SharedString("{}"))"#, absolute_source_path)
        }
        Expression::Condition { condition, true_expr, false_expr } => {
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
        Expression::Array { element_ty, values } => {
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
        Expression::Object { ty, values } => {
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
        Expression::PathElements { elements } => compile_path(elements, component),
        Expression::EasingCurve(EasingCurve::Linear) => "sixtyfps::EasingCurve()".into(),
        Expression::EasingCurve(EasingCurve::CubicBezier(a, b, c, d)) => format!(
            "sixtyfps::EasingCurve(sixtyfps::EasingCurve::Tag::CubicBezier, {}, {}, {}, {})",
            a, b, c, d
        ),
        Expression::EnumerationValue(value) => {
            format!("sixtyfps::{}::{}", value.enumeration.name, value.to_string())
        }
        Expression::Uncompiled(_) => panic!(),
        Expression::Invalid => format!("\n#error invalid expression\n"),
    }
}

pub struct GridLayoutWithCells<'a> {
    grid: &'a GridLayout,
    var_creation_code: String,
    cell_ref_variable: String,
    spacing: String,
}

#[derive(derive_more::From)]
enum LayoutTreeItem<'a> {
    GridLayout(GridLayoutWithCells<'a>),
    PathLayout(&'a PathLayout),
}

impl<'a> LayoutTreeItem<'a> {
    fn layout_info(&self) -> String {
        match self {
            LayoutTreeItem::GridLayout(grid_layout) => format!(
                "sixtyfps::grid_layout_info(&{}, {})",
                grid_layout.cell_ref_variable, grid_layout.spacing
            ),
            LayoutTreeItem::PathLayout(_) => todo!(),
        }
    }
}

trait LayoutItemCodeGen {
    fn get_property_ref(&self, name: &str) -> String;
    fn get_layout_info_ref<'a, 'b>(
        &'a self,
        layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
        component: &Rc<Component>,
    ) -> String;
}

impl LayoutItemCodeGen for LayoutItem {
    fn get_property_ref(&self, name: &str) -> String {
        match self {
            LayoutItem::Element(e) => e.get_property_ref(name),
            LayoutItem::Layout(l) => l.get_property_ref(name),
        }
    }
    fn get_layout_info_ref<'a, 'b>(
        &'a self,
        layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
        component: &Rc<Component>,
    ) -> String {
        match self {
            LayoutItem::Element(e) => e.get_layout_info_ref(layout_tree, component),
            LayoutItem::Layout(l) => l.get_layout_info_ref(layout_tree, component),
        }
    }
}

impl LayoutItemCodeGen for Layout {
    fn get_property_ref(&self, name: &str) -> String {
        let moved_property_name = match self.rect().mapped_property_name(name) {
            Some(name) => name,
            None => return "nullptr".to_owned(),
        };
        format!("&self->{}", moved_property_name)
    }
    fn get_layout_info_ref<'a, 'b>(
        &'a self,
        layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
        component: &Rc<Component>,
    ) -> String {
        let self_as_layout_tree_item = collect_layouts_recursively(layout_tree, &self, component);
        self_as_layout_tree_item.layout_info()
    }
}

impl LayoutItemCodeGen for ElementRc {
    fn get_property_ref(&self, name: &str) -> String {
        if self.borrow().lookup_property(name) == Type::Length {
            format!("&self->{}.{}", self.borrow().id, name)
        } else {
            "nullptr".to_owned()
        }
    }
    fn get_layout_info_ref<'a, 'b>(
        &'a self,
        _layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
        _component: &Rc<Component>,
    ) -> String {
        format!(
            "sixtyfps::{vt}.layouting_info({{&sixtyfps::{vt}, const_cast<sixtyfps::{ty}*>(&self->{id})}})",
            vt = self.borrow().base_type.as_native().vtable_symbol,
            ty = self.borrow().base_type.as_native().class_name,
            id = self.borrow().id,
        )
    }
}

fn collect_layouts_recursively<'a, 'b>(
    layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
    layout: &'a Layout,
    component: &Rc<Component>,
) -> &'b LayoutTreeItem<'a> {
    match layout {
        Layout::GridLayout(grid_layout) => {
            let mut creation_code = Vec::new();
            for cell in &grid_layout.elems {
                creation_code.push(format!(
                    "        {{ {c}, {r}, {cs}, {rs}, {li}, {x}, {y}, {w}, {h} }},",
                    c = cell.col,
                    r = cell.row,
                    cs = cell.colspan,
                    rs = cell.rowspan,
                    li = cell.item.get_layout_info_ref(layout_tree, component),
                    x = cell.item.get_property_ref("x"),
                    y = cell.item.get_property_ref("y"),
                    w = cell.item.get_property_ref("width"),
                    h = cell.item.get_property_ref("height")
                ));
            }
            let cell_ref_variable = format!("cells_{}", layout_tree.len()).to_owned();
            creation_code.insert(
                0,
                format!("    sixtyfps::GridLayoutCellData {}_data[] = {{", cell_ref_variable,),
            );
            creation_code.push("    };".to_owned());
            creation_code.push(format!(
                "    const sixtyfps::Slice<sixtyfps::GridLayoutCellData> {cv}{{{cv}_data, std::size({cv}_data)}};",
                cv = cell_ref_variable
            ));
            let spacing = if let Some(spacing) = &grid_layout.spacing {
                let variable = format!("spacing_{}", layout_tree.len());
                creation_code.push(format!(
                    "auto {} = {};",
                    variable,
                    compile_expression(spacing, component)
                ));
                variable
            } else {
                "0.".into()
            };

            layout_tree.push(
                GridLayoutWithCells {
                    grid: grid_layout,
                    var_creation_code: creation_code.join("\n"),
                    cell_ref_variable,
                    spacing,
                }
                .into(),
            )
        }
        Layout::PathLayout(path_layout) => layout_tree.push(path_layout.into()),
    }
    layout_tree.last().unwrap()
}

impl<'a> LayoutTreeItem<'a> {
    fn layout_info_collecting_code(&self) -> Option<String> {
        match self {
            LayoutTreeItem::GridLayout(grid_layout) => Some(grid_layout.var_creation_code.clone()),
            LayoutTreeItem::PathLayout(_) => None,
        }
    }

    fn emit_solve_calls(&self, component: &Rc<Component>, code_stream: &mut Vec<String>) {
        match self {
            LayoutTreeItem::GridLayout(grid_layout) => {
                code_stream.push("    { ".into());
                code_stream.push(format!(
                    "    auto width = {};",
                    compile_expression(&grid_layout.grid.rect.width_reference, component)
                ));
                code_stream.push(format!(
                    "    auto height = {};",
                    compile_expression(&grid_layout.grid.rect.height_reference, component)
                ));
                code_stream.push("    sixtyfps::GridLayoutData grid { ".into());
                code_stream.push(format!(
                    "        width, height, {}, {}, {},",
                    compile_expression(&grid_layout.grid.rect.x_reference, component),
                    compile_expression(&grid_layout.grid.rect.y_reference, component),
                    grid_layout.spacing,
                ));
                code_stream
                    .push(format!("        {cv}", cv = grid_layout.cell_ref_variable).to_owned());
                code_stream.push("    };".to_owned());
                code_stream.push("    sixtyfps::solve_grid_layout(&grid);".to_owned());
                code_stream.push("    } ".into());
            }
            LayoutTreeItem::PathLayout(path_layout) => {
                code_stream.push("{".to_owned());

                let path_layout_item_data =
                    |elem: &ElementRc, elem_cpp: &str, component_cpp: &str| {
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
                    code_stream.push("    sixtyfps::PathLayoutItemData items[] = {".to_owned());
                    for elem in &path_layout.elements {
                        code_stream
                            .push(format!("        {},", path_layout_item_data_for_elem(elem)));
                    }
                    code_stream.push("    };".to_owned());
                    "        {items, std::size(items)},".to_owned()
                } else {
                    code_stream
                        .push("    std::vector<sixtyfps::PathLayoutItemData> items;".to_owned());
                    for elem in &path_layout.elements {
                        if elem.borrow().repeated.is_some() {
                            let root_element =
                                elem.borrow().base_type.as_component().root_element.clone();
                            code_stream.push(format!(
                                "    for (auto &&sub_comp : self->repeater_{}.data)",
                                elem.borrow().id
                            ));
                            code_stream.push(format!(
                                "         items.push_back({});",
                                path_layout_item_data(
                                    &root_element,
                                    &format!("sub_comp->{}", root_element.borrow().id),
                                    "sub_comp",
                                )
                            ));
                        } else {
                            code_stream.push(format!(
                                "     items.push_back({});",
                                path_layout_item_data_for_elem(elem)
                            ));
                        }
                    }
                    "        {items.data(), std::size(items)},".to_owned()
                };

                code_stream.push(format!(
                    "    auto path = {};",
                    compile_path(&path_layout.path, component)
                ));

                code_stream.push(format!(
                    "    auto x = {};",
                    compile_expression(&path_layout.rect.x_reference, component)
                ));
                code_stream.push(format!(
                    "    auto y = {};",
                    compile_expression(&path_layout.rect.y_reference, component)
                ));
                code_stream.push(format!(
                    "    auto width = {};",
                    compile_expression(&path_layout.rect.width_reference, component)
                ));
                code_stream.push(format!(
                    "    auto height = {};",
                    compile_expression(&path_layout.rect.height_reference, component)
                ));

                code_stream.push(format!(
                    "    auto offset = {};",
                    compile_expression(&path_layout.offset_reference, component)
                ));

                code_stream.push("    sixtyfps::PathLayoutData pl { ".into());
                code_stream.push("        &path,".to_owned());
                code_stream.push(slice);
                code_stream.push("        x, y, width, height, offset".to_owned());
                code_stream.push("    };".to_owned());
                code_stream.push("    sixtyfps::solve_path_layout(&pl);".to_owned());
                code_stream.push("}".to_owned());
            }
        }
    }
}

fn compute_layout(component: &Rc<Component>) -> Vec<String> {
    let mut res = vec![];

    res.push(format!(
        "[[maybe_unused]] auto self = reinterpret_cast<const {ty}*>(component.instance);",
        ty = component_id(component)
    ));
    component.layout_constraints.borrow().iter().for_each(|layout| {
        let mut inverse_layout_tree = Vec::new();

        collect_layouts_recursively(&mut inverse_layout_tree, layout, component);

        res.extend(
            inverse_layout_tree.iter().filter_map(|layout| layout.layout_info_collecting_code()),
        );

        inverse_layout_tree
            .iter()
            .rev()
            .for_each(|layout| layout.emit_solve_calls(component, &mut res));
    });

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
                        .native_class
                        .cpp_type
                        .as_ref()
                        .map(|cpp_type| {
                            new_struct_with_bindings(&cpp_type, &element.bindings, component)
                        })
                        .unwrap_or_default();
                    format!(
                        "sixtyfps::PathElement::{}({})",
                        element.element_type.native_class.class_name, element_initializer
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
