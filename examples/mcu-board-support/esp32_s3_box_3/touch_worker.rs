// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[cfg(not(test))]
use super::{
    BoardI2c, now_us,
    touch_samples::{TouchProducer, TouchSample, TouchState},
};
#[cfg(not(test))]
use core::sync::atomic::AtomicBool;
#[cfg(all(not(test), feature = "box-profile"))]
use core::sync::atomic::AtomicU32;
#[cfg(not(test))]
use core::sync::atomic::Ordering;
#[cfg(all(not(test), feature = "box-profile"))]
use embedded_hal::i2c::I2c;
#[cfg(not(test))]
use gt911::Gt911Blocking;

#[cfg(not(test))]
pub(super) static REQUEST: AtomicBool = AtomicBool::new(false);
#[cfg(all(not(test), feature = "box-profile"))]
pub(super) static READS: AtomicU32 = AtomicU32::new(0);
#[cfg(all(not(test), feature = "box-profile"))]
pub(super) static ERRORS: AtomicU32 = AtomicU32::new(0);

/// The display core exclusively owns the controller and bus. Slint receives
/// timestamped samples on its own core after the controller transaction ends.
#[cfg(not(test))]
pub(super) struct TouchWorker {
    #[cfg(feature = "box-imu")]
    imu: super::imu::Imu,
    i2c: BoardI2c,
    driver: Gt911Blocking<BoardI2c>,
    address: u8,
    producer: TouchProducer<'static>,
    pending: Option<TouchSample>,
    state: TouchState,
    policy: PollPolicy,
}

#[cfg(not(test))]
impl TouchWorker {
    pub(super) fn new(
        mut i2c: BoardI2c,
        driver: Gt911Blocking<BoardI2c>,
        address: u8,
        producer: TouchProducer<'static>,
    ) -> Self {
        Self {
            #[cfg(feature = "box-imu")]
            imu: super::imu::Imu::new(&mut i2c, now_us()),
            i2c,
            driver,
            address,
            producer,
            pending: None,
            state: TouchState::Released,
            policy: PollPolicy::default(),
        }
    }

    pub(super) fn poll(&mut self) {
        self.poll_touch();
        #[cfg(feature = "box-imu")]
        {
            let started = now_us();
            self.imu.poll(&mut self.i2c, started);
            #[cfg(feature = "box-profile")]
            super::imu::Imu::record_read_time(now_us() - started);
        }
    }

    fn poll_touch(&mut self) {
        if let Some(sample) = self.pending.take() {
            if let Err(sample) = self.producer.push(sample) {
                self.pending = Some(sample);
                return;
            }
        }
        let start = now_us();
        if self.policy.backing_off(start) {
            return;
        }
        let requested = REQUEST.swap(false, Ordering::AcqRel);
        #[cfg(feature = "box-profile")]
        let diagnostic_contact = super::profile::mode() != 0;
        #[cfg(not(feature = "box-profile"))]
        let diagnostic_contact = false;
        if !self.policy.should_poll(start, requested, diagnostic_contact) {
            return;
        }
        let result = self.driver.get_touch(&mut self.i2c);
        #[cfg(feature = "box-profile")]
        if diagnostic_contact {
            let mut point = [0u8; 8];
            let read = self.i2c.write_read(self.address, &[0x81, 0x50], &mut point);
            let ack = self.i2c.write(self.address, &[0x81, 0x4e, 0]);
            if read.is_err() || ack.is_err() {
                ERRORS.fetch_add(1, Ordering::Relaxed);
            }
        }
        let completed_us = now_us();
        #[cfg(feature = "box-profile")]
        READS.fetch_add(1, Ordering::Relaxed);
        let state = match result {
            Ok(point) => {
                self.policy.ready(point.is_some());
                match point {
                    Some(point) => TouchState::Contact { x: point.x, y: point.y },
                    None => TouchState::Released,
                }
            }
            Err(gt911::Error::NotReady) => {
                self.policy.not_ready();
                return;
            }
            Err(_) => {
                #[cfg(feature = "box-profile")]
                ERRORS.fetch_add(1, Ordering::Relaxed);
                self.policy.failed(completed_us);
                return;
            }
        };
        if state == self.state {
            return;
        }
        self.state = state;
        let sample = TouchSample { state, completed_us };
        if let Err(sample) = self.producer.push(sample) {
            self.pending = Some(sample);
        }
    }
}

#[derive(Default)]
struct PollPolicy {
    active: bool,
    probe_at: u64,
    retry_at: u64,
    failures: u32,
}

impl PollPolicy {
    fn backing_off(&self, now: u64) -> bool {
        now < self.retry_at
    }
    fn should_poll(&mut self, now: u64, requested: bool, budget: bool) -> bool {
        if self.backing_off(now) || (!requested && !self.active && now < self.probe_at && !budget) {
            return false;
        }
        self.probe_at = now + 100_000;
        true
    }
    fn ready(&mut self, active: bool) {
        self.active = active;
        self.failures = 0;
    }
    fn not_ready(&mut self) {
        self.failures = 0;
    }
    fn failed(&mut self, now: u64) {
        self.failures += 1;
        if self.failures >= 3 {
            self.retry_at = now + 500_000;
            self.failures = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PollPolicy;
    #[test]
    fn idle_probe_is_bounded_but_irq_and_contact_poll_each_tick() {
        let mut policy = PollPolicy::default();
        assert!(policy.should_poll(0, false, false));
        assert!(!policy.should_poll(16_700, false, false));
        assert!(policy.should_poll(16_700, true, false));
        policy.ready(true);
        assert!(policy.should_poll(33_400, false, false));
        policy.ready(false);
        assert!(!policy.should_poll(50_100, false, false));
        assert!(policy.should_poll(133_400, false, false));
    }
    #[test]
    fn not_ready_preserves_contact_until_an_explicit_release() {
        let mut policy = PollPolicy::default();
        policy.ready(true);
        policy.not_ready();
        assert!(policy.active);
        assert!(policy.should_poll(1, false, false));
        policy.ready(false);
        assert!(!policy.active);
        assert!(!policy.should_poll(2, false, false));
    }
    #[test]
    fn failures_back_off_and_recover_without_a_phantom_release() {
        let mut policy = PollPolicy::default();
        policy.ready(true);
        for now in [10, 20, 30] {
            policy.failed(now);
        }
        assert!(policy.active);
        assert!(policy.backing_off(500_029));
        assert!(!policy.should_poll(500_029, true, true));
        assert!(policy.should_poll(500_030, true, false));
        policy.ready(true);
        policy.failed(500_031);
        policy.not_ready();
        policy.failed(500_032);
        policy.failed(500_033);
        assert!(!policy.backing_off(500_034));
    }
    #[test]
    fn diagnostic_budget_does_not_change_contact_state() {
        let mut policy = PollPolicy::default();
        assert!(policy.should_poll(0, false, true));
        assert!(policy.should_poll(1, false, true));
        assert!(!policy.active);
    }
}
