// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![cfg(test)]

extern crate alloc;
#[path = "../../../imu.rs"]
mod imu;
#[path = "../../../../../../demos/printerdemo_mcu/ink_particles.rs"]
mod ink_particles;
#[path = "../../../tilt_fusion.rs"]
mod tilt_fusion;

#[path = "../../../background_job.rs"]
mod background_job;

#[test]
fn two_core_vials_match_serial_buffers_through_hold_and_release() {
    use ink_particles::{FrameState, Paint, Vial};
    use std::sync::atomic::{AtomicBool, Ordering};
    let queue = background_job::Queue::new();
    let stop = AtomicBool::new(false);
    let mut vials = core::array::from_fn::<_, 4, _>(|index| Vial::new(48, 140, index));
    let mut parallel_vials = core::array::from_fn::<_, 4, _>(|index| Vial::new(48, 140, index));
    for v in &mut parallel_vials {
        v.advance_tilt([0, -1024], 0);
    }
    let mut expected =
        core::array::from_fn::<_, 4, _>(|_| [vec![0u16; 48 * 140], vec![0; 48 * 140]]);
    let mut actual = expected.clone();
    let mut expected_states = core::array::from_fn::<_, 4, _>(|_| {
        core::array::from_fn::<_, 2, _>(|_| FrameState::default())
    });
    let mut actual_states = core::array::from_fn::<_, 4, _>(|_| {
        core::array::from_fn::<_, 2, _>(|_| FrameState::default())
    });
    std::thread::scope(|threads| {
        threads.spawn(|| {
            while !stop.load(Ordering::Acquire) {
                queue.step();
                std::thread::yield_now();
            }
        });
        for tick in 0..900 {
            let back = tick as usize & 1;
            let up = [[0, -1024], [265, -989], [-512, -886], [1024, 0]][tick as usize / 90 % 4];
            for (index, vial) in vials.iter_mut().enumerate() {
                vial.set_level(if (400..440).contains(&tick) {
                    1024
                } else {
                    [409, 665, 512, 819][index]
                });
                vial.set_held((20..300).contains(&tick) || (600..700).contains(&tick), tick);
                let parallel = &mut parallel_vials[index];
                parallel.set_level(if (400..440).contains(&tick) {
                    1024
                } else {
                    [409, 665, 512, 819][index]
                });
                parallel.set_held((20..300).contains(&tick) || (600..700).contains(&tick), tick);
                vial.advance_tilt(up, tick);
                vial.restore_cached(&mut expected[index][back], &mut expected_states[index][back]);
                vial.stamp_cached(
                    tick,
                    &mut expected[index][back],
                    &mut expected_states[index][back],
                );
            }
            let [yv, rv, cv, bv] = &mut parallel_vials;
            let mut next = 0;
            queue.scope(
                &mut || {
                    let v = if next == 0 { &mut *yv } else { &mut *cv };
                    v.advance_tilt(up, tick);
                    next += 1;
                    next == 2
                },
                || {
                    rv.advance_tilt(up, tick);
                    bv.advance_tilt(up, tick);
                },
                std::thread::yield_now,
            );
            let [yellow, red, cyan, black] = &mut actual;
            let [ys, rs, cs, bs] = &mut actual_states;
            let mut paints = [
                Paint::new(yv, tick, &mut yellow[back], &mut ys[back]),
                Paint::new(cv, tick, &mut cyan[back], &mut cs[back]),
            ];
            let mut next = 0;
            queue.scope(
                &mut || {
                    next += usize::from(paints[next].step());
                    next == 2
                },
                || {
                    rv.restore_cached(&mut red[back], &mut rs[back]);
                    rv.stamp_cached(tick, &mut red[back], &mut rs[back]);
                    bv.restore_cached(&mut black[back], &mut bs[back]);
                    bv.stamp_cached(tick, &mut black[back], &mut bs[back]);
                },
                std::thread::yield_now,
            );
            assert_eq!(actual, expected, "tick {tick}");
        }
        stop.store(true, Ordering::Release);
    });
}
