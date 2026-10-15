// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// GSCAN starts at vertical blanking, not at visible gate zero.
pub const BACK_PORCH: u16 = 12;
pub const FRONT_PORCH: u16 = 12;
pub const TOTAL_GATES: u16 = 320 + BACK_PORCH + FRONT_PORCH;
pub const BEAM_MARGIN: u16 = 8;
const GROUP_OVERHEAD_US: u32 = 160;
pub fn after_gate(x: u16, width: u16) -> u16 {
    BACK_PORCH + x + width + BEAM_MARGIN
}

/// Keep writes clear of the next scan's fetch, as well as the preceding scan.
pub fn deadline_gate(x: u16) -> u16 {
    (BACK_PORCH + x).saturating_sub(BEAM_MARGIN)
}

/// Evaluate every ordered native gate group, including command and beam-following waits.
#[cfg_attr(target_arch = "arm", unsafe(link_section = ".data.ram_func"))]
#[inline(never)]
pub fn fits_regions(
    remaining: u32,
    period: u32,
    scan: u16,
    spi_hz: u32,
    rectangles: impl Iterator<Item = (u16, u16, u16)>,
) -> bool {
    let (mut elapsed, mut group_cost, mut group_x, mut group_width) = (0u32, 0u32, None, 0u16);
    let finish = |elapsed: &mut u32, x: u16, width: u16, cost: u32| {
        let target = after_gate(x, width);
        let wait = u32::from(target.saturating_sub(scan)) * period / u32::from(TOTAL_GATES);
        *elapsed = (*elapsed).max(wait) + cost;
        *elapsed + 700 < remaining + u32::from(deadline_gate(x)) * period / u32::from(TOTAL_GATES)
    };
    for (x, width, height) in rectangles {
        if let Some(previous) = group_x {
            if previous != x {
                if !finish(&mut elapsed, previous, group_width, group_cost) {
                    return false;
                }
                group_cost = 0;
            }
        }
        group_x = Some(x);
        group_width = width;
        // Rounding the wire clock down makes this budget conservative for other clocks too.
        let half_mhz = (spi_hz / 500_000).max(1);
        group_cost +=
            (u32::from(width) * u32::from(height) * 32).div_ceil(half_mhz) + GROUP_OVERHEAD_US;
    }
    group_x.is_none_or(|x| finish(&mut elapsed, x, group_width, group_cost))
}

/// A conservative lower bound for the next pass's first gate.
pub fn earliest_deadline(read_us: u64, scan: u16, wave_delta: u32, x: u16) -> Option<u64> {
    let gate = deadline_gate(x);
    let distance = match wave_delta {
        0 => u64::from(TOTAL_GATES.saturating_sub(scan)) + u64::from(gate),
        1 => u64::from(gate.saturating_sub(scan)),
        _ => return None,
    };
    Some(read_us + distance * 14000 / 350)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_bound_never_exceeds_an_actual_accepted_scan_deadline() {
        for period in [14000u64, 15000, 17000, 20000] {
            for gates in TOTAL_GATES..350 {
                for scan in 0..gates {
                    for x in [0u16, 1, 16, 127, 250, 319] {
                        let bound = super::earliest_deadline(0, scan, 0, x).unwrap();
                        let actual = (u64::from(gates - scan) + u64::from(deadline_gate(x)))
                            * period
                            / u64::from(gates);
                        assert!(bound <= actual);
                        let bound = super::earliest_deadline(0, scan, 1, x).unwrap();
                        let actual = u64::from(deadline_gate(x).saturating_sub(scan)) * period
                            / u64::from(gates);
                        assert!(bound <= actual);
                    }
                }
            }
        }
        assert!(super::earliest_deadline(0, 0, 2, 300).is_none());
    }
    #[test]
    fn actual_region_cost_checks_each_gate_group_instead_of_a_full_bound() {
        let sparse = [(0, 4, 8), (120, 8, 12), (300, 8, 10)];
        assert!(super::fits_regions(17000, 17000, 0, 37500000, sparse.into_iter()));
        assert!(!super::fits_regions(17000, 17000, 0, 37500000, (0..40).map(|c| (c * 8, 8, 240))));
    }
}

#[cfg(test)]
#[test]
fn blanking_offset_and_prefetch_margin_bound_both_sides_of_a_write() {
    assert_eq!(deadline_gate(0), 4);
    assert_eq!(deadline_gate(100), 104);
    assert_eq!(after_gate(0, 8), 28);
    assert_eq!(after_gate(312, 8), 340);
    assert_eq!(earliest_deadline(1000, 105, 1, 100), Some(1000));
    assert_eq!(earliest_deadline(1000, 102, 1, 100), Some(1080));
    for x in 0..320 {
        assert!(after_gate(x, 1) > BACK_PORCH + x);
        assert!(deadline_gate(x) < BACK_PORCH + x);
    }
}
