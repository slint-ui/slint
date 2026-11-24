// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The anchors of the `anchors` experimental feature: `left`, `horizontal-center`, `right`,
//! `top`, `vertical-center`, and `bottom`.
//!
//! An anchor is an edge of an element. Its binding names an edge of another element,
//! plus or minus a length. [`lower_anchors`] turns the anchors of each axis into bindings of
//! `x` and `width`, or `y` and `height`, in the coordinates of the element's parent.
//! Outside of anchor bindings, an anchor can only be read as the difference of two anchors.

#![allow(clippy::mutable_key_type, reason = "ByAddress<ElementRc> keys hash by pointer")]

use crate::diagnostics::{BuildDiagnostics, SourceLocation, Spanned};
use crate::element_type::{ElementType, PropertyLookupMode, PropertyLookupResult};
use crate::expression_tree::{BindingExpression, Expression};
use crate::expression_types::{Orientation as Axis, Unit};
use crate::langtype::Type;
use crate::namedreference::NamedReference;
use crate::native_class::PropertyVisibility;
use crate::object_tree::{Document, ElementRc, recurse_elem, visit_element_expressions};
use crate::parser::syntax_nodes;
use by_address::ByAddress;
use smol_str::SmolStr;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

const AXES: [Axis; 2] = [Axis::Horizontal, Axis::Vertical];

fn position(axis: Axis) -> &'static str {
    match axis {
        Axis::Horizontal => "x",
        Axis::Vertical => "y",
    }
}

fn size(axis: Axis) -> &'static str {
    match axis {
        Axis::Horizontal => "width",
        Axis::Vertical => "height",
    }
}

fn preferred_size(axis: Axis) -> &'static str {
    match axis {
        Axis::Horizontal => "preferred-width",
        Axis::Vertical => "preferred-height",
    }
}

fn axis_word(axis: Axis) -> &'static str {
    match axis {
        Axis::Horizontal => "horizontal",
        Axis::Vertical => "vertical",
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Edge {
    Start,
    Center,
    End,
}

const ANCHORS: [(&str, Axis, Edge); 6] = [
    ("left", Axis::Horizontal, Edge::Start),
    ("horizontal-center", Axis::Horizontal, Edge::Center),
    ("right", Axis::Horizontal, Edge::End),
    ("top", Axis::Vertical, Edge::Start),
    ("vertical-center", Axis::Vertical, Edge::Center),
    ("bottom", Axis::Vertical, Edge::End),
];

fn anchor_names(axis: Axis) -> impl Iterator<Item = &'static str> {
    ANCHORS.iter().filter(move |(_, a, _)| *a == axis).map(|(name, ..)| *name)
}

fn anchor_name(axis: Axis, edge: Edge) -> &'static str {
    ANCHORS.iter().find(|(_, a, e)| *a == axis && *e == edge).unwrap().0
}

/// The lookup result of the anchor property `name`, or `None` if `name` isn't an anchor.
pub(crate) fn anchor_property(name: &str) -> Option<PropertyLookupResult<'static>> {
    let (name, ..) = ANCHORS.iter().find(|(n, ..)| *n == name)?;
    Some(PropertyLookupResult {
        resolved_name: (*name).into(),
        property_type: Type::LogicalLength,
        property_visibility: PropertyVisibility::Input,
        declared_pure: None,
        is_local_to_component: false,
        is_in_direct_base: false,
        is_shadowable: true,
        builtin_function: None,
        is_slint_sc: false,
        internal_name: None,
        deprecated: None,
    })
}

/// The axis and edge of the property `name` of `elem`, if it's an anchor and not a property
/// that a component declares under the same name.
fn anchor_of(elem: &ElementRc, name: &str) -> Option<(Axis, Edge)> {
    let (_, axis, edge) = ANCHORS.iter().find(|(n, ..)| *n == name)?;
    let e = elem.borrow();
    let lookup = e.lookup_property(name, PropertyLookupMode::InternalName);
    (lookup.is_valid()
        && lookup.internal_name.is_none()
        && e.property_declaration_node(name).is_none())
    .then_some((*axis, *edge))
}

