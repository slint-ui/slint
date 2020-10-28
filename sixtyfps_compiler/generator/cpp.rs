/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
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
        pub definitions: Vec<Declaration>,
    }

    impl Display for File {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            for i in &self.includes {
                writeln!(f, "#include {}", i)?;
            }
            for d in &self.declarations {
                write!(f, "\n{}", d)?;
            }
            for d in &self.definitions {
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
                        is_constructor_or_destructor: f.is_constructor_or_destructor,
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
        pub is_constructor_or_destructor: bool,
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
            if !self.is_constructor_or_destructor {
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

use crate::diagnostics::{BuildDiagnostics, CompilerDiagnostic, Level, Spanned};
use crate::expression_tree::{
    BuiltinFunction, EasingCurve, Expression, ExpressionSpanned, NamedReference,
};
use crate::langtype::Type;
use crate::layout::LayoutGeometry;
use crate::object_tree::{Component, Document, Element, ElementRc, RepeatedElementInfo};
use cpp_ast::*;
use itertools::Itertools;
use std::collections::{BTreeMap, HashMap};
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
            Type::Percent => Some("float".to_owned()),
            Type::Bool => Some("bool".to_owned()),
            Type::Object { fields, name } => {
                if let Some(name) = name {
                    Some(name.clone())
                } else {
                    let elem = fields.values().map(|v| v.cpp_type()).collect::<Option<Vec<_>>>()?;
                    // This will produce a tuple
                    Some(format!("std::tuple<{}>", elem.join(", ")))
                }
            }
            Type::Array(i) => Some(format!("std::shared_ptr<sixtyfps::Model<{}>>", i.cpp_type()?)),
            Type::Resource => Some("sixtyfps::Resource".to_owned()),
            Type::Builtin(elem) => elem.native_class.cpp_type.clone(),
            Type::Enumeration(enumeration) => Some(format!("sixtyfps::{}", enumeration.name)),
            _ => None,
        }
    }
}

fn get_cpp_type(ty: &Type, type_node: &dyn Spanned, diag: &mut BuildDiagnostics) -> String {
    ty.cpp_type().unwrap_or_else(|| {
        let err = CompilerDiagnostic {
            message: "Cannot map property type to C++".into(),
            span: type_node.span(),
            level: Level::Error,
        };
        diag.push_internal_error(err.into());
        "".into()
    })
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
            "sixtyfps::PropertyAnimation",
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

fn handle_property_binding(
    elem: &ElementRc,
    prop_name: &str,
    binding_expression: &Expression,
    init: &mut Vec<String>,
) {
    let item = elem.borrow();
    let component = item.enclosing_component.upgrade().unwrap();
    let id = &item.id;
    let prop_ty = item.lookup_property(prop_name);
    if let Type::Signal { args } = &prop_ty {
        let signal_accessor_prefix = if item.property_declarations.contains_key(prop_name) {
            String::new()
        } else {
            format!("{id}.", id = id.clone())
        };
        let mut params = args.iter().enumerate().map(|(i, ty)| {
            format!("[[maybe_unused]] {} arg_{}", ty.cpp_type().unwrap_or_default(), i)
        });

        init.push(format!(
            "{signal_accessor_prefix}{prop}.set_handler(
                    [this]({params}) {{
                        [[maybe_unused]] auto self = this;
                        {code};
                    }});",
            signal_accessor_prefix = signal_accessor_prefix,
            prop = prop_name,
            params = params.join(", "),
            code = compile_expression(binding_expression, &component)
        ));
    } else if let Expression::TwoWayBinding(nr, next) = &binding_expression {
        init.push(format!(
            "sixtyfps::Property<{ty}>::link_two_way(&{p1}, &{p2});",
            ty = prop_ty.cpp_type().unwrap_or_default(),
            p1 = access_member(elem, prop_name, &component, "this"),
            p2 = access_named_reference(nr, &component, "this")
        ));
        if let Some(next) = next {
            handle_property_binding(elem, prop_name, next, init)
        }
    } else {
        let accessor_prefix = if item.property_declarations.contains_key(prop_name) {
            String::new()
        } else {
            format!("{id}.", id = id.clone())
        };

        let component = &item.enclosing_component.upgrade().unwrap();

        let init_expr = compile_expression(binding_expression, component);
        let setter = if binding_expression.is_constant() {
            format!("set({});", init_expr)
        } else {
            let binding_code = format!(
                "[this]() {{
                            [[maybe_unused]] auto self = this;
                            return {init};
                        }}",
                init = init_expr
            );
            property_set_binding_code(component, &item, prop_name, binding_code)
        };
        if let Some(vp) = super::as_flickable_viewport_property(elem, prop_name) {
            init.push(format!(
                "{accessor_prefix}viewport.{cpp_prop}.{setter};",
                accessor_prefix = accessor_prefix,
                cpp_prop = vp,
                setter = setter,
            ))
        } else {
            init.push(format!(
                "{accessor_prefix}{cpp_prop}.{setter};",
                accessor_prefix = accessor_prefix,
                cpp_prop = prop_name,
                setter = setter,
            ))
        }
    }
}

