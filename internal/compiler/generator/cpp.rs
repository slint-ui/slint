// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

/*! module for the C++ code generator
*/

// cSpell:ignore cmath constexpr cstdlib decltype intptr itertools nullptr prepended struc subcomponent uintptr vals

use std::fmt::Write;

fn ident(ident: &str) -> String {
    if ident.contains('-') {
        ident.replace('-', "_")
    } else {
        ident.into()
    }
}

/// Given a property reference to a native item (eg, the property name is empty)
/// return tokens to the `ItemRc`
fn access_item_rc(pr: &llr::PropertyReference, ctx: &EvaluationContext) -> String {
    let mut ctx = ctx;
    let mut component_access = "self".into();

    let pr = match pr {
        llr::PropertyReference::InParent { level, parent_reference } => {
            for _ in 0..level.get() {
                component_access = format!("{}->parent", component_access);
                ctx = ctx.parent.as_ref().unwrap().ctx;
            }
            parent_reference
        }
        other => other,
    };

    match pr {
        llr::PropertyReference::InNativeItem { sub_component_path, item_index, prop_name } => {
            assert!(prop_name.is_empty());
            let (sub_compo_path, sub_component) =
                follow_sub_component_path(ctx.current_sub_component.unwrap(), sub_component_path);
            if !sub_component_path.is_empty() {
                component_access = format!("{}->{}", &component_access, &sub_compo_path);
            }
            let component_rc = format!("{}->self_weak.lock()->into_dyn()", &component_access);
            let item_index_in_tree = sub_component.items[*item_index].index_in_tree;
            let item_index = if item_index_in_tree == 0 {
                format!("{}->tree_index", &component_access)
            } else {
                format!(
                    "{}->tree_index_of_first_child + {} - 1",
                    &component_access, item_index_in_tree
                )
            };

            format!("{}, {}", &component_rc, item_index)
        }
        _ => unreachable!(),
    }
}

/// This module contains some data structure that helps represent a C++ code.
/// It is then rendered into an actual C++ text using the Display trait
mod cpp_ast {

    use std::cell::Cell;
    use std::fmt::{Display, Error, Formatter};
    thread_local!(static INDENTATION : Cell<u32> = Cell::new(0));
    fn indent(f: &mut Formatter<'_>) -> Result<(), Error> {
        INDENTATION.with(|i| {
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
        TypeAlias(TypeAlias),
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
            if self.members.is_empty() && self.friends.is_empty() {
                writeln!(f, "class {};", self.name)
            } else {
                writeln!(f, "class {} {{", self.name)?;
                INDENTATION.with(|x| x.set(x.get() + 1));
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
                INDENTATION.with(|x| x.set(x.get() - 1));
                indent(f)?;
                writeln!(f, "}};")
            }
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
                        is_friend: false,
                        statements: f.statements.take(),
                        template_parameters: f.template_parameters.clone(),
                        constructor_member_initializers: f.constructor_member_initializers.clone(),
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
        pub is_friend: bool,
        /// The list of statement instead the function.  When None,  this is just a function
        /// declaration without the definition
        pub statements: Option<Vec<String>>,
        /// What's inside template<...> if any
        pub template_parameters: Option<String>,
        /// Explicit initializers, such as FooClass::FooClass() : someMember(42) {}
        pub constructor_member_initializers: Vec<String>,
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
            if self.is_friend {
                write!(f, "friend ")?;
            }
            // all functions are `inline` because we are in a header
            write!(f, "inline ")?;
            if !self.is_constructor_or_destructor {
                write!(f, "auto ")?;
            }
            write!(f, "{} {}", self.name, self.signature)?;
            if let Some(st) = &self.statements {
                if !self.constructor_member_initializers.is_empty() {
                    writeln!(f, "\n : {}", self.constructor_member_initializers.join(","))?;
                }
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
        pub array_size: Option<usize>,
        pub init: Option<String>,
    }

    impl Display for Var {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
            indent(f)?;
            write!(f, "{} {}", self.ty, self.name)?;
            if let Some(size) = self.array_size {
                write!(f, "[{}]", size)?;
            }
            if let Some(i) = &self.init {
                write!(f, " = {}", i)?;
            }
            writeln!(f, ";")
        }
    }

    #[derive(Default, Debug)]
    pub struct TypeAlias {
        pub new_name: String,
        pub old_name: String,
    }

    impl Display for TypeAlias {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            indent(f)?;
            writeln!(f, "using {} = {};", self.new_name, self.old_name)
        }
    }

    pub trait CppType {
        fn cpp_type(&self) -> Option<String>;
    }

    pub fn escape_string(str: &str) -> String {
        let mut result = String::with_capacity(str.len());
        for x in str.chars() {
            match x {
                '\n' => result.push_str("\\n"),
                '\\' => result.push_str("\\\\"),
                '\"' => result.push_str("\\\""),
                '\t' => result.push_str("\\t"),
                '\r' => result.push_str("\\r"),
                _ if !x.is_ascii() || (x as u32) < 32 => {
                    use std::fmt::Write;
                    write!(result, "\\U{:0>8x}", x as u32).unwrap();
                }
                _ => result.push(x),
            }
        }
        result
    }
}

use crate::expression_tree::{BuiltinFunction, EasingCurve};
use crate::langtype::{NativeClass, Type};
use crate::layout::Orientation;
use crate::llr::{
    self, EvaluationContext as llr_EvaluationContext, ParentCtx as llr_ParentCtx,
    TypeResolutionContext as _,
};
use crate::object_tree::Document;
use cpp_ast::*;
use itertools::{Either, Itertools};
use std::collections::BTreeMap;
use std::num::NonZeroUsize;

type EvaluationContext<'a> = llr_EvaluationContext<'a, String>;
type ParentCtx<'a> = llr_ParentCtx<'a, String>;

impl CppType for Type {
    fn cpp_type(&self) -> Option<String> {
        match self {
            Type::Void => Some("void".to_owned()),
            Type::Float32 => Some("float".to_owned()),
            Type::Int32 => Some("int".to_owned()),
            Type::String => Some("slint::SharedString".to_owned()),
            Type::Color => Some("slint::Color".to_owned()),
            Type::Duration => Some("std::int64_t".to_owned()),
            Type::Angle => Some("float".to_owned()),
            Type::PhysicalLength => Some("float".to_owned()),
            Type::LogicalLength => Some("float".to_owned()),
            Type::Percent => Some("float".to_owned()),
            Type::Bool => Some("bool".to_owned()),
            Type::Struct { name: Some(name), node: Some(_), .. } => Some(ident(name)),
            Type::Struct { name: Some(name), node: None, .. } => {
                Some(if name.starts_with("slint::") {
                    name.clone()
                } else {
                    format!("slint::cbindgen_private::{}", ident(name))
                })
            }
            Type::Struct { fields, .. } => {
                let elem = fields.values().map(|v| v.cpp_type()).collect::<Option<Vec<_>>>()?;

                Some(format!("std::tuple<{}>", elem.join(", ")))
            }

            Type::Array(i) => Some(format!("std::shared_ptr<slint::Model<{}>>", i.cpp_type()?)),
            Type::Image => Some("slint::Image".to_owned()),
            Type::Enumeration(enumeration) => {
                Some(format!("slint::cbindgen_private::{}", ident(&enumeration.name)))
            }
            Type::Brush => Some("slint::Brush".to_owned()),
            Type::LayoutCache => Some("slint::SharedVector<float>".into()),
            _ => None,
        }
    }
}

fn to_cpp_orientation(o: Orientation) -> &'static str {
    match o {
        Orientation::Horizontal => "slint::cbindgen_private::Orientation::Horizontal",
        Orientation::Vertical => "slint::cbindgen_private::Orientation::Vertical",
    }
}

/// If the expression is surrounded with parentheses, remove these parentheses
fn remove_parentheses(expr: &str) -> &str {
    if expr.starts_with('(') && expr.ends_with(')') {
        let mut level = 0;
        // check that the opening and closing parentheses are on the same level
        for byte in expr[1..expr.len() - 1].as_bytes() {
            match byte {
                b')' if level == 0 => return expr,
                b')' => level -= 1,
                b'(' => level += 1,
                _ => (),
            }
        }
        &expr[1..expr.len() - 1]
    } else {
        expr
    }
}

#[test]
fn remove_parentheses_test() {
    assert_eq!(remove_parentheses("(foo(bar))"), "foo(bar)");
    assert_eq!(remove_parentheses("(foo).bar"), "(foo).bar");
    assert_eq!(remove_parentheses("(foo(bar))"), "foo(bar)");
    assert_eq!(remove_parentheses("(foo)(bar)"), "(foo)(bar)");
    assert_eq!(remove_parentheses("(foo).get()"), "(foo).get()");
    assert_eq!(remove_parentheses("((foo).get())"), "(foo).get()");
    assert_eq!(remove_parentheses("(((()())()))"), "((()())())");
    assert_eq!(remove_parentheses("((()())())"), "(()())()");
    assert_eq!(remove_parentheses("(()())()"), "(()())()");
    assert_eq!(remove_parentheses("()())("), "()())(");
}

fn property_set_value_code(
    property: &llr::PropertyReference,
    value_expr: &str,
    ctx: &EvaluationContext,
) -> String {
    let prop = access_member(property, ctx);
    if let Some(animation) = ctx.current_sub_component.and_then(|c| c.animations.get(property)) {
        let animation_code = compile_expression(animation, ctx);
        return format!("{}.set_animated_value({}, {})", prop, value_expr, animation_code);
    }
    format!("{}.set({})", prop, value_expr)
}

fn handle_property_init(
    prop: &llr::PropertyReference,
    binding_expression: &llr::BindingExpression,
    init: &mut Vec<String>,
    ctx: &EvaluationContext,
) {
    let prop_access = access_member(prop, ctx);
    let prop_type = ctx.property_ty(prop);
    if let Type::Callback { args, .. } = &prop_type {
        let mut ctx2 = ctx.clone();
        ctx2.argument_types = args;

        let mut params = args.iter().enumerate().map(|(i, ty)| {
            format!("[[maybe_unused]] {} arg_{}", ty.cpp_type().unwrap_or_default(), i)
        });

        init.push(format!(
            "{prop_access}.set_handler(
                    [this]({params}) {{
                        [[maybe_unused]] auto self = this;
                        return {code};
                    }});",
            prop_access = prop_access,
            params = params.join(", "),
            code = compile_expression_wrap_return(&binding_expression.expression.borrow(), &ctx2)
        ));
    } else {
        let init_expr =
            compile_expression_wrap_return(&binding_expression.expression.borrow(), ctx);

        init.push(if binding_expression.is_constant && !binding_expression.is_state_info {
            format!("{}.set({});", prop_access, init_expr)
        } else {
            let binding_code = format!(
                "[this]() {{
                            [[maybe_unused]] auto self = this;
                            return {init};
                        }}",
                init = init_expr
            );

            if binding_expression.is_state_info {
                format!("slint::private_api::set_state_binding({}, {});", prop_access, binding_code)
            } else {
                match &binding_expression.animation {
                    Some(llr::Animation::Static(anim)) => {
                        let anim = compile_expression(anim, ctx);
                        format!("{}.set_animated_binding({}, {});", prop_access, binding_code, anim)
                    }
                    Some(llr::Animation::Transition (
                        anim
                    )) => {
                        let anim = compile_expression(anim, ctx);
                        format!(
                            "{}.set_animated_binding_for_transition({},
                            [this](uint64_t *start_time) -> slint::cbindgen_private::PropertyAnimation {{
                                [[maybe_unused]] auto self = this;
                                auto [anim, time] = {};
                                *start_time = time;
                                return anim;
                            }});",
                            prop_access,
                            binding_code,
                            anim,
                        )
                    }
                    None => format!("{}.set_binding({});", prop_access, binding_code),
                }
            }
        });
    }
}

