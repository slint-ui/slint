// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*! module for the C++ code generator
*/

// cSpell:ignore cstdlib cmath constexpr nullptr decltype intptr uintptr

use std::fmt::Write;

fn ident(ident: &str) -> String {
    if ident.contains('-') {
        ident.replace('-', "_")
    } else {
        ident.into()
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

type EvaluationContext<'a> = llr_EvaluationContext<'a, String>;
type ParentCtx<'a> = llr_ParentCtx<'a, String>;

impl CppType for Type {
    fn cpp_type(&self) -> Option<String> {
        match self {
            Type::Void => Some("void".to_owned()),
            Type::Float32 => Some("float".to_owned()),
            Type::Int32 => Some("int".to_owned()),
            Type::String => Some("sixtyfps::SharedString".to_owned()),
            Type::Color => Some("sixtyfps::Color".to_owned()),
            Type::Duration => Some("std::int64_t".to_owned()),
            Type::Angle => Some("float".to_owned()),
            Type::PhysicalLength => Some("float".to_owned()),
            Type::LogicalLength => Some("float".to_owned()),
            Type::Percent => Some("float".to_owned()),
            Type::Bool => Some("bool".to_owned()),
            Type::Struct { name: Some(name), node: Some(_), .. } => Some(ident(name)),
            Type::Struct { name: Some(name), node: None, .. } => {
                Some(format!("sixtyfps::cbindgen_private::{}", ident(name)))
            }
            Type::Struct { fields, .. } => {
                let elem = fields.values().map(|v| v.cpp_type()).collect::<Option<Vec<_>>>()?;

                Some(format!("std::tuple<{}>", elem.join(", ")))
            }

            Type::Array(i) => Some(format!("std::shared_ptr<sixtyfps::Model<{}>>", i.cpp_type()?)),
            Type::Image => Some("sixtyfps::Image".to_owned()),
            Type::Builtin(elem) => elem.native_class.cpp_type.clone(),
            Type::Enumeration(enumeration) => {
                Some(format!("sixtyfps::cbindgen_private::{}", ident(&enumeration.name)))
            }
            Type::Brush => Some("sixtyfps::Brush".to_owned()),
            Type::LayoutCache => Some("sixtyfps::SharedVector<float>".into()),
            _ => None,
        }
    }
}

fn to_cpp_orientation(o: Orientation) -> &'static str {
    match o {
        Orientation::Horizontal => "sixtyfps::Orientation::Horizontal",
        Orientation::Vertical => "sixtyfps::Orientation::Vertical",
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

/*
fn new_struct_with_bindings(
    type_name: &str,
    bindings: &crate::object_tree::BindingsMap,
    component: &Rc<Component>,
) -> String {
    let bindings_initialization: Vec<String> = bindings
        .iter()
        .map(|(prop, initializer)| {
            let initializer = compile_expression(&initializer.borrow(), component);
            format!("var.{} = {};", ident(prop), initializer)
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

fn property_animation_code(component: &Rc<Component>, animation: &ElementRc) -> String {
    new_struct_with_bindings(
        "sixtyfps::cbindgen_private::PropertyAnimation",
        &animation.borrow().bindings,
        component,
    )
}
*/
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

/*
fn handle_property_binding(
    elem: &ElementRc,
    prop_name: &str,
    binding_expression: &BindingExpression,
    init: &mut Vec<String>,
) {
    let item = elem.borrow();
    let component = item.enclosing_component.upgrade().unwrap();
    let prop_access = access_member(elem, prop_name, &component, "this");
    let prop_type = item.lookup_property(prop_name).property_type;
    if let Type::Callback { args, .. } = &prop_type {
        if matches!(binding_expression.expression, Expression::Invalid) {
            return;
        }
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
            code = compile_expression_wrap_return(binding_expression, &component)
        ));
    } else {
        if matches!(binding_expression.expression, Expression::Invalid) {
            return;
        }

        let component = &item.enclosing_component.upgrade().unwrap();

        let init_expr = compile_expression_wrap_return(binding_expression, component);

        let is_constant = binding_expression.analysis.as_ref().map_or(false, |a| a.is_const);
        init.push(if is_constant {
            format!("{}.set({});", prop_access, init_expr)
        } else {
            let binding_code = format!(
                "[this]() {{
                            [[maybe_unused]] auto self = this;
                            return {init};
                        }}",
                init = init_expr
            );

            let is_state_info = matches!(prop_type, Type::Struct { name: Some(name), .. } if name.ends_with("::StateInfo"));
            if is_state_info {
                format!("sixtyfps::private_api::set_state_binding({}, {});", prop_access, binding_code)
            } else {
                match &binding_expression.animation {
                    Some(crate::object_tree::PropertyAnimation::Static(anim)) => {
                        let anim = property_animation_code(component, anim);
                        format!("{}.set_animated_binding({}, {});", prop_access, binding_code, anim)
                    }
                    Some(crate::object_tree::PropertyAnimation::Transition {
                        state_ref,
                        animations,
                    }) => {
                        let state_tokens = compile_expression(state_ref, component);
                        let mut anim_expr = animations.iter().map(|a| {
                            let cond = compile_expression(
                                &a.condition(Expression::ReadLocalVariable {
                                    name: "state".into(),
                                    ty: state_ref.ty(),
                                }),
                                component,
                            );
                            let anim = property_animation_code(component, &a.animation);
                            format!("if ({}) {{ return {}; }}", remove_parentheses(&cond), anim)
                        });
                        format!(
                            "{}.set_animated_binding_for_transition({},
                            [this](uint64_t *start_time) -> sixtyfps::cbindgen_private::PropertyAnimation {{
                                [[maybe_unused]] auto self = this;
                                auto state = {};
                                *start_time = state.change_time;
                                {}
                                return {{}};
                            }});",
                            prop_access,
                            binding_code,
                            state_tokens,
                            anim_expr.join(" ")
                        )
                    }
                    None => format!("{}.set_binding({});", prop_access, binding_code),
                }
            }
        });
    }
}
*/

fn handle_property_init(
    prop: &llr::PropertyReference,
    binding_expression: &llr::BindingExpression,
    init: &mut Vec<String>,
    ctx: &EvaluationContext,
) {
    /*
    let item = elem.borrow();
    let component = item.enclosing_component.upgrade().unwrap();
     */
    let prop_access = access_member(prop, ctx);
    let prop_type = ctx.property_ty(prop);
    if let Type::Callback { args, .. } = &prop_type {
        let mut ctx2 = ctx.clone();
        ctx2.argument_types = &args;

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
            code = compile_expression_wrap_return(&binding_expression.expression, &ctx2)
        ));
    } else {
        let init_expr = compile_expression_wrap_return(&binding_expression.expression, ctx);

        init.push(if binding_expression.is_constant {
            format!("{}.set({});", prop_access, init_expr)
        } else {
            let binding_code = format!(
                "[this]() {{
                            [[maybe_unused]] auto self = this;
                            return {init};
                        }}",
                init = init_expr
            );

            let is_state_info = matches!(prop_type, Type::Struct { name: Some(name), .. } if name.ends_with("::StateInfo"));
            if is_state_info {
                format!("sixtyfps::private_api::set_state_binding({}, {});", prop_access, binding_code)
            } else {
                match &binding_expression.animation {
                    Some(llr::Animation::Static(anim)) => {
                        let anim = compile_expression(anim, ctx);
                        format!("{}.set_animated_binding({}, {});", prop_access, binding_code, anim)
                    }
                    Some(llr::Animation::Transition (
                        anim
                    )) => {
                        /*
                        let anim = llr_compile_expression(anim, ctx);
                        let state_tokens = compile_expression(state_ref, component);
                        let mut anim_expr = animations.iter().map(|a| {
                            let cond = compile_expression(
                                &a.condition(Expression::ReadLocalVariable {
                                    name: "state".into(),
                                    ty: state_ref.ty(),
                                }),
                                component,
                            );
                            let anim = property_animation_code(component, &a.animation);
                            format!("if ({}) {{ return {}; }}", remove_parentheses(&cond), anim)
                        });
                        format!(
                            "{}.set_animated_binding_for_transition({},
                            [this](uint64_t *start_time) -> sixtyfps::cbindgen_private::PropertyAnimation {{
                                [[maybe_unused]] auto self = this;
                                auto state = {};
                                *start_time = state.change_time;
                                {}
                                return {{}};
                            }});",
                            prop_access,
                            binding_code,
                            state_tokens,
                            anim_expr.join(" ")
                        )
                        */
                        todo!()
                    }
                    None => format!("{}.set_binding({});", prop_access, binding_code),
                }
            }
        });
    }
}

/*
fn handle_item(elem: &ElementRc, field_access: Access, main_struct: &mut Struct) {
    let item = elem.borrow();
    main_struct.members.push((
        field_access,
        Declaration::Var(Var {
            ty: format!(
                "sixtyfps::cbindgen_private::{}",
                ident(&item.base_type.as_native().class_name)
            ),
            name: ident(&item.id),
            init: Some("{}".to_owned()),
            ..Default::default()
        }),
    ));
}
*/

/*
fn handle_repeater(
    repeated: &RepeatedElementInfo,
    base_component: &Rc<Component>,
    parent_component: &Rc<Component>,
    repeater_count: u32,
    component_struct: &mut Struct,
    init: &mut Vec<String>,
    children_visitor_cases: &mut Vec<String>,
    diag: &mut BuildDiagnostics,
) {
    let parent_element = base_component.parent_element.upgrade().unwrap();
    let repeater_id = format!("repeater_{}", ident(&parent_element.borrow().id));

    let mut model = compile_expression(&repeated.model, parent_component);
    if repeated.is_conditional_element {
        // bool converts to int
        // FIXME: don't do a heap allocation here
        model = format!("std::make_shared<sixtyfps::private_api::IntModel>({})", model)
    };

    // FIXME: optimize  if repeated.model.is_constant()
    init.push(format!(
        "self->{repeater_id}.set_model_binding([self] {{ (void)self; return {model}; }});",
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
    } else {
        children_visitor_cases.push(format!(
            "\n        case {i}: {{
                self->{id}.ensure_updated(self);
                return self->{id}.visit(order, visitor);
            }}",
            id = repeater_id,
            i = repeater_count,
        ));
    }

    component_struct.members.push((
        Access::Private,
        Declaration::Var(Var {
            ty: format!(
                "sixtyfps::private_api::Repeater<class {}, {}>",
                component_id(base_component),
                model_data_type(&parent_element, diag)
            ),
            name: repeater_id,
            ..Default::default()
        }),
    ));
}
*/

