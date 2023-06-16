// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

use std::fmt::{Display, Result, Write};

use itertools::Itertools;

use super::{
    EvaluationContext, Expression, ParentCtx, PropertyReference, PublicComponent, SubComponent,
};

pub fn pretty_print(root: &PublicComponent, writer: &mut dyn Write) -> Result {
    PrettyPrinter { writer, indentation: 0 }.print_root(root)
}

pub fn pretty_print_component(
    root: &PublicComponent,
    component: &SubComponent,
    writer: &mut dyn Write,
) -> Result {
    PrettyPrinter { writer, indentation: 0 }.print_component(root, component, None)
}

struct PrettyPrinter<'a> {
    writer: &'a mut dyn Write,
    indentation: usize,
}

impl<'a> PrettyPrinter<'a> {
    fn print_root(&mut self, root: &PublicComponent) -> Result {
        for c in &root.sub_components {
            self.print_component(root, c, None)?
        }
        self.print_component(root, &root.item_tree.root, None)
    }

    fn print_component(
        &mut self,
        root: &PublicComponent,
        sc: &SubComponent,
        parent: Option<ParentCtx<'_>>,
    ) -> Result {
        let ctx = EvaluationContext::new_sub_component(root, sc, (), parent);
        writeln!(self.writer, "component {} {{", sc.name)?;
        self.indentation += 1;
        for p in &sc.properties {
            self.indent()?;
            writeln!(self.writer, "property <{}> {}; //{}", p.ty, p.name, p.use_count.get())?;
        }
        for f in &sc.functions {
            self.indent()?;
            writeln!(
                self.writer,
                "function {} ({}) -> {} {{ {} }}; ",
                f.name,
                f.args.iter().map(ToString::to_string).join(", "),
                f.ret_ty,
                DisplayExpression(&f.code, &ctx)
            )?;
        }
        for (p, init) in &sc.property_init {
            self.indent()?;
            writeln!(
                self.writer,
                "{}: {};",
                DisplayPropertyRef(p, &ctx),
                DisplayExpression(&init.expression.borrow(), &ctx)
            )?
        }
        for ssc in &sc.sub_components {
            self.indent()?;
            writeln!(self.writer, "{} := {} {{}};", ssc.name, ssc.ty.name)?;
        }
        for i in &sc.items {
            self.indent()?;
            writeln!(self.writer, "{} := {} {{}};", i.name, i.ty.class_name)?;
        }
        for (idx, r) in sc.repeated.iter().enumerate() {
            self.indent()?;
            write!(self.writer, "for in {} : ", DisplayExpression(&r.model.borrow(), &ctx))?;
            self.print_component(root, &r.sub_tree.root, Some(ParentCtx::new(&ctx, Some(idx))))?
        }
        for w in &sc.popup_windows {
            self.indent()?;
            self.print_component(root, &w.root, Some(ParentCtx::new(&ctx, None)))?
        }
        self.indentation -= 1;
        self.indent()?;
        writeln!(self.writer, "}}")
    }

    fn indent(&mut self) -> Result {
        for _ in 0..self.indentation {
            self.writer.write_str("    ")?;
        }
        Ok(())
    }
}

pub struct DisplayPropertyRef<'a, T>(pub &'a PropertyReference, pub &'a EvaluationContext<'a, T>);
impl<T> Display for DisplayPropertyRef<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result {
        let mut ctx = self.1;
        match &self.0 {
            PropertyReference::Local { sub_component_path, property_index } => {
                if let Some(g) = ctx.current_global {
                    write!(f, "{}.{}", g.name, g.properties[*property_index].name)
                } else {
                    let mut sc = ctx.current_sub_component.unwrap();
                    for i in sub_component_path {
                        write!(f, "{}.", sc.sub_components[*i].name)?;
                        sc = &sc.sub_components[*i].ty;
                    }
                    write!(f, "{}", sc.properties[*property_index].name)
                }
            }
            PropertyReference::InNativeItem { sub_component_path, item_index, prop_name } => {
                let mut sc = ctx.current_sub_component.unwrap();
                for i in sub_component_path {
                    write!(f, "{}.", sc.sub_components[*i].name)?;
                    sc = &sc.sub_components[*i].ty;
                }
                let i = &sc.items[*item_index];
                write!(f, "{}.{}", i.name, prop_name)
            }
            PropertyReference::InParent { level, parent_reference } => {
                for _ in 0..level.get() {
                    ctx = ctx.parent.unwrap().ctx;
                }
                write!(f, "{}", Self(parent_reference, ctx))
            }
            PropertyReference::Global { global_index, property_index } => {
                let g = &ctx.public_component.globals[*global_index];
                write!(f, "{}.{}", g.name, g.properties[*property_index].name)
            }
            PropertyReference::Function { sub_component_path, function_index } => {
                if let Some(g) = ctx.current_global {
                    write!(f, "{}.{}", g.name, g.functions[*function_index].name)
                } else {
                    let mut sc = ctx.current_sub_component.unwrap();
                    for i in sub_component_path {
                        write!(f, "{}.", sc.sub_components[*i].name)?;
                        sc = &sc.sub_components[*i].ty;
                    }
                    write!(f, "{}", sc.functions[*function_index].name)
                }
            }
            PropertyReference::GlobalFunction { global_index, function_index } => {
                let g = &ctx.public_component.globals[*global_index];
                write!(f, "{}.{}", g.name, g.functions[*function_index].name)
            }
        }
    }
}

