// Copyright © 2025 David Haig
// SPDX-License-Identifier: MIT

use embassy_sync::channel::Channel;
use slint::ComponentHandle;
use slint_generated::{Globals, MainWindow};

use crate::{error, warn};

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone)]
pub enum Action {
    HardwareUserBtnPressed(bool),
    TouchscreenToggleBtn(bool),
}

#[cfg(feature = "mcu")]
type ActionChannelType = Channel<embassy_sync::blocking_mutex::raw::ThreadModeRawMutex, Action, 2>;

#[cfg(feature = "simulator")]
type ActionChannelType =
    Channel<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, Action, 2>;

pub static ACTION: ActionChannelType = Channel::new();

// see mcu::hardware or simulator::hardware modules for impl
// depending on features used
pub trait Hardware {
    fn green_led_set_high(&mut self) {}

    fn green_led_set_low(&mut self) {}
}

pub struct Controller<'a, Hardware> {
    main_window: &'a MainWindow,
    hardware: Hardware,
}

impl<'a, H> Controller<'a, H>
where
    H: Hardware,
{
    pub fn new(main_window: &'a MainWindow, hardware: H) -> Self {
        Self { main_window, hardware }
    }

    pub async fn run(&mut self) {
        self.set_action_event_handlers();

        loop {
            let action = ACTION.receive().await;

            match self.process_action(action).await {
                Ok(()) => {
                    // all good
                }
                Err(e) => {
                    error!("process action: {:?}", e);
                }
            }
        }
    }

    pub async fn process_action(&mut self, action: Action) -> Result<(), ()> {
        let globals = self.main_window.global::<Globals>();

        match action {
            Action::HardwareUserBtnPressed(is_pressed) => {
                globals.set_hardware_user_btn_pressed(is_pressed);
            }
            Action::TouchscreenToggleBtn(on) => {
                if on {
                    self.hardware.green_led_set_low();
                } else {
                    self.hardware.green_led_set_high()
                }
            }
        }
        Ok(())
    }

    // user initiated action event handlers
    fn set_action_event_handlers(&self) {
        let globals = self.main_window.global::<Globals>();
        globals.on_toggle_btn(|on| send_action(Action::TouchscreenToggleBtn(on)));
    }
}

pub fn send_action(a: Action) {
    // use non-blocking try_send here because this function needs is called from sync code (the gui callbacks)
    match ACTION.try_send(a) {
        Ok(_) => {
            // see loop in `fn run()` for dequeue
        }
        Err(a) => {
            // this could happen because the controller is slow to respond or we are making too many requests
            warn!("user action queue full, could not add: {:?}", a)
        }
    }
}
