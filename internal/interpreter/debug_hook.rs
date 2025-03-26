// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{eval::ComponentInstance, Value};

use smol_str::SmolStr;

pub type DebugHookCallback = Box<dyn Fn(&str, usize, &crate::Value) -> Value>;

thread_local! { static DEBUG_HOOK_CALLBACK: std::cell::RefCell<Option<DebugHookCallback>> = Default::default(); }

pub(crate) fn set_debug_hook_callback(func: Option<DebugHookCallback>) {
    DEBUG_HOOK_CALLBACK.with(|callback| {
        *callback.borrow_mut() = func;
    })
}

fn find_repeat_count(instance: &ComponentInstance) -> usize {
    // FIXME: Do something to find the component's repeated index.
    match instance {
        ComponentInstance::InstanceRef(_) => 0,
        ComponentInstance::GlobalComponent(_) => 0,
    }
}

pub(crate) fn debug_hook_triggered(
    instance: &ComponentInstance,
    id: SmolStr,
    value: Value,
) -> Value {
    let repeat_count = find_repeat_count(instance);
    DEBUG_HOOK_CALLBACK.with(|callback| {
        if let Some(callback) = &*callback.borrow() {
            callback(id.as_str(), repeat_count, &value)
        } else {
            value
        }
    })
}
