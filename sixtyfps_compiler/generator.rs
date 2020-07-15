/*!
The module responsible for the code generation.

There is one sub module for every language
*/

use crate::diagnostics::Diagnostics;
use crate::{
    expression_tree::{Expression, NamedReference},
    object_tree::{Component, ElementRc},
    typeregister::Type,
};
use std::rc::Rc;

#[cfg(feature = "cpp")]
mod cpp;

#[cfg(feature = "rust")]
pub mod rust;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum OutputFormat {
    #[cfg(feature = "cpp")]
    Cpp,
    #[cfg(feature = "rust")]
    Rust,
}

impl OutputFormat {
    pub fn guess_from_extension(path: &std::path::Path) -> Option<Self> {
        match path.extension().and_then(|ext| ext.to_str()) {
            #[cfg(feature = "cpp")]
            Some("cpp") | Some("cxx") | Some("h") | Some("hpp") => Some(Self::Cpp),
            #[cfg(feature = "rust")]
            Some("rs") => Some(Self::Rust),
            _ => None,
        }
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            #[cfg(feature = "cpp")]
            "cpp" => Ok(Self::Cpp),
            #[cfg(feature = "rust")]
            "rust" => Ok(Self::Rust),
            _ => Err(format!("Unknown outpout format {}", s)),
        }
    }
}

pub fn generate(
    format: OutputFormat,
    destination: &mut impl std::io::Write,
    component: &Rc<Component>,
    diag: &mut Diagnostics,
) -> std::io::Result<()> {
    #![allow(unused_variables)]
    #![allow(unreachable_code)]
    match format {
        #[cfg(feature = "cpp")]
        OutputFormat::Cpp => {
            if let Some(output) = cpp::generate(component, diag) {
                write!(destination, "{}", output)?;
            }
        }
        #[cfg(feature = "rust")]
        OutputFormat::Rust => {
            if let Some(output) = rust::generate(component, diag) {
                write!(destination, "{}", output)?;
            }
        }
    }
    Ok(())
}

/// Visit each item in order in which they should appear in the children tree array.
/// The parameter of the visitor are the item, and the first_children_offset
#[allow(dead_code)]
pub fn build_array_helper(component: &Component, mut visit_item: impl FnMut(&ElementRc, u32)) {
    visit_item(&component.root_element, 1);
    visit_children(&component.root_element, 1, &mut visit_item);

    fn sub_children_count(e: &ElementRc) -> usize {
        let mut count = e.borrow().children.len();
        for i in &e.borrow().children {
            count += sub_children_count(i);
        }
        count
    }

    fn visit_children(
        item: &ElementRc,
        children_offset: u32,
        visit_item: &mut impl FnMut(&ElementRc, u32),
    ) {
        let mut offset = children_offset + item.borrow().children.len() as u32;
        for i in &item.borrow().children {
            visit_item(i, offset);
            offset += sub_children_count(i) as u32;
        }

        let mut offset = children_offset + item.borrow().children.len() as u32;
        for e in &item.borrow().children {
            visit_children(e, offset, visit_item);
            offset += sub_children_count(e) as u32;
        }
    }
}

trait ExpressionCompiler {
    type CompiledExpression;
    type CompiledComponentAccess;

    fn string_literal(string: &str) -> Self::CompiledExpression;
    fn number_literal(value: f64) -> Self::CompiledExpression;
    fn bool_literal(value: bool) -> Self::CompiledExpression;
    fn resource_reference(path: &str) -> Self::CompiledExpression;

    fn cast_number_to_string(number: Self::CompiledExpression) -> Self::CompiledExpression;
    fn cast_number_to_color(number: Self::CompiledExpression) -> Self::CompiledExpression;
    fn cast_expr_to_model(ty: Type, expr: Self::CompiledExpression) -> Self::CompiledExpression;

    fn current_component(component: &Rc<Component>) -> Self::CompiledComponentAccess;
    fn parent_component(component: Self::CompiledComponentAccess) -> Self::CompiledComponentAccess;

    fn read_property(
        component: Self::CompiledComponentAccess,
        elem: ElementRc,
        name: &str,
    ) -> Self::CompiledExpression;
    fn write_property(
        component: Self::CompiledComponentAccess,
        elem: ElementRc,
        name: &str,
        value: Self::CompiledExpression,
    ) -> Self::CompiledExpression;
    fn read_extra_component_property(
        component: Self::CompiledComponentAccess,
        name: &str,
    ) -> Self::CompiledExpression;
    fn emit_signal(
        component: Self::CompiledComponentAccess,
        elem: ElementRc,
        name: &str,
    ) -> Self::CompiledExpression;

    fn object_access(
        base: Self::CompiledExpression,
        base_expr: &Expression,
        name: &str,
    ) -> Self::CompiledExpression;

    fn block(iter: impl Iterator<Item = Self::CompiledExpression>) -> Self::CompiledExpression;

    fn binary_op(
        op: char,
        lhs: Self::CompiledExpression,
        rhs: Self::CompiledExpression,
    ) -> Self::CompiledExpression;
    fn unary_op(op: char, expr: Self::CompiledExpression) -> Self::CompiledExpression;

    fn conditional(
        _if: Self::CompiledExpression,
        then: Self::CompiledExpression,
        _else: Self::CompiledExpression,
    ) -> Self::CompiledExpression;
}