/// The axis whose anchors, position, or size `name` sets on `elem`.
fn axis_of(elem: &ElementRc, name: &str) -> Option<Axis> {
    if let Some((axis, _)) = anchor_of(elem, name) {
        return Some(axis);
    }
    AXES.into_iter().find(|axis| name == position(*axis) || name == size(*axis))
}

fn is_auto(expr: &Expression) -> bool {
    let Expression::Uncompiled(node) = expr else { return false };
    let text = node.text().to_string();
    text.trim().trim_end_matches(';').trim() == "auto"
}

/// A property that a state sets to `auto`, which releases it in that state.
pub struct Released {
    /// The element that declares the state
    owner: ElementRc,
    state: usize,
    target: ElementRc,
    property: SmolStr,
    node: syntax_nodes::StatePropertyChange,
}

/// The anchors of a document, found before its expressions get resolved.
pub struct Anchors {
    /// The axes of the elements that set an anchor, in a binding or in a state
    anchored: HashSet<(ByAddress<ElementRc>, Axis)>,
    released: Vec<Released>,
}

/// Removes the bindings and state changes that set an anchor to `auto`, before they get resolved.
/// `width: auto` and `height: auto` count only on an axis that has anchors.
pub fn remove_auto(doc: &Document) -> Anchors {
    let mut released = Vec::new();
    let mut anchored = HashSet::new();
    for component in &doc.inner_components {
        recurse_elem(&component.root_element, &(), &mut |elem, _| {
            let e = elem.borrow();
            for (name, _) in e.real_bindings() {
                if let Some((axis, _)) = anchor_of(elem, name) {
                    anchored.insert((ByAddress(elem.clone()), axis));
                }
            }
            for state in &e.states {
                for (nr, ..) in &state.property_changes {
                    if let Some((axis, _)) = anchor_of(&nr.element(), nr.name()) {
                        anchored.insert((ByAddress(nr.element()), axis));
                    }
                }
            }
        });
        let releases = |elem: &ElementRc, name: &str| {
            anchor_of(elem, name).is_some()
                || axis_of(elem, name)
                    .is_some_and(|axis| anchored.contains(&(ByAddress(elem.clone()), axis)))
        };
        recurse_elem(&component.root_element, &(), &mut |elem, _| {
            let auto_bindings = elem
                .borrow()
                .real_bindings()
                .filter(|(name, b)| releases(elem, name) && is_auto(&b.borrow().expression))
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>();
            for name in auto_bindings {
                elem.borrow_mut().take_binding(&name);
            }
            let mut states = std::mem::take(&mut elem.borrow_mut().states);
            for (index, state) in states.iter_mut().enumerate() {
                state.property_changes.retain(|(nr, expr, node)| {
                    let release = releases(&nr.element(), nr.name()) && is_auto(expr);
                    if release {
                        released.push(Released {
                            owner: elem.clone(),
                            state: index,
                            target: nr.element(),
                            property: nr.name().clone(),
                            node: node.clone(),
                        });
                    }
                    !release
                });
            }
            elem.borrow_mut().states = states;
        });
    }
    Anchors { anchored, released }
}

/// The values set on one axis of an element, in its bindings or in one of its states.
#[derive(Clone, Default)]
struct AxisSet {
    /// By property name: an anchor, the position, or the size
    values: Vec<(SmolStr, Expression, SourceLocation)>,
}

impl AxisSet {
    fn set(&mut self, name: SmolStr, expr: Expression, location: SourceLocation) {
        self.remove(&name);
        self.values.push((name, expr, location));
    }

    fn remove(&mut self, name: &str) {
        self.values.retain(|(n, ..)| n != name);
    }

    fn contains(&self, name: &str) -> bool {
        self.values.iter().any(|(n, ..)| n == name)
    }

    fn has_anchor(&self, elem: &ElementRc) -> bool {
        self.values.iter().any(|(name, ..)| anchor_of(elem, name).is_some())
    }
}

struct Context<'a> {
    parents: HashMap<ByAddress<ElementRc>, ElementRc>,
    diag: &'a mut BuildDiagnostics,
}

