// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A spring's `bounce` approaching 1 never reaches its target, so it is
//! already an infinite animation regardless of `iteration-count`. Requiring
//! `iteration-count: -1` in that case keeps "this animation runs forever" expressible in
//! exactly one way.

use crate::diagnostics::BuildDiagnostics;
use crate::expression_tree::{EasingCurve, Expression, Unit};
use crate::object_tree::{Document, ElementRc, PropertyAnimation, recurse_elem};

/// Above this `bounce` value a spring either takes too long to settle or never settles.
const MAX_SETTLING_BOUNCE: f32 = 0.95;

pub fn check_spring_animation(doc: &Document, diag: &mut BuildDiagnostics) {
    for component in &doc.inner_components {
        recurse_elem(&component.root_element, &(), &mut |elem, _| {
            let elem = elem.borrow();
            for (_, binding) in elem.real_bindings() {
                match &binding.borrow().animation {
                    Some(PropertyAnimation::Static(anim)) => check_anim_element(anim, diag),
                    Some(PropertyAnimation::Transition { animations, .. }) => {
                        for a in animations {
                            check_anim_element(&a.animation, diag);
                        }
                    }
                    None => {}
                }
            }
            for t in &elem.transitions {
                for (_, _, anim) in &t.property_animations {
                    check_anim_element(anim, diag);
                }
            }
        });
    }
}

fn check_anim_element(anim: &ElementRc, diag: &mut BuildDiagnostics) {
    let anim = anim.borrow();
    let Some(easing) = anim.binding_cell_including_synthetic("easing") else { return };
    let Expression::EasingCurve(EasingCurve::Spring(bounce)) = &easing.borrow().expression else {
        return;
    };
    if *bounce <= MAX_SETTLING_BOUNCE {
        return;
    }
    let Some(iteration_count) = anim.binding_cell_including_synthetic("iteration-count") else {
        return;
    };
    let Expression::NumberLiteral(count, Unit::None) = &iteration_count.borrow().expression else {
        return;
    };
    if *count != -1.0 {
        diag.push_error(
            format!(
                "A spring with a bounce greater than {MAX_SETTLING_BOUNCE} never settles; \
                 set `iteration-count: -1` to make the animation explicitly infinite"
            ),
            &easing.borrow().span,
        );
    }
}
