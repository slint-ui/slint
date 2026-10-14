// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::{vec, vec::Vec};

pub const BACKGROUND: [u8; 3] = [5, 9, 20];
pub const COLORS: [[u8; 3]; 3] = [[255, 156, 16], [255, 56, 108], [20, 192, 255]];
const BLACK_GLOW: [[u8; 3]; 3] = [[218, 234, 255], [94, 203, 236], [203, 157, 224]];
pub const PARTICLES: usize = 40;

pub struct FrameState {
    size: (usize, usize),
    level: Option<u16>,
    bounds: [[u16; 4]; PARTICLES],
}

impl Default for FrameState {
    fn default() -> Self {
        Self { size: (0, 0), level: None, bounds: [[0; 4]; PARTICLES] }
    }
}

fn vertical_distance(phase: u32, seed: u32, speed: u32, travel: u32) -> u32 {
    (seed % travel + phase * speed) % travel
}

pub trait Pixel: Copy {
    fn word(self) -> u16;
    fn from_word(value: u16) -> Self;
    fn copy_background(pixels: &mut [Self], background: &[u16]) {
        for (pixel, &value) in pixels.iter_mut().zip(background) {
            *pixel = Self::from_word(value);
        }
    }
}

impl Pixel for u16 {
    fn word(self) -> u16 {
        self
    }
    fn from_word(value: u16) -> Self {
        value
    }
    fn copy_background(pixels: &mut [Self], background: &[u16]) {
        pixels.copy_from_slice(background);
    }
}

const fn rgb(color: [u8; 3]) -> u16 {
    ((color[0] as u16 >> 3) << 11) | ((color[1] as u16 >> 2) << 5) | (color[2] as u16 >> 3)
}

fn blend(a: [u8; 3], b: [u8; 3], weight: u32) -> [u8; 3] {
    core::array::from_fn(|i| {
        ((a[i] as u32 * (255 - weight) + b[i] as u32 * weight + 127) / 255) as u8
    })
}

fn wave(phase: u32) -> i32 {
    let t = ((phase & 16383) >> 4) as i32;
    let x = match (phase >> 14) & 3 {
        0 => t,
        1 => 1024 - t,
        2 => -t,
        _ => t - 1024,
    };
    x * (2048 - x.abs()) >> 10
}

fn seed(index: usize, vial: usize) -> u32 {
    let mut x =
        (index as u32 + 1).wrapping_mul(0x9e3779b9) ^ (vial as u32 + 1).wrapping_mul(0x85ebca6b);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^ (x >> 15)
}

const fn glow_atlas() -> [u8; 52224] {
    let mut atlas = [0; 52224];
    let mut radius = 1i32;
    let mut offset = 0usize;
    while radius <= 5 {
        let width = (radius * 2 + 1) as usize;
        let r2 = radius * radius * 256;
        let mut phase = 0;
        while phase < 256 {
            let mut y = 0;
            while y < width {
                let dy = (y as i32 - radius) * 16 + 8 - (phase >> 4) as i32;
                let mut x = 0;
                while x < width {
                    let dx = (x as i32 - radius) * 16 + 8 - (phase & 15) as i32;
                    let distance = r2 - dx * dx - dy * dy;
                    atlas[offset + phase * width * width + y * width + x] =
                        if distance > 0 { (distance * 32 / r2) as u8 } else { 0 };
                    x += 1;
                }
                y += 1;
            }
            phase += 1;
        }
        offset += 256 * width * width;
        radius = if radius == 3 { 5 } else { radius + 1 };
    }
    atlas
}

static GLOW: [u8; 52224] = glow_atlas();

fn add(dst: u16, source: u16) -> u16 {
    let rb = (dst as u32 & 0xf81f) + (source as u32 & 0xf81f);
    let overflow = rb & 0x10020;
    let rb = (rb & 0xf81f) | (overflow - (overflow >> 5));
    let g = (dst as u32 & 0x07e0) + (source as u32 & 0x07e0);
    let overflow = g & 0x0800;
    (rb | (g & 0x07e0) | (overflow - (overflow >> 6))) as u16
}

