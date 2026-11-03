// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore qualname

use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransitionDirection {
    In,
    Out,
    InOut,
}

#[derive(Debug, Clone)]
pub struct TransitionPropertyAnimation {
    /// The state id as computed in lower_state
    pub state_id: i32,
    /// The direction of the transition
    pub direction: TransitionDirection,
    /// The content of the `animation` object
    pub animation: ElementRc,
}

impl TransitionPropertyAnimation {
    /// Return an expression which returns a boolean which is true if the transition is active.
    /// The state argument is an expression referencing the state property of type StateInfo
    pub fn condition(&self, state: Expression) -> Expression {
        match self.direction {
            TransitionDirection::In => Expression::BinaryExpression {
                lhs: Box::new(Expression::StructFieldAccess {
                    base: Box::new(state),
                    name: "current-state".into(),
                }),
                rhs: Box::new(Expression::NumberLiteral(self.state_id as _, Unit::None)),
                op: '=',
                source_location: None,
            },
            TransitionDirection::Out => Expression::BinaryExpression {
                lhs: Box::new(Expression::StructFieldAccess {
                    base: Box::new(state),
                    name: "previous-state".into(),
                }),
                rhs: Box::new(Expression::NumberLiteral(self.state_id as _, Unit::None)),
                op: '=',
                source_location: None,
            },
            TransitionDirection::InOut => Expression::BinaryExpression {
                lhs: Box::new(Expression::BinaryExpression {
                    source_location: None,
                    lhs: Box::new(Expression::StructFieldAccess {
                        base: Box::new(state.clone()),
                        name: "current-state".into(),
                    }),
                    rhs: Box::new(Expression::NumberLiteral(self.state_id as _, Unit::None)),
                    op: '=',
                }),
                rhs: Box::new(Expression::BinaryExpression {
                    source_location: None,
                    lhs: Box::new(Expression::StructFieldAccess {
                        base: Box::new(state),
                        name: "previous-state".into(),
                    }),
                    rhs: Box::new(Expression::NumberLiteral(self.state_id as _, Unit::None)),
                    op: '=',
                }),
                op: '|',
                source_location: None,
            },
        }
    }
}