/// Returns the text of the C++ code produced by the given root component
pub fn generate(doc: &Document) -> impl std::fmt::Display {
    let mut file = File::default();

    file.includes.push("<array>".into());
    file.includes.push("<limits>".into());
    file.includes.push("<cstdlib>".into()); // TODO: ideally only include this if needed (by to_float)
    file.includes.push("<cmath>".into()); // TODO: ideally only include this if needed (by floor/ceil/round)
    file.includes.push("<slint.h>".into());

    file.declarations.extend(doc.root_component.embedded_file_resources.borrow().iter().map(
        |(path, er)| {
            match &er.kind {
                crate::embedded_resources::EmbeddedResourcesKind::RawData => {
                    let file = crate::fileaccess::load_file(std::path::Path::new(path)).unwrap(); // embedding pass ensured that the file exists
                    let data = file.read();

                    let mut init = "{ ".to_string();

                    for (index, byte) in data.iter().enumerate() {
                        if index > 0 {
                            init.push(',');
                        }
                        write!(&mut init, "0x{:x}", byte).unwrap();
                        if index % 16 == 0 {
                            init.push('\n');
                        }
                    }

                    init.push('}');

                    Declaration::Var(Var {
                        ty: "inline uint8_t".into(),
                        name: format!("slint_embedded_resource_{}", er.id),
                        array_size: Some(data.len()),
                        init: Some(init),
                    })
                }
                crate::embedded_resources::EmbeddedResourcesKind::TextureData(_) => todo!(),
                crate::embedded_resources::EmbeddedResourcesKind::BitmapFontData(_) => todo!(),
            }
        },
    ));

    for ty in doc.root_component.used_types.borrow().structs.iter() {
        if let Type::Struct { fields, name: Some(name), node: Some(_) } = ty {
            generate_struct(&mut file, name, fields);
        }
    }

    let llr = llr::lower_to_item_tree::lower_to_item_tree(&doc.root_component);

    // Forward-declare the root so that sub-components can access singletons, the window, etc.
    file.declarations.push(Declaration::Struct(Struct {
        name: ident(&llr.item_tree.root.name),
        ..Default::default()
    }));

    for sub_compo in &llr.sub_components {
        let sub_compo_id = ident(&sub_compo.name);
        let mut sub_compo_struct = Struct { name: sub_compo_id.clone(), ..Default::default() };
        generate_sub_component(
            &mut sub_compo_struct,
            sub_compo,
            &llr,
            None,
            Access::Public,
            &mut file,
        );
        file.definitions.extend(sub_compo_struct.extract_definitions().collect::<Vec<_>>());
        file.declarations.push(Declaration::Struct(sub_compo_struct));
    }

    for glob in llr.globals.iter().filter(|glob| !glob.is_builtin) {
        generate_global(&mut file, glob, &llr);
        file.definitions.extend(glob.aliases.iter().map(|name| {
            Declaration::TypeAlias(TypeAlias { old_name: ident(&glob.name), new_name: ident(name) })
        }))
    }

    generate_public_component(&mut file, &llr);

    file.definitions.push(Declaration::Var(Var{
        ty: format!(
            "[[maybe_unused]] constexpr slint::private_api::VersionCheckHelper<{}, {}, {}>",
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH")),
        name: "THE_SAME_VERSION_MUST_BE_USED_FOR_THE_COMPILER_AND_THE_RUNTIME".into(),
        init: Some("slint::private_api::VersionCheckHelper<SLINT_VERSION_MAJOR, SLINT_VERSION_MINOR, SLINT_VERSION_PATCH>()".into()),
        ..Default::default()
    }));

    file
}

fn generate_struct(file: &mut File, name: &str, fields: &BTreeMap<String, Type>) {
    let mut members = fields
        .iter()
        .map(|(name, t)| {
            (
                Access::Public,
                Declaration::Var(Var {
                    ty: t.cpp_type().unwrap(),
                    name: ident(name),
                    ..Default::default()
                }),
            )
        })
        .collect::<Vec<_>>();
    members.sort_unstable_by(|a, b| match (&a.1, &b.1) {
        (Declaration::Var(a), Declaration::Var(b)) => a.name.cmp(&b.name),
        _ => unreachable!(),
    });
    members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "operator==".to_owned(),
            signature: format!("(const {0} &a, const {0} &b) -> bool = default", name),
            is_friend: true,
            statements: None,
            ..Function::default()
        }),
    ));

    file.declarations.push(Declaration::Struct(Struct {
        name: name.into(),
        members,
        ..Default::default()
    }))
}

/// Generate the component in `file`.
///
/// `sub_components`, if Some, will be filled with all the sub component which needs to be added as friends
fn generate_public_component(file: &mut File, component: &llr::PublicComponent) {
    let root_component = &component.item_tree.root;
    let component_id = ident(&root_component.name);
    let mut component_struct = Struct { name: component_id.clone(), ..Default::default() };

    // The window need to be the first member so it is destroyed last
    component_struct.members.push((
        // FIXME: many of the different component bindings need to access this
        Access::Public,
        Declaration::Var(Var {
            ty: "slint::Window".into(),
            name: "m_window".into(),
            init: Some("slint::Window{slint::private_api::WindowAdapterRc()}".into()),
            ..Default::default()
        }),
    ));

    let ctx = EvaluationContext {
        public_component: component,
        current_sub_component: Some(&component.item_tree.root),
        current_global: None,
        generator_state: "this".to_string(),
        parent: None,
        argument_types: &[],
    };

    let old_declarations = file.declarations.len();

    generate_item_tree(
        &mut component_struct,
        &component.item_tree,
        component,
        None,
        component_id,
        Access::Private, // Hide properties and other fields from the C++ API
        file,
    );

    // Give generated sub-components, etc. access to our fields

    for new_decl in file.declarations.iter().skip(old_declarations) {
        if let Declaration::Struct(struc @ Struct { .. }) = new_decl {
            component_struct.friends.push(struc.name.clone());
        };
    }

    let declarations = generate_public_api_for_properties(&component.public_properties, &ctx);
    component_struct.members.extend(declarations.into_iter().map(|decl| (Access::Public, decl)));

    component_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "show".into(),
            signature: "()".into(),
            statements: Some(vec!["m_window.show();".into()]),
            ..Default::default()
        }),
    ));

    component_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "hide".into(),
            signature: "()".into(),
            statements: Some(vec!["m_window.hide();".into()]),
            ..Default::default()
        }),
    ));

    component_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "window".into(),
            signature: "() const -> slint::Window&".into(),
            statements: Some(vec![format!(
                "return const_cast<{} *>(this)->m_window;",
                component_struct.name
            )]),
            ..Default::default()
        }),
    ));

    component_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "run".into(),
            signature: "()".into(),
            statements: Some(vec![
                "show();".into(),
                "slint::run_event_loop();".into(),
                "hide();".into(),
            ]),
            ..Default::default()
        }),
    ));

    component_struct.friends.push("slint::private_api::WindowAdapterRc".into());

    component_struct
        .friends
        .extend(component.sub_components.iter().map(|sub_compo| ident(&sub_compo.name)));

    component_struct.friends.extend(
        component
            .item_tree
            .root
            .repeated
            .iter()
            .map(|repeater| ident(&repeater.sub_tree.root.name)),
    );

    for glob in &component.globals {
        let ty = if glob.is_builtin {
            format!("slint::cbindgen_private::{}", glob.name)
        } else {
            ident(&glob.name)
        };

        component_struct.members.push((
            Access::Public, // FIXME
            Declaration::Var(Var {
                ty: format!("std::shared_ptr<{}>", ty),
                name: format!("global_{}", ident(&glob.name)),
                init: Some(format!("std::make_shared<{}>(this)", ty)),
                ..Default::default()
            }),
        ));
    }

    let mut global_accessor_function_body = Vec::new();
    for glob in component.globals.iter().filter(|glob| glob.exported && !glob.is_builtin) {
        let accessor_statement = format!(
            "{0}if constexpr(std::is_same_v<T, {1}>) {{ return *global_{1}.get(); }}",
            if global_accessor_function_body.is_empty() { "" } else { "else " },
            ident(&glob.name),
        );
        global_accessor_function_body.push(accessor_statement);
    }
    if !global_accessor_function_body.is_empty() {
        global_accessor_function_body.push(
            "else { static_assert(!sizeof(T*), \"The type is not global/or exported\"); }".into(),
        );

        component_struct.members.push((
            Access::Public,
            Declaration::Function(Function {
                name: "global".into(),
                signature: "() const -> const T&".into(),
                statements: Some(global_accessor_function_body),
                template_parameters: Some("typename T".into()),
                ..Default::default()
            }),
        ));
    }

    file.definitions.extend(component_struct.extract_definitions().collect::<Vec<_>>());
    file.declarations.push(Declaration::Struct(component_struct));
}

