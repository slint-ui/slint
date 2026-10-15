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
        // The panel is landscape, rotated 90 degrees from the sensor frame.
        let x = -g[1];
        let y = -g[0];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_is_upright_even_after_a_sideways_pose() {
        let mut f = Fusion::new();
        for _ in 0..500 {
            f.update([0., -1., 0.], [0.; 3], 0.016);
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
            f.update([1., 0., 0.], [0.004, 0.002, -0.003], 0.016);
        }
        assert!(f.up()[0].abs() < 8 && f.up()[1] < -1020);
        let up = f.up();
        assert_eq!(f.update([0.; 3], [0.; 3], 0.016), up);
        assert_eq!(f.update([1., 0., 0.], [f32::NAN; 3], 0.016), up);
    }

    #[test]
    fn gyro_predicts_rotation_before_accelerometer_correction() {
        let mut f = Fusion::new();
        f.update([1., 0., 0.], [0.; 3], 0.016);
        for _ in 0..30 {
            f.update([1., 0., 0.], [0., 0., 1.5], 0.016);
        }
        assert!(f.up()[0] > 100);
    }
}
