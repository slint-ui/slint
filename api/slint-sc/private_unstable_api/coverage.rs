// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Counting the coverage points of the `.slint` code.
//!
//! Code generated with `--coverage` holds a static [`Counters`] and bumps the
//! count of a point where it is reached. The counters print as a profile
//! that names the points' locations, for `slint-sc-coverage` to report.

use core::sync::atomic::{AtomicU32, Ordering};

/// The hit counts of the `N` coverage points of a generated file, and the map
/// locating them.
pub struct Counters<const N: usize> {
    counts: [AtomicU32; N],
    /// The `file` and `point` records of the profile.
    map: &'static str,
}

impl<const N: usize> Counters<N> {
    /// Counters at zero for the points `map` describes.
    pub const fn new(map: &'static str) -> Self {
        Self { counts: [const { AtomicU32::new(0) }; N], map }
    }

    /// Count that the point was reached. The count saturates. A load and a
    /// store rather than a read-modify-write, which the smallest targets lack:
    /// an interrupt hitting the same point in between loses a count, never
    /// the fact that the point was reached.
    pub fn hit(&self, point: usize) {
        let count = &self.counts[point];
        count.store(count.load(Ordering::Relaxed).saturating_add(1), Ordering::Relaxed);
    }

    /// Count the outcome of a decision as one of two points, and yield it.
    pub fn branch(&self, when_true: usize, when_false: usize, condition: bool) -> bool {
        self.hit(if condition { when_true } else { when_false });
        condition
    }
}

/// The profile: the map, then the count of each point that was reached.
///
/// Combinators rather than `?`, whose error paths no test can reach, while
/// the runtime is held at complete region coverage.
impl<const N: usize> core::fmt::Display for Counters<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let counts = self.counts.iter().map(|count| count.load(Ordering::Relaxed));
        let mut reached = counts.enumerate().filter(|(_, count)| *count != 0);
        f.write_str("slint-sc-coverage 1\n").and_then(|()| f.write_str(self.map)).and_then(|()| {
            reached.try_for_each(|(point, count)| writeln!(f, "count {point} {count}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_prints() {
        // Generated code holds the counters in a static, whose initializer
        // runs at compile time; a local exercises `new` at run time.
        let counters = Counters::<3>::new("file a.slint\npoint element 0 1:1-1:2\n");
        counters.hit(0);
        counters.hit(0);
        assert!(counters.branch(1, 2, true));
        assert!(!counters.branch(1, 2, false));
        assert!(!counters.branch(1, 2, false));
        let profile = crate::Sink::format(format_args!("{counters}"));
        assert_eq!(
            profile.as_str(),
            "slint-sc-coverage 1\nfile a.slint\npoint element 0 1:1-1:2\ncount 0 2\ncount 1 1\ncount 2 2\n"
        );
        let untouched = Counters::<1>::new("");
        let profile = crate::Sink::format(format_args!("{untouched}"));
        assert_eq!(profile.as_str(), "slint-sc-coverage 1\n");
    }
}
