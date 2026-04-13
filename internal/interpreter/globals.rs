// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Storage for `global <Name> { … }` runtime instances.
//!
//! Created once per root instance and shared across the sub-component tree.
//! Globals carry properties, callbacks and functions, but no items or children.

use crate::Value;
use crate::erased::{SubComponentCallback, SubComponentProperty};
use crate::eval::{EvalContext, eval_expression};
use i_slint_compiler::llr::{CompilationUnit, GlobalIdx, LocalMemberIndex};
use i_slint_core::{Callback, Property};
use std::pin::Pin;
use std::rc::Rc;
use typed_index_collections::TiVec;

pub struct GlobalInstance {
    pub compilation_unit: Rc<CompilationUnit>,
    pub global_idx: GlobalIdx,
    pub properties: TiVec<i_slint_compiler::llr::PropertyIdx, SubComponentProperty>,
    pub callbacks: TiVec<i_slint_compiler::llr::CallbackIdx, SubComponentCallback>,
    /// `ChangeTracker`s installed for the global's `changed X => { … }`
    /// handlers. Stored here so they stay alive with the global instance.
    pub change_trackers: std::cell::RefCell<Vec<i_slint_core::properties::ChangeTracker>>,
}

/// All globals for one root component.
pub struct GlobalStorage {
    globals: TiVec<GlobalIdx, Option<Rc<GlobalInstance>>>,
}

impl GlobalStorage {
    /// Allocate one `GlobalInstance` per declared global.
    /// Bindings are installed lazily by [`install_global_bindings`].
    pub fn new(compilation_unit: &Rc<CompilationUnit>) -> Self {
        let globals = compilation_unit
            .globals
            .iter_enumerated()
            .map(|(idx, global)| {
                if global.is_builtin {
                    // Builtin globals (e.g. `AccessKit`) need runtime-managed
                    // backing state, not interpreter-level `Property<Value>`.
                    // The Rust codegen generates dedicated struct fields for
                    // these. Until the interpreter has type-aware global
                    // instances, skip them — their properties are rarely
                    // accessed directly and the test suite passes without them.
                    return None;
                }
                let properties = global
                    .properties
                    .iter()
                    .map(|p| Rc::pin(Property::new(crate::eval::default_value_for_type(&p.ty))))
                    .collect();
                let callbacks =
                    global.callbacks.iter().map(|_| Rc::pin(Callback::default())).collect();
                Some(Rc::new(GlobalInstance {
                    compilation_unit: compilation_unit.clone(),
                    global_idx: idx,
                    properties,
                    callbacks,
                    change_trackers: std::cell::RefCell::new(Vec::new()),
                }))
            })
            .collect();
        Self { globals }
    }

    pub fn get(&self, idx: GlobalIdx) -> Option<&Rc<GlobalInstance>> {
        self.globals.get(idx)?.as_ref()
    }

    /// Look up a non-builtin global by its exported name (or alias).
    /// Returns the matching `GlobalComponent` and its runtime `GlobalInstance`.
    pub fn find_by_name<'a>(
        &'a self,
        compilation_unit: &'a CompilationUnit,
        name: &str,
    ) -> Option<(&'a i_slint_compiler::llr::GlobalComponent, &'a Rc<GlobalInstance>)> {
        let needle = i_slint_compiler::parser::normalize_identifier(name);
        for (idx, global) in compilation_unit.globals.iter_enumerated() {
            if !global.exported || global.is_builtin {
                continue;
            }
            let name_matches =
                i_slint_compiler::parser::normalize_identifier(&global.name) == needle;
            let alias_matches = global
                .aliases
                .iter()
                .any(|a| i_slint_compiler::parser::normalize_identifier(a) == needle);
            if name_matches || alias_matches {
                return self.get(idx).map(|inst| (global, inst));
            }
        }
        None
    }
}

/// Walk every global in `storage` and install its `init_values`. Change
/// trackers for `changed X => { … }` handlers are installed in a second
/// pass (see `install_global_change_trackers`) after all init values have
/// been applied, matching the sub-component ordering.
pub fn install_global_bindings(storage: &Rc<GlobalStorage>) {
    for g in storage.globals.iter().flatten() {
        install_for_global(g, storage);
    }
    for g in storage.globals.iter().flatten() {
        install_global_change_trackers(g, storage);
    }
}

fn install_global_change_trackers(g: &Rc<GlobalInstance>, storage: &Rc<GlobalStorage>) {
    let cu = g.compilation_unit.clone();
    let global = &cu.globals[g.global_idx];
    let mut trackers = g.change_trackers.borrow_mut();
    for (prop_idx, expr) in &global.change_callbacks {
        let tracker = i_slint_core::properties::ChangeTracker::default();
        let weak_storage_get = Rc::downgrade(storage);
        let weak_storage_set = Rc::downgrade(storage);
        let global_idx = g.global_idx;
        let prop_idx = *prop_idx;
        let notify_expr = expr.borrow().clone();
        tracker.init(
            (),
            move |()| -> Value {
                let Some(st) = weak_storage_get.upgrade() else { return Value::Void };
                let Some(gi) = st.get(global_idx) else { return Value::Void };
                Pin::as_ref(&gi.properties[prop_idx]).get()
            },
            move |(), _| {
                let Some(st) = weak_storage_set.upgrade() else { return };
                let mut ctx = EvalContext::for_global(Rc::downgrade(&st));
                eval_expression(&mut ctx, &notify_expr);
            },
        );
        trackers.push(tracker);
    }
}

fn install_for_global(g: &Rc<GlobalInstance>, storage: &Rc<GlobalStorage>) {
    let cu = g.compilation_unit.clone();
    let global = &cu.globals[g.global_idx];
    for (member, binding) in &global.init_values {
        let expr = binding.expression.borrow().clone();
        let weak_storage = Rc::downgrade(storage);

        match member {
            LocalMemberIndex::Property(idx) => {
                let prop = Pin::as_ref(&g.properties[*idx]);
                if binding.kind == i_slint_compiler::llr::BindingKind::Constant {
                    let mut ctx = EvalContext::for_global(weak_storage.clone());
                    prop.set(eval_expression(&mut ctx, &expr));
                    continue;
                }
                let expr = expr.clone();
                prop.set_binding(move || {
                    let mut ctx = EvalContext::for_global(weak_storage.clone());
                    eval_expression(&mut ctx, &expr)
                });
            }
            LocalMemberIndex::Callback(idx) => {
                let cb = Pin::as_ref(&g.callbacks[*idx]);
                let expr = expr.clone();
                cb.set_handler(move |(args,): &(Vec<Value>,)| -> Value {
                    let mut ctx = EvalContext::for_global(weak_storage.clone());
                    ctx.function_arguments = args.clone();
                    eval_expression(&mut ctx, &expr)
                });
            }
            LocalMemberIndex::Function(_) | LocalMemberIndex::Native { .. } => {
                // Function bodies live on `GlobalComponent::functions[*].code`.
                // Natives don't appear on globals.
            }
        }
    }
}
