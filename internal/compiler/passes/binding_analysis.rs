// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Compute binding analysis and attempt to find binding loops

use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

use by_address::ByAddress;

use crate::diagnostics::{BuildDiagnostics, Spanned};
use crate::expression_tree::{BindingExpression, BuiltinFunction, Expression};
use crate::langtype::ElementType;
use crate::layout::{LayoutItem, Orientation};
use crate::namedreference::NamedReference;
use crate::object_tree::{find_parent_element, Document, ElementRc, PropertyAnimation};
use derive_more as dm;

use crate::expression_tree::Callable;
use crate::CompilerConfiguration;
use smol_str::{SmolStr, ToSmolStr};

/// Maps the alias in the other direction than what the BindingExpression::two_way_binding does.
/// So if binding for property A has B in its BindingExpression::two_way_binding, then
/// ReverseAliases maps B to A.
type ReverseAliases = HashMap<NamedReference, Vec<NamedReference>>;

pub fn binding_analysis(
    doc: &Document,
    compiler_config: &CompilerConfiguration,
    diag: &mut BuildDiagnostics,
) {
    let mut reverse_aliases = Default::default();
    mark_used_base_properties(doc);
    propagate_is_set_on_aliases(doc, &mut reverse_aliases);
    perform_binding_analysis(
        doc,
        &reverse_aliases,
        compiler_config.error_on_binding_loop_with_window_layout,
        diag,
    );
}

/// A reference to a property which might be deep in a component path.
/// eg: `foo.bar.baz.background`: `baz.background` is the `prop` and `foo` and `bar` are in elements
#[derive(Hash, PartialEq, Eq, Clone)]
struct PropertyPath {
    elements: Vec<ByAddress<ElementRc>>,
    prop: NamedReference,
}

impl std::fmt::Debug for PropertyPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for e in &self.elements {
            write!(f, "{}.", e.borrow().id)?;
        }
        self.prop.fmt(f)
    }
}

impl PropertyPath {
    /// Given a namedReference accessed by something on the same leaf component
    /// as self, return a new PropertyPath that represent the property pointer
    /// to by nr in the higher possible element
    fn relative(&self, second: &PropertyPath) -> Self {
        let mut element =
            second.elements.first().map_or_else(|| second.prop.element(), |f| f.0.clone());
        if element.borrow().enclosing_component.upgrade().unwrap().is_global() {
            return second.clone();
        }
        let mut elements = self.elements.clone();
        loop {
            let enclosing = element.borrow().enclosing_component.upgrade().unwrap();
            if enclosing.parent_element.upgrade().is_some()
                || !Rc::ptr_eq(&element, &enclosing.root_element)
            {
                break;
            }

            if let Some(last) = elements.pop() {
                #[cfg(debug_assertions)]
                fn check_that_element_is_in_the_component(
                    e: &ElementRc,
                    c: &Rc<crate::object_tree::Component>,
                ) -> bool {
                    let enclosing = e.borrow().enclosing_component.upgrade().unwrap();
                    Rc::ptr_eq(c, &enclosing)
                        || enclosing
                            .parent_element
                            .upgrade()
                            .is_some_and(|e| check_that_element_is_in_the_component(&e, c))
                }
                #[cfg(debug_assertions)]
                debug_assert!(
                    check_that_element_is_in_the_component(
                        &element,
                        last.borrow().base_type.as_component()
                    ),
                    "The element is not in the component pointed at by the path ({self:?} / {second:?})"
                );
                element = last.0;
            } else {
                break;
            }
        }
        if second.elements.is_empty() {
            debug_assert!(elements.last().map_or(true, |x| *x != ByAddress(second.prop.element())));
            Self { elements, prop: NamedReference::new(&element, second.prop.name().clone()) }
        } else {
            elements.push(ByAddress(element));
            elements.extend(second.elements.iter().skip(1).cloned());
            Self { elements, prop: second.prop.clone() }
        }
    }
}

impl From<NamedReference> for PropertyPath {
    fn from(prop: NamedReference) -> Self {
        Self { elements: vec![], prop }
    }
}

#[derive(Default)]
struct AnalysisContext {
    visited: HashSet<PropertyPath>,
    /// The stack of properties that depends on each other
    currently_analyzing: linked_hash_set::LinkedHashSet<PropertyPath>,
    /// When set, one of the property in the `currently_analyzing` stack is the window layout property
    /// And we should issue a warning if that's part of a loop instead of an error
    window_layout_property: Option<PropertyPath>,
    error_on_binding_loop_with_window_layout: bool,
}

