// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use super::{MAX_PARTICLES, PARTICLES, Vial, particle_radius, seed, wave};

#[derive(Clone, Copy, Default)]
pub(super) struct Bubble {
    pub x: i32,
    pub y: i32,
    pub age: u16,
    pub state: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Surface {
    pub up: [i16; 2],
    pub cut: i32,
    pub ripple: [i16; 2],
}

impl Surface {
    pub fn ripple_offset(self, x: usize, width: usize) -> i32 {
        let phase = (x * 65536 / width.saturating_sub(1).max(1)) as u32;
        (self.ripple[0] as i32 * wave(phase) + self.ripple[1] as i32 * wave(phase * 2)) >> 10
    }

    pub fn cut_at(self, x: i32, width: usize) -> i32 {
        let column = (x / 256).clamp(0, width as i32 - 1) as usize;
        self.cut + self.up[1] as i32 * self.ripple_offset(column, width) / 1024
    }
}

pub(super) struct Tilt {
    pub surface: Surface,
    pub bubbles: [Bubble; MAX_PARTICLES],
    pub starts: alloc::vec::Vec<i16>,
    pub surface_rows: core::ops::Range<i32>,
    tick: u64,
    level: u16,
    held: bool,
    slosh: super::slosh::Slosh,
}

impl Tilt {
    pub fn new(tick: u64) -> Self {
        Self {
            surface: Surface { up: [0, -1024], cut: 0, ripple: [0; 2] },
            bubbles: [Bubble::default(); MAX_PARTICLES],
            starts: alloc::vec::Vec::new(),
            surface_rows: 0..0,
            tick,
            level: u16::MAX,
            held: false,
            slosh: Default::default(),
        }
    }

    pub fn update(&mut self, vial: &Vial, flow: [i16; 2], tick: u64) -> bool {
        let target = if flow[1] <= -886 && flow[0].abs() <= 512 {
            flow
        } else {
            let x: i16 = if flow[0] < 0 { -512 } else { 512 };
            [x, -886]
        };
        let elapsed = tick.saturating_sub(self.tick).min(4) as u32;
        let up = self.slosh.update(
            target,
            elapsed,
            vial.index,
            vial.height.saturating_sub(vial.surface),
            vial.width,
        );
        let idle = super::slosh::idle_ripples(tick, vial.index);
        let ripple = [self.slosh.ripple_q8() + idle[0], idle[1]];
        let changed =
            up != self.surface.up || vial.level != self.level || ripple != self.surface.ripple;
        if changed {
            self.surface = Surface {
                up,
                cut: projection(up, vial.width as i32 * 128, (vial.surface + 3) as i32 * 256),
                ripple,
            };
            self.level = vial.level;
        }
        self.tick = tick;
        let normal = (PARTICLES * vial.level as usize).div_ceil(1024);
        let extra = (vial.extra_population() as usize).div_ceil(256);
        let new_hold = vial.held && !self.held;
        self.held = vial.held;
        for index in 0..MAX_PARTICLES {
            let seed = seed(index, vial.index);
            let radius = particle_radius(index, seed);
            let b = &mut self.bubbles[index];
            if vial.height.saturating_sub(vial.surface) < 8
                || (index < PARTICLES && index >= normal)
                || (index >= PARTICLES && index - PARTICLES >= extra)
            {
                b.state = 0;
                continue;
            }
            if b.state == 0 && (index < PARTICLES || vial.held) {
                b.state = 2;
                b.age = if index >= PARTICLES && new_hold {
                    ((index - PARTICLES) * 3) as u16
                } else {
                    0
                };
            }
            if b.state == 2 {
                if index >= PARTICLES && !vial.held {
                    b.state = 0;
                    continue;
                }
                if b.age > elapsed as u16 {
                    b.age -= elapsed as u16;
                    continue;
                }
                *b = spawn(vial, up, radius, seed.wrapping_add((tick as u32).wrapping_mul(31)));
                if index < PARTICLES {
                    let depth =
                        (self.surface.cut - projection(up, b.x, b.y) - radius as i32 * 256).max(0);
                    let travel = depth * ((seed >> 16) & 1023) as i32 / 1024;
                    b.x += up[0] as i32 * travel / 1024;
                    b.y += up[1] as i32 * travel / 1024;
                    constrain(vial, radius, b);
                    b.age = 20;
                }
            }
            if b.state != 1 {
                continue;
            }
            let speed = 48 + radius as i32 * 14 + ((seed >> 24) & 15) as i32;
            let wobble =
                (wave(seed.wrapping_add((tick as u32).wrapping_mul(170)))
                    - wave(seed.wrapping_add(
                        (tick.saturating_sub(elapsed as u64) as u32).wrapping_mul(170),
                    )))
                    / 2;
            b.x += (flow[0] as i32 * speed * elapsed as i32 - flow[1] as i32 * wobble) / 1024;
            b.y += (flow[1] as i32 * speed * elapsed as i32 + flow[0] as i32 * wobble) / 1024;
            constrain(vial, radius, b);
            b.age = b.age.saturating_add(elapsed as u16);
            if projection(up, b.x, b.y) + radius as i32 * 256
                >= self.surface.cut_at(b.x, vial.width)
            {
                if index < PARTICLES || vial.held {
                    *b = spawn(vial, up, radius, seed.wrapping_add((tick as u32).wrapping_mul(31)));
                } else {
                    b.state = 0;
                }
            }
        }
        changed
    }