fn blend_word(dst: u16, source: u16, alpha: u32) -> u16 {
    let red = ((dst >> 11) as u32 * (32 - alpha) + (source >> 11) as u32 * alpha) >> 5;
    let green = ((dst >> 5 & 63) as u32 * (32 - alpha) + (source >> 5 & 63) as u32 * alpha) >> 5;
    let blue = ((dst & 31) as u32 * (32 - alpha) + (source & 31) as u32 * alpha) >> 5;
    ((red << 11) | (green << 5) | blue) as u16
}

pub struct Vial {
    pub width: usize,
    pub height: usize,
    index: usize,
    background: Vec<u16>,
    empty: Vec<u16>,
    liquid: Vec<u16>,
    meniscus: Vec<[u8; 2]>,
    mask: Vec<u8>,
    rim: Vec<u8>,
    surface: usize,
    level: u16,
    colors: [[u16; 33]; 3],
}

impl Vial {
    pub fn new(width: usize, height: usize, index: usize) -> Self {
        assert!((8..=512).contains(&width) && (24..=512).contains(&height));
        assert!(index < 4);
        let colors = (if index == 3 { BLACK_GLOW } else { COLORS }).map(|color| {
            core::array::from_fn(|a| {
                let hot = blend(color, [255; 3], (a as u32 * a as u32 / 5).min(128));
                rgb(hot.map(|v| ((v as u32 * a as u32 + 16) / 32) as u8))
            })
        });
        let mut vial = Self {
            width,
            height,
            index,
            background: vec![0; width * height],
            empty: vec![0; width * height],
            liquid: vec![0; width * height],
            meniscus: vec![[0; 2]; width * 7],
            mask: vec![0; width * height],
            rim: vec![0; width * height],
            surface: height,
            level: u16::MAX,
            colors,
        };
        for y in 0..height {
            for x in 0..width {
                let mut inside = 0;
                let mut inner = 0;
                for sy in [2, 6, 10, 14] {
                    for sx in [2, 6, 10, 14] {
                        let radius = width.min(height) as i32 * 7 - 8;
                        let px = x as i32 * 16 + sx;
                        let dx = px - px.clamp(radius + 8, width as i32 * 16 - radius - 8);
                        let py = y as i32 * 16 + sy;
                        let cy = py.clamp(radius + 8, height as i32 * 16 - radius - 8);
                        let dy = py - cy;
                        let distance = dx * dx + dy * dy;
                        if distance <= radius * radius {
                            inside += 1;
                        }
                        if distance <= (radius - 20) * (radius - 20) {
                            inner += 1;
                        }
                    }
                }
                vial.mask[y * width + x] = (inside * 255 / 16) as u8;
                vial.rim[y * width + x] = ((inside - inner) * 255 / 16) as u8;
            }
        }
        for y in 0..7 {
            for x in 0..width {
                let (mut outer, mut inner) = (0, 0);
                let rx = width as i64 * 8 - 16;
                let ry = 48i64;
                for sy in [2, 6, 10, 14] {
                    for sx in [2, 6, 10, 14] {
                        let dx = x as i64 * 16 + sx - width as i64 * 8;
                        let dy = y as i64 * 16 + sy - 56;
                        if dx * dx * ry * ry + dy * dy * rx * rx <= rx * rx * ry * ry {
                            outer += 1;
                        }
                        let (ix, iy) = (rx - 12, ry - 12);
                        if dx * dx * iy * iy + dy * dy * ix * ix <= ix * ix * iy * iy {
                            inner += 1;
                        }
                    }
                }
                vial.meniscus[y * width + x] =
                    [(outer * 255 / 16) as u8, ((outer - inner) * 255 / 16) as u8];
            }
        }
        for y in 0..height {
            for x in 0..width {
                let at = y * width + x;
                let tint = vial.tint(x);
                let dx = (x as i32 * 2 - width as i32 + 1).unsigned_abs();
                let shade = dx * dx * 100 / (width * width) as u32;
                let glass = blend([7, 13, 24], [44, 62, 84], shade);
                let mut light = (170i32 - (x as i32 - 5).abs() * 34).max(0) as u32;
                light = light * (height * 4 - y * 3) as u32 / (height * 4) as u32;
                let rim = vial.rim[at] as u32;
                let right = (82i32 - (x as i32 - width as i32 + 4).abs() * 30).max(0) as u32;
                let top_x = x as i32 - width as i32 / 2;
                let arch = 3 + top_x * top_x * 6 / (width * width / 4) as i32;
                let cap = (230i32 - (y as i32 - arch).abs() * 90).max(0) as u32;
                let cap = cap * (width as i32 / 3 - top_x.abs()).max(0) as u32 / (width / 3) as u32;
                let polish = |color| {
                    let color = blend(color, blend(tint, [255; 3], 65), rim);
                    let color = blend(color, [180, 207, 234], light);
                    let color = blend(color, [174, 207, 240], right);
                    let color = blend(color, [231, 240, 255], cap);
                    rgb(blend(BACKGROUND, color, vial.mask[at] as u32))
                };
                vial.empty[at] = polish(glass);
                let liquid = if index == 3 {
                    blend([3, 6, 11], tint, 9 + (height - y) as u32 * 6 / height as u32)
                } else {
                    blend(
                        glass,
                        tint,
                        (55 + shade + (y * y * 100 / (height * height)) as u32).min(220),
                    )
                };
                vial.liquid[at] = polish(liquid);
            }
        }
        vial.set_level(0);
        vial
    }

