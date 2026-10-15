// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore QMI gyro
use super::tilt_fusion::Fusion;
#[cfg(feature = "pico2-queue-profile")]
use core::sync::atomic::AtomicI32;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
static DESIRED_FLIP: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "pico2-queue-profile")]
static FORCE_ROTATION: AtomicU32 = AtomicU32::new(2);

pub fn desired_flip() -> bool {
    DESIRED_FLIP.load(Ordering::Acquire)
}

#[cfg(feature = "pico2-queue-profile")]
pub fn force_rotation(value: u32) {
    FORCE_ROTATION.store(value, Ordering::Release);
    if value < 2 {
        DESIRED_FLIP.store(value != 0, Ordering::Release);
    }
}

static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::Relaxed);
}
use embedded_hal::i2c::I2c;

#[cfg(feature = "pico2-queue-profile")]
static FORCED_UP: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
static REPLAY_UP: AtomicU32 = AtomicU32::new(0xfc00_0000);

static UP: AtomicU32 = AtomicU32::new(0xfc00_0000);
#[cfg(feature = "pico2-queue-profile")]
static RAW: [AtomicI32; 6] = [const { AtomicI32::new(0) }; 6];
#[cfg(feature = "pico2-queue-profile")]
static READ_US: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
static SAMPLES: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
static ERRORS: AtomicU32 = AtomicU32::new(0);

pub fn up() -> [i16; 2] {
    #[cfg(feature = "pico2-queue-profile")]
    let forced = FORCED_UP.load(Ordering::Acquire);
    #[cfg(feature = "pico2-queue-profile")]
    let word = if forced != 0 {
        forced
    } else if matches!(super::profile::RUN.mode(), 12 | 13) {
        REPLAY_UP.load(Ordering::Acquire)
    } else {
        UP.load(Ordering::Acquire)
    };
    #[cfg(not(feature = "pico2-queue-profile"))]
    let word = UP.load(Ordering::Acquire);
    [word as i16, (word >> 16) as i16]
}

#[cfg(feature = "pico2-queue-profile")]
pub fn telemetry() -> ([i32; 6], u32, u32, u32) {
    (
        RAW.each_ref().map(|v| v.load(Ordering::Relaxed)),
        SAMPLES.load(Ordering::Relaxed),
        ERRORS.load(Ordering::Relaxed),
        READ_US.load(Ordering::Relaxed),
    )
}

#[cfg(feature = "pico2-queue-profile")]
pub fn force_up(up: Option<[i16; 2]>) {
    let word = up.map_or(0, |u| u[0] as u16 as u32 | ((u[1] as u16 as u32) << 16));
    FORCED_UP.store(word, Ordering::Release);
}

pub struct Imu {
    address: Option<u8>,
    landscape: super::orientation::Landscape,
    #[cfg(feature = "pico2-queue-profile")]
    applied: bool,
    fusion: Fusion,
    next_poll: u64,
    previous_stamp: Option<u32>,
    #[cfg(feature = "pico2-queue-profile")]
    replay: (u32, u64, Fusion),
}

impl Imu {
    pub fn new(bus: &mut impl I2c, now: u64) -> Self {
        let mut address = None;
        for at in [0x6a, 0x6b] {
            let mut who = [0];
            if bus.write_read(at, &[0], &mut who).is_ok() && who[0] == 5 {
                // QMI8658: auto-increment, ±4 g, ±512 dps, 125 Hz, 17.5 Hz low-pass filters.
                let settings = [[8, 0], [2, 0x60], [3, 0x16], [4, 0x56], [6, 0x77], [8, 3]];
                if settings.iter().all(|setting| bus.write(at, setting).is_ok()) {
                    address = Some(at);
                    break;
                }
            }
        }
        #[cfg(feature = "pico2-queue-profile")]
        if address.is_none() {
            ERRORS.fetch_add(1, Ordering::Relaxed);
        }
        Self {
            address,
            landscape: super::orientation::Landscape::new(),
            #[cfg(feature = "pico2-queue-profile")]
            applied: false,
            fusion: Fusion::new(),
            next_poll: now + 200_000,
            previous_stamp: None,
            #[cfg(feature = "pico2-queue-profile")]
            replay: (u32::MAX, 0, Fusion::new()),
        }
    }