fn generate_item_tree(
    target_struct: &mut Struct,
    sub_tree: &llr::ItemTree,
    root: &llr::PublicComponent,
    parent_ctx: Option<ParentCtx>,
    item_tree_class_name: String,
    field_access: Access,
    file: &mut File,
) {
    target_struct.friends.push(format!(
        "vtable::VRc<slint::private_api::ComponentVTable, {}>",
        item_tree_class_name
    ));

    generate_sub_component(
        target_struct,
        &sub_tree.root,
        root,
        parent_ctx.clone(),
        field_access,
        file,
    );

    let root_access = if parent_ctx.is_some() { "parent->root" } else { "self" };

    let mut item_tree_array: Vec<String> = Default::default();
    let mut item_array: Vec<String> = Default::default();

    sub_tree.tree.visit_in_array(&mut |node, children_offset, parent_index| {
        let parent_index = parent_index as u32;

        if node.repeated {
            assert_eq!(node.children.len(), 0);
            let mut repeater_index = node.item_index;
            let mut sub_component = &sub_tree.root;
            for i in &node.sub_component_path {
                repeater_index += sub_component.sub_components[*i].repeater_offset;
                sub_component = &sub_component.sub_components[*i].ty;
            }
            item_tree_array.push(format!(
                "slint::private_api::make_dyn_node({}, {})",
                repeater_index, parent_index
            ));
        } else {
            let mut compo_offset = String::new();
            let mut sub_component = &sub_tree.root;
            for i in &node.sub_component_path {
                let next_sub_component_name = ident(&sub_component.sub_components[*i].name);
                write!(
                    compo_offset,
                    "offsetof({}, {}) + ",
                    ident(&sub_component.name),
                    next_sub_component_name
                )
                .unwrap();
                sub_component = &sub_component.sub_components[*i].ty;
            }

            let item = &sub_component.items[node.item_index];

            if item.is_flickable_viewport {
                compo_offset += "offsetof(slint::cbindgen_private::Flickable, viewport) + ";
            }

            let children_count = node.children.len() as u32;
            let children_index = children_offset as u32;
            let item_array_index = item_array.len() as u32;

            item_tree_array.push(format!(
                "slint::private_api::make_item_node({}, {}, {}, {}, {})",
                children_count, children_index, parent_index, item_array_index, node.is_accessible
            ));
            item_array.push(format!(
                "{{ {}, {} offsetof({}, {}) }}",
                item.ty.cpp_vtable_getter,
                compo_offset,
                &ident(&sub_component.name),
                ident(&item.name),
            ));
        }
    });

    let mut visit_children_statements = vec![
        "static const auto dyn_visit = [] (const uint8_t *base,  [[maybe_unused]] slint::private_api::TraversalOrder order, [[maybe_unused]] slint::private_api::ItemVisitorRefMut visitor, [[maybe_unused]] uintptr_t dyn_index) -> uint64_t {".to_owned(),
        format!("    [[maybe_unused]] auto self = reinterpret_cast<const {}*>(base);", item_tree_class_name)];
    let mut subtree_range_statement = vec!["    std::abort();".into()];
    let mut subtree_component_statement = vec!["    std::abort();".into()];

    if target_struct.members.iter().any(|(_, declaration)| {
        matches!(&declaration, Declaration::Function(func @ Function { .. }) if func.name == "visit_dynamic_children")
    }) {
        visit_children_statements
            .push("    return self->visit_dynamic_children(dyn_index, order, visitor);".into());
        subtree_range_statement = vec![
                format!("auto self = reinterpret_cast<const {}*>(component.instance);", item_tree_class_name),
                "return self->subtree_range(dyn_index);".to_owned(),
        ];
        subtree_component_statement = vec![
                format!("auto self = reinterpret_cast<const {}*>(component.instance);", item_tree_class_name),
                "self->subtree_component(dyn_index, subtree_index, result);".to_owned(),
        ];
    } else {
        visit_children_statements.push("    std::abort();".into());
     }

    visit_children_statements.extend([
        "};".into(),
        format!("auto self_rc = reinterpret_cast<const {}*>(component.instance)->self_weak.lock()->into_dyn();", item_tree_class_name),
        "return slint::cbindgen_private::slint_visit_item_tree(&self_rc, get_item_tree(component) , index, order, visitor, dyn_visit);".to_owned(),
    ]);

    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "visit_children".into(),
            signature: "(slint::private_api::ComponentRef component, intptr_t index, slint::private_api::TraversalOrder order, slint::private_api::ItemVisitorRefMut visitor) -> uint64_t".into(),
            is_static: true,
            statements: Some(visit_children_statements),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "get_item_ref".into(),
            signature: "(slint::private_api::ComponentRef component, uintptr_t index) -> slint::private_api::ItemRef".into(),
            is_static: true,
            statements: Some(vec![
                "return slint::private_api::get_item_ref(component, get_item_tree(component), item_array(), index);".to_owned(),
            ]),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "get_subtree_range".into(),
            signature: "([[maybe_unused]] slint::private_api::ComponentRef component, [[maybe_unused]] uintptr_t dyn_index) -> slint::private_api::IndexRange".into(),
            is_static: true,
            statements: Some(subtree_range_statement),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "get_subtree_component".into(),
            signature: "([[maybe_unused]] slint::private_api::ComponentRef component, [[maybe_unused]] uintptr_t dyn_index, [[maybe_unused]] uintptr_t subtree_index, [[maybe_unused]] slint::private_api::ComponentWeak *result) -> void".into(),
            is_static: true,
            statements: Some(subtree_component_statement),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "get_item_tree".into(),
            signature: "(slint::private_api::ComponentRef) -> slint::cbindgen_private::Slice<slint::private_api::ItemTreeNode>".into(),
            is_static: true,
            statements: Some(vec![
                "return item_tree();".to_owned(),
            ]),
            ..Default::default()
        }),
    ));

    let parent_item_from_parent_component = parent_ctx.as_ref()
        .and_then(|parent| {
            parent
                .repeater_index
                .map(|idx| parent.ctx.current_sub_component.unwrap().repeated[idx].index_in_tree)
        }).map(|parent_index|
            vec![
                format!(
                    "auto self = reinterpret_cast<const {}*>(component.instance);",
                    item_tree_class_name,
                ),
                format!(
                    "*result = {{ self->parent->self_weak, self->parent->tree_index_of_first_child + {} - 1 }};",
                    parent_index,
                )
            ])
        .unwrap_or_default();
    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "parent_node".into(),
            signature: "([[maybe_unused]] slint::private_api::ComponentRef component, [[maybe_unused]] slint::private_api::ItemWeak *result) -> void".into(),
            is_static: true,
            statements: Some(parent_item_from_parent_component,),
            ..Default::default()
        }),
    ));

    // Statements will be overridden for repeated components!
    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "subtree_index".into(),
            signature: "([[maybe_unused]] slint::private_api::ComponentRef component) -> uintptr_t"
                .into(),
            is_static: true,
            statements: Some(vec!["return std::numeric_limits<uintptr_t>::max();".into()]),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "item_tree".into(),
            signature: "() -> slint::cbindgen_private::Slice<slint::private_api::ItemTreeNode>".into(),
            is_static: true,
            statements: Some(vec![
                "static const slint::private_api::ItemTreeNode children[] {".to_owned(),
                format!("    {} }};", item_tree_array.join(", \n")),
                "return { const_cast<slint::private_api::ItemTreeNode*>(children), std::size(children) };"
                    .to_owned(),
            ]),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "item_array".into(),
            signature: "() -> const slint::private_api::ItemArray".into(),
            is_static: true,
            statements: Some(vec![
                "static const slint::private_api::ItemArrayEntry items[] {".to_owned(),
                format!("    {} }};", item_array.join(", \n")),
                "return { const_cast<slint::private_api::ItemArrayEntry*>(items), std::size(items) };"
                    .to_owned(),
            ]),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "layout_info".into(),
            signature:
                "([[maybe_unused]] slint::private_api::ComponentRef component, slint::cbindgen_private::Orientation o) -> slint::cbindgen_private::LayoutInfo"
                    .into(),
            is_static: true,
            statements: Some(vec![format!(
                "return reinterpret_cast<const {}*>(component.instance)->layout_info(o);",
                item_tree_class_name
            )]),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "accessible_role".into(),
            signature:
                "([[maybe_unused]] slint::private_api::ComponentRef component, uintptr_t index) -> slint::cbindgen_private::AccessibleRole"
                    .into(),
            is_static: true,
            statements: Some(vec![format!(
                "return reinterpret_cast<const {}*>(component.instance)->accessible_role(index);",
                item_tree_class_name
            )]),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "accessible_string_property".into(),
            signature:
                "([[maybe_unused]] slint::private_api::ComponentRef component, uintptr_t index, slint::cbindgen_private::AccessibleStringProperty what, slint::SharedString *result) -> void"
                    .into(),
            is_static: true,
            statements: Some(vec![format!(
                "*result = reinterpret_cast<const {}*>(component.instance)->accessible_string_property(index, what);",
                item_tree_class_name
            )]),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        Access::Public,
        Declaration::Var(Var {
            ty: "static const slint::private_api::ComponentVTable".to_owned(),
            name: "static_vtable".to_owned(),
            ..Default::default()
        }),
    ));

    file.definitions.push(Declaration::Var(Var {
        ty: "const slint::private_api::ComponentVTable".to_owned(),
        name: format!("{}::static_vtable", item_tree_class_name),
        init: Some(format!(
            "{{ visit_children, get_item_ref, get_subtree_range, get_subtree_component, \
                get_item_tree, parent_node, subtree_index, layout_info, \
                accessible_role, accessible_string_property, \
                slint::private_api::drop_in_place<{}>, slint::private_api::dealloc }}",
            item_tree_class_name
        )),
        ..Default::default()
    }));

    let mut create_parameters = Vec::new();
    let mut init_parent_parameters = "";

    if let Some(parent) = &parent_ctx {
        let parent_type =
            format!("class {} const *", ident(&parent.ctx.current_sub_component.unwrap().name));
        create_parameters.push(format!("{} parent", parent_type));

        init_parent_parameters = ", parent";
    }

    let mut create_code = vec![
        format!(
            "auto self_rc = vtable::VRc<slint::private_api::ComponentVTable, {0}>::make();",
            target_struct.name
        ),
        format!("auto self = const_cast<{0} *>(&*self_rc);", target_struct.name),
        "self->self_weak = vtable::VWeak(self_rc).into_dyn();".into(),
    ];

    if parent_ctx.is_none() {
        create_code.extend([format!(
            "{}->m_window.window_handle().set_component(*self_rc);",
            root_access
        )]);
    }

    create_code.extend([
        format!(
            "{}->m_window.window_handle().register_component(self, self->item_array());",
            root_access
        ),
        format!("self->init({}, self->self_weak, 0, 1 {});", root_access, init_parent_parameters),
        format!("return slint::ComponentHandle<{0}>{{ self_rc }};", target_struct.name),
    ]);

    target_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "create".into(),
            signature: format!(
                "({}) -> slint::ComponentHandle<{}>",
                create_parameters.join(","),
                target_struct.name
            ),
            statements: Some(create_code),
            is_static: true,
            ..Default::default()
        }),
    ));

    let mut destructor = vec!["auto self = this;".to_owned()];

    destructor.push(format!(
        "{}->m_window.window_handle().unregister_component(self, item_array());",
        root_access
    ));

    target_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: format!("~{}", target_struct.name),
            signature: "()".to_owned(),
            is_constructor_or_destructor: true,
            statements: Some(destructor),
            ..Default::default()
        }),
    ));
}