/// Returns the text of the C++ code produced by the given root component
pub fn generate(doc: &Document) -> Option<impl std::fmt::Display> {
    let mut file = File::default();

    file.includes.push("<array>".into());
    file.includes.push("<limits>".into());
    file.includes.push("<cstdlib>".into()); // TODO: ideally only include this if needed (by to_float)
    file.includes.push("<cmath>".into()); // TODO: ideally only include this if needed (by floor/ceil/round)
    file.includes.push("<sixtyfps.h>".into());

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
                        name: format!("sfps_embedded_resource_{}", er.id),
                        array_size: Some(data.len()),
                        init: Some(init),
                    })
                }
                crate::embedded_resources::EmbeddedResourcesKind::TextureData(_) => todo!(),
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
        generate_sub_component(&mut sub_compo_struct, sub_compo, &llr, None, &mut file);
        file.definitions.extend(sub_compo_struct.extract_definitions().collect::<Vec<_>>());
        file.declarations.push(Declaration::Struct(sub_compo_struct));
    }

    // let mut components_to_add_as_friends = vec![];
    // for sub_comp in doc.root_component.used_types.borrow().sub_components.iter() {
    //     generate_component(
    //         &mut file,
    //         sub_comp,
    //         &doc.root_component,
    //         diag,
    //         &mut components_to_add_as_friends,
    //     );
    // }
    //
    // for glob in doc
    //     .root_component
    //     .used_types
    //     .borrow()
    //     .globals
    //     .iter()
    //     .filter(|glob| glob.requires_code_generation())
    // {
    //     generate_component(
    //         &mut file,
    //         glob,
    //         &doc.root_component,
    //         diag,
    //         &mut components_to_add_as_friends,
    //     );
    //
    //     if glob.visible_in_public_api() {
    //         file.definitions.extend(glob.global_aliases().into_iter().map(|name| {
    //             Declaration::TypeAlias(TypeAlias {
    //                 old_name: ident(&glob.root_element.borrow().id),
    //                 new_name: name,
    //             })
    //         }))
    //     }
    // }
    llr.globals
        .iter()
        .filter(|glob| !glob.is_builtin)
        .for_each(|glob| generate_global(&mut file, glob, &llr));

    generate_public_component(&mut file, &llr);

    file.definitions.push(Declaration::Var(Var{
        ty: format!(
            "[[maybe_unused]] constexpr sixtyfps::private_api::VersionCheckHelper<{}, {}, {}>",
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH")),
        name: "THE_SAME_VERSION_MUST_BE_USED_FOR_THE_COMPILER_AND_THE_RUNTIME".into(),
        init: Some("sixtyfps::private_api::VersionCheckHelper<SIXTYFPS_VERSION_MAJOR, SIXTYFPS_VERSION_MINOR, SIXTYFPS_VERSION_PATCH>()".into()),
        ..Default::default()
    }));

    Some(file)
}

fn generate_struct(file: &mut File, name: &str, fields: &BTreeMap<String, Type>) {
    let mut operator_eq = String::new();
    let mut members = fields
        .iter()
        .map(|(name, t)| {
            write!(operator_eq, " && a.{0} == b.{0}", ident(name)).unwrap();
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
            signature: format!("(const {0} &a, const {0} &b) -> bool", name),
            is_friend: true,
            statements: Some(vec![format!("return true{};", operator_eq)]),
            ..Function::default()
        }),
    ));
    members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "operator!=".to_owned(),
            signature: format!("(const {0} &a, const {0} &b) -> bool", name),
            is_friend: true,
            statements: Some(vec!["return !(a == b);".into()]),
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

    let ctx = EvaluationContext {
        public_component: component,
        current_sub_component: Some(&component.item_tree.root),
        current_global: None,
        generator_state: "this".to_string(),
        parent: None,
        argument_types: &[],
    };

    // component_struct.friends.extend(
    //     component
    //         .used_types
    //         .borrow()
    //         .sub_components
    //         .iter()
    //         .map(self::component_id)
    //         .chain(std::mem::take(sub_components).into_iter()),
    // );

    // for c in component.popup_windows.borrow().iter() {
    //     let mut friends = vec![self::component_id(&c.component)];
    //     generate_component(file, &c.component, root_component, diag, &mut friends);
    //     sub_components.extend_from_slice(friends.as_slice());
    //     component_struct.friends.append(&mut friends);
    // }

    // let expose_property = |property: &PropertyDeclaration| -> bool {
    //     if component.is_global() || component.is_root_component.get() {
    //         property.expose_in_public_api
    //     } else {
    //         false
    //     }
    // };

    component_struct.members.iter_mut().for_each(|(access, _)| {
        *access = Access::Private;
    });

    let declarations = generate_public_api_for_properties(&component.public_properties, &ctx);
    component_struct.members.extend(declarations.into_iter().map(|decl| (Access::Public, decl)));

    // let mut constructor_arguments = String::new();
    // let mut constructor_member_initializers = vec![];
    // let mut constructor_code = vec![];

    let mut window_init = None;
    let mut access = Access::Private;

    window_init = Some("sixtyfps::Window{sixtyfps::private_api::WindowRc()}".into());
    // FIXME: many of the different component bindings need to access this
    access = Access::Public;

    component_struct.members.push((
        access,
        Declaration::Var(Var {
            ty: "sixtyfps::Window".into(),
            name: "m_window".into(),
            init: window_init,
            ..Default::default()
        }),
    ));

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
            signature: "() const -> sixtyfps::Window&".into(),
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
                "sixtyfps::run_event_loop();".into(),
                "hide();".into(),
            ]),
            ..Default::default()
        }),
    ));

    component_struct.friends.push("sixtyfps::private_api::WindowRc".into());

    component_struct
        .friends
        .extend(component.sub_components.iter().map(|sub_compo| ident(&sub_compo.name)));

    generate_item_tree(
        &mut component_struct,
        &component.item_tree,
        &component,
        None,
        component_id.clone(),
        file,
    );

    // } else if is_sub_component {
    //     let root_ptr_type = format!("const {} *", self::component_id(root_component));
    //
    //     constructor_arguments =
    //         format!("{} root, [[maybe_unused]] uintptr_t tree_index, [[maybe_unused]] uintptr_t tree_index_of_first_child", root_ptr_type);
    //
    //     component_struct.members.push((
    //         Access::Private,
    //         Declaration::Var(Var {
    //             ty: "sixtyfps::Window".into(),
    //             name: "m_window".into(),
    //             ..Default::default()
    //         }),
    //     ));
    //     constructor_member_initializers.push("m_window(root->m_window.window_handle())".into());
    //
    //     component_struct.members.push((
    //         Access::Private,
    //         Declaration::Var(Var {
    //             ty: root_ptr_type,
    //             name: "m_root".to_owned(),
    //             ..Default::default()
    //         }),
    //     ));
    //     constructor_member_initializers.push("m_root(root)".into());
    //     constructor_code.push("(void)m_root;".into()); // silence warning about unused variable.
    //
    //     // self_weak is not really self in that case, it is a pointer to the enclosing component
    //     component_struct.members.push((
    //         Access::Private,
    //         Declaration::Var(Var {
    //             ty: "sixtyfps::cbindgen_private::ComponentWeak".into(),
    //             name: "self_weak".into(),
    //             ..Default::default()
    //         }),
    //     ));
    //
    //     component_struct.members.push((
    //         Access::Private,
    //         Declaration::Var(Var {
    //             ty: "uintptr_t".to_owned(),
    //             name: "tree_index_of_first_child".to_owned(),
    //             ..Default::default()
    //         }),
    //     ));
    //     constructor_member_initializers
    //         .push("tree_index_of_first_child(tree_index_of_first_child)".into());
    //     constructor_code.push("(void)this->tree_index_of_first_child;".into()); // silence warning about unused variable.
    //
    //     component_struct.members.push((
    //         Access::Private,
    //         Declaration::Var(Var {
    //             ty: "uintptr_t".to_owned(),
    //             name: "tree_index".to_owned(),
    //             ..Default::default()
    //         }),
    //     ));
    //     constructor_member_initializers.push("tree_index(tree_index)".into());
    //     constructor_code.push("(void)this->tree_index;".into()); // silence warning about unused variable.
    //
    //     let root_element = component.root_element.borrow();
    //     let get_root_item = if root_element.sub_component().is_some() {
    //         format!("{}.root_item()", ident(&root_element.id))
    //     } else {
    //         format!("&{}", ident(&root_element.id))
    //     };
    //     component_struct.members.push((
    //         Access::Public,
    //         Declaration::Function(Function {
    //             name: "root_item".to_owned(),
    //             signature: "() const".into(),
    //             statements: Some(vec![format!("return {};", get_root_item)]),
    //             ..Default::default()
    //         }),
    //     ));
    //
    //
    //     if !children_visitor_cases.is_empty() {
    //         component_struct.members.push((
    //             Access::Public,
    //             Declaration::Function(Function {
    //                 name: "visit_dynamic_children".into(),
    //                 signature: "(intptr_t dyn_index, [[maybe_unused]] sixtyfps::private_api::TraversalOrder order, [[maybe_unused]] sixtyfps::private_api::ItemVisitorRefMut visitor) const -> uint64_t".into(),
    //                 statements: Some(vec![
    //                     "    auto self = this;".to_owned(),
    //                     format!("    switch(dyn_index) {{ {} }};", children_visitor_cases.join("")),
    //                     "    std::abort();".to_owned(),
    //                 ]),
    //                 ..Default::default()
    //             }),
    //         ));
    //     }
    //
    //     init_signature = "(sixtyfps::cbindgen_private::ComponentWeak enclosing_component)";
    //     init.insert(0, "self_weak = enclosing_component;".to_string());
    // }
    //

    // // For globals nobody calls init(), so move the init code into the constructor.
    // // For everything else we want for whoever creates us to call init() when ready.
    // if component.is_global() {
    //     constructor_code.extend(init);
    // } else {
    //     component_struct.members.push((
    //         if !component.is_global() && !is_sub_component {
    //             Access::Private
    //         } else {
    //             Access::Public
    //         },
    //         Declaration::Function(Function {
    //             name: "init".to_owned(),
    //             signature: format!("{} -> void", init_signature),
    //             statements: Some(init),
    //             ..Default::default()
    //         }),
    //     ));
    // }
    // Became:

    //
    // component_struct.members.push((
    //     if !component.is_global() && !is_sub_component { Access::Private } else { Access::Public },
    //     Declaration::Function(Function {
    //         name: component_id,
    //         signature: format!("({})", constructor_arguments),
    //         is_constructor_or_destructor: true,
    //         statements: Some(constructor_code),
    //         constructor_member_initializers,
    //         ..Default::default()
    //     }),
    // ));
    //
    // let used_types = component.used_types.borrow();
    //
    // let global_field_name = |glob| format!("global_{}", self::component_id(glob));
    //

    for glob in &component.globals {
        let ty = if glob.is_builtin {
            format!("sixtyfps::cbindgen_private::{}", glob.name)
        } else {
            todo!()
        };

        component_struct.members.push((
            Access::Public, // FIXME
            Declaration::Var(Var {
                ty: format!("std::shared_ptr<{}>", ty),
                name: format!("global_{}", ident(&glob.name)),
                init: Some(format!("std::make_shared<{}>()", ty)),
                ..Default::default()
            }),
        ));
    }

    // for glob in used_types.globals.iter() {
    //     let ty = match &glob.root_element.borrow().base_type {
    //         Type::Void => self::component_id(glob),
    //         Type::Builtin(b) => {
    //             format!("sixtyfps::cbindgen_private::{}", b.native_class.class_name)
    //         }
    //         _ => unreachable!(),
    //     };
    //
    //     component_struct.members.push((
    //         Access::Private,
    //         Declaration::Var(Var {
    //             ty: format!("std::shared_ptr<{}>", ty),
    //             name: global_field_name(glob),
    //             init: Some(format!("std::make_shared<{}>()", ty)),
    //             ..Default::default()
    //         }),
    //     ));
    // }

    // let mut global_accessor_function_body = Vec::new();
    //
    // for glob in used_types
    //     .globals
    //     .iter()
    //     .filter(|glob| glob.visible_in_public_api() && glob.requires_code_generation())
    // {
    //     let mut accessor_statement = String::new();
    //
    //     if !global_accessor_function_body.is_empty() {
    //         accessor_statement.push_str("else ");
    //     }
    //
    //     accessor_statement.push_str(&format!(
    //         "if constexpr(std::is_same_v<T, {}>) {{ return *{}.get(); }}",
    //         self::component_id(glob),
    //         global_field_name(glob)
    //     ));
    //
    //     global_accessor_function_body.push(accessor_statement);
    // }
    //
    // if !global_accessor_function_body.is_empty() {
    //     global_accessor_function_body.push(
    //         "else { static_assert(!sizeof(T*), \"The type is not global/or exported\"); }".into(),
    //     );
    //
    //     component_struct.members.push((
    //         Access::Public,
    //         Declaration::Function(Function {
    //             name: "global".into(),
    //             signature: "() const -> const T&".into(),
    //             statements: Some(global_accessor_function_body),
    //             template_parameters: Some("typename T".into()),
    //             ..Default::default()
    //         }),
    //     ));
    // }
    //
    file.definitions.extend(component_struct.extract_definitions().collect::<Vec<_>>());
    file.declarations.push(Declaration::Struct(component_struct));
}

