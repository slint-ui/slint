// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Changed-pixel bounds of a rendered line against the previous frame.

pub(crate) struct ChangedSpan {
    halves: [(u16, u16); 2],
}

impl ChangedSpan {
    /// Pixels outside these half-row bounds are equal in both compared rows.
    pub(crate) fn copy_bounds(&self) -> [(usize, usize); 2] {
        self.halves.map(|(left, right)| (left as usize, right as usize))
    }

    /// The first changed pixel and the end of the last one.
    pub(crate) fn bounds(&self) -> (usize, usize) {
        let [(left, right), (second_left, second_right)] = self.copy_bounds();
        (left.min(second_left), right.max(second_right))
    }
}

pub(crate) fn changed_line(current: &[u16], previous: &[u16], x: usize) -> Option<ChangedSpan> {
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
    let span = ChangedSpan { halves };
    let (left, right) = span.bounds();
    (left < right).then_some(span)
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
    fn changed_line_bounds_each_screen_half() {
        let previous = [0u16; 320];
        let mut current = previous;
        current[10] = 1;
        current[150] = 1;
        current[200] = 1;
        let span = changed_line(&current[4..], &previous[4..], 4).unwrap();
        assert_eq!(span.copy_bounds(), [(10, 151), (200, 201)]);
        assert_eq!(span.bounds(), (10, 201));
        let right_only = changed_line(&current[160..], &previous[160..], 160).unwrap();
        assert_eq!(right_only.bounds(), (200, 201));
        assert!(changed_line(&previous, &previous, 0).is_none());
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
