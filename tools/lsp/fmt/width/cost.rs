// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! What "pretty" means for the layout search: the cost of a (partial) layout.

use super::PAGE_WIDTH;

/// The cost of a layout, summed over its lines and choices.
///
/// The derived `Ord` compares lexicographically in field order, which is the
/// intended priority: staying within [`PAGE_WIDTH`] dominates, then following
/// the author's input layout, then using fewer lines. Height ranking *below*
/// deviation is deliberate — otherwise the search would collapse every
/// construct the author spread out, since fewer lines is always cheaper.
// Not used by the shipped pipeline yet; exercised by the width tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cost {
    /// Sum of the squared character counts past [`PAGE_WIDTH`], per line.
    /// Squaring degrades gracefully: one line 20 characters over is worse
    /// than two lines 10 over.
    overflow: u64,
    /// Number of groups decided against the author's input layout.
    deviation: u32,
    /// Number of newlines.
    height: u32,
}

// Not used by the shipped pipeline yet; exercised by the width tests.
#[allow(dead_code)]
impl Cost {
    pub const ZERO: Cost = Cost { overflow: 0, deviation: 0, height: 0 };

    /// The penalty for deciding one group against the author's input layout.
    pub const DEVIATION: Cost = Cost { overflow: 0, deviation: 1, height: 0 };

    /// The cost of placing `length` characters starting at `column`.
    ///
    /// Defined as a difference of squared excesses so that placing a line in
    /// pieces costs exactly the same as placing it at once — the pruning in
    /// the search is only sound under this "splitting" contract.
    pub fn text(column: u32, length: u32) -> Cost {
        Cost { overflow: squared_excess(column + length) - squared_excess(column), ..Cost::ZERO }
    }

    /// The cost of a newline followed by `indent_width` characters of
    /// indentation. Charging the indentation is essential: otherwise breaking
    /// at an indent beyond [`PAGE_WIDTH`] would look free, and the search
    /// would prefer many overflowing short lines over the least-bad
    /// compromise.
    pub fn newline(indent_width: u32) -> Cost {
        Cost { overflow: squared_excess(indent_width), height: 1, ..Cost::ZERO }
    }
}

impl std::ops::Add for Cost {
    type Output = Cost;

    fn add(self, other: Cost) -> Cost {
        Cost {
            overflow: self.overflow + other.overflow,
            deviation: self.deviation + other.deviation,
            height: self.height + other.height,
        }
    }
}

/// The squared number of characters by which `column` exceeds [`PAGE_WIDTH`].
/// Columns at or before the page width cost nothing.
fn squared_excess(column: u32) -> u64 {
    u64::from(column.saturating_sub(PAGE_WIDTH)).pow(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_within_the_page_width_is_free() {
        assert_eq!(Cost::text(0, PAGE_WIDTH), Cost::ZERO);
        assert_eq!(Cost::text(PAGE_WIDTH / 2, PAGE_WIDTH / 2), Cost::ZERO);
        assert_eq!(Cost::text(PAGE_WIDTH, 0), Cost::ZERO);
    }

    #[test]
    fn text_past_the_page_width_costs_the_squared_excess() {
        // A line ending 3 past the limit costs 3², wherever it started.
        assert_eq!(Cost::text(0, PAGE_WIDTH + 3), Cost::text(PAGE_WIDTH, 3));
        assert_eq!(
            Cost::text(0, PAGE_WIDTH + 3) + Cost::newline(0),
            Cost { overflow: 9, deviation: 0, height: 1 }
        );
        // Text placed entirely past the limit costs only its own excess.
        assert_eq!(
            Cost::text(PAGE_WIDTH + 10, 5),
            Cost { overflow: (15 * 15) - (10 * 10), ..Cost::ZERO }
        );
    }

    #[test]
    fn splitting_a_line_into_pieces_costs_the_same_as_placing_it_whole() {
        // The contract the search's pruning relies on, exhaustively around
        // the page-width boundary.
        for column in (0..=2 * PAGE_WIDTH).step_by(7) {
            for first_length in (0..=PAGE_WIDTH).step_by(11) {
                for second_length in (0..=PAGE_WIDTH).step_by(13) {
                    assert_eq!(
                        Cost::text(column, first_length + second_length),
                        Cost::text(column, first_length)
                            + Cost::text(column + first_length, second_length),
                        "splitting at column {column} into {first_length}+{second_length}"
                    );
                }
            }
        }
    }

    #[test]
    fn text_cost_is_monotone_in_the_column() {
        for column in 0..2 * PAGE_WIDTH {
            for length in [0, 1, 30, PAGE_WIDTH] {
                assert!(
                    Cost::text(column, length) <= Cost::text(column + 1, length),
                    "text({column}, {length}) must not get cheaper further right"
                );
            }
        }
    }

    #[test]
    fn addition_sums_every_component() {
        // Pins `+` as a component-wise sum — a max-instead-of-sum mutation
        // would silently wreck the "cost of a layout = sum over its lines"
        // semantics while passing the comparison tests.
        assert_eq!(
            Cost::newline(0) + Cost::newline(2) + Cost::DEVIATION + Cost::DEVIATION,
            Cost { overflow: 0, deviation: 2, height: 2 }
        );
        assert_eq!(
            Cost::text(PAGE_WIDTH, 3) + Cost::text(PAGE_WIDTH, 4),
            Cost { overflow: 9 + 16, deviation: 0, height: 0 }
        );
    }

    #[test]
    fn newline_charges_its_indentation() {
        assert_eq!(Cost::newline(0), Cost { overflow: 0, deviation: 0, height: 1 });
        assert_eq!(Cost::newline(PAGE_WIDTH + 4), Cost { overflow: 16, deviation: 0, height: 1 });
    }

    #[test]
    fn comparison_is_lexicographic_overflow_then_deviation_then_height() {
        let cheap = Cost::ZERO;
        let tall = Cost::newline(0) + Cost::newline(0);
        let deviating = Cost::DEVIATION;
        let overflowing = Cost::text(PAGE_WIDTH, 1);

        // Any overflow is worse than any amount of deviation or height.
        assert!(deviating < overflowing);
        assert!(tall + tall + deviating + deviating < overflowing);
        // Any deviation is worse than any height.
        assert!(tall + tall < deviating);
        assert!(cheap < tall);
    }
}
