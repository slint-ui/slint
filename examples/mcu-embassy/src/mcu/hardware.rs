// Copyright © 2025 David Haig
// SPDX-License-Identifier: MIT

use crate::controller::Hardware;

pub struct HardwareMcu {
    pub green_led: embassy_stm32::gpio::Output<'static>,
}

impl Hardware for HardwareMcu {
    fn green_led_set_high(&mut self) {
        self.green_led.set_high();
    }

    fn green_led_set_low(&mut self) {
        self.green_led.set_low();
    }
}
