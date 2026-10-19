// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore gyro

fn unit(v: [f32; 3]) -> Option<[f32; 3]> {
    let length = libm::sqrtf(v.iter().map(|x| x * x).sum());
    (length > 0.01 && length.is_finite()).then(|| v.map(|x| x / length))
}

pub struct Fusion {
    gravity: Option<[f32; 3]>,
    bias: [f32; 3],
    last_accel: [f32; 3],
    angle: f32,
}

impl Fusion {
    pub const fn new() -> Self {
        Self { gravity: None, bias: [0.; 3], last_accel: [0.; 3], angle: 0. }
    }

    pub fn update(&mut self, accel: [f32; 3], gyro: [f32; 3], dt: f32) -> [i16; 2] {
        let Some(measured) = unit(accel) else { return self.up() };
        if !gyro.iter().all(|x| x.is_finite()) || !dt.is_finite() || dt <= 0. {
            return self.up();
        }
        let norm = libm::sqrtf(accel.iter().map(|x| x * x).sum());
        let confidence = (1. - (norm - 1.).abs() / 0.25).clamp(0., 1.);
        let still = confidence > 0.8
            && gyro.iter().all(|x| x.abs() < 0.025)
            && accel.iter().zip(self.last_accel).all(|(a, b)| (a - b).abs() < 0.003);
        self.last_accel = accel;
        if still {
            let gain = dt.min(0.05) / 12.;
            for (bias, value) in self.bias.iter_mut().zip(gyro) {
                *bias += (value - *bias) * gain;
            }
        }
        let g = self.gravity.unwrap_or(measured);
        let w = core::array::from_fn::<_, 3, _>(|i| gyro[i] - self.bias[i]);
        let step = dt.min(0.05);
        let predicted = [
            g[0] + (g[1] * w[2] - g[2] * w[1]) * step,
            g[1] + (g[2] * w[0] - g[0] * w[2]) * step,
            g[2] + (g[0] * w[1] - g[1] * w[0]) * step,
        ];
        let gain = confidence * step / (0.45 + step);
        let corrected =
            core::array::from_fn(|i| predicted[i] + (measured[i] - predicted[i]) * gain);
        let g = if dt > 0.2 { measured } else { unit(corrected).unwrap_or(g) };
        self.gravity = Some(g);
        let [x, y, _] = g;
        let projection = libm::sqrtf(x * x + y * y);
        let t = ((projection - 0.14) / 0.28).clamp(0., 1.);
        let weight = t * t * (3. - 2. * t);
        let target = libm::atan2f(x, -y) * weight;
        let pi = core::f32::consts::PI;
        let error = libm::remainderf(target - self.angle, 2. * pi);
        self.angle += (error * step / (0.055 + step)).clamp(-8. * step, 8. * step);
        if weight == 0. && self.angle.abs() < 0.002 {
            self.angle = 0.;
        }
        self.up()
    }

    pub fn up(&self) -> [i16; 2] {
        [(libm::sinf(self.angle) * 1024.) as i16, (-libm::cosf(self.angle) * 1024.) as i16]
    }
}

#[cfg(any(test, feature = "box-profile"))]
pub fn replay(seconds: f32) -> ([f32; 3], [f32; 3]) {
    let rate = core::f32::consts::TAU / 6.;
    let swing = core::f32::consts::TAU / 9.;
    let theta = seconds * rate;
    let beta = core::f32::consts::FRAC_PI_4 * (1. - libm::cosf(seconds * swing));
    let db = core::f32::consts::FRAC_PI_4 * swing * libm::sinf(seconds * swing);
    let (st, ct, sb, cb) =
        (libm::sinf(theta), libm::cosf(theta), libm::sinf(beta), libm::cosf(beta));
    let a = [sb * st, -sb * ct, cb];
    let d = [db * cb * st + rate * sb * ct, -db * cb * ct + rate * sb * st, -db * sb];
    let g = [d[1] * a[2] - d[2] * a[1], d[2] * a[0] - d[0] * a[2], d[0] * a[1] - d[1] * a[0]];
    (a, g)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_does_not_replace_gravity() {
        let mut f = Fusion::new();
        for _ in 0..100 {
            f.update([0., -1., 0.], [0.; 3], 0.016);
        }
        let up = f.up();
        for _ in 0..20 {
            f.update([0., 2., 0.], [0.; 3], 0.016);
        }
        assert_eq!(f.up(), up);
    }

    #[test]
    fn replay_returns_flat_without_heading_jumps() {
        let mut f = Fusion::new();
        let mut previous = [0, -1024];
        for tick in 0..1125 {
            let (a, g) = replay(tick as f32 * 0.016);
            let up = f.update(a, g, 0.016);
            let delta = (up[0] as i32 - previous[0] as i32).abs()
                + (up[1] as i32 - previous[1] as i32).abs();
            assert!(delta < 200);
            previous = up;
        }
        for _ in 0..500 {
            f.update([0., 0., 1.], [0.; 3], 0.016);
        }
        assert_eq!(f.up(), [0, -1024]);
    }

    #[test]
    fn flat_is_upright_even_after_a_sideways_pose() {
        let mut f = Fusion::new();
        for _ in 0..500 {
            f.update([1., 0., 0.], [0.; 3], 0.016);
        }
        assert!(f.up()[0] > 1000);
        for _ in 0..500 {
            f.update([0., 0., 1.], [0.001, -0.001, 0.002], 0.016);
        }
        assert_eq!(f.up(), [0, -1024]);
    }

    #[test]
    fn gravity_corrects_bias_and_rejects_invalid_samples() {
        let mut f = Fusion::new();
        for _ in 0..4000 {
            f.update([0., -1., 0.], [0.004, 0.002, -0.003], 0.016);
        }
        assert!(f.up()[0].abs() < 8 && f.up()[1] < -1020);
        let up = f.up();
        assert_eq!(f.update([0.; 3], [0.; 3], 0.016), up);
        assert_eq!(f.update([1., 0., 0.], [f32::NAN; 3], 0.016), up);
    }

    #[test]
    fn clockwise_rotation_makes_liquid_counter_rotate() {
        let mut f = Fusion::new();
        f.update([0., -1., 0.], [0.; 3], 0.016);
        for _ in 0..30 {
            f.update([0., -1., 0.], [0., 0., 1.5], 0.016);
        }
        assert!(f.up()[0] < -100);
    }
}
