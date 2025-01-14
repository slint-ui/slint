// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![no_main]
#![no_std]
#![cfg(target_os = "uefi")]

extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec;
use core::slice;
use core::sync::atomic::{AtomicPtr, Ordering};
use core::time::Duration;
use log::info;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{platform::software_renderer, SharedString};
use uefi::boot::ScopedProtocol;
use uefi::prelude::*;
use uefi::proto::console::{gop::BltPixel, pointer::Pointer};
use uefi::Char16;

slint::include_modules!();

static MOUSE_POINTER: AtomicPtr<ScopedProtocol<Pointer>> = AtomicPtr::new(core::ptr::null_mut());

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
        uefi::boot::stall(1000);
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

fn pointer_init() {
    // mouse pointer
    let handle = uefi::boot::get_handle_for_protocol::<Pointer>().expect("miss Pointer protocol");
    let mut pointer = uefi::boot::open_protocol_exclusive::<Pointer>(handle)
        .expect("can't open Pointer protocol.");
    pointer.reset(false).expect("Failed to reset pointer device.");
    info!("pointer inited, mode = {:?}.", pointer.mode());
    let raw_ptr = Box::into_raw(Box::new(pointer));
    MOUSE_POINTER.store(raw_ptr, Ordering::Relaxed);
}

