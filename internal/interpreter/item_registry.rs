// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Maps a native item class name to a factory that allocates it.
//!
//! Built lazily on first access.
//! Covers every builtin item in `i_slint_core::items` plus the backend-specific
//! widgets exposed through `i_slint_backend_selector::NativeWidgets`.

use crate::erased::ErasedItemRc;
use crate::item_holder::make_item;
use i_slint_core::items::*;
use i_slint_core::rtti::BuiltinItem;
use std::collections::HashMap;
use std::sync::OnceLock;

pub type ItemFactory = fn() -> ErasedItemRc;

pub struct ItemRegistry {
    factories: HashMap<&'static str, ItemFactory>,
}

impl ItemRegistry {
    pub fn factory(&self, class_name: &str) -> Option<ItemFactory> {
        self.factories.get(class_name).copied()
    }

    pub fn global() -> &'static ItemRegistry {
        static REGISTRY: OnceLock<ItemRegistry> = OnceLock::new();
        REGISTRY.get_or_init(ItemRegistry::build)
    }

    fn build() -> Self {
        let mut factories: HashMap<&'static str, ItemFactory> = HashMap::new();

        macro_rules! reg {
            ($($ty:ident),* $(,)?) => {
                $(
                    factories.insert(<$ty as BuiltinItem>::name(), make_item::<$ty> as ItemFactory);
                )*
            };
        }

        reg!(
            ComponentContainer,
            Empty,
            ImageItem,
            ClippedImage,
            ComplexText,
            StyledTextItem,
            SimpleText,
            Rectangle,
            BasicBorderRectangle,
            BorderRectangle,
            TouchArea,
            FocusScope,
            KeyBinding,
            SwipeGestureHandler,
            ScaleRotateGestureHandler,
            Path,
            Flickable,
            WindowItem,
            TextInput,
            Clip,
            BoxShadow,
            Transform,
            Opacity,
            Layer,
            DragArea,
            DropArea,
            ContextMenu,
            MenuItem,
        );

        // Walk the backend's `NativeWidgets` type list and register each entry.
        // Same pattern the rust code generator uses to enumerate native items.
        <i_slint_backend_selector::NativeWidgets as NativeHelper>::push(&mut factories);

        ItemRegistry { factories }
    }
}

/// Walks a nested-tuple type list `(T1, (T2, (…, ())))` and registers each `Ti`.
trait NativeHelper {
    fn push(factories: &mut HashMap<&'static str, ItemFactory>);
}

impl NativeHelper for () {
    fn push(_: &mut HashMap<&'static str, ItemFactory>) {}
}

impl<T: 'static + Default + BuiltinItem + vtable::HasStaticVTable<ItemVTable>, Next: NativeHelper>
    NativeHelper for (T, Next)
{
    fn push(factories: &mut HashMap<&'static str, ItemFactory>) {
        factories.insert(<T as BuiltinItem>::name(), make_item::<T> as ItemFactory);
        Next::push(factories);
    }
}
