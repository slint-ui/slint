// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! How a property is used, as the passes find out and the LLR keeps it

#[derive(Clone, Default, Debug)]
pub struct PropertyAnalysis {
    /// true if somewhere in the code, there is an expression that changes this property with an assignment
    pub is_set: bool,

    /// True if this property might be set from a different component.
    pub is_set_externally: bool,

    /// true if somewhere in the code, an expression is reading this property
    /// Note: currently this is only set in the binding analysis pass
    pub is_read: bool,

    /// true if this property is read from another component
    pub is_read_externally: bool,

    /// True if the property is linked to another property that is read only. That property becomes read-only
    pub is_linked_to_read_only: bool,

    /// True if this property is linked to another property
    pub is_linked: bool,
}

impl PropertyAnalysis {
    /// Merge analysis from base element for inlining
    ///
    /// Contrary to `merge`, we don't keep the external uses because
    /// they should come from us
    pub fn merge_with_base(&mut self, other: &PropertyAnalysis) {
        self.is_set |= other.is_set;
        self.is_read |= other.is_read;
    }

    /// Merge the analysis
    pub fn merge(&mut self, other: &PropertyAnalysis) {
        self.is_set |= other.is_set;
        self.is_read |= other.is_read;
        self.is_read_externally |= other.is_read_externally;
        self.is_set_externally |= other.is_set_externally;
    }

    /// Return true if it is read or set or used in any way
    pub fn is_used(&self) -> bool {
        self.is_read || self.is_read_externally || self.is_set || self.is_set_externally
    }
}