fn get_key_press() -> Option<char> {
    use slint::platform::Key::*;
    use uefi::proto::console::text::Key as UefiKey;
    use uefi::proto::console::text::ScanCode as Scan;

    let nl = Char16::try_from('\r').unwrap();

    match uefi::system::with_stdin(|stdin| stdin.read_key()) {
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
    use uefi::boot::*;

    let watchdog_timeout = Duration::from_secs(120);
    let timeout = watchdog_timeout.min(max_timeout.unwrap_or(watchdog_timeout));

    // SAFETY: The event is closed before returning from this function.
    let timer = unsafe {
        uefi::boot::create_event(EventType::TIMER, Tpl::APPLICATION, None, None).unwrap()
    };
    uefi::boot::set_timer(&timer, TimerTrigger::Periodic((timeout.as_nanos() / 100) as u64))
        .unwrap();

    uefi::boot::set_watchdog_timer(2 * watchdog_timeout.as_micros() as usize, 0x10000, None)
        .unwrap();

    uefi::system::with_stdin(|stdin| {
        // SAFETY: The cloned handles are only used to wait for further input events and
        // are then immediately dropped.
        let ptr = MOUSE_POINTER.load(Ordering::Relaxed);
        let pointer_ref = unsafe { &*ptr };
        let mut events = unsafe {
            [
                stdin.wait_for_key_event().unwrap(),
                pointer_ref.wait_for_input_event().unwrap(),
                timer.unsafe_clone(),
            ]
        };
        uefi::boot::wait_for_event(&mut events).unwrap();
    });

    uefi::boot::set_watchdog_timer(2 * watchdog_timeout.as_micros() as usize, 0x10000, None)
        .unwrap();
    uefi::boot::close_event(timer).unwrap();
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

#[repr(transparent)]
#[derive(Clone, Copy)]
/// RGBA-8-8-8-8
struct PngRGBAPixel([u8; 4]);

impl PngRGBAPixel {
    fn new() -> Self {
        PngRGBAPixel([254, 254, 254, 0])
    }
    fn from_rgba(&mut self, r: u8, g: u8, b: u8, a: u8) {
        self.0 = [r, g, b, a];
    }

    fn blend_blt_pixel(&self, background: &mut BltPixel) {
        // Alpha Blending
        // Result = Foreground×α + Background×(1−α)
        let alpha = self.0[3] as f32 / 255.0;
        let r = self.0[0] as f32;
        let g = self.0[1] as f32;
        let b = self.0[2] as f32;

        let blended_r = ((1.0 - alpha) * background.red as f32 + alpha * r) as u8;
        let blended_g = ((1.0 - alpha) * background.green as f32 + alpha * g) as u8;
        let blended_b = ((1.0 - alpha) * background.blue as f32 + alpha * b) as u8;

        background.red = blended_r;
        background.green = blended_g;
        background.blue = blended_b;
    }
}

struct Platform {
    window: Rc<software_renderer::MinimalSoftwareWindow>,
    timer_freq: f64,
    timer_start: f64,
}

impl Default for Platform {
    fn default() -> Self {
        pointer_init();
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
        use uefi::{boot::*, proto::console::gop::*};

        let gop_handle = uefi::boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();

        // SAFETY: uefi-rs wants us to use open_protocol_exclusive(), which will not work
        // on real hardware. We can only hope that any other users of this
        // handle/protocol behave and don't interfere with our uses of it.
        let mut gop = unsafe {
            uefi::boot::open_protocol::<GraphicsOutput>(
                OpenProtocolParams {
                    handle: gop_handle,
                    agent: uefi::boot::image_handle(),
                    controller: None,
                },
                OpenProtocolAttributes::GetProtocol,
            )
            .unwrap()
        };

        let info = gop.current_mode_info();
        let mut fb = alloc::vec![SlintBltPixel(BltPixel::new(0, 0, 0)); info.resolution().0 * info.resolution().1];

        //mouse pixel
        let png: &[u8] = &include_bytes!("resource/cursor.png")[..];
        let header = minipng::decode_png_header(png).expect("bad PNG");
        let mut buffer = vec![0; header.required_bytes_rgba8bpc()];
        let mut image = minipng::decode_png(png, &mut buffer).expect("bad PNG");
        image.convert_to_rgba8bpc().expect("Failed to convert to RGBA8bit");
        info!("pointer png image size: {}x{} ", image.width(), image.height());
        let pointer_x = image.width() as usize;
        let pointer_y = image.height() as usize;
        let image_size: usize = (image.width() * image.height()) as usize;
        let mut vec_png = alloc::vec![PngRGBAPixel::new(); image_size];
        let mut mfb = alloc::vec![BltPixel::new(254, 254, 254); image_size];
        for i in 0..image_size {
            vec_png[i].from_rgba(
                image.pixels()[4 * i + 0], //r
                image.pixels()[4 * i + 1], //g
                image.pixels()[4 * i + 2], //b
                image.pixels()[4 * i + 3], //a
            );
            vec_png[i].blend_blt_pixel(&mut mfb[i]);
        }

        self.window.set_size(slint::PhysicalSize::new(
            info.resolution().0.try_into().unwrap(),
            info.resolution().1.try_into().unwrap(),
        ));

        let mut position = slint::LogicalPosition::new(0.0, 0.0);

        let ptr = MOUSE_POINTER.load(Ordering::Relaxed);
        let mpointer = unsafe { &mut *ptr };
        let conpointer = unsafe { &*ptr };
        let mouse_mode = conpointer.mode();
        let mut is_mouse_move = false;

        loop {
            slint::platform::update_timers_and_animations();

            // key handle until no input
            while let Some(key) = get_key_press() {
                // EFI does not distinguish between pressed and released events.
                let text = SharedString::from(key);
                self.window.try_dispatch_event(WindowEvent::KeyPressed { text: text.clone() })?;
                self.window.try_dispatch_event(WindowEvent::KeyReleased { text })?;
            }
            // mouse handle until no input
            while let Some(mut mouse) =
                mpointer.read_state().expect("Failed to read state from Pointer.")
            {
                position.x +=
                    (mouse.relative_movement[0] as f32) / (mouse_mode.resolution[0] as f32);
                position.y +=
                    (mouse.relative_movement[1] as f32) / (mouse_mode.resolution[1] as f32);

                let button: PointerEventButton = match mouse.button {
                    [true, true] => PointerEventButton::Left,
                    [true, false] => PointerEventButton::Left,
                    [false, true] => PointerEventButton::Right,
                    [false, false] => PointerEventButton::Other,
                };

                if position.x < 0.0 {
                    position.x = 0.0;
                } else if position.x > (info.resolution().0 - pointer_x) as f32 {
                    position.x = (info.resolution().0 - pointer_x) as f32;
                    mouse.relative_movement[0] = (info.resolution().0) as i32;
                }

                if position.y < 0.0 {
                    position.y = 0.0;
                } else if position.y > (info.resolution().1 - pointer_y) as f32 {
                    position.y = (info.resolution().1 - pointer_y) as f32;
                    mouse.relative_movement[1] = (info.resolution().1) as i32;
                }

                self.window.try_dispatch_event(WindowEvent::PointerMoved { position })?;
                self.window.try_dispatch_event(WindowEvent::PointerExited {})?;
                self.window.try_dispatch_event(WindowEvent::PointerPressed { position, button })?;
                self.window
                    .try_dispatch_event(WindowEvent::PointerReleased { position, button })?;
                is_mouse_move = true;
            }

            if is_mouse_move {
                self.window.request_redraw();
                is_mouse_move = false;
            };

            self.window.draw_if_needed(|renderer| {
                renderer.render(&mut fb, info.resolution().0);

                // SAFETY: SlintBltPixel is a repr(transparent) BltPixel so it is safe to transform.
                let blt_fb =
                    unsafe { slice::from_raw_parts(fb.as_ptr() as *const BltPixel, fb.len()) };
                let blt_mfb = unsafe {
                    slice::from_raw_parts_mut(mfb.as_mut_ptr() as *mut BltPixel, mfb.len())
                };

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

                // get framebuffer from UEFI.
                gop.blt(BltOp::VideoToBltBuffer {
                    buffer: blt_mfb,
                    src: (position.x as usize, position.y as usize),
                    dest: BltRegion::Full,
                    dims: (pointer_x, pointer_y),
                })
                .unwrap();

                // mouse cursor RGBA render to framebuffer.
                for y in 0..pointer_y {
                    for x in 0..pointer_x {
                        vec_png[x + y * pointer_x].blend_blt_pixel(&mut mfb[x + y * pointer_x]);
                    }
                }

                // write framebuffer to UEFI.
                gop.blt(BltOp::BufferToVideo {
                    buffer: blt_mfb,
                    src: BltRegion::Full,
                    dest: (position.x as usize, position.y as usize),
                    dims: (pointer_x, pointer_y),
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
fn main() -> Status {
    slint::platform::set_platform(Box::<Platform>::default()).unwrap();

    let ui = Demo::new().unwrap();

    ui.set_firmware_vendor(
        String::from_utf16_lossy(uefi::system::firmware_vendor().to_u16_slice()).into(),
    );
    ui.set_firmware_version(
        format!(
            "{}.{:02}",
            uefi::system::firmware_revision() >> 16,
            uefi::system::firmware_revision() & 0xffff
        )
        .into(),
    );
    ui.set_uefi_version(uefi::system::uefi_revision().to_string().into());

    let mut buf = [0u8; 1];
    let guid = uefi::runtime::VariableVendor::GLOBAL_VARIABLE;
    let sb = uefi::runtime::get_variable(cstr16!("SecureBoot"), &guid, &mut buf);
    ui.set_secure_boot(if sb.is_ok() { buf[0] == 1 } else { false });

    ui.run().unwrap();

    Status::SUCCESS
}
