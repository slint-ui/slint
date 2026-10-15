// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore QMI gyro
use super::tilt_fusion::Fusion;
#[cfg(feature = "pico2-queue-profile")]
use core::sync::atomic::AtomicI32;
use core::sync::atomic::{AtomicU32, Ordering};
use embedded_hal::i2c::I2c;

static UP: AtomicU32 = AtomicU32::new(0xfc00_0000);
#[cfg(feature = "pico2-queue-profile")]
static RAW: [AtomicI32; 6] = [const { AtomicI32::new(0) }; 6];
#[cfg(feature = "pico2-queue-profile")]
static READ_US: AtomicU32 = AtomicU32::new(0);
static SAMPLES: AtomicU32 = AtomicU32::new(0);
static ERRORS: AtomicU32 = AtomicU32::new(0);

pub fn up() -> [i16; 2] {
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

pub struct Imu {
    address: Option<u8>,
    fusion: Fusion,
    next_poll: u64,
    previous_stamp: Option<u32>,
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
        if address.is_none() {
            ERRORS.fetch_add(1, Ordering::Relaxed);
        }
        Self { address, fusion: Fusion::new(), next_poll: now + 200_000, previous_stamp: None }
    }

    pub fn poll(&mut self, bus: &mut impl I2c, now: u64) {
        if now < self.next_poll {
            return;
        }
        self.next_poll = now + 16_000;
        let Some(address) = self.address else { return };
        let mut bytes = [0; 17];
        if bus.write_read(address, &[0x30], &mut bytes).is_err() {
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
        UP.store(up[0] as u16 as u32 | ((up[1] as u16 as u32) << 16), Ordering::Release);
        SAMPLES.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "pico2-queue-profile")]
        {
            for (slot, value) in RAW.iter().zip(raw) {
                slot.store(value as i32, Ordering::Relaxed);
            }
            READ_US.store((super::now_us() - now) as u32, Ordering::Relaxed);
        }
    }
}
