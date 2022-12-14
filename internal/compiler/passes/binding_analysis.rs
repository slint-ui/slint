// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! Compute binding analysis and attempt to find binding loops

use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

use by_address::ByAddress;

use crate::diagnostics::BuildDiagnostics;
use crate::diagnostics::Spanned;
use crate::expression_tree::BindingExpression;
use crate::expression_tree::BuiltinFunction;
use crate::expression_tree::Expression;
use crate::langtype::ElementType;

use crate::layout::LayoutItem;
use crate::layout::Orientation;
use crate::namedreference::NamedReference;
use crate::object_tree::Document;
use crate::object_tree::PropertyAnimation;
use crate::object_tree::{Component, ElementRc};
use derive_more as dm;

/// Maps the alias in the other direction than what the BindingExpression::two_way_binding does.
/// So if binding for property A has B in its BindingExpression::two_way_binding, then
/// ReverseAliases maps B to A.
type ReverseAliases = HashMap<NamedReference, Vec<NamedReference>>;

pub fn binding_analysis(doc: &Document, diag: &mut BuildDiagnostics) {
    let component = &doc.root_component;
    let mut reverse_aliases = Default::default();
    mark_used_base_properties(component);
    propagate_is_set_on_aliases(component, &mut reverse_aliases);
    perform_binding_analysis(component, &reverse_aliases, diag);
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
                    c: &Rc<Component>,
                ) -> bool {
                    let enclosing = e.borrow().enclosing_component.upgrade().unwrap();
                    Rc::ptr_eq(c, &enclosing)
                        || enclosing
                            .parent_element
                            .upgrade()
                            .map_or(false, |e| check_that_element_is_in_the_component(&e, c))
                }
                #[cfg(debug_assertions)]
                debug_assert!(
                    check_that_element_is_in_the_component(
                        &element,
                        last.borrow().base_type.as_component()
                    ),
                    "The element is not in the component pointed at by the path ({:?} / {:?})",
                    self,
                    second
                );
                element = last.0;
            } else {
                break;
            }
        }
        if second.elements.is_empty() {
            debug_assert!(elements.last().map_or(true, |x| *x != ByAddress(second.prop.element())));
            Self { elements, prop: NamedReference::new(&element, second.prop.name()) }
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
    currently_analyzing: linked_hash_set::LinkedHashSet<PropertyPath>,
}

fn perform_binding_analysis(
    component: &Rc<Component>,
    reverse_aliases: &ReverseAliases,
    diag: &mut BuildDiagnostics,
) {
    for c in &component.used_types.borrow().sub_components {
        perform_binding_analysis(c, reverse_aliases, diag);
    }

    let mut context = AnalysisContext::default();
    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        component,
        &(),
        &mut |e, _| analyze_element(e, &mut context, reverse_aliases, diag),
    );
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
        analyse_binding(
            &PropertyPath::from(NamedReference::new(elem, name)),
            context,
            reverse_aliases,
            diag,
        );
    }
    for (_, nr) in &elem.borrow().accessibility_props.0 {
        process_property(&PropertyPath::from(nr.clone()), context, reverse_aliases, diag);
    }
}

#[derive(Copy, Clone, dm::BitAnd, dm::BitOr, dm::BitAndAssign, dm::BitOrAssign)]
struct DependsOnExternal(bool);

