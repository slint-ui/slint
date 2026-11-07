// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Sets [`BindingExpression::can_outlive_sub_component`](crate::llr::BindingExpression).

use crate::llr::{
    CompilationUnit, EvaluationContext, LocalMemberIndex, MemberReference, SubComponentIdx,
};
use std::collections::{HashMap, HashSet};

type Member = (SubComponentIdx, LocalMemberIndex);

/// The sub component that holds `reference`, or `None` for a global.
fn resolve(reference: &MemberReference, ctx: &EvaluationContext) -> Option<Member> {
    let MemberReference::Relative { parent_level, local_reference } = reference else {
        return None;
    };
    let mut sub_component = ctx.parent_sub_component_idx(*parent_level)?;
    for instance in &local_reference.sub_component_path {
        sub_component =
            ctx.compilation_unit.sub_components[sub_component].sub_components[*instance].ty;
    }
    Some((sub_component, local_reference.reference.clone()))
}

pub fn mark_outliving_bindings(root: &CompilationUnit) {
    let mut links = HashMap::<Member, Vec<Member>>::new();
    let mut pending = Vec::new();
    let in_parent = |reference: &MemberReference| matches!(reference, MemberReference::Relative { parent_level, .. } if *parent_level > 0);
    root.for_each_sub_components(&mut |_, sub_component, ctx| {
        for link in sub_component.two_way_bindings.iter().filter(|link| link.is_model.is_none()) {
            for (end, other) in [(&link.prop1, &link.prop2), (&link.prop2, &link.prop1)] {
                if in_parent(other)
                    && let Some(member) = resolve(end, ctx)
                {
                    pending.push(member);
                }
            }
            match (resolve(&link.prop1, ctx), resolve(&link.prop2, ctx)) {
                (Some(a), Some(b)) => {
                    links.entry(a.clone()).or_default().push(b.clone());
                    links.entry(b).or_default().push(a);
                }
                (Some(member), None) | (None, Some(member)) => pending.push(member),
                (None, None) => {}
            }
        }
    });

    let mut linked_to_global = HashSet::new();
    while let Some(member) = pending.pop() {
        if linked_to_global.insert(member.clone())
            && let Some(others) = links.get(&member)
        {
            pending.extend(others.iter().cloned());
        }
    }

    root.for_each_sub_components(&mut |_, sub_component, ctx| {
        for (property, binding) in &sub_component.property_init {
            let outlives = matches!(property, MemberReference::Global { .. })
                || resolve(property, ctx).is_some_and(|m| linked_to_global.contains(&m));
            binding.can_outlive_sub_component.set(outlives);
        }
    });
}
