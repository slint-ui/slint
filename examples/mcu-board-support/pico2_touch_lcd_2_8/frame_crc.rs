// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicU32, Ordering, compiler_fence};
use embassy_rp::pac;
use embassy_rp::pac::dma::vals::{Calc, DataSize, TreqSel};

pub static LAST: AtomicU32 = AtomicU32::new(0);
pub static KNOWN: AtomicU32 = AtomicU32::new(0);
static mut SINK: u32 = 0;

pub fn calculate(data: &[u8]) -> u32 {
    let channel = pac::DMA.ch(1);
    let words = data.as_ptr() as usize & 3 == 0 && data.len() % 4 == 0;
    pac::DMA.sniff_data().write_value(u32::MAX);
    pac::DMA.sniff_ctrl().write(|w| {
        w.set_en(true);
        w.set_dmach(1);
        w.set_calc(Calc::CRC32R);
        w.set_out_rev(true);
        w.set_out_inv(true);
    });
    channel.read_addr().write_value(data.as_ptr() as u32);
    channel.write_addr().write_value(core::ptr::addr_of_mut!(SINK) as u32);
    channel.trans_count().write(|w| {
        w.set_mode(0.into());
        w.set_count((data.len() / if words { 4 } else { 1 }) as u32);
    });
    compiler_fence(Ordering::SeqCst);
    channel.ctrl_trig().write(|w| {
        w.set_treq_sel(TreqSel::PERMANENT);
        w.set_data_size(if words { DataSize::SIZE_WORD } else { DataSize::SIZE_BYTE });
        w.set_incr_read(true);
        w.set_incr_write(false);
        w.set_chain_to(1);
        w.set_irq_quiet(true);
        w.set_sniff_en(true);
        w.set_en(true);
    });
    while channel.ctrl_trig().read().busy() {}
    compiler_fence(Ordering::SeqCst);
    let crc = pac::DMA.sniff_data().read();
    pac::DMA.sniff_ctrl().modify(|w| w.set_en(false));
    LAST.store(crc, Ordering::Release);
    crc
}
