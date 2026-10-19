// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pub fn candidate(tick: u32, origin_us: u32, period_us: u32, now_us: u32) -> u32 {
    if !(14_000..20_000).contains(&period_us) {
        return tick;
    }
    tick.wrapping_add(u32::from(now_us.wrapping_sub(origin_us) >= period_us))
}

pub fn advance(previous: u32, candidate: u32) -> u32 {
    if candidate.wrapping_sub(previous) < 1 << 31 { candidate } else { previous }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_scan_reads_do_not_delay_the_next_input_boundary() {
        assert_eq!(candidate(10, 1_000_000, 17_050, 1_017_049), 10);
        assert_eq!(candidate(10, 1_000_000, 17_050, 1_017_050), 11);
        assert_eq!(candidate(10, 1_000_000, 17_050, 1_020_170), 11);
        assert_eq!(candidate(11, 1_017_050, 17_050, 1_020_171), 11);
    }

    #[test]
    fn confirmation_jitter_cannot_repeat_an_input_phase() {
        let predicted = candidate(10, 1_000_000, 17_050, 1_017_050);
        assert_eq!(advance(predicted, candidate(10, 1_000_030, 17_050, 1_017_060)), 11);
        assert_eq!(advance(predicted, candidate(11, 1_017_030, 17_050, 1_017_100)), 11);
    }

    #[test]
    fn missing_measurements_allow_only_one_predicted_phase() {
        assert_eq!(candidate(10, 1_000_000, 17_050, 1_100_000), 11);
        for period in [0, 13_999, 20_000, u32::MAX] {
            assert_eq!(candidate(10, 1_000_000, period, 1_100_000), 10);
        }
    }

    #[test]
    fn timestamp_and_phase_wraps_remain_monotonic() {
        let origin = u32::MAX - 1_000;
        assert_eq!(candidate(u32::MAX, origin, 17_050, 16_049), 0);
        assert_eq!(advance(u32::MAX, 0), 0);
        assert_eq!(advance(0, u32::MAX), 0);
    }
}