    #[cfg(any(test, feature = "pico2-queue-profile"))]
    pub fn count(&self, vial: &Vial) -> usize {
        if vial.height.saturating_sub(vial.surface) < 8 {
            return 0;
        }
        (PARTICLES * vial.level as usize).div_ceil(1024)
            + self.bubbles[PARTICLES..].iter().filter(|b| b.state != 0).count()
    }
}

pub(super) fn projection(up: [i16; 2], x: i32, y: i32) -> i32 {
    (up[0] as i32 * x + up[1] as i32 * y) / 1024
}

fn spawn(vial: &Vial, up: [i16; 2], radius: usize, seed: u32) -> Bubble {
    let half = vial.width.min(vial.height) as i32 * 128;
    let r = (half - (radius as i32 + 2) * 256).max(128);
    let jitter = ((seed >> 8) % 4097) as i32 - 2048;
    let vx = -up[0] as i32 * 1024 + up[1] as i32 * jitter;
    let vy = -up[1] as i32 * 1024 - up[0] as i32 * jitter;
    let length = (vx as i64 * vx as i64 + vy as i64 * vy as i64) as u64;
    let scale = r as f32 / length.isqrt().max(1) as f32;
    let center_y = vial.height as i32 * 256 - half;
    let mut b = Bubble {
        x: vial.width as i32 * 128 + (vx as f32 * scale) as i32,
        y: center_y + (vy as f32 * scale) as i32,
        age: 0,
        state: 1,
    };
    constrain(vial, radius, &mut b);
    b
}

fn constrain(vial: &Vial, radius: usize, b: &mut Bubble) {
    let half = vial.width.min(vial.height) as i32 * 128;
    let r = (half - (radius as i32 + 2) * 256).max(128);
    let center_x = vial.width as i32 * 128;
    let center_y = b.y.clamp(half, vial.height as i32 * 256 - half);
    let dx = b.x - center_x;
    let dy = b.y - center_y;
    let square = dx as i64 * dx as i64 + dy as i64 * dy as i64;
    if square > r as i64 * r as i64 {
        let length = (square as u64).isqrt() as i32;
        let scale = r as f32 / length as f32;
        b.x = center_x + (dx as f32 * scale) as i32;
        b.y = center_y + (dy as f32 * scale) as i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_bubble_moves_along_the_wall_instead_of_stopping() {
        let v = Vial::new(48, 140, 0);
        let mut b = Bubble { x: 39 * 256, y: 80 * 256, age: 0, state: 1 };
        let initial_y = b.y;
        for _ in 0..100 {
            b.x += 80;
            b.y -= 80;
            constrain(&v, 5, &mut b);
            assert!(b.x <= 41 * 256);
        }
        assert!(b.y < initial_y - 20 * 256);
    }
}