fn perform_binding_analysis(
    doc: &Document,
    reverse_aliases: &ReverseAliases,
    error_on_binding_loop_with_window_layout: bool,
    diag: &mut BuildDiagnostics,
) {
    let mut context =
        AnalysisContext { error_on_binding_loop_with_window_layout, ..Default::default() };
    doc.visit_all_used_components(|component| {
        crate::object_tree::recurse_elem_including_sub_components_no_borrow(
            component,
            &(),
            &mut |e, _| analyze_element(e, &mut context, reverse_aliases, diag),
        )
    });
}

fn analyze_element(
    elem: &ElementRc,
    context: &mut AnalysisContext,
    reverse_aliases: &ReverseAliases,
    diag: &mut BuildDiagnostics,
) {
    for (name, binding) in &elem.borrow().bindings {
        if binding.borrow().analysis.is_some() {
            continue;
        }
        analyze_binding(
            &PropertyPath::from(NamedReference::new(elem, name.clone())),
            context,
            reverse_aliases,
            diag,
        );
    }
    for cb in elem.borrow().change_callbacks.values() {
        for e in cb.borrow().iter() {
            recurse_expression(elem, e, &mut |prop, r| {
                process_property(prop, r, context, reverse_aliases, diag);
            });
        }
    }
    const P: ReadType = ReadType::PropertyRead;
    for nr in elem.borrow().accessibility_props.0.values() {
        process_property(&PropertyPath::from(nr.clone()), P, context, reverse_aliases, diag);
    }
    if let Some(g) = elem.borrow().geometry_props.as_ref() {
        process_property(&g.x.clone().into(), P, context, reverse_aliases, diag);
        process_property(&g.y.clone().into(), P, context, reverse_aliases, diag);
        process_property(&g.width.clone().into(), P, context, reverse_aliases, diag);
        process_property(&g.height.clone().into(), P, context, reverse_aliases, diag);
    }

    if let Some(component) = elem.borrow().enclosing_component.upgrade() {
        if Rc::ptr_eq(&component.root_element, elem) {
            for e in component.init_code.borrow().iter() {
                recurse_expression(elem, e, &mut |prop, r| {
                    process_property(prop, r, context, reverse_aliases, diag);
                });
            }
            component.root_constraints.borrow_mut().visit_named_references(&mut |nr| {
                process_property(&nr.clone().into(), P, context, reverse_aliases, diag);
            });
            component.popup_windows.borrow().iter().for_each(|p| {
                process_property(&p.x.clone().into(), P, context, reverse_aliases, diag);
                process_property(&p.y.clone().into(), P, context, reverse_aliases, diag);
            });
            component.timers.borrow().iter().for_each(|t| {
                process_property(&t.interval.clone().into(), P, context, reverse_aliases, diag);
                process_property(&t.running.clone().into(), P, context, reverse_aliases, diag);
                process_property(&t.triggered.clone().into(), P, context, reverse_aliases, diag);
            });
        }
    }

    if let Some(repeated) = &elem.borrow().repeated {
        recurse_expression(elem, &repeated.model, &mut |prop, r| {
            process_property(prop, r, context, reverse_aliases, diag);
        });
        if let Some(lv) = &repeated.is_listview {
            process_property(&lv.viewport_y.clone().into(), P, context, reverse_aliases, diag);
            process_property(&lv.viewport_height.clone().into(), P, context, reverse_aliases, diag);
            process_property(&lv.viewport_width.clone().into(), P, context, reverse_aliases, diag);
            process_property(&lv.listview_height.clone().into(), P, context, reverse_aliases, diag);
            process_property(&lv.listview_width.clone().into(), P, context, reverse_aliases, diag);
        }
    }
    if let Some((h, v)) = &elem.borrow().layout_info_prop {
        process_property(&h.clone().into(), P, context, reverse_aliases, diag);
        process_property(&v.clone().into(), P, context, reverse_aliases, diag);
    }
}

#[derive(Copy, Clone, dm::BitAnd, dm::BitOr, dm::BitAndAssign, dm::BitOrAssign)]
struct DependsOnExternal(bool);