    pub fn set_level(&mut self, level: u16) {
        let level = level.min(1024);
        if level == self.level {
            return;
        }
        self.level = level;
        self.surface = self.height - (self.height * level as usize / 1024);
        let split = self.surface * self.width;
        self.background[..split].copy_from_slice(&self.empty[..split]);
        self.background[split..].copy_from_slice(&self.liquid[split..]);
        for y in self.surface..(self.surface + 7).min(self.height) {
            for x in 0..self.width {
                let at = y * self.width + x;
                let [coverage, rim] = self.meniscus[(y - self.surface) * self.width + x];
                let pixel = self.background[at];
                let color = [
                    ((pixel >> 11) * 255 / 31) as u8,
                    ((pixel >> 5 & 63) * 255 / 63) as u8,
                    ((pixel & 31) * 255 / 31) as u8,
                ];
                let tint = self.tint(x);
                let plane = blend(color, tint, coverage as u32 / 3);
                let ring = blend(plane, blend(tint, [255; 3], 128), rim as u32);
                self.background[at] = rgb(blend(color, ring, self.mask[at] as u32));
            }
        }
    }

    fn tint(&self, x: usize) -> [u8; 3] {
        if self.index == 3 {
            let accent = if x * 2 < self.width { [75, 185, 211] } else { [197, 119, 188] };
            blend([196, 213, 235], accent, 35)
        } else {
            COLORS[self.index]
        }
    }

    pub fn draw<P: Pixel>(&self, tick: u64, pixels: &mut [P]) {
        self.restore(pixels);
        self.stamp(tick, pixels);
    }

    pub fn restore<P: Pixel>(&self, pixels: &mut [P]) {
        assert_eq!(pixels.len(), self.background.len());
        P::copy_background(pixels, &self.background);
    }

    pub fn restore_cached<P: Pixel>(&self, pixels: &mut [P], state: &mut FrameState) {
        assert_eq!(pixels.len(), self.background.len());
        if state.size != (self.width, self.height) || state.level != Some(self.level) {
            self.restore(pixels);
            state.size = (self.width, self.height);
            state.level = Some(self.level);
        } else {
            for &[x, y, width, height] in &state.bounds {
                for row in y as usize..(y + height) as usize {
                    let start = row * self.width + x as usize;
                    let end = start + width as usize;
                    P::copy_background(&mut pixels[start..end], &self.background[start..end]);
                }
            }
        }
        state.bounds.fill([0; 4]);
    }

