// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Tracking of attached input devices, to drive the `Launcher` global's
//! `keyboard-attached` and `input-device-attached` properties.
//!
//! On LinuxKMS, input devices can come and go: without a keyboard the focus
//! ring in the UI makes no sense, and without any input device at all the UI
//! shows a hint. [`install()`] selects the backend with a libinput event hook
//! that tracks the attached keyboards, pointer devices, and touch screens.
//! The hook runs on the event loop thread. Outside LinuxKMS, a no-op twin of
//! [`install()`] keeps `main()` free of conditional compilation: input
//! devices are assumed to be present (the Launcher global's property
//! defaults) and the backend is selected as usual.

#[cfg(all(
    target_os = "linux",
    any(feature = "backend-linuxkms", feature = "backend-linuxkms-noseat")
))]
mod linuxkms {
    use crate::{Launcher, LauncherWindow};
    use slint::ComponentHandle;
    use std::cell::{Cell, OnceCell};
    use std::rc::Rc;

    #[derive(Clone, Copy, Default, PartialEq)]
    struct AttachedInputDevices {
        keyboard: bool,
        pointer: bool,
        touch: bool,
    }

    impl AttachedInputDevices {
        fn any(self) -> bool {
            self.keyboard || self.pointer || self.touch
        }
    }

    fn is_keyboard(device: &input::Device) -> bool {
        if !device.has_capability(input::DeviceCapability::Keyboard) {
            return false;
        }
        // The keyboard capability is also claimed by devices that merely have
        // keys, such as power buttons. Require udev's ID_INPUT_KEYBOARD, which
        // is only set for devices with a full set of keys.
        // SAFETY: the returned udev device is dropped before the libinput
        // device, and no udev context is shared with other threads.
        unsafe { device.udev_device() }.is_some_and(|udev_device| {
            udev_device.property_value("ID_INPUT_KEYBOARD").is_some_and(|value| value == "1")
        })
    }

    /// Connects the window to the monitor installed by [`install()`].
    pub struct Monitor {
        window: Rc<OnceCell<slint::Weak<LauncherWindow>>>,
    }

    impl Monitor {
        pub fn attach(&self, ui: &LauncherWindow) {
            // No input devices until libinput reports them.
            let launcher = ui.global::<Launcher>();
            launcher.set_keyboard_attached(false);
            launcher.set_input_device_attached(false);
            let _ = self.window.set(ui.as_weak());
        }
    }

    pub fn install() -> Result<Monitor, slint::PlatformError> {
        let window: Rc<OnceCell<slint::Weak<LauncherWindow>>> = Rc::new(OnceCell::new());
        let hook_window = window.clone();
        let keyboards = Cell::new(0usize);
        let pointers = Cell::new(0usize);
        let touch_screens = Cell::new(0usize);
        slint::BackendSelector::new()
            // This build of the launcher launches demos by replacing itself,
            // which only makes sense on LinuxKMS. Pin the backend so that the
            // runtime selection cannot diverge from that launch mode.
            .backend_name("linuxkms".into())
            .with_libinput_event_hook(move |event| {
                use input::event::DeviceEvent;
                if let input::Event::Device(device_event) = event {
                    use input::event::EventTrait;
                    let delta = match device_event {
                        DeviceEvent::Added(_) => 1,
                        DeviceEvent::Removed(_) => -1,
                        _ => return false,
                    };
                    let state = || AttachedInputDevices {
                        keyboard: keyboards.get() > 0,
                        pointer: pointers.get() > 0,
                        touch: touch_screens.get() > 0,
                    };
                    let previous = state();
                    let device = device_event.device();
                    let update = |counter: &Cell<usize>, claimed: bool| {
                        if claimed {
                            counter.set(counter.get().saturating_add_signed(delta));
                        }
                    };
                    update(&keyboards, is_keyboard(&device));
                    update(&pointers, device.has_capability(input::DeviceCapability::Pointer));
                    update(&touch_screens, device.has_capability(input::DeviceCapability::Touch));
                    let current = state();
                    if current != previous
                        && let Some(ui) = hook_window.get().and_then(|weak| weak.upgrade())
                    {
                        let launcher = ui.global::<Launcher>();
                        let keyboard_was_attached = launcher.get_keyboard_attached();
                        launcher.set_keyboard_attached(current.keyboard);
                        launcher.set_input_device_attached(current.any());
                        if current.keyboard && !keyboard_was_attached {
                            ui.invoke_reset_focus();
                        }
                    }
                }
                false // don't consume the event
            })
            .select()?;
        Ok(Monitor { window })
    }
}

#[cfg(all(
    target_os = "linux",
    any(feature = "backend-linuxkms", feature = "backend-linuxkms-noseat")
))]
pub use linuxkms::install;

#[cfg(not(all(
    target_os = "linux",
    any(feature = "backend-linuxkms", feature = "backend-linuxkms-noseat")
)))]
mod noop {
    pub struct Monitor;

    impl Monitor {
        pub fn attach(&self, _ui: &crate::LauncherWindow) {}
    }

    pub fn install() -> Result<Monitor, slint::PlatformError> {
        Ok(Monitor)
    }
}

#[cfg(not(all(
    target_os = "linux",
    any(feature = "backend-linuxkms", feature = "backend-linuxkms-noseat")
)))]
pub use noop::install;
