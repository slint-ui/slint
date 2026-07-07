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
use i_slint_core::rtti;
use i_slint_core::{Callback, Property};
use std::pin::Pin;
use std::rc::Rc;
use typed_index_collections::TiVec;

/// Name-based access to a native (builtin) global like `NativeStyleMetrics`,
/// whose state lives in a backend-provided struct instead of interpreter
/// `Property<Value>` slots.
pub trait NativeGlobal {
    fn get_property(self: Pin<&Self>, name: &str) -> Option<Value>;
    fn set_property(
        self: Pin<&Self>,
        name: &str,
        value: Value,
        animation: Option<i_slint_core::items::PropertyAnimation>,
    ) -> Result<(), ()>;
    fn invoke_callback(self: Pin<&Self>, name: &str, args: &[Value]) -> Option<Value>;
    fn set_callback_handler(
        self: Pin<&Self>,
        name: &str,
        handler: Box<dyn Fn(&[Value]) -> Value>,
    ) -> Result<(), ()>;
    fn prepare_property_for_two_way_binding(
        self: Pin<&Self>,
        name: &str,
    ) -> Option<Pin<Rc<Property<Value>>>>;
}

impl<T: rtti::BuiltinGlobal + 'static> NativeGlobal for T {
    fn get_property(self: Pin<&Self>, name: &str) -> Option<Value> {
        let (_, prop) = T::properties::<Value>().into_iter().find(|(k, _)| *k == name)?;
        prop.get(self).ok()
    }

    fn set_property(
        self: Pin<&Self>,
        name: &str,
        value: Value,
        animation: Option<i_slint_core::items::PropertyAnimation>,
    ) -> Result<(), ()> {
        let (_, prop) = T::properties::<Value>().into_iter().find(|(k, _)| *k == name).ok_or(())?;
        prop.set(self, value, animation)
    }

    fn invoke_callback(self: Pin<&Self>, name: &str, args: &[Value]) -> Option<Value> {
        let (_, cb) = T::callbacks::<Value>().into_iter().find(|(k, _)| *k == name)?;
        cb.call(self, args).ok()
    }

    fn set_callback_handler(
        self: Pin<&Self>,
        name: &str,
        handler: Box<dyn Fn(&[Value]) -> Value>,
    ) -> Result<(), ()> {
        let (_, cb) = T::callbacks::<Value>().into_iter().find(|(k, _)| *k == name).ok_or(())?;
        cb.set_handler(self, handler)
    }

    fn prepare_property_for_two_way_binding(
        self: Pin<&Self>,
        name: &str,
    ) -> Option<Pin<Rc<Property<Value>>>> {
        let (_, prop) = T::properties::<Value>().into_iter().find(|(k, _)| *k == name)?;
        Some(prop.prepare_for_two_way_binding(self))
    }
}

/// Instantiate the backend-provided global with the given class name.
/// `None` when the selected backend has no native global of that name.
fn instantiate_native_global(class_name: &str) -> Option<Pin<Rc<dyn NativeGlobal>>> {
    trait Helper {
        fn instantiate(_name: &str) -> Option<Pin<Rc<dyn NativeGlobal>>> {
            None
        }
    }
    impl Helper for () {}
    impl<T: rtti::BuiltinGlobal + 'static, Next: Helper> Helper for (T, Next) {
        fn instantiate(name: &str) -> Option<Pin<Rc<dyn NativeGlobal>>> {
            if name == T::name() { Some(T::new()) } else { Next::instantiate(name) }
        }
    }
    <i_slint_backend_selector::NativeGlobals as Helper>::instantiate(class_name)
}

pub struct GlobalInstance {
    pub compilation_unit: Rc<CompilationUnit>,
    pub global_idx: GlobalIdx,
    pub properties: TiVec<i_slint_compiler::llr::PropertyIdx, SubComponentProperty>,
    pub callbacks: TiVec<i_slint_compiler::llr::CallbackIdx, SubComponentCallback>,
    /// `Property<()>` per callback with `needs_tracker`; see
    /// `SubComponentInstance::callback_trackers`.
    pub callback_trackers: TiVec<
        i_slint_compiler::llr::CallbackIdx,
        Option<std::pin::Pin<Rc<i_slint_core::properties::Property<()>>>>,
    >,
    /// `ChangeTracker`s installed for the global's `changed X => { … }`
    /// handlers. Stored here so they stay alive with the global instance.
    pub change_trackers: std::cell::RefCell<Vec<i_slint_core::properties::ChangeTracker>>,
    /// Backend-provided state for a builtin global (`NativeStyleMetrics`,
    /// `NativePalette`, …); access goes by member name through the rtti
    /// tables. The interpreter-level slots above stay empty.
    pub native: Option<Pin<Rc<dyn NativeGlobal>>>,
}

