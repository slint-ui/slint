// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::fmt::{Display, Result, Write};

use itertools::Itertools;

use crate::expression_tree::MinMaxOp;

use super::{
    CompilationUnit, EvaluationContext, Expression, ParentCtx, PropertyReference, SubComponent,
};

pub fn pretty_print(root: &CompilationUnit, writer: &mut dyn Write) -> Result {
    PrettyPrinter { writer, indentation: 0 }.print_root(root)
}

pub fn pretty_print_component(
    root: &CompilationUnit,
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
    fn print_root(&mut self, root: &CompilationUnit) -> Result {
        for g in &root.globals {
            if !g.is_builtin {
                self.print_global(root, g)?;
            }
        }
        for c in &root.sub_components {
            self.print_component(root, c, None)?
        }
        for p in &root.public_components {
            self.print_component(root, &p.item_tree.root, None)?
        }
        Ok(())
    }

    fn print_component(
        &mut self,
        root: &CompilationUnit,
        sc: &SubComponent,
        parent: Option<ParentCtx<'_>>,
    ) -> Result {
        let ctx = EvaluationContext::new_sub_component(root, sc, (), parent);
        writeln!(self.writer, "component {} {{", sc.name)?;
        self.indentation += 1;
        for p in &sc.properties {
            self.indent()?;
            writeln!(self.writer, "property <{}> {}; //use={}", p.ty, p.name, p.use_count.get())?;
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
        for (p1, p2) in &sc.two_way_bindings {
            self.indent()?;
            writeln!(
                self.writer,
                "{} <=> {};",
                DisplayPropertyRef(p1, &ctx),
                DisplayPropertyRef(p2, &ctx)
            )?
        }
        for (p, init) in &sc.property_init {
            self.indent()?;
            writeln!(
                self.writer,
                "{}: {};{}",
                DisplayPropertyRef(p, &ctx),
                DisplayExpression(&init.expression.borrow(), &ctx),
                if init.is_constant { " /*const*/" } else { "" }
            )?
        }
        for (p, e) in &sc.change_callbacks {
            self.indent()?;
            writeln!(
                self.writer,
                "changed {} => {};",
                DisplayPropertyRef(p, &ctx),
                DisplayExpression(&e.borrow(), &ctx),
            )?
        }
        for ssc in &sc.sub_components {
            self.indent()?;
            writeln!(self.writer, "{} := {} {{}};", ssc.name, ssc.ty.name)?;
        }
        for (item, geom) in std::iter::zip(&sc.items, &sc.geometries) {
            self.indent()?;
            let geometry = geom.as_ref().map_or(String::new(), |geom| {
                format!("geometry: {}", DisplayExpression(&geom.borrow(), &ctx))
            });
            writeln!(self.writer, "{} := {} {{ {geometry} }};", item.name, item.ty.class_name)?;
        }
        for (idx, r) in sc.repeated.iter().enumerate() {
            self.indent()?;
            write!(self.writer, "for in {} : ", DisplayExpression(&r.model.borrow(), &ctx))?;
            self.print_component(
                root,
                &r.sub_tree.root,
                Some(ParentCtx::new(&ctx, Some(idx as u32))),
            )?
        }
        for w in &sc.popup_windows {
            self.indent()?;
            self.print_component(root, &w.item_tree.root, Some(ParentCtx::new(&ctx, None)))?
        }
        self.indentation -= 1;
        self.indent()?;
        writeln!(self.writer, "}}")
    }

    fn print_global(&mut self, root: &CompilationUnit, global: &super::GlobalComponent) -> Result {
        let ctx = EvaluationContext::new_global(root, global, ());
        if global.exported {
            write!(self.writer, "export ")?;
        }
        let aliases = global.aliases.join(",");
        let aliases = if aliases.is_empty() { String::new() } else { format!(" /*{aliases}*/") };
        writeln!(self.writer, "global {} {{{aliases}", global.name)?;
        self.indentation += 1;
        for ((p, init), is_const) in
            std::iter::zip(&global.properties, &global.init_values).zip(&global.const_properties)
        {
            self.indent()?;
            let init = init.as_ref().map_or(String::new(), |init| {
                format!(
                    ": {}{}",
                    DisplayExpression(&init.expression.borrow(), &ctx,),
                    if init.is_constant { "/*const*/" } else { "" }
                )
            });
            writeln!(
                self.writer,
                "property <{}> {}{init}; //use={}{}",
                p.ty,
                p.name,
                p.use_count.get(),
                if *is_const { "  const" } else { "" }
            )?;
        }
        for (p, e) in &global.change_callbacks {
            self.indent()?;
            writeln!(
                self.writer,
                "changed {} => {};",
                global.properties[*p].name,
                DisplayExpression(&e.borrow(), &ctx),
            )?
        }
        for f in &global.functions {
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
                let i = &sc.items[*item_index as usize];
                write!(f, "{}.{}", i.name, prop_name)
            }
            PropertyReference::InParent { level, parent_reference } => {
                for _ in 0..level.get() {
                    ctx = ctx.parent.unwrap().ctx;
                }
                write!(f, "{}", Self(parent_reference, ctx))
            }
            PropertyReference::Global { global_index, property_index } => {
                let g = &ctx.compilation_unit.globals[*global_index];
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
                let g = &ctx.compilation_unit.globals[*global_index];
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
            Expression::ImageReference { resource_ref, nine_slice } => {
                write!(f, "{:?}", resource_ref)?;
                if let Some(nine_slice) = &nine_slice {
                    write!(f, "nine-slice({:?})", nine_slice)?;
                }
                Ok(())
            }
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
            Expression::MinMax { ty: _, op, lhs, rhs } => match op {
                MinMaxOp::Min => write!(f, "min({}, {})", e(lhs), e(rhs)),
                MinMaxOp::Max => write!(f, "max({}, {})", e(lhs), e(rhs)),
            },
            Expression::EmptyComponentFactory => write!(f, "<empty-component-factory>",),
            Expression::TranslationReference { format_args, string_index, plural } => {
                match plural {
                    Some(plural) => write!(
                        f,
                        "@tr({:?} % {}, {})",
                        string_index,
                        DisplayExpression(plural, ctx),
                        DisplayExpression(format_args, ctx)
                    ),
                    None => write!(
                        f,
                        "@tr({:?}, {})",
                        string_index,
                        DisplayExpression(format_args, ctx)
                    ),
                }
            }
        }
    }
}
