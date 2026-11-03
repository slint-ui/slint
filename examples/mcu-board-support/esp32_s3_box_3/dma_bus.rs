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

#[cfg(feature = "box-display-profile")]
#[derive(Clone, Copy, Default)]
pub(super) struct Profile {
    pub copy_us: u64,
    pub setup_us: u64,
    pub wait_us: u64,
    pub overlap_us: u64,
    pub chunks: usize,
}

/// Keeps DMA buffers owned by the transfer and reduces shared-bus polling.
pub(super) struct DmaPanelBus {
    spi: Option<SpiDma<'static, Blocking>>,
    rx: Option<DmaRxBuf>,
    tx: Option<DmaTxBuf>,
    spare_tx: Option<DmaTxBuf>,
    #[cfg(feature = "box-display-profile")]
    profile: Profile,
}

impl DmaPanelBus {
    pub(super) fn new(
        spi: SpiDma<'static, Blocking>,
        rx: DmaRxBuf,
        tx: DmaTxBuf,
        spare_tx: DmaTxBuf,
    ) -> Self {
        Self {
            spi: Some(spi),
            rx: Some(rx),
            tx: Some(tx),
            spare_tx: Some(spare_tx),
            #[cfg(feature = "box-display-profile")]
            profile: Profile::default(),
        }
    }

    #[cfg(feature = "box-display-profile")]
    pub(super) fn take_profile(&mut self) -> Profile {
        core::mem::take(&mut self.profile)
    }

    pub(super) fn apply_config(&mut self, config: &Config) -> Result<(), ConfigError> {
        self.spi.as_mut().unwrap().apply_config(config)
    }

    fn fill_buffer(&mut self, buffer: &mut DmaTxBuf, data: &[u8]) {
        #[cfg(feature = "box-display-profile")]
        let started = super::now_us();
        buffer.set_length(data.len());
        #[cfg(feature = "box-display-profile")]
        let copy_started = super::now_us();
        buffer.as_mut_slice()[..data.len()].copy_from_slice(data);
        #[cfg(feature = "box-display-profile")]
        if data.len() > 64 {
            self.profile.setup_us += copy_started - started;
            self.profile.copy_us += super::now_us() - copy_started;
        }
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
        let mut spi = self.spi.take().unwrap();
        let mut buffer = self.tx.take().unwrap();
        let mut spare = self.spare_tx.take().unwrap();
        let mut chunks = data.chunks(buffer.capacity());
        let mut chunk = chunks.next().unwrap();
        self.fill_buffer(&mut buffer, chunk);
        loop {
            #[cfg(feature = "box-display-profile")]
            let submitted = super::now_us();
            let transfer =
                match spi.half_duplex_write(mode, command, address, dummy, chunk.len(), buffer) {
                    Ok(transfer) => transfer,
                    Err((error, spi, buffer)) => {
                        self.spi = Some(spi);
                        self.tx = Some(buffer);
                        self.spare_tx = Some(spare);
                        return Err(error);
                    }
                };
            #[cfg(feature = "box-display-profile")]
            if chunk.len() > 64 {
                self.profile.setup_us += super::now_us() - submitted;
                self.profile.chunks += 1;
            }
            let next = chunks.next();
            if let Some(next) = next {
                #[cfg(feature = "box-display-profile")]
                let before = self.profile.copy_us;
                self.fill_buffer(&mut spare, next);
                #[cfg(feature = "box-display-profile")]
                {
                    self.profile.overlap_us += self.profile.copy_us - before;
                }
            }
            #[cfg(feature = "box-display-profile")]
            let wait_started = super::now_us();
            if chunk.len() > 64 {
                while !transfer.is_done() {
                    esp_hal::rom::ets_delay_us(50);
                }
            }
            (spi, buffer) = transfer.wait();
            #[cfg(feature = "box-display-profile")]
            if chunk.len() > 64 {
                self.profile.wait_us += super::now_us() - wait_started;
            }
            let Some(next) = next else {
                break;
            };
            core::mem::swap(&mut buffer, &mut spare);
            chunk = next;
        }
        self.spi = Some(spi);
        self.tx = Some(buffer);
        self.spare_tx = Some(spare);
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
