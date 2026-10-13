// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore jtag

//! The host link of the ESP32-S3 boards whose USB-C connector is wired to the chip's
//! USB-Serial-JTAG port.

use core::sync::atomic::{AtomicBool, Ordering};
use embedded_hal::delay::DelayNs;
use esp_hal::time::Instant;

/// See [`crate::HostLink`].
static RESTART_ON_PANIC: AtomicBool = AtomicBool::new(false);

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    match info.location() {
        Some(location) => esp_println::println!("Panic at {location}: {}", info.message()),
        None => esp_println::println!("Panic: {}", info.message()),
    }
    if RESTART_ON_PANIC.load(Ordering::Relaxed) {
        // Lets the message leave the USB FIFO
        esp_hal::delay::Delay::new().delay_ms(100);
        esp_hal::system::software_reset()
    }
    loop {}
}

pub(crate) fn host_link(
    usb_device: esp_hal::peripherals::USB_DEVICE<'static>,
    board_name: &'static str,
) -> impl crate::HostLink {
    RESTART_ON_PANIC.store(true, Ordering::Relaxed);
    UsbSerialHostLink {
        rx: esp_hal::usb_serial_jtag::UsbSerialJtag::new(usb_device).split().0,
        board_name,
    }
}

/// Receives through the USB-Serial-JTAG port, and sends through `esp_println` like the log
/// output.
struct UsbSerialHostLink {
    rx: esp_hal::usb_serial_jtag::UsbSerialJtagRx<'static, esp_hal::Blocking>,
    board_name: &'static str,
}

impl crate::HostLink for UsbSerialHostLink {
    fn read(&mut self, buffer: &mut [u8]) -> usize {
        // The FIFO only holds one 64 byte USB packet: stopping at the first empty read
        // would receive one packet per call.
        const PAUSE: esp_hal::time::Duration = esp_hal::time::Duration::from_millis(2);
        let mut len = self.rx.drain_rx_fifo(buffer);
        if len == 0 {
            return 0;
        }
        let mut last_data = Instant::now();
        while len < buffer.len() {
            let read = self.rx.drain_rx_fifo(&mut buffer[len..]);
            if read > 0 {
                len += read;
                last_data = Instant::now();
            } else if last_data.elapsed() > PAUSE {
                break;
            }
        }
        len
    }

    fn write(&mut self, bytes: &[u8]) {
        esp_println::Printer::write_bytes(bytes);
    }

    fn board_name(&self) -> &str {
        self.board_name
    }

    fn free_memory(&self) -> Option<usize> {
        Some(esp_alloc::HEAP.free())
    }
}
