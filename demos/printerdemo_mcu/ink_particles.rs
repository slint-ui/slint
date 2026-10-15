// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::{borrow::Cow, vec, vec::Vec};

#[path = "ink_particles/native_assets.rs"]
mod native_assets;

pub const BACKGROUND: [u8; 3] = [5, 9, 20];
pub const COLORS: [[u8; 3]; 3] = [[255, 156, 16], [255, 56, 108], [20, 192, 255]];
const BLACK_GLOW: [[u8; 3]; 3] = [[218, 234, 255], [94, 203, 236], [203, 157, 224]];
pub const PARTICLES: usize = 40;
const HELD_BOOST_PERCENT: usize = 60;
const EXTRA_PARTICLES: usize = PARTICLES * HELD_BOOST_PERCENT / 100;
const MAX_PARTICLES: usize = PARTICLES + EXTRA_PARTICLES;

#[derive(Default)]
pub struct FrameState {
    size: (usize, usize),
    level: Option<u16>,
    rows: Vec<[u16; 2]>,
}

fn particle_radius(index: usize, seed: u32) -> usize {
    if index >= PARTICLES || index % 8 == 0 {
        5
    } else if seed & 7 == 0 {
        3
    } else if seed & 3 != 0 {
        2
    } else {
        1
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

#[inline(always)]
fn particle_weights(coverage: u32, opacity: u32, mask: u32, shine: u32) -> (u32, u32) {
    let amount = coverage * opacity;
    let ring = (amount * (32 - coverage) + 128) / 256;
    if mask == 255 {
        ((amount + 16) / 32, (ring * (8 + shine) + 20) / 40)
    } else {
        ((amount * mask + 4080) / 8160, (ring * (8 + shine) * mask + 5100) / 10200)
    }
}

#[inline(always)]
fn shade_row<P: Pixel, const MASKED: bool>(
    pixels: &mut [P],
    mask: &[u8],
    colors: &[u16; 33],
    opacity: u32,
    radius: usize,
    dx: i32,
    dy: i32,
) {
    let divisor = (radius * radius * 8) as u32;
    let mut distance = (radius * radius * 256) as i32 - dx * dx - dy * dy;
    let mut delta = -32 * dx - 256;
    for (x, (destination, &mask)) in pixels.iter_mut().zip(mask).enumerate() {
        let coverage = if distance > 0 { distance as u32 / divisor } else { 0 };
        distance += delta;
        delta -= 512;
        if coverage != 0 {
            let mask = if MASKED { mask as u32 } else { 255 };
            let shine = ((-dx - x as i32 * 16 - dy) * 8 / radius as i32 + 12).clamp(0, 32) as u32;
            let (body, rim) = particle_weights(coverage, opacity, mask, shine);
            let pixel = blend_word(destination.word(), rgb([3, 7, 13]), body.min(32));
            *destination = P::from_word(add(pixel, colors[rim.min(32) as usize]));
        }
    }
}

struct StampPlan {
    tick: u64,
    population: u32,
    travel_q8: u32,
    phase: u32,
    baseline_count: usize,
    extra_population: u32,
}

impl StampPlan {
    fn new(vial: &Vial, tick: u64) -> Self {
        let travel_q8 = if vial.height.saturating_sub(vial.surface) < 8 {
            0
        } else {
            (vial.height - vial.surface - 6) as u32 * 256
        };
        let population = PARTICLES as u32 * vial.level as u32 / 4;
        Self {
            tick,
            population,
            travel_q8,
            phase: if travel_q8 == 0 { 0 } else { (tick % travel_q8 as u64) as u32 },
            baseline_count: (population as usize).div_ceil(256),
            extra_population: vial.extra_population(),
        }
    }

    fn count(&self) -> usize {
        if self.travel_q8 == 0 { 0 } else { self.baseline_count + EXTRA_PARTICLES }
    }
}

#[cfg(any(feature = "pico2", test))]
pub struct Paint<'a, P> {
    vial: &'a Vial,
    pixels: &'a mut [P],
    state: &'a mut FrameState,
    plan: StampPlan,
    row: usize,
    ordinal: usize,
}

#[cfg(any(feature = "pico2", test))]
impl<'a, P: Pixel> Paint<'a, P> {
    pub fn new(vial: &'a Vial, tick: u64, pixels: &'a mut [P], state: &'a mut FrameState) -> Self {
        assert_eq!(pixels.len(), vial.width * vial.height);
        state.rows.reserve(vial.height.saturating_sub(state.rows.len()));
        Self { vial, pixels, state, plan: StampPlan::new(vial, tick), row: 0, ordinal: 0 }
    }

    #[cfg(feature = "pico2-queue-profile")]
    pub fn restoring(&self) -> bool {
        self.row < self.vial.height
    }

    pub fn step(&mut self) -> bool {
        if self.row < self.vial.height {
            let end = (self.row + 8).min(self.vial.height);
            self.vial.restore_rows(self.pixels, self.state, self.row..end);
            self.row = end;
            if end == self.vial.height {
                self.state.size = (self.vial.width, self.vial.height);
                self.state.level = Some(self.vial.level);
                self.state.rows.clear();
                self.state.rows.resize(self.vial.height, [self.vial.width as u16, 0]);
            }
        } else if self.ordinal < self.plan.count() {
            if let Some((_, [x, y, width, height])) =
                self.vial.stamp_particle(&self.plan, self.ordinal, self.pixels)
            {
                if width != 0 {
                    for row in &mut self.state.rows[y as usize..(y + height) as usize] {
                        row[0] = row[0].min(x);
                        row[1] = row[1].max(x + width);
                    }
                }
            }
            self.ordinal += 1;
        }
        self.row == self.vial.height && self.ordinal == self.plan.count()
    }
}

pub struct Vial {
    pub width: usize,
    pub height: usize,
    index: usize,
    surface_pixels: Vec<u16>,
    empty: Cow<'static, [u16]>,
    liquid: Cow<'static, [u16]>,
    meniscus: Vec<[u8; 2]>,
    mask: Cow<'static, [u8]>,
    opaque_rows: Vec<[u16; 2]>,
    surface: usize,
    level: u16,
    held: bool,
    extra_births: [Option<u32>; EXTRA_PARTICLES],
    colors: [[u16; 33]; 3],
}

impl Vial {
    pub fn new(width: usize, height: usize, index: usize) -> Self {
        Self::build(width, height, index, true)
    }

    fn build(width: usize, height: usize, index: usize, use_assets: bool) -> Self {
        assert!((8..=512).contains(&width) && (24..=512).contains(&height));
        assert!(index < 4);
        let colors = (if index == 3 { BLACK_GLOW } else { COLORS }).map(|color| {
            core::array::from_fn(|a| {
                let hot = blend(color, [255; 3], (a as u32 * a as u32 / 5).min(128));
                rgb(hot.map(|v| ((v as u32 * a as u32 + 16) / 32) as u8))
            })
        });
        let assets = use_assets.then(|| native_assets::vial(width, height, index)).flatten();
        let mut rim = if assets.is_none() { vec![0; width * height] } else { Vec::new() };
        let mut vial = Self {
            width,
            height,
            index,
            surface_pixels: vec![0; width * 7],
            empty: assets
                .map_or_else(|| Cow::Owned(vec![0; width * height]), |a| Cow::Borrowed(a.empty)),
            liquid: assets
                .map_or_else(|| Cow::Owned(vec![0; width * height]), |a| Cow::Borrowed(a.liquid)),
            meniscus: vec![[0; 2]; width * 7],
            mask: assets
                .map_or_else(|| Cow::Owned(vec![0; width * height]), |a| Cow::Borrowed(a.mask)),
            opaque_rows: Vec::new(),
            surface: height,
            level: u16::MAX,
            held: false,
            extra_births: [None; EXTRA_PARTICLES],
            colors,
        };
        if assets.is_none() {
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
                    vial.mask.to_mut()[y * width + x] = (inside * 255 / 16) as u8;
                    rim[y * width + x] = ((inside - inner) * 255 / 16) as u8;
                }
            }
        }
        vial.opaque_rows = vial
            .mask
            .chunks_exact(width)
            .map(|row| {
                let left = row.iter().position(|&mask| mask == 255).unwrap_or(width);
                let right = left + row[left..].iter().take_while(|&&mask| mask == 255).count();
                [left as u16, right as u16]
            })
            .collect();
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
        if assets.is_none() {
            for y in 0..height {
                for x in 0..width {
                    let at = y * width + x;
                    let tint = vial.tint(x);
                    let dx = (x as i32 * 2 - width as i32 + 1).unsigned_abs();
                    let shade = dx * dx * 100 / (width * width) as u32;
                    let glass = blend([7, 13, 24], [44, 62, 84], shade);
                    let mut light = (170i32 - (x as i32 - 5).abs() * 34).max(0) as u32;
                    light = light * (height * 4 - y * 3) as u32 / (height * 4) as u32;
                    let rim = rim[at] as u32;
                    let right = (82i32 - (x as i32 - width as i32 + 4).abs() * 30).max(0) as u32;
                    let top_x = x as i32 - width as i32 / 2;
                    let arch = 3 + top_x * top_x * 6 / (width * width / 4) as i32;
                    let cap = (230i32 - (y as i32 - arch).abs() * 90).max(0) as u32;
                    let cap =
                        cap * (width as i32 / 3 - top_x.abs()).max(0) as u32 / (width / 3) as u32;
                    let polish = |color| {
                        let color = blend(color, blend(tint, [255; 3], 65), rim);
                        let color = blend(color, [180, 207, 234], light);
                        let color = blend(color, [174, 207, 240], right);
                        let color = blend(color, [231, 240, 255], cap);
                        rgb(blend(BACKGROUND, color, vial.mask[at] as u32))
                    };
                    vial.empty.to_mut()[at] = polish(glass);
                    let liquid = if index == 3 {
                        blend([3, 6, 11], tint, 9 + (height - y) as u32 * 6 / height as u32)
                    } else {
                        blend(
                            glass,
                            tint,
                            (55 + shade + (y * y * 100 / (height * height)) as u32).min(220),
                        )
                    };
                    vial.liquid.to_mut()[at] = polish(liquid);
                }
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
        for y in self.surface..(self.surface + 7).min(self.height) {
            for x in 0..self.width {
                let at = y * self.width + x;
                let [coverage, rim] = self.meniscus[(y - self.surface) * self.width + x];
                let pixel = self.liquid[at];
                let color = [
                    ((pixel >> 11) * 255 / 31) as u8,
                    ((pixel >> 5 & 63) * 255 / 63) as u8,
                    ((pixel & 31) * 255 / 31) as u8,
                ];
                let tint = self.tint(x);
                let plane = blend(color, tint, coverage as u32 / 3);
                let ring = blend(plane, blend(tint, [255; 3], 128), rim as u32);
                self.surface_pixels[(y - self.surface) * self.width + x] =
                    rgb(blend(color, ring, self.mask[at] as u32));
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

    #[cfg(test)]
    pub fn draw<P: Pixel>(&self, tick: u64, pixels: &mut [P]) {
        self.restore(pixels);
        self.stamp(tick, pixels);
    }

    pub fn restore<P: Pixel>(&self, pixels: &mut [P]) {
        assert_eq!(pixels.len(), self.width * self.height);
        let split = self.surface * self.width;
        let end = (self.surface + 7).min(self.height) * self.width;
        P::copy_background(&mut pixels[..split], &self.empty[..split]);
        P::copy_background(&mut pixels[split..end], &self.surface_pixels[..end - split]);
        P::copy_background(&mut pixels[end..], &self.liquid[end..]);
    }

    #[cfg_attr(
        all(feature = "pico2", target_arch = "arm"),
        unsafe(link_section = ".data.ram_func")
    )]
    #[cfg_attr(all(feature = "pico2", target_arch = "arm"), inline(never))]
    pub fn restore_cached<P: Pixel>(&self, pixels: &mut [P], state: &mut FrameState) {
        assert_eq!(pixels.len(), self.width * self.height);
        self.restore_rows(pixels, state, 0..self.height);
        state.size = (self.width, self.height);
        state.level = Some(self.level);
        state.rows.clear();
    }

    #[inline(always)]
    fn restore_rows<P: Pixel>(
        &self,
        pixels: &mut [P],
        state: &FrameState,
        rows: core::ops::Range<usize>,
    ) {
        if state.size != (self.width, self.height) || state.level != Some(self.level) {
            let start = rows.start * self.width;
            let end = rows.end * self.width;
            let split = self.surface * self.width;
            let surface_end = (self.surface + 7).min(self.height) * self.width;
            let empty_end = end.min(split);
            if start < empty_end {
                P::copy_background(&mut pixels[start..empty_end], &self.empty[start..empty_end]);
            }
            let surface_start = start.max(split);
            let surface_end_clipped = end.min(surface_end);
            if surface_start < surface_end_clipped {
                P::copy_background(
                    &mut pixels[surface_start..surface_end_clipped],
                    &self.surface_pixels[surface_start - split..surface_end_clipped - split],
                );
            }
            let liquid_start = start.max(surface_end);
            if liquid_start < end {
                P::copy_background(&mut pixels[liquid_start..end], &self.liquid[liquid_start..end]);
            }
        } else {
            for row in rows {
                let [left, right] = state.rows.get(row).copied().unwrap_or([0; 2]);
                if left < right {
                    let start = row * self.width + left as usize;
                    let end = row * self.width + right as usize;
                    let background = if row < self.surface {
                        &self.empty[start..end]
                    } else if row < self.surface + 7 {
                        let offset = (row - self.surface) * self.width + left as usize;
                        &self.surface_pixels[offset..offset + (right - left) as usize]
                    } else {
                        &self.liquid[start..end]
                    };
                    P::copy_background(&mut pixels[start..end], background);
                }
            }
        }
    }

    pub fn stamp_cached<P: Pixel>(&self, tick: u64, pixels: &mut [P], state: &mut FrameState) {
        let bounds = self.stamp(tick, pixels);
        state.rows.clear();
        state.rows.resize(self.height, [self.width as u16, 0]);
        for [x, y, width, height] in bounds {
            if width != 0 {
                for row in &mut state.rows[y as usize..(y + height) as usize] {
                    row[0] = row[0].min(x);
                    row[1] = row[1].max(x + width);
                }
            }
        }
    }

    #[cfg_attr(
        all(feature = "pico2", target_arch = "arm"),
        unsafe(link_section = ".data.ram_func")
    )]
    #[cfg_attr(all(feature = "pico2", target_arch = "arm"), inline(never))]
    pub fn stamp<P: Pixel>(&self, tick: u64, pixels: &mut [P]) -> [[u16; 4]; MAX_PARTICLES] {
        let mut bounds = [[0; 4]; MAX_PARTICLES];
        let plan = StampPlan::new(self, tick);
        for ordinal in 0..plan.count() {
            if let Some((index, area)) = self.stamp_particle(&plan, ordinal, pixels) {
                bounds[index] = area;
            }
        }
        bounds
    }

    #[cfg_attr(
        all(feature = "pico2", target_arch = "arm"),
        unsafe(link_section = ".data.ram_func")
    )]
    #[inline(never)]
    fn stamp_particle<P: Pixel>(
        &self,
        plan: &StampPlan,
        ordinal: usize,
        pixels: &mut [P],
    ) -> Option<(usize, [u16; 4])> {
        let StampPlan { tick, population, travel_q8, phase, baseline_count, extra_population } =
            *plan;
        let extra = ordinal.checked_sub(baseline_count);
        let index = extra.map_or(ordinal, |extra| PARTICLES + extra);
        let age = if let Some(extra) = extra {
            let Some(born) = self.extra_births[extra] else { return None };
            let age = (tick as u32).wrapping_sub(born);
            if age > i32::MAX as u32 {
                return None;
            }
            Some(age)
        } else {
            None
        };
        let seed = seed(index, self.index);
        let radius = particle_radius(index, seed);
        let margin = 5 + radius;
        let span = self.width.saturating_sub(margin * 2).max(1);
        let home = margin + ((seed >> 8) as usize % span);
        let cx = home as i32 * 16
            + (wave(seed.wrapping_add((tick as u32).wrapping_mul(170))) * 32 >> 10);
        let speed_q8 = 48 + radius as u32 * 14 + ((seed >> 24) & 15);
        let (distance, visibility) = if let Some(elapsed) = age {
            let extra = ordinal - baseline_count;
            let distance = elapsed.saturating_mul(speed_q8);
            if distance >= travel_q8 {
                return None;
            }
            (distance, extra_population.saturating_sub(extra as u32 * 256).min(256))
        } else {
            (
                vertical_distance(phase, seed, speed_q8, travel_q8),
                population.saturating_sub(index as u32 * 256).min(256),
            )
        };
        let cy = (self.height as i32 - 4) * 16 - (distance >> 4) as i32;
        let opacity = (distance.min(travel_q8 - distance) * 32 / 768).min(32) * visibility / 256;
        let width = radius * 2 + 1;
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
        let bounds = [left as u16, top as u16, (right - left) as u16, (bottom - top) as u16];
        if left == right || top == bottom {
            return Some((index, bounds));
        }
        let row_len = (right - left) as usize;
        for py in top..bottom {
            let start = py as usize * self.width + left as usize;
            let row = &mut pixels[start..start + row_len];
            let mask = &self.mask[start..start + row_len];
            let opaque = self.opaque_rows[py as usize];
            let dx = left * 16 + 8 - cx;
            let dy = py * 16 + 8 - cy;
            if left >= opaque[0] as i32 && right <= opaque[1] as i32 {
                shade_row::<P, false>(row, mask, &self.colors[color], opacity, radius, dx, dy);
            } else {
                shade_row::<P, true>(row, mask, &self.colors[color], opacity, radius, dx, dy);
            }
        }
        Some((index, bounds))
    }

    pub fn set_held(&mut self, held: bool, tick: u64) {
        let new_press = held && !self.held;
        self.held = held;
        if self.height.saturating_sub(self.surface) < 8 {
            self.extra_births.fill(None);
            return;
        }
        let tick = tick as u32;
        let travel = (self.height - self.surface - 6) as u32 * 256;
        let extra_count = (self.extra_population() as usize).div_ceil(256);
        for extra in 0..EXTRA_PARTICLES {
            if let Some(born) = self.extra_births[extra] {
                let age = tick.wrapping_sub(born);
                if age > i32::MAX as u32 {
                    if !held {
                        self.extra_births[extra] = None;
                    }
                } else {
                    let index = PARTICLES + extra;
                    let seed = seed(index, self.index);
                    let speed = 48 + particle_radius(index, seed) as u32 * 14 + ((seed >> 24) & 15);
                    if age.saturating_mul(speed) >= travel {
                        self.extra_births[extra] = None;
                    }
                }
            }
            if held && extra < extra_count && self.extra_births[extra].is_none() {
                self.extra_births[extra] =
                    Some(tick.wrapping_add(if new_press { extra as u32 * 3 } else { 0 }));
            }
        }
    }

    fn extra_population(&self) -> u32 {
        (PARTICLES as u32 * self.level as u32 / 4) * HELD_BOOST_PERCENT as u32 / 100
    }

    #[cfg(any(test, feature = "pico2-queue-profile"))]
    pub fn particle_count(&self) -> usize {
        if self.height.saturating_sub(self.surface) < 8 {
            return 0;
        }
        (PARTICLES * self.level as usize).div_ceil(1024)
            + self.extra_births.iter().filter(|born| born.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn legacy_background(vial: &Vial) -> Vec<u16> {
        let mut background = vial.empty.to_vec();
        let split = vial.surface * vial.width;
        background[split..].copy_from_slice(&vial.liquid[split..]);
        for y in vial.surface..(vial.surface + 7).min(vial.height) {
            for x in 0..vial.width {
                let at = y * vial.width + x;
                let [coverage, rim] = vial.meniscus[(y - vial.surface) * vial.width + x];
                let pixel = background[at];
                let color = [
                    ((pixel >> 11) * 255 / 31) as u8,
                    ((pixel >> 5 & 63) * 255 / 63) as u8,
                    ((pixel & 31) * 255 / 31) as u8,
                ];
                let tint = vial.tint(x);
                let plane = blend(color, tint, coverage as u32 / 3);
                let ring = blend(plane, blend(tint, [255; 3], 128), rim as u32);
                background[at] = rgb(blend(color, ring, vial.mask[at] as u32));
            }
        }
        background
    }

    #[test]
    fn bounded_paint_matches_cached_frames_through_holds_and_level_changes() {
        for (width, height) in [(48, 140), (51, 134), (32, 96)] {
            for index in 0..4 {
                let mut vial = Vial::new(width, height, index);
                let mut serial = [vec![0u16; width * height], vec![0; width * height]];
                let mut batched = serial.clone();
                let mut serial_states = core::array::from_fn::<_, 2, _>(|_| FrameState::default());
                let mut batched_states = core::array::from_fn::<_, 2, _>(|_| FrameState::default());
                for tick in 0..1200 {
                    let back = tick as usize & 1;
                    let level = [409, 665, 512, 819][index];
                    vial.set_level(if (500..540).contains(&tick) { 1024 } else { level });
                    vial.set_held((20..400).contains(&tick) || (700..850).contains(&tick), tick);
                    vial.restore_cached(&mut serial[back], &mut serial_states[back]);
                    vial.stamp_cached(tick, &mut serial[back], &mut serial_states[back]);
                    let mut paint =
                        Paint::new(&vial, tick, &mut batched[back], &mut batched_states[back]);
                    let mut steps = 1;
                    while !paint.step() {
                        steps += 1;
                    }
                    assert!(steps <= height.div_ceil(8) + MAX_PARTICLES);
                    assert!(paint.step());
                    assert_eq!(serial[back], batched[back], "vial {index}, tick {tick}");
                    assert_eq!(serial_states[back].rows, batched_states[back].rows);
                }
            }
        }
    }

    #[test]
    fn row_shading_matches_direct_subpixel_coverage() {
        let colors = core::array::from_fn(|i| rgb([i as u8 * 7, i as u8 * 5, i as u8 * 3]));
        for radius in [1i32, 2, 3, 5] {
            let width = (radius * 2 + 1) as usize;
            for phase in 0..256 {
                for y in 0..width {
                    let dx = -radius * 16 + 8 - (phase & 15);
                    let dy = (y as i32 - radius) * 16 + 8 - (phase >> 4);
                    for opacity in [0, 1, 7, 16, 31, 32] {
                        for mask in [0u8, 13, 127, 255] {
                            let initial: Vec<u16> =
                                (0..width).map(|x| (x as u16).wrapping_mul(6137)).collect();
                            let mut expected = initial.clone();
                            for (x, pixel) in expected.iter_mut().enumerate() {
                                let px = dx + x as i32 * 16;
                                let r2 = radius * radius * 256;
                                let coverage = ((r2 - px * px - dy * dy).max(0) * 32 / r2) as u32;
                                let shine = ((-px - dy) * 8 / radius + 12).clamp(0, 32) as u32;
                                let body = (coverage * opacity * mask as u32 + 4080) / 8160;
                                let ring = (coverage * (32 - coverage) * opacity + 128) / 256;
                                let rim = (ring * (8 + shine) * mask as u32 + 5100) / 10200;
                                *pixel = add(
                                    blend_word(*pixel, rgb([3, 7, 13]), body),
                                    colors[rim as usize],
                                );
                            }
                            let masks = vec![mask; width];
                            let mut actual = initial.clone();
                            shade_row::<u16, true>(
                                &mut actual,
                                &masks,
                                &colors,
                                opacity,
                                radius as usize,
                                dx,
                                dy,
                            );
                            assert_eq!(actual, expected);
                            if mask == 255 {
                                let mut opaque = initial;
                                shade_row::<u16, false>(
                                    &mut opaque,
                                    &masks,
                                    &colors,
                                    opacity,
                                    radius as usize,
                                    dx,
                                    dy,
                                );
                                assert_eq!(opaque, expected);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn cached_opaque_spans_cover_only_full_glass_samples() {
        for (width, height) in [(8, 24), (32, 96), (48, 140), (51, 134)] {
            let vial = Vial::new(width, height, 0);
            for (y, &[left, right]) in vial.opaque_rows.iter().enumerate() {
                assert!(left <= right && right as usize <= width);
                assert!(
                    vial.mask[y * width + left as usize..y * width + right as usize]
                        .iter()
                        .all(|&mask| mask == 255)
                );
            }
        }
    }

    #[test]
    fn opaque_particle_weights_preserve_every_coverage_fade_and_highlight() {
        for coverage in 0..=32 {
            for opacity in 0..=32 {
                for mask in 0..=255 {
                    for shine in 0..=32 {
                        let body = (coverage * opacity * mask + 4080) / 8160;
                        let ring = (coverage * (32 - coverage) * opacity + 128) / 256;
                        let rim = (ring * (8 + shine) * mask + 5100) / 10200;
                        assert_eq!(particle_weights(coverage, opacity, mask, shine), (body, rim));
                    }
                }
            }
        }
    }

    #[test]
    fn borrowed_background_spans_preserve_full_composition_at_every_level() {
        for (width, height) in [(8, 24), (48, 140), (51, 134)] {
            for index in 0..4 {
                let mut vial = Vial::new(width, height, index);
                let mut actual = vec![0; width * height];
                for level in 0..=1024 {
                    vial.set_level(level);
                    vial.restore(&mut actual);
                    assert_eq!(
                        actual,
                        legacy_background(&vial),
                        "{width}x{height}, vial {index}, level {level}"
                    );
                }
            }
        }
    }

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
            let background = legacy_background(&vial);
            let mut changed = 0;
            for tick in 0..240 {
                vial.draw(tick, &mut frame);
                for (at, (&pixel, &background)) in frame.iter().zip(&background).enumerate() {
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
    fn native_assets_preserve_procedural_frames() {
        for index in 0..4 {
            let mut asset = Vial::new(48, 140, index);
            let mut reference = Vial::build(48, 140, index, false);
            let mut a = vec![0; 48 * 140];
            let mut b = a.clone();
            for level in [0, 205, 410, 819, 1024] {
                asset.set_level(level);
                reference.set_level(level);
                for tick in [0, 60, 360, 60_000] {
                    asset.draw(tick, &mut a);
                    reference.draw(tick, &mut b);
                    assert_eq!(a, b, "vial {index}, level {level}, tick {tick}");
                }
            }
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
    fn holding_adds_sixty_percent_large_bubbles_and_release_preserves_them() {
        for (index, level, normal, boosted) in
            [(0, 409, 16, 26), (1, 665, 26, 42), (2, 512, 20, 32), (3, 819, 32, 52)]
        {
            let mut vial = Vial::new(48, 140, index);
            vial.set_level(level);
            assert_eq!(vial.particle_count(), normal);
            vial.set_held(true, 100);
            for extra in 0..EXTRA_PARTICLES {
                assert_eq!(particle_radius(PARTICLES + extra, seed(PARTICLES + extra, index)), 5);
            }
            assert_eq!(vial.particle_count(), boosted);
            assert_eq!(vial.extra_population(), (PARTICLES as u32 * level as u32 / 4) * 60 / 100);
            let mut frame = vec![0; 48 * 140];
            vial.restore(&mut frame);
            let first = vial.stamp(100, &mut frame);
            assert!(first[PARTICLES][1] >= 128);
            assert_eq!(first[PARTICLES + 1], [0; 4]);
            vial.restore(&mut frame);
            let later = vial.stamp(130, &mut frame);
            assert!(later[PARTICLES][1] < first[PARTICLES][1]);
            vial.draw(130, &mut frame);
            let before_release = frame.clone();
            vial.set_held(false, 130);
            vial.draw(130, &mut frame);
            assert_eq!(frame, before_release);
            assert!(vial.particle_count() > normal);
            for tick in 131..800 {
                let births = vial.extra_births;
                vial.set_held(false, tick);
                for (extra, born) in births.into_iter().enumerate() {
                    if let Some(born) = born {
                        let seed = seed(PARTICLES + extra, index);
                        let speed = 48
                            + particle_radius(PARTICLES + extra, seed) as u32 * 14
                            + ((seed >> 24) & 15);
                        let reached_top = (tick as u32).wrapping_sub(born) * speed
                            >= (vial.height - vial.surface - 6) as u32 * 256;
                        assert_eq!(vial.extra_births[extra].is_none(), reached_top);
                    } else {
                        assert_eq!(vial.extra_births[extra], None);
                    }
                }
            }
            assert_eq!(vial.particle_count(), normal);
        }
    }

    #[test]
    fn cached_hold_and_release_preserve_normal_frames_without_trails() {
        for index in 0..4 {
            let mut vial = Vial::new(48, 140, index);
            let mut normal = Vial::new(48, 140, index);
            vial.set_level([409, 665, 512, 819][index]);
            normal.set_level([409, 665, 512, 819][index]);
            let mut buffers = [vec![0; 48 * 140], vec![0; 48 * 140]];
            let mut states = [FrameState::default(), FrameState::default()];
            let mut reference = vec![0; 48 * 140];
            for tick in 0..1000 {
                let held = (60..180).contains(&tick) || (240..300).contains(&tick);
                vial.set_held(held, tick);
                let back = tick as usize % 2;
                vial.restore_cached(&mut buffers[back], &mut states[back]);
                vial.stamp_cached(tick, &mut buffers[back], &mut states[back]);
                vial.draw(tick, &mut reference);
                assert_eq!(buffers[back], reference, "vial {index}, tick {tick}");
                assert!(vial.particle_count() <= MAX_PARTICLES);
                if vial.extra_births.iter().all(Option::is_none) {
                    normal.draw(tick, &mut reference);
                    assert_eq!(buffers[back], reference);
                }
            }
        }
    }

    #[test]
    fn live_births_survive_repress_and_handle_panel_tick_wrap() {
        let mut vial = Vial::new(48, 140, 0);
        vial.set_level(1024);
        let start = u32::MAX as u64 - 4;
        vial.set_held(true, start);
        let first_birth = vial.extra_births[0];
        vial.set_held(true, 5);
        assert_eq!(vial.extra_births[0], first_birth);
        assert_eq!(vial.particle_count(), MAX_PARTICLES);
        let mut frame = vec![0; 48 * 140];
        vial.draw(5, &mut frame);
        vial.set_held(false, 6);
        vial.set_held(true, 7);
        assert_eq!(vial.extra_births[0], first_birth);
        assert!(vial.held);
        vial.set_level(0);
        assert_eq!(vial.particle_count(), 0);
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
