// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Exact changed-pixel bounds for eight normal or sixteen exceptional display windows.

pub(crate) const LIMIT: usize = 8;
// The BOX-3 settings benchmark trades extra SPI pixels for less window-planning work.
const ADJACENT_EXTRA_PIXELS: usize = 256;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Rect {
    fn area(self) -> usize {
        self.width * self.height
    }
    fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            width: (self.x + self.width).max(other.x + other.width) - x,
            height: (self.y + self.height).max(other.y + other.height) - y,
        }
    }
}

pub(crate) struct ChangedSpan {
    rect: Rect,
    halves: [(u16, u16); 2],
}

impl ChangedSpan {
    /// Pixels outside these half-row bounds are equal in both compared rows.
    pub(crate) fn copy_bounds(&self) -> [(usize, usize); 2] {
        self.halves.map(|(left, right)| (left as usize, right as usize))
    }
}

pub(crate) struct Plan {
    cover: Cover,
    rows: [[(u16, u16); 2]; 240],
}

struct Cover {
    rectangles: [Rect; LIMIT * 2 + 1],
    count: usize,
    adjacent_extra: usize,
}

impl Cover {
    fn new(adjacent_extra: usize) -> Self {
        Self { rectangles: [Rect::default(); LIMIT * 2 + 1], count: 0, adjacent_extra }
    }
    pub fn rectangles(&self) -> &[Rect] {
        &self.rectangles[..self.count]
    }

    fn push(&mut self, row: Rect) {
        // Equal adjacent spans need no additional address-window command.
        if let Some(last) = self.rectangles[..self.count].last_mut() {
            if last.x == row.x && last.width == row.width && last.y + last.height == row.y {
                last.height += row.height;
                return;
            }
            if last.y + last.height == row.y {
                let joined = last.union(row);
                if joined.area().saturating_sub(last.area() + row.area()) <= self.adjacent_extra {
                    *last = joined;
                    return;
                }
            }
        }
        self.rectangles[self.count] = row;
        self.count += 1;
        if self.count <= LIMIT {
            return;
        }
        // Bound planning work: at most thirty-six pairs, regardless of image height.
        let mut best = (usize::MAX, 0, 1);
        for a in 0..self.count {
            for b in a + 1..self.count {
                let extra = self.rectangles[a]
                    .union(self.rectangles[b])
                    .area()
                    .saturating_sub(self.rectangles[a].area() + self.rectangles[b].area());
                if extra < best.0 {
                    best = (extra, a, b);
                }
            }
        }
        self.rectangles[best.1] = self.rectangles[best.1].union(self.rectangles[best.2]);
        self.rectangles.copy_within(best.2 + 1..self.count, best.2);
        self.count -= 1;
    }

    fn area(&self) -> usize {
        self.rectangles().iter().map(|rect| rect.area()).sum()
    }
}

impl Plan {
    pub fn new() -> Self {
        Self { cover: Cover::new(ADJACENT_EXTRA_PIXELS), rows: [[(u16::MAX, 0); 2]; 240] }
    }
    pub fn reset(&mut self) {
        self.cover.count = 0;
        self.cover.adjacent_extra = ADJACENT_EXTRA_PIXELS;
        self.rows.fill([(u16::MAX, 0); 2]);
    }
    pub fn rectangles(&self) -> &[Rect] {
        self.cover.rectangles()
    }
    pub fn line(&mut self, current: &[u16], previous: &[u16], x: usize, y: usize) {
        if let Some(rect) = changed_line(current, previous, x, y) {
            self.add(rect);
        }
    }

    pub fn add(&mut self, span: ChangedSpan) {
        let row = &mut self.rows[span.rect.y];
        for (bounds, changed) in row.iter_mut().zip(span.halves) {
            bounds.0 = bounds.0.min(changed.0);
            bounds.1 = bounds.1.max(changed.1);
        }
        self.cover.push(span.rect);
    }