impl Context<'_> {
    fn parent(&self, elem: &ElementRc) -> Option<ElementRc> {
        self.parents.get(&ByAddress(elem.clone())).cloned()
    }

    /// `elem`, then its parent, up to the root of its component.
    fn ancestors(&self, elem: &ElementRc) -> impl Iterator<Item = ElementRc> {
        std::iter::successors(Some(elem.clone()), |e| self.parent(e))
    }

    /// The closest element that `a` and `b` are inside of.
    fn common_ancestor(&self, a: &ElementRc, b: &ElementRc) -> Option<ElementRc> {
        let b_ancestors = self.ancestors(b).map(ByAddress).collect::<HashSet<_>>();
        self.ancestors(a).find(|e| b_ancestors.contains(&ByAddress(e.clone())))
    }
}

fn property(elem: &ElementRc, name: &'static str) -> Expression {
    Expression::PropertyReference(NamedReference::new(elem, SmolStr::new_static(name)))
}

fn binary(lhs: Expression, op: char, rhs: Expression) -> Expression {
    Expression::BinaryExpression {
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        op,
        source_location: None,
    }
}

fn number(value: f64) -> Expression {
    Expression::NumberLiteral(value, Unit::None)
}

fn is_flickable(elem: &ElementRc) -> bool {
    elem.borrow().builtin_type().is_some_and(|b| b.name == "Flickable")
}

/// The position of an anchor of `elem`, in the coordinates of the children of `frame`.
/// Adds the positions of the elements on the path from `frame` to `elem`.
fn edge_in_frame(
    elem: &ElementRc,
    axis: Axis,
    edge: Edge,
    frame: &ElementRc,
    ctx: &Context,
) -> Result<Expression, String> {
    let Some(common) = ctx.common_ancestor(elem, frame) else {
        return Err("Can't anchor to an element of another component".into());
    };
    let content = match axis {
        Axis::Horizontal => "content-x",
        Axis::Vertical => "content-y",
    };
    // The children of a Flickable are in its content. The Flickable's own anchors are
    // edges of the content, so that its children scroll with it.
    let path = |start: &ElementRc, own_content: bool| {
        let mut terms = Vec::new();
        for (i, e) in ctx.ancestors(start).take_while(|e| !Rc::ptr_eq(e, &common)).enumerate() {
            if (own_content || i > 0) && is_flickable(&e) {
                terms.push(property(&e, content));
            }
            terms.push(property(&e, position(axis)));
        }
        terms
    };
    let size = || property(elem, size(axis));
    let mut terms = path(elem, false).into_iter().map(|t| ('+', t)).collect::<Vec<_>>();
    terms.extend(path(frame, true).into_iter().map(|t| ('-', t)));
    match edge {
        Edge::Start => {}
        Edge::Center => terms.push(('+', binary(size(), '/', number(2.)))),
        Edge::End => terms.push(('+', size())),
    }
    let zero = || Expression::NumberLiteral(0., Unit::Px);
    Ok(terms
        .into_iter()
        .fold(None, |sum, (op, term)| match sum {
            None if op == '+' => Some(term),
            None => Some(binary(zero(), op, term)),
            Some(sum) => Some(binary(sum, op, term)),
        })
        .unwrap_or_else(zero))
}

/// Whether `elem` places its children in an element other than its root, with `@children`.
fn places_children_inside(elem: &ElementRc) -> bool {
    let mut base = elem.borrow().base_type.clone();
    while let ElementType::Component(component) = base {
        let inside = component
            .child_insertion_points
            .borrow()
            .values()
            .any(|cip| !Rc::ptr_eq(&cip.parent, &component.root_element));
        if inside {
            return true;
        }
        base = component.root_element.borrow().base_type.clone();
    }
    false
}

fn contains_anchor(expr: &Expression) -> bool {
    let mut found = false;
    expr.visit_recursive(&mut |e| {
        if let Expression::PropertyReference(nr) = e
            && anchor_of(&nr.element(), nr.name()).is_some()
        {
            found = true;
        }
    });
    found
}

