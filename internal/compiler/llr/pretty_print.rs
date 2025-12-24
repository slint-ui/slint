// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::fmt::{Display, Result, Write};

use itertools::Itertools;

use crate::expression_tree::MinMaxOp;

use super::{
    CompilationUnit, EvaluationContext, Expression, LocalMemberIndex, LocalMemberReference,
    MemberReference, ParentScope, SubComponentIdx,
};

pub fn pretty_print(root: &CompilationUnit, writer: &mut dyn Write) -> Result {
    PrettyPrinter { writer, indentation: 0 }.print_root(root)
}

struct PrettyPrinter<'a> {
    writer: &'a mut dyn Write,
    indentation: usize,
}

impl PrettyPrinter<'_> {
    fn print_root(&mut self, root: &CompilationUnit) -> Result {
        for (idx, g) in root.globals.iter_enumerated() {
            if !g.is_builtin {
                self.print_global(root, idx, g)?;
            }
        }
        for c in root.sub_components.keys() {
            self.print_component(root, c, None)?
        }

        Ok(())
    }

    fn print_component(
        &mut self,
        root: &CompilationUnit,
        sc_idx: SubComponentIdx,
        parent: Option<&ParentScope<'_>>,
    ) -> Result {
        let ctx = EvaluationContext::new_sub_component(root, sc_idx, (), parent);
        let sc = &root.sub_components[sc_idx];
        writeln!(self.writer, "component {} {{", sc.name)?;
        self.indentation += 1;
        for p in &sc.properties {
            self.indent()?;
            writeln!(self.writer, "property <{}> {}; //use={}", p.ty, p.name, p.use_count.get())?;
        }
        for c in &sc.callbacks {
            self.indent()?;
            writeln!(
                self.writer,
                "callback {} ({}) -> {};",
                c.name,
                c.args.iter().map(ToString::to_string).join(", "),
                c.ret_ty,
            )?;
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
        for (p1, p2, fields) in &sc.two_way_bindings {
            self.indent()?;
            writeln!(
                self.writer,
                "{} <=> {}{}{};",
                DisplayPropertyRef(p1, &ctx),
                DisplayPropertyRef(p2, &ctx),
                if fields.is_empty() { "" } else { "." },
                fields.join(".")
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
            writeln!(self.writer, "{} := {} {{}};", ssc.name, root.sub_components[ssc.ty].name)?;
        }
        for (item, geom) in std::iter::zip(&sc.items, &sc.geometries) {
            self.indent()?;
            let geometry = geom.as_ref().map_or(String::new(), |geom| {
                format!("geometry: {}", DisplayExpression(&geom.borrow(), &ctx))
            });
            writeln!(self.writer, "{} := {} {{ {geometry} }};", item.name, item.ty.class_name)?;
        }
        for (idx, r) in sc.repeated.iter_enumerated() {
            self.indent()?;
            write!(self.writer, "for in {} : ", DisplayExpression(&r.model.borrow(), &ctx))?;
            self.print_component(root, r.sub_tree.root, Some(&ParentScope::new(&ctx, Some(idx))))?
        }
        for t in &sc.menu_item_trees {
            self.indent()?;
            self.print_component(root, t.root, Some(&ParentScope::new(&ctx, None)))?
        }
        for w in &sc.popup_windows {
            self.indent()?;
            self.print_component(root, w.item_tree.root, Some(&ParentScope::new(&ctx, None)))?
        }
        self.indentation -= 1;
        self.indent()?;
        writeln!(self.writer, "}}")
    }

    fn print_global(
        &mut self,
        root: &CompilationUnit,
        idx: super::GlobalIdx,
        global: &super::GlobalComponent,
    ) -> Result {
        let ctx = EvaluationContext::new_global(root, idx, ());
        if global.exported {
            write!(self.writer, "export ")?;
        }
        let aliases = global.aliases.join(",");
        let aliases = if aliases.is_empty() { String::new() } else { format!(" /*{aliases}*/") };
        writeln!(self.writer, "global {} {{{aliases}", global.name)?;
        self.indentation += 1;
        for (p, is_const) in std::iter::zip(&global.properties, &global.const_properties) {
            self.indent()?;
            writeln!(
                self.writer,
                "property <{}> {}; //use={}{}",
                p.ty,
                p.name,
                p.use_count.get(),
                if *is_const { "  const" } else { "" }
            )?;
        }
        for c in &global.callbacks {
            self.indent()?;
            writeln!(
                self.writer,
                "callback {} ({}) -> {};",
                c.name,
                c.args.iter().map(ToString::to_string).join(", "),
                c.ret_ty,
            )?;
        }
        for (p, init) in &global.init_values {
            self.indent()?;
            match p {
                LocalMemberIndex::Property(p) => {
                    writeln!(
                        self.writer,
                        "{}: {}{};",
                        global.properties[*p].name,
                        DisplayExpression(&init.expression.borrow(), &ctx,),
                        if init.is_constant { "/*const*/" } else { "" }
                    )?;
                }
                LocalMemberIndex::Callback(c) => {
                    writeln!(
                        self.writer,
                        "{} => {};",
                        global.callbacks[*c].name,
                        DisplayExpression(&init.expression.borrow(), &ctx,),
                    )?;
                }
                _ => unreachable!(),
            }
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

pub struct DisplayPropertyRef<'a, T>(pub &'a MemberReference, pub &'a EvaluationContext<'a, T>);
impl<T> Display for DisplayPropertyRef<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result {
        let ctx = self.1;
        match &self.0 {
            MemberReference::Relative { parent_level, local_reference } => {
                print_local_ref(f, ctx, local_reference, *parent_level)
            }
            MemberReference::Global { global_index, member } => {
                let g = &ctx.compilation_unit.globals[*global_index];
                match member {
                    LocalMemberIndex::Property(property_index) => {
                        write!(f, "{}.{}", g.name, g.properties[*property_index].name)
                    }
                    LocalMemberIndex::Function(function_index) => {
                        write!(f, "{}.{}", g.name, g.functions[*function_index].name)
                    }
                    _ => write!(f, "<invalid reference in global>"),
                }
            }
        }
    }
}

pub struct DisplayLocalRef<'a, T>(pub &'a LocalMemberReference, pub &'a EvaluationContext<'a, T>);
impl<T> Display for DisplayLocalRef<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result {
        print_local_ref(f, self.1, self.0, 0)
    }
}

