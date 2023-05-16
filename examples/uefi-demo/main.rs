// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: MIT

#![no_main]
#![no_std]

extern crate alloc;

use alloc::{
    boxed::Box,
    format,
    rc::Rc,
    string::{String, ToString},
};
use core::{slice, time::Duration};
use slint::{platform::software_renderer, SharedString};
use uefi::{prelude::*, proto::console::gop::BltPixel, Char16};
use uefi_services::system_table;

slint::include_modules!();

fn st() -> &'static mut SystemTable<Boot> {
    // SAFETY: uefi_services::init() is always called first in main()
    // and we never operate outside boot services
    unsafe { system_table().as_mut() }
}

fn timer_tick() -> u64 {
    #[cfg(target_arch = "x86")]
    unsafe {
        core::arch::x86::_rdtsc()
    }

    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::x86_64::_rdtsc()
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        let mut ticks: u64;
        core::arch::asm!("mrs {}, cntvct_el0", out(reg) ticks);
        ticks
    }
}

fn timer_freq() -> u64 {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let start = timer_tick();
        st().boot_services().stall(1000);
        let end = timer_tick();
        (end - start) * 1000
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        let mut freq: u64;
        core::arch::asm!("mrs {}, cntfrq_el0", out(reg) freq);
        freq
    }
}

fn get_key_press() -> Option<char> {
    use slint::platform::Key::*;
    use uefi::proto::console::text::Key as UefiKey;
    use uefi::proto::console::text::ScanCode as Scan;

    let nl = Char16::try_from('\r').unwrap();

    match st().stdin().read_key() {
        Err(_) | Ok(None) => None,
        Ok(Some(UefiKey::Printable(key))) if key == nl => Some('\n'),
        Ok(Some(UefiKey::Printable(key))) => Some(char::from(key)),
        Ok(Some(UefiKey::Special(key))) => Some(
            match key {
                Scan::UP => UpArrow,
                Scan::DOWN => DownArrow,
                Scan::RIGHT => RightArrow,
                Scan::LEFT => LeftArrow,
                Scan::HOME => Home,
                Scan::END => End,
                Scan::INSERT => Insert,
                Scan::DELETE => Delete,
                Scan::PAGE_UP => PageUp,
                Scan::PAGE_DOWN => PageDown,
                Scan::ESCAPE => Escape,
                Scan::FUNCTION_1 => F1,
                Scan::FUNCTION_2 => F2,
                Scan::FUNCTION_3 => F3,
                Scan::FUNCTION_4 => F4,
                Scan::FUNCTION_5 => F5,
                Scan::FUNCTION_6 => F6,
                Scan::FUNCTION_7 => F7,
                Scan::FUNCTION_8 => F8,
                Scan::FUNCTION_9 => F9,
                Scan::FUNCTION_10 => F10,
                Scan::FUNCTION_11 => F11,
                Scan::FUNCTION_12 => F12,
                Scan::FUNCTION_13 => F13,
                Scan::FUNCTION_14 => F14,
                Scan::FUNCTION_15 => F15,
                Scan::FUNCTION_16 => F16,
                Scan::FUNCTION_17 => F17,
                Scan::FUNCTION_18 => F18,
                Scan::FUNCTION_19 => F19,
                Scan::FUNCTION_20 => F20,
                Scan::FUNCTION_21 => F21,
                Scan::FUNCTION_22 => F22,
                Scan::FUNCTION_23 => F23,
                Scan::FUNCTION_24 => F24,
                _ => return None,
            }
            .into(),
        ),
    }
}

fn wait_for_input(max_timeout: Option<Duration>) {
    use uefi::table::boot::*;

    let watchdog_timeout = Duration::from_secs(120);
    let timeout = watchdog_timeout.min(max_timeout.unwrap_or(watchdog_timeout));

    let bs = st().boot_services();

    // SAFETY: The event is closed before returning from this function.
    let timer = unsafe { bs.create_event(EventType::TIMER, Tpl::APPLICATION, None, None).unwrap() };
    bs.set_timer(&timer, TimerTrigger::Periodic((timeout.as_nanos() / 100) as u64)).unwrap();

    bs.set_watchdog_timer(2 * watchdog_timeout.as_micros() as usize, 0x10000, None).unwrap();

    {
        // SAFETY: The cloned handles are only used to wait for further input events and
        // are then immediately dropped.
        let mut events =
            unsafe { [st().stdin().wait_for_key_event().unsafe_clone(), timer.unsafe_clone()] };
        bs.wait_for_event(&mut events).unwrap();
    }

    bs.set_watchdog_timer(2 * watchdog_timeout.as_micros() as usize, 0x10000, None).unwrap();
    bs.close_event(timer).unwrap();
}