/// Return a NamedReference for a qualified name used in a state (or transition),
/// if the reference is invalid, there will be a diagnostic
fn lookup_property_from_qualified_name_for_state(
    node: syntax_nodes::QualifiedName,
    r: &ElementRc,
    diag: &mut BuildDiagnostics,
) -> Option<(NamedReference, Type)> {
    let qualname = QualifiedTypeName::from_node(node.clone());
    let check = |lookup: &PropertyLookupResult<'_>, diag: &mut BuildDiagnostics| {
        #[cfg(feature = "slint-sc")]
        lookup.check_slint_sc(&qualname, &node, diag);
        if !lookup.property_type.is_property_type() {
            diag.push_error(format!("'{qualname}' is not a valid property"), &node);
        } else if !lookup.is_valid_for_assignment() {
            diag.push_error(
                format!(
                    "'{}' cannot be set in a state because it is '{}'",
                    qualname, lookup.property_visibility
                ),
                &node,
            );
        }
    };
    match qualname.members.as_slice() {
        [unresolved_prop_name] => {
            let lookup_result = r
                .borrow()
                .lookup_property(unresolved_prop_name.as_ref(), PropertyLookupMode::ComponentLocal);
            check(&lookup_result, diag);
            Some((
                NamedReference::new(r, lookup_result.internal_or_resolved_name()),
                lookup_result.property_type,
            ))
        }
        [elem_id, unresolved_prop_name] => {
            if let Some(element) = find_element_by_id(r, elem_id.as_ref()) {
                let lookup_result = element.borrow().lookup_property(
                    unresolved_prop_name.as_ref(),
                    PropertyLookupMode::ComponentLocal,
                );
                if !lookup_result.is_valid() {
                    diag.push_error(
                        format!("'{unresolved_prop_name}' not found in '{elem_id}'"),
                        &node,
                    );
                } else {
                    check(&lookup_result, diag);
                }
                Some((
                    NamedReference::new(&element, lookup_result.internal_or_resolved_name()),
                    lookup_result.property_type,
                ))
            } else {
                diag.push_error(format!("'{elem_id}' is not a valid element id"), &node);
                None
            }
        }
        _ => {
            diag.push_error(format!("'{qualname}' is not a valid property"), &node);
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct State {
    pub id: SmolStr,
    pub condition: Option<Expression>,
    pub property_changes: Vec<(NamedReference, Expression, syntax_nodes::StatePropertyChange)>,
    /// Where the source writes this state's selection. `None` for a state
    /// without a condition, which is never selected.
    pub selection: Option<ConditionLocation>,
}

#[derive(Debug, Clone)]
pub struct Transition {
    pub direction: TransitionDirection,
    pub state_id: SmolStr,
    pub property_animations: Vec<(NamedReference, SourceLocation, ElementRc)>,
    pub catch_all_property_animation: Option<(SourceLocation, ElementRc)>,
    pub node: syntax_nodes::Transition,
}

impl Transition {
    fn from_node(
        trs: syntax_nodes::Transition,
        r: &ElementRc,
        tr: &TypeRegister,
        diag: &mut BuildDiagnostics,
    ) -> Transition {
        let direction_text = trs
            .first_child_or_token()
            .and_then(|t| t.as_token().map(|tok| tok.text().to_string()))
            .unwrap_or_default();

        let mut property_animations = Vec::new();
        let mut catch_all_property_animation: Option<(SourceLocation, _)> = None;
        for pa in trs.PropertyAnimation() {
            if let Some(star) = pa.child_token(SyntaxKind::Star) {
                let star = star.to_source_location();
                if let Some((first, _)) = &catch_all_property_animation {
                    push_duplicate_catch_all_error(star, first.clone(), diag);
                } else {
                    catch_all_property_animation =
                        Some((star, catch_all_animation_element_from_node(&pa, diag, tr)));
                }
                continue;
            }
            for qn in pa.QualifiedName() {
                if let Some((ne, prop_type)) =
                    lookup_property_from_qualified_name_for_state(qn.clone(), r, diag)
                    && let Some(anim_element) =
                        animation_element_from_node(&pa, &qn, prop_type, diag, tr)
                {
                    property_animations.push((ne, qn.to_source_location(), anim_element));
                }
            }
        }

        Transition {
            direction: match direction_text.as_str() {
                "in" => TransitionDirection::In,
                "out" => TransitionDirection::Out,
                "in-out" => TransitionDirection::InOut,
                "in_out" => TransitionDirection::InOut,
                _ => {
                    unreachable!("Unknown transition direction: '{}'", direction_text);
                }
            },
            state_id: trs
                .DeclaredIdentifier()
                .and_then(|x| parser::identifier_text(&x))
                .unwrap_or_default(),
            property_animations,
            catch_all_property_animation,
            node: trs.clone(),
        }
    }
}

fn validate_transition_directions(transitions: &[Transition], diag: &mut BuildDiagnostics) {
    let mut seen_catch_all = HashMap::<&SmolStr, [Option<&SourceLocation>; 2]>::new();
    for t in transitions {
        let Some((span, _)) = &t.catch_all_property_animation else { continue };
        let claimed = seen_catch_all.entry(&t.state_id).or_default();
        let directions: &[usize] = match t.direction {
            TransitionDirection::In => &[0],
            TransitionDirection::Out => &[1],
            TransitionDirection::InOut => &[0, 1],
        };
        if let Some(first) = directions.iter().find_map(|&d| claimed[d]) {
            push_duplicate_catch_all_error(span.clone(), first.clone(), diag);
        } else {
            for &d in directions {
                claimed[d] = Some(span);
            }
        }
    }
}

fn push_duplicate_catch_all_error(
    span: SourceLocation,
    first: SourceLocation,
    diag: &mut BuildDiagnostics,
) {
    diag.push_error_with_span(
        "Only one 'animate *' is allowed per state and direction".into(),
        span,
    );
    diag.push_note_with_span("The first 'animate *' is here".into(), first);
}

impl Element {
    pub(super) fn apply_states_and_transitions(
        node: &syntax_nodes::Element,
        r: &ElementRc,
        is_legacy_syntax: bool,
        diag: &mut BuildDiagnostics,
        tr: &TypeRegister,
    ) {
        for state in node.States().flat_map(|s| s.State()) {
            let condition = state.Expression();
            // `when` is a contextual keyword, so it is the state's only
            // `Identifier` token: its name is a `DeclaredIdentifier`.
            let when = state.child_token(SyntaxKind::Identifier).filter(|t| t.text() == "when");
            // Without a condition a state is never selected, so its property
            // changes are code that can't run.
            #[cfg(feature = "slint-sc")]
            if condition.is_none() {
                diag.slint_sc_error(
                    "A state without a 'when' condition is",
                    &state.DeclaredIdentifier(),
                );
            }
            let s = State {
                id: parser::identifier_text(&state.DeclaredIdentifier()).unwrap_or_default(),
                condition: condition.map(|e| Expression::Uncompiled(e.into())),
                property_changes: state
                    .StatePropertyChange()
                    .filter_map(|s| {
                        lookup_property_from_qualified_name_for_state(s.QualifiedName(), r, diag)
                            .map(|(ne, _)| {
                                (ne, Expression::Uncompiled(s.BindingExpression().into()), s)
                            })
                    })
                    .collect(),
                selection: when.map(|when| ConditionLocation::StateSelection {
                    name: state.DeclaredIdentifier().to_source_location(),
                    when: when.to_source_location(),
                }),
            };
            for trs in state.Transition() {
                #[cfg(feature = "slint-sc")]
                diag.slint_sc_error("Transitions are", &trs);
                let mut t = Transition::from_node(trs, r, tr, diag);
                t.state_id.clone_from(&s.id);
                r.borrow_mut().transitions.push(t);
            }
            r.borrow_mut().states.push(s);
        }

        for ts in node.Transitions() {
            #[cfg(feature = "slint-sc")]
            diag.slint_sc_error("Transitions are", &ts);
            if !is_legacy_syntax {
                diag.push_error("'transitions' block are no longer supported. Use 'in {...}' and 'out {...}' directly in the state definition".into(), &ts);
            }
            for trs in ts.Transition() {
                let trans = Transition::from_node(trs, r, tr, diag);
                r.borrow_mut().transitions.push(trans);
            }
        }

        validate_transition_directions(&r.borrow().transitions, diag);
    }
}