/// The closest common ancestor of the elements whose anchors `expr` reads,
/// which is the cheapest frame for a distance between them.
fn distance_frame(expr: &Expression, ctx: &Context) -> Option<ElementRc> {
    let mut frame: Option<ElementRc> = None;
    expr.visit_recursive(&mut |e| {
        if let Expression::PropertyReference(nr) = e
            && anchor_of(&nr.element(), nr.name()).is_some()
        {
            let elem = nr.element();
            frame = Some(match frame.take() {
                None => elem,
                Some(f) => ctx.common_ancestor(&f, &elem).unwrap_or(f),
            });
        }
    });
    frame
}

const NOT_A_LENGTH: &str =
    "An anchor isn't a length. Subtract two anchors to get the distance between them";

/// Replaces the anchors in `expr` by their position in the coordinates of `frame`.
/// Returns the axis if `expr` is an anchor, or `None` if it's a length.
fn lower_expression(
    expr: &Expression,
    frame: &ElementRc,
    ctx: &Context,
) -> Result<(Expression, Option<Axis>), String> {
    if !contains_anchor(expr) {
        return Ok((expr.clone(), None));
    }
    match expr {
        Expression::PropertyReference(nr) => {
            let (axis, edge) = anchor_of(&nr.element(), nr.name()).unwrap();
            Ok((edge_in_frame(&nr.element(), axis, edge, frame, ctx)?, Some(axis)))
        }
        Expression::BinaryExpression { lhs, rhs, op: op @ ('+' | '-'), .. } => {
            let (l, l_axis) = lower_expression(lhs, frame, ctx)?;
            let (r, r_axis) = lower_expression(rhs, frame, ctx)?;
            let axis = match (l_axis, r_axis, op) {
                (Some(axis), None, _) | (None, Some(axis), '+') => Some(axis),
                (Some(a), Some(b), '-') if a == b => {
                    if let Some(distance_frame) = distance_frame(expr, ctx)
                        && !Rc::ptr_eq(&distance_frame, frame)
                    {
                        return Ok((lower_length(expr, &distance_frame, ctx)?, None));
                    }
                    None
                }
                (Some(_), Some(_), '-') => {
                    return Err(
                        "Can't subtract a vertical anchor from a horizontal anchor, or the reverse"
                            .into(),
                    );
                }
                (Some(_), Some(_), _) => {
                    return Err(
                        "Can't add two anchors. Subtract them to get the distance between them"
                            .into(),
                    );
                }
                (None, Some(_), _) => return Err("Can't subtract an anchor from a length".into()),
                (None, None, _) => None,
            };
            Ok((binary(l, *op, r), axis))
        }
        Expression::Condition { condition, true_expr, false_expr, source_location } => {
            let condition = lower_length(condition, frame, ctx)?;
            let (t, t_axis) = lower_expression(true_expr, frame, ctx)?;
            let (f, f_axis) = lower_expression(false_expr, frame, ctx)?;
            if t_axis != f_axis {
                return Err(
                    "Both branches of the condition must be anchors of the same axis, or lengths"
                        .into(),
                );
            }
            Ok((
                Expression::Condition {
                    condition: Box::new(condition),
                    true_expr: Box::new(t),
                    false_expr: Box::new(f),
                    source_location: source_location.clone(),
                },
                t_axis,
            ))
        }
        Expression::CodeBlock(statements) => {
            let mut axis = None;
            let mut lowered = Vec::with_capacity(statements.len());
            for (i, statement) in statements.iter().enumerate() {
                let (s, a) = lower_expression(statement, frame, ctx)?;
                if i + 1 == statements.len() {
                    axis = a;
                } else if a.is_some() {
                    return Err(NOT_A_LENGTH.into());
                }
                lowered.push(s);
            }
            Ok((Expression::CodeBlock(lowered), axis))
        }
        _ => {
            let mut lowered = expr.clone();
            let mut error = None;
            lowered.visit_mut(|sub| match lower_length(sub, frame, ctx) {
                Ok(l) => *sub = l,
                Err(e) => {
                    error.get_or_insert(e);
                }
            });
            error.map_or(Ok((lowered, None)), Err)
        }
    }
}