/*
fn generate_component(
    file: &mut File,
    component: &Rc<Component>,
    root_component: &Rc<Component>,
    diag: &mut BuildDiagnostics,
    sub_components: &mut Vec<String>,
) {
    let component_id = component_id(component);
    let mut component_struct = Struct { name: component_id.clone(), ..Default::default() };

    let is_child_component = component.parent_element.upgrade().is_some();
    let is_sub_component = component.is_sub_component();

    if component.is_root_component.get() {
        component_struct.friends.extend(
            component
                .used_types
                .borrow()
                .sub_components
                .iter()
                .map(self::component_id)
                .chain(std::mem::take(sub_components).into_iter()),
        );
    }

    for c in component.popup_windows.borrow().iter() {
        let mut friends = vec![self::component_id(&c.component)];
        generate_component(file, &c.component, root_component, diag, &mut friends);
        sub_components.extend_from_slice(friends.as_slice());
        component_struct.friends.append(&mut friends);
    }

    let expose_property = |property: &PropertyDeclaration| -> bool {
        if component.is_global() || component.is_root_component.get() {
            property.expose_in_public_api
        } else {
            false
        }
    };

    let field_access = if !component.is_root_component.get() || component.is_global() {
        Access::Public
    } else {
        Access::Private
    };

    let mut init_signature = "()";
    let mut init = vec!["[[maybe_unused]] auto self = this;".into()];

    for (prop_name, property_decl) in component.root_element.borrow().property_declarations.iter() {
        let cpp_name = ident(prop_name);
        let access = if let Some(alias) = &property_decl.is_alias {
            access_named_reference(alias, component, "this")
        } else {
            format!("this->{}", cpp_name)
        };

        let ty = if let Type::Callback { args, return_type } = &property_decl.property_type {
            let param_types =
                args.iter().map(|t| get_cpp_type(t, property_decl, diag)).collect::<Vec<_>>();
            let return_type = return_type
                .as_ref()
                .map_or("void".into(), |t| get_cpp_type(t, property_decl, diag));
            if expose_property(property_decl) {
                let callback_emitter = vec![format!(
                    "return {}.call({});",
                    access,
                    (0..args.len()).map(|i| format!("arg_{}", i)).join(", ")
                )];
                component_struct.members.push((
                    Access::Public,
                    Declaration::Function(Function {
                        name: format!("invoke_{}", cpp_name),
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
                    }),
                ));
                component_struct.members.push((
                    Access::Public,
                    Declaration::Function(Function {
                        name: format!("on_{}", cpp_name),
                        template_parameters: Some("typename Functor".into()),
                        signature: "(Functor && callback_handler) const".into(),
                        statements: Some(vec![format!(
                            "{}.set_handler(std::forward<Functor>(callback_handler));",
                            access
                        )]),
                        ..Default::default()
                    }),
                ));
            }
            format!("sixtyfps::private_api::Callback<{}({})>", return_type, param_types.join(", "))
        } else {
            let cpp_type = get_cpp_type(&property_decl.property_type, property_decl, diag);

            if expose_property(property_decl) {
                let prop_getter: Vec<String> = vec![format!("return {}.get();", access)];
                component_struct.members.push((
                    Access::Public,
                    Declaration::Function(Function {
                        name: format!("get_{}", cpp_name),
                        signature: format!("() const -> {}", cpp_type),
                        statements: Some(prop_getter),
                        ..Default::default()
                    }),
                ));

                let set_value = if let Some(alias) = &property_decl.is_alias {
                    property_set_value_code(component, &alias.element(), alias.name(), "value")
                } else {
                    property_set_value_code(component, &component.root_element, prop_name, "value")
                };

                let prop_setter: Vec<String> = vec![
                    "[[maybe_unused]] auto self = this;".into(),
                    format!("{}.{};", access, set_value),
                ];
                component_struct.members.push((
                    Access::Public,
                    Declaration::Function(Function {
                        name: format!("set_{}", cpp_name),
                        signature: format!("(const {} &value) const", cpp_type),
                        statements: Some(prop_setter),
                        ..Default::default()
                    }),
                ));
            }
            format!("sixtyfps::private_api::Property<{}>", cpp_type)
        };

        if is_sub_component {
            component_struct.members.push((
                Access::Public,
                Declaration::Function(Function {
                    name: format!("get_{}", &cpp_name),
                    signature: "() const".to_owned(),
                    statements: Some(vec![format!("return &{};", access)]),
                    ..Default::default()
                }),
            ));
        }
        if property_decl.is_alias.is_none() {
            component_struct.members.push((
                field_access,
                Declaration::Var(Var { ty, name: cpp_name, ..Default::default() }),
            ));
        }
    }

    let mut constructor_arguments = String::new();
    let mut constructor_member_initializers = vec![];
    let mut constructor_code = vec![];

    if component.is_root_component.get() || is_child_component {
        let mut window_init = None;
        let mut access = Access::Private;

        if component.is_root_component.get() {
            window_init = Some("sixtyfps::Window{sixtyfps::private_api::WindowRc()}".into());
            // FIXME: many of the different component bindings need to access this
            access = Access::Public;
        } else {
            constructor_member_initializers
                .push("m_window(parent->m_window.window_handle())".into());
        }

        component_struct.members.push((
            access,
            Declaration::Var(Var {
                ty: "sixtyfps::Window".into(),
                name: "m_window".into(),
                init: window_init,
                ..Default::default()
            }),
        ));

        component_struct.members.push((
            access,
            Declaration::Var(Var {
                ty: format!(
                    "vtable::VWeak<sixtyfps::private_api::ComponentVTable, {}>",
                    component_id
                ),
                name: "self_weak".into(),
                ..Var::default()
            }),
        ));
    }

    if let Some(parent_element) = component.parent_element.upgrade() {
        if parent_element.borrow().repeated.as_ref().map_or(false, |r| !r.is_conditional_element) {
            let cpp_model_data_type = model_data_type(&parent_element, diag);
            component_struct.members.push((
                Access::Private,
                Declaration::Var(Var {
                    ty: "sixtyfps::private_api::Property<int>".into(),
                    name: "index".into(),
                    ..Default::default()
                }),
            ));

            component_struct.members.push((
                Access::Private,
                Declaration::Var(Var {
                    ty: format!("sixtyfps::private_api::Property<{}>", cpp_model_data_type),
                    name: "model_data".into(),
                    ..Default::default()
                }),
            ));

            let update_statements = vec!["index.set(i);".into(), "model_data.set(data);".into()];
            component_struct.members.push((
                Access::Public, // Because Repeater accesses it
                Declaration::Function(Function {
                    name: "update_data".into(),
                    signature: format!(
                        "(int i, const {} &data) const -> void",
                        cpp_model_data_type
                    ),
                    statements: Some(update_statements),
                    ..Function::default()
                }),
            ));
        } else if parent_element.borrow().repeated.is_some() {
            component_struct.members.push((
                Access::Public, // Because Repeater accesses it
                Declaration::Function(Function {
                    name: "update_data".into(),
                    signature: "(int, int) const -> void".into(),
                    statements: Some(vec![]),
                    ..Function::default()
                }),
            ));
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
        constructor_arguments = format!("{} parent", parent_type);
        constructor_code.push("this->parent = parent;".into());
        component_struct.members.push((
            Access::Public, // Because Repeater accesses it
            Declaration::Var(Var {
                ty: parent_type,
                name: "parent".into(),
                init: Some("nullptr".to_owned()),
                ..Default::default()
            }),
        ));
        component_struct.friends.push(parent_component_id);

        if parent_element.borrow().repeated.as_ref().map_or(false, |r| r.is_listview.is_some()) {
            let p_y = access_member(&component.root_element, "y", component, "this");
            let p_height = access_member(&component.root_element, "height", component, "this");
            let p_width = access_member(&component.root_element, "width", component, "this");

            component_struct.members.push((
                Access::Public, // Because Repeater accesses it
                Declaration::Function(Function {
                    name: "listview_layout".into(),
                    signature:
                        "(float *offset_y, const sixtyfps::private_api::Property<float> *viewport_width) const -> void"
                            .to_owned(),
                    statements: Some(vec![
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
        } else if parent_element.borrow().repeated.is_some() {
            component_struct.members.push((
                Access::Public, // Because Repeater accesses it
                Declaration::Function(Function {
                    name: "box_layout_data".into(),
                    signature: "(sixtyfps::Orientation o) const -> sixtyfps::BoxLayoutCellData".to_owned(),
                    statements: Some(vec!["return { layout_info({&static_vtable, const_cast<void *>(static_cast<const void *>(this))}, o) };".into()]),

                    ..Function::default()
                }),
            ));
        }
    }

    if component.is_root_component.get() {
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
                signature: "() const -> sixtyfps::Window&".into(),
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
                    "sixtyfps::run_event_loop();".into(),
                    "hide();".into(),
                ]),
                ..Default::default()
            }),
        ));

        init.insert(0, "m_window.window_handle().init_items(this, item_tree());".into());

        component_struct.friends.push("sixtyfps::private_api::WindowRc".into());
    }

    struct TreeBuilder<'a> {
        tree_array: Vec<String>,
        children_visitor_cases: Vec<String>,
        constructor_member_initializers: &'a mut Vec<String>,
        root_ptr: String,
        item_index_base: &'static str,
        field_access: Access,
        component_struct: &'a mut Struct,
        component: &'a Rc<Component>,
        root_component: &'a Rc<Component>,
        diag: &'a mut BuildDiagnostics,
        file: &'a mut File,
        sub_components: &'a mut Vec<String>,
        init: &'a mut Vec<String>,
    }
    impl<'a> super::ItemTreeBuilder for TreeBuilder<'a> {
        // offsetof(App, sub_component) + offsetof(SubComponent, sub_sub_component) + ...
        type SubComponentState = String;

        fn push_repeated_item(
            &mut self,
            item_rc: &crate::object_tree::ElementRc,
            repeater_count: u32,
            parent_index: u32,
            component_state: &Self::SubComponentState,
        ) {
            if component_state.is_empty() {
                let item = item_rc.borrow();
                let base_component = item.base_type.as_component();
                let mut friends = Vec::new();
                generate_component(
                    self.file,
                    base_component,
                    self.root_component,
                    self.diag,
                    &mut friends,
                );
                self.sub_components.extend_from_slice(friends.as_slice());
                self.sub_components.push(self::component_id(base_component));
                self.component_struct.friends.append(&mut friends);
                self.component_struct.friends.push(self::component_id(base_component));
                let repeated = item.repeated.as_ref().unwrap();
                handle_repeater(
                    repeated,
                    base_component,
                    self.component,
                    repeater_count,
                    self.component_struct,
                    self.init,
                    &mut self.children_visitor_cases,
                    self.diag,
                );
            }

            self.tree_array.push(format!(
                "sixtyfps::private_api::make_dyn_node({}, {})",
                repeater_count, parent_index
            ));
        }
        fn push_native_item(
            &mut self,
            item_rc: &ElementRc,
            children_offset: u32,
            parent_index: u32,
            component_state: &Self::SubComponentState,
        ) {
            let item = item_rc.borrow();
            if component_state.is_empty() {
                handle_item(item_rc, self.field_access, self.component_struct);
            }
            if item.is_flickable_viewport {
                self.tree_array.push(format!(
                    "sixtyfps::private_api::make_item_node({}offsetof({}, {}) + offsetof(sixtyfps::cbindgen_private::Flickable, viewport), SIXTYFPS_GET_ITEM_VTABLE(RectangleVTable), {}, {}, {})",
                    component_state,
                    &self::component_id(&item.enclosing_component.upgrade().unwrap()),
                    ident(&crate::object_tree::find_parent_element(item_rc).unwrap().borrow().id),
                    item.children.len(),
                    children_offset,
                    parent_index,
                ));
            } else if let Type::Native(native_class) = &item.base_type {
                self.tree_array.push(format!(
                    "sixtyfps::private_api::make_item_node({}offsetof({}, {}), {}, {}, {}, {})",
                    component_state,
                    &self::component_id(&item.enclosing_component.upgrade().unwrap()),
                    ident(&item.id),
                    native_class.cpp_vtable_getter,
                    item.children.len(),
                    children_offset,
                    parent_index,
                ));
            } else {
                panic!("item don't have a native type");
            }
        }
        fn enter_component(
            &mut self,
            item_rc: &ElementRc,
            sub_component: &Rc<Component>,
            children_offset: u32,
            component_state: &Self::SubComponentState,
        ) -> Self::SubComponentState {
            let item = item_rc.borrow();
            // Sub-components don't have an entry in the item tree themselves, but we propagate their tree offsets through the constructors.
            if component_state.is_empty() {
                let class_name = self::component_id(sub_component);
                let member_name = ident(&item.id);

                self.init.push(format!("{}.init(self_weak.into_dyn());", member_name));

                self.component_struct.members.push((
                    self.field_access,
                    Declaration::Var(Var {
                        ty: class_name,
                        name: member_name,
                        ..Default::default()
                    }),
                ));

                self.constructor_member_initializers.push(format!(
                    "{member_name}{{{root_ptr}, {item_index_base}{local_index}, {item_index_base}{local_children_offset}}}",
                    root_ptr = self.root_ptr,
                    member_name = ident(&item.id),
                    item_index_base = self.item_index_base,
                    local_index = item.item_index.get().unwrap(),
                    local_children_offset = children_offset
                ));
            };

            format!(
                "{}offsetof({}, {}) + ",
                component_state,
                &self::component_id(&item.enclosing_component.upgrade().unwrap()),
                ident(&item.id)
            )
        }

        fn enter_component_children(
            &mut self,
            item_rc: &ElementRc,
            repeater_count: u32,
            component_state: &Self::SubComponentState,
            _sub_component_state: &Self::SubComponentState,
        ) {
            let item = item_rc.borrow();
            if component_state.is_empty() {
                let sub_component = item.sub_component().unwrap();
                let member_name = ident(&item.id);

                let sub_component_repeater_count = sub_component.repeater_count();
                if sub_component_repeater_count > 0 {
                    let mut case_code = String::new();
                    for local_repeater_index in 0..sub_component_repeater_count {
                        write!(case_code, "case {}: ", repeater_count + local_repeater_index)
                            .unwrap();
                    }

                    self.children_visitor_cases.push(format!(
                        "\n        {case_code} {{
                                return self->{id}.visit_dynamic_children(dyn_index - {base}, order, visitor);
                            }}",
                        case_code = case_code,
                        id = member_name,
                        base = repeater_count,
                    ));
                }
            }
        }
    }

    // For children of sub-components, the item index generated by the generate_item_indices pass
    // starts at 1 (0 is the root element).
    let item_index_base = if is_sub_component { "tree_index_of_first_child - 1 + " } else { "" };

    let mut builder = TreeBuilder {
        tree_array: vec![],
        children_visitor_cases: vec![],
        constructor_member_initializers: &mut constructor_member_initializers,
        root_ptr: access_root_tokens(component),
        item_index_base,
        field_access,
        component_struct: &mut component_struct,
        component,
        root_component,
        diag,
        file,
        sub_components,
        init: &mut init,
    };
    if !component.is_global() {
        super::build_item_tree(component, &String::new(), &mut builder);
    }

    let tree_array = std::mem::take(&mut builder.tree_array);
    let children_visitor_cases = std::mem::take(&mut builder.children_visitor_cases);
    drop(builder);

    super::handle_property_bindings_init(component, |elem, prop, binding| {
        handle_property_binding(elem, prop, binding, &mut init)
    });

    if is_child_component || component.is_root_component.get() {
        let maybe_constructor_param = if constructor_arguments.is_empty() { "" } else { "parent" };

        let mut create_code = vec![
            format!("auto self_rc = vtable::VRc<sixtyfps::private_api::ComponentVTable, {0}>::make({1});", component_id, maybe_constructor_param),
            format!("auto self = const_cast<{0} *>(&*self_rc);", component_id),
            "self->self_weak = vtable::VWeak(self_rc);".into(),
            "self->init();".into(),
        ];

        if component.is_root_component.get() {
            create_code.push(
                "self->m_window.window_handle().set_component(**self->self_weak.lock());".into(),
            );
        }

        create_code.extend(
            component.setup_code.borrow().iter().map(|code| compile_expression(code, component)),
        );
        create_code
            .push(format!("return sixtyfps::ComponentHandle<{0}>{{ self_rc }};", component_id));

        component_struct.members.push((
            Access::Public,
            Declaration::Function(Function {
                name: "create".into(),
                signature: format!(
                    "({}) -> sixtyfps::ComponentHandle<{}>",
                    constructor_arguments, component_id
                ),
                statements: Some(create_code),
                is_static: true,
                ..Default::default()
            }),
        ));

        let mut destructor = vec!["[[maybe_unused]] auto self = this;".to_owned()];

        if is_child_component {
            destructor.push("if (!parent) return;".to_owned())
        }

        destructor
            .push("m_window.window_handle().free_graphics_resources(this, item_tree());".into());

        component_struct.members.push((
            Access::Public,
            Declaration::Function(Function {
                name: format!("~{}", component_id),
                signature: "()".to_owned(),
                is_constructor_or_destructor: true,
                statements: Some(destructor),
                ..Default::default()
            }),
        ));

        generate_component_vtable(
            &mut component_struct,
            component_id.clone(),
            component,
            children_visitor_cases,
            tree_array,
            file,
        );
    } else if is_sub_component {
        let root_ptr_type = format!("const {} *", self::component_id(root_component));

        constructor_arguments =
            format!("{} root, [[maybe_unused]] uintptr_t tree_index, [[maybe_unused]] uintptr_t tree_index_of_first_child", root_ptr_type);

        component_struct.members.push((
            Access::Private,
            Declaration::Var(Var {
                ty: "sixtyfps::Window".into(),
                name: "m_window".into(),
                ..Default::default()
            }),
        ));
        constructor_member_initializers.push("m_window(root->m_window.window_handle())".into());

        component_struct.members.push((
            Access::Private,
            Declaration::Var(Var {
                ty: root_ptr_type,
                name: "m_root".to_owned(),
                ..Default::default()
            }),
        ));
        constructor_member_initializers.push("m_root(root)".into());
        constructor_code.push("(void)m_root;".into()); // silence warning about unused variable.

        // self_weak is not really self in that case, it is a pointer to the enclosing component
        component_struct.members.push((
            Access::Private,
            Declaration::Var(Var {
                ty: "sixtyfps::cbindgen_private::ComponentWeak".into(),
                name: "self_weak".into(),
                ..Default::default()
            }),
        ));

        component_struct.members.push((
            Access::Private,
            Declaration::Var(Var {
                ty: "uintptr_t".to_owned(),
                name: "tree_index_of_first_child".to_owned(),
                ..Default::default()
            }),
        ));
        constructor_member_initializers
            .push("tree_index_of_first_child(tree_index_of_first_child)".into());
        constructor_code.push("(void)this->tree_index_of_first_child;".into()); // silence warning about unused variable.

        component_struct.members.push((
            Access::Private,
            Declaration::Var(Var {
                ty: "uintptr_t".to_owned(),
                name: "tree_index".to_owned(),
                ..Default::default()
            }),
        ));
        constructor_member_initializers.push("tree_index(tree_index)".into());
        constructor_code.push("(void)this->tree_index;".into()); // silence warning about unused variable.

        let root_element = component.root_element.borrow();
        let get_root_item = if root_element.sub_component().is_some() {
            format!("{}.root_item()", ident(&root_element.id))
        } else {
            format!("&{}", ident(&root_element.id))
        };
        component_struct.members.push((
            Access::Public,
            Declaration::Function(Function {
                name: "root_item".to_owned(),
                signature: "() const".into(),
                statements: Some(vec![format!("return {};", get_root_item)]),
                ..Default::default()
            }),
        ));

        if !children_visitor_cases.is_empty() {
            component_struct.members.push((
                Access::Public,
                Declaration::Function(Function {
                    name: "visit_dynamic_children".into(),
                    signature: "(intptr_t dyn_index, [[maybe_unused]] sixtyfps::private_api::TraversalOrder order, [[maybe_unused]] sixtyfps::private_api::ItemVisitorRefMut visitor) const -> uint64_t".into(),
                    statements: Some(vec![
                        "    auto self = this;".to_owned(),
                        format!("    switch(dyn_index) {{ {} }};", children_visitor_cases.join("")),
                        "    std::abort();".to_owned(),
                    ]),
                    ..Default::default()
                }),
            ));
        }

        init_signature = "(sixtyfps::cbindgen_private::ComponentWeak enclosing_component)";
        init.insert(0, "self_weak = enclosing_component;".to_string());
    }

    // For globals nobody calls init(), so move the init code into the constructor.
    // For everything else we want for whoever creates us to call init() when ready.
    if component.is_global() {
        constructor_code.extend(init);
    } else {
        component_struct.members.push((
            if !component.is_global() && !is_sub_component {
                Access::Private
            } else {
                Access::Public
            },
            Declaration::Function(Function {
                name: "init".to_owned(),
                signature: format!("{} -> void", init_signature),
                statements: Some(init),
                ..Default::default()
            }),
        ));
    }

    component_struct.members.push((
        if !component.is_global() && !is_sub_component { Access::Private } else { Access::Public },
        Declaration::Function(Function {
            name: component_id,
            signature: format!("({})", constructor_arguments),
            is_constructor_or_destructor: true,
            statements: Some(constructor_code),
            constructor_member_initializers,
            ..Default::default()
        }),
    ));

    let used_types = component.used_types.borrow();

    let global_field_name = |glob| format!("global_{}", self::component_id(glob));

    for glob in used_types.globals.iter() {
        let ty = match &glob.root_element.borrow().base_type {
            Type::Void => self::component_id(glob),
            Type::Builtin(b) => {
                format!("sixtyfps::cbindgen_private::{}", b.native_class.class_name)
            }
            _ => unreachable!(),
        };

        component_struct.members.push((
            Access::Private,
            Declaration::Var(Var {
                ty: format!("std::shared_ptr<{}>", ty),
                name: global_field_name(glob),
                init: Some(format!("std::make_shared<{}>()", ty)),
                ..Default::default()
            }),
        ));
    }
    let mut global_accessor_function_body = Vec::new();

    for glob in used_types
        .globals
        .iter()
        .filter(|glob| glob.visible_in_public_api() && glob.requires_code_generation())
    {
        let mut accessor_statement = String::new();

        if !global_accessor_function_body.is_empty() {
            accessor_statement.push_str("else ");
        }

        accessor_statement.push_str(&format!(
            "if constexpr(std::is_same_v<T, {}>) {{ return *{}.get(); }}",
            self::component_id(glob),
            global_field_name(glob)
        ));

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
*/