fn analyse_binding(
    current: &PropertyPath,
    context: &mut AnalysisContext,
    reverse_aliases: &ReverseAliases,
    diag: &mut BuildDiagnostics,
) -> DependsOnExternal {
    let mut depends_on_external = DependsOnExternal(false);
    let element = current.prop.element();
    let name = current.prop.name();
    if context.currently_analyzing.back().map_or(false, |r| r == current)
        && !element.borrow().bindings[name].borrow().two_way_bindings.is_empty()
    {
        let span = element.borrow().bindings[name]
            .borrow()
            .span
            .clone()
            .or_else(|| element.borrow().node.as_ref().map(|n| n.to_source_location()));
        diag.push_error(format!("Property '{name}' cannot refer to itself"), &span);
        return depends_on_external;
    }

    if context.currently_analyzing.contains(current) {
        for it in context.currently_analyzing.iter().rev() {
            let p = &it.prop;
            let elem = p.element();
            let elem = elem.borrow();
            let binding = elem.bindings[p.name()].borrow();
            if binding.analysis.as_ref().unwrap().is_in_binding_loop.replace(true) {
                break;
            }

            let span =
                binding.span.clone().or_else(|| elem.node.as_ref().map(|n| n.to_source_location()));
            diag.push_error(
                format!("The binding for the property '{}' is part of a binding loop", p.name()),
                &span,
            );

            if it == current {
                break;
            }
        }
        return depends_on_external;
    }

    let binding = &element.borrow().bindings[name];
    if binding.borrow().analysis.as_ref().map_or(false, |a| a.no_external_dependencies) {
        return depends_on_external;
    } else if !context.visited.insert(current.clone()) {
        return DependsOnExternal(true);
    }

    binding.borrow_mut().analysis = Some(Default::default());
    context.currently_analyzing.insert(current.clone());

    let b = binding.borrow();
    for nr in &b.two_way_bindings {
        if nr != &current.prop {
            depends_on_external |= process_property(
                &current.relative(&nr.clone().into()),
                context,
                reverse_aliases,
                diag,
            );
        }
    }

    let mut process_prop = |prop: &PropertyPath| {
        depends_on_external |=
            process_property(&current.relative(prop), context, reverse_aliases, diag);
        for x in reverse_aliases.get(&prop.prop).unwrap_or(&Default::default()) {
            if x != &current.prop && x != &prop.prop {
                depends_on_external |= process_property(
                    &current.relative(&x.clone().into()),
                    context,
                    reverse_aliases,
                    diag,
                );
            }
        }
    };

    recurse_expression(&b.expression, &mut process_prop);

    let mut is_const =
        b.expression.is_constant() && b.two_way_bindings.iter().all(|n| n.is_constant());

    if is_const && matches!(b.expression, Expression::Invalid) {
        // check the base
        if let Some(base) = element.borrow().sub_component() {
            is_const = NamedReference::new(&base.root_element, name).is_constant();
        }
    }
    drop(b);
    binding.borrow_mut().analysis.as_mut().unwrap().is_const = is_const;

    match &binding.borrow().animation {
        Some(PropertyAnimation::Static(e)) => analyze_element(e, context, reverse_aliases, diag),
        Some(PropertyAnimation::Transition { animations, state_ref }) => {
            recurse_expression(state_ref, &mut process_prop);
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

/// Process the property `prop`
///
/// This will visit all the bindings from that property
fn process_property(
    prop: &PropertyPath,
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
        .entry(prop.prop.name().into())
        .or_default()
    {
        a => {
            a.is_read = true;
            DependsOnExternal(prop.elements.is_empty() && a.is_set_externally)
        }
    };

    let mut prop = prop.clone();

    loop {
        let element = prop.prop.element();
        if element.borrow().bindings.contains_key(prop.prop.name()) {
            analyse_binding(&prop, context, reverse_aliases, diag);
        }
        let next = if let ElementType::Component(base) = &element.borrow().base_type {
            if element.borrow().property_declarations.contains_key(prop.prop.name()) {
                break;
            }
            base.root_element.clone()
        } else {
            break;
        };
        next.borrow()
            .property_analysis
            .borrow_mut()
            .entry(prop.prop.name().into())
            .or_default()
            .is_read_externally = true;
        prop.elements.push(element.into());
        prop.prop = NamedReference::new(&next, prop.prop.name());
    }
    depends_on_external
}

// Same as in crate::visit_all_named_references_in_element, but not mut
fn recurse_expression(expr: &Expression, vis: &mut impl FnMut(&PropertyPath)) {
    expr.visit(|sub| recurse_expression(sub, vis));
    match expr {
        Expression::PropertyReference(r)
        | Expression::CallbackReference(r, _)
        | Expression::FunctionReference(r, _) => vis(&r.clone().into()),
        Expression::LayoutCacheAccess { layout_cache_prop, .. } => {
            vis(&layout_cache_prop.clone().into())
        }
        Expression::SolveLayout(l, o) | Expression::ComputeLayoutInfo(l, o) => {
            // we should only visit the layout geometry for the orientation
            if matches!(expr, Expression::SolveLayout(..)) {
                l.rect().size_reference(*o).map(|nr| vis(&nr.clone().into()));
            }
            match l {
                crate::layout::Layout::GridLayout(l) => {
                    visit_layout_items_dependencies(l.elems.iter().map(|it| &it.item), *o, vis)
                }
                crate::layout::Layout::BoxLayout(l) => {
                    visit_layout_items_dependencies(l.elems.iter(), *o, vis)
                }
                crate::layout::Layout::PathLayout(l) => {
                    for it in &l.elements {
                        vis(&NamedReference::new(it, "width").into());
                        vis(&NamedReference::new(it, "height").into());
                    }
                }
            }
            if let Some(g) = l.geometry() {
                let mut g = g.clone();
                g.rect = Default::default(); // already visited;
                g.visit_named_references(&mut |nr| vis(&nr.clone().into()))
            }
        }
        Expression::FunctionCall { function, arguments, .. } => {
            if let Expression::BuiltinFunctionReference(
                BuiltinFunction::ImplicitLayoutInfo(orientation),
                _,
            ) = &**function
            {
                if let [Expression::ElementReference(item)] = arguments.as_slice() {
                    visit_implicit_layout_info_dependencies(
                        *orientation,
                        &item.upgrade().unwrap(),
                        vis,
                    );
                }
            }
        }
        _ => {}
    }
}

fn visit_layout_items_dependencies<'a>(
    items: impl Iterator<Item = &'a LayoutItem>,
    orientation: Orientation,
    vis: &mut impl FnMut(&PropertyPath),
) {
    for it in items {
        let mut element = it.element.clone();
        if element.borrow().repeated.is_some() {
            element = it.element.borrow().base_type.as_component().root_element.clone();
        }

        if let Some(nr) = element.borrow().layout_info_prop(orientation) {
            vis(&nr.clone().into());
        } else {
            if let ElementType::Component(base) = &element.borrow().base_type {
                if let Some(nr) = base.root_element.borrow().layout_info_prop(orientation) {
                    vis(&PropertyPath {
                        elements: vec![ByAddress(element.clone())],
                        prop: nr.clone(),
                    });
                }
            }
            visit_implicit_layout_info_dependencies(orientation, &element, vis);
        }

        for (nr, _) in it.constraints.for_each_restrictions(orientation) {
            vis(&nr.clone().into())
        }
    }
}

/// The builtin function can call native code, and we need to visit the properties that are accessed by it
fn visit_implicit_layout_info_dependencies(
    orientation: crate::layout::Orientation,
    item: &ElementRc,
    vis: &mut impl FnMut(&PropertyPath),
) {
    let base_type = item.borrow().base_type.to_string();
    match base_type.as_str() {
        "Image" => {
            vis(&NamedReference::new(item, "source").into());
            if orientation == Orientation::Vertical {
                vis(&NamedReference::new(item, "width").into());
            }
        }
        "Text" | "TextInput" => {
            vis(&NamedReference::new(item, "text").into());
            vis(&NamedReference::new(item, "font-family").into());
            vis(&NamedReference::new(item, "font-size").into());
            vis(&NamedReference::new(item, "font-weight").into());
            vis(&NamedReference::new(item, "letter-spacing").into());
            vis(&NamedReference::new(item, "wrap").into());
            let wrap_set = item.borrow().is_binding_set("wrap", false)
                || item
                    .borrow()
                    .property_analysis
                    .borrow()
                    .get("wrap")
                    .map_or(false, |a| a.is_set || a.is_set_externally);
            if wrap_set && orientation == Orientation::Vertical {
                vis(&NamedReference::new(item, "width").into());
            }
            if base_type.as_str() == "TextInput" {
                vis(&NamedReference::new(item, "single-line").into());
            } else {
                vis(&NamedReference::new(item, "overflow").into());
            }
        }

        _ => (),
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
fn propagate_is_set_on_aliases(component: &Rc<Component>, reverse_aliases: &mut ReverseAliases) {
    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        component,
        &(),
        &mut |e, _| {
            for (name, binding) in &e.borrow().bindings {
                if !binding.borrow().two_way_bindings.is_empty() {
                    check_alias(e, name, &binding.borrow());

                    let nr = NamedReference::new(e, name);
                    for a in &binding.borrow().two_way_bindings {
                        if a != &nr
                            && !a
                                .element()
                                .borrow()
                                .enclosing_component
                                .upgrade()
                                .unwrap()
                                .is_global()
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
        },
    );

    fn check_alias(e: &ElementRc, name: &str, binding: &BindingExpression) {
        // Note: since the analysis hasn't been run, any property access will result in a non constant binding. this is slightly non-optimal
        let is_binding_constant =
            binding.is_constant() && binding.two_way_bindings.iter().all(|n| n.is_constant());
        if is_binding_constant && !NamedReference::new(e, name).is_externally_modified() {
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
        if !alias.is_externally_modified() {
            alias.mark_as_set();
            if let Some(bind) = alias.element().borrow().bindings.get(alias.name()) {
                propagate_alias(&bind.borrow())
            }
        }
    }

    for c in &component.used_types.borrow().sub_components {
        propagate_is_set_on_aliases(c, reverse_aliases);
    }
}

/// Make sure that the is_set_externally is true for all bindings
fn mark_used_base_properties(component: &Rc<Component>) {
    crate::object_tree::recurse_elem_including_sub_components_no_borrow(
        component,
        &(),
        &mut |element, _| {
            if !matches!(element.borrow().base_type, ElementType::Component(_)) {
                return;
            }
            for (name, binding) in &element.borrow().bindings {
                if binding.borrow().has_binding() {
                    crate::namedreference::mark_property_set_derived_in_base(element.clone(), name);
                }
            }
        },
    );

    for c in &component.used_types.borrow().sub_components {
        mark_used_base_properties(c);
    }
}
