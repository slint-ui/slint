// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore hdisplay vdisplay htotal vtotal vrefresh
use super::{select_default_mode, select_requested_mode};
use drm::control::{Mode, ModeTypeFlags};

fn mode(width: u16, height: u16, refresh: u32, preferred: bool) -> Mode {
    drm_ffi::drm_mode_modeinfo {
        hdisplay: width,
        vdisplay: height,
        htotal: 2200,
        vtotal: 1125,
        clock: 2475 * refresh,
        vrefresh: refresh,
        type_: if preferred { ModeTypeFlags::PREFERRED.bits() } else { 0 },
        ..Default::default()
    }
    .into()
}

#[test]
fn index_selects_the_exact_mode() {
    let modes = [mode(1920, 1080, 60, true), mode(1280, 720, 60, false)];
    assert_eq!(select_requested_mode(&modes, "0").unwrap(), modes[0]);
    assert_eq!(select_requested_mode(&modes, "1").unwrap(), modes[1]);
    assert_eq!(select_requested_mode(&modes, "2").unwrap_err(), "Mode index is out of bounds: 2");
    assert!(select_requested_mode(&[], "0").is_err());
}

#[test]
fn resolution_matches_both_dimensions() {
    let modes =
        [mode(1920, 1080, 60, true), mode(1920, 720, 60, false), mode(1280, 720, 60, false)];
    assert_eq!(select_requested_mode(&modes, "1920x720").unwrap(), modes[1]);
    assert_eq!(select_requested_mode(&modes, "1280x720").unwrap(), modes[2]);
}

#[test]
fn preferred_mode_wins_over_higher_refresh_rate() {
    let modes = [mode(1920, 1080, 60, true), mode(1920, 1080, 144, false)];
    assert_eq!(select_requested_mode(&modes, "1920x1080").unwrap(), modes[0]);
}

#[test]
fn highest_refresh_rate_wins_without_a_preferred_mode() {
    let modes = [mode(1920, 1080, 144, false), mode(1920, 1080, 60, false)];
    assert_eq!(select_requested_mode(&modes, "1920x1080").unwrap(), modes[0]);
}

#[test]
fn highest_refresh_rate_wins_among_preferred_modes() {
    let modes =
        [mode(1920, 1080, 75, true), mode(1920, 1080, 60, true), mode(1920, 1080, 144, false)];
    assert_eq!(select_requested_mode(&modes, "1920x1080").unwrap(), modes[0]);
}

#[test]
fn refresh_rate_uses_timing_precision() {
    let sixty_hz = mode(1920, 1080, 60, false);
    let mut fractional: drm_ffi::drm_mode_modeinfo = sixty_hz.into();
    fractional.clock = 148352;
    let modes = [sixty_hz, fractional.into()];
    assert_eq!(modes[0].vrefresh(), modes[1].vrefresh());
    assert_eq!(select_requested_mode(&modes, "1920x1080").unwrap(), modes[0]);
}

#[test]
fn unavailable_resolution_returns_an_error() {
    let modes = [mode(1920, 1080, 60, true)];
    for available in [&modes[..], &[]] {
        assert_eq!(
            select_requested_mode(available, "1280x720").unwrap_err(),
            "Requested DRM mode 1280x720 is not available"
        );
    }
}

#[test]
fn malformed_requests_return_an_error() {
    let modes = [mode(1920, 1080, 60, true)];
    for request in [
        "",
        "invalid",
        "-1",
        "1920X1080",
        "1920x",
        "x1080",
        "1920x1080x60",
        "65536x1080",
        "1920x65536",
        "1920x1080@60",
    ] {
        assert!(select_requested_mode(&modes, request).is_err(), "accepted {request:?}");
    }
}

#[test]
fn preferred_mode_wins_over_the_current_mode() {
    let modes = [mode(1280, 720, 60, false), mode(1920, 1080, 60, true)];
    assert_eq!(select_default_mode(&modes, Some(modes[0])), Some(modes[1]));
}

#[test]
fn largest_preferred_mode_wins() {
    let modes = [mode(1280, 720, 60, true), mode(1920, 1080, 50, true), mode(1920, 1080, 60, true)];
    assert_eq!(select_default_mode(&modes, None), Some(modes[2]));
}

#[test]
fn current_mode_wins_without_a_preferred_mode() {
    let modes = [mode(1280, 720, 60, false), mode(1280, 720, 50, false)];
    assert_eq!(select_default_mode(&modes, Some(modes[1])), Some(modes[1]));
}

#[test]
fn current_mode_is_kept_when_the_output_does_not_list_it() {
    let modes = [mode(1920, 1080, 60, false)];
    let current = mode(1280, 800, 60, false);
    assert_eq!(select_default_mode(&modes, Some(current)), Some(current));
    assert_eq!(select_default_mode(&[], Some(current)), Some(current));
}

#[test]
fn largest_then_fastest_mode_wins_without_preferred_or_current_mode() {
    let modes = [
        mode(1920, 1080, 24, false),
        mode(1920, 1080, 60, false),
        mode(3840, 2160, 30, false),
        mode(3840, 2160, 24, false),
    ];
    assert_eq!(select_default_mode(&modes, None), Some(modes[2]));
    assert_eq!(select_default_mode(&[], None), None);
}
