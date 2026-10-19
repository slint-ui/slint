// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pub(super) fn idle_ripples(tick: u64, index: usize) -> [i16; 2] {
    core::array::from_fn(|mode| {
        let period = [73, 107][mode] + index as u32 * [11, 13][mode];
        let phase = (tick as u32).wrapping_add(super::seed(mode + 17, index));
        let segment = phase / period;
        let t = phase % period * 256 / period;
        let weight = t * t * (768 - 2 * t) >> 16;
        let sample = |segment| (super::seed(segment as usize, index + mode * 4) >> 24) as i32 - 128;
        ((sample(segment) * (256 - weight) as i32 + sample(segment + 1) * weight as i32)
            * [140, 39][mode]
            / 32768) as i16
    })
}

#[derive(Default)]
pub(super) struct Slosh {
    angle: f32,
    velocity: f32,
    initialized: bool,
    ripple: f32,
    ripple_velocity: f32,
}

impl Slosh {
    pub fn ripple_q8(&self) -> i16 {
        libm::roundf(self.ripple * 256.) as i16
    }

    pub fn update(
        &mut self,
        target: [i16; 2],
        elapsed: u32,
        index: usize,
        depth: usize,
        width: usize,
    ) -> [i16; 2] {
        const LIMIT: f32 = core::f32::consts::PI / 6.;
        const DT: f32 = 1. / 60.;
        let goal = libm::atan2f(target[0] as f32, -target[1] as f32).clamp(-LIMIT, LIMIT);
        if !self.initialized {
            self.initialized = true;
            self.angle = goal;
            return target;
        }
        // Cylindrical slosh stiffness follows tanh(1.841 * depth / radius).
        let kh = (3.682 * depth as f32 / width.max(1) as f32).min(3.);
        let kh2 = kh * kh;
        let restoring = kh * (27. + kh2) / (27. + 9. * kh2);
        let frequency = 14. * libm::sqrtf(restoring.max(0.05));
        let damping = [0.68, 0.80, 0.94, 1.08][index];
        let ripple_frequency = 15. + index as f32 * 0.7;
        for _ in 0..elapsed.min(4) {
            let acceleration = frequency * frequency * (goal - self.angle)
                - 2. * damping * frequency * self.velocity;
            self.ripple_velocity += (20. * acceleration
                - ripple_frequency * ripple_frequency * self.ripple
                - 5. * self.ripple_velocity)
                * DT;
            self.ripple += self.ripple_velocity * DT;
            if self.ripple.abs() > 3. {
                self.ripple = self.ripple.clamp(-3., 3.);
                if self.ripple * self.ripple_velocity > 0. {
                    self.ripple_velocity = 0.;
                }
            }
            if self.ripple.abs() < 1. / 512. && self.ripple_velocity.abs() < 0.01 {
                self.ripple = 0.;
                self.ripple_velocity = 0.;
            }
            self.velocity += acceleration * DT;
            self.angle += self.velocity * DT;
            if self.angle.abs() > LIMIT {
                self.angle = self.angle.clamp(-LIMIT, LIMIT);
                if self.angle * self.velocity > 0. {
                    self.velocity = 0.;
                }
            }
        }
        if (goal - self.angle).abs() < 0.0005 && self.velocity.abs() < 0.003 {
            self.angle = goal;
            self.velocity = 0.;
            return target;
        }
        [
            libm::roundf(libm::sinf(self.angle) * 1024.) as i16,
            -libm::roundf(libm::cosf(self.angle) * 1024.) as i16,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vials_respond_independently_and_settle_to_the_same_gravity() {
        let mut vials: [Slosh; 4] = core::array::from_fn(|_| Slosh::default());
        let target = [350, -962];
        for (index, vial) in vials.iter_mut().enumerate() {
            vial.update([0, -1024], 0, index, 56 + index * 16, 48);
        }
        let mut response = [[0; 2]; 4];
        for _ in 0..8 {
            for (index, vial) in vials.iter_mut().enumerate() {
                response[index] = vial.update(target, 1, index, 56 + index * 16, 48);
            }
        }
        assert!(response.windows(2).all(|pair| pair[0] != pair[1]));
        assert!(response.iter().all(|up| up[0] > 0 && up[0] < target[0]));
        for _ in 0..240 {
            for (index, vial) in vials.iter_mut().enumerate() {
                assert_eq!(vial.update(target, 1, index, 56, 48)[1].signum(), -1);
            }
        }
        for (index, vial) in vials.iter_mut().enumerate() {
            assert_eq!(vial.update(target, 1, index, 56, 48), target);
        }
    }

    #[test]
    fn abrupt_reversals_remain_bounded_and_flat_settles_exactly() {
        for index in 0..4 {
            let mut vial = Slosh::default();
            vial.update([0, -1024], 0, index, 112, 48);
            for tick in 0..600 {
                let target = if tick / 13 % 2 == 0 { [512, -886] } else { [-512, -886] };
                let up = vial.update(target, 1, index, 112, 48);
                assert!(up[0].abs() <= 512 && up[1] <= -886);
                assert!(vial.angle.is_finite() && vial.velocity.is_finite());
            }
            for _ in 0..240 {
                vial.update([0, -1024], 1, index, 112, 48);
            }
            assert_eq!(vial.update([0, -1024], 1, index, 112, 48), [0, -1024]);
        }
    }

    #[test]
    fn motion_ripple_decays_after_settling() {
        for index in 0..4 {
            let mut vial = Slosh::default();
            vial.update([0, -1024], 0, index, 80, 48);
            for _ in 0..60 {
                vial.update([0, -1024], 1, index, 80, 48);
                assert_eq!(vial.ripple_q8(), 0);
            }
            let mut positive = false;
            let mut negative = false;
            let mut peak = 0;
            for _ in 0..120 {
                vial.update([350, -962], 1, index, 80, 48);
                let ripple = vial.ripple_q8();
                peak = peak.max(ripple.abs());
                positive |= ripple > 0;
                negative |= ripple < 0;
                assert!(ripple.abs() <= 768);
            }
            assert!(
                positive && negative && peak >= 384,
                "vial {index}: positive={positive}, negative={negative}, peak={peak}"
            );
            for _ in 0..240 {
                vial.update([350, -962], 1, index, 80, 48);
            }
            assert_eq!(vial.ripple_q8(), 0);
        }
    }

    #[test]
    fn idle_ripples_are_subtle_smooth_and_independent() {
        let mut previous = core::array::from_fn::<_, 4, _>(|i| idle_ripples(0, i));
        let mut changed = [0; 4];
        let mut different = [0; 3];
        for tick in 1..3600 {
            let current = core::array::from_fn::<_, 4, _>(|i| idle_ripples(tick, i));
            for index in 0..4 {
                let ripple = current[index];
                assert!(ripple[0].abs() <= 140 && ripple[1].abs() <= 39);
                let delta =
                    (ripple[0] - previous[index][0]).abs() + (ripple[1] - previous[index][1]).abs();
                assert!(delta <= 12, "vial {index}, tick {tick}: delta {delta}");
                changed[index] += usize::from(delta != 0);
            }
            for index in 0..3 {
                different[index] += usize::from(current[index] != current[index + 1]);
            }
            previous = current;
        }
        assert!(changed.into_iter().all(|count| count > 1800));
        assert!(different.into_iter().all(|count| count > 3500));
    }

    #[test]
    fn shallow_liquid_has_a_slower_restoring_response() {
        let mut shallow = Slosh::default();
        let mut deep = Slosh::default();
        shallow.update([0, -1024], 0, 0, 3, 48);
        deep.update([0, -1024], 0, 0, 100, 48);
        let mut a = [0; 2];
        let mut b = [0; 2];
        for _ in 0..8 {
            a = shallow.update([350, -962], 1, 0, 3, 48);
            b = deep.update([350, -962], 1, 0, 100, 48);
        }
        assert!(a[0] < b[0]);
    }
}