/*
fn generate_component_vtable(
    component_struct: &mut Struct,
    component_id: String,
    component: &Rc<Component>,
    children_visitor_cases: Vec<String>,
    tree_array: Vec<String>,
    file: &mut File,
) {
    component_struct
        .friends
        .push(format!("vtable::VRc<sixtyfps::private_api::ComponentVTable, {}>", component_id));
    component_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "visit_children".into(),
            signature: "(sixtyfps::private_api::ComponentRef component, intptr_t index, sixtyfps::private_api::TraversalOrder order, sixtyfps::private_api::ItemVisitorRefMut visitor) -> uint64_t".into(),
            is_static: true,
            statements: Some(vec![
                "static const auto dyn_visit = [] (const uint8_t *base,  [[maybe_unused]] sixtyfps::private_api::TraversalOrder order, [[maybe_unused]] sixtyfps::private_api::ItemVisitorRefMut visitor, uintptr_t dyn_index) -> uint64_t {".to_owned(),
                format!("    [[maybe_unused]] auto self = reinterpret_cast<const {}*>(base);", component_id),
                format!("    switch(dyn_index) {{ {} }};", children_visitor_cases.join("")),
                "    std::abort();\n};".to_owned(),
                format!("auto self_rc = reinterpret_cast<const {}*>(component.instance)->self_weak.lock()->into_dyn();", component_id),
                "return sixtyfps::cbindgen_private::sixtyfps_visit_item_tree(&self_rc, item_tree() , index, order, visitor, dyn_visit);".to_owned(),
            ]),
            ..Default::default()
        }),
    ));
    component_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "get_item_ref".into(),
            signature: "(sixtyfps::private_api::ComponentRef component, uintptr_t index) -> sixtyfps::private_api::ItemRef".into(),
            is_static: true,
            statements: Some(vec![
                "return sixtyfps::private_api::get_item_ref(component, item_tree(), index);".to_owned(),
            ]),
            ..Default::default()
        }),
    ));
    let parent_item_from_parent_component = if let Some(parent_index) =
        component.parent_element.upgrade().and_then(|e| e.borrow().item_index.get().copied())
    {
        format!(
            // that does not work when the parent is not a component with a ComponentVTable
            //"   *result = sixtyfps::private_api::parent_item(self->parent->self_weak.into_dyn(), self->parent->item_tree(), {});",
            "self->parent->self_weak.vtable()->parent_item(self->parent->self_weak.lock()->borrow(), {}, result);",
            parent_index,
        )
    } else {
        "".to_owned()
    };
    component_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "parent_item".into(),
            signature: "(sixtyfps::private_api::ComponentRef component, uintptr_t index, sixtyfps::private_api::ItemWeak *result) -> void".into(),
            is_static: true,
            statements: Some(vec![
                format!("auto self = reinterpret_cast<const {}*>(component.instance);", component_id),
                "if (index == 0) {".into(),
                parent_item_from_parent_component,
                "   return;".into(),
                "}".into(),
                "*result = sixtyfps::private_api::parent_item(self->self_weak.into_dyn(), item_tree(), index);".into(),
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
                format!("    {} }};", tree_array.join(", \n")),
                "return { const_cast<sixtyfps::private_api::ItemTreeNode*>(children), std::size(children) };"
                    .to_owned(),
            ]),
            ..Default::default()
        }),
    ));
    component_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "layout_info".into(),
            signature:
                "([[maybe_unused]] sixtyfps::private_api::ComponentRef component, sixtyfps::Orientation o) -> sixtyfps::LayoutInfo"
                    .into(),
            is_static: true,
            statements: Some(layout_info_function_body(component, format!(
                "[[maybe_unused]] auto self = reinterpret_cast<const {}*>(component.instance);",
                component_id
            ), None)),
            ..Default::default()
        }),
    ));
    component_struct.members.push((
        Access::Public,
        Declaration::Var(Var {
            ty: "static const sixtyfps::private_api::ComponentVTable".to_owned(),
            name: "static_vtable".to_owned(),
            ..Default::default()
        }),
    ));
    let root_elem = component.root_element.borrow();

    let get_root_item_ref = if root_elem.sub_component().is_some() {
        format!("this->{id}.root_item()", id = ident(&root_elem.id))
    } else {
        format!("&this->{id}", id = ident(&root_elem.id))
    };

    let mut builtin_root_element = component.root_element.clone();
    while let Some(sub_component) = builtin_root_element.clone().borrow().sub_component() {
        builtin_root_element = sub_component.root_element.clone();
    }

    component_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "root_item".into(),
            signature: "() const -> sixtyfps::private_api::ItemRef".into(),
            statements: Some(vec![format!(
                "return {{ {vt}, const_cast<sixtyfps::cbindgen_private::{cpp_type}*>({root_item_ref}) }};",
                cpp_type = builtin_root_element.borrow().base_type.as_native().class_name,
                vt = builtin_root_element.borrow().base_type.as_native().cpp_vtable_getter,
                root_item_ref = get_root_item_ref
            )]),
            ..Default::default()
        }),
    ));
    file.definitions.push(Declaration::Var(Var {
        ty: "const sixtyfps::private_api::ComponentVTable".to_owned(),
        name: format!("{}::static_vtable", component_id),
        init: Some(format!(
            "{{ visit_children, get_item_ref, parent_item,  layout_info, sixtyfps::private_api::drop_in_place<{}>, sixtyfps::private_api::dealloc }}",
            component_id)
        ),
        ..Default::default()
    }));
}
*/

