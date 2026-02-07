// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::*;
use core::marker::PhantomData;

struct TwoWayBindingModel<T, ItemTree, Getter, Setter> {
    phantom: PhantomData<fn(T) -> T>,
    item_tree: ItemTree,
    getter: Getter,
    setter: Setter,
}

// Safety:  IS_TWO_WAY_BINDING is false
unsafe impl<T, ItemTree, Getter, Setter> BindingCallable<T>
    for TwoWayBindingModel<T, ItemTree, Getter, Setter>
where
    Getter: Fn(&ItemTree) -> Option<T>,
    Setter: Fn(&ItemTree, &T),
{
    fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
        if let Some(v) = (self.getter)(&self.item_tree) {
            *value = v;
        }
        BindingResult::KeepBinding
    }

    unsafe fn intercept_set_binding(self: Pin<&Self>, _new_binding: *mut BindingHolder) -> bool {
        panic!("Cannot assign a binding to a property bound two-way to a model");
    }

    fn intercept_set(self: Pin<&Self>, value: &T) -> bool {
        (self.setter)(&self.item_tree, value);
        true
    }
}

impl<T: 'static> Property<T> {
    /// Helper function to create a two way binding to a model data
    /// The item_tree is a ItemTreeWeak.
    /// The getter gets the model data from the data_property (and record dependency)
    /// the setter
    pub fn link_two_way_to_model_data<ItemTree: 'static>(
        self: Pin<&Self>,
        item_tree: ItemTree,
        getter: impl Fn(&ItemTree) -> Option<T> + 'static,
        setter: impl Fn(&ItemTree, &T) + 'static,
    ) {
        let binding = TwoWayBindingModel { phantom: PhantomData, item_tree, getter, setter };
        // Safety: TwoWayBindingModel implement BindingCallable<T> which is the same T as the `Self`
        unsafe { self.handle.set_binding(binding) };
    }
}
