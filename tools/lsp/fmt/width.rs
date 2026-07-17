// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Width-aware layout search for the query-based formatter.
//!
//! The search decides, per group of correlated break points, between its
//! single-line and its multiline layout, so that lines stay within
//! [`PAGE_WIDTH`] where possible while otherwise respecting the author's
//! input layout. See `MAX_LINE_WIDTH_DESIGN.md` in this directory.
//!
//! Widths and columns count characters, not bytes — byte counting would make
//! non-ASCII string literals overflow spuriously.

pub mod cost;
pub mod doc;
pub mod search;

/// The line width the formatter tries to stay within. Characters beyond this
/// column make a layout increasingly expensive.
pub const PAGE_WIDTH: u32 = 100;

/// The column beyond which the search stops optimizing (the paper's
/// computation width limit W, recommended as the page width + 25%). Output is
/// still produced past it, just without the optimality guarantee; this cutoff
/// is what bounds the running time.
// Not used by the shipped pipeline yet; used by the width search.
#[allow(dead_code)]
pub const COMPUTATION_WIDTH: u32 = PAGE_WIDTH + PAGE_WIDTH / 4;

/// Identity of one correlated layout choice. Every measured softline and
/// conditional atom sharing one measure span belongs to the same group and
/// flips between the two [`Variant`]s together.
// Not used by the shipped pipeline yet; exercised by the width tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupId(pub u32);

/// The two layouts a group can take.
// Not used by the shipped pipeline yet; exercised by the width tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    SingleLine,
    Multiline,
}