fn generate_sub_component(
    target_struct: &mut Struct,
    component: &llr::SubComponent,
    root: &llr::PublicComponent,
    parent_ctx: Option<ParentCtx>,
    field_access: Access,
    file: &mut File,
) {
    let root_ptr_type = format!("const {} *", ident(&root.item_tree.root.name));

    let mut init_parameters = vec![
        format!("{} root", root_ptr_type),
        "slint::cbindgen_private::ComponentWeak enclosing_component".into(),
        "uintptr_t tree_index".into(),
        "uintptr_t tree_index_of_first_child".into(),
    ];

    let mut init: Vec<String> =
        vec!["auto self = this;".into(), "self->self_weak = enclosing_component;".into()];

    target_struct.members.push((
        Access::Public,
        Declaration::Var(Var {
            ty: "slint::cbindgen_private::ComponentWeak".into(),
            name: "self_weak".into(),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        field_access,
        Declaration::Var(Var { ty: root_ptr_type, name: "root".to_owned(), ..Default::default() }),
    ));
    init.push("self->root = root;".into());

    target_struct.members.push((
        field_access,
        Declaration::Var(Var {
            ty: "uintptr_t".to_owned(),
            name: "tree_index_of_first_child".to_owned(),
            ..Default::default()
        }),
    ));
    init.push("this->tree_index_of_first_child = tree_index_of_first_child;".into());

    target_struct.members.push((
        field_access,
        Declaration::Var(Var {
            ty: "uintptr_t".to_owned(),
            name: "tree_index".to_owned(),
            ..Default::default()
        }),
    ));
    init.push("self->tree_index = tree_index;".into());

    if let Some(parent_ctx) = &parent_ctx {
        let parent_type =
            format!("class {} const *", ident(&parent_ctx.ctx.current_sub_component.unwrap().name));
        init_parameters.push(format!("{} parent", parent_type));

        target_struct.members.push((
            field_access,
            Declaration::Var(Var {
                ty: parent_type,
                name: "parent".to_owned(),
                ..Default::default()
            }),
        ));
        init.push("self->parent = parent;".into());
    }

    let ctx = EvaluationContext::new_sub_component(
        root,
        component,
        "self->root".into(),
        parent_ctx.clone(),
    );

    component.popup_windows.iter().for_each(|c| {
        let component_id = ident(&c.root.name);
        let mut popup_struct = Struct { name: component_id.clone(), ..Default::default() };
        generate_item_tree(
            &mut popup_struct,
            c,
            root,
            Some(ParentCtx::new(&ctx, None)),
            component_id,
            Access::Public,
            file,
        );
        file.definitions.extend(popup_struct.extract_definitions().collect::<Vec<_>>());
        file.declarations.push(Declaration::Struct(popup_struct));
    });

    for property in component.properties.iter().filter(|p| p.use_count.get() > 0) {
        let cpp_name = ident(&property.name);

        let ty = if let Type::Callback { args, return_type } = &property.ty {
            let param_types = args.iter().map(|t| t.cpp_type().unwrap()).collect::<Vec<_>>();
            let return_type =
                return_type.as_ref().map_or("void".to_owned(), |t| t.cpp_type().unwrap());
            format!("slint::private_api::Callback<{}({})>", return_type, param_types.join(", "))
        } else {
            format!("slint::private_api::Property<{}>", property.ty.cpp_type().unwrap())
        };

        target_struct.members.push((
            field_access,
            Declaration::Var(Var { ty, name: cpp_name, ..Default::default() }),
        ));
    }

    let mut children_visitor_cases = Vec::new();
    let mut subtrees_ranges_cases = Vec::new();
    let mut subtrees_components_cases = Vec::new();

    for sub in &component.sub_components {
        let field_name = ident(&sub.name);
        let local_tree_index: u32 = sub.index_in_tree as _;
        let local_index_of_first_child: u32 = sub.index_of_first_child_in_tree as _;

        // For children of sub-components, the item index generated by the generate_item_indices pass
        // starts at 1 (0 is the root element).
        let global_index = if local_tree_index == 0 {
            "tree_index".into()
        } else {
            format!("tree_index_of_first_child + {} - 1", local_tree_index)
        };
        let global_children = if local_index_of_first_child == 0 {
            "0".into()
        } else {
            format!("tree_index_of_first_child + {} - 1", local_index_of_first_child)
        };

        init.push(format!(
            "this->{}.init(root, self_weak.into_dyn(), {}, {});",
            field_name, global_index, global_children
        ));

        let sub_component_repeater_count = sub.ty.repeater_count();
        if sub_component_repeater_count > 0 {
            let mut case_code = String::new();
            let repeater_offset = sub.repeater_offset;

            for local_repeater_index in 0..sub_component_repeater_count {
                write!(case_code, "case {}: ", repeater_offset + local_repeater_index).unwrap();
            }

            children_visitor_cases.push(format!(
                "\n        {case_code} {{
                        return self->{id}.visit_dynamic_children(dyn_index - {base}, order, visitor);
                    }}",
                case_code = case_code,
                id = field_name,
                base = repeater_offset,
            ));
            subtrees_ranges_cases.push(format!(
                "\n        {case_code} {{
                        return self->{id}.subtree_range(dyn_index - {base});
                    }}",
                case_code = case_code,
                id = field_name,
                base = repeater_offset,
            ));
            subtrees_components_cases.push(format!(
                "\n        {case_code} {{
                        self->{id}.subtree_component(dyn_index - {base}, subtree_index, result);
                        return;
                    }}",
                case_code = case_code,
                id = field_name,
                base = repeater_offset,
            ));
        }

        target_struct.members.push((
            field_access,
            Declaration::Var(Var {
                ty: ident(&sub.ty.name),
                name: field_name,
                ..Default::default()
            }),
        ));
    }

    for (prop1, prop2) in &component.two_way_bindings {
        init.push(format!(
            "slint::private_api::Property<{ty}>::link_two_way(&{p1}, &{p2});",
            ty = ctx.property_ty(prop1).cpp_type().unwrap(),
            p1 = access_member(prop1, &ctx),
            p2 = access_member(prop2, &ctx),
        ));
    }

    let mut properties_init_code = Vec::new();
    for (prop, expression) in &component.property_init {
        if expression.use_count.get() > 0 {
            handle_property_init(prop, expression, &mut properties_init_code, &ctx)
        }
    }

    for item in &component.items {
        if item.is_flickable_viewport {
            continue;
        }
        target_struct.members.push((
            field_access,
            Declaration::Var(Var {
                ty: format!("slint::cbindgen_private::{}", ident(&item.ty.class_name)),
                name: ident(&item.name),
                init: Some("{}".to_owned()),
                ..Default::default()
            }),
        ));
    }

    for (idx, repeated) in component.repeated.iter().enumerate() {
        let data_type = if let Some(data_prop) = repeated.data_prop {
            repeated.sub_tree.root.properties[data_prop].ty.clone()
        } else {
            Type::Int32
        };

        generate_repeated_component(
            repeated,
            root,
            ParentCtx::new(&ctx, Some(idx)),
            &data_type,
            file,
        );

        let repeater_id = format!("repeater_{}", idx);

        let mut model = compile_expression(&repeated.model.borrow(), &ctx);
        if repeated.model.ty(&ctx) == Type::Bool {
            // bool converts to int
            // FIXME: don't do a heap allocation here
            model = format!("std::make_shared<slint::private_api::IntModel>({})", model)
        }

        // FIXME: optimize  if repeated.model.is_constant()
        properties_init_code.push(format!(
            "self->{repeater_id}.set_model_binding([self] {{ (void)self; return {model}; }});",
            repeater_id = repeater_id,
            model = model,
        ));

        let ensure_updated = if let Some(listview) = &repeated.listview {
            let vp_y = access_member(&listview.viewport_y, &ctx);
            let vp_h = access_member(&listview.viewport_height, &ctx);
            let lv_h = access_member(&listview.listview_height, &ctx);
            let vp_w = access_member(&listview.viewport_width, &ctx);
            let lv_w = access_member(&listview.listview_width, &ctx);

            format!(
                "self->{}.ensure_updated_listview(self, &{}, &{}, &{}, {}.get(), {}.get());",
                repeater_id, vp_w, vp_h, vp_y, lv_w, lv_h
            )
        } else {
            format!("self->{id}.ensure_updated(self);", id = repeater_id)
        };

        children_visitor_cases.push(format!(
            "\n        case {i}: {{
                {e_u}
                return self->{id}.visit(order, visitor);
            }}",
            id = repeater_id,
            i = idx,
            e_u = ensure_updated,
        ));
        subtrees_ranges_cases.push(format!(
            "\n        case {i}: {{
                {e_u}
                return self->{id}.index_range();
            }}",
            i = idx,
            e_u = ensure_updated,
            id = repeater_id,
        ));
        subtrees_components_cases.push(format!(
            "\n        case {i}: {{
                {e_u}
                *result = self->{id}.component_at(subtree_index);
                return;
            }}",
            i = idx,
            e_u = ensure_updated,
            id = repeater_id,
        ));

        target_struct.members.push((
            field_access,
            Declaration::Var(Var {
                ty: format!(
                    "slint::private_api::Repeater<class {}, {}>",
                    ident(&repeated.sub_tree.root.name),
                    data_type.cpp_type().unwrap(),
                ),
                name: repeater_id,
                ..Default::default()
            }),
        ));
    }

    init.extend(properties_init_code);
    init.extend(component.init_code.iter().map(|e| compile_expression(&e.borrow(), &ctx)));

    target_struct.members.push((
        field_access,
        Declaration::Function(Function {
            name: "init".to_owned(),
            signature: format!("({}) -> void", init_parameters.join(",")),
            statements: Some(init),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        field_access,
        Declaration::Function(Function {
            name: "layout_info".into(),
            signature: "(slint::cbindgen_private::Orientation o) const -> slint::cbindgen_private::LayoutInfo"
                .into(),
            statements: Some(vec![
                "[[maybe_unused]] auto self = this;".into(),
                format!(
                    "return o == slint::cbindgen_private::Orientation::Horizontal ? {} : {};",
                    compile_expression(&component.layout_info_h.borrow(), &ctx),
                    compile_expression(&component.layout_info_v.borrow(), &ctx)
                ),
            ]),
            ..Default::default()
        }),
    ));

    let mut accessible_function = |name: &str,
                                   signature: &str,
                                   forward_args: &str,
                                   code: Vec<String>| {
        let mut code = ["[[maybe_unused]] auto self = this;".into()]
            .into_iter()
            .chain(code.into_iter())
            .collect::<Vec<_>>();

        let mut else_ = "";
        for sub in &component.sub_components {
            let sub_items_count = sub.ty.child_item_count();
            code.push(format!("{else_}if (index == {}) {{", sub.index_in_tree,));
            code.push(format!("    return self->{}.{name}(0{forward_args});", ident(&sub.name)));
            if sub_items_count > 1 {
                code.push(format!(
                    "}} else if (index >= {} && index < {}) {{",
                    sub.index_of_first_child_in_tree,
                    sub.index_of_first_child_in_tree + sub_items_count - 1
                ));
                code.push(format!(
                    "    return self->{}.{name}(index - {}{forward_args});",
                    ident(&sub.name),
                    sub.index_of_first_child_in_tree - 1
                ));
            }
            else_ = "} else ";
        }
        code.push(format!("{else_}return {{}};"));
        target_struct.members.push((
            field_access,
            Declaration::Function(Function {
                name: name.into(),
                signature: signature.into(),
                statements: Some(code),
                ..Default::default()
            }),
        ));
    };

    let mut accessible_role_cases = vec!["switch (index) {".into()];
    let mut accessible_string_cases = vec!["switch ((index << 8) | uintptr_t(what)) {".into()];
    for ((index, what), expr) in &component.accessible_prop {
        let expr = compile_expression(&expr.borrow(), &ctx);
        if what == "Role" {
            accessible_role_cases.push(format!("    case {index}: return {expr};"));
        } else {
            accessible_string_cases.push(format!("    case ({index} << 8) | uintptr_t(slint::cbindgen_private::AccessibleStringProperty::{what}): return {expr};"));
        }
    }
    accessible_role_cases.push("}".into());
    accessible_string_cases.push("}".into());

    accessible_function(
        "accessible_role",
        "(uintptr_t index) const -> slint::cbindgen_private::AccessibleRole",
        "",
        accessible_role_cases,
    );
    accessible_function(
        "accessible_string_property",
        "(uintptr_t index, slint::cbindgen_private::AccessibleStringProperty what) const -> slint::SharedString",
        ", what",
        accessible_string_cases,
    );

    if !children_visitor_cases.is_empty() {
        target_struct.members.push((
            field_access,
            Declaration::Function(Function {
                name: "visit_dynamic_children".into(),
                signature: "(intptr_t dyn_index, [[maybe_unused]] slint::private_api::TraversalOrder order, [[maybe_unused]] slint::private_api::ItemVisitorRefMut visitor) const -> uint64_t".into(),
                statements: Some(vec![
                    "    auto self = this;".to_owned(),
                    format!("    switch(dyn_index) {{ {} }};", children_visitor_cases.join("")),
                    "    std::abort();".to_owned(),
                ]),
                ..Default::default()
            }),
        ));
        target_struct.members.push((
            field_access,
            Declaration::Function(Function {
                name: "subtree_range".into(),
                signature: "(uintptr_t dyn_index) const -> slint::private_api::IndexRange".into(),
                statements: Some(vec![
                    "[[maybe_unused]] auto self = this;".to_owned(),
                    format!("    switch(dyn_index) {{ {} }};", subtrees_ranges_cases.join("")),
                    "    std::abort();".to_owned(),
                ]),
                ..Default::default()
            }),
        ));
        target_struct.members.push((
            field_access,
            Declaration::Function(Function {
                name: "subtree_component".into(),
                signature: "(uintptr_t dyn_index, [[maybe_unused]] uintptr_t subtree_index, [[maybe_unused]] slint::private_api::ComponentWeak *result) const -> void".into(),
                statements: Some(vec![
                    "[[maybe_unused]] auto self = this;".to_owned(),
                    format!("    switch(dyn_index) {{ {} }};", subtrees_components_cases.join("")),
                    "    std::abort();".to_owned(),
                ]),
                ..Default::default()
            }),
        ));
    }
}

fn generate_repeated_component(
    repeated: &llr::RepeatedElement,
    root: &llr::PublicComponent,
    parent_ctx: ParentCtx,
    model_data_type: &Type,
    file: &mut File,
) {
    let repeater_id = ident(&repeated.sub_tree.root.name);
    let mut repeater_struct = Struct { name: repeater_id.clone(), ..Default::default() };
    generate_item_tree(
        &mut repeater_struct,
        &repeated.sub_tree,
        root,
        Some(parent_ctx.clone()),
        repeater_id.clone(),
        Access::Public,
        file,
    );

    let ctx = EvaluationContext {
        public_component: root,
        current_sub_component: Some(&repeated.sub_tree.root),
        current_global: None,
        generator_state: "self".into(),
        parent: Some(parent_ctx),
        argument_types: &[],
    };

    let access_prop = |&property_index| {
        access_member(
            &llr::PropertyReference::Local { sub_component_path: vec![], property_index },
            &ctx,
        )
    };
    let index_prop = repeated.index_prop.iter().map(access_prop);
    let data_prop = repeated.data_prop.iter().map(access_prop);

    let mut update_statements = vec!["[[maybe_unused]] auto self = this;".into()];
    update_statements.extend(index_prop.map(|prop| format!("{}.set(i);", prop)));
    update_statements.extend(data_prop.map(|prop| format!("{}.set(data);", prop)));

    repeater_struct.members.push((
        Access::Public, // Because Repeater accesses it
        Declaration::Function(Function {
            name: "update_data".into(),
            signature: format!(
                "([[maybe_unused]] int i, [[maybe_unused]] const {} &data) const -> void",
                model_data_type.cpp_type().unwrap()
            ),
            statements: Some(update_statements),
            ..Function::default()
        }),
    ));

    if let Some(listview) = &repeated.listview {
        let p_y = access_member(&listview.prop_y, &ctx);
        let p_height = access_member(&listview.prop_height, &ctx);
        let p_width = access_member(&listview.prop_width, &ctx);

        repeater_struct.members.push((
            Access::Public, // Because Repeater accesses it
            Declaration::Function(Function {
                name: "listview_layout".into(),
                signature:
                    "(float *offset_y, const slint::private_api::Property<float> *viewport_width) const -> void"
                        .to_owned(),
                statements: Some(vec![
                    "[[maybe_unused]] auto self = this;".into(),
                    "float vp_w = viewport_width->get();".to_owned(),

                    format!("{}.set(*offset_y);", p_y), // FIXME: shouldn't that be handled by apply layout?
                    format!("*offset_y += {}.get();", p_height),
                    format!("float w = {}.get();", p_width),
                    "if (vp_w < w)".to_owned(),
                    "    viewport_width->set(w);".to_owned(),
                ]),
                ..Function::default()
            }),
        ));
    } else {
        repeater_struct.members.push((
            Access::Public, // Because Repeater accesses it
            Declaration::Function(Function {
                name: "box_layout_data".into(),
                signature: "(slint::cbindgen_private::Orientation o) const -> slint::cbindgen_private::BoxLayoutCellData".to_owned(),
                statements: Some(vec!["return { layout_info({&static_vtable, const_cast<void *>(static_cast<const void *>(this))}, o) };".into()]),

                ..Function::default()
            }),
        ));
    }

    if let Some(index_prop) = repeated.index_prop {
        // Override default subtree_index function implementation
        let subtree_index_func = repeater_struct
            .members
            .iter_mut()
            .find(|(_, d)| matches!(d, Declaration::Function(f) if f.name == "subtree_index"));

        if let Declaration::Function(f) = &mut subtree_index_func.unwrap().1 {
            let index = access_prop(&index_prop);
            f.statements = Some(vec![
                format!(
                    "auto self = reinterpret_cast<const {}*>(component.instance);",
                    repeater_id
                ),
                format!("return {}.get();", index),
            ]);
        }
    }

    file.definitions.extend(repeater_struct.extract_definitions().collect::<Vec<_>>());
    file.declarations.push(Declaration::Struct(repeater_struct));
}

fn generate_global(file: &mut File, global: &llr::GlobalComponent, root: &llr::PublicComponent) {
    let mut global_struct = Struct { name: ident(&global.name), ..Default::default() };

    for property in global.properties.iter().filter(|p| p.use_count.get() > 0) {
        let cpp_name = ident(&property.name);

        let ty = if let Type::Callback { args, return_type } = &property.ty {
            let param_types = args.iter().map(|t| t.cpp_type().unwrap()).collect::<Vec<_>>();
            let return_type =
                return_type.as_ref().map_or("void".to_owned(), |t| t.cpp_type().unwrap());
            format!("slint::private_api::Callback<{}({})>", return_type, param_types.join(", "))
        } else {
            format!("slint::private_api::Property<{}>", property.ty.cpp_type().unwrap())
        };

        global_struct.members.push((
            // FIXME: this is public (and also was public in the pre-llr generator) because other generated code accesses the
            // fields directly. But it shouldn't be from an API point of view since the same `global_struct` class is public API
            // when the global is exported and exposed in the public component.
            Access::Public,
            Declaration::Var(Var { ty, name: cpp_name, ..Default::default() }),
        ));
    }

    let mut init = vec!["(void)this->root;".into()];

    let ctx = EvaluationContext::new_global(root, global, "this->root".into());

    for (property_index, expression) in global.init_values.iter().enumerate() {
        if global.properties[property_index].use_count.get() == 0 {
            continue;
        }

        if let Some(expression) = expression.as_ref() {
            handle_property_init(
                &llr::PropertyReference::Local { sub_component_path: vec![], property_index },
                expression,
                &mut init,
                &ctx,
            )
        }
    }

    let root_ptr_type = format!("const {} *", ident(&root.item_tree.root.name));
    global_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: ident(&global.name),
            signature: format!("({} root)", root_ptr_type),
            is_constructor_or_destructor: true,
            statements: Some(init),
            constructor_member_initializers: vec!["root(root)".into()],
            ..Default::default()
        }),
    ));
    global_struct.members.push((
        Access::Private,
        Declaration::Var(Var { ty: root_ptr_type, name: "root".to_owned(), ..Default::default() }),
    ));

    let declarations = generate_public_api_for_properties(&global.public_properties, &ctx);
    global_struct.members.extend(declarations.into_iter().map(|decl| (Access::Public, decl)));

    file.definitions.extend(global_struct.extract_definitions().collect::<Vec<_>>());
    file.declarations.push(Declaration::Struct(global_struct));
}

