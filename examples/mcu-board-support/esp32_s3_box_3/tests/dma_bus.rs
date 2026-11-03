// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

extern crate self as esp_hal;

use std::{cell::RefCell, marker::PhantomData, rc::Rc};

pub struct Blocking;
pub mod rom {
    pub fn ets_delay_us(_: u32) {}
}

pub mod dma {
    pub struct DmaTxBuf(pub Vec<u8>, pub usize);
    impl DmaTxBuf {
        pub fn capacity(&self) -> usize {
            self.0.len()
        }
        pub fn set_length(&mut self, len: usize) {
            assert!(len <= self.0.len());
            self.1 = len;
        }
        pub fn as_mut_slice(&mut self) -> &mut [u8] {
            &mut self.0[..self.1]
        }
    }
    pub struct DmaRxBuf(pub Vec<u8>, pub usize);
    impl DmaRxBuf {
        pub fn set_length(&mut self, len: usize) {
            assert!(len <= self.0.len());
            self.1 = len;
        }
        pub fn as_slice(&self) -> &[u8] {
            &self.0[..self.1]
        }
    }
}

#[derive(Default)]
struct Wire {
    bytes: Vec<u8>,
    submissions: usize,
    fail_on: Option<usize>,
}

pub mod spi {
    #[derive(Debug, PartialEq)]
    pub struct Error;
    pub mod master {
        use super::Error;
        use crate::{
            dma::{DmaRxBuf, DmaTxBuf},
            *,
        };
        #[derive(Clone, Copy)]
        pub struct DataMode;
        #[derive(Clone, Copy)]
        pub struct Command;
        #[derive(Clone, Copy)]
        pub struct Address;
        pub struct Config;
        pub struct ConfigError;
        pub struct SpiDma<'d, Dm> {
            pub(crate) wire: Rc<RefCell<Wire>>,
            pub(crate) lifetime: PhantomData<&'d Dm>,
        }
        pub struct Tx<'d, Dm>(SpiDma<'d, Dm>, DmaTxBuf);
        impl<'d, Dm> Tx<'d, Dm> {
            pub fn is_done(&self) -> bool {
                true
            }
            pub fn wait(self) -> (SpiDma<'d, Dm>, DmaTxBuf) {
                self.0.wire.borrow_mut().bytes.extend_from_slice(&self.1.0[..self.1.1]);
                (self.0, self.1)
            }
        }
        pub struct Rx<'d, Dm>(SpiDma<'d, Dm>, DmaRxBuf);
        impl<'d, Dm> Rx<'d, Dm> {
            pub fn wait(mut self) -> (SpiDma<'d, Dm>, DmaRxBuf) {
                self.1.0[..self.1.1].fill(0x5a);
                (self.0, self.1)
            }
        }
        impl<'d, Dm> SpiDma<'d, Dm> {
            pub fn apply_config(&mut self, _: &Config) -> Result<(), ConfigError> {
                Ok(())
            }
            pub fn half_duplex_write(
                self,
                _: DataMode,
                _: Command,
                _: Address,
                _: u8,
                len: usize,
                buffer: DmaTxBuf,
            ) -> Result<Tx<'d, Dm>, (Error, Self, DmaTxBuf)> {
                assert_eq!(len, buffer.1);
                let failed = {
                    let mut wire = self.wire.borrow_mut();
                    wire.submissions += 1;
                    wire.fail_on == Some(wire.submissions)
                };
                if failed { Err((Error, self, buffer)) } else { Ok(Tx(self, buffer)) }
            }
            pub fn half_duplex_read(
                self,
                _: DataMode,
                _: Command,
                _: Address,
                _: u8,
                len: usize,
                buffer: DmaRxBuf,
            ) -> Result<Rx<'d, Dm>, (Error, Self, DmaRxBuf)> {
                assert_eq!(len, buffer.1);
                Ok(Rx(self, buffer))
            }
        }
    }
}

#[path = "../dma_bus.rs"]
mod dma_bus;

use dma_bus::DmaPanelBus;
use spi::master::{Address, Command, DataMode, SpiDma};

fn bus(capacity: usize) -> (DmaPanelBus, Rc<RefCell<Wire>>) {
    let wire = Rc::new(RefCell::new(Wire::default()));
    let spi = SpiDma { wire: wire.clone(), lifetime: PhantomData };
    (
        DmaPanelBus::new(
            spi,
            dma::DmaRxBuf(vec![0; 4], 0),
            dma::DmaTxBuf(vec![0xa5; capacity], 0),
            dma::DmaTxBuf(vec![0x5a; capacity], 0),
        ),
        wire,
    )
}

fn write(bus: &mut DmaPanelBus, data: &[u8]) -> Result<(), spi::Error> {
    bus.half_duplex_write(DataMode, Command, Address, 0, data)
}

#[test]
fn transfers_preserve_byte_order_and_partial_tail_across_buffer_reuse() {
    for capacity in [1, 7, 8192] {
        let (mut bus, wire) = bus(capacity);
        let mut expected = Vec::new();
        for len in [0, 1, 2, 63, 64, 65, 8191, 8192, 8193, 16384, 153600, 3, 0, 7] {
            let data: Vec<u8> = (0..len).map(|i| (i * 37 + len) as u8).collect();
            write(&mut bus, &data).unwrap();
            expected.extend_from_slice(&data);
            assert_eq!(wire.borrow().bytes, expected, "capacity {capacity}, length {len}");
        }
    }
}

#[test]
fn failed_submission_restores_both_buffers_and_spi_for_retry() {
    for failed_chunk in 1..=4 {
        let (mut bus, wire) = bus(7);
        wire.borrow_mut().fail_on = Some(failed_chunk);
        assert_eq!(write(&mut bus, &[19; 25]), Err(spi::Error));
        {
            let mut wire = wire.borrow_mut();
            assert_eq!(wire.bytes, vec![19; (failed_chunk - 1) * 7]);
            wire.fail_on = None;
            wire.bytes.clear();
        }
        let data: Vec<u8> = (0..31).collect();
        write(&mut bus, &data).unwrap();
        assert_eq!(wire.borrow().bytes, data);
        let mut read = [0; 2];
        bus.half_duplex_read(DataMode, Command, Address, 1, &mut read).unwrap();
        assert_eq!(read, [0x5a; 2]);
        write(&mut bus, &[0x2c]).unwrap();
        assert_eq!(wire.borrow().bytes.last(), Some(&0x2c));
    }
}
