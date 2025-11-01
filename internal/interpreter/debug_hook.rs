// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{dynamic_item_tree, Value};

use smol_str::SmolStr;

use std::pin::Pin;

pub type DebugHookCallback = Box<dyn Fn(&str, crate::Value) -> Value>;

pub(crate) fn set_debug_hook_callback(
    component: Pin<&dynamic_item_tree::ItemTreeBox>,
    func: Option<DebugHookCallback>,
) {
    let Some(global_storage) = component.description().compiled_globals.clone() else {
        return;
    };
    *(global_storage.debug_hook_callback.borrow_mut()) = func;
}

pub(crate) fn debug_hook_triggered(
    component_instance: &dynamic_item_tree::InstanceRef,
    id: SmolStr,
    value: Value,
) -> Value {
    let Some(global_storage) = component_instance.description.compiled_globals.clone() else {
        return value;
    };
    let callback = global_storage.debug_hook_callback.borrow();

    if let Some(callback) = callback.as_ref() {
        callback(id.as_str(), value)
    } else {
        value
    }
}