    pub fn area(&self) -> usize {
        self.cover.area()
    }

    /// Retain the cheap normal plan unless it exceeds the internal DMA packet.
    pub fn fit_packet(&mut self, max_pixels: usize) {
        if self.area() <= max_pixels {
            return;
        }
        for budget in [96, 0] {
            let mut cover = Cover::new(budget);
            for (y, halves) in self.rows.iter().enumerate() {
                let left = halves[0].0.min(halves[1].0);
                let right = halves[0].1.max(halves[1].1);
                if left < right {
                    cover.push(Rect {
                        x: left as usize,
                        y,
                        width: (right - left) as usize,
                        height: 1,
                    });
                }
            }
            if cover.area() < self.area() {
                self.cover = cover;
            }
            if self.area() <= max_pixels {
                return;
            }
        }
        // A wide row can contain unchanged space between labels and controls.
        // Exceptional frames use eight windows per screen half to omit that space.
        let mut split = Cover::new(0);
        for half in 0..2 {
            let mut cover = Cover::new(0);
            for (y, halves) in self.rows.iter().enumerate() {
                let (left, right) = halves[half];
                if left < right {
                    cover.push(Rect {
                        x: left as usize,
                        y,
                        width: (right - left) as usize,
                        height: 1,
                    });
                }
            }
            split.rectangles[split.count..split.count + cover.count]
                .copy_from_slice(cover.rectangles());
            split.count += cover.count;
        }
        if split.area() < self.area() {
            split.rectangles[..split.count].sort_unstable_by_key(|rect| (rect.y, rect.x));
            self.cover = split;
        }
    }

    #[cfg(test)]
    pub fn between(
        current: &[u16],
        previous: &[u16],
        stride: usize,
        height: usize,
        dirty: impl IntoIterator<Item = Rect>,
    ) -> Self {
        assert_eq!(current.len(), previous.len());
        assert!(stride * height <= current.len());
        assert!(height <= 240);
        let mut rows = [(usize::MAX, 0usize); 240];
        for rect in dirty {
            assert!(rect.x + rect.width <= stride && rect.y + rect.height <= height);
            for row in &mut rows[rect.y..rect.y + rect.height] {
                row.0 = row.0.min(rect.x);
                row.1 = row.1.max(rect.x + rect.width);
            }
        }
        let mut plan = Self::new();
        for (y, &(left, right)) in rows[..height].iter().enumerate() {
            if left >= right {
                continue;
            }
            let start = y * stride + left;
            plan.line(
                &current[start..y * stride + right],
                &previous[start..y * stride + right],
                left,
                y,
            );
        }
        plan.cover.rectangles[..plan.cover.count].sort_unstable_by_key(|rect| (rect.y, rect.x));
        plan
    }
}

pub(crate) fn changed_line(
    current: &[u16],
    previous: &[u16],
    x: usize,
    y: usize,
) -> Option<ChangedSpan> {
    assert_eq!(current.len(), previous.len());
    let mut halves = [(u16::MAX, 0); 2];
    for (half, start, end) in
        [(0, x, (x + current.len()).min(160)), (1, x.max(160), x + current.len())]
    {
        if start < end {
            let first = start - x;
            let last = end - x;
            if let Some((left, right)) = changed_span(&current[first..last], &previous[first..last])
            {
                halves[half] = ((start + left) as u16, (start + right) as u16);
            }
        }
    }
    let left = halves[0].0.min(halves[1].0) as usize;
    let right = halves[0].1.max(halves[1].1) as usize;
    if left >= right {
        return None;
    }
    let rect = Rect { x: left, y, width: right - left, height: 1 };
    Some(ChangedSpan { rect, halves })
}