fn analyze_binding(
    current: &PropertyPath,
    context: &mut AnalysisContext,
    reverse_aliases: &ReverseAliases,
    diag: &mut BuildDiagnostics,
) -> DependsOnExternal {
    let mut depends_on_external = DependsOnExternal(false);
    let element = current.prop.element();
    let name = current.prop.name();
    if (context.currently_analyzing.back() == Some(current))
        && !element.borrow().bindings[name].borrow().two_way_bindings.is_empty()
    {
        let span = element.borrow().bindings[name]
            .borrow()
            .span
            .clone()
            .unwrap_or_else(|| element.borrow().to_source_location());
        diag.push_error(format!("Property '{name}' cannot refer to itself"), &span);
        return depends_on_external;
    }

    if context.currently_analyzing.contains(current) {
        let mut loop_description = String::new();
        let mut has_window_layout = false;
        for it in context.currently_analyzing.iter().rev() {
            if context.window_layout_property.as_ref().is_some_and(|p| p == it) {
                has_window_layout = true;
            }
            if !loop_description.is_empty() {
                loop_description.push_str(" -> ");
            }
            match it.prop.element().borrow().id.as_str() {
                "" => loop_description.push_str(it.prop.name()),
                id => {
                    loop_description.push_str(id);
                    loop_description.push_str(".");
                    loop_description.push_str(it.prop.name());
                }
            }
            if it == current {
                break;
            }
        }

        for it in context.currently_analyzing.iter().rev() {
            let p = &it.prop;
            let elem = p.element();
            let elem = elem.borrow();
            let binding = elem.bindings[p.name()].borrow();
            if binding.analysis.as_ref().unwrap().is_in_binding_loop.replace(true) {
                break;
            }

            let span = binding.span.clone().unwrap_or_else(|| elem.to_source_location());
            if !context.error_on_binding_loop_with_window_layout && has_window_layout {
                diag.push_warning(format!("The binding for the property '{}' is part of a binding loop ({loop_description}).\nThis was allowed in previous version of Slint, but is deprecated and may cause panic at runtime", p.name()), &span);
            } else {
                diag.push_error(format!("The binding for the property '{}' is part of a binding loop ({loop_description})", p.name()), &span);
            }
            if it == current {
                break;
            }
        }
        return depends_on_external;
    }

    let binding = &element.borrow().bindings[name];
    if binding.borrow().analysis.as_ref().is_some_and(|a| a.no_external_dependencies) {
        return depends_on_external;
    } else if !context.visited.insert(current.clone()) {
        return DependsOnExternal(true);
    }

    if let Ok(mut b) = binding.try_borrow_mut() {
        b.analysis = Some(Default::default());
    };
    context.currently_analyzing.insert(current.clone());

    let b = binding.borrow();
    for nr in &b.two_way_bindings {
        if nr != &current.prop {
            depends_on_external |= process_property(
                &current.relative(&nr.clone().into()),
                ReadType::PropertyRead,
                context,
                reverse_aliases,
                diag,
            );
        }
    }

    let mut process_prop = |prop: &PropertyPath, r| {
        depends_on_external |=
            process_property(&current.relative(prop), r, context, reverse_aliases, diag);
        for x in reverse_aliases.get(&prop.prop).unwrap_or(&Default::default()) {
            if x != &current.prop && x != &prop.prop {
                depends_on_external |= process_property(
                    &current.relative(&x.clone().into()),
                    ReadType::PropertyRead,
                    context,
                    reverse_aliases,
                    diag,
                );
            }
        }
    };

    recurse_expression(&current.prop.element(), &b.expression, &mut process_prop);

    let mut is_const =
        b.expression.is_constant() && b.two_way_bindings.iter().all(|n| n.is_constant());

    if is_const && matches!(b.expression, Expression::Invalid) {
        // check the base
        if let Some(base) = element.borrow().sub_component() {
            is_const = NamedReference::new(&base.root_element, name.clone()).is_constant();
        }
    }
    drop(b);

    if let Ok(mut b) = binding.try_borrow_mut() {
        // We have a loop (through different component so we're still borrowed)
        b.analysis.as_mut().unwrap().is_const = is_const;
    }

    match &binding.borrow().animation {
        Some(PropertyAnimation::Static(e)) => analyze_element(e, context, reverse_aliases, diag),
        Some(PropertyAnimation::Transition { animations, state_ref }) => {
            recurse_expression(&current.prop.element(), state_ref, &mut process_prop);
            for a in animations {
                analyze_element(&a.animation, context, reverse_aliases, diag);
            }
        }
        None => (),
    }

    let o = context.currently_analyzing.pop_back();
    assert_eq!(&o.unwrap(), current);

    depends_on_external
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum ReadType {
    // Read from the native code
    NativeRead,
    // Read from another property binding in Slint
    PropertyRead,
}

/// Process the property `prop`
///
/// This will visit all the bindings from that property
fn process_property(
    prop: &PropertyPath,
    read_type: ReadType,
    context: &mut AnalysisContext,
    reverse_aliases: &ReverseAliases,
    diag: &mut BuildDiagnostics,
) -> DependsOnExternal {
    let depends_on_external = match prop
        .prop
        .element()
        .borrow()
        .property_analysis
        .borrow_mut()
        .entry(prop.prop.name().clone())
        .or_default()
    {
        a => {
            if read_type == ReadType::PropertyRead {
                a.is_read = true;
            }
            DependsOnExternal(prop.elements.is_empty() && a.is_set_externally)
        }
    };

    let mut prop = prop.clone();

    loop {
        let element = prop.prop.element();
        if element.borrow().bindings.contains_key(prop.prop.name()) {
            analyze_binding(&prop, context, reverse_aliases, diag);
            break;
        }
        let next = match &element.borrow().base_type {
            ElementType::Component(base) => {
                if element.borrow().property_declarations.contains_key(prop.prop.name()) {
                    break;
                }
                base.root_element.clone()
            }
            ElementType::Builtin(builtin) => {
                if builtin.properties.contains_key(prop.prop.name()) {
                    visit_builtin_property(builtin, &prop, context, reverse_aliases, diag);
                }
                break;
            }
            _ => break,
        };
        next.borrow()
            .property_analysis
            .borrow_mut()
            .entry(prop.prop.name().clone())
            .or_default()
            .is_read_externally = true;
        prop.elements.push(element.into());
        prop.prop = NamedReference::new(&next, prop.prop.name().clone());
    }
    depends_on_external
}

// Same as in crate::visit_all_named_references_in_element, but not mut
fn recurse_expression(
    elem: &ElementRc,
    expr: &Expression,
    vis: &mut impl FnMut(&PropertyPath, ReadType),
) {
    const P: ReadType = ReadType::PropertyRead;
    expr.visit(|sub| recurse_expression(elem, sub, vis));
    match expr {
        Expression::PropertyReference(r) => vis(&r.clone().into(), P),
        Expression::LayoutCacheAccess { layout_cache_prop, .. } => {
            vis(&layout_cache_prop.clone().into(), P)
        }
        Expression::SolveLayout(l, o) | Expression::ComputeLayoutInfo(l, o) => {
            // we should only visit the layout geometry for the orientation
            if matches!(expr, Expression::SolveLayout(..)) {
                if let Some(nr) = l.rect().size_reference(*o) {
                    vis(&nr.clone().into(), P);
                }
            }
            match l {
                crate::layout::Layout::GridLayout(l) => {
                    visit_layout_items_dependencies(l.elems.iter().map(|it| &it.item), *o, vis)
                }
                crate::layout::Layout::BoxLayout(l) => {
                    visit_layout_items_dependencies(l.elems.iter(), *o, vis)
                }
            }

            let mut g = l.geometry().clone();
            g.rect = Default::default(); // already visited;
            g.visit_named_references(&mut |nr| vis(&nr.clone().into(), P))
        }
        Expression::FunctionCall {
            function: Callable::Callback(nr) | Callable::Function(nr),
            ..
        } => vis(&nr.clone().into(), P),
        Expression::FunctionCall { function: Callable::Builtin(b), arguments, .. } => match b {
            BuiltinFunction::ImplicitLayoutInfo(orientation) => {
                if let [Expression::ElementReference(item)] = arguments.as_slice() {
                    visit_implicit_layout_info_dependencies(
                        *orientation,
                        &item.upgrade().unwrap(),
                        vis,
                    );
                }
            }
            BuiltinFunction::ItemAbsolutePosition => {
                if let Some(Expression::ElementReference(item)) = arguments.first() {
                    let mut item = item.upgrade().unwrap();
                    while let Some(parent) = find_parent_element(&item) {
                        item = parent;
                        vis(
                            &NamedReference::new(&item, SmolStr::new_static("x")).into(),
                            ReadType::NativeRead,
                        );
                        vis(
                            &NamedReference::new(&item, SmolStr::new_static("y")).into(),
                            ReadType::NativeRead,
                        );
                    }
                }
            }
            BuiltinFunction::ItemFontMetrics => {
                if let Some(Expression::ElementReference(item)) = arguments.first() {
                    let item = item.upgrade().unwrap();
                    vis(
                        &NamedReference::new(&item, SmolStr::new_static("font-size")).into(),
                        ReadType::NativeRead,
                    );
                    vis(
                        &NamedReference::new(&item, SmolStr::new_static("font-weight")).into(),
                        ReadType::NativeRead,
                    );
                    vis(
                        &NamedReference::new(&item, SmolStr::new_static("font-family")).into(),
                        ReadType::NativeRead,
                    );
                    vis(
                        &NamedReference::new(&item, SmolStr::new_static("font-italic")).into(),
                        ReadType::NativeRead,
                    );
                }
            }
            BuiltinFunction::GetWindowDefaultFontSize => {
                let root =
                    elem.borrow().enclosing_component.upgrade().unwrap().root_element.clone();
                if root.borrow().builtin_type().is_some_and(|bt| bt.name == "Window") {
                    vis(
                        &NamedReference::new(&root, SmolStr::new_static("default-font-size"))
                            .into(),
                        ReadType::PropertyRead,
                    );
                }
            }
            _ => {}
        },
        _ => {}
    }
}

fn visit_layout_items_dependencies<'a>(
    items: impl Iterator<Item = &'a LayoutItem>,
    orientation: Orientation,
    vis: &mut impl FnMut(&PropertyPath, ReadType),
) {
    for it in items {
        let mut element = it.element.clone();
        if element
            .borrow()
            .repeated
            .as_ref()
            .map(|r| recurse_expression(&element, &r.model, vis))
            .is_some()
        {
            element = it.element.borrow().base_type.as_component().root_element.clone();
        }

        if let Some(nr) = element.borrow().layout_info_prop(orientation) {
            vis(&nr.clone().into(), ReadType::PropertyRead);
        } else {
            if let ElementType::Component(base) = &element.borrow().base_type {
                if let Some(nr) = base.root_element.borrow().layout_info_prop(orientation) {
                    vis(
                        &PropertyPath {
                            elements: vec![ByAddress(element.clone())],
                            prop: nr.clone(),
                        },
                        ReadType::PropertyRead,
                    );
                }
            }
            visit_implicit_layout_info_dependencies(orientation, &element, vis);
        }

        for (nr, _) in it.constraints.for_each_restrictions(orientation) {
            vis(&nr.clone().into(), ReadType::PropertyRead)
        }
    }
}