fn generate_public_api_for_properties(
    public_properties: &llr::PublicProperties,
    ctx: &EvaluationContext,
) -> Vec<Declaration> {
    let mut declarations = Vec::new();
    for (p, (ty, r)) in public_properties.iter() {
        let prop_ident = ident(p);

        let access = access_member(r, ctx);

        if let Type::Callback { args, return_type } = ty {
            let param_types = args.iter().map(|t| t.cpp_type().unwrap()).collect::<Vec<_>>();
            let return_type = return_type.as_ref().map_or("void".into(), |t| t.cpp_type().unwrap());
            let callback_emitter = vec![
                "[[maybe_unused]] auto self = this;".into(),
                format!(
                    "return {}.call({});",
                    access,
                    (0..args.len()).map(|i| format!("arg_{}", i)).join(", ")
                ),
            ];
            declarations.push(Declaration::Function(Function {
                name: format!("invoke_{}", ident(p)),
                signature: format!(
                    "({}) const -> {}",
                    param_types
                        .iter()
                        .enumerate()
                        .map(|(i, ty)| format!("{} arg_{}", ty, i))
                        .join(", "),
                    return_type
                ),
                statements: Some(callback_emitter),
                ..Default::default()
            }));
            declarations.push(Declaration::Function(Function {
                name: format!("on_{}", ident(p)),
                template_parameters: Some("typename Functor".into()),
                signature: "(Functor && callback_handler) const".into(),
                statements: Some(vec![
                    "[[maybe_unused]] auto self = this;".into(),
                    format!("{}.set_handler(std::forward<Functor>(callback_handler));", access),
                ]),
                ..Default::default()
            }));
        } else {
            let cpp_property_type = ty.cpp_type().expect("Invalid type in public properties");
            let prop_getter: Vec<String> = vec![
                "[[maybe_unused]] auto self = this;".into(),
                format!("return {}.get();", access),
            ];
            declarations.push(Declaration::Function(Function {
                name: format!("get_{}", &prop_ident),
                signature: format!("() const -> {}", &cpp_property_type),
                statements: Some(prop_getter),
                ..Default::default()
            }));

            let prop_setter: Vec<String> = vec![
                "[[maybe_unused]] auto self = this;".into(),
                property_set_value_code(r, "value", ctx) + ";",
            ];
            declarations.push(Declaration::Function(Function {
                name: format!("set_{}", &prop_ident),
                signature: format!("(const {} &value) const", &cpp_property_type),
                statements: Some(prop_setter),
                ..Default::default()
            }));
        }
    }
    declarations
}

