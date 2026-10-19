// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use super::tilt_fusion::Fusion;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use embedded_hal::i2c::I2c;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static UP: AtomicU32 = AtomicU32::new(0xfc00_0000);
static SAMPLES: AtomicU32 = AtomicU32::new(0);
static ERRORS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "box-profile")]
static MAX_READ_US: AtomicU32 = AtomicU32::new(0);

pub fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::Relaxed);
}

pub fn up() -> [i16; 2] {
    let word = UP.load(Ordering::Acquire);
    [word as i16, (word >> 16) as i16]
}

#[cfg(feature = "box-profile")]
pub fn telemetry() -> [u32; 3] {
    [
        SAMPLES.load(Ordering::Relaxed),
        ERRORS.load(Ordering::Relaxed),
        MAX_READ_US.load(Ordering::Relaxed),
    ]
}

pub struct Imu {
    address: Option<u8>,
    fusion: Fusion,
    previous: Option<u64>,
    next_poll: u64,
}

impl Imu {
    pub fn new(bus: &mut impl I2c, now: u64) -> Self {
        let mut address = None;
        for at in [0x68, 0x69] {
            let mut id = [0];
            if bus.write_read(at, &[0x75], &mut id).is_ok() && matches!(id[0], 0x60 | 0x67) {
                // Espressif's ICM42670 driver: ±500 dps, ±4 g, 100 Hz, low-noise modes.
                if bus.write(at, &[0x20, 0x49, 0x49]).is_ok()
                    && bus.write(at, &[0x1f, 0x0f]).is_ok()
                {
                    log::info!("BOX_IMU,address={at:#04x},whoami={:#04x}", id[0]);
                    address = Some(at);
                    break;
                }
            }
        }
        if address.is_none() {
            log::warn!("BOX IMU unavailable; Ink stays upright");
        }
        Self { address, fusion: Fusion::new(), previous: None, next_poll: now + 50_000 }
    }

    pub fn poll(&mut self, bus: &mut impl I2c, now: u64) {
        if now < self.next_poll || !ACTIVE.load(Ordering::Relaxed) {
            return;
        }
        let Some(address) = self.address else { return };
        self.next_poll = now + 16_000;
        let mut bytes = [0; 12];
        if bus.write_read(address, &[0x0b], &mut bytes).is_err() {
            ERRORS.fetch_add(1, Ordering::Relaxed);
            self.next_poll = now + 250_000;
            return;
        }
        let raw: [i16; 6] =
            core::array::from_fn(|i| i16::from_be_bytes([bytes[2 * i], bytes[2 * i + 1]]));
        // The BOX framebuffer uses the panel's 180-degree landscape orientation.
        let accel = [-(raw[0] as f32) / 8192., -(raw[1] as f32) / 8192., raw[2] as f32 / 8192.];
        let radians = core::f32::consts::PI / (180. * 65.5);
        let gyro =
            [-(raw[3] as f32) * radians, -(raw[4] as f32) * radians, raw[5] as f32 * radians];
        if SAMPLES.load(Ordering::Relaxed) == 0 {
            log::info!("BOX_IMU_RAW,accel={:?},gyro={:?}", &raw[..3], &raw[3..]);
        }
        #[cfg(feature = "box-profile")]
        let (accel, gyro) = if option_env!("BOX_BENCH_TILT") == Some("replay") {
            super::tilt_fusion::replay(now as f32 / 1_000_000.)
        } else {
            (accel, gyro)
        };
        let dt = self
            .previous
            .replace(now)
            .map_or(1. / 60., |previous| (now - previous) as f32 / 1_000_000.);
        let up = self.fusion.update(accel, gyro, dt);
        UP.store(up[0] as u16 as u32 | ((up[1] as u16 as u32) << 16), Ordering::Release);
        SAMPLES.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(feature = "box-profile")]
    pub fn record_read_time(elapsed: u64) {
        MAX_READ_US.fetch_max(elapsed.min(u32::MAX as u64) as u32, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal::i2c::{ErrorKind, ErrorType, Operation};

    #[derive(Default)]
    struct Bus {
        registers: alloc::vec::Vec<alloc::vec::Vec<u8>>,
        fail: bool,
    }
    impl ErrorType for Bus {
        type Error = ErrorKind;
    }
    impl I2c for Bus {
        fn transaction(
            &mut self,
            address: u8,
            operations: &mut [Operation<'_>],
        ) -> Result<(), Self::Error> {
            if self.fail || address != 0x68 {
                return Err(ErrorKind::Bus);
            }
            let mut register = 0;
            for operation in operations {
                match operation {
                    Operation::Write(bytes) => {
                        register = bytes[0];
                        self.registers.push(bytes.to_vec());
                    }
                    Operation::Read(bytes) if register == 0x75 => bytes[0] = 0x60,
                    Operation::Read(bytes) => {
                        bytes.copy_from_slice(&[0, 0, 0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0])
                    }
                }
            }
            Ok(())
        }
    }

    #[test]
    fn configures_both_sensors_and_bounds_reads_and_error_retries() {
        let mut bus = Bus::default();
        let mut imu = Imu::new(&mut bus, 0);
        assert_eq!(imu.address, Some(0x68));
        assert!(bus.registers.iter().any(|r| r == &[0x20, 0x49, 0x49]));
        assert!(bus.registers.iter().any(|r| r == &[0x1f, 0x0f]));
        let configured = bus.registers.len();
        imu.poll(&mut bus, 100_000);
        assert_eq!(bus.registers.len(), configured);
        set_active(true);
        imu.poll(&mut bus, 100_000);
        assert_eq!(up(), [0, -1024]);
        let sampled = bus.registers.len();
        imu.poll(&mut bus, 115_999);
        assert_eq!(bus.registers.len(), sampled);
        bus.fail = true;
        imu.poll(&mut bus, 116_000);
        assert_eq!(imu.next_poll, 366_000);
        bus.fail = false;
        imu.poll(&mut bus, 365_999);
        assert_eq!(bus.registers.len(), sampled);
        imu.poll(&mut bus, 366_000);
        assert_eq!(bus.registers.len(), sampled + 1);
        set_active(false);
    }
}