pub struct DisplayExpression<'a, T>(pub &'a Expression, pub &'a EvaluationContext<'a, T>);
impl<'a, T> Display for DisplayExpression<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result {
        let ctx = self.1;
        let e = |e: &'a Expression| DisplayExpression(e, ctx);
        match self.0 {
            Expression::StringLiteral(x) => write!(f, "{:?}", x),
            Expression::NumberLiteral(x) => write!(f, "{:?}", x),
            Expression::BoolLiteral(x) => write!(f, "{:?}", x),
            Expression::PropertyReference(x) => write!(f, "{}", DisplayPropertyRef(x, ctx)),
            Expression::FunctionParameterReference { index } => write!(f, "arg_{}", index),
            Expression::StoreLocalVariable { name, value } => {
                write!(f, "{} = {}", name, e(value))
            }
            Expression::ReadLocalVariable { name, .. } => write!(f, "{}", name),
            Expression::StructFieldAccess { base, name } => write!(f, "{}.{}", e(base), name),
            Expression::ArrayIndex { array, index } => write!(f, "{}[{}]", e(array), e(index)),
            Expression::Cast { from, to } => write!(f, "{} /*as {:?}*/", e(from), to),
            Expression::CodeBlock(v) => {
                write!(f, "{{ {} }}", v.iter().map(e).join("; "))
            }
            Expression::BuiltinFunctionCall { function, arguments } => {
                write!(f, "{:?}({})", function, arguments.iter().map(e).join(", "))
            }
            Expression::CallBackCall { callback, arguments } => {
                write!(
                    f,
                    "{}({})",
                    DisplayPropertyRef(callback, ctx),
                    arguments.iter().map(e).join(", ")
                )
            }
            Expression::FunctionCall { function, arguments } => {
                write!(
                    f,
                    "{}({})",
                    DisplayPropertyRef(function, ctx),
                    arguments.iter().map(e).join(", ")
                )
            }
            Expression::ExtraBuiltinFunctionCall { function, arguments, .. } => {
                write!(f, "{}({})", function, arguments.iter().map(e).join(", "))
            }
            Expression::PropertyAssignment { property, value } => {
                write!(f, "{} = {}", DisplayPropertyRef(property, ctx), e(value))
            }
            Expression::ModelDataAssignment { level, value } => {
                write!(f, "data_{} = {}", level, e(value))
            }
            Expression::ArrayIndexAssignment { array, index, value } => {
                write!(f, "{}[{}] = {}", e(array), e(index), e(value))
            }
            Expression::BinaryExpression { lhs, rhs, op } => {
                write!(f, "({} {} {})", e(lhs), op, e(rhs))
            }
            Expression::UnaryOp { sub, op } => write!(f, "{}{}", op, e(sub)),
            Expression::ImageReference { resource_ref } => write!(f, "{:?}", resource_ref),
            Expression::Condition { condition, true_expr, false_expr } => {
                write!(f, "({} ? {} : {})", e(condition), e(true_expr), e(false_expr))
            }
            Expression::Array { values, .. } => {
                write!(f, "[{}]", values.iter().map(e).join(", "))
            }
            Expression::Struct { values, .. } => write!(
                f,
                "{{ {} }}",
                values.iter().map(|(k, v)| format!("{}: {}", k, e(v))).join(", ")
            ),
            Expression::EasingCurve(x) => write!(f, "{:?}", x),
            Expression::LinearGradient { angle, stops } => write!(
                f,
                "@linear-gradient({}, {})",
                e(angle),
                stops.iter().map(|(e1, e2)| format!("{} {}", e(e1), e(e2))).join(", ")
            ),
            Expression::RadialGradient { stops } => write!(
                f,
                "@radial-gradient(circle, {})",
                stops.iter().map(|(e1, e2)| format!("{} {}", e(e1), e(e2))).join(", ")
            ),
            Expression::EnumerationValue(x) => write!(f, "{}", x),
            Expression::ReturnStatement(Some(x)) => write!(f, "return {}", e(x)),
            Expression::ReturnStatement(None) => f.write_str("return"),
            Expression::LayoutCacheAccess { layout_cache_prop, index, repeater_index: None } => {
                write!(f, "{}[{}]", DisplayPropertyRef(layout_cache_prop, ctx), index)
            }
            Expression::LayoutCacheAccess {
                layout_cache_prop,
                index,
                repeater_index: Some(ri),
            } => {
                write!(f, "{}[{} % {}]", DisplayPropertyRef(layout_cache_prop, ctx), index, e(ri))
            }
            Expression::BoxLayoutFunction { .. } => write!(f, "BoxLayoutFunction(TODO)",),
            Expression::ComputeDialogLayoutCells { .. } => {
                write!(f, "ComputeDialogLayoutCells(TODO)",)
            }
        }
    }
}