fn follow_sub_component_path<'a>(
    root: &'a llr::SubComponent,
    sub_component_path: &[usize],
) -> (String, &'a llr::SubComponent) {
    let mut compo_path = String::new();
    let mut sub_component = root;
    for i in sub_component_path {
        let sub_component_name = ident(&sub_component.sub_components[*i].name);
        write!(compo_path, "{}.", sub_component_name).unwrap();
        sub_component = &sub_component.sub_components[*i].ty;
    }
    (compo_path, sub_component)
}

fn access_window_field(ctx: &EvaluationContext) -> String {
    let root = &ctx.generator_state;
    format!("{}->window().window_handle()", root)
}

/// Returns the code that can access the given property (but without the set or get)
///
/// to be used like:
/// ```ignore
/// let access = access_member(...);
/// format!("{}.get()", access)
/// ```
fn access_member(reference: &llr::PropertyReference, ctx: &EvaluationContext) -> String {
    fn in_native_item(
        ctx: &EvaluationContext,
        sub_component_path: &[usize],
        item_index: usize,
        prop_name: &str,
        path: &str,
    ) -> String {
        let (compo_path, sub_component) =
            follow_sub_component_path(ctx.current_sub_component.unwrap(), sub_component_path);
        let item_name = ident(&sub_component.items[item_index].name);
        if prop_name.is_empty() {
            // then this is actually a reference to the element itself
            format!("{}->{}{}", path, compo_path, item_name)
        } else {
            let property_name = ident(prop_name);
            let flick = sub_component.items[item_index]
                .is_flickable_viewport
                .then(|| "viewport.")
                .unwrap_or_default();
            format!("{}->{}{}.{}{}", path, compo_path, item_name, flick, property_name)
        }
    }

    match reference {
        llr::PropertyReference::Local { sub_component_path, property_index } => {
            if let Some(sub_component) = ctx.current_sub_component {
                let (compo_path, sub_component) =
                    follow_sub_component_path(sub_component, sub_component_path);
                let property_name = ident(&sub_component.properties[*property_index].name);
                format!("self->{}{}", compo_path, property_name)
            } else if let Some(current_global) = ctx.current_global {
                format!("this->{}", ident(&current_global.properties[*property_index].name))
            } else {
                unreachable!()
            }
        }
        llr::PropertyReference::InNativeItem { sub_component_path, item_index, prop_name } => {
            in_native_item(ctx, sub_component_path, *item_index, prop_name, "self")
        }
        llr::PropertyReference::InParent { level, parent_reference } => {
            let mut ctx = ctx;
            let mut path = "self".to_string();
            for _ in 0..level.get() {
                write!(path, "->parent").unwrap();
                ctx = ctx.parent.as_ref().unwrap().ctx;
            }

            match &**parent_reference {
                llr::PropertyReference::Local { sub_component_path, property_index } => {
                    let sub_component = ctx.current_sub_component.unwrap();
                    let (compo_path, sub_component) =
                        follow_sub_component_path(sub_component, sub_component_path);
                    let property_name = ident(&sub_component.properties[*property_index].name);
                    format!("{}->{}{}", path, compo_path, property_name)
                }
                llr::PropertyReference::InNativeItem {
                    sub_component_path,
                    item_index,
                    prop_name,
                } => in_native_item(ctx, sub_component_path, *item_index, prop_name, &path),
                llr::PropertyReference::InParent { .. } | llr::PropertyReference::Global { .. } => {
                    unreachable!()
                }
            }
        }
        llr::PropertyReference::Global { global_index, property_index } => {
            let root_access = &ctx.generator_state;
            let global = &ctx.public_component.globals[*global_index];
            let global_id = format!("global_{}", ident(&global.name));
            let property_name = ident(
                &ctx.public_component.globals[*global_index].properties[*property_index].name,
            );
            format!("{}->{}->{}", root_access, global_id, property_name)
        }
    }
}

/// Returns the NativeClass for a PropertyReference::InNativeItem
/// (or a InParent of InNativeItem )
fn native_item<'a>(
    item_ref: &llr::PropertyReference,
    ctx: &'a EvaluationContext,
) -> &'a NativeClass {
    match item_ref {
        llr::PropertyReference::InNativeItem { sub_component_path, item_index, prop_name: _ } => {
            let mut sub_component = ctx.current_sub_component.unwrap();
            for i in sub_component_path {
                sub_component = &sub_component.sub_components[*i].ty;
            }
            &sub_component.items[*item_index].ty
        }
        llr::PropertyReference::InParent { level, parent_reference } => {
            let mut ctx = ctx;
            for _ in 0..level.get() {
                ctx = ctx.parent.as_ref().unwrap().ctx;
            }
            native_item(parent_reference, ctx)
        }
        _ => unreachable!(),
    }
}

