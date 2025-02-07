// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::Cli;
use by_address::ByAddress;
use i_slint_compiler::{
    expression_tree::{Callable, Expression},
    langtype::Type,
    lookup::{LookupCtx, LookupObject, LookupResult, LookupResultCallable},
    namedreference::NamedReference,
    object_tree::ElementRc,
    parser::{SyntaxKind, SyntaxNode},
};
use smol_str::{format_smolstr, SmolStr};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    io::Write,
    rc::Rc,
};

#[derive(Clone, Default)]
pub struct LookupChangeState {
    /// the lookup change pass will map that property reference to another
    property_mappings: HashMap<NamedReference, String>,

    /// What needs to be added before the closing brace of the component
    extra_component_stuff: Rc<RefCell<Vec<u8>>>,

    /// Replace `self.` with that id
    replace_self: Option<SmolStr>,

    /// Elements that should get an id
    elements_id: HashMap<ByAddress<ElementRc>, SmolStr>,

    /// the full lookup scope
    pub scope: Vec<ElementRc>,
}

pub(crate) fn fold_node(
    node: &SyntaxNode,
    file: &mut impl Write,
    state: &mut crate::State,
    args: &Cli,
) -> std::io::Result<bool> {
    let kind = node.kind();
    if kind == SyntaxKind::QualifiedName
        && node.parent().is_some_and(|n| n.kind() == SyntaxKind::Expression)
    {
        return fully_qualify_property_access(node, file, state);
    } else if kind == SyntaxKind::Element
        && node.parent().is_some_and(|n| n.kind() == SyntaxKind::Component)
    {
        return move_properties_to_root(node, state, file, args);
    } else if kind == SyntaxKind::Element {
        if let Some(new_id) = state
            .current_elem
            .as_ref()
            .and_then(|e| state.lookup_change.elements_id.get(&ByAddress(e.clone())))
        {
            write!(file, "{new_id} := ")?;
        }
    } else if matches!(
        kind,
        SyntaxKind::Binding | SyntaxKind::TwoWayBinding | SyntaxKind::CallbackConnection
    ) && !state.lookup_change.property_mappings.is_empty()
        && node.parent().is_some_and(|n| n.kind() == SyntaxKind::Element)
    {
        if let Some(el) = &state.current_elem {
            let prop_name = i_slint_compiler::parser::normalize_identifier(
                node.child_token(SyntaxKind::Identifier).unwrap().text(),
            );
            let nr = NamedReference::new(el, prop_name);
            if let Some(new_name) = state.lookup_change.property_mappings.get(&nr).cloned() {
                state.lookup_change.replace_self = Some(
                    state
                        .lookup_change
                        .elements_id
                        .get(&ByAddress(el.clone()))
                        .map_or_else(|| el.borrow().id.clone(), |s| s.clone()),
                );
                for n in node.children_with_tokens() {
                    let extra = state.lookup_change.extra_component_stuff.clone();
                    if n.kind() == SyntaxKind::Identifier {
                        write!(&mut *extra.borrow_mut(), "{new_name}")?;
                    } else {
                        crate::visit_node_or_token(n, &mut *extra.borrow_mut(), state, args)?;
                    }
                }
                state.lookup_change.replace_self = None;
                return Ok(true);
            }
        }
    } else if matches!(kind, SyntaxKind::PropertyDeclaration | SyntaxKind::CallbackDeclaration) {
        if let Some(el) = &state.current_elem {
            let prop_name = i_slint_compiler::parser::normalize_identifier(
                &node.child_node(SyntaxKind::DeclaredIdentifier).unwrap().text().to_string(),
            );
            let nr = NamedReference::new(el, prop_name);
            if state.lookup_change.property_mappings.contains_key(&nr) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Make sure that a qualified name is fully qualified with `self.`.
/// Also rename the property in `state.lookup_change.property_mappings`
fn fully_qualify_property_access(
    node: &SyntaxNode,
    file: &mut impl Write,
    state: &mut crate::State,
) -> std::io::Result<bool> {
    let mut it = node
        .children_with_tokens()
        .filter(|n| n.kind() == SyntaxKind::Identifier)
        .filter_map(|n| n.into_token());
    let first = match it.next() {
        Some(first) => first,
        None => return Ok(false),
    };
    let first_str = i_slint_compiler::parser::normalize_identifier(first.text());
    with_lookup_ctx(state, |ctx| -> std::io::Result<bool> {
        ctx.current_token = Some(first.clone().into());
        let global_lookup = i_slint_compiler::lookup::global_lookup();
        match global_lookup.lookup(ctx, &first_str) {
            Some(
                LookupResult::Expression { expression: Expression::PropertyReference(nr), .. }
                | LookupResult::Callable(LookupResultCallable::Callable(
                    Callable::Callback(nr) | Callable::Function(nr),
                )),
            ) => {
                if let Some(new_name) = state.lookup_change.property_mappings.get(&nr) {
                    write!(file, "root.{new_name} ")?;
                    Ok(true)
                } else {
                    let element = nr.element();
                    if state
                        .current_component
                        .as_ref()
                        .is_some_and(|c| Rc::ptr_eq(&element, &c.root_element))
                    {
                        write!(file, "root.")?;
                    } else if state
                        .lookup_change
                        .scope
                        .last()
                        .is_some_and(|e| Rc::ptr_eq(&element, e))
                    {
                        if let Some(replace_self) = &state.lookup_change.replace_self {
                            write!(file, "{replace_self}.")?;
                        } else {
                            write!(file, "self.")?;
                        }
                    }
                    Ok(false)
                }
            }
            Some(LookupResult::Expression {
                expression: Expression::ElementReference(el), ..
            }) => {
                let second = match it.next() {
                    Some(second) => second,
                    None => return Ok(false),
                };
                let prop_name = i_slint_compiler::parser::normalize_identifier(second.text());
                let nr = NamedReference::new(&el.upgrade().unwrap(), prop_name);
                if let Some(new_name) = state.lookup_change.property_mappings.get(&nr) {
                    write!(file, "root.{new_name} ")?;
                    Ok(true)
                } else if let Some(replace_self) = &state.lookup_change.replace_self {
                    if first_str == "self" || first_str == "parent" {
                        if first_str == "self" {
                            write!(file, "{replace_self}.{second} ")?;
                        } else {
                            let replace_parent = state
                                .lookup_change
                                .elements_id
                                .get(&ByAddress(nr.element()))
                                .map_or_else(|| nr.element().borrow().id.clone(), |s| s.clone());
                            write!(file, "{replace_parent}.{second} ")?;
                        }
                        for t in it {
                            write!(file, ".{} ", t.text())?;
                        }
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    })
    .unwrap_or(Ok(false))
}

/// Move the properties from state.lookup_change.property_mappings to the root
fn move_properties_to_root(
    node: &SyntaxNode,
    state: &mut crate::State,
    file: &mut impl Write,
    args: &Cli,
) -> std::io::Result<bool> {
    if state.lookup_change.property_mappings.is_empty() {
        return Ok(false);
    }
    let mut seen_brace = false;
    for c in node.children_with_tokens() {
        let k = c.kind();
        if k == SyntaxKind::LBrace {
            seen_brace = true;
        } else if seen_brace && k != SyntaxKind::Whitespace {
            let property_mappings = state.lookup_change.property_mappings.clone();
            for (nr, prop) in property_mappings.iter() {
                let decl =
                    nr.element().borrow().property_declarations.get(nr.name()).unwrap().clone();
                let n2: SyntaxNode = decl.node.clone().unwrap();
                let old_current_element =
                    std::mem::replace(&mut state.current_elem, Some(nr.element()));
                state.lookup_change.replace_self = Some(
                    state
                        .lookup_change
                        .elements_id
                        .get(&ByAddress(nr.element()))
                        .map_or_else(|| nr.element().borrow().id.clone(), |s| s.clone()),
                );
                for c2 in n2.children_with_tokens() {
                    if c2.kind() == SyntaxKind::DeclaredIdentifier {
                        write!(file, " {prop} ")?;
                    } else {
                        crate::visit_node_or_token(c2, file, state, args)?;
                    }
                }
                state.lookup_change.replace_self = None;
                write!(file, "\n    ")?;
                state.current_elem = old_current_element;
            }
            seen_brace = false;
        }

        if k == SyntaxKind::RBrace {
            file.write_all(&*std::mem::take(
                &mut *state.lookup_change.extra_component_stuff.borrow_mut(),
            ))?;
        }
        crate::visit_node_or_token(c, file, state, args)?;
    }

    Ok(true)
}

pub(crate) fn with_lookup_ctx<R>(
    state: &crate::State,
    f: impl FnOnce(&mut LookupCtx) -> R,
) -> Option<R> {
    let mut build_diagnostics = Default::default();
    let tr = &state.current_doc.as_ref()?.local_registry;
    let mut lookup_context = LookupCtx::empty_context(tr, &mut build_diagnostics);

    let ty = state
        .current_elem
        .as_ref()
        .zip(state.property_name.as_ref())
        .map_or(Type::Invalid, |(e, n)| e.borrow().lookup_property(n).property_type);

    lookup_context.property_name = state.property_name.as_ref().map(SmolStr::as_str);
    lookup_context.property_type = ty;
    lookup_context.component_scope = &state.lookup_change.scope;
    Some(f(&mut lookup_context))
}

pub(crate) fn collect_movable_properties(state: &mut crate::State) {
    pub fn collect_movable_properties_recursive(vec: &mut Vec<NamedReference>, elem: &ElementRc) {
        for c in &elem.borrow().children {
            if c.borrow().repeated.is_some() {
                continue;
            }
            vec.extend(
                c.borrow()
                    .property_declarations
                    .iter()
                    .map(|(name, _)| NamedReference::new(c, name.clone())),
            );
            collect_movable_properties_recursive(vec, c);
        }
    }
    if let Some(c) = &state.current_component {
        let mut props_to_move = Vec::new();
        collect_movable_properties_recursive(&mut props_to_move, &c.root_element);
        let mut seen = HashSet::new();
        state.lookup_change.property_mappings = props_to_move
            .into_iter()
            .map(|nr| {
                let element = nr.element();
                ensure_element_has_id(&element, &mut state.lookup_change.elements_id);
                if let Some(parent) = i_slint_compiler::object_tree::find_parent_element(&element) {
                    ensure_element_has_id(&parent, &mut state.lookup_change.elements_id);
                }
                let mut name = format!("_{}_{}", element.borrow().id, nr.name());
                while !seen.insert(name.clone())
                    || c.root_element.borrow().lookup_property(&name).is_valid()
                {
                    name = format!("_{name}");
                }
                (nr, name)
            })
            .collect()
    }
}

fn ensure_element_has_id(
    element: &ElementRc,
    elements_id: &mut HashMap<ByAddress<ElementRc>, SmolStr>,
) {
    static ID_GEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    if element.borrow().id.is_empty() {
        elements_id.entry(ByAddress(element.clone())).or_insert_with(|| {
            format_smolstr!("_{}", ID_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        });
    }
}

pub(crate) fn enter_element(state: &mut crate::State) {
    if state.lookup_change.scope.last().is_some_and(|e| e.borrow().base_type.to_string() == "Path")
    {
        // Path's sub-elements have strange lookup rules: They are considering self as the Path
        state.lookup_change.replace_self = Some("parent".into());
    } else {
        state.lookup_change.scope.extend(state.current_elem.iter().cloned())
    }
}