    pub fn apply_flip(&mut self, flipped: bool) {
        self.fusion.set_flipped(flipped);
        let u = self.fusion.up();
        UP.store(u[0] as u16 as u32 | ((u[1] as u16 as u32) << 16), Ordering::Release);
        #[cfg(feature = "pico2-queue-profile")]
        {
            self.applied = flipped;
            self.replay.2.set_flipped(flipped);
            let u = self.replay.2.up();
            REPLAY_UP.store(u[0] as u16 as u32 | ((u[1] as u16 as u32) << 16), Ordering::Release);
        }
    }

    pub fn poll(&mut self, bus: &mut impl I2c, now: u64) {
        if now < self.next_poll {
            return;
        }
        self.next_poll = now + if ACTIVE.load(Ordering::Relaxed) { 16_000 } else { 32_000 };
        let Some(address) = self.address else { return };
        let mut bytes = [0; 17];
        if bus.write_read(address, &[0x30], &mut bytes).is_err() {
            #[cfg(feature = "pico2-queue-profile")]
            ERRORS.fetch_add(1, Ordering::Relaxed);
            UP.store(0xfc00_0000, Ordering::Release);
            self.next_poll = now + 250_000;
            return;
        }
        let stamp = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]);
        if self.previous_stamp == Some(stamp) {
            return;
        }
        let dt = self
            .previous_stamp
            .replace(stamp)
            .map_or(0.016, |last| (stamp.wrapping_sub(last) & 0xffffff) as f32 / 125.);
        let raw = core::array::from_fn::<_, 6, _>(|i| {
            i16::from_le_bytes([bytes[5 + i * 2], bytes[6 + i * 2]])
        });
        let accel = core::array::from_fn(|i| raw[i] as f32 / 8192.);
        let gyro =
            core::array::from_fn(|i| raw[i + 3] as f32 * (core::f32::consts::PI / (180. * 64.)));
        let up = self.fusion.update(accel, gyro, dt);
        let base_up = self.fusion.base_up();
        #[cfg(feature = "pico2-queue-profile")]
        let base_up = {
            let run = super::profile::RUN.token();
            if matches!(run & 255, 12 | 13) {
                if run != self.replay.0 {
                    self.replay = (run, now, Fusion::new());
                    self.replay.2.set_flipped(self.applied);
                }
                let t = (now - self.replay.1) as f32 * 0.000001;
                let (a, g) = super::tilt_fusion::replay(t);
                let u = self.replay.2.update(a, g, dt);
                REPLAY_UP
                    .store(u[0] as u16 as u32 | ((u[1] as u16 as u32) << 16), Ordering::Release);
                self.replay.2.base_up()
            } else {
                base_up
            }
        };
        let wanted = self.landscape.observe(base_up, dt);
        #[cfg(feature = "pico2-queue-profile")]
        let wanted = match FORCE_ROTATION.load(Ordering::Acquire) {
            0 => false,
            1 => true,
            _ => wanted,
        };
        DESIRED_FLIP.store(wanted, Ordering::Release);
        UP.store(up[0] as u16 as u32 | ((up[1] as u16 as u32) << 16), Ordering::Release);
        #[cfg(feature = "pico2-queue-profile")]
        {
            SAMPLES.fetch_add(1, Ordering::Relaxed);
            for (slot, value) in RAW.iter().zip(raw) {
                slot.store(value as i32, Ordering::Relaxed);
            }
            READ_US.store((super::now_us() - now) as u32, Ordering::Relaxed);
        }
    }
}
