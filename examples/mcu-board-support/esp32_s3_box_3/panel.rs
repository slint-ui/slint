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

    fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        self.cs.set_low();
        let result = Self::write_chunks(&mut self.spi, data);
        self.cs.set_high();
        result
    }

    fn write_chunks(spi: &mut PanelSpi, data: &[u8]) -> Result<(), Error> {
        const CHUNK: usize = 16 * 1024;

        for chunk in data.chunks(CHUNK) {
            spi.half_duplex_write(DataMode::Single, Command::None, Address::None, 0, chunk)?;
        }
        Ok(())
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

    pub(super) fn wait_blank(&mut self) -> Option<u64> {
        let start = super::now_us();
        let mut active_seen = false;
        while super::now_us() - start < 40_000 {
            let mut data = [0; 2];
            if self.read(0x45, 1, &mut data).is_err() {
                return None;
            }
            let line = u16::from_be_bytes(data);
            if line < 269 {
                active_seen = true;
            }
            if active_seen && (269..297).contains(&line) {
                return Some(super::now_us());
            }
            if line < 269 {
                let until_blank = (269 - line as u32) * 16700 / 297;

                if until_blank > 500 {
                    esp_hal::rom::ets_delay_us(until_blank - 200);
                }
            }
        }
        None
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