/// The builtin function can call native code, and we need to visit the properties that are accessed by it
fn visit_implicit_layout_info_dependencies(
    orientation: crate::layout::Orientation,
    item: &ElementRc,
    vis: &mut impl FnMut(&PropertyPath, ReadType),
) {
    let base_type = item.borrow().base_type.to_smolstr();
    const N: ReadType = ReadType::NativeRead;
    match base_type.as_str() {
        "Image" => {
            vis(&NamedReference::new(item, SmolStr::new_static("source")).into(), N);
            if orientation == Orientation::Vertical {
                vis(&NamedReference::new(item, SmolStr::new_static("width")).into(), N);
            }
        }
        "Text" | "TextInput" => {
            vis(&NamedReference::new(item, SmolStr::new_static("text")).into(), N);
            vis(&NamedReference::new(item, SmolStr::new_static("font-family")).into(), N);
            vis(&NamedReference::new(item, SmolStr::new_static("font-size")).into(), N);
            vis(&NamedReference::new(item, SmolStr::new_static("font-weight")).into(), N);
            vis(&NamedReference::new(item, SmolStr::new_static("letter-spacing")).into(), N);
            vis(&NamedReference::new(item, SmolStr::new_static("wrap")).into(), N);
            let wrap_set = item.borrow().is_binding_set("wrap", false)
                || item
                    .borrow()
                    .property_analysis
                    .borrow()
                    .get("wrap")
                    .is_some_and(|a| a.is_set || a.is_set_externally);
            if wrap_set && orientation == Orientation::Vertical {
                vis(&NamedReference::new(item, SmolStr::new_static("width")).into(), N);
            }
            if base_type.as_str() == "TextInput" {
                vis(&NamedReference::new(item, SmolStr::new_static("single-line")).into(), N);
            } else {
                vis(&NamedReference::new(item, SmolStr::new_static("overflow")).into(), N);
            }
        }

        _ => (),
    }
}

