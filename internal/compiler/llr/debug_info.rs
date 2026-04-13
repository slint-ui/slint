// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Debug information attached to LLR structures.
//! The interpreter uses it for highlighting, live preview and element picking.
//!
//! Populated only when [`crate::CompilerConfiguration::debug_info`] is set.
//! Treat entries as advisory and tolerate missing data.
//!
//! These types hold a [`crate::diagnostics::SourceLocation`] which isn't `Send`.
//! Making LLR `Send` will require replacing it with a path + offset.

use crate::diagnostics::SourceLocation;
use smol_str::SmolStr;
use typed_index_collections::TiVec;

use super::item_tree::ItemInstanceIdx;

/// Debug info for a single item within a [`SubComponent`](super::SubComponent).
#[derive(Debug, Clone)]
pub struct ItemDebugInfo {
    /// Source range of the element in the `.slint` source.
    pub source_location: SourceLocation,
    /// Qualified id of the element, e.g. `MyComponent::my-button`.
    pub qualified_id: Option<SmolStr>,
    /// Stable hash identifying the source element across builds.
    /// See [`crate::object_tree::ElementDebugInfo::element_hash`].
    pub element_hash: u64,
}

/// Debug info for a [`SubComponent`](super::SubComponent).
#[derive(Debug, Clone)]
pub struct SubComponentDebugInfo {
    /// Source location of the sub-component's root element.
    pub source_location: SourceLocation,
    /// One entry per [`ItemInstanceIdx`].
    pub items: TiVec<ItemInstanceIdx, ItemDebugInfo>,
}
