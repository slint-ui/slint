// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use esp_hal::{
    Blocking,
    dma::{DmaRxBuf, DmaTxBuf},
    spi::{
        Error,
        master::{Address, Command, Config, ConfigError, DataMode, SpiDma},
    },
};

/// Keeps DMA buffers owned by the transfer and reduces shared-bus polling.
pub(super) struct DmaPanelBus {
    spi: Option<SpiDma<'static, Blocking>>,
    rx: Option<DmaRxBuf>,
    tx: Option<DmaTxBuf>,
}

impl DmaPanelBus {
    pub(super) fn new(spi: SpiDma<'static, Blocking>, rx: DmaRxBuf, tx: DmaTxBuf) -> Self {
        Self { spi: Some(spi), rx: Some(rx), tx: Some(tx) }
    }

    pub(super) fn apply_config(&mut self, config: &Config) -> Result<(), ConfigError> {
        self.spi.as_mut().unwrap().apply_config(config)
    }

    pub(super) fn flush(&mut self) -> Result<(), Error> {
        // Every operation restores the peripheral only after wait completes.
        Ok(())
    }

    pub(super) fn half_duplex_write(
        &mut self,
        mode: DataMode,
        command: Command,
        address: Address,
        dummy: u8,
        data: &[u8],
    ) -> Result<(), Error> {
        if data.is_empty() {
            return Ok(());
        }
        let spi = self.spi.take().unwrap();
        let mut buffer = self.tx.take().unwrap();
        buffer.set_length(data.len());
        buffer.as_mut_slice()[..data.len()].copy_from_slice(data);
        let transfer =
            match spi.half_duplex_write(mode, command, address, dummy, data.len(), buffer) {
                Ok(transfer) => transfer,
                Err((error, spi, buffer)) => {
                    self.spi = Some(spi);
                    self.tx = Some(buffer);
                    return Err(error);
                }
            };
        if data.len() > 64 {
            while !transfer.is_done() {
                esp_hal::rom::ets_delay_us(50);
            }
        }
        let (spi, buffer) = transfer.wait();
        self.spi = Some(spi);
        self.tx = Some(buffer);
        Ok(())
    }

    pub(super) fn half_duplex_read(
        &mut self,
        mode: DataMode,
        command: Command,
        address: Address,
        dummy: u8,
        data: &mut [u8],
    ) -> Result<(), Error> {
        let spi = self.spi.take().unwrap();
        let mut buffer = self.rx.take().unwrap();
        buffer.set_length(data.len());
        let transfer = match spi.half_duplex_read(mode, command, address, dummy, data.len(), buffer)
        {
            Ok(transfer) => transfer,
            Err((error, spi, buffer)) => {
                self.spi = Some(spi);
                self.rx = Some(buffer);
                return Err(error);
            }
        };
        let (spi, buffer) = transfer.wait();
        data.copy_from_slice(&buffer.as_slice()[..data.len()]);
        self.spi = Some(spi);
        self.rx = Some(buffer);
        Ok(())
    }
}