fn print_local_ref<T>(
    f: &mut std::fmt::Formatter<'_>,
    ctx: &EvaluationContext<T>,
    local_ref: &LocalMemberReference,
    parent_level: usize,
) -> Result {
    if let Some(g) = ctx.current_global() {
        match &local_ref.reference {
            LocalMemberIndex::Property(property_index) => {
                write!(f, "{}.{}", g.name, g.properties[*property_index].name)
            }
            LocalMemberIndex::Function(function_index) => {
                write!(f, "{}.{}", g.name, g.functions[*function_index].name)
            }
            _ => write!(f, "<invalid reference in global>"),
        }
    } else {
        let Some(s) = ctx.parent_sub_component_idx(parent_level) else {
            return write!(f, "<invalid parent reference>");
        };
        let mut sc = &ctx.compilation_unit.sub_components[s];

        for i in &local_ref.sub_component_path {
            write!(f, "{}.", sc.sub_components[*i].name)?;
            sc = &ctx.compilation_unit.sub_components[sc.sub_components[*i].ty];
        }
        match &local_ref.reference {
            LocalMemberIndex::Property(property_index) => {
                write!(f, "{}", sc.properties[*property_index].name)
            }
            LocalMemberIndex::Callback(callback_index) => {
                write!(f, "{}", sc.callbacks[*callback_index].name)
            }
            LocalMemberIndex::Function(function_index) => {
                write!(f, "{}", sc.functions[*function_index].name)
            }
            LocalMemberIndex::Native { item_index, prop_name } => {
                let i = &sc.items[*item_index];
                write!(f, "{}.{}", i.name, prop_name)
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
            Expression::StringLiteral(x) => write!(f, "{x:?}"),
            Expression::NumberLiteral(x) => write!(f, "{x:?}"),
            Expression::BoolLiteral(x) => write!(f, "{x:?}"),
            Expression::PropertyReference(x) => write!(f, "{}", DisplayPropertyRef(x, ctx)),
            Expression::FunctionParameterReference { index } => write!(f, "arg_{index}"),
            Expression::StoreLocalVariable { name, value } => {
                write!(f, "{} = {}", name, e(value))
            }
            Expression::ReadLocalVariable { name, .. } => write!(f, "{name}"),
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
            Expression::ItemMemberFunctionCall { function } => {
                write!(f, "{}()", DisplayPropertyRef(function, ctx))
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
                write!(f, "{resource_ref:?}")?;
                if let Some(nine_slice) = &nine_slice {
                    write!(f, "nine-slice({nine_slice:?})")?;
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
            Expression::EasingCurve(x) => write!(f, "{x:?}"),
            Expression::MouseCursor(x) => write!(f, "{x:?}"),
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
            Expression::ConicGradient { from_angle, stops } => write!(
                f,
                "@conic-gradient(from {}, {})",
                e(from_angle),
                stops.iter().map(|(e1, e2)| format!("{} {}", e(e1), e(e2))).join(", ")
            ),
            Expression::EnumerationValue(x) => write!(f, "{x}"),
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