fn visit_builtin_property(
    builtin: &crate::langtype::BuiltinElement,
    prop: &PropertyPath,
    context: &mut AnalysisContext,
    reverse_aliases: &ReverseAliases,
    diag: &mut BuildDiagnostics,
) {
    let name = prop.prop.name();
    if builtin.name == "Window" {
        for (p, orientation) in
            [("width", Orientation::Horizontal), ("height", Orientation::Vertical)]
        {
            if name == p {
                // find the actual root component
                let is_root = |e: &ElementRc| -> bool {
                    ElementRc::ptr_eq(
                        e,
                        &e.borrow().enclosing_component.upgrade().unwrap().root_element,
                    )
                };
                let mut root = prop.prop.element();
                if !is_root(&root) {
                    return;
                };
                for e in prop.elements.iter().rev() {
                    if !is_root(&e.0) {
                        return;
                    }
                    root = e.0.clone();
                }
                if let Some(p) = root.borrow().layout_info_prop(orientation) {
                    let path = PropertyPath::from(p.clone());
                    let old_layout = context.window_layout_property.replace(path.clone());
                    process_property(&path, ReadType::NativeRead, context, reverse_aliases, diag);
                    context.window_layout_property = old_layout;
                };
            }
        }
    }
}

/// Make sure that the is_set property analysis is set to any property which has a two way binding
/// to a property that is, itself, is set
///
/// Example:
/// ```slint
/// Xx := TouchArea {
///    property <int> bar <=> foo;
///    clicked => { bar+=1; }
///    property <int> foo; // must ensure that this is not considered as const, because the alias with bar
/// }
/// ```
fn propagate_is_set_on_aliases(doc: &Document, reverse_aliases: &mut ReverseAliases) {
    doc.visit_all_used_components(|component| {
        crate::object_tree::recurse_elem_including_sub_components_no_borrow(
            component,
            &(),
            &mut |e, _| visit_element(e, reverse_aliases),
        );
    });

    fn visit_element(e: &ElementRc, reverse_aliases: &mut ReverseAliases) {
        for (name, binding) in &e.borrow().bindings {
            if !binding.borrow().two_way_bindings.is_empty() {
                check_alias(e, name, &binding.borrow());

                let nr = NamedReference::new(e, name.clone());
                for a in &binding.borrow().two_way_bindings {
                    if a != &nr
                        && !a.element().borrow().enclosing_component.upgrade().unwrap().is_global()
                    {
                        reverse_aliases.entry(a.clone()).or_default().push(nr.clone())
                    }
                }
            }
        }
        for decl in e.borrow().property_declarations.values() {
            if let Some(alias) = &decl.is_alias {
                mark_alias(alias)
            }
        }
    }

    fn check_alias(e: &ElementRc, name: &SmolStr, binding: &BindingExpression) {
        // Note: since the analysis hasn't been run, any property access will result in a non constant binding. this is slightly non-optimal
        let is_binding_constant =
            binding.is_constant() && binding.two_way_bindings.iter().all(|n| n.is_constant());
        if is_binding_constant && !NamedReference::new(e, name.clone()).is_externally_modified() {
            for alias in &binding.two_way_bindings {
                crate::namedreference::mark_property_set_derived_in_base(
                    alias.element(),
                    alias.name(),
                );
            }
            return;
        }

        propagate_alias(binding);
    }

    fn propagate_alias(binding: &BindingExpression) {
        for alias in &binding.two_way_bindings {
            mark_alias(alias);
        }
    }

    fn mark_alias(alias: &NamedReference) {
        alias.mark_as_set();
        if !alias.is_externally_modified() {
            if let Some(bind) = alias.element().borrow().bindings.get(alias.name()) {
                propagate_alias(&bind.borrow())
            }
        }
    }
}

/// Make sure that the is_set_externally is true for all bindings.
/// And change bindings are used externally
fn mark_used_base_properties(doc: &Document) {
    doc.visit_all_used_components(|component| {
        crate::object_tree::recurse_elem_including_sub_components_no_borrow(
            component,
            &(),
            &mut |element, _| {
                if !matches!(element.borrow().base_type, ElementType::Component(_)) {
                    return;
                }
                for (name, binding) in &element.borrow().bindings {
                    if binding.borrow().has_binding() {
                        crate::namedreference::mark_property_set_derived_in_base(
                            element.clone(),
                            name,
                        );
                    }
                }
                for name in element.borrow().change_callbacks.keys() {
                    crate::namedreference::mark_property_read_derived_in_base(
                        element.clone(),
                        name,
                    );
                }
            },
        );
    });
}