fn compile_expression(expr: &llr::Expression, ctx: &EvaluationContext) -> String {
    use llr::Expression;
    match expr {
        Expression::StringLiteral(s) => {
            format!(r#"slint::SharedString(u8"{}")"#, escape_string(s.as_str()))
        }
        Expression::NumberLiteral(num) => {
            if *num > 1_000_000_000. {
                // If the numbers are too big, decimal notation will give too many digit
                format!("{:+e}", num)
            } else {
                num.to_string()
            }
        }
        Expression::BoolLiteral(b) => b.to_string(),
        Expression::PropertyReference(nr) => {
            let access = access_member(nr, ctx);
            format!(r#"{}.get()"#, access)
        }
        Expression::BuiltinFunctionCall { function, arguments } => {
            compile_builtin_function_call(*function, arguments, ctx)
        }
        Expression::CallBackCall{ callback, arguments } => {
            let f = access_member(callback, ctx);
            let mut a = arguments.iter().map(|a| compile_expression(a, ctx));
            format!("{}.call({})", f, a.join(","))
        }
        Expression::ExtraBuiltinFunctionCall { function, arguments, return_ty: _ } => {
            let mut a = arguments.iter().map(|a| compile_expression(a, ctx));
            format!("slint::private_api::{}({})", ident(function), a.join(","))
        }
        Expression::FunctionParameterReference { index, .. } => format!("arg_{}", index),
        Expression::StoreLocalVariable { name, value } => {
            format!("auto {} = {};", ident(name), compile_expression(value, ctx))
        }
        Expression::ReadLocalVariable { name, .. } => ident(name),
        Expression::StructFieldAccess { base, name } => match base.ty(ctx) {
            Type::Struct { fields, name : None, .. } => {
                let index = fields
                    .keys()
                    .position(|k| k == name)
                    .expect("Expression::ObjectAccess: Cannot find a key in an object");
                format!("std::get<{}>({})", index, compile_expression(base, ctx))
            }
            Type::Struct{..} => {
                format!("{}.{}", compile_expression(base, ctx), ident(name))
            }
            _ => panic!("Expression::ObjectAccess's base expression is not an Object type"),
        },
        Expression::ArrayIndex { array, index } => {
            format!(
                "slint::private_api::access_array_index({}, {})",
                compile_expression(array, ctx), compile_expression(index, ctx)
            )
        },
        Expression::Cast { from, to } => {
            let f = compile_expression(&*from, ctx);
            match (from.ty(ctx), to) {
                (from, Type::String) if from.as_unit_product().is_some() => {
                    format!("slint::SharedString::from_number({})", f)
                }
                (Type::Float32, Type::Model) | (Type::Int32, Type::Model) => {
                    format!("std::make_shared<slint::private_api::IntModel>({})", f)
                }
                (Type::Array(_), Type::Model) => f,
                (Type::Float32, Type::Color) => {
                    format!("slint::Color::from_argb_encoded({})", f)
                }
                (Type::Color, Type::Brush) => {
                    format!("slint::Brush({})", f)
                }
                (Type::Brush, Type::Color) => {
                    format!("{}.color()", f)
                }
                (Type::Struct { .. }, Type::Struct{ fields, name: Some(_), ..}) => {
                    format!(
                        "[&](const auto &o){{ {struct_name} s; {fields} return s; }}({obj})",
                        struct_name = to.cpp_type().unwrap(),
                        fields = fields.keys().enumerate().map(|(i, n)| format!("s.{} = std::get<{}>(o); ", ident(n), i)).join(""),
                        obj = f,
                    )
                }
                (Type::Array(..), Type::PathData)
                    if matches!(
                        from.as_ref(),
                        Expression::Array { element_ty: Type::Struct { .. }, .. }
                    ) =>
                {
                    let path_elements = match from.as_ref() {
                        Expression::Array { element_ty: _, values, as_model: _ } => values
                            .iter()
                            .map(|path_elem_expr| {
                                let (field_count, qualified_elem_type_name) = match path_elem_expr.ty(ctx) {
                                    Type::Struct{ fields, name: Some(name), .. } => (fields.len(), name),
                                    _ => unreachable!()
                                };
                                // Turn slint::private_api::PathLineTo into `LineTo`
                                let elem_type_name = qualified_elem_type_name.split("::").last().unwrap().strip_prefix("Path").unwrap();
                                let elem_init = if field_count > 0 {
                                    compile_expression(path_elem_expr, ctx)
                                } else {
                                    String::new()
                                };
                                format!("slint::private_api::PathElement::{}({})", elem_type_name, elem_init)
                            }),
                        _ => {
                            unreachable!()
                        }
                    }.collect::<Vec<_>>();
                    format!(
                        r#"[&](){{
                            slint::private_api::PathElement elements[{}] = {{
                                {}
                            }};
                            return slint::private_api::PathData(&elements[0], std::size(elements));
                        }}()"#,
                        path_elements.len(),
                        path_elements.join(",")
                    )
                }
                (Type::Struct { .. }, Type::PathData)
                    if matches!(
                        from.as_ref(),
                        Expression::Struct { ty: Type::Struct { .. }, .. }
                    ) =>
                {
                    let (events, points) = match from.as_ref() {
                        Expression::Struct { ty: _, values } => (
                            compile_expression(&values["events"], ctx),
                            compile_expression(&values["points"], ctx),
                        ),
                        _ => {
                            unreachable!()
                        }
                    };
                    format!(
                        r#"[&](auto events, auto points){{
                            return slint::private_api::PathData(events.ptr, events.len, points.ptr, points.len);
                        }}({}, {})"#,
                        events, points
                    )
                }
                _ => f,
            }
        }
        Expression::CodeBlock(sub) => {
            let len = sub.len();
            let mut x = sub.iter().enumerate().map(|(i, e)| {
                if i == len - 1 {
                    return_compile_expression(e, ctx, None) + ";"
                }
                else {
                    compile_expression(e, ctx)
                }

            });

            format!("[&]{{ {} }}()", x.join(";"))
        }
        Expression::PropertyAssignment { property, value} => {
            let value = compile_expression(value, ctx);
            property_set_value_code(property, &value, ctx)
        }
        Expression::ModelDataAssignment { level, value } => {
            let value = compile_expression(value, ctx);
            let mut path = "self".to_string();
            let mut ctx2 = ctx;
            let mut repeater_index = None;
            for _ in 0..=*level {
                let x = ctx2.parent.clone().unwrap();
                ctx2 = x.ctx;
                repeater_index = x.repeater_index;
                write!(path, "->parent").unwrap();
            }
            let repeater_index = repeater_index.unwrap();
            let mut index_prop = llr::PropertyReference::Local {
                sub_component_path: vec![],
                property_index: ctx2.current_sub_component.unwrap().repeated[repeater_index]
                    .index_prop
                    .unwrap(),
            };
            if let Some(level) = NonZeroUsize::new(*level) {
                index_prop =
                    llr::PropertyReference::InParent { level, parent_reference: index_prop.into() };
            }
            let index_access = access_member(&index_prop, ctx);
            write!(path, "->repeater_{}", repeater_index).unwrap();
            format!("{}.model_set_row_data({}.get(), {})", path, index_access, value)
        }
        Expression::ArrayIndexAssignment { array, index, value } => {
            debug_assert!(matches!(array.ty(ctx), Type::Array(_)));
            let base_e = compile_expression(array, ctx);
            let index_e = compile_expression(index, ctx);
            let value_e = compile_expression(value, ctx);
            format!(
                "{}->set_row_data({}, {})",
                base_e, index_e, value_e
            )
        }
        Expression::BinaryExpression { lhs, rhs, op } => {
            let mut buffer = [0; 3];
            format!(
                "({lhs} {op} {rhs})",
                lhs = compile_expression(&*lhs, ctx),
                rhs = compile_expression(&*rhs, ctx),
                op = match op {
                    '=' => "==",
                    '!' => "!=",
                    '≤' => "<=",
                    '≥' => ">=",
                    '&' => "&&",
                    '|' => "||",
                    '/' => "/(float)",
                    _ => op.encode_utf8(&mut buffer),
                },
            )
        }
        Expression::UnaryOp { sub, op } => {
            format!("({op} {sub})", sub = compile_expression(&*sub, ctx), op = op,)
        }
        Expression::ImageReference { resource_ref, .. }  => {
            match resource_ref {
                crate::expression_tree::ImageReference::None => r#"slint::Image()"#.to_string(),
                crate::expression_tree::ImageReference::AbsolutePath(path) => format!(r#"slint::Image::load_from_path(slint::SharedString(u8"{}"))"#, escape_string(path.as_str())),
                crate::expression_tree::ImageReference::EmbeddedData { resource_id, extension } => {
                    let symbol = format!("slint_embedded_resource_{}", resource_id);
                    format!(r#"slint::private_api::load_image_from_embedded_data({symbol}, "{}")"#, escape_string(extension))
                }
                crate::expression_tree::ImageReference::EmbeddedTexture{..} => todo!(),
            }
        }
        Expression::Condition { condition, true_expr, false_expr } => {
            let ty = expr.ty(ctx);
            let cond_code = compile_expression(condition, ctx);
            let cond_code = remove_parentheses(&cond_code);
            let true_code = return_compile_expression(true_expr, ctx, Some(&ty));
            let false_code = return_compile_expression(false_expr, ctx, Some(&ty));
            format!(
                r#"[&]() -> {} {{ if ({}) {{ {}; }} else {{ {}; }}}}()"#,
                ty.cpp_type().unwrap_or_else(|| "void".to_string()),
                cond_code,
                true_code,
                false_code
            )
        }
        Expression::Array { element_ty, values, as_model } => {
            let ty = element_ty.cpp_type().unwrap();
            let mut val = values.iter().map(|e| format!("{ty} ( {expr} )", expr = compile_expression(e, ctx), ty = ty));
            if *as_model {
                format!(
                    "std::make_shared<slint::private_api::ArrayModel<{count},{ty}>>({val})",
                    count = values.len(),
                    ty = ty,
                    val = val.join(", ")
                )
            } else {
                format!(
                    "slint::cbindgen_private::Slice<{ty}>{{ std::array<{ty}, {count}>{{ {val} }}.data(), {count} }}",
                    count = values.len(),
                    ty = ty,
                    val = val.join(", ")
                )
            }
        }
        Expression::Struct { ty, values } => {
            if let Type::Struct{fields, name: None, ..} = ty {
                let mut elem = fields.keys().map(|k| {
                    values
                        .get(k)
                        .map(|e| compile_expression(e, ctx))
                        .unwrap_or_else(|| "(Error: missing member in object)".to_owned())
                });
                format!("std::make_tuple({})", elem.join(", "))
            } else if let Type::Struct{ name: Some(_), .. } = ty {
                format!(
                    "[&]({args}){{ {ty} o{{}}; {fields}return o; }}({vals})",
                    args = (0..values.len()).map(|i| format!("const auto &a_{}", i)).join(", "),
                    ty = ty.cpp_type().unwrap(),
                    fields = values.keys().enumerate().map(|(i, f)| format!("o.{} = a_{}; ", ident(f), i)).join(""),
                    vals = values.values().map(|e| compile_expression(e, ctx)).join(", "),
                )
            } else {
                panic!("Expression::Object is not a Type::Object")
            }
        }
        Expression::EasingCurve(EasingCurve::Linear) => "slint::cbindgen_private::EasingCurve()".into(),
        Expression::EasingCurve(EasingCurve::CubicBezier(a, b, c, d)) => format!(
            "slint::cbindgen_private::EasingCurve(slint::cbindgen_private::EasingCurve::Tag::CubicBezier, {}, {}, {}, {})",
            a, b, c, d
        ),
        Expression::LinearGradient{angle, stops} => {
            let angle = compile_expression(angle, ctx);
            let mut stops_it = stops.iter().map(|(color, stop)| {
                let color = compile_expression(color, ctx);
                let position = compile_expression(stop, ctx);
                format!("slint::private_api::GradientStop{{ {}, {}, }}", color, position)
            });
            format!(
                "[&] {{ const slint::private_api::GradientStop stops[] = {{ {} }}; return slint::Brush(slint::private_api::LinearGradientBrush({}, stops, {})); }}()",
                stops_it.join(", "), angle, stops.len()
            )
        }
        Expression::RadialGradient{ stops} => {
            let mut stops_it = stops.iter().map(|(color, stop)| {
                let color = compile_expression(color, ctx);
                let position = compile_expression(stop, ctx);
                format!("slint::private_api::GradientStop{{ {}, {}, }}", color, position)
            });
            format!(
                "[&] {{ const slint::private_api::GradientStop stops[] = {{ {} }}; return slint::Brush(slint::private_api::RadialGradientBrush(stops, {})); }}()",
                stops_it.join(", "), stops.len()
            )
        }
        Expression::EnumerationValue(value) => {
            format!(
                "slint::cbindgen_private::{}::{}",
                value.enumeration.name,
                ident(&value.to_pascal_case()),
            )
        }
        Expression::ReturnStatement(Some(expr)) => format!(
            "throw slint::private_api::ReturnWrapper<{}>({})",
            expr.ty(ctx).cpp_type().unwrap_or_default(),
            compile_expression(expr, ctx)
        ),
        Expression::ReturnStatement(None) => "throw slint::private_api::ReturnWrapper<void>()".to_owned(),
        Expression::LayoutCacheAccess { layout_cache_prop, index, repeater_index } =>  {
            let cache = access_member(layout_cache_prop, ctx);
            if let Some(ri) = repeater_index {
                format!("slint::private_api::layout_cache_access({}.get(), {}, {})", cache, index, compile_expression(ri, ctx))
            } else {
                format!("{}.get()[{}]", cache, index)
            }
        }
        Expression::BoxLayoutFunction {
            cells_variable,
            repeater_indices,
            elements,
            orientation,
            sub_expression,
        } => box_layout_function(
            cells_variable,
            repeater_indices.as_ref().map(String::as_str),
            elements,
            *orientation,
            sub_expression,
            ctx,
        ),
        Expression::ComputeDialogLayoutCells { cells_variable, roles, unsorted_cells } => {
            let cells_variable = ident(cells_variable);
            let mut cells = match &**unsorted_cells {
                Expression::Array { values, .. } => {
                    values.iter().map(|v| compile_expression(v, ctx))
                }
                _ => panic!("dialog layout unsorted cells not an array"),
            };
            format!("slint::cbindgen_private::GridLayoutCellData {cv}_array [] = {{ {c} }};\
                    slint::cbindgen_private::slint_reorder_dialog_button_layout({cv}_array, {r});\
                    slint::cbindgen_private::Slice<slint::cbindgen_private::GridLayoutCellData> {cv} {{ std::data({cv}_array), std::size({cv}_array) }}",
                    r = compile_expression(roles, ctx),
                    cv = cells_variable,
                    c = cells.join(", "),
                )

        }
    }
}