#[repr(transparent)]
#[derive(Clone, Copy)]
struct SlintBltPixel(BltPixel);

impl software_renderer::TargetPixel for SlintBltPixel {
    fn blend(&mut self, color: software_renderer::PremultipliedRgbaColor) {
        let a = (u8::MAX - color.alpha) as u16;
        self.0.red = (self.0.red as u16 * a / 255) as u8 + color.red;
        self.0.green = (self.0.green as u16 * a / 255) as u8 + color.green;
        self.0.blue = (self.0.blue as u16 * a / 255) as u8 + color.blue;
    }

    fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        SlintBltPixel(BltPixel::new(red, green, blue))
    }
}

struct Platform {
    window: Rc<software_renderer::MinimalSoftwareWindow>,
    timer_freq: f64,
    timer_start: f64,
}

impl Default for Platform {
    fn default() -> Self {
        Self {
            window: software_renderer::MinimalSoftwareWindow::new(
                software_renderer::RepaintBufferType::ReusedBuffer,
            ),
            timer_freq: timer_freq() as f64,
            timer_start: timer_tick() as f64,
        }
    }
}

impl slint::platform::Platform for Platform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> Duration {
        Duration::from_secs_f64((timer_tick() as f64 - self.timer_start) / self.timer_freq)
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        use uefi::{proto::console::gop::*, table::boot::*};

        let bs = st().boot_services();

        let gop_handle = bs.get_handle_for_protocol::<GraphicsOutput>().unwrap();

        // SAFETY: uefi-rs wants us to use open_protocol_exclusive(), which will not work
        // on real hardware. We can only hope that any other users of this
        // handle/protocol behave and don't interfere with our uses of it.
        let mut gop = unsafe {
            bs.open_protocol::<GraphicsOutput>(
                OpenProtocolParams {
                    handle: gop_handle,
                    agent: bs.image_handle(),
                    controller: None,
                },
                OpenProtocolAttributes::GetProtocol,
            )
            .unwrap()
        };

        let info = gop.current_mode_info();
        let mut fb = alloc::vec![SlintBltPixel(BltPixel::new(0, 0, 0)); info.resolution().0 * info.resolution().1];

        self.window.set_size(slint::PhysicalSize::new(
            info.resolution().0.try_into().unwrap(),
            info.resolution().1.try_into().unwrap(),
        ));

        loop {
            slint::platform::update_timers_and_animations();

            while let Some(key) = get_key_press() {
                // EFI does not distinguish between pressed and released events.
                let text = SharedString::from(key);
                self.window.dispatch_event(slint::platform::WindowEvent::KeyPressed {
                    text: text.clone(),
                });
                self.window.dispatch_event(slint::platform::WindowEvent::KeyReleased { text });
            }

            self.window.draw_if_needed(|renderer| {
                renderer.render(&mut fb, info.resolution().0);

                // SAFETY: SlintBltPixel is a repr(transparent) BltPixel so it is safe to transform.
                let blt_fb =
                    unsafe { slice::from_raw_parts(fb.as_ptr() as *const BltPixel, fb.len()) };

                // We could let the software renderer draw to gop.frame_buffer() directly, but that
                // requires dealing with different frame buffer formats. The blit buffer is easier to
                // deal with and guaranteed to be available by the UEFI spec. This also reduces tearing
                // by quite a bit.
                gop.blt(BltOp::BufferToVideo {
                    buffer: blt_fb,
                    src: BltRegion::Full,
                    dest: (0, 0),
                    dims: info.resolution(),
                })
                .unwrap();
            });

            if !self.window.has_active_animations() {
                wait_for_input(slint::platform::duration_until_next_timer_update());
            }
        }
    }
}

#[entry]
fn main(_image_handle: Handle, mut st: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut st).unwrap();

    slint::platform::set_platform(Box::<Platform>::default()).unwrap();

    let ui = Demo::new().unwrap();

    ui.set_firmware_vendor(String::from_utf16_lossy(st.firmware_vendor().to_u16_slice()).into());
    ui.set_firmware_version(
        format!("{}.{:02}", st.firmware_revision() >> 16, st.firmware_revision() & 0xffff).into(),
    );
    ui.set_uefi_version(st.uefi_revision().to_string().into());

    let mut buf = [0u8; 1];
    let guid = uefi::table::runtime::VariableVendor::GLOBAL_VARIABLE;
    let sb = st.runtime_services().get_variable(cstr16!("SecureBoot"), &guid, &mut buf);
    ui.set_secure_boot(if sb.is_ok() { buf[0] == 1 } else { false });

    ui.run().unwrap();

    Status::SUCCESS
}