    pub fn stamp_cached<P: Pixel>(&self, tick: u64, pixels: &mut [P], state: &mut FrameState) {
        state.bounds = self.stamp(tick, pixels);
    }

    pub fn stamp<P: Pixel>(&self, tick: u64, pixels: &mut [P]) -> [[u16; 4]; PARTICLES] {
        let mut bounds = [[0; 4]; PARTICLES];
        if self.height.saturating_sub(self.surface) < 8 {
            return bounds;
        }
        let population = PARTICLES as u32 * self.level as u32 / 4;
        let travel_q8 = (self.height - self.surface - 6) as u32 * 256;
        let phase = (tick % travel_q8 as u64) as u32;
        for index in 0..self.particle_count() {
            let seed = seed(index, self.index);
            let radius = if index % 8 == 0 {
                5
            } else if seed & 7 == 0 {
                3
            } else if seed & 3 != 0 {
                2
            } else {
                1
            };
            let margin = 5 + radius;
            let span = self.width.saturating_sub(margin * 2).max(1);
            let home = margin + ((seed >> 8) as usize % span);
            let cx = home as i32 * 16
                + (wave(seed.wrapping_add((tick as u32).wrapping_mul(170))) * 32 >> 10);
            let speed_q8 = 48 + radius as u32 * 14 + ((seed >> 24) & 15);
            let distance = vertical_distance(phase, seed, speed_q8, travel_q8);
            let cy = (self.height as i32 - 4) * 16 - (distance >> 4) as i32;
            let visibility = population.saturating_sub(index as u32 * 256).min(256);
            let opacity =
                (distance.min(travel_q8 - distance) * 32 / 768).min(32) * visibility / 256;
            let width = radius * 2 + 1;
            let phase = ((cy & 15) as usize * 16 + (cx & 15) as usize) * width * width;
            let offset = match radius {
                1 => 0,
                2 => 2304,
                3 => 8704,
                _ => 21248,
            } + phase;
            let color = if self.index == 3 {
                match index % 5 {
                    1 => 1,
                    2 => 2,
                    _ => 0,
                }
            } else {
                self.index
            };
            let x0 = (cx >> 4) - radius as i32;
            let y0 = (cy >> 4) - radius as i32;
            let left = x0.max(0).min(self.width as i32);
            let top = y0.max(self.surface as i32 + 2).min(self.height as i32);
            let right = (x0 + width as i32).max(left).min(self.width as i32);
            let bottom = (y0 + width as i32).max(top).min(self.height as i32);
            bounds[index] = [left as u16, top as u16, (right - left) as u16, (bottom - top) as u16];
            for y in 0..width {
                let py = y0 + y as i32;
                if py < self.surface as i32 + 2 || py < 0 || py >= self.height as i32 {
                    continue;
                }
                for x in 0..width {
                    let px = x0 + x as i32;
                    if px < 0 || px >= self.width as i32 {
                        continue;
                    }
                    let at = py as usize * self.width + px as usize;
                    let coverage = GLOW[offset + y * width + x] as u32;
                    if coverage != 0 {
                        let mask = self.mask[at] as u32;
                        let body = (coverage * opacity * mask + 4080) / 8160;
                        let pixel = blend_word(pixels[at].word(), rgb([3, 7, 13]), body.min(32));
                        let dx = px * 16 + 8 - cx;
                        let dy = py * 16 + 8 - cy;
                        let shine = ((-dx - dy) * 8 / radius as i32 + 12).clamp(0, 32) as u32;
                        let ring = (coverage * (32 - coverage) * opacity + 128) / 256;
                        let rim = (ring * (8 + shine) * mask + 5100) / 10200;
                        pixels[at] =
                            P::from_word(add(pixel, self.colors[color][rim.min(32) as usize]));
                        continue;
                    }
                }
            }
        }
        bounds
    }

