// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::item_tree::ItemRc;
use i_slint_core::items::{
    BasicBorderRectangle, BorderRectangle, ComplexText, Rectangle, SimpleText, TextInput,
};
use i_slint_core::window::WindowInner;
use std::ops::ControlFlow;

pub fn find(parent: &ItemRc, id: &str) -> ItemRc {
    parent
        .visit_descendants(|item| {
            let matched = (0..item.element_count().unwrap_or(0))
                .filter_map(|index| item.element_type_names_and_ids(index))
                .flatten()
                .any(|(_, name)| name == id);
            if matched { ControlFlow::Break(item.clone()) } else { ControlFlow::Continue(()) }
        })
        .unwrap_or_else(|| panic!("Missing item {id}"))
}

pub fn color(window: &slint::Window, control: &str, element: &str) -> slint::Brush {
    let root = ItemRc::new_root(WindowInner::from_pub(window).component());
    let item = find(&find(&root, control), element);
    macro_rules! read {
        ($ty:ty, $property:ident) => {
            if let Some(item) = item.downcast::<$ty>() {
                return item.as_pin_ref().$property();
            }
        };
    }
    read!(Rectangle, background);
    read!(BasicBorderRectangle, background);
    read!(BorderRectangle, background);
    read!(SimpleText, color);
    read!(ComplexText, color);
    read!(TextInput, color);
    panic!("Item {element} has no supported color property")
}
