// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Rc;

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{BuiltinFunction, Callable, EasingCurve, Expression};
use crate::object_tree::{
    Component, ElementRc, PropertyAnimation, recurse_elem_including_sub_components,
    visit_all_expressions,
};

/// Check the validity of expressions
///
/// - Check that the GetWindowScaleFactor and GetWindowDefaultFontSize are not called in a global
pub fn check_expressions(doc: &crate::object_tree::Document, diag: &mut BuildDiagnostics) {
    for component in &doc.inner_components {
        visit_all_expressions(component, |e, _| check_expression(component, e, diag));
        recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
            for binding in elem.borrow().bindings.values() {
                if let Some(anim) = &binding.borrow().animation {
                    check_animation(anim, diag);
                }
            }
        });
    }
}

/// Check that a spring animation doesn't mix the `bounce`/`duration` parametrization
/// with the `mass`/`stiffness`/`damping` one, since only one of the two is used at runtime.
fn check_animation(anim: &PropertyAnimation, diag: &mut BuildDiagnostics) {
    match anim {
        PropertyAnimation::Static(e) => check_spring_animation_fields(e, diag),
        PropertyAnimation::Transition { animations, .. } => {
            for a in animations {
                check_spring_animation_fields(&a.animation, diag);
            }
        }
    }
}

fn check_spring_animation_fields(anim_element: &ElementRc, diag: &mut BuildDiagnostics) {
    let anim_element = anim_element.borrow();

    enum EasingKind {
        // no `easing` binding at all (implicit Linear)
        Missing,
        Spring,
        NonSpring,
        // not an easing
        Unknown,
    }

    let easing_kind = match anim_element.bindings.get("easing") {
        None => EasingKind::Missing,
        Some(curve) => match &curve.borrow().expression {
            Expression::EasingCurve(EasingCurve::Spring) => EasingKind::Spring,
            Expression::EasingCurve(_) => EasingKind::NonSpring,
            _ => EasingKind::Unknown,
        },
    };

    if matches!(easing_kind, EasingKind::Unknown) {
        return;
    }

    let has = |name: &str| anim_element.bindings.contains_key(name);
    let span_for = |name: &str| {
        anim_element.bindings.get(name).and_then(|b| b.borrow().span.clone()).unwrap_or_default()
    };

    if !matches!(easing_kind, EasingKind::Spring) {
        let mut check_binding = |name: &str| {
            if has(name) {
                diag.push_error_with_span(
                    format!("Cannot have '{name}' with a non Spring easing curve").into(),
                    span_for(name),
                );
            }
        };
        check_binding("bounce");
        check_binding("mass");
        check_binding("stiffness");
        check_binding("damping");
        return;
    }

    // only reach here for an explicit `easing: spring;`
    let span = span_for("easing");
    let duration_bounce_set = has("bounce") || has("duration");
    let physical_set = has("mass") || has("stiffness") || has("damping");
    if duration_bounce_set && physical_set {
        diag.push_error_with_span(
            "Cannot mix 'bounce'/'duration' with 'mass'/'stiffness'/'damping' in a spring animation"
                .into(),
            span,
        );
    } else if physical_set && !(has("mass") && has("stiffness") && has("damping")) {
        diag.push_error_with_span(
            "'mass', 'stiffness' and 'damping' must all be set together in a spring animation"
                .into(),
            span,
        );
    } else if duration_bounce_set && !has("duration") {
        diag.push_error_with_span("A spring easing must have a duration".into(), span);
    }
}

fn check_expression(component: &Rc<Component>, e: &Expression, diag: &mut BuildDiagnostics) {
    if let Expression::FunctionCall { function: Callable::Builtin(b), source_location, .. } = e {
        match b {
            BuiltinFunction::GetWindowScaleFactor if component.is_global() => {
                diag.push_error("Cannot convert between logical and physical length in a global component, because the scale factor is not known".into(), source_location);
            }
            BuiltinFunction::GetWindowDefaultFontSize if component.is_global() => {
                diag.push_error("Cannot convert between rem and logical length in a global component, because the default font size is not known".into(), source_location);
            }
            _ => {}
        }
    }
    e.visit(|e| check_expression(component, e, diag))
}
