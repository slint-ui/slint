// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::{Value, dynamic_item_tree};

use smol_str::SmolStr;

use std::pin::Pin;

pub type DebugHookCallback = Box<dyn Fn(&str) -> Option<Value>>;

pub(crate) fn set_debug_hook_callback(
    component: Pin<&dynamic_item_tree::ItemTreeBox>,
    func: Option<DebugHookCallback>,
) {
    let Some(global_storage) = component.description().compiled_globals() else {
        return;
    };
    *(global_storage.debug_hook_callback.borrow_mut()) = func;
}

pub(crate) fn trigger_debug_hook(
    component_instance: &dynamic_item_tree::InstanceRef,
    id: SmolStr,
) -> Option<Value> {
    component_instance.description.compiled_globals().and_then(|global_storage| {
        let callback = global_storage.debug_hook_callback.borrow();
        callback.as_ref().and_then(|callback| callback(&id))
    })
}