/// All globals for one root component.
pub struct GlobalStorage {
    globals: TiVec<GlobalIdx, Option<Rc<GlobalInstance>>>,
    /// The owning instance, so global bindings can reach the window.
    pub root: std::cell::OnceCell<
        vtable::VWeak<i_slint_core::item_tree::ItemTreeVTable, crate::instance::Instance>,
    >,
    /// Set through [`ComponentInstance::set_debug_hook_callback`]; see [`crate::debug_hook`].
    pub debug_hook_callback: std::cell::RefCell<Option<crate::debug_hook::DebugHookCallback>>,
}

impl GlobalStorage {
    /// Allocate one `GlobalInstance` per declared global.
    /// Bindings are installed separately by [`install_global_bindings`].
    pub fn new(compilation_unit: &Rc<CompilationUnit>) -> Self {
        let globals = compilation_unit
            .globals
            .iter_enumerated()
            .map(|(idx, global)| {
                if global.is_builtin {
                    // Backends without a matching native global leave the
                    // slot empty and reads produce `Value::Void`.
                    let native = instantiate_native_global(&global.name)?;
                    return Some(Rc::new(GlobalInstance {
                        compilation_unit: compilation_unit.clone(),
                        global_idx: idx,
                        properties: TiVec::new(),
                        callbacks: TiVec::new(),
                        callback_trackers: TiVec::new(),
                        change_trackers: std::cell::RefCell::new(Vec::new()),
                        native: Some(native),
                    }));
                }
                let properties = global
                    .properties
                    .iter()
                    .map(|p| Rc::pin(Property::new(crate::eval::default_value_for_type(&p.ty))))
                    .collect();
                let callbacks =
                    global.callbacks.iter().map(|_| Rc::pin(Callback::default())).collect();
                let callback_trackers = global
                    .callbacks
                    .iter()
                    .map(|c| {
                        c.needs_tracker
                            .then(|| Rc::pin(i_slint_core::properties::Property::new(())))
                    })
                    .collect();
                Some(Rc::new(GlobalInstance {
                    compilation_unit: compilation_unit.clone(),
                    global_idx: idx,
                    properties,
                    callbacks,
                    callback_trackers,
                    change_trackers: std::cell::RefCell::new(Vec::new()),
                    native: None,
                }))
            })
            .collect();
        Self {
            globals,
            root: std::cell::OnceCell::new(),
            debug_hook_callback: std::cell::RefCell::new(None),
        }
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
            if !global.exported {
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

/// Install every global's `init_values`, then in a second pass the change
/// trackers for `changed X => { … }` handlers, so trackers observe the
/// fully initialized values (matching the sub-component ordering).
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
            {
                let cu = cu.clone();
                move |(), _| {
                    let Some(st) = weak_storage_set.upgrade() else { return };
                    let mut ctx = EvalContext::for_global(Rc::downgrade(&st), cu.clone());
                    eval_expression(&mut ctx, &notify_expr);
                }
            },
        );
        trackers.push(tracker);
    }
}

fn install_for_global(g: &Rc<GlobalInstance>, storage: &Rc<GlobalStorage>) {
    if g.native.is_some() {
        // Native globals carry their own state; there are no interpreted
        // init values to install.
        return;
    }
    let cu = g.compilation_unit.clone();
    let global = &cu.globals[g.global_idx];
    for (member, binding) in &global.init_values {
        let expr = binding.expression.borrow().clone();
        let weak_storage = Rc::downgrade(storage);

        match member {
            LocalMemberIndex::Property(idx) => {
                let prop = Pin::as_ref(&g.properties[*idx]);
                if binding.kind == i_slint_compiler::llr::BindingKind::Constant {
                    let mut ctx = EvalContext::for_global(weak_storage.clone(), cu.clone());
                    prop.set(eval_expression(&mut ctx, &expr));
                    continue;
                }
                let expr = expr.clone();
                let cu = cu.clone();
                prop.set_binding(move || {
                    let mut ctx = EvalContext::for_global(weak_storage.clone(), cu.clone());
                    eval_expression(&mut ctx, &expr)
                });
            }
            LocalMemberIndex::Callback(idx) => {
                let cb = Pin::as_ref(&g.callbacks[*idx]);
                let expr = expr.clone();
                let cu = cu.clone();
                let arg_types = global.callbacks[*idx].args.clone();
                cb.set_handler(move |args: &[Value]| -> Value {
                    let mut ctx = EvalContext::for_global(weak_storage.clone(), cu.clone());
                    ctx.function_arg_types = arg_types.clone();
                    ctx.function_arguments = args.to_vec();
                    eval_expression(&mut ctx, &expr)
                });
            }
            LocalMemberIndex::Function(_)
            | LocalMemberIndex::Native { .. }
            | LocalMemberIndex::Timer(_) => {
                // Function bodies live on `GlobalComponent::functions[*].code`.
                // Natives and timers don't appear on globals.
            }
        }
    }
}
