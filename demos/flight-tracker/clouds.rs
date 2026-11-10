// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore equirectangular Perlin xorshift

//! Clouds for the globe, generated from noise at startup and shaped like the earth's weather:
//! a belt along the equator, clear subtropics, and storm tracks in the mid-latitudes, stretched
//! from west to east like the jet streams pull them.

use std::f32::consts::PI;

use glam::Vec3;

use crate::background::fill_rows;
use crate::camera::{smoothstep, surface};
use crate::renderer::Imagery;

// The web build generates them on one thread, so at half the resolution.
#[cfg(not(target_arch = "wasm32"))]
pub const WIDTH: u32 = 2048;
#[cfg(target_arch = "wasm32")]
pub const WIDTH: u32 = 1024;
pub const HEIGHT: u32 = WIDTH / 2;

/// The cloud cover, equirectangular like the map, from 0 for a clear sky.
pub fn generate() -> Imagery {
    let noise = Noise::new(0x5eed_c10d);
    let (width, height) = (WIDTH as usize, HEIGHT as usize);
    let mut pixels = vec![0u8; width * height];
    fill_rows(&mut pixels, width, |first_row, rows| {
        for (k, value) in rows.iter_mut().enumerate() {
            let (x, y) = (k % width, first_row + k / width);
            let lon = (x as f32 + 0.5) / width as f32 * 2.0 * PI - PI;
            let lat = PI / 2.0 - (y as f32 + 0.5) / height as f32 * PI;
            *value = (cover(&noise, lon, lat) * 255.0).round() as u8;
        }
    });
    Imagery::gray(WIDTH, HEIGHT, pixels)
}

/// How cloudy it is at `lon` and `lat`, in radians, from 0 to 1.
fn cover(noise: &Noise, lon: f32, lat: f32) -> f32 {
    // Squeezed from north to south, so that the clouds stretch from west to east.
    let p = surface(lon, lat) * Vec3::new(3.2, 5.6, 3.2);
    // Warping the noise by more noise curls the clouds into swirls.
    let warp = Vec3::new(
        noise.fbm(p + Vec3::new(1.7, 9.2, 3.1), 2),
        noise.fbm(p + Vec3::new(8.3, 2.8, 5.6), 2),
        noise.fbm(p + Vec3::new(4.4, 6.1, 7.9), 2),
    );
    let shape = noise.fbm(p + warp * 0.6, 6);
    let degrees = lat.to_degrees();
    let band = |center: f32, width: f32| (-((degrees.abs() - center) / width).powi(2)).exp();
    let tropics = (-((degrees - 6.0) / 7.0).powi(2)).exp();
    let weather = 0.16 * tropics - 0.17 * band(24.0, 9.0) + 0.12 * band(55.0, 14.0) + 0.03;
    let cover = smoothstep(0.0, 1.0, (shape + weather - 0.05) / 0.28);
    // Finer noise inside the clouds, so that they aren't flat.
    let texture = 0.72 + 0.28 * (noise.fbm(p * 4.0 + Vec3::splat(11.0), 2) * 2.0).clamp(-1.0, 1.0);
    cover * texture
}

/// Ken Perlin's improved gradient noise, in three dimensions so that it wraps around the globe
/// without a seam.
struct Noise {
    permutation: [u8; 512],
}

impl Noise {
    fn new(mut seed: u64) -> Self {
        let mut values: [u8; 256] = std::array::from_fn(|i| i as u8);
        for i in (1..256).rev() {
            // xorshift64
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            values.swap(i, (seed % (i as u64 + 1)) as usize);
        }
        Self { permutation: std::array::from_fn(|i| values[i % 256]) }
    }

    fn sample(&self, p: Vec3) -> f32 {
        let cell = p.floor();
        let f = p - cell;
        let (x, y, z) = (cell.x as i32 & 255, cell.y as i32 & 255, cell.z as i32 & 255);
        let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
        let (u, v, w) = (fade(f.x), fade(f.y), fade(f.z));
        let p = |i: i32| self.permutation[i as usize] as i32;
        let (a, b) = (p(x) + y, p(x + 1) + y);
        let (aa, ab, ba, bb) = (p(a) + z, p(a + 1) + z, p(b) + z, p(b + 1) + z);
        let grad = |hash: i32, x: f32, y: f32, z: f32| {
            let h = hash & 15;
            let u = if h < 8 { x } else { y };
            let v = if h < 4 {
                y
            } else if h == 12 || h == 14 {
                x
            } else {
                z
            };
            (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
        };
        let lerp = |t: f32, a: f32, b: f32| a + t * (b - a);
        lerp(
            w,
            lerp(
                v,
                lerp(u, grad(p(aa), f.x, f.y, f.z), grad(p(ba), f.x - 1.0, f.y, f.z)),
                lerp(u, grad(p(ab), f.x, f.y - 1.0, f.z), grad(p(bb), f.x - 1.0, f.y - 1.0, f.z)),
            ),
            lerp(
                v,
                lerp(
                    u,
                    grad(p(aa + 1), f.x, f.y, f.z - 1.0),
                    grad(p(ba + 1), f.x - 1.0, f.y, f.z - 1.0),
                ),
                lerp(
                    u,
                    grad(p(ab + 1), f.x, f.y - 1.0, f.z - 1.0),
                    grad(p(bb + 1), f.x - 1.0, f.y - 1.0, f.z - 1.0),
                ),
            ),
        )
    }

    /// Octaves of noise, each at twice the frequency and half the amplitude of the one before.
    fn fbm(&self, p: Vec3, octaves: u32) -> f32 {
        let (mut sum, mut amplitude, mut frequency) = (0.0, 0.5, 1.0);
        for _ in 0..octaves {
            sum += amplitude * self.sample(p * frequency);
            amplitude *= 0.5;
            frequency *= 2.03;
        }
        sum
    }
}

#[cfg(test)]
mod tests {
    /// Writes the clouds to the PNG file that `FLIGHT_TRACKER_CLOUDS_PNG` names, to look at them.
    #[test]
    fn write_png() {
        let Ok(path) = std::env::var("FLIGHT_TRACKER_CLOUDS_PNG") else { return };
        let start = std::time::Instant::now();
        let clouds = super::generate();
        eprintln!("generated in {:?}", start.elapsed());
        image::GrayImage::from_raw(super::WIDTH, super::HEIGHT, clouds.levels[0].clone())
            .unwrap()
            .save(path)
            .unwrap();
    }
}