fn generate_item_tree(
    target_struct: &mut Struct,
    sub_tree: &llr::ItemTree,
    root: &llr::PublicComponent,
    parent_ctx: Option<ParentCtx>,
    item_tree_class_name: String,
    file: &mut File,
) {
    target_struct.friends.push(format!(
        "vtable::VRc<sixtyfps::private_api::ComponentVTable, {}>",
        item_tree_class_name
    ));

    generate_sub_component(target_struct, &sub_tree.root, root, parent_ctx.clone(), file);

    let root_access = if parent_ctx.is_some() { "parent->root" } else { "self" };

    let mut tree_array: Vec<String> = Default::default();

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
            tree_array.push(format!(
                "sixtyfps::private_api::make_dyn_node({}, {})",
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
                todo!()
            } else {
                let children_count = node.children.len() as u32;
                let children_index = children_offset as u32;

                tree_array.push(format!(
                    "sixtyfps::private_api::make_item_node({} offsetof({}, {}), {}, {}, {}, {})",
                    compo_offset,
                    &ident(&sub_component.name),
                    ident(&item.name),
                    item.ty.cpp_vtable_getter,
                    children_count,
                    children_index,
                    parent_index,
                ));
            }
        }
    });

    let mut visit_children_statements = vec![
        "static const auto dyn_visit = [] (const uint8_t *base,  [[maybe_unused]] sixtyfps::private_api::TraversalOrder order, [[maybe_unused]] sixtyfps::private_api::ItemVisitorRefMut visitor, [[maybe_unused]] uintptr_t dyn_index) -> uint64_t {".to_owned(),
        format!("    [[maybe_unused]] auto self = reinterpret_cast<const {}*>(base);", item_tree_class_name)];

    if target_struct.members.iter().any(|(_, declaration)| match &declaration {
        Declaration::Function(func @ Function { .. }) if func.name == "visit_dynamic_children" => {
            true
        }
        _ => false,
    }) {
        visit_children_statements
            .push("    return self->visit_dynamic_children(dyn_index, order, visitor);".into());
    }

    visit_children_statements.extend([        
        "    std::abort();\n};".to_owned(),
        format!("auto self_rc = reinterpret_cast<const {}*>(component.instance)->self_weak.lock()->into_dyn();", item_tree_class_name),
        "return sixtyfps::cbindgen_private::sixtyfps_visit_item_tree(&self_rc, item_tree() , index, order, visitor, dyn_visit);".to_owned(),
    ]);

    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "visit_children".into(),
            signature: "(sixtyfps::private_api::ComponentRef component, intptr_t index, sixtyfps::private_api::TraversalOrder order, sixtyfps::private_api::ItemVisitorRefMut visitor) -> uint64_t".into(),
            is_static: true,
            statements: Some(visit_children_statements),
            ..Default::default()
        }),
    ));
    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "get_item_ref".into(),
            signature: "(sixtyfps::private_api::ComponentRef component, uintptr_t index) -> sixtyfps::private_api::ItemRef".into(),
            is_static: true,
            statements: Some(vec![
                "return sixtyfps::private_api::get_item_ref(component, item_tree(), index);".to_owned(),
            ]),
            ..Default::default()
        }),
    ));

    let parent_item_from_parent_component = if let Some(parent_index) =
        parent_ctx.as_ref().and_then(|parent| {
            parent
                .repeater_index
                .map(|idx| parent.ctx.current_sub_component.unwrap().repeated[idx].index_in_tree)
        }) {
        format!(
            // that does not work when the parent is not a component with a ComponentVTable
            //"   *result = sixtyfps::private_api::parent_item(self->parent->self_weak.into_dyn(), self->parent->item_tree(), {});",
            "self->parent->self_weak.vtable()->parent_item(self->parent->self_weak.lock()->borrow(), {}, result);",
            parent_index,
        )
    } else {
        "".to_owned()
    };
    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "parent_item".into(),
            signature: "(sixtyfps::private_api::ComponentRef component, uintptr_t index, sixtyfps::private_api::ItemWeak *result) -> void".into(),
            is_static: true,
            statements: Some(vec![
                format!("auto self = reinterpret_cast<const {}*>(component.instance);", item_tree_class_name),
                "if (index == 0) {".into(),
                parent_item_from_parent_component,
                "   return;".into(),
                "}".into(),
                "*result = sixtyfps::private_api::parent_item(self->self_weak.into_dyn(), item_tree(), index);".into(),
            ]),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        Access::Private,
        Declaration::Function(Function {
            name: "item_tree".into(),
            signature: "() -> sixtyfps::Slice<sixtyfps::private_api::ItemTreeNode>".into(),
            is_static: true,
            statements: Some(vec![
                "static const sixtyfps::private_api::ItemTreeNode children[] {".to_owned(),
                format!("    {} }};", tree_array.join(", \n")),
                "return { const_cast<sixtyfps::private_api::ItemTreeNode*>(children), std::size(children) };"
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
                "([[maybe_unused]] sixtyfps::private_api::ComponentRef component, sixtyfps::Orientation o) -> sixtyfps::LayoutInfo"
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
        Access::Public,
        Declaration::Var(Var {
            ty: "static const sixtyfps::private_api::ComponentVTable".to_owned(),
            name: "static_vtable".to_owned(),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        Access::Public,
        Declaration::Var(Var {
            ty: format!(
                "vtable::VWeak<sixtyfps::private_api::ComponentVTable, {}>",
                item_tree_class_name
            ),
            name: "self_weak".into(),
            ..Var::default()
        }),
    ));

    /*
    let root_elem = component.root_element.borrow();

    let get_root_item_ref = if root_elem.sub_component().is_some() {
        format!("this->{id}.root_item()", id = ident(&root_elem.id))
    } else {
        format!("&this->{id}", id = ident(&root_elem.id))
    };

    let mut builtin_root_element = component.root_element.clone();
    while let Some(sub_component) = builtin_root_element.clone().borrow().sub_component() {
        builtin_root_element = sub_component.root_element.clone();
    }

    target_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "root_item".into(),
            signature: "() const -> sixtyfps::private_api::ItemRef".into(),
            statements: Some(vec![format!(
                "return {{ {vt}, const_cast<sixtyfps::cbindgen_private::{cpp_type}*>({root_item_ref}) }};",
                cpp_type = builtin_root_element.borrow().base_type.as_native().class_name,
                vt = builtin_root_element.borrow().base_type.as_native().cpp_vtable_getter,
                root_item_ref = get_root_item_ref
            )]),
            ..Default::default()
        }),
    ));
    */
    file.definitions.push(Declaration::Var(Var {
        ty: "const sixtyfps::private_api::ComponentVTable".to_owned(),
        name: format!("{}::static_vtable", item_tree_class_name),
        init: Some(format!(
            "{{ visit_children, get_item_ref, parent_item,  layout_info, sixtyfps::private_api::drop_in_place<{}>, sixtyfps::private_api::dealloc }}",
            item_tree_class_name)
        ),
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
            "auto self_rc = vtable::VRc<sixtyfps::private_api::ComponentVTable, {0}>::make();",
            target_struct.name
        ),
        format!("auto self = const_cast<{0} *>(&*self_rc);", target_struct.name),
        "self->self_weak = vtable::VWeak(self_rc);".into(),
        format!("{}->m_window.window_handle().init_items(self, item_tree());", root_access),
        format!("self->init({}, 0, 1 {});", root_access, init_parent_parameters),
        format!(
            "{}->m_window.window_handle().set_component(**self->self_weak.lock());",
            root_access
        ),
    ];

    // FIXME: Implement this
    // create_code.extend(
    //     component.setup_code.borrow().iter().map(|code| compile_expression(code, component)),
    // );
    create_code
        .push(format!("return sixtyfps::ComponentHandle<{0}>{{ self_rc }};", target_struct.name));

    target_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "create".into(),
            signature: format!(
                "({}) -> sixtyfps::ComponentHandle<{}>",
                create_parameters.join(","),
                target_struct.name
            ),
            statements: Some(create_code),
            is_static: true,
            ..Default::default()
        }),
    ));

    let mut destructor = vec!["[[maybe_unused]] auto self = this;".to_owned()];

    destructor.push(format!(
        "{}->m_window.window_handle().free_graphics_resources(this, item_tree());",
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
    file: &mut File,
) {
    let root_ptr_type = format!("const {} *", ident(&root.item_tree.root.name));

    let mut init_parameters = vec![
        format!("{} root", root_ptr_type),
        "[[maybe_unused]] uintptr_t tree_index".into(),
        "[[maybe_unused]] uintptr_t tree_index_of_first_child".into(),
    ];

    let mut init: Vec<String> = vec!["[[maybe_unused]] auto self = this;".into()];

    target_struct.members.push((
        Access::Public, // FIXME
        Declaration::Var(Var {
            ty: root_ptr_type.clone(),
            name: "root".to_owned(),
            ..Default::default()
        }),
    ));
    init.push("this->root = root;".into());

    target_struct.members.push((
        Access::Private,
        Declaration::Var(Var {
            ty: "uintptr_t".to_owned(),
            name: "tree_index_of_first_child".to_owned(),
            ..Default::default()
        }),
    ));
    init.push("this->tree_index_of_first_child = tree_index_of_first_child;".into());

    target_struct.members.push((
        Access::Private,
        Declaration::Var(Var {
            ty: "uintptr_t".to_owned(),
            name: "tree_index".to_owned(),
            ..Default::default()
        }),
    ));
    init.push("this->tree_index = tree_index;".into());

    if let Some(parent_ctx) = &parent_ctx {
        let parent_type =
            format!("class {} const *", ident(&parent_ctx.ctx.current_sub_component.unwrap().name));
        init_parameters.push(format!("{} parent", parent_type));

        target_struct.members.push((
            Access::Private,
            Declaration::Var(Var {
                ty: parent_type,
                name: "parent".to_owned(),
                ..Default::default()
            }),
        ));
        init.push("this->parent = parent;".into());
    }

    let ctx = EvaluationContext::new_sub_component(
        root,
        component,
        "this->root".into(),
        parent_ctx.clone(),
    );

    for property in &component.properties {
        let cpp_name = ident(&property.name);
        /*
        let access = if let Some(alias) = &property_decl.is_alias {
            access_named_reference(alias, component, "this")
        } else {
            format!("this->{}", cpp_name)
        };
        */

        let ty = if let Type::Callback { args, return_type } = &property.ty {
            let param_types = args.iter().map(|t| t.cpp_type().unwrap()).collect::<Vec<_>>();
            let return_type =
                return_type.as_ref().map_or("void".to_owned(), |t| t.cpp_type().unwrap());
            format!("sixtyfps::private_api::Callback<{}({})>", return_type, param_types.join(", "))
        } else {
            format!("sixtyfps::private_api::Property<{}>", property.ty.cpp_type().unwrap())
        };

        /*
        if is_sub_component {
            component_struct.members.push((
                Access::Public,
                Declaration::Function(Function {
                    name: format!("get_{}", &cpp_name),
                    signature: "() const".to_owned(),
                    statements: Some(vec![format!("return &{};", access)]),
                    ..Default::default()
                }),
            ));
        }
        */

        target_struct.members.push((
            Access::Public, //FIXME: field_access,
            Declaration::Var(Var { ty, name: cpp_name, ..Default::default() }),
        ));
    }

    let mut children_visitor_cases = Vec::new();

    let mut subcomponent_init_code = Vec::new();
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

        subcomponent_init_code.push(format!(
            "this->{}.init(root, {}, {});",
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
        }

        target_struct.members.push((
            Access::Public,
            Declaration::Var(Var {
                ty: ident(&sub.ty.name),
                name: field_name,
                ..Default::default()
            }),
        ));
    }

    for (prop1, prop2) in &component.two_way_bindings {
        init.push(format!(
            "sixtyfps::private_api::Property<{ty}>::link_two_way(&{p1}, &{p2});",
            ty = ctx.property_ty(prop1).cpp_type().unwrap(),
            p1 = access_member(prop1, &ctx),
            p2 = access_member(prop2, &ctx),
        ));
    }

    let mut properties_init_code = Vec::new();
    for (prop, expression) in &component.property_init {
        handle_property_init(prop, expression, &mut properties_init_code, &ctx)
    }

    for item in &component.items {
        if item.is_flickable_viewport {
            continue;
        }
        target_struct.members.push((
            Access::Public,
            Declaration::Var(Var {
                ty: format!("sixtyfps::cbindgen_private::{}", ident(&item.ty.class_name)),
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
            &repeated,
            root,
            ParentCtx::new(&ctx, Some(idx)),
            &data_type,
            file,
        );

        let repeater_id = format!("repeater_{}", idx);

        let mut model = compile_expression(&repeated.model, &ctx);
        if repeated.model.ty(&ctx) == Type::Bool {
            // bool converts to int
            // FIXME: don't do a heap allocation here
            model = format!("std::make_shared<sixtyfps::private_api::IntModel>({})", model)
        }

        // FIXME: optimize  if repeated.model.is_constant()
        properties_init_code.push(format!(
            "self->{repeater_id}.set_model_binding([self] {{ (void)self; return {model}; }});",
            repeater_id = repeater_id,
            model = model,
        ));

        children_visitor_cases.push(format!(
            "\n        case {i}: {{
                self->{id}.ensure_updated(self);
                return self->{id}.visit(order, visitor);
            }}",
            id = repeater_id,
            i = idx,
        ));

        target_struct.members.push((
            Access::Private,
            Declaration::Var(Var {
                ty: format!(
                    "sixtyfps::private_api::Repeater<class {}, {}>",
                    ident(&repeated.sub_tree.root.name),
                    data_type.cpp_type().unwrap(),
                ),
                name: repeater_id,
                ..Default::default()
            }),
        ));
    }

    init.extend(subcomponent_init_code);
    init.extend(properties_init_code);

    target_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "init".to_owned(),
            signature: format!("({}) -> void", init_parameters.join(",")),
            statements: Some(init),
            ..Default::default()
        }),
    ));

    target_struct.members.push((
        Access::Public,
        Declaration::Function(Function {
            name: "layout_info".into(),
            signature: "(sixtyfps::cbindgen_private::Orientation o) const -> sixtyfps::LayoutInfo"
                .into(),
            statements: Some(vec![
                "[[maybe_unused]] auto self = this;".into(),
                format!(
                    "return o == sixtyfps::cbindgen_private::Orientation::Horizontal ? {} : {};",
                    compile_expression(&component.layout_info_h, &ctx),
                    compile_expression(&component.layout_info_v, &ctx)
                ),
            ]),
            ..Default::default()
        }),
    ));

    if !children_visitor_cases.is_empty() {
        target_struct.members.push((
            Access::Public,
            Declaration::Function(Function {
                name: "visit_dynamic_children".into(),
                signature: "(intptr_t dyn_index, [[maybe_unused]] sixtyfps::private_api::TraversalOrder order, [[maybe_unused]] sixtyfps::private_api::ItemVisitorRefMut visitor) const -> uint64_t".into(),
                statements: Some(vec![
                    "    auto self = this;".to_owned(),
                    format!("    switch(dyn_index) {{ {} }};", children_visitor_cases.join("")),
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
                "(int i, const {} &data) const -> void",
                model_data_type.cpp_type().unwrap()
            ),
            statements: Some(update_statements),
            ..Function::default()
        }),
    ));

    file.definitions.extend(repeater_struct.extract_definitions().collect::<Vec<_>>());
    file.declarations.push(Declaration::Struct(repeater_struct));
}

fn generate_global(file: &mut File, global: &llr::GlobalComponent, root: &llr::PublicComponent) {
    let mut global_struct = Struct { name: ident(&global.name), ..Default::default() };

    for property in &global.properties {
        let cpp_name = ident(&property.name);

        let ty = if let Type::Callback { args, return_type } = &property.ty {
            let param_types = args.iter().map(|t| t.cpp_type().unwrap()).collect::<Vec<_>>();
            let return_type =
                return_type.as_ref().map_or("void".to_owned(), |t| t.cpp_type().unwrap());
            format!("sixtyfps::private_api::Callback<{}({})>", return_type, param_types.join(", "))
        } else {
            format!("sixtyfps::private_api::Property<{}>", property.ty.cpp_type().unwrap())
        };

        /*
        if is_sub_component {
            component_struct.members.push((
                Access::Public,
                Declaration::Function(Function {
                    name: format!("get_{}", &cpp_name),
                    signature: "() const".to_owned(),
                    statements: Some(vec![format!("return &{};", access)]),
                    ..Default::default()
                }),
            ));
        }
        */

        global_struct.members.push((
            // FIXME: this is public (and also was public in the pre-llr generator) because other generated code accesses the
            // fields directly. But it shouldn't be from an API point of view since the same `global_struct` class is public API
            // when the global is exported and exposed in the public component.
            Access::Public,
            Declaration::Var(Var { ty, name: cpp_name, ..Default::default() }),
        ));
    }

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

        let access = access_member(r, &ctx);

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

            let set_value = "set(value)"; // FIXME: Do the right thing here!
                                          // let set_value = if let Some(alias) = &property_decl.is_alias {
                                          //     property_set_value_code(component, &alias.element(), alias.name(), "value")
                                          // } else {
                                          //     property_set_value_code(component, &component.root_element, prop_name, "value")
                                          // };

            let prop_setter: Vec<String> = vec![
                "[[maybe_unused]] auto self = this;".into(),
                format!("{}.{};", access, set_value),
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
    format!("{}->m_window.window_handle()", root)
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
            let property_name = ident(&prop_name);
            let flick = sub_component.items[item_index]
                .is_flickable_viewport
                .then(|| ".viewport")
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
                let property_name = ident(&current_global.properties[*property_index].name);
                format!("global_{}.{}", ident(&current_global.name), property_name)
            } else {
                unreachable!()
            }
        }
        llr::PropertyReference::InNativeItem { sub_component_path, item_index, prop_name } => {
            in_native_item(ctx, sub_component_path, *item_index, prop_name, "this")
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

/*

/// Returns the code that can access the component of the given element
fn access_element_component<'a>(
    element: &ElementRc,
    current_component: &Rc<Component>,
    component_cpp: &'a str,
) -> Cow<'a, str> {
    let e = element.borrow();
    let enclosing_component = e.enclosing_component.upgrade().unwrap();
    if Rc::ptr_eq(current_component, &enclosing_component) {
        component_cpp.into()
    } else {
        access_element_component(
            element,
            &current_component
                .parent_element
                .upgrade()
                .unwrap()
                .borrow()
                .enclosing_component
                .upgrade()
                .unwrap(),
            &format!("{}->parent", component_cpp),
        )
        .to_string()
        .into()
    }
}

// Returns an expression that will compute the absolute item index in the item tree for a
// given element. For elements of a child component or the root component, the item_index
// is already absolute within the corresponding item tree. For sub-components we return an
// expression that computes the value at run-time.
fn absolute_element_item_index_expression(element: &ElementRc) -> String {
    let element = element.borrow();
    let local_index = element.item_index.get().unwrap();
    let enclosing_component = element.enclosing_component.upgrade().unwrap();
    if enclosing_component.is_sub_component() {
        if *local_index == 0 {
            "this->tree_index".to_string()
        } else if *local_index == 1 {
            "this->tree_index_of_first_child".to_string()
        } else {
            format!("this->tree_index_of_first_child + {}", local_index)
        }
    } else {
        local_index.to_string()
    }
}
*/

fn compile_expression(expr: &llr::Expression, ctx: &EvaluationContext) -> String {
    use llr::Expression;
    match expr {
        Expression::StringLiteral(s) => {
            format!(r#"sixtyfps::SharedString(u8"{}")"#, escape_string(s.as_str()))
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
            compile_builtin_function_call(*function, &arguments, ctx)
        }
        Expression::CallBackCall{ callback, arguments } => {
            let f = access_member(callback, ctx);
            let mut a = arguments.iter().map(|a| compile_expression(a, ctx));
            format!("{}.call({})", f, a.join(","))
        }
        Expression::ExtraBuiltinFunctionCall { function, arguments } => {
            let mut a = arguments.iter().map(|a| compile_expression(a, ctx));
            format!("{}({})", ident(&function), a.join(","))
        }
        /*
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
        */
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
            format!("[&](const auto &model, const auto &index){{ model->track_row_data_changes(index); return model->row_data(index); }}({}, {})", compile_expression(array, ctx), compile_expression(index, ctx))
        },
        Expression::Cast { from, to } => {
            let f = compile_expression(&*from, ctx);
            match (from.ty(ctx), to) {
                (Type::Float32, Type::String) | (Type::Int32, Type::String) => {
                    format!("sixtyfps::SharedString::from_number({})", f)
                }
                (Type::Float32, Type::Model) | (Type::Int32, Type::Model) => {
                    format!("std::make_shared<sixtyfps::private_api::IntModel>({})", f)
                }
                (Type::Array(_), Type::Model) => f,
                (Type::Float32, Type::Color) => {
                    format!("sixtyfps::Color::from_argb_encoded({})", f)
                }
                (Type::Color, Type::Brush) => {
                    format!("sixtyfps::Brush({})", f)
                }
                (Type::Brush, Type::Color) => {
                    format!("{}.color()", f)
                }
                (Type::Struct { .. }, Type::Struct{ fields, name: Some(_), ..}) => {
                    format!(
                        "[&](const auto &o){{ {struct_name} s; auto& [{field_members}] = s; {fields}; return s; }}({obj})",
                        struct_name = to.cpp_type().unwrap(),
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

        /*
        Expression::FunctionCall { function, arguments, source_location: _  } => match &**function {
            Expression::BuiltinFunctionReference(BuiltinFunction::SetFocusItem, _) => {
                if arguments.len() != 1 {
                    panic!("internal error: incorrect argument count to SetFocusItem call");
                }
                if let Expression::ElementReference(focus_item) = &arguments[0] {
                    let focus_item = focus_item.upgrade().unwrap();
                    let component_ref = access_element_component(&focus_item, component, "self");
                    format!("self->m_window.window_handle().set_focus_item({}->self_weak.lock()->into_dyn(), {});", component_ref, absolute_element_item_index_expression(&focus_item))
                } else {
                    panic!("internal error: argument to SetFocusItem must be an element")
                }
            }
            Expression::BuiltinFunctionReference(BuiltinFunction::ShowPopupWindow, _) => {
                if arguments.len() != 1 {
                    panic!("internal error: incorrect argument count to SetFocusItem call");
                }
                if let Expression::ElementReference(popup_window) = &arguments[0] {
                    let popup_window = popup_window.upgrade().unwrap();
                    let pop_comp = popup_window.borrow().enclosing_component.upgrade().unwrap();
                    let popup_window_rcid = component_id(&pop_comp);
                    let parent_component = pop_comp.parent_element.upgrade().unwrap().borrow().enclosing_component.upgrade().unwrap();
                    let popup_list = parent_component.popup_windows.borrow();
                    let popup = popup_list.iter().find(|p| Rc::ptr_eq(&p.component, &pop_comp)).unwrap();
                    let x = access_named_reference(&popup.x, component, "self");
                    let y = access_named_reference(&popup.y, component, "self");
                    let parent_component_ref = access_element_component(&popup.parent_element, component, "self");
                    format!(
                        "self->m_window.window_handle().show_popup<{}>(self, {{ {}.get(), {}.get() }}, {{ {}->self_weak.lock()->into_dyn(), {} }} );",
                        popup_window_rcid, x, y,
                        parent_component_ref,
                        absolute_element_item_index_expression(&popup.parent_element),
                    )
                } else {
                    panic!("internal error: argument to SetFocusItem must be an element")
                }
            }
            Expression::BuiltinFunctionReference(BuiltinFunction::ImplicitLayoutInfo(orientation), _) => {

            }
            Expression::BuiltinFunctionReference(BuiltinFunction::RegisterCustomFontByPath, _) => {
                if arguments.len() != 1 {
                    panic!("internal error: incorrect argument count to RegisterCustomFontByPath call");
                }
                if let Expression::StringLiteral(font_path) = &arguments[0] {
                    format!("sixtyfps::private_api::register_font_from_path(\"{}\");", escape_string(font_path))
                } else {
                    panic!("internal error: argument to RegisterCustomFontByPath must be a string literal")
                }
            }
            Expression::BuiltinFunctionReference(BuiltinFunction::RegisterCustomFontByMemory, _) => {
                if arguments.len() != 1 {
                    panic!("internal error: incorrect argument count to RegisterCustomFontByMemory call");
                }
                if let Expression::NumberLiteral(resource_id, _) = &arguments[0] {
                    let resource_id: usize = *resource_id as _;
                    let symbol = format!("sfps_embedded_resource_{}", resource_id);
                    format!("sixtyfps::private_api::register_font_from_data({}, std::size({}));", symbol, symbol)
                } else {
                    panic!("internal error: argument to RegisterCustomFontByMemory must be a number")
                }
            }
            _ => {
                let mut args = arguments.iter().map(|e| compile_expression(e, component));

                format!("{}({})", compile_expression(function, component), args.join(", "))
            }
        },
        */
        Expression::PropertyAssignment { property, value} => {
            let value = compile_expression(value, ctx);
            property_set_value_code(property, &value, ctx)
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
                    '/' => "/(double)",
                    _ => op.encode_utf8(&mut buffer),
                },
            )
        }
        Expression::UnaryOp { sub, op } => {
            format!("({op} {sub})", sub = compile_expression(&*sub, ctx), op = op,)
        }
        Expression::ImageReference { resource_ref, .. }  => {
            match resource_ref {
                crate::expression_tree::ImageReference::None => r#"sixtyfps::Image()"#.to_string(),
                crate::expression_tree::ImageReference::AbsolutePath(path) => format!(r#"sixtyfps::Image::load_from_path(sixtyfps::SharedString(u8"{}"))"#, escape_string(path.as_str())),
                crate::expression_tree::ImageReference::EmbeddedData { resource_id, extension } => {
                    let symbol = format!("sfps_embedded_resource_{}", resource_id);
                    format!(
                        r#"sixtyfps::Image(sixtyfps::cbindgen_private::types::ImageInner::EmbeddedData(sixtyfps::Slice<uint8_t>{{std::data({}), std::size({})}}, sixtyfps::Slice<uint8_t>{{const_cast<uint8_t *>(reinterpret_cast<const uint8_t *>(u8"{}")), {}}}))"#,
                        symbol, symbol, escape_string(extension), extension.as_bytes().len()
                    )
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
                    "std::make_shared<sixtyfps::private_api::ArrayModel<{count},{ty}>>({val})",
                    count = values.len(),
                    ty = ty,
                    val = val.join(", ")
                )
            } else {
                format!(
                    "sixtyfps::Slice<{ty}>{{ std::array<{ty}, {count}>{{ {val} }}.data(), {count} }}",
                    count = values.len(),
                    ty = ty,
                    val = val.join(", ")
                )
            }
        }
        Expression::Struct { ty, values } => {
            if let Type::Struct{fields, ..} = ty {
                let mut elem = fields.keys().map(|k| {
                    values
                        .get(k)
                        .map(|e| compile_expression(e, ctx))
                        .unwrap_or_else(|| "(Error: missing member in object)".to_owned())
                });
                format!("{}{{{}}}", ty.cpp_type().unwrap(), elem.join(", "))
            } else {
                panic!("Expression::Object is not a Type::Object")
            }
        }
        /*
        Expression::PathData(data)  => compile_path(data, component),
        */
        Expression::EasingCurve(EasingCurve::Linear) => "sixtyfps::cbindgen_private::EasingCurve()".into(),
        Expression::EasingCurve(EasingCurve::CubicBezier(a, b, c, d)) => format!(
            "sixtyfps::cbindgen_private::EasingCurve(sixtyfps::cbindgen_private::EasingCurve::Tag::CubicBezier, {}, {}, {}, {})",
            a, b, c, d
        ),
        Expression::LinearGradient{angle, stops} => {
            let angle = compile_expression(angle, ctx);
            let mut stops_it = stops.iter().map(|(color, stop)| {
                let color = compile_expression(color, ctx);
                let position = compile_expression(stop, ctx);
                format!("sixtyfps::private_api::GradientStop{{ {}, {}, }}", color, position)
            });
            format!(
                "[&] {{ const sixtyfps::private_api::GradientStop stops[] = {{ {} }}; return sixtyfps::Brush(sixtyfps::private_api::LinearGradientBrush({}, stops, {})); }}()",
                stops_it.join(", "), angle, stops.len()
            )
        }
        Expression::EnumerationValue(value) => {
            format!("sixtyfps::cbindgen_private::{}::{}", value.enumeration.name, ident(&value.to_string()))
        }
        Expression::ReturnStatement(Some(expr)) => format!(
            "throw sixtyfps::private_api::ReturnWrapper<{}>({})",
            expr.ty(ctx).cpp_type().unwrap_or_default(),
            compile_expression(expr, ctx)
        ),
        Expression::ReturnStatement(None) => "throw sixtyfps::private_api::ReturnWrapper<void>()".to_owned(),
        Expression::LayoutCacheAccess { layout_cache_prop, index, repeater_index } =>  {
            let cache = access_member(layout_cache_prop, ctx);
            if let Some(ri) = repeater_index {
                format!("sixtyfps::private_api::layout_cache_access({}.get(), {}, {})", cache, index, compile_expression(ri, ctx))
            } else {
                format!("{}.get()[{}]", cache, index)
            }
        }
        ,
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
            let cells_variable = ident(&cells_variable);
            let mut cells = match &**unsorted_cells {
                Expression::Array { values, .. } => {
                    values.iter().map(|v| compile_expression(v, ctx))
                }
                _ => panic!("dialog layout unsorted cells not an array"),
            };
            format!("sixtyfps::cbindgen_private::GridLayoutCellData {cv} [] = {{ {c} }};\
                    sixtyfps::cbindgen_private::DialogButtonRole roles[] = {{ {r} }};\
                    sixtyfps::cbindgen_private::sixtyfps_reorder_dialog_button_layout({cv}, {r});\
                    ",
                    r = compile_expression(roles, ctx),
                    cv = cells_variable,
                    c = cells.join(", "),
                )

        }

        _ => todo!("unimplemented llr expression: {:#?}", expr),
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
            "self->m_window.window_handle().scale_factor()".into()
        }
        BuiltinFunction::Debug => {
            "[](auto... args){ (std::cout << ... << args) << std::endl; return nullptr; }"
                .into()
        }
        BuiltinFunction::Mod => format!("static_cast<int>({}) % static_cast<int>({})", a.next().unwrap(), a.next().unwrap()),
        BuiltinFunction::Round => format!("std::round({})", a.next().unwrap()),
        BuiltinFunction::Ceil => format!("std::ceil({})", a.next().unwrap()),
        BuiltinFunction::Floor => format!("std::floor({})", a.next().unwrap()),
        BuiltinFunction::Sqrt => format!("std::sqrt({})", a.next().unwrap()),
        BuiltinFunction::Abs => format!("std::abs({})", a.next().unwrap()),
        BuiltinFunction::Log => format!("std::log({}) / std::log({})", a.next().unwrap(), a.next().unwrap()),
        BuiltinFunction::Pow => format!("std::pow(({}), ({}))", a.next().unwrap(), a.next().unwrap()),
        BuiltinFunction::Sin => format!("std::sin(({}) * {})", a.next().unwrap(), pi_180),
        BuiltinFunction::Cos => format!("std::cos(({}) * {})", a.next().unwrap(), pi_180),
        BuiltinFunction::Tan => format!("std::tan(({}) * {})", a.next().unwrap(), pi_180),
        BuiltinFunction::ASin => format!("std::asin({}) / {}", a.next().unwrap(), pi_180),
        BuiltinFunction::ACos => format!("std::acos({}) / {}", a.next().unwrap(), pi_180),
        BuiltinFunction::ATan => format!("std::atan({}) / {}", a.next().unwrap(), pi_180),
        BuiltinFunction::SetFocusItem => {
            "self->m_window.window_handle().set_focus_item".into()
        }
        BuiltinFunction::ShowPopupWindow => {
            "self->m_window.window_handle().show_popup".into()
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
            "[](const auto &a){ auto e1 = std::end(a); auto e2 = const_cast<char*>(e1); std::strtod(std::begin(a), &e2); return e1 == e2; }"
                .into()
        }
        BuiltinFunction::StringToFloat => {
            "[](const auto &a){ auto e1 = std::end(a); auto e2 = const_cast<char*>(e1); auto r = std::strtod(std::begin(a), &e2); return e1 == e2 ? r : 0; }"
                .into()
        }
        BuiltinFunction::ColorBrighter => {
            "[](const auto &color, float factor) { return color.brighter(factor); }".into()
        }
        BuiltinFunction::ColorDarker => {
            "[](const auto &color, float factor) { return color.darker(factor); }".into()
        }
        BuiltinFunction::ImageSize => {
            format!("{}.size()", a.next().unwrap())
        }
        BuiltinFunction::ArrayLength => {
            "[](const auto &model) { (*model).track_row_count_changes(); return (*model).row_count(); }".into()
        }
        BuiltinFunction::Rgb => {
            "[](int r, int g, int b, float a) {{ return sixtyfps::Color::from_argb_uint8(std::clamp(a * 255., 0., 255.), std::clamp(r, 0, 255), std::clamp(g, 0, 255), std::clamp(b, 0, 255)); }}".into()
        }
        BuiltinFunction::RegisterCustomFontByPath => {
            panic!("internal error: RegisterCustomFontByPath can only be evaluated from within a FunctionCall expression")
        }
        BuiltinFunction::RegisterCustomFontByMemory => {
            panic!("internal error: RegisterCustomFontByMemory can only be evaluated from within a FunctionCall expression")
        }
        BuiltinFunction::ImplicitLayoutInfo(orient) => {
            if let [llr::Expression::PropertyReference(pr)] = arguments {
                let native = native_item(pr, ctx);
                format!(
                    "{vt}->layout_info({{{vt}, const_cast<sixtyfps::cbindgen_private::{ty}*>(&{i})}}, {o}, &{window})",
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

/*
fn compile_assignment(
    lhs: &Expression,
    op: char,
    rhs: String,
    component: &Rc<Component>,
) -> String {
    match lhs {
        Expression::PropertyReference(nr) => {
            let access = access_named_reference(nr, component, "self");
            let set = property_set_value_code(
                component,
                &nr.element(),
                nr.name(),
                &(if op == '=' { rhs } else { format!("{}.get() {} {}", access, op, rhs) }),
            );
            format!("{}.{}", access, set)
        }
        Expression::StructFieldAccess { base, name } => {
            let tmpobj = "tmpobj";
            let get_obj = compile_expression(base, component);
            let ty = base.ty();
            let member = match &ty {
                Type::Struct { fields, name: None, .. } => {
                    let index = fields
                        .keys()
                        .position(|k| k == name)
                        .expect("Expression::ObjectAccess: Cannot find a key in an object");
                    format!("std::get<{}>({})", index, tmpobj)
                }
                Type::Struct { .. } => format!("{}.{}", tmpobj, ident(name)),
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
            let repeater_id = format!("repeater_{}", ident(&element.borrow().id));
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
        Expression::ArrayIndex { array, index } => {
            let array = compile_expression(array, component);
            let index = compile_expression(index, component);
            if op == '=' {
                format!("{}->set_row_data({}, {})", array, index, rhs)
            } else {
                format!(
                    "{array}->set_row_data({index}, {array}->row_data({index}) {op} {rhs})",
                    array = array,
                    index = index,
                    op = op,
                    rhs = rhs,
                )
            }
        }

        _ => panic!("typechecking should make sure this was a PropertyReference"),
    }
}
*/

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
        format!("std::vector<sixtyfps::BoxLayoutCellData> {};", ident(cells_variable));
    let mut repeater_idx = 0usize;

    for item in elements {
        match item {
            Either::Left(value) => {
                push_code += &format!("cells.push_back({{ {} }});", compile_expression(value, ctx));
            }
            Either::Right(repeater) => {
                push_code += &format!("self->repeater_{}.ensure_updated(self);", repeater);

                if let Some(ri) = &repeated_indices {
                    push_code += &format!("{}[{}] = cells.size();", ri, repeater_idx * 2);
                    push_code += &format!(
                        "{ri}[{c}] = self->repeater_{id}.inner ? self->repeater_{id}.inner->data.size() : 0;",
                        ri = ri,
                        c = repeater_idx * 2 + 1,
                        id = repeater,
                    );
                }
                repeater_idx += 1;
                push_code += &format!(
                    "if (self->repeater_{id}.inner) \
                        for (auto &&sub_comp : self->repeater_{id}.inner->data) \
                           cells.push_back((*sub_comp.ptr)->box_layout_data({o}));",
                    id = repeater,
                    o = to_cpp_orientation(orientation),
                );
            }
        }
    }

    let ri = repeated_indices.as_ref().map_or(String::new(), |ri| {
        format!("std::array<unsigned int, {}> {};", 2 * repeater_idx, ri)
    });
    ri + &push_code + &compile_expression(sub_expression, ctx)
}

/*

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
                            new_struct_with_bindings(cpp_type, &element.bindings, component)
                        })
                        .unwrap_or_default();
                    format!(
                        "sixtyfps::private_api::PathElement::{}({})",
                        element.element_type.native_class.class_name, element_initializer
                    )
                })
                .collect();
            format!(
                r#"[&](){{
                sixtyfps::private_api::PathElement elements[{}] = {{
                    {}
                }};
                return sixtyfps::private_api::PathData(&elements[0], std::size(elements));
            }}()"#,
                converted_elements.len(),
                converted_elements.join(",")
            )
        }
        crate::expression_tree::Path::Events(events, points) => {
            let converted_events =
                events.iter().map(|event| compile_expression(event, component)).collect::<Vec<_>>();

            let converted_coordinates =
                points.into_iter().map(|pt| compile_expression(pt, component)).collect::<Vec<_>>();

            format!(
                r#"[&](){{
                sixtyfps::private_api::PathEvent events[{}] = {{
                    {}
                }};
                sixtyfps::private_api::Point coordinates[{}] = {{
                    {}
                }};
                return sixtyfps::private_api::PathData(&events[0], std::size(events), &coordinates[0], std::size(coordinates));
            }}()"#,
                converted_events.len(),
                converted_events.join(","),
                converted_coordinates.len(),
                converted_coordinates.join(",")
            )
        }
    }
}
*/

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
                "[&]{{ try {{ {}; }} catch(const sixtyfps::private_api::ReturnWrapper<void> &w) {{ }} }}()",
                compile_expression(expr, ctx)
            )
        } else {
            let cpp_ty = ty.cpp_type().unwrap_or_default();
            format!(
                "[&]() -> {} {{ try {{ {}; }} catch(const sixtyfps::private_api::ReturnWrapper<{}> &w) {{ return w.value; }} }}()",
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
