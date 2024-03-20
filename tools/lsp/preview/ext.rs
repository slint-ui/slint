// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use i_slint_compiler::layout;

use crate::common;
use crate::preview::ui;

pub trait ElementRcNodeExt {
    fn layout_kind(&self) -> crate::preview::ui::LayoutKind;
}

impl ElementRcNodeExt for common::ElementRcNode {
    fn layout_kind(&self) -> crate::preview::ui::LayoutKind {
        self.with_element_debug(|_, l| match l {
            Some(layout::Layout::GridLayout(_)) => ui::LayoutKind::Grid,
            Some(layout::Layout::BoxLayout(layout::BoxLayout {
                orientation: layout::Orientation::Horizontal,
                ..
            })) => ui::LayoutKind::Horizontal,
            Some(layout::Layout::BoxLayout(layout::BoxLayout {
                orientation: layout::Orientation::Vertical,
                ..
            })) => ui::LayoutKind::Vertical,
            _ => ui::LayoutKind::None,
        })
    }
}