    pub fn particle_count(&self) -> usize {
        (PARTICLES as usize * self.level as usize).div_ceil(1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saturating_add_matches_component_reference() {
        for a in (0..=65535).step_by(131) {
            for b in (0..=65535).step_by(257) {
                let expected = ((((a >> 11) + (b >> 11)).min(31)) << 11)
                    | ((((a >> 5 & 63) + (b >> 5 & 63)).min(63)) << 5)
                    | ((a & 31) + (b & 31)).min(31);
                assert_eq!(add(a, b), expected);
            }
        }
    }
    #[test]
    fn particles_stay_inside_liquid_and_leave_glass_unchanged() {
        for index in 0..4 {
            let mut vial = Vial::new(51, 134, index);
            vial.set_level([410, 205, 512, 819][index]);
            let mut frame = vec![0; 51 * 134];
            let mut changed = 0;
            for tick in 0..240 {
                vial.draw(tick, &mut frame);
                for (at, (&pixel, &background)) in frame.iter().zip(&vial.background).enumerate() {
                    if at / 51 < vial.surface + 2 || vial.mask[at] == 0 {
                        assert_eq!(pixel, background);
                    }
                    changed += usize::from(pixel != background);
                }
            }
            assert!(changed > 1000);
        }
    }
    #[test]
    fn empty_vial_is_static_and_frame_generation_is_repeatable() {
        let mut vial = Vial::new(32, 120, 0);
        let mut a = vec![0; 3840];
        let mut b = a.clone();
        vial.draw(0, &mut a);
        vial.draw(100, &mut b);
        assert_eq!(a, b);
        vial.set_level(1024);
        vial.draw(100, &mut a);
        vial.draw(100, &mut b);
        assert_eq!(a, b);
    }

    #[test]
    fn cached_buffers_match_full_redraw_through_jumps_levels_and_resize() {
        for index in 0..4 {
            let mut vial = Vial::new(51, 134, index);
            let mut buffers = [vec![0; 51 * 134], vec![0; 51 * 134]];
            let mut states = [FrameState::default(), FrameState::default()];
            let mut reference = vec![0; 51 * 134];
            for step in 0..960 {
                if step % 80 == 0 {
                    vial.set_level([0, 205, 410, 819, 1024][step / 80 % 5]);
                }
                let tick = if step % 3 == 0 { 60_000 - step as u64 * 13 } else { step as u64 * 29 };
                let back = step % 2;
                vial.restore_cached(&mut buffers[back], &mut states[back]);
                vial.stamp_cached(tick, &mut buffers[back], &mut states[back]);
                vial.draw(tick, &mut reference);
                assert_eq!(buffers[back], reference, "vial {index}, step {step}");
            }
            let resized = Vial::new(32, 96, index);
            buffers[0].resize(32 * 96, 0xdead);
            reference.resize(32 * 96, 0);
            resized.restore_cached(&mut buffers[0], &mut states[0]);
            resized.stamp_cached(123, &mut buffers[0], &mut states[0]);
            resized.draw(123, &mut reference);
            assert_eq!(buffers[0], reference);
        }
    }

    #[test]
    fn density_scales_with_fill_level() {
        let mut vial = Vial::new(51, 134, 0);
        for (level, count) in [(0, 0), (204, 8), (409, 16), (512, 20), (819, 32), (1024, 40)] {
            vial.set_level(level);
            assert_eq!(vial.particle_count(), count);
        }
    }

    #[test]
    fn rising_speed_is_independent_of_liquid_height() {
        for travel in [20 * 256, 45 * 256, 90 * 256] {
            for tick in 0..240 {
                let a = vertical_distance(tick % travel, 927_423, 87, travel);
                let b = vertical_distance((tick + 1) % travel, 927_423, 87, travel);
                assert_eq!((b + travel - a) % travel, 87);
            }
        }
    }
}