/// Like [`lower_expression`], but `expr` must be a length.
fn lower_length(expr: &Expression, frame: &ElementRc, ctx: &Context) -> Result<Expression, String> {
    match lower_expression(expr, frame, ctx)? {
        (lowered, None) => Ok(lowered),
        (_, Some(_)) => Err(NOT_A_LENGTH.into()),
    }
}

/// Lowers the anchors, position, and size of one axis of `elem`.
fn lower_axis(
    elem: &ElementRc,
    axis: Axis,
    set: &AxisSet,
    location: &SourceLocation,
    ctx: &mut Context,
) -> Option<[(&'static str, Expression, SourceLocation); 2]> {
    let Some(parent) = ctx.parent(elem) else {
        ctx.diag
            .push_error("Anchors can't be set on the root element of a component".into(), location);
        return None;
    };
    let start_name = anchor_name(axis, Edge::Start);
    if let Some((_, _, location)) = set.values.get(2) {
        ctx.diag.push_error(
            format!(
                "Set at most two of '{start_name}', '{}', '{}', and '{}'. '{}' counts as '{start_name}'",
                anchor_name(axis, Edge::Center),
                anchor_name(axis, Edge::End),
                size(axis),
                position(axis),
            ),
            location,
        );
        return None;
    }
    let mut start = None;
    let mut center = None;
    let mut end = None;
    let mut size_value = None;
    for (name, expr, location) in &set.values {
        if name == size(axis) {
            size_value = Some((expr.clone(), location.clone()));
            continue;
        }
        let (edge, lowered) = if name == position(axis) {
            (Edge::Start, expr.clone())
        } else {
            let (_, edge) = anchor_of(elem, name).unwrap();
            let message = match lower_expression(expr, &parent, ctx) {
                Ok((lowered, Some(a))) if a == axis => {
                    let mut reads_own_position = false;
                    lowered.visit_recursive(&mut |e| {
                        reads_own_position |= matches!(e, Expression::PropertyReference(nr)
                            if Rc::ptr_eq(&nr.element(), elem) && nr.name() == position(axis));
                    });
                    if reads_own_position {
                        Err("An element can't anchor to itself or to its children".into())
                    } else {
                        Ok(lowered)
                    }
                }
                Ok((_, Some(_))) => Err(format!(
                    "'{name}' is a {} anchor and can't be bound to a {} anchor",
                    axis_word(axis),
                    axis_word(axis.orthogonal())
                )),
                Ok((_, None)) => Err(format!(
                    "'{name}' must be bound to an anchor. Use 'parent.{} + <length>' to set a margin",
                    anchor_name(axis, edge)
                )),
                Err(message) => Err(message),
            };
            match message {
                Ok(lowered) => (edge, lowered),
                Err(message) => {
                    ctx.diag.push_error(message, location);
                    return None;
                }
            }
        };
        let slot = match edge {
            Edge::Start => &mut start,
            Edge::Center => &mut center,
            Edge::End => &mut end,
        };
        if slot.is_some() {
            ctx.diag.push_error(
                format!("'{start_name}' and '{}' both set the same edge", position(axis)),
                location,
            );
            return None;
        }
        *slot = Some((lowered, location.clone()));
    }

    let own_size = || property(elem, size(axis));
    let given_size = || {
        size_value
            .clone()
            .unwrap_or_else(|| (property(elem, preferred_size(axis)), location.clone()))
    };
    let (position, size) = match (start, center, end) {
        (Some(s), None, None) => (s, given_size()),
        (Some(s), None, Some((e, l))) => {
            let size = binary(e, '-', s.0.clone());
            (s, (size, l))
        }
        (Some(s), Some((c, l)), None) => {
            let size = binary(binary(c, '-', s.0.clone()), '*', number(2.));
            (s, (size, l))
        }
        (None, Some((c, l)), None) => {
            ((binary(c, '-', binary(own_size(), '/', number(2.))), l), given_size())
        }
        (None, Some((c, _)), Some((e, l))) => {
            let size = binary(binary(e.clone(), '-', c), '*', number(2.));
            ((binary(e, '-', own_size()), l.clone()), (size, l))
        }
        (None, None, Some((e, l))) => ((binary(e, '-', own_size()), l), given_size()),
        (None, None, None) => {
            let centered =
                binary(binary(property(&parent, size(axis)), '-', own_size()), '/', number(2.));
            ((centered, location.clone()), given_size())
        }
        (Some(_), Some(_), Some(_)) => unreachable!(),
    };
    Some([(self::position(axis), position.0, position.1), (self::size(axis), size.0, size.1)])
}

/// Lowers the anchors of the components of `doc`. `anchors` comes from [`remove_auto`].
pub fn lower_anchors(doc: &Document, anchors: Anchors, diag: &mut BuildDiagnostics) {
    let Anchors { anchored, released } = anchors;
    let mut parents = HashMap::new();
    let mut elements = Vec::new();
    for component in &doc.inner_components {
        recurse_elem(&component.root_element, &(), &mut |elem, _| {
            elements.push(elem.clone());
            for child in &elem.borrow().children {
                parents.insert(ByAddress(child.clone()), elem.clone());
            }
        });
    }
    let mut ctx = Context { parents, diag };

    let mut released_by_state: HashMap<(ByAddress<ElementRc>, usize), Vec<&Released>> =
        HashMap::new();
    for r in &released {
        released_by_state.entry((ByAddress(r.owner.clone()), r.state)).or_default().push(r);
    }

    let mut state_changes: HashMap<(ByAddress<ElementRc>, Axis), Vec<(ElementRc, usize)>> =
        HashMap::new();
    let mut add_state_change = |target: ElementRc, name: &str, owner: &ElementRc, state| {
        let Some(axis) = axis_of(&target, name) else { return };
        let key = (ByAddress(target), axis);
        let changes = state_changes.entry(key).or_default();
        if !changes.iter().any(|(o, s)| Rc::ptr_eq(o, owner) && *s == state) {
            changes.push((owner.clone(), state));
        }
    };
    for owner in &elements {
        for (index, state) in owner.borrow().states.iter().enumerate() {
            for (nr, ..) in &state.property_changes {
                add_state_change(nr.element(), nr.name(), owner, index);
            }
        }
    }
    for r in &released {
        add_state_change(r.target.clone(), &r.property, &r.owner, r.state);
    }

    for elem in &elements {
        for axis in AXES {
            let key = (ByAddress(elem.clone()), axis);
            if !anchored.contains(&key) {
                continue;
            }
            let mut base = AxisSet::default();
            let mut location = None;
            let mut has_error = false;
            for name in anchor_names(axis).chain([position(axis), size(axis)]) {
                let e = elem.borrow();
                let Some(binding) = e.binding(name).filter(|b| b.has_binding()) else {
                    continue;
                };
                let binding_location = binding.span.clone().unwrap_or_default();
                if anchor_of(elem, name).is_some() {
                    location.get_or_insert_with(|| binding_location.clone());
                    if !binding.two_way_bindings.is_empty() {
                        has_error = true;
                        ctx.diag.push_error(
                            "An anchor can't have a two-way binding".into(),
                            &binding_location,
                        );
                    }
                    if binding.animation.is_some() {
                        has_error = true;
                        ctx.diag.push_error(
                            format!(
                                "An anchor can't be animated. Animate '{}' and '{}' instead",
                                position(axis),
                                size(axis)
                            ),
                            &binding_location,
                        );
                    }
                }
                base.set(name.into(), binding.expression.clone(), binding_location);
            }
            for name in anchor_names(axis) {
                elem.borrow_mut().take_binding(name);
            }
            let location = location.unwrap_or_else(|| elem.borrow().to_source_location());
            let placed_by_parent = ctx.parent(elem).and_then(|parent| {
                if crate::layout::is_layout(&parent.borrow().base_type) {
                    Some("A layout positions its children, so they can't set anchors")
                } else if places_children_inside(&parent) {
                    Some("The parent places its children with '@children', so they can't set anchors")
                } else {
                    None
                }
            });
            if let Some(message) = placed_by_parent {
                ctx.diag.push_error(message.into(), &location);
                for (owner, state_index) in state_changes.remove(&key).unwrap_or_default() {
                    let mut states = std::mem::take(&mut owner.borrow_mut().states);
                    states[state_index].property_changes.retain(|(nr, ..)| {
                        !Rc::ptr_eq(&nr.element(), elem) || anchor_of(elem, nr.name()).is_none()
                    });
                    owner.borrow_mut().states = states;
                }
                continue;
            }

            if !has_error
                && base.has_anchor(elem)
                && let Some(lowered) = lower_axis(elem, axis, &base, &location, &mut ctx)
            {
                for (name, expr, location) in lowered {
                    if base.contains(name) {
                        continue;
                    }
                    // Keeps the animation of an `animate x` without a binding
                    if let Some(mut binding) = elem.borrow().binding_mut(name) {
                        binding.expression = expr;
                        binding.span = Some(location);
                        binding.priority = 1;
                        continue;
                    }
                    let mut binding = BindingExpression::new_with_span(expr, location);
                    binding.priority = 1;
                    binding.from_source = true;
                    elem.borrow_mut().set_binding(name.into(), binding);
                }
            }

            for (owner, state_index) in state_changes.remove(&key).unwrap_or_default() {
                let mut set = base.clone();
                let mut states = std::mem::take(&mut owner.borrow_mut().states);
                let state = &mut states[state_index];
                let is_axis_change = |nr: &NamedReference| {
                    Rc::ptr_eq(&nr.element(), elem) && axis_of(elem, nr.name()) == Some(axis)
                };
                let mut change = None;
                for (nr, expr, node) in &state.property_changes {
                    if is_axis_change(nr) {
                        let l = node.BindingExpression().to_source_location();
                        change.get_or_insert_with(|| (l.clone(), node.clone()));
                        set.set(nr.name().clone(), expr.clone(), l);
                    }
                }
                let state_released =
                    released_by_state.get(&(ByAddress(owner.clone()), state_index));
                for r in state_released.into_iter().flatten() {
                    if Rc::ptr_eq(&r.target, elem) && axis_of(elem, &r.property) == Some(axis) {
                        set.remove(&r.property);
                        change.get_or_insert_with(|| (location.clone(), r.node.clone()));
                    }
                }
                if !base.has_anchor(elem) && !set.has_anchor(elem) {
                    owner.borrow_mut().states = states;
                    continue;
                }
                state.property_changes.retain(|(nr, ..)| !is_axis_change(nr));
                if let Some((change_location, node)) = change
                    && let Some(lowered) = lower_axis(elem, axis, &set, &change_location, &mut ctx)
                {
                    for (name, expr, _) in lowered {
                        state.property_changes.push((
                            NamedReference::new(elem, SmolStr::new_static(name)),
                            expr,
                            node.clone(),
                        ));
                    }
                }
                owner.borrow_mut().states = states;
            }
        }
    }

    // Anchors read outside of anchor bindings
    for elem in &elements {
        let element_location = elem.borrow().to_source_location();
        let changed_anchor = elem
            .borrow()
            .change_callbacks
            .keys()
            .find_map(|name| anchor_of(elem, name).map(|(axis, _)| axis));
        if let Some(axis) = changed_anchor {
            ctx.diag.push_error(
                format!(
                    "An anchor has no 'changed' callback. Use 'changed {}' or 'changed {}' instead",
                    position(axis),
                    size(axis)
                ),
                &element_location,
            );
        }
        let lower = |expr: &mut Expression, location: &SourceLocation, ctx: &mut Context| {
            let Some(frame) = distance_frame(expr, ctx) else { return };
            match lower_length(expr, &frame, ctx) {
                Ok(lowered) => *expr = lowered,
                Err(e) => {
                    ctx.diag.push_error(e, location);
                    *expr = Expression::Invalid;
                }
            }
        };
        for (_, binding) in elem.borrow().real_bindings() {
            let mut binding = binding.borrow_mut();
            let location = binding.span.clone().unwrap_or_else(|| element_location.clone());
            lower(&mut binding.expression, &location, &mut ctx);
        }
        visit_element_expressions(elem, |expr, _, _| lower(expr, &element_location, &mut ctx));
    }
}