fn handle_item(elem: &ElementRc, main_struct: &mut Struct, init: &mut Vec<String>) {
    let item = elem.borrow();
    main_struct.members.push((
        Access::Private,
        Declaration::Var(Var {
            ty: format!("sixtyfps::{}", item.base_type.as_native().class_name),
            name: item.id.clone(),
            init: Some("{}".to_owned()),
        }),
    ));

    for (prop_name, binding_expression) in &item.bindings {
        handle_property_binding(elem, prop_name, binding_expression, init);
    }
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
    layout_repeater_code: &mut Vec<String>,
    diag: &mut BuildDiagnostics,
) {
    let parent_element = base_component.parent_element.upgrade().unwrap();
    let repeater_id = format!("repeater_{}", parent_element.borrow().id);

    let mut model = compile_expression(&repeated.model, parent_component);
    if repeated.is_conditional_element {
        // bool converts to int
        // FIXME: don't do a heap allocation here
        model = format!("std::make_shared<sixtyfps::IntModel>({})", model)
    };

    // FIXME: optimize  if repeated.model.is_constant()
    init.push(format!(
        "self->{repeater_id}.set_model_binding([self] {{ return {model}; }});",
        repeater_id = repeater_id,
        model = model,
    ));

    if let Some(listview) = &repeated.is_listview {
        let vp_y = access_named_reference(&listview.viewport_y, parent_component, "self");
        let vp_h = access_named_reference(&listview.viewport_height, parent_component, "self");
        let lv_h = access_named_reference(&listview.listview_height, parent_component, "self");
        let vp_w = access_named_reference(&listview.viewport_width, parent_component, "self");
        let lv_w = access_named_reference(&listview.listview_width, parent_component, "self");

        let ensure_updated = format!(
            "self->{}.ensure_updated_listview(self, &{}, &{}, &{}, {}.get(), {}.get());",
            repeater_id, vp_w, vp_h, vp_y, lv_w, lv_h
        );

        children_visitor_cases.push(format!(
            "\n        case {i}: {{
                {e_u}
                return self->{id}.visit(order, visitor);
            }}",
            i = repeater_count,
            e_u = ensure_updated,
            id = repeater_id,
        ));

        layout_repeater_code.push(ensure_updated)
    } else {
        children_visitor_cases.push(format!(
            "\n        case {i}: {{
                self->{id}.ensure_updated(self);
                return self->{id}.visit(order, visitor);
            }}",
            id = repeater_id,
            i = repeater_count,
        ));

        layout_repeater_code.push(format!("self->{}.compute_layout();", repeater_id));
    }

    repeated_input_branch.push(format!(
        "\n        case {i}: return self->{id}.item_at(rep_index);",
        i = repeater_count,
        id = repeater_id,
    ));

    component_struct.members.push((
        Access::Private,
        Declaration::Var(Var {
            ty: format!(
                "sixtyfps::Repeater<class {}, {}>",
                component_id(base_component),
                model_data_type(&parent_element, diag)
            ),
            name: repeater_id,
            init: None,
        }),
    ));
}

