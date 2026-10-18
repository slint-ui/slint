// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use embassy_executor::InterruptExecutor;
use embassy_rp::interrupt::{InterruptExt, Priority, typelevel::Handler};

static EXECUTOR: InterruptExecutor = InterruptExecutor::new();

pub struct Doorbell;
impl Handler<embassy_rp::interrupt::typelevel::SIO_IRQ_BELL> for Doorbell {
    unsafe fn on_interrupt() {
        let sio = embassy_rp::pac::SIO;
        sio.doorbell_in_clr().write_value(embassy_rp::pac::sio::regs::DoorbellInClr(1));
        if sio.cpuid().read() == 0 {
            // Embassy's interrupt pender targets the calling core, including timer callbacks.
            sio.doorbell_out_set().write_value(embassy_rp::pac::sio::regs::DoorbellOutSet(1));
        } else {
            // Only core 1 polls this executor, after start has initialized it.
            unsafe { EXECUTOR.on_interrupt() };
        }
    }
}
embassy_rp::bind_interrupts!(struct Irqs { SIO_IRQ_BELL => Doorbell; });

pub fn route_core0_wakes() {
    use embassy_rp::interrupt::SIO_IRQ_BELL;
    let _ = Irqs;
    embassy_rp::pac::SIO.doorbell_in_clr().write_value(embassy_rp::pac::sio::regs::DoorbellInClr(1));
    SIO_IRQ_BELL.set_priority(Priority::P2);
    SIO_IRQ_BELL.unpend();
    unsafe { SIO_IRQ_BELL.enable() };
}

pub fn start(
    dma: embassy_rp::Peri<'static, embassy_rp::peripherals::DMA_CH0>,
    backlight: embassy_rp::gpio::Output<'static>,
) {
    use embassy_rp::interrupt::{DMA_IRQ_0, SIO_IRQ_BELL, USBCTRL_IRQ};
    DMA_IRQ_0.set_priority(Priority::P1);
    USBCTRL_IRQ.set_priority(Priority::P3);
    SIO_IRQ_BELL.set_priority(Priority::P2);
    embassy_rp::pac::SIO.doorbell_in_clr().write_value(embassy_rp::pac::sio::regs::DoorbellInClr(1));
    EXECUTOR.start(SIO_IRQ_BELL).spawn(super::frame_queue::display_task(dma, backlight).unwrap());
}
