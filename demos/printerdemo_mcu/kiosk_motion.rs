// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pub const SCENE_SECONDS: u64 = 10;

pub fn scroll_offset(tick: u64, maximum: f32) -> f32 {
    let maximum = maximum.max(0.) as u32 as u64;
    if maximum == 0 {
        return 0.;
    }
    let travel = tick % (maximum * 2);
    travel.min(maximum * 2 - travel) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolling_visits_both_ends_with_whole_pixel_steps() {
        for maximum in [0., 1., 117., 207.9] {
            let mut previous = 0.;
            for tick in 0..1000 {
                let offset = scroll_offset(tick, maximum);
                assert_eq!(offset.fract(), 0.);
                assert!(offset >= 0. && offset <= maximum);
                assert!((offset - previous).abs() <= 1.);
                previous = offset;
            }
        }
        assert_eq!(scroll_offset(207, 207.9), 207.);
        assert_eq!(scroll_offset(414, 207.9), 0.);
        assert_eq!(scroll_offset(u64::MAX, 0.), 0.);
    }
}
