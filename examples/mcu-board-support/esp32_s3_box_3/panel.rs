// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::rc::Rc;
use core::cell::RefCell;
use esp_hal::{
    gpio::Output,
    spi::{
        Error,
        master::{Address, Command, Config, DataMode},
    },
    time::Rate,
};
use mipidsi::interface::{Interface, InterfaceKind};

type PanelSpi = super::dma_bus::DmaPanelBus;

/// The ILI9342C's 0xB1 frame rate control: the full oscillator, and 31 clocks per line.
/// A full-screen write takes about 32 ms; tearing starts once a scan overtakes it.
/// Scanning at about half the write speed lets the write start behind one scan
/// and finish before the next one catches up, at a refresh close to 60 Hz.
/// Half the oscillator with 16 clocks per line corrupts colors on the BOX-3.
pub(super) const FRAME_RATE_CONTROL: [u8; 2] = [0x00, 0x1f];
/// Vertical porch lines, as set with the 0xB5 command.
pub(super) const FRONT_PORCH: u16 = 14;
pub(super) const BACK_PORCH: u16 = 14;
const ACTIVE_END: u16 = BACK_PORCH + 240;
const LINES: u16 = ACTIVE_END + FRONT_PORCH;
// The stock 27-clock line at the full oscillator frequency refreshes 297 lines every 16.7 ms.
const LINE_NS: u32 = 16_700_000 / 297 * 31 / 27;
pub(super) const REFRESH_US: u32 = LINES as u32 * LINE_NS / 1000;

/// Owns the display pins, including bidirectional SDA for panel diagnostics.
pub(super) struct Panel {
    spi: PanelSpi,
    cs: Output<'static>,
    dc: Output<'static>,
    buffer: [u8; 512],
}

impl Panel {
    pub(super) fn new(spi: PanelSpi, cs: Output<'static>, dc: Output<'static>) -> Self {
        Self { spi, cs, dc, buffer: [0; 512] }
    }

    #[cfg(feature = "box-display-profile")]
    pub(super) fn take_dma_profile(&mut self) -> super::dma_bus::Profile {
        self.spi.take_profile()
    }

    fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        self.cs.set_low();
        let result = Self::write_chunks(&mut self.spi, data);
        self.cs.set_high();
        result
    }

    fn write_chunks(spi: &mut PanelSpi, data: &[u8]) -> Result<(), Error> {
        spi.half_duplex_write(DataMode::Single, Command::None, Address::None, 0, data)
    }

    pub(super) fn command(&mut self, command: u8, parameters: &[u8]) -> Result<(), Error> {
        self.dc.set_low();
        self.write(&[command])?;
        self.dc.set_high();
        self.write(parameters)
    }

    pub(super) fn packed_rectangle(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        data: &[u8],
    ) -> Result<(), Error> {
        self.command(
            0x2a,
            &[(x >> 8) as u8, x as u8, ((x + w - 1) >> 8) as u8, (x + w - 1) as u8],
        )?;
        self.command(
            0x2b,
            &[(y >> 8) as u8, y as u8, ((y + h - 1) >> 8) as u8, (y + h - 1) as u8],
        )?;
        self.command(0x2c, &[])?;
        self.write(data)
    }

    /// Writes `run`, opening an address window from its first band down to the last row.
    pub(super) fn stream(
        &mut self,
        band: super::frame_queue::band_stream::Band,
        run: &mut super::frame_queue::band_stream::Run<'_, super::frame_queue::Slot>,
    ) -> Result<(), Error> {
        let end = band.x + band.width - 1;
        self.command(0x2a, &[(band.x >> 8) as u8, band.x as u8, (end >> 8) as u8, end as u8])?;
        self.command(0x2b, &[(band.y >> 8) as u8, band.y as u8, 0, 239])?;
        self.command(0x2c, &[])?;
        self.cs.set_low();
        let result = run.send_each(|buffer, len| {
            self.spi.write_buffer(DataMode::Single, Command::None, Address::None, 0, buffer, len)
        });
        self.cs.set_high();
        result
    }

    /// Waits for the panel's vertical blank, unless `interrupted` returns true first.
    pub(super) fn wait_blank(&mut self, interrupted: impl Fn() -> bool) -> Option<u64> {
        let start = super::now_us();
        let mut active_seen = false;
        while super::now_us() - start < 2 * u64::from(REFRESH_US) {
            if interrupted() {
                return None;
            }
            let line = self.scanline()?;
            if line >= ACTIVE_END {
                if active_seen {
                    return Some(super::now_us());
                }
                continue;
            }
            active_seen = true;
            let until_blank = u32::from(ACTIVE_END - line) * REFRESH_US / u32::from(LINES);
            if until_blank > 500 && sleep_unless(until_blank - 200, &interrupted) {
                return None;
            }
        }
        None
    }

    /// Waits until the panel scans its first row, so a top-to-bottom write stays behind the scan,
    /// or until `stop` returns true.
    pub(super) fn wait_scan_start(&mut self, stop: impl Fn() -> bool) {
        let start = super::now_us();
        let mut porch_seen = false;
        while super::now_us() - start < 2 * u64::from(REFRESH_US) {
            if stop() {
                return;
            }
            let Some(line) = self.scanline() else { return };
            if !(BACK_PORCH..ACTIVE_END).contains(&line) {
                porch_seen = true;
                continue;
            }
            if porch_seen {
                return;
            }
            let until_porch = u32::from(ACTIVE_END - line) * REFRESH_US / u32::from(LINES);
            if until_porch > 500 && sleep_unless(until_porch - 200, &stop) {
                return;
            }
        }
    }

    fn scanline(&mut self) -> Option<u16> {
        let mut data = [0; 2];
        self.read(0x45, 1, &mut data).ok()?;
        Some(u16::from_be_bytes(data))
    }

    fn read(&mut self, command: u8, dummy: u8, data: &mut [u8]) -> Result<(), Error> {
        self.spi.apply_config(&Config::default().with_frequency(Rate::from_mhz(1))).unwrap();
        self.dc.set_low();
        self.cs.set_low();
        let result = self.spi.half_duplex_read(
            DataMode::Single,
            Command::_8Bit(command as u16, DataMode::Single),
            Address::None,
            dummy,
            data,
        );
        self.cs.set_high();
        self.dc.set_high();
        self.spi.apply_config(&Config::default().with_frequency(Rate::from_mhz(40))).unwrap();
        result
    }
}

/// Sleeps for `us`, unless `stop` returns true first, which it then reports.
fn sleep_unless(us: u32, stop: &impl Fn() -> bool) -> bool {
    let until = super::now_us() + u64::from(us);
    while super::now_us() < until {
        if stop() {
            return true;
        }
        esp_hal::rom::ets_delay_us(100);
    }
    false
}

/// Implements the existing mipidsi initialization and pixel packing contract.
pub(super) struct PanelInterface(pub(super) Rc<RefCell<Panel>>);

impl Interface for PanelInterface {
    type Word = u8;
    type Error = Error;
    const KIND: InterfaceKind = InterfaceKind::Serial4Line;

    fn send_command(&mut self, command: u8, args: &[u8]) -> Result<(), Error> {
        self.0.borrow_mut().command(command, args)
    }

    fn send_pixels<const N: usize>(
        &mut self,
        pixels: impl IntoIterator<Item = [u8; N]>,
    ) -> Result<(), Error> {
        let mut panel = self.0.borrow_mut();
        let mut pixels = pixels.into_iter();
        loop {
            let mut len = 0;
            for chunk in panel.buffer.chunks_exact_mut(N) {
                if let Some(pixel) = pixels.next() {
                    chunk.copy_from_slice(&pixel);
                    len += N;
                } else {
                    break;
                }
            }
            if len == 0 {
                return Ok(());
            }
            panel.cs.set_low();
            let Panel { spi, buffer, .. } = &mut *panel;
            let result = Panel::write_chunks(spi, &buffer[..len]);
            panel.cs.set_high();
            result?;
        }
    }

    fn send_repeated_pixel<const N: usize>(
        &mut self,
        pixel: [u8; N],
        count: u32,
    ) -> Result<(), Error> {
        self.send_pixels(core::iter::repeat(pixel).take(count as usize))
    }
}
