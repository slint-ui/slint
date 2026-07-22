// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Counter used to generate deterministic unique symbol names while running the
//! passes.

use std::cell::Cell;
use std::rc::Rc;

use smol_str::{SmolStr, format_smolstr};

/// A counter, shared across the whole compilation: the
/// [`crate::typeloader::TypeLoader`] holds it and hands a reference to the passes
/// that generate names. Sharing it makes the names unique across all the
/// documents of a compilation, including ones pulled in by inlining, so they
/// cannot clash once components from different documents end up in the same
/// generated code.
#[derive(Default)]
pub struct SymbolCounters {
    next: Cell<usize>,
}

impl SymbolCounters {
    pub fn shared() -> Rc<Self> {
        Rc::new(Self::default())
    }

    /// Return a unique name made of `base` followed by a number, e.g.
    /// `generate_name("tmpobj_conv_")` -> `tmpobj_conv_0`.
    pub fn generate_name(&self, base: &str) -> SmolStr {
        let n = self.next.get();
        self.next.set(n + 1);
        format_smolstr!("{base}{n}")
    }
}