fn enclosing_component_for_element<T: ExpressionCompiler>(
    element: &ElementRc,
    component: &Rc<Component>,
    compiled_component: T::CompiledComponentAccess,
) -> T::CompiledComponentAccess {
    let e = element.borrow();
    let enclosing_component = e.enclosing_component.upgrade().unwrap();
    if Rc::ptr_eq(component, &enclosing_component) {
        compiled_component
    } else {
        enclosing_component_for_element::<T>(
            element,
            &component
                .parent_element
                .upgrade()
                .unwrap()
                .borrow()
                .enclosing_component
                .upgrade()
                .unwrap(),
            T::parent_component(compiled_component),
        )
    }
}

pub fn compile_expression<T: ExpressionCompiler>(
    e: &Expression,
    component: &Rc<Component>,
) -> T::CompiledExpression {
    match e {
        Expression::StringLiteral(s) => T::string_literal(s.as_ref()),
        Expression::NumberLiteral(n, unit) => T::number_literal(unit.normalize(*n)),
        Expression::BoolLiteral(b) => T::bool_literal(*b),
        Expression::Cast { from, to } => {
            let f = compile_expression::<T>(&*from, &component);
            match (from.ty(), to) {
                (Type::Float32, Type::String) | (Type::Int32, Type::String) => {
                    T::cast_number_to_string(f)
                }
                (m_ty, Type::Model) => T::cast_expr_to_model(m_ty, f),
                (Type::Float32, Type::Color) => T::cast_number_to_color(f),
                _ => f,
            }
        }
        Expression::PropertyReference(NamedReference { element, name }) => {
            let element = element.upgrade().unwrap();
            T::read_property(
                enclosing_component_for_element(
                    &element,
                    component,
                    T::current_component(component),
                ),
                element,
                name.as_str(),
            )
        }
        Expression::RepeaterIndexReference { element } => {
            if element.upgrade().unwrap().borrow().base_type == Type::Component(component.clone()) {
                T::read_extra_component_property(T::current_component(component), "index")
            } else {
                todo!();
            }
        }
        Expression::RepeaterModelReference { element } => {
            if element.upgrade().unwrap().borrow().base_type == Type::Component(component.clone()) {
                T::read_extra_component_property(T::current_component(component), "model_data")
            } else {
                todo!();
            }
        }
        Expression::ObjectAccess { base, name } => {
            let base_e = compile_expression::<T>(base, component);
            T::object_access(base_e, &*base, name.as_ref())
        }
        Expression::CodeBlock(sub) => {
            T::block(sub.iter().map(|e| compile_expression::<T>(e, &component)))
        }
        Expression::SignalReference(_) => panic!("Signal in a expression?"),
        Expression::FunctionCall { function } => {
            if let Expression::SignalReference(NamedReference { element, name, .. }) = &**function {
                let element = element.upgrade().unwrap();
                T::emit_signal(
                    enclosing_component_for_element(
                        &element,
                        component,
                        T::current_component(component),
                    ),
                    element,
                    name.as_str(),
                )
            } else {
                panic!("the function {:?} is not a signal", e);
            }
        }
        Expression::SelfAssignment { lhs, rhs, op } => match &**lhs {
            Expression::PropertyReference(NamedReference { element, name }) => {
                let element = element.upgrade().unwrap();
                let c = enclosing_component_for_element(
                    &element,
                    component,
                    T::current_component(component),
                );

                let value = T::binary_op(
                    *op,
                    T::read_property(c, element, name.as_ref()),
                    compile_expression::<T>(&*rhs, &component),
                );
                T::write_property(c, element, name.as_ref(), value)
            }
            _ => panic!("typechecking should make sure this was a PropertyReference"),
        },
        Expression::BinaryExpression { lhs, rhs, op } => T::binary_op(
            *op,
            compile_expression::<T>(&*rhs, &component),
            compile_expression::<T>(&*rhs, &component),
        ),
        Expression::UnaryOp { sub, op } => {
            T::unary_op(*op, compile_expression::<T>(&*sub, &component))
        }
        Expression::ResourceReference { absolute_source_path } => {
            T::resource_reference(absolute_source_path.as_ref())
        }
        Expression::Condition { condition, true_expr, false_expr } => {
            let condition_code = compile_expression::<T>(&*condition, component);
            let true_code = compile_expression::<T>(&*true_expr, component);
            let false_code = compile_expression::<T>(&*false_expr, component);
            T::conditional(condition_code, true_code, false_code)
        }
        Expression::Invalid | Expression::Uncompiled(_) => {
            let error = format!("unsupported expression {:?}", e);
            quote!(compile_error! {#error})
        }
        Expression::Array { values, .. } => {
            //let rust_element_ty = rust_type(&element_ty, &Default::default());
            let val = values.iter().map(|e| compile_expression(e, component));
            quote!([#(#val as _),*])
        }
        Expression::Object { ty, values } => {
            if let Type::Object(ty) = ty {
                let elem = ty.iter().map(|(k, t)| {
                    values.get(k).map(|e| {
                        let ce = compile_expression(e, component);
                        let t = rust_type(t, &Default::default()).unwrap_or_default();
                        quote!(#ce as #t)
                    })
                });
                // This will produce a tuple
                quote!((#(#elem,)*))
            } else {
                panic!("Expression::Object is not a Type::Object")
            }
        }
        Expression::PathElements { elements } => compile_path(elements, component),
    }
}
