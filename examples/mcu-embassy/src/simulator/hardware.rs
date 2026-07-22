// Copyright © 2025 David Haig
// SPDX-License-Identifier: MIT

use crate::controller::Hardware;
use crate::info;

pub struct HardwareSim {}

impl Hardware for HardwareSim {
    fn green_led_set_high(&mut self) {
        info!("green led OFF");
    }

    fn green_led_set_low(&mut self) {
        info!("green led ON");
    }
}
