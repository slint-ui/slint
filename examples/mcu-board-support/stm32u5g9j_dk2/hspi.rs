// Copyright © 2024 William Spinelli <174336620+williams-one@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

// from embassy/examples/stm32u5/src/bin/hspi_memory_mapped.rs

use defmt::info;
use embassy_stm32::hspi::{AddressSize, DummyCycles, Hspi, HspiWidth, Instance, TransferConfig};
use embassy_stm32::mode::Async;
use embassy_stm32::rcc;

pub fn rcc_init(config: &mut embassy_stm32::Config) {
    config.rcc.pll2 = Some(rcc::Pll {
        source: rcc::PllSource::HSE,
        prediv: rcc::PllPreDiv::DIV4,
        mul: rcc::PllMul::MUL66,
        divp: None,
        divq: Some(rcc::PllDiv::DIV2),
        divr: None,
    });
    config.rcc.mux.hspi1sel = rcc::mux::Hspisel::PLL2_Q; // 132 MHz
}

// Custom implementation for MX66UW1G45G NOR flash memory from Macronix.
// Chip commands are hardcoded as they depend on the chip used.
// This implementation enables Octa I/O (OPI) and Double Transfer Rate (DTR)

pub struct OctaDtrFlashMemory<'d, I: Instance> {
    hspi: Hspi<'d, I, Async>,
}

impl<'d, I: Instance> OctaDtrFlashMemory<'d, I> {
    const CMD_READ_OCTA_DTR: u16 = 0xEE11;

    const CMD_RESET_ENABLE: u8 = 0x66;
    const CMD_RESET_ENABLE_OCTA_DTR: u16 = 0x6699;
    const CMD_RESET: u8 = 0x99;
    const CMD_RESET_OCTA_DTR: u16 = 0x9966;

    const CMD_WRITE_ENABLE: u8 = 0x06;

    const CMD_READ_SR: u8 = 0x05;

    const CMD_WRITE_CR2: u8 = 0x72;

    const CR2_REG1_ADDR: u32 = 0x00000000;
    const CR2_OCTA_DTR: u8 = 0x02;

    const CR2_REG3_ADDR: u32 = 0x00000300;
    const CR2_DC_6_CYCLES: u8 = 0x07;

    pub async fn new(hspi: Hspi<'d, I, Async>) -> Self {
        let mut memory = Self { hspi };

        memory.reset_memory().await;
        memory.enable_octa_dtr().await;
        memory
    }

    async fn enable_octa_dtr(&mut self) {
        self.write_enable_spi().await;
        self.write_cr2_spi(Self::CR2_REG3_ADDR, Self::CR2_DC_6_CYCLES);
        self.write_enable_spi().await;
        self.write_cr2_spi(Self::CR2_REG1_ADDR, Self::CR2_OCTA_DTR);
    }

    pub async fn enable_mm(&mut self) {
        let read_config = TransferConfig {
            iwidth: HspiWidth::OCTO,
            instruction: Some(Self::CMD_READ_OCTA_DTR as u32),
            isize: AddressSize::_16Bit,
            idtr: true,
            adwidth: HspiWidth::OCTO,
            adsize: AddressSize::_32Bit,
            addtr: true,
            dwidth: HspiWidth::OCTO,
            ddtr: true,
            dummy: DummyCycles::_6,
            ..Default::default()
        };

        let write_config = TransferConfig {
            iwidth: HspiWidth::OCTO,
            isize: AddressSize::_16Bit,
            idtr: true,
            adwidth: HspiWidth::OCTO,
            adsize: AddressSize::_32Bit,
            addtr: true,
            dwidth: HspiWidth::OCTO,
            ddtr: true,
            ..Default::default()
        };
        self.hspi.enable_memory_mapped_mode(read_config, write_config).unwrap();
    }

    async fn exec_command_spi(&mut self, cmd: u8) {
        let transaction = TransferConfig {
            iwidth: HspiWidth::SING,
            instruction: Some(cmd as u32),
            ..Default::default()
        };
        info!("Excuting command: 0x{:X}", transaction.instruction.unwrap());
        self.hspi.blocking_command(&transaction).unwrap();
    }

    async fn exec_command_octa_dtr(&mut self, cmd: u16) {
        let transaction = TransferConfig {
            iwidth: HspiWidth::OCTO,
            instruction: Some(cmd as u32),
            isize: AddressSize::_16Bit,
            idtr: true,
            ..Default::default()
        };
        info!("Excuting command: 0x{:X}", transaction.instruction.unwrap());
        self.hspi.blocking_command(&transaction).unwrap();
    }

    fn wait_write_finish_spi(&mut self) {
        while (self.read_sr_spi() & 0x01) != 0 {}
    }

    pub async fn reset_memory(&mut self) {
        // servono entrambi i comandi?
        self.exec_command_octa_dtr(Self::CMD_RESET_ENABLE_OCTA_DTR).await;
        self.exec_command_octa_dtr(Self::CMD_RESET_OCTA_DTR).await;
        self.exec_command_spi(Self::CMD_RESET_ENABLE).await;
        self.exec_command_spi(Self::CMD_RESET).await;
        self.wait_write_finish_spi();
    }

    async fn write_enable_spi(&mut self) {
        self.exec_command_spi(Self::CMD_WRITE_ENABLE).await;
    }

    pub fn read_sr_spi(&mut self) -> u8 {
        let mut buffer = [0; 1];
        let transaction: TransferConfig = TransferConfig {
            iwidth: HspiWidth::SING,
            instruction: Some(Self::CMD_READ_SR as u32),
            dwidth: HspiWidth::SING,
            ..Default::default()
        };
        self.hspi.blocking_read(&mut buffer, transaction).unwrap();
        // info!("Read MX66LM1G45G SR register: 0x{:x}", buffer[0]);
        buffer[0]
    }

    pub fn write_cr2_spi(&mut self, addr: u32, value: u8) {
        let buffer = [value; 1];
        let transaction: TransferConfig = TransferConfig {
            iwidth: HspiWidth::SING,
            instruction: Some(Self::CMD_WRITE_CR2 as u32),
            adwidth: HspiWidth::SING,
            address: Some(addr),
            adsize: AddressSize::_32Bit,
            dwidth: HspiWidth::SING,
            ..Default::default()
        };
        self.hspi.blocking_write(&buffer, transaction).unwrap();
    }
}
