// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Recover scan wraps even when a transfer spans most of a refresh period.
pub fn wraps(elapsed_us: u32, previous: u16, current: u16, period_us: u32) -> u32 {
    let period = period_us.clamp(14_000, 20_000);
    // A near-identical positive scan is not evidence of a wrap after only half a period.
    // Recover ambiguous negative jumps only after enough elapsed time to traverse the scan.
    if previous <= current.saturating_add(100) {
        let minimum = if previous > current { period * 3 / 5 } else { period * 9 / 10 };
        if elapsed_us < minimum {
            return 0;
        }
    }
    let scale = period * 350;
    let elapsed = elapsed_us.min(1_000_000) * 350;
    let phase = (i32::from(previous) - i32::from(current)) * period as i32;
    (elapsed as i32 + phase + (scale / 2) as i32).max(0) as u32 / scale
}
#[cfg(test)]
mod tests {
    #[test]
    fn long_transfers_do_not_hide_wraps_with_small_scan_differences() {
        for actual in [16_500u32, 17_000, 17_500] {
            for gates in 320..350u32 {
                for start in (0..actual).step_by(211) {
                    for elapsed in [20, 500, 4000, 8000, 16000, 17000, 30000, 50000, 100000] {
                        let old = start * gates / actual;
                        let new = ((start + elapsed) % actual) * gates / actual;
                        assert_eq!(
                            super::wraps(elapsed, old as u16, new as u16, 17000),
                            (start + elapsed) / actual,
                            "period={actual} gates={gates} start={start} elapsed={elapsed}"
                        );
                    }
                }
            }
        }
        assert_eq!(super::wraps(16500, 330, 320, 17000), 1);
        assert_eq!(super::wraps(9226, 221, 223, 17000), 0);
        assert_eq!(super::wraps(10822, 184, 185, 17000), 0);
        assert_eq!(super::wraps(74, 182, 1, 17000), 1);
    }
}