#[inline(always)]
unsafe fn equal_eight<const ALIGNMENT: u8>(a: *const u16, b: *const u16) -> bool {
    // The caller supplies eight pixels. Each block advances sixteen bytes,
    // retaining the alignment selected once for the complete spans.
    unsafe {
        if ALIGNMENT == 0 {
            let a = a.cast::<u32>();
            let b = b.cast::<u32>();
            a.read() == b.read()
                && a.add(1).read() == b.add(1).read()
                && a.add(2).read() == b.add(2).read()
                && a.add(3).read() == b.add(3).read()
        } else if ALIGNMENT == 1 {
            let ap = a.add(1).cast::<u32>();
            let bp = b.add(1).cast::<u32>();
            a.read() == b.read()
                && a.add(7).read() == b.add(7).read()
                && ap.read() == bp.read()
                && ap.add(1).read() == bp.add(1).read()
                && ap.add(2).read() == bp.add(2).read()
        } else {
            let a = a.cast::<u32>();
            let b = b.cast::<u32>();
            a.read_unaligned() == b.read_unaligned()
                && a.add(1).read_unaligned() == b.add(1).read_unaligned()
                && a.add(2).read_unaligned() == b.add(2).read_unaligned()
                && a.add(3).read_unaligned() == b.add(3).read_unaligned()
        }
    }
}

fn bounds_with_alignment<const ALIGNMENT: u8>(
    current: &[u16],
    previous: &[u16],
) -> Option<(usize, usize)> {
    let blocks = current.len() / 8;
    let tail = blocks * 8;
    let mut first = 0;
    while first < blocks {
        // Only complete eight-pixel blocks reach the wide-load helper.
        if !unsafe {
            equal_eight::<ALIGNMENT>(
                current.as_ptr().add(first * 8),
                previous.as_ptr().add(first * 8),
            )
        } {
            break;
        }
        first += 1;
    }
    if first == blocks {
        let left = current[tail..].iter().zip(&previous[tail..]).position(|(a, b)| a != b)?;
        let right =
            current[tail..].iter().zip(&previous[tail..]).rposition(|(a, b)| a != b).unwrap() + 1;
        return Some((tail + left, tail + right));
    }
    let left = first * 8
        + current[first * 8..first * 8 + 8]
            .iter()
            .zip(&previous[first * 8..first * 8 + 8])
            .position(|(a, b)| a != b)
            .unwrap();
    if let Some(right) = current[tail..].iter().zip(&previous[tail..]).rposition(|(a, b)| a != b) {
        return Some((left, tail + right + 1));
    }
    let mut last = blocks - 1;
    while last > first {
        if !unsafe {
            equal_eight::<ALIGNMENT>(
                current.as_ptr().add(last * 8),
                previous.as_ptr().add(last * 8),
            )
        } {
            break;
        }
        last -= 1;
    }
    let right = last * 8
        + current[last * 8..last * 8 + 8]
            .iter()
            .zip(&previous[last * 8..last * 8 + 8])
            .rposition(|(a, b)| a != b)
            .unwrap()
        + 1;
    Some((left, right))
}