/// Returns the text of the C++ code produced by the given root component
pub fn generate(doc: &Document, diag: &mut BuildDiagnostics) -> Option<impl std::fmt::Display> {
    let mut file = File::default();

    file.includes.push("<array>".into());
    file.includes.push("<limits>".into());
    file.includes.push("<sixtyfps.h>".into());

    for ty in &doc.inner_structs {
        if let Type::Object { fields, name: Some(name) } = ty {
            generate_struct(&mut file, name, fields, diag);
        }
    }
    for glob in doc.root_component.used_global.borrow().iter() {
        generate_component(&mut file, glob, diag, None);
    }

    generate_component(&mut file, &doc.root_component, diag, None);

    file.definitions.push(Declaration::Var(Var{
        ty: format!(
            "constexpr sixtyfps::private_api::VersionCheckHelper<{}, {}, {}>",
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH")),
        name: "THE_SAME_VERSION_MUST_BE_USED_FOR_THE_COMPILER_AND_THE_RUNTIME".into(),
        init: Some("sixtyfps::private_api::VersionCheckHelper<int(sixtyfps::private_api::VersionCheck::Major), int(sixtyfps::private_api::VersionCheck::Minor), int(sixtyfps::private_api::VersionCheck::Patch)>()".into())
    }));

    if diag.has_error() {
        None
    } else {
        Some(file)
    }
}

fn generate_struct(
    file: &mut File,
    name: &str,
    fields: &BTreeMap<String, Type>,
    diag: &mut BuildDiagnostics,
) {
    let mut members = fields
        .iter()
        .map(|(name, t)| {
            (
                Access::Public,
                Declaration::Var(Var {
                    ty: t.cpp_type().unwrap_or_else(|| {
                        diag.push_error(
                            format!("Cannot map {} to a C++ type", t),
                            &Option::<crate::parser::SyntaxNodeWithSourceFile>::None,
                        );
                        Default::default()
                    }),
                    name: name.clone(),
                    ..Default::default()
                }),
            )
        })
        .collect::<Vec<_>>();
    members.sort_unstable_by(|a, b| match (&a.1, &b.1) {
        (Declaration::Var(a), Declaration::Var(b)) => a.name.cmp(&b.name),
        _ => unreachable!(),
    });
    file.declarations.push(Declaration::Struct(Struct {
        name: name.into(),
        members,
        ..Default::default()
    }))
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
        let ty = if let Type::Signal { args } = &property_decl.property_type {
            let param_types = args
                .iter()
                .map(|t| get_cpp_type(t, &property_decl.type_node, diag))
                .collect::<Vec<_>>();
            if property_decl.expose_in_public_api && is_root {
                let signal_emitter = vec![format!(
                    "{}.emit({});",
                    cpp_name,
                    (0..args.len()).map(|i| format!("arg_{}", i)).join(", ")
                )];
                component_struct.members.push((
                    Access::Public,
                    Declaration::Function(Function {
                        name: format!("emit_{}", cpp_name),
                        signature: format!(
                            "({})",
                            param_types
                                .iter()
                                .enumerate()
                                .map(|(i, ty)| format!("{} arg_{}", ty, i))
                                .join(", ")
                        ),
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
            format!("sixtyfps::Signal<{}>", param_types.join(", "))
        } else {
            let cpp_type =
                get_cpp_type(&property_decl.property_type, &property_decl.type_node, diag);

            if property_decl.expose_in_public_api && is_root {
                let access = if let Some(alias) = &property_decl.is_alias {
                    access_named_reference(alias, component, "this")
                } else {
                    format!("this->{}", cpp_name)
                };

                let prop_getter: Vec<String> = vec![format!("return {}.get();", access)];
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
                    "{}.{};",
                    access,
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

        if property_decl.is_alias.is_none() {
            component_struct.members.push((
                if component.is_global() { Access::Public } else { Access::Private },
                Declaration::Var(Var { ty, name: cpp_name.clone(), init: None }),
            ));
        }
    }

    let mut constructor_parent_arg = String::new();

    if !is_root {
        let parent_element = component.parent_element.upgrade().unwrap();

        let mut update_statements = vec![];
        let cpp_model_data_type = model_data_type(&parent_element, diag);

        if !parent_element.borrow().repeated.as_ref().map_or(false, |r| r.is_conditional_element) {
            component_struct.members.push((
                Access::Private,
                Declaration::Var(Var {
                    ty: "sixtyfps::Property<int>".into(),
                    name: "index".into(),
                    init: None,
                }),
            ));

            component_struct.members.push((
                Access::Private,
                Declaration::Var(Var {
                    ty: format!("sixtyfps::Property<{}>", cpp_model_data_type),
                    name: "model_data".into(),
                    init: None,
                }),
            ));

            update_statements = vec!["index.set(i);".into(), "model_data.set(data);".into()];
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
        let parent_type = format!("class {} const *", parent_component_id);
        constructor_parent_arg = format!("{} parent", parent_type);
        init.push("this->parent = parent;".into());
        component_struct.members.push((
            Access::Public, // Because Repeater accesses it
            Declaration::Var(Var {
                ty: parent_type,
                name: "parent".into(),
                init: Some("nullptr".to_owned()),
            }),
        ));
        component_struct.friends.push(parent_component_id);
        component_struct.members.push((
            Access::Public, // Because Repeater accesses it
            Declaration::Function(Function {
                name: "update_data".into(),
                signature: format!(
                    "([[maybe_unused]] int i, [[maybe_unused]] const {} &data) -> void",
                    cpp_model_data_type
                ),
                statements: Some(update_statements),
                ..Function::default()
            }),
        ));

        if parent_element.borrow().repeated.as_ref().map_or(false, |r| r.is_listview.is_some()) {
            let p_y = access_member(&component.root_element, "y", component, "this");
            let p_height = access_member(&component.root_element, "height", component, "this");
            let p_width = access_member(&component.root_element, "width", component, "this");
            component_struct.members.push((
                Access::Public, // Because Repeater accesses it
                Declaration::Function(Function {
                    name: "listview_layout".into(),
                    signature:
                        "(float *offset_y, const sixtyfps::Property<float> *viewport_width) const -> void"
                            .to_owned(),
                    statements: Some(vec![
                        "compute_layout({&component_type, const_cast<void *>(static_cast<const void *>(this))});".to_owned(),
                        format!("{}.set(*offset_y);", p_y),
                        format!("*offset_y += {}.get();", p_height),
                        "float vp_w = viewport_width->get();".to_owned(),
                        format!("float w = {}.get();", p_width),
                        "if (vp_w < w)".to_owned(),
                        "    viewport_width->set(w);".to_owned(),
                    ]),
                    ..Function::default()
                }),
            ));
        }
    } else if !component.is_global() {
        component_struct.members.push((
            Access::Public, // FIXME: many of the different component bindings need to access this
            Declaration::Var(Var {
                ty: "sixtyfps::private_api::ComponentWindow".into(),
                name: "window".into(),
                ..Var::default()
            }),
        ));

        let root_elem = component.root_element.borrow();
        component_struct.members.push((
            Access::Public,
            Declaration::Function(Function {
                name: "root_item".into(),
                signature: "() -> VRef<sixtyfps::private_api::ItemVTable>".into(),
                statements: Some(vec![format!(
                    "return {{ &sixtyfps::private_api::{vt}, &this->{id} }};",
                    vt = root_elem.base_type.as_native().vtable_symbol,
                    id = root_elem.id
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
        ));

        init.push(format!(
            "{}.init_items(this, item_tree());",
            window = window_ref_expression(component)
        ));
    }

    let mut children_visitor_cases = vec![];
    let mut repeated_input_branch = vec![];
    let mut repeater_layout_code = vec![];
    let mut tree_array = vec![];
    let mut repeater_count = 0;
    super::build_array_helper(component, |item_rc, children_offset, is_flickable_rect| {
        let item = item_rc.borrow();
        if is_flickable_rect {
            tree_array.push(format!(
                "sixtyfps::private_api::make_item_node(offsetof({}, {}) + offsetof(sixtyfps::Flickable, viewport), &sixtyfps::private_api::RectangleVTable, {}, {})",
                &component_id,
                item.id,
                item.children.len(),
                tree_array.len() + 1,
            ));
        } else if item.base_type == Type::Void {
            assert!(component.is_global());
            for (prop_name, binding_expression) in &item.bindings {
                handle_property_binding(item_rc, prop_name, binding_expression, &mut init);
            }
        } else if let Some(repeated) = &item.repeated {
            tree_array.push(format!("sixtyfps::private_api::make_dyn_node({})", repeater_count,));
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
                &mut repeater_layout_code,
                diag,
            );
            repeater_count += 1;
        } else {
            tree_array.push(format!(
                "sixtyfps::private_api::make_item_node(offsetof({}, {}), &sixtyfps::private_api::{}, {}, {})",
                &component_id,
                item.id,
                item.base_type.as_native().vtable_symbol,
                if super::is_flickable(item_rc) { 1 } else { item.children.len() },
                children_offset,
            ));
            handle_item(item_rc, &mut component_struct, &mut init);
        }
    });

    for extra_init_code in component.setup_code.borrow().iter() {
        init.push(compile_expression(extra_init_code, component));
    }

    component_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: component_id.clone(),
            signature: format!("({})", constructor_parent_arg),
            is_constructor_or_destructor: true,
            statements: Some(init),
            ..Default::default()
        }),
    ));

    if !component.is_global() {
        let mut destructor = Vec::new();

        destructor.push("auto self = this;".to_owned());
        if component.parent_element.upgrade().is_some() {
            destructor.push("if (!parent) return;".to_owned())
        }
        destructor.push(format!(
            "{window}.free_graphics_resources(this);",
            window = window_ref_expression(component)
        ));

        component_struct.members.push((
            Access::Public,
            Declaration::Function(Function {
                name: format!("~{}", component_id.clone()),
                signature: "()".to_owned(),
                is_constructor_or_destructor: true,
                statements: Some(destructor),
                ..Default::default()
            }),
        ));
    }

    if !component.is_global() {
        component_struct.members.push((
            Access::Private,
            Declaration::Function(Function {
                name: "visit_children".into(),
                signature: "(sixtyfps::private_api::ComponentRef component, intptr_t index, sixtyfps::TraversalOrder order, sixtyfps::private_api::ItemVisitorRefMut visitor) -> int64_t".into(),
                is_static: true,
                statements: Some(vec![
                    "static const auto dyn_visit = [] (const uint8_t *base,  [[maybe_unused]] sixtyfps::TraversalOrder order, [[maybe_unused]] sixtyfps::private_api::ItemVisitorRefMut visitor, uintptr_t dyn_index) -> int64_t {".to_owned(),
                    format!("    [[maybe_unused]] auto self = reinterpret_cast<const {}*>(base);", component_id),
                    format!("    switch(dyn_index) {{ {} }};", children_visitor_cases.join("")),
                    "    std::abort();\n};".to_owned(),
                    "return sixtyfps::sixtyfps_visit_item_tree(component, item_tree() , index, order, visitor, dyn_visit);".to_owned(),
                ]),
                ..Default::default()
            }),
        ));

        component_struct.members.push((
            Access::Private,
            Declaration::Function(Function {
                name: "item_tree".into(),
                signature: "() -> sixtyfps::Slice<sixtyfps::private_api::ItemTreeNode>".into(),
                is_static: true,
                statements: Some(vec![
                    "static const sixtyfps::private_api::ItemTreeNode children[] {".to_owned(),
                    format!("    {} }};", tree_array.join(", ")),
                    "return { const_cast<sixtyfps::private_api::ItemTreeNode*>(children), std::size(children) };"
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
                    "(sixtyfps::private_api::ComponentRef component, sixtyfps::MouseEvent mouse_event, const sixtyfps::private_api::ComponentWindow *window, const sixtyfps::private_api::ComponentRef *app_component) -> sixtyfps::InputEventResult"
                        .into(),
                is_static: true,
                statements: Some(vec![
                    format!("    auto self = reinterpret_cast<{}*>(component.instance);", component_id),
                    "return sixtyfps::private_api::process_input_event(component, self->mouse_grabber, mouse_event, item_tree(), [self](int dyn_index, [[maybe_unused]] int rep_index) {".into(),
                    format!("    switch(dyn_index) {{ {} }};", repeated_input_branch.join("")),
                    "    return sixtyfps::private_api::ComponentRef{nullptr, nullptr};\n}, window, app_component);".into(),
                ]),
                ..Default::default()
            }),
        ));

        component_struct.members.push((
            Access::Private,
            Declaration::Function(Function {
                name: "key_event".into(),
                signature:
                    "(sixtyfps::private_api::ComponentRef component, const sixtyfps::KeyEvent *key_event, const sixtyfps::private_api::ComponentWindow *window) -> sixtyfps::KeyEventResult"
                    .into(),
                    is_static: true,
                    statements: Some(vec![
                        format!("    auto self = reinterpret_cast<{}*>(component.instance);", component_id),
                        "return sixtyfps::private_api::process_key_event(component, self->focus_item, key_event, item_tree(), [self](int dyn_index, [[maybe_unused]] int rep_index) {".into(),
                        format!("    switch(dyn_index) {{ {} }};", repeated_input_branch.join("")),
                        "    return sixtyfps::private_api::ComponentRef{nullptr, nullptr};\n}, window);".into(),
                    ]),
                    ..Default::default()
                }),
            ));

        component_struct.members.push((
            Access::Private,
            Declaration::Var(Var {
                ty: "int64_t".into(),
                name: "focus_item".into(),
                init: Some("-1".into()),
            }),
        ));
        component_struct.members.push((
            Access::Private,
            Declaration::Function(Function {
                name: "focus_event".into(),
                signature:
                    "(sixtyfps::private_api::ComponentRef component, const sixtyfps::FocusEvent *focus_event, const sixtyfps::private_api::ComponentWindow *window) -> sixtyfps::FocusEventResult"
                        .into(),
                is_static: true,
                statements: Some(vec![
                    format!("    auto self = reinterpret_cast<{}*>(component.instance);", component_id),
                    "return sixtyfps::private_api::process_focus_event(component, self->focus_item, focus_event, item_tree(), [self](int dyn_index, [[maybe_unused]] int rep_index) {".into(),
                    format!("    switch(dyn_index) {{ {} }};", repeated_input_branch.join("")),
                    "    return sixtyfps::private_api::ComponentRef{nullptr, nullptr};\n}, window);".into(),
                ]),
                ..Default::default()
            }),
        ));

        component_struct.members.push((
            Access::Public, // FIXME: we call this function from tests
            Declaration::Function(Function {
                name: "compute_layout".into(),
                signature: "(sixtyfps::private_api::ComponentRef component) -> void".into(),
                is_static: true,
                statements: Some(compute_layout(component, &mut repeater_layout_code)),
                ..Default::default()
            }),
        ));

        component_struct.members.push((
            Access::Public,
            Declaration::Var(Var {
                ty: "static const sixtyfps::private_api::ComponentVTable".to_owned(),
                name: "component_type".to_owned(),
                init: None,
            }),
        ));
    }

    for glob in component.used_global.borrow().iter() {
        component_struct.members.push((
            Access::Private,
            Declaration::Var(Var {
                ty: format!("std::shared_ptr<{}>", self::component_id(glob)),
                name: format!("global_{}", glob.id),
                init: Some(format!("std::make_shared<{}>()", self::component_id(glob))),
            }),
        ));
    }

    file.definitions.extend(component_struct.extract_definitions().collect::<Vec<_>>());
    file.declarations.push(Declaration::Struct(component_struct));

    if !component.is_global() {
        file.definitions.push(Declaration::Var(Var {
            ty: "const sixtyfps::private_api::ComponentVTable".to_owned(),
            name: format!("{}::component_type", component_id),
            init: Some(
                "{ visit_children, nullptr, compute_layout, input_event, key_event, focus_event }"
                    .to_owned(),
            ),
        }));
    }
}

fn component_id(component: &Rc<Component>) -> String {
    if component.id.is_empty() {
        format!("Component_{}", component.root_element.borrow().id)
    } else {
        component.id.clone()
    }
}

fn model_data_type(parent_element: &ElementRc, diag: &mut BuildDiagnostics) -> String {
    if parent_element.borrow().repeated.as_ref().unwrap().is_conditional_element {
        return "int".into();
    }
    let model_data_type = crate::expression_tree::Expression::RepeaterModelReference {
        element: Rc::downgrade(parent_element),
    }
    .ty();
    model_data_type
        .cpp_type()
        .unwrap_or_else(|| {
            diag.push_internal_error(
                CompilerDiagnostic {
                    message: format!("Cannot map property type {} to C++", model_data_type).into(),
                    span: parent_element
                        .borrow()
                        .node
                        .as_ref()
                        .map(|n| n.span())
                        .unwrap_or_default(),
                    level: Level::Error,
                }
                .into(),
            );
            String::default()
        })
        .to_owned()
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
        if e.property_declarations.contains_key(name) || name == "" || component.is_global() {
            format!("{}->{}", component_cpp, name)
        } else if let Some(vp) = super::as_flickable_viewport_property(element, name) {
            format!("{}->{}.viewport.{}", component_cpp, e.id.as_str(), vp)
        } else {
            format!("{}->{}.{}", component_cpp, e.id.as_str(), name)
        }
    } else if enclosing_component.is_global() {
        let mut root_component = component.clone();
        let mut component_cpp = component_cpp.to_owned();
        while let Some(p) = root_component.parent_element.upgrade() {
            root_component = p.borrow().enclosing_component.upgrade().unwrap();
            component_cpp = format!("{}->parent", component_cpp);
        }
        let global_comp = format!("{}->global_{}", component_cpp, enclosing_component.id);
        access_member(element, name, &enclosing_component, &global_comp)
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

/// Call access_member  for a NamedReference
fn access_named_reference(
    nr: &NamedReference,
    component: &Rc<Component>,
    component_cpp: &str,
) -> String {
    access_member(&nr.element.upgrade().unwrap(), &nr.name, component, component_cpp)
}

/// Return an expression that gets the window
fn window_ref_expression(component: &Rc<Component>) -> String {
    let mut root_component = component.clone();
    let mut component_cpp = "self".to_owned();
    while let Some(p) = root_component.parent_element.upgrade() {
        root_component = p.borrow().enclosing_component.upgrade().unwrap();
        component_cpp = format!("{}->parent", component_cpp);
    }
    format!("{}->window", component_cpp)
}

fn compile_expression(e: &crate::expression_tree::Expression, component: &Rc<Component>) -> String {
    match e {
        Expression::StringLiteral(s) => {
            format!(r#"sixtyfps::SharedString("{}")"#, s.escape_debug())
        }
        Expression::NumberLiteral(n, unit) => unit.normalize(*n).to_string(),
        Expression::BoolLiteral(b) => b.to_string(),
        Expression::PropertyReference(nr) => {
            let access =
                access_named_reference(nr, component, "self");
            format!(r#"{}.get()"#, access)
        }
        Expression::SignalReference(nr) => format!(
            "{}.emit",
            access_named_reference(nr, component, "self")
        ),
        Expression::BuiltinFunctionReference(funcref) => match funcref {
            BuiltinFunction::GetWindowScaleFactor => {
                format!("{}.scale_factor", window_ref_expression(component))
            }
            BuiltinFunction::Debug => {
                "[](auto... args){ (std::cout << ... << args) << std::endl; return nullptr; }"
                    .into()
            }
            BuiltinFunction::SetFocusItem => {
                format!("{}.set_focus_item", window_ref_expression(component))
            }
        },
        Expression::ElementReference(_) => todo!("Element references are only supported in the context of built-in function calls at the moment"),
        Expression::MemberFunction { .. } => panic!("member function expressions must not appear in the code generator anymore"),
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
        Expression::FunctionParameterReference { index, .. } => format!("arg_{}", index),
        Expression::StoreLocalVariable { name, value } => {
            format!("auto {} = {};", name, compile_expression(value, component))
        }
        Expression::ReadLocalVariable { name, .. } => name.clone(),
        Expression::ObjectAccess { base, name } => match base.ty() {
            Type::Object { fields, name : None } => {
                let index = fields
                    .keys()
                    .position(|k| k == name)
                    .expect("Expression::ObjectAccess: Cannot find a key in an object");
                format!("std::get<{}>({})", index, compile_expression(base, component))
            }
            Type::Object{..} => {
                format!("{}.{}", compile_expression(base, component), name)
            }
            _ => panic!("Expression::ObjectAccess's base expression is not an Object type"),
        },
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
                (Type::Float32, Type::Color) => {
                    format!("sixtyfps::Color::from_argb_encoded({})", f)
                }
                (Type::Object { .. }, Type::Object{ fields, name: Some(n)}) => {
                    format!(
                        "[&](const auto &o){{ {struct_name} s; auto& [{field_members}] = s; {fields}; return s; }}({obj})",
                        struct_name = n,
                        field_members = (0..fields.len()).map(|idx| format!("f_{}", idx)).join(", "),
                        obj = f,
                        fields = (0..fields.len())
                            .map(|idx| format!("f_{} = std::get<{}>(o)", idx, idx))
                            .join("; ")
                    )
                }
                _ => f,
            }
        }
        Expression::CodeBlock(sub) => {
            let mut x = sub.iter().map(|e| compile_expression(e, component)).collect::<Vec<_>>();
            x.last_mut().map(|s| *s = format!("return {};", s));

            format!("[&]{{ {} }}()", x.join(";"))
        }
        Expression::FunctionCall { function, arguments } => match &**function {
            Expression::BuiltinFunctionReference(BuiltinFunction::SetFocusItem) => {
                if arguments.len() != 1 {
                    panic!("internal error: incorrect argument count to SetFocusItem call");
                }
                if let Expression::ElementReference(focus_item) = &arguments[0] {
                    let focus_item = focus_item.upgrade().unwrap();
                    let focus_item = focus_item.borrow();
                    let window_ref = window_ref_expression(component);
                    let component =
                        format!("{{&{}::component_type, this}}", component_id(component));
                    let item = format!(
                        "{{&sixtyfps::private_api::{vt}, &{item}}}",
                        vt = focus_item.base_type.as_native().vtable_symbol,
                        item = focus_item.id
                    );
                    format!("{}.set_focus_item({}, {});", window_ref, component, item)
                } else {
                    panic!("internal error: argument to SetFocusItem must be an element")
                }
            }
            _ => {
                let mut args = arguments.iter().map(|e| compile_expression(e, component));

                format!("{}({})", compile_expression(&function, component), args.join(", "))
            }
        },
        Expression::SelfAssignment { lhs, rhs, op } => {
            let rhs = compile_expression(&*rhs, &component);
            compile_assignment(lhs, *op, rhs, component)
        }
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
            let ty = e.ty();
            if ty == Type::Invalid || ty == Type::Void {
                format!(
                    r#"[&]() {{ if ({}) {{ {}; }} else {{ {}; }}}}()"#,
                    cond_code,
                    true_code,
                    false_code
                )
            } else {
                format!(
                    r#"[&]() -> {} {{ if ({}) {{ return {}; }} else {{ return {}; }}}}()"#,
                    ty.cpp_type().unwrap(),
                    cond_code,
                    true_code,
                    false_code
                )
            }
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
                    .join(", ")
            )
        }
        Expression::Object { ty, values } => {
            if let Type::Object{fields, name} = ty {
                let mut elem = fields.keys().map(|k| {
                    values
                        .get(k)
                        .map(|e| compile_expression(e, component))
                        .unwrap_or_else(|| "(Error: missing member in object)".to_owned())
                });
                if let Some(name) = name {
                    format!("{}{{{}}}", name, elem.join(", "))
                } else {
                    format!("std::make_tuple({})", elem.join(", "))
                }
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
        Expression::Uncompiled(_) | Expression::TwoWayBinding(..) => panic!(),
        Expression::Invalid => format!("\n#error invalid expression\n"),
    }
}

fn compile_assignment(
    lhs: &Expression,
    op: char,
    rhs: String,
    component: &Rc<Component>,
) -> String {
    match lhs {
        Expression::PropertyReference(nr) => {
            let access = access_named_reference(nr, component, "self");
            if op == '=' {
                format!(r#"{lhs}.set({rhs})"#, lhs = access, rhs = rhs)
            } else {
                format!(r#"{lhs}.set({lhs}.get() {op} {rhs})"#, lhs = access, rhs = rhs, op = op,)
            }
        }
        Expression::ObjectAccess { base, name } => {
            let tmpobj = "tmpobj";
            let get_obj = compile_expression(base, component);
            let ty = base.ty();
            let member = match &ty {
                Type::Object { fields, name: None } => {
                    let index = fields
                        .keys()
                        .position(|k| k == name)
                        .expect("Expression::ObjectAccess: Cannot find a key in an object");
                    format!("std::get<{}>({})", index, tmpobj)
                }
                Type::Object { .. } => format!("{}.{}", tmpobj, name),
                _ => panic!("Expression::ObjectAccess's base expression is not an Object type"),
            };
            let op = if op == '=' { ' ' } else { op };
            let new_value = format!(
                "[&]{{ auto {tmp} = {get}; {member} {op}= {rhs}; return {tmp}; }}()",
                tmp = tmpobj,
                get = get_obj,
                member = member,
                op = op,
                rhs = rhs,
            );
            compile_assignment(base, '=', new_value, component)
        }
        Expression::RepeaterModelReference { element } => {
            let element = element.upgrade().unwrap();
            let parent_component = element.borrow().base_type.as_component().clone();
            let repeater_access = access_member(
                &parent_component
                    .parent_element
                    .upgrade()
                    .unwrap()
                    .borrow()
                    .enclosing_component
                    .upgrade()
                    .unwrap()
                    .root_element,
                "",
                component,
                "self",
            );
            let index_access = access_member(&parent_component.root_element, "", component, "self");
            let repeater_id = format!("repeater_{}", element.borrow().id);
            if op == '=' {
                format!(
                    "{}{}.model_set_row_data({}index.get(), {})",
                    repeater_access, repeater_id, index_access, rhs
                )
            } else {
                format!(
                    "{}{}.model_set_row_data({}index.get(), {} {} {})",
                    repeater_access,
                    repeater_id,
                    index_access,
                    rhs,
                    op,
                    compile_expression(lhs, component)
                )
            }
        }
        _ => panic!("typechecking should make sure this was a PropertyReference"),
    }
}

struct CppLanguageLayoutGen;
impl crate::layout::gen::Language for CppLanguageLayoutGen {
    type CompiledCode = String;

    fn make_grid_layout_cell_data<'a, 'b>(
        item: &'a crate::layout::LayoutItem,
        col: u16,
        row: u16,
        colspan: u16,
        rowspan: u16,
        layout_tree: &'b mut Vec<crate::layout::gen::LayoutTreeItem<'a, Self>>,
        component: &Rc<Component>,
    ) -> Self::CompiledCode {
        let layout_info = get_layout_info_ref(item, layout_tree, component);
        let lay_rect = item.rect();
        let get_property_ref = |p: &Option<NamedReference>| match p {
            Some(nr) => format!("&{}", access_named_reference(nr, component, "self")),
            None => "nullptr".to_owned(),
        };
        format!(
            "        {{ {c}, {r}, {cs}, {rs}, {li}, {x}, {y}, {w}, {h} }},",
            c = col,
            r = row,
            cs = colspan,
            rs = rowspan,
            li = layout_info,
            x = get_property_ref(&lay_rect.x_reference),
            y = get_property_ref(&lay_rect.y_reference),
            w = get_property_ref(&lay_rect.width_reference),
            h = get_property_ref(&lay_rect.height_reference)
        )
    }

    fn grid_layout_tree_item<'a, 'b>(
        layout_tree: &'b mut Vec<crate::layout::gen::LayoutTreeItem<'a, Self>>,
        geometry: &'a LayoutGeometry,
        cells: Vec<Self::CompiledCode>,
        component: &Rc<Component>,
    ) -> crate::layout::gen::LayoutTreeItem<'a, Self> {
        let cell_ref_variable = format!("cells_{}", layout_tree.len()).to_owned();
        let mut creation_code = cells;
        creation_code.insert(
            0,
            format!("    sixtyfps::GridLayoutCellData {}_data[] = {{", cell_ref_variable,),
        );
        creation_code.push("    };".to_owned());
        creation_code.push(format!(
                "    const sixtyfps::Slice<sixtyfps::GridLayoutCellData> {cv}{{{cv}_data, std::size({cv}_data)}};",
                cv = cell_ref_variable
            ));

        let (padding, spacing) = generate_layout_padding_and_spacing(
            &mut creation_code,
            geometry,
            &layout_tree,
            component,
        );

        LayoutTreeItem::GridLayout {
            geometry: &geometry,
            spacing,
            padding,
            var_creation_code: creation_code.join("\n"),
            cell_ref_variable,
        }
    }

    fn box_layout_tree_item<'a, 'b>(
        layout_tree: &'b mut Vec<crate::layout::gen::LayoutTreeItem<'a, Self>>,
        box_layout: &'a crate::layout::BoxLayout,
        component: &Rc<Component>,
    ) -> crate::layout::gen::LayoutTreeItem<'a, Self> {
        let cells: Vec<_> = box_layout
            .elems
            .iter()
            .map(|cell| {
                let layout_info = get_layout_info_ref(&cell, layout_tree, component);
                let lay_rect = cell.rect();
                let get_property_ref = |p: &Option<NamedReference>| match p {
                    Some(nr) => format!("&{}", access_named_reference(nr, component, "self")),
                    None => "nullptr".to_owned(),
                };
                format!(
                    "        {{ {li}, {x}, {y}, {w}, {h} }},",
                    li = layout_info,
                    x = get_property_ref(&lay_rect.x_reference),
                    y = get_property_ref(&lay_rect.y_reference),
                    w = get_property_ref(&lay_rect.width_reference),
                    h = get_property_ref(&lay_rect.height_reference)
                )
            })
            .collect();

        let cell_ref_variable = format!("cells_{}", layout_tree.len()).to_owned();
        let mut creation_code = cells;
        creation_code.insert(
            0,
            format!("    sixtyfps::BoxLayoutCellData {}_data[] = {{", cell_ref_variable,),
        );
        creation_code.push("    };".to_owned());
        creation_code.push(format!(
                "    const sixtyfps::Slice<sixtyfps::BoxLayoutCellData> {cv}{{{cv}_data, std::size({cv}_data)}};",
                cv = cell_ref_variable
            ));

        let (padding, spacing) = generate_layout_padding_and_spacing(
            &mut creation_code,
            &box_layout.geometry,
            &layout_tree,
            component,
        );

        LayoutTreeItem::BoxLayout {
            is_horizontal: box_layout.is_horizontal,
            geometry: &box_layout.geometry,
            spacing,
            padding,
            var_creation_code: creation_code.join("\n"),
            cell_ref_variable,
        }
    }
}

type LayoutTreeItem<'a> = crate::layout::gen::LayoutTreeItem<'a, CppLanguageLayoutGen>;

fn get_layout_info_ref<'a, 'b>(
    item: &'a crate::layout::LayoutItem,
    layout_tree: &'b mut Vec<LayoutTreeItem<'a>>,
    component: &Rc<Component>,
) -> String {
    let layout_info = item.layout.as_ref().map(|l| {
        let layout_tree_item =
            crate::layout::gen::collect_layouts_recursively(layout_tree, l, component);
        match layout_tree_item {
            LayoutTreeItem::GridLayout { cell_ref_variable, spacing, padding, .. } => format!(
                "sixtyfps::grid_layout_info(&{}, {}, &{})",
                cell_ref_variable, spacing, padding
            ),
            LayoutTreeItem::BoxLayout {
                spacing,
                cell_ref_variable,
                padding,
                is_horizontal,
                ..
            } => format!(
                "sixtyfps::box_layout_info(&{}, {}, &{}, {})",
                cell_ref_variable, spacing, padding, is_horizontal
            ),
            LayoutTreeItem::PathLayout(_) => todo!(),
        }
    });
    let elem_info = item.element.as_ref().map(|elem| {
            format!(
                "sixtyfps::private_api::{vt}.layouting_info({{&sixtyfps::private_api::{vt}, const_cast<sixtyfps::{ty}*>(&self->{id})}}, &{window})",
                vt = elem.borrow().base_type.as_native().vtable_symbol,
                ty = elem.borrow().base_type.as_native().class_name,
                id = elem.borrow().id,
                window = window_ref_expression(component)
            )
        });
    let mut layout_info = match (layout_info, elem_info) {
        (None, None) => String::default(),
        (None, Some(x)) => x,
        (Some(x), None) => x,
        (Some(layout_info), Some(elem_info)) => format!("{}.merge({})", layout_info, elem_info),
    };
    if item.constraints.has_explicit_restrictions() {
        layout_info = format!("[&]{{ auto layout_info = {};", layout_info);
        for (expr, name) in item.constraints.for_each_restrictions().iter() {
            if let Some(e) = expr {
                layout_info += &format!(
                    " layout_info.{} = {}.get();",
                    name,
                    access_named_reference(e, component, "self")
                );
            }
        }
        layout_info += " return layout_info; }()";
    }
    layout_info
}

fn generate_layout_padding_and_spacing<'a, 'b>(
    creation_code: &mut Vec<String>,
    layout_geometry: &LayoutGeometry,
    layout_tree: &'b Vec<LayoutTreeItem<'a>>,
    component: &Rc<Component>,
) -> (String, String) {
    let spacing = if let Some(spacing) = &layout_geometry.spacing {
        let variable = format!("spacing_{}", layout_tree.len());
        creation_code.push(format!(
            "auto {} = {}.get();",
            variable,
            access_named_reference(spacing, component, "self")
        ));
        variable
    } else {
        "0.".into()
    };

    let padding = format!("padding_{}", layout_tree.len());
    let padding_prop = |expr| {
        if let Some(nr) = expr {
            format!("{}.get()", access_named_reference(nr, component, "self"))
        } else {
            "0.".into()
        }
    };
    creation_code.push(format!(
        "sixtyfps::Padding {} = {{ {}, {}, {}, {} }};",
        padding,
        padding_prop(layout_geometry.padding.left.as_ref()),
        padding_prop(layout_geometry.padding.right.as_ref()),
        padding_prop(layout_geometry.padding.top.as_ref()),
        padding_prop(layout_geometry.padding.bottom.as_ref()),
    ));

    (padding, spacing)
}

impl<'a> LayoutTreeItem<'a> {
    fn emit_solve_calls(&self, component: &Rc<Component>, code_stream: &mut Vec<String>) {
        let layout_prop = |p: &Option<NamedReference>| match p {
            Some(nr) => format!("{}.get()", access_named_reference(nr, component, "self")),
            None => "0".into(),
        };
        match self {
            LayoutTreeItem::GridLayout {
                geometry, spacing, cell_ref_variable, padding, ..
            } => {
                code_stream.push("    { ".into());
                code_stream.push("    sixtyfps::GridLayoutData grid { ".into());
                code_stream.push(format!(
                    "        {w}, {h}, {x}, {y}, {s}, &{p},",
                    w = layout_prop(&geometry.rect.width_reference),
                    h = layout_prop(&geometry.rect.height_reference),
                    x = layout_prop(&geometry.rect.x_reference),
                    y = layout_prop(&geometry.rect.y_reference),
                    s = spacing,
                    p = padding,
                ));
                code_stream.push(format!("        {cv}", cv = cell_ref_variable));
                code_stream.push("    };".to_owned());
                code_stream.push("    sixtyfps::solve_grid_layout(&grid);".to_owned());
                code_stream.push("    } ".into());
            }
            LayoutTreeItem::BoxLayout {
                geometry,
                spacing,
                cell_ref_variable,
                padding,
                is_horizontal,
                ..
            } => {
                code_stream.push("    { ".into());
                code_stream.push("    sixtyfps::BoxLayoutData box { ".into());
                code_stream.push(format!(
                    "        {w}, {h}, {x}, {y}, {s}, &{p},",
                    w = layout_prop(&geometry.rect.width_reference),
                    h = layout_prop(&geometry.rect.height_reference),
                    x = layout_prop(&geometry.rect.x_reference),
                    y = layout_prop(&geometry.rect.y_reference),
                    s = spacing,
                    p = padding,
                ));
                code_stream.push(format!("        {cv}", cv = cell_ref_variable));
                code_stream.push("    };".to_owned());
                code_stream
                    .push(format!("    sixtyfps::solve_box_layout(&box, {});", is_horizontal));
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
                                "    for (auto &&sub_comp : self->repeater_{}.inner->data)",
                                elem.borrow().id
                            ));
                            code_stream.push(format!(
                                "         items.push_back({});",
                                path_layout_item_data(
                                    &root_element,
                                    &format!("sub_comp.ptr->{}", root_element.borrow().id),
                                    "sub_comp.ptr",
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

                code_stream
                    .push(format!("    auto x = {};", layout_prop(&path_layout.rect.x_reference)));
                code_stream
                    .push(format!("    auto y = {};", layout_prop(&path_layout.rect.y_reference)));
                code_stream.push(format!(
                    "    auto width = {};",
                    layout_prop(&path_layout.rect.width_reference)
                ));
                code_stream.push(format!(
                    "    auto height = {};",
                    layout_prop(&path_layout.rect.height_reference)
                ));

                code_stream.push(format!(
                    "    auto offset = {};",
                    layout_prop(&Some(path_layout.offset_reference.clone()))
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

fn compute_layout(
    component: &Rc<Component>,
    repeater_layout_code: &mut Vec<String>,
) -> Vec<String> {
    let mut res = vec![];

    res.push(format!(
        "[[maybe_unused]] auto self = reinterpret_cast<const {ty}*>(component.instance);",
        ty = component_id(component)
    ));
    component.layouts.borrow().iter().for_each(|layout| {
        let mut inverse_layout_tree = Vec::new();

        res.push("    {".into());
        crate::layout::gen::collect_layouts_recursively(
            &mut inverse_layout_tree,
            layout,
            component,
        );

        res.extend(inverse_layout_tree.iter().filter_map(|layout| match layout {
            LayoutTreeItem::GridLayout { var_creation_code, .. } => Some(var_creation_code.clone()),
            LayoutTreeItem::BoxLayout { var_creation_code, .. } => Some(var_creation_code.clone()),
            LayoutTreeItem::PathLayout(_) => None,
        }));

        inverse_layout_tree
            .iter()
            .rev()
            .for_each(|layout| layout.emit_solve_calls(component, &mut res));
        res.push("    }".into());
    });

    res.append(repeater_layout_code);

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
