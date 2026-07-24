// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::fmt::{Display, Result, Write};

use itertools::{Either, Itertools};

use crate::expression_tree::MinMaxOp;
use crate::langtype::{StructName, Type};
use crate::layout::Orientation;

use super::{
    Animation, CompilationUnit, EvaluationContext, Expression, LocalMemberIndex,
    LocalMemberReference, MemberReference, ParentScope, SubComponentIdx,
};

pub fn pretty_print(root: &CompilationUnit, writer: &mut dyn Write) -> Result {
    PrettyPrinter { writer, indentation: 0 }.print_root(root)
}

/// Print compiler-internal builtin structs by their name; they have no slint
/// name, so `Type`'s Display spells out all their fields.
struct DisplayType<'a>(&'a Type);
impl Display for DisplayType<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result {
        match self.0 {
            Type::Struct(s) => match &s.name {
                StructName::Builtin(b) if b.slint_name().is_none() => {
                    write!(f, "{}", <&str>::from(b))
                }
                _ => write!(f, "{}", self.0),
            },
            _ => write!(f, "{}", self.0),
        }
    }
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
        // Repeater, popup, and menu trees print inline under their parent,
        // because their expressions resolve in the parent scope.
        for c in &root.used_sub_components {
            self.print_component(root, *c, None)?
        }
        for p in &root.public_components {
            self.print_component(root, p.item_tree.root, None)?
        }
        if let Some(p) = &root.popup_menu {
            self.print_component(root, p.item_tree.root, None)?
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
            writeln!(
                self.writer,
                "property <{}> {}; //use={}",
                DisplayType(&p.ty),
                p.name,
                p.use_count.get()
            )?;
        }
        for c in &sc.callbacks {
            self.indent()?;
            writeln!(
                self.writer,
                "callback {} ({}) -> {};",
                c.name,
                c.args.iter().map(|t| DisplayType(t).to_string()).join(", "),
                DisplayType(&c.ret_ty),
            )?;
        }
        for f in &sc.functions {
            self.indent()?;
            writeln!(
                self.writer,
                "function {} ({}) -> {} {{ {} }}; ",
                f.name,
                f.args.iter().map(|t| DisplayType(t).to_string()).join(", "),
                DisplayType(&f.ret_ty),
                DisplayExpression(&f.code.borrow(), &ctx)
            )?;
        }
        for twb in &sc.two_way_bindings {
            self.indent()?;
            writeln!(
                self.writer,
                "{} <=> {}{}{};",
                DisplayLocalRef(&twb.prop1, &ctx),
                DisplayPropertyRef(&twb.prop2, &ctx),
                if twb.field_access.is_empty() { "" } else { "." },
                twb.field_access.join(".")
            )?
        }
        for (p, init) in &sc.property_init {
            self.indent()?;
            write!(
                self.writer,
                "{}: {}",
                DisplayPropertyRef(p, &ctx),
                DisplayExpression(&init.expression.borrow(), &ctx)
            )?;
            match &init.animation {
                Some(Animation::Static(a)) => {
                    write!(self.writer, " animate {}", DisplayExpression(a, &ctx))?
                }
                Some(Animation::Transition(a)) => {
                    write!(self.writer, " animate transition {}", DisplayExpression(a, &ctx))?
                }
                None => {}
            }
            writeln!(
                self.writer,
                ";{}",
                if init.kind == super::BindingKind::Constant { " /*const*/" } else { "" }
            )?
        }
        for (p, a) in &sc.animations {
            self.indent()?;
            writeln!(
                self.writer,
                "animate {} {{ {} }};",
                DisplayLocalRef(p, &ctx),
                DisplayExpression(a, &ctx)
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
        for e in &sc.pre_init_code {
            self.indent()?;
            writeln!(self.writer, "pre-init => {};", DisplayExpression(&e.borrow(), &ctx))?
        }
        for e in &sc.init_code {
            self.indent()?;
            writeln!(self.writer, "init => {};", DisplayExpression(&e.borrow(), &ctx))?
        }
        for (name, e) in
            [("layout-info-h", &sc.layout_info_h), ("layout-info-v", &sc.layout_info_v)]
        {
            self.indent()?;
            writeln!(self.writer, "{}: {};", name, DisplayExpression(&e.borrow(), &ctx))?
        }
        if let Some(e) = &sc.grid_layout_input_for_repeated {
            self.indent()?;
            writeln!(
                self.writer,
                "grid-layout-input-for-repeated: {};",
                DisplayExpression(&e.borrow(), &ctx)
            )?
        }
        if let Some(e) = &sc.flexbox_layout_item_info_for_repeated {
            self.indent()?;
            writeln!(
                self.writer,
                "flexbox-layout-item-info-for-repeated: {};",
                DisplayExpression(&e.borrow(), &ctx)
            )?
        }
        for (i, c) in sc.grid_layout_children.iter_enumerated() {
            self.indent()?;
            writeln!(
                self.writer,
                "grid-layout-child[{}] {{ h: {}; v: {} }};",
                usize::from(i),
                DisplayExpression(&c.layout_info_h.borrow(), &ctx),
                DisplayExpression(&c.layout_info_v.borrow(), &ctx)
            )?
        }
        for t in &sc.timers {
            self.indent()?;
            writeln!(
                self.writer,
                "timer {{ interval: {}; running: {}; triggered => {} }};",
                DisplayExpression(&t.interval.borrow(), &ctx),
                DisplayExpression(&t.running.borrow(), &ctx),
                DisplayExpression(&t.triggered.borrow(), &ctx)
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
        for ((item_index, prop), e) in &sc.accessible_prop {
            self.indent()?;
            writeln!(
                self.writer,
                "{}.accessible-{}: {};",
                item_name_in_tree(root, sc, *item_index)
                    .unwrap_or_else(|| format!("@{item_index}")),
                crate::generator::to_kebab_case(prop),
                DisplayExpression(&e.borrow(), &ctx)
            )?
        }
        for (idx, r) in sc.repeated.iter_enumerated() {
            self.indent()?;
            write!(
                self.writer,
                "{} {} : /*@repeater({})*/ ",
                if r.index_prop.is_none() && r.data_prop.is_none() { "if" } else { "for in" },
                DisplayExpression(&r.model.borrow(), &ctx),
                usize::from(idx)
            )?;
            self.print_component(root, r.sub_tree.root, Some(&ParentScope::new(&ctx, Some(idx))))?
        }
        for (i, t) in sc.menu_item_trees.iter().enumerate() {
            self.indent()?;
            write!(self.writer, "menu : /*@menu({i})*/ ")?;
            self.print_component(root, t.root, Some(&ParentScope::new(&ctx, None)))?
        }
        for (i, w) in sc.popup_windows.iter().enumerate() {
            self.indent()?;
            let parent = ParentScope::new(&ctx, None);
            // The position is evaluated in the popup's own scope.
            let popup_ctx =
                EvaluationContext::new_sub_component(root, w.item_tree.root, (), Some(&parent));
            write!(
                self.writer,
                "{} at {} : /*@popup({i})*/ ",
                if w.is_tooltip { "tooltip" } else { "popup" },
                DisplayExpression(&w.position.borrow(), &popup_ctx)
            )?;
            self.print_component(root, w.item_tree.root, Some(&parent))?
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
        let emission = if global.from_library {
            " /*from library*/"
        } else if !global.must_generate() {
            " /*not generated*/"
        } else {
            ""
        };
        writeln!(self.writer, "global {} {{{aliases}{emission}", global.name)?;
        self.indentation += 1;
        for (p, is_const) in std::iter::zip(&global.properties, &global.const_properties) {
            self.indent()?;
            writeln!(
                self.writer,
                "property <{}> {}; //use={}{}",
                DisplayType(&p.ty),
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
                c.args.iter().map(|t| DisplayType(t).to_string()).join(", "),
                DisplayType(&c.ret_ty),
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
                        if init.kind == super::BindingKind::Constant { "/*const*/" } else { "" }
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
        for (p, animation) in &global.animations {
            self.indent()?;
            match p {
                LocalMemberIndex::Property(p) => {
                    writeln!(
                        self.writer,
                        "animate {} {{ {} }}",
                        global.properties[*p].name,
                        DisplayExpression(animation, &ctx),
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
                DisplayExpression(&f.code.borrow(), &ctx)
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

/// Name an item by its tree index, following sub-component instances to
/// their root item (the index of an element that is itself a component).
fn item_name_in_tree(
    root: &CompilationUnit,
    sc: &super::SubComponent,
    tree_index: u32,
) -> Option<String> {
    if let Some(item) = sc.items.iter().find(|i| i.index_in_tree == tree_index) {
        return Some(item.name.to_string());
    }
    let ssc = sc.sub_components.iter().find(|s| s.index_in_tree == tree_index)?;
    Some(format!("{}.{}", ssc.name, item_name_in_tree(root, &root.sub_components[ssc.ty], 0)?))
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
                    LocalMemberIndex::Callback(callback_index) => {
                        write!(f, "{}.{}", g.name, g.callbacks[*callback_index].name)
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
            LocalMemberIndex::Callback(callback_index) => {
                write!(f, "{}.{}", g.name, g.callbacks[*callback_index].name)
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
            LocalMemberIndex::Native { item_index, prop_name, .. } => {
                let i = &sc.items[*item_index];
                write!(f, "{}.{}", i.name, prop_name)
            }
            LocalMemberIndex::Timer(timer_index) => {
                write!(f, "timer#{}", usize::from(*timer_index))
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
            Expression::KeysLiteral(keys) => {
                if keys.is_physical {
                    write!(f, "@physical-keys({keys})",)
                } else {
                    write!(f, "@keys({keys})",)
                }
            }
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
            Expression::SliceIndexAssignment { slice_name, index, value } => {
                write!(f, "{}[{}] = {}", slice_name, index, e(value))
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
            Expression::RadialGradient { center, radius, stops } => {
                let center_str = center
                    .as_ref()
                    .map(|(cx, cy)| format!(" at {} {}", e(cx), e(cy)))
                    .unwrap_or_default();
                let radius_str = radius.as_ref().map(|r| format!(" {}", e(r))).unwrap_or_default();
                write!(
                    f,
                    "@radial-gradient(circle{radius_str}{center_str}, {})",
                    stops.iter().map(|(e1, e2)| format!("{} {}", e(e1), e(e2))).join(", ")
                )
            }
            Expression::ConicGradient { from_angle, center, stops } => {
                let center_str = center
                    .as_ref()
                    .map(|(cx, cy)| format!(" at {} {}", e(cx), e(cy)))
                    .unwrap_or_default();
                write!(
                    f,
                    "@conic-gradient(from {}{center_str}, {})",
                    e(from_angle),
                    stops.iter().map(|(e1, e2)| format!("{} {}", e(e1), e(e2))).join(", ")
                )
            }
            Expression::EnumerationValue(x) => write!(f, "{x}"),
            Expression::LayoutCacheAccess {
                layout_cache_prop,
                index,
                repeater_index: None,
                ..
            } => {
                write!(f, "{}[{}]", DisplayPropertyRef(layout_cache_prop, ctx), index)
            }
            Expression::LayoutCacheAccess {
                layout_cache_prop,
                index,
                repeater_index: Some(ri),
                entries_per_item,
            } => {
                write!(
                    f,
                    "{0}[{0}[{1}] + {2} * {3}]",
                    DisplayPropertyRef(layout_cache_prop, ctx),
                    index,
                    e(ri),
                    entries_per_item
                )
            }
            Expression::GridRepeaterCacheAccess {
                layout_cache_prop,
                index,
                repeater_index,
                stride,
                child_offset,
                inner_repeater_index,
                entries_per_item,
            } => {
                if let Some(inner_idx) = inner_repeater_index {
                    write!(
                        f,
                        "{0}[{0}[{1}] + {2} * {3} + {4} * {5} + {6}]",
                        DisplayPropertyRef(layout_cache_prop, ctx),
                        index,
                        e(repeater_index),
                        e(stride),
                        e(inner_idx),
                        entries_per_item,
                        child_offset
                    )
                } else {
                    write!(
                        f,
                        "{0}[{0}[{1}] + {2} * {3} + {4}]",
                        DisplayPropertyRef(layout_cache_prop, ctx),
                        index,
                        e(repeater_index),
                        e(stride),
                        child_offset
                    )
                }
            }
            Expression::WithLayoutItemInfo {
                cells_variable,
                repeater_indices_var_name,
                repeater_steps_var_name,
                elements,
                orientation,
                sub_expression,
            } => {
                write!(
                    f,
                    "{{ {} = [{}] /*{}*/; ",
                    cells_variable,
                    elements
                        .iter()
                        .map(|x| match x {
                            Either::Left(x) => e(x).to_string(),
                            Either::Right(r) =>
                                format!("@repeater({})", usize::from(r.repeater_index)),
                        })
                        .join(", "),
                    match orientation {
                        Orientation::Horizontal => "horizontal",
                        Orientation::Vertical => "vertical",
                    }
                )?;
                if let Some(v) = repeater_indices_var_name {
                    write!(f, "{v} = @repeater-indices; ")?;
                }
                if let Some(v) = repeater_steps_var_name {
                    write!(f, "{v} = @repeater-steps; ")?;
                }
                write!(f, "{} }}", e(sub_expression))
            }
            Expression::WithFlexboxLayoutItemInfo { .. } => {
                write!(f, "WithFlexboxLayoutItemInfo(TODO)",)
            }
            Expression::SolveFlexboxLayoutWithMeasure { .. } => {
                write!(f, "SolveFlexboxLayoutWithMeasure(TODO)",)
            }
            Expression::WithGridInputData { .. } => write!(f, "WithGridInputData(TODO)",),
            Expression::MinMax { ty: _, op, lhs, rhs } => match op {
                MinMaxOp::Min => write!(f, "min({}, {})", e(lhs), e(rhs)),
                MinMaxOp::Max => write!(f, "max({}, {})", e(lhs), e(rhs)),
            },
            Expression::EmptyComponentFactory => write!(f, "<empty-component-factory>",),
            Expression::EmptyDataTransfer => write!(f, "<empty-data-transfer>",),
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