fn changed_span(current: &[u16], previous: &[u16]) -> Option<(usize, usize)> {
    assert_eq!(current.len(), previous.len());
    let a = current.as_ptr() as usize;
    let b = previous.as_ptr() as usize;
    if (a | b) & 3 == 0 {
        bounds_with_alignment::<0>(current, previous)
    } else if (a ^ b) & 3 == 0 {
        bounds_with_alignment::<1>(current, previous)
    } else {
        bounds_with_alignment::<2>(current, previous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_block_edges_and_unaligned_slices() {
        for length in 1..65 {
            for index in 0..length {
                let a = [0u16; 66];
                let mut b = a;
                b[index + 1] = 1;
                assert_eq!(
                    changed_span(&a[1..length + 1], &b[1..length + 1]),
                    Some((index, index + 1))
                );
            }
        }
        assert_eq!(changed_span(&[1, 2, 3], &[1, 2, 3]), None);
    }
    #[test]
    fn word_comparison_covers_matching_and_different_pointer_alignments() {
        #[repr(align(4))]
        struct Words([u16; 16]);
        for first in 0..2 {
            for second in 0..2 {
                for changed in 0..9 {
                    let a = Words(core::array::from_fn(|i| i as u16 * 37));
                    let mut b = Words([0xaaaa; 16]);
                    b.0[second..second + 8].copy_from_slice(&a.0[first..first + 8]);
                    if changed < 8 {
                        b.0[second + changed] ^= 0xffff;
                    }
                    assert_eq!(
                        changed_span(&a.0[first..first + 8], &b.0[second..second + 8]).is_none(),
                        changed == 8
                    );
                    assert_eq!(b.0[second + 8], 0xaaaa);
                }
            }
        }
    }
    #[test]
    fn bounds_are_limited_and_cover_every_change() {
        let mut previous = vec![0; 320 * 240];
        let mut current = previous.clone();
        let dirty = Rect { x: 3, y: 5, width: 310, height: 230 };
        for frame in 0..240 {
            current.copy_from_slice(&previous);
            for y in 5..235 {
                let x = 3 + (y * 43 + frame * 17) % 310;
                current[y * 320 + x] = frame as u16 + 1;
            }
            let plan = Plan::between(&current, &previous, 320, 240, [dirty]);
            assert!(plan.rectangles().len() <= LIMIT);
            for rect in plan.rectangles() {
                for y in rect.y..rect.y + rect.height {
                    let start = y * 320 + rect.x;
                    previous[start..start + rect.width]
                        .copy_from_slice(&current[start..start + rect.width]);
                }
            }
            assert_eq!(current, previous);
        }
    }
    #[test]
    fn capacity_replanning_never_expands_or_loses_changed_pixels() {
        let old = vec![0u16; 320 * 240];
        let mut current = old.clone();
        let dirty = Rect { x: 0, y: 0, width: 320, height: 240 };
        let mut oversized = 0;
        for frame in 0..240 {
            current.fill(0);
            for y in 20..220 {
                let left = (y * 7 + frame * 13) % 160;
                let width = 20 + (y * 3 + frame) % 90;
                current[y * 320 + left..y * 320 + left + width].fill(1);
            }
            let mut plan = Plan::between(&current, &old, 320, 240, [dirty]);
            let area = plan.area();
            let rectangles = plan.rectangles().to_vec();
            plan.fit_packet(32 * 1024);
            oversized += usize::from(area > 32 * 1024);
            assert!(plan.area() <= area);
            assert!(plan.rectangles().len() <= LIMIT * 2);
            if area <= 32 * 1024 {
                assert_eq!(plan.rectangles(), rectangles);
            }
            let mut replay = old.clone();
            for rect in plan.rectangles() {
                for y in rect.y..rect.y + rect.height {
                    let start = y * 320 + rect.x;
                    replay[start..start + rect.width]
                        .copy_from_slice(&current[start..start + rect.width]);
                }
            }
            assert_eq!(replay, current);
        }
        assert!(oversized > 0);
    }
    #[test]
    fn split_windows_omit_an_unchanged_gap_and_fit_internal_packet() {
        let old = vec![0u16; 320 * 240];
        let mut current = old.clone();
        for y in 0..240 {
            current[y * 320 + 4 + y % 17] = 1;
            current[y * 320 + 310 - y % 13] = 2;
        }
        let mut plan =
            Plan::between(&current, &old, 320, 240, [Rect { x: 0, y: 0, width: 320, height: 240 }]);
        assert!(plan.area() > 32 * 1024);
        plan.fit_packet(32 * 1024);
        assert!(plan.area() <= 32 * 1024);
        assert!(plan.rectangles().len() > LIMIT);
        assert!(plan.rectangles().len() <= LIMIT * 2);
        let mut replay = old.clone();
        for rect in plan.rectangles() {
            assert!(rect.x + rect.width <= 160 || rect.x >= 160);
            for y in rect.y..rect.y + rect.height {
                let start = y * 320 + rect.x;
                replay[start..start + rect.width]
                    .copy_from_slice(&current[start..start + rect.width]);
            }
        }
        assert_eq!(current, replay);
    }
    #[test]
    fn resetting_a_reused_plan_drops_previous_damage_and_split_bounds() {
        let old = vec![0u16; 320 * 240];
        let mut current = old.clone();
        for y in 0..240 {
            current[y * 320 + 4] = 1;
            current[y * 320 + 310] = 2;
        }
        let mut plan =
            Plan::between(&current, &old, 320, 240, [Rect { x: 0, y: 0, width: 320, height: 240 }]);
        plan.fit_packet(32 * 1024);
        plan.reset();
        assert_eq!(plan.area(), 0);
        let mut row = [0u16; 320];
        row[200] = 3;
        plan.line(&row, &[0; 320], 0, 20);
        plan.fit_packet(32 * 1024);
        assert_eq!(plan.rectangles(), &[Rect { x: 200, y: 20, width: 1, height: 1 }]);
    }
    #[test]
    fn dirty_regions_padding_and_erased_pixels() {
        let mut old = vec![0; 40 * 20];
        let mut new = old.clone();
        let regions =
            [Rect { x: 2, y: 1, width: 8, height: 8 }, Rect { x: 25, y: 9, width: 10, height: 10 }];
        new[3 * 40 + 4] = 0xffff;
        new[13 * 40 + 29] = 0x100;
        for current in [&new, &vec![0; 40 * 20]] {
            let plan = Plan::between(current, &old, 40, 20, regions);
            for r in plan.rectangles() {
                for y in r.y..r.y + r.height {
                    old[y * 40 + r.x..y * 40 + r.x + r.width]
                        .copy_from_slice(&current[y * 40 + r.x..y * 40 + r.x + r.width]);
                }
            }
            assert_eq!(*current, old);
        }
    }
    #[repr(align(4))]
    struct Pixels([u16; 330]);
    #[test]
    fn every_boundary_and_alignment_matches_scalar() {
        for len in 0..=320 {
            for aoffset in 0..2 {
                for boffset in 0..2 {
                    let a = Pixels([0x15; 330]);
                    let mut b = Pixels([0x15; 330]);
                    // Unequal canaries must never affect bounds outside the slices.
                    b.0[boffset + len] = 0xffff;
                    let empty =
                        changed_span(&a.0[aoffset..aoffset + len], &b.0[boffset..boffset + len]);
                    assert_eq!(empty, None);
                    for index in 0..len {
                        b.0[boffset + index] ^= 1;
                        assert_eq!(
                            changed_span(
                                &a.0[aoffset..aoffset + len],
                                &b.0[boffset..boffset + len]
                            ),
                            Some((index, index + 1)),
                            "len{len} aoffset{aoffset} boffset{boffset} index{index}"
                        );
                        b.0[boffset + index] ^= 1;
                    }
                }
            }
        }
    }
    #[test]
    fn randomized_sparse_and_dense_spans_match_scalar() {
        let mut seed = 0x912bbcda_u32;
        for case in 0..12000 {
            let len = case % 321;
            let aoffset = case % 2;
            let boffset = (case / 2) % 2;
            let a = Pixels([0x30; 330]);
            let mut b = Pixels([0x30; 330]);
            for value in &mut b.0[boffset..boffset + len] {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                if seed % ((case % 31 + 1) as u32) == 0 {
                    *value ^= 1;
                }
            }
            let current = &a.0[aoffset..aoffset + len];
            let previous = &b.0[boffset..boffset + len];
            let expected = current.iter().zip(previous).position(|(a, b)| a != b).map(|left| {
                (left, current.iter().zip(previous).rposition(|(a, b)| a != b).unwrap() + 1)
            });
            assert_eq!(changed_span(current, previous), expected);
        }
    }
}