fn compile_builtin_function_call(
    function: BuiltinFunction,
    arguments: &[llr::Expression],
    ctx: &EvaluationContext,
) -> String {
    let mut a = arguments.iter().map(|a| compile_expression(a, ctx));
    let pi_180 = std::f64::consts::PI / 180.0;

    match function {
        BuiltinFunction::GetWindowScaleFactor => {
            let window = access_window_field(ctx);
            format!("{}.scale_factor()", window)
        }
        BuiltinFunction::AnimationTick => "slint::cbindgen_private::slint_animation_tick()".into(),
        BuiltinFunction::Debug => {
            format!("std::cout << {} << std::endl;", a.join("<<"))
        }
        BuiltinFunction::Mod => format!("std::fmod({}, {})", a.next().unwrap(), a.next().unwrap()),
        BuiltinFunction::Round => format!("std::round({})", a.next().unwrap()),
        BuiltinFunction::Ceil => format!("std::ceil({})", a.next().unwrap()),
        BuiltinFunction::Floor => format!("std::floor({})", a.next().unwrap()),
        BuiltinFunction::Sqrt => format!("std::sqrt({})", a.next().unwrap()),
        BuiltinFunction::Abs => format!("std::abs({})", a.next().unwrap()),
        BuiltinFunction::Log => {
            format!("std::log({}) / std::log({})", a.next().unwrap(), a.next().unwrap())
        }
        BuiltinFunction::Pow => {
            format!("std::pow(({}), ({}))", a.next().unwrap(), a.next().unwrap())
        }
        BuiltinFunction::Sin => format!("std::sin(({}) * {})", a.next().unwrap(), pi_180),
        BuiltinFunction::Cos => format!("std::cos(({}) * {})", a.next().unwrap(), pi_180),
        BuiltinFunction::Tan => format!("std::tan(({}) * {})", a.next().unwrap(), pi_180),
        BuiltinFunction::ASin => format!("std::asin({}) / {}", a.next().unwrap(), pi_180),
        BuiltinFunction::ACos => format!("std::acos({}) / {}", a.next().unwrap(), pi_180),
        BuiltinFunction::ATan => format!("std::atan({}) / {}", a.next().unwrap(), pi_180),
        BuiltinFunction::SetFocusItem => {
            if let [llr::Expression::PropertyReference(pr)] = arguments {
                let window = access_window_field(ctx);
                let focus_item = access_item_rc(pr, ctx);
                format!("{}.set_focus_item({});", window, focus_item)
            } else {
                panic!("internal error: invalid args to SetFocusItem {:?}", arguments)
            }
        }
        /*  std::from_chars is unfortunately not yet implemented in gcc
        BuiltinFunction::StringIsFloat => {
            "[](const auto &a){ double v; auto r = std::from_chars(std::begin(a), std::end(a), v); return r.ptr == std::end(a); }"
                .into()
        }
        BuiltinFunction::StringToFloat => {
            "[](const auto &a){ double v; auto r = std::from_chars(std::begin(a), std::end(a), v); return r.ptr == std::end(a) ? v : 0; }"
                .into()
        }*/
        BuiltinFunction::StringIsFloat => {
            format!("[](const auto &a){{ auto e1 = std::end(a); auto e2 = const_cast<char*>(e1); std::strtod(std::begin(a), &e2); return e1 == e2; }}({})", a.next().unwrap())
        }
        BuiltinFunction::StringToFloat => {
            format!("[](const auto &a){{ auto e1 = std::end(a); auto e2 = const_cast<char*>(e1); auto r = std::strtod(std::begin(a), &e2); return e1 == e2 ? r : 0; }}({})", a.next().unwrap())
        }
        BuiltinFunction::ColorBrighter => {
            format!("{}.brighter({})", a.next().unwrap(), a.next().unwrap())
        }
        BuiltinFunction::ColorDarker => {
            format!("{}.darker({})", a.next().unwrap(), a.next().unwrap())
        }
        BuiltinFunction::ImageSize => {
            format!("{}.size()", a.next().unwrap())
        }
        BuiltinFunction::ArrayLength => {
            format!("[](const auto &model){{ (*model).track_row_count_changes(); return (*model).row_count(); }}({})", a.next().unwrap())
        }
        BuiltinFunction::Rgb => {
            format!("slint::Color::from_argb_uint8(std::clamp(static_cast<float>({a}) * 255., 0., 255.), std::clamp(static_cast<int>({r}), 0, 255), std::clamp(static_cast<int>({g}), 0, 255), std::clamp(static_cast<int>({b}), 0, 255))",
                r = a.next().unwrap(),
                g = a.next().unwrap(),
                b = a.next().unwrap(),
                a = a.next().unwrap(),
            )
        }
        BuiltinFunction::DarkColorScheme => {
            format!("{}.dark_color_scheme()", access_window_field(ctx))
        }
        BuiltinFunction::ShowPopupWindow => {
            if let [llr::Expression::NumberLiteral(popup_index), x, y, llr::Expression::PropertyReference(parent_ref)] =
                arguments
            {
                let mut parent_ctx = ctx;
                let mut component_access = "self".into();

                if let llr::PropertyReference::InParent { level, .. } = parent_ref {
                    for _ in 0..level.get() {
                        component_access = format!("{}->parent", component_access);
                        parent_ctx = parent_ctx.parent.as_ref().unwrap().ctx;
                    }
                };

                let window = access_window_field(ctx);
                let current_sub_component = parent_ctx.current_sub_component.unwrap();
                let popup_window_id =
                    ident(&current_sub_component.popup_windows[*popup_index as usize].root.name);
                let parent_component = access_item_rc(parent_ref, ctx);
                let x = compile_expression(x, ctx);
                let y = compile_expression(y, ctx);
                format!(
                    "{window}.show_popup<{popup_window_id}>({component_access}, {{ static_cast<float>({x}), static_cast<float>({y}) }}, {{ {parent_component} }})"
                )
            } else {
                panic!("internal error: invalid args to ShowPopupWindow {:?}", arguments)
            }
        }
        BuiltinFunction::RegisterCustomFontByPath => {
            if let [llr::Expression::StringLiteral(path)] = arguments {
                let window = access_window_field(ctx);
                format!("{window}.register_font_from_path(\"{}\");", escape_string(path))
            } else {
                panic!(
                    "internal error: argument to RegisterCustomFontByPath must be a string literal"
                )
            }
        }
        BuiltinFunction::RegisterCustomFontByMemory => {
            if let [llr::Expression::NumberLiteral(resource_id)] = &arguments {
                let window = access_window_field(ctx);
                let resource_id: usize = *resource_id as _;
                let symbol = format!("slint_embedded_resource_{}", resource_id);
                format!("{window}.register_font_from_data({symbol}, std::size({symbol}));")
            } else {
                panic!("internal error: invalid args to RegisterCustomFontByMemory {:?}", arguments)
            }
        }
        BuiltinFunction::RegisterBitmapFont => {
            todo!()
        }
        BuiltinFunction::ImplicitLayoutInfo(orient) => {
            if let [llr::Expression::PropertyReference(pr)] = arguments {
                let native = native_item(pr, ctx);
                format!(
                    "{vt}->layout_info({{{vt}, const_cast<slint::cbindgen_private::{ty}*>(&{i})}}, {o}, &{window})",
                    vt = native.cpp_vtable_getter,
                    ty = native.class_name,
                    o = to_cpp_orientation(orient),
                    i = access_member(pr, ctx),
                    window = access_window_field(ctx)
                )
            } else {
                panic!("internal error: invalid args to ImplicitLayoutInfo {:?}", arguments)
            }
        }
    }
}

fn box_layout_function(
    cells_variable: &str,
    repeated_indices: Option<&str>,
    elements: &[Either<llr::Expression, usize>],
    orientation: Orientation,
    sub_expression: &llr::Expression,
    ctx: &llr_EvaluationContext<String>,
) -> String {
    let repeated_indices = repeated_indices.map(ident);
    let mut push_code =
        "std::vector<slint::cbindgen_private::BoxLayoutCellData> cells_vector;".to_owned();
    let mut repeater_idx = 0usize;

    for item in elements {
        match item {
            Either::Left(value) => {
                write!(
                    push_code,
                    "cells_vector.push_back({{ {} }});",
                    compile_expression(value, ctx)
                )
                .unwrap();
            }
            Either::Right(repeater) => {
                write!(push_code, "self->repeater_{}.ensure_updated(self);", repeater).unwrap();

                if let Some(ri) = &repeated_indices {
                    write!(push_code, "{}_array[{}] = cells_vector.size();", ri, repeater_idx * 2)
                        .unwrap();
                    write!(push_code,
                        "{ri}_array[{c}] = self->repeater_{id}.inner ? self->repeater_{id}.inner->data.size() : 0;",
                        ri = ri,
                        c = repeater_idx * 2 + 1,
                        id = repeater,
                    ).unwrap();
                }
                repeater_idx += 1;
                write!(
                    push_code,
                    "if (self->repeater_{id}.inner) \
                        for (auto &&sub_comp : self->repeater_{id}.inner->data) \
                           cells_vector.push_back((*sub_comp.ptr)->box_layout_data({o}));",
                    id = repeater,
                    o = to_cpp_orientation(orientation),
                )
                .unwrap();
            }
        }
    }

    let ri = repeated_indices.as_ref().map_or(String::new(), |ri| {
        write!(
            push_code,
            "slint::cbindgen_private::Slice<int> {ri}{{ {ri}_array.data(), {ri}_array.size() }};",
            ri = ri
        )
        .unwrap();
        format!("std::array<int, {}> {}_array;", 2 * repeater_idx, ri)
    });
    format!(
        "[&]{{ {} {} slint::cbindgen_private::Slice<slint::cbindgen_private::BoxLayoutCellData>{}{{cells_vector.data(), cells_vector.size()}}; return {}; }}()",
        ri,
        push_code,
        ident(cells_variable),
        compile_expression(sub_expression, ctx)
    )
}

/// Like compile_expression, but wrap inside a try{}catch{} block to intercept the return
fn compile_expression_wrap_return(expr: &llr::Expression, ctx: &EvaluationContext) -> String {
    let mut return_type = None;
    expr.visit_recursive(&mut |e| {
        if let llr::Expression::ReturnStatement(val) = e {
            return_type = Some(val.as_ref().map_or(Type::Void, |v| v.ty(ctx)));
        }
    });

    if let Some(ty) = return_type {
        if ty == Type::Void || ty == Type::Invalid {
            format!(
                "[&]{{ try {{ {}; }} catch(const slint::private_api::ReturnWrapper<void> &w) {{ }} }}()",
                compile_expression(expr, ctx)
            )
        } else {
            let cpp_ty = ty.cpp_type().unwrap_or_default();
            format!(
                "[&]() -> {} {{ try {{ {}; }} catch(const slint::private_api::ReturnWrapper<{}> &w) {{ return w.value; }} }}()",
                cpp_ty,
                return_compile_expression(expr, ctx, Some(&ty)),
                cpp_ty
            )
        }
    } else {
        compile_expression(expr, ctx)
    }
}

/// Like compile expression, but prepended with `return` if not void.
/// ret_type is the expecting type that should be returned with that return statement
fn return_compile_expression(
    expr: &llr::Expression,
    ctx: &EvaluationContext,
    ret_type: Option<&Type>,
) -> String {
    let e = compile_expression(expr, ctx);
    if ret_type == Some(&Type::Void) || ret_type == Some(&Type::Invalid) {
        e
    } else {
        let ty = expr.ty(ctx);
        if ty == Type::Invalid && ret_type.is_some() {
            // e is unreachable so it probably throws. But we still need to return something to avoid a warning
            format!("{}; return {{}}", e)
        } else if ty == Type::Invalid || ty == Type::Void {
            e
        } else {
            format!("return {}", e)
        }
    }
}
