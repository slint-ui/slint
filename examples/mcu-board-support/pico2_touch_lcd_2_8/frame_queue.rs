// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use slint::platform::software_renderer::{PhysicalRegion, Rgb565BigEndianPixel};

embassy_rp::bind_interrupts!(struct DmaInterrupts {
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH0>;
});

const CAPACITY: usize = 32 * 1024;
const SLOT_COUNT: usize = 3;
use super::packet_slot::{Slot, Write};
pub static DISPLAY_READY: AtomicBool = AtomicBool::new(false);
static NEXT_SLOT: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
static COUNTER_VERSION: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
static NEXT_FRAME: AtomicU32 = AtomicU32::new(1);
#[cfg(feature = "pico2-queue-profile")]
static DELIVERED_FRAME: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
static PRESENTED: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
static CHANGING: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
static TRANSFER_US: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
static TRANSFER_FRAMES: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
static TRANSPORT: [AtomicU32; 3] = [const { AtomicU32::new(0) }; 3];

#[cfg(feature = "pico2-queue-profile")]
pub fn transport_counters() -> [u32; 3] {
    TRANSPORT.each_ref().map(|value| value.load(Ordering::Relaxed))
}

#[derive(Clone, Copy)]
struct Rectangle {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    offset: usize,
}
const EMPTY_RECTANGLE: Rectangle = Rectangle { x: 0, y: 0, width: 0, height: 0, offset: 0 };
struct Packet {
    rectangles: [Rectangle; 48],
    count: usize,
    used: usize,
    #[cfg(feature = "pico2-queue-profile")]
    frame_hash: u32,
    #[cfg(feature = "pico2-queue-profile")]
    frame_id: u32,
    first: bool,
    frame_columns: [u32; super::uniform_tiles::COLUMNS],
    last: bool,
    bytes: super::wire_format::PacketBytes<CAPACITY>,
}
impl Packet {
    const fn empty() -> Self {
        Self {
            rectangles: [EMPTY_RECTANGLE; 48],
            count: 0,
            used: 0,
            #[cfg(feature = "pico2-queue-profile")]
            frame_hash: 0,
            #[cfg(feature = "pico2-queue-profile")]
            frame_id: 0,
            first: false,
            frame_columns: [0; super::uniform_tiles::COLUMNS],
            last: false,
            bytes: super::wire_format::PacketBytes([0; CAPACITY]),
        }
    }
}
static PACKETS: [Slot<Packet>; SLOT_COUNT] = [const { Slot::new(Packet::empty()) }; SLOT_COUNT];
#[derive(Default)]
pub struct PublishStats {
    #[cfg(feature = "pico2-queue-profile")]
    pub frame_id: u32,
    #[cfg(feature = "pico2-queue-profile")]
    pub wait_us: u64,
    #[cfg(feature = "pico2-queue-profile")]
    pub copy_us: u64,
    pub pixels: u32,
    pub lines: u32,
    #[cfg(feature = "pico2-queue-profile")]
    pub plan_us: u64,
}

async fn reserve(_stats: &mut PublishStats) -> Write<'static, Packet> {
    let index = NEXT_SLOT.load(Ordering::Relaxed);
    NEXT_SLOT.store((index + 1) % SLOT_COUNT as u32, Ordering::Relaxed);
    let slot = &PACKETS[index as usize];
    #[cfg(feature = "pico2-queue-profile")]
    let started = embassy_time::Instant::now();
    let mut packet = loop {
        if let Some(packet) = slot.try_write() {
            break packet;
        }
        embassy_time::Timer::after_micros(100).await;
    };
    #[cfg(feature = "pico2-queue-profile")]
    {
        _stats.wait_us += started.elapsed().as_micros();
    }
    packet.count = 0;
    packet.used = 0;
    packet.last = false;
    packet.first = false;
    packet
}

pub async fn publish(
    pixels: &[Rgb565BigEndianPixel],
    region: &PhysicalRegion,
    _frame_hash: u32,
    uniform_tiles: &mut [u32],
) -> PublishStats {
    let mut stats = PublishStats::default();
    let position = region.bounding_box_origin();
    let size = region.bounding_box_size();
    if size.width == 0 || size.height == 0 {
        return stats;
    }
    let source =
        unsafe { core::slice::from_raw_parts(pixels.as_ptr().cast::<u16>(), pixels.len()) };
    #[cfg(feature = "pico2-queue-profile")]
    let plan_started = embassy_time::Instant::now();
    let columns = super::uniform_tiles::plan(
        source,
        uniform_tiles,
        position.x as usize,
        position.y as usize,
        size.width as usize,
        size.height as usize,
    );
    #[cfg(feature = "pico2-queue-profile")]
    {
        stats.plan_us = plan_started.elapsed().as_micros();
    }
    let mut rectangles = super::uniform_tiles::rectangles(&columns).peekable();
    if rectangles.peek().is_none() {
        return stats;
    }
    #[cfg(feature = "pico2-queue-profile")]
    {
        stats.frame_id = NEXT_FRAME.fetch_add(1, Ordering::Relaxed);
    }
    let mut packet = reserve(&mut stats).await;
    #[cfg(feature = "pico2-queue-profile")]
    {
        packet.frame_id = stats.frame_id;
        packet.frame_hash = _frame_hash;
    }
    packet.first = true;
    packet.frame_columns = columns;
    while let Some((x, y, height)) = rectangles.next() {
        let (x, y, height) = (usize::from(x), usize::from(y), usize::from(height));
        let width = super::uniform_tiles::TILE;
        let length = width * height * 2;
        if packet.count == packet.rectangles.len() || CAPACITY - packet.used < length {
            packet.publish();
            packet = reserve(&mut stats).await;
            #[cfg(feature = "pico2-queue-profile")]
            {
                packet.frame_id = stats.frame_id;
                packet.frame_hash = _frame_hash;
            }
        }
        #[cfg(feature = "pico2-queue-profile")]
        let started = embassy_time::Instant::now();
        let offset = packet.used;
        let count = packet.count;
        packet.rectangles[count] = Rectangle {
            x: x as u16,
            y: y as u16,
            width: width as u16,
            height: height as u16,
            offset,
        };
        packet.count += 1;
        let frame =
            unsafe { core::slice::from_raw_parts(pixels.as_ptr().cast::<u8>(), pixels.len() * 2) };
        super::wire_format::pack(
            frame,
            320,
            x,
            y,
            width,
            height,
            &mut packet.bytes.0[offset..offset + length],
        );
        packet.used += length;
        packet.last = rectangles.peek().is_none();
        #[cfg(feature = "pico2-queue-profile")]
        {
            stats.copy_us += started.elapsed().as_micros();
        }
        stats.pixels += (width * height) as u32;
        stats.lines += height as u32;
    }
    packet.publish();
    stats
}

fn byte(byte: u8) {
    while !embassy_rp::pac::SPI1.sr().read().tnf() {}
    embassy_rp::pac::SPI1.dr().write(|w| w.set_data(u16::from(byte)));
}
#[unsafe(link_section = ".data.ram_func")]
#[inline(never)]
fn command(command: u8, data: &[u8]) {
    super::cs_low();
    embassy_rp::pac::SIO.gpio_out(0).value_clr().write_value(super::DC_PIN_MASK);
    byte(command);
    super::flush_spi1();
    super::dc_high();
    for &value in data {
        byte(value);
    }
    super::flush_spi1();
    super::cs_high();
    super::drain_spi1_rx();
}

pub fn configure_scan() {
    command(
        0xb2,
        &[
            super::scan_budget::BACK_PORCH as u8,
            super::scan_budget::FRONT_PORCH as u8,
            0,
            0x33,
            0x33,
        ],
    );
}

#[cfg(feature = "pico2-queue-profile")]
pub fn panel_check(expected: &[u8]) -> [u32; 4] {
    assert_eq!(expected.len(), 320 * 240 * 2);
    command(0x3a, &[0x66]);
    let mut pixels = [0u16; 320];
    let mut result = [0u32; 4];
    for y in 0u16..240 {
        command(0x2a, &[0, 0, 1, 63]);
        command(0x2b, &[0, y as u8, 0, y as u8]);
        super::panel_scan::memory_row(&mut pixels);
        for (x, actual) in pixels.into_iter().enumerate() {
            let index = usize::from(y) * 320 + x;
            let expected = u16::from_be_bytes([expected[index * 2], expected[index * 2 + 1]]);
            if expected != actual {
                if result[0] == 0 {
                    result[1] = index as u32;
                    result[2] = u32::from(expected);
                    result[3] = u32::from(actual);
                }
                result[0] += 1;
            }
        }
    }
    command(0x3a, &[0x55]);
    result
}

struct RamFuture<F>(F);

impl<F: core::future::Future> core::future::Future for RamFuture<F> {
    type Output = F::Output;

    #[unsafe(link_section = ".data.ram_func")]
    #[inline(never)]
    fn poll(
        self: core::pin::Pin<&mut Self>,
        context: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        // The pinned wrapper never moves its inner future.
        unsafe { self.map_unchecked_mut(|this| &mut this.0) }.poll(context)
    }
}

#[embassy_executor::task]
pub fn display_task(
    channel: embassy_rp::Peri<'static, embassy_rp::peripherals::DMA_CH0>,
    backlight: embassy_rp::gpio::Output<'static>,
) -> impl core::future::Future<Output = ()> {
    RamFuture(display_loop(channel, backlight))
}

async fn display_loop(
    channel: embassy_rp::Peri<'static, embassy_rp::peripherals::DMA_CH0>,
    mut backlight: embassy_rp::gpio::Output<'static>,
) {
    let mut visible = false;
    let mut dma = embassy_rp::dma::Channel::new(channel, DmaInterrupts);
    #[cfg(feature = "pico2-queue-profile")]
    let spi_hz = {
        let spi = embassy_rp::pac::SPI1;
        embassy_rp::clocks::clk_peri_freq()
            / (u32::from(spi.cpsr().read().cpsdvsr()) * (u32::from(spi.cr0().read().scr()) + 1))
    };
    #[cfg(feature = "pico2-queue-profile")]
    let mut previous_hash = 0;
    #[cfg(feature = "pico2-queue-profile")]
    let mut frame_us = 0u64;
    let mut scan = super::panel_sync::ScanClock::default();
    let mut wave = 0;
    let mut last_sample = embassy_time::Instant::now();
    let mut next_slot = 0;
    loop {
        if DISPLAY_READY.load(Ordering::Acquire) && last_sample.elapsed().as_micros() >= 500 {
            scan.sample();
            last_sample = embassy_time::Instant::now();
        }
        let packet = if DISPLAY_READY.load(Ordering::Acquire) {
            PACKETS[next_slot].try_read()
        } else {
            None
        };
        let Some(packet) = packet else {
            embassy_time::Timer::after_micros(100).await;
            continue;
        };
        #[cfg(feature = "pico2-queue-profile")]
        let started = embassy_time::Instant::now();
        if packet.first {
            wave = scan.start_frame(&packet.frame_columns).await;
        }
        for rect in &packet.rectangles[..packet.count] {
            scan.wait_after(wave, rect.x, rect.width).await;
            #[cfg(feature = "pico2-queue-profile")]
            let busy_started = embassy_time::Instant::now();
            let x_end = rect.x + rect.width - 1;
            let y_end = rect.y + rect.height - 1;
            command(
                0x2a,
                &[
                    rect.x.to_be_bytes()[0],
                    rect.x.to_be_bytes()[1],
                    x_end.to_be_bytes()[0],
                    x_end.to_be_bytes()[1],
                ],
            );
            command(
                0x2b,
                &[
                    rect.y.to_be_bytes()[0],
                    rect.y.to_be_bytes()[1],
                    y_end.to_be_bytes()[0],
                    y_end.to_be_bytes()[1],
                ],
            );
            command(0x2c, &[]);
            let spi = embassy_rp::pac::SPI1;
            spi.cr1().modify(|w| w.set_sse(false));
            spi.cr0().modify(|w| {
                w.set_dss(15);
                w.set_spo(true);
                w.set_sph(true);
            });
            spi.cr1().modify(|w| w.set_sse(true));
            super::cs_low();
            super::dc_high();
            let length = rect.width as usize * rect.height as usize * 2;
            let transfer = unsafe {
                let words = packet.bytes.words(rect.offset, length);
                dma.write(
                    words as *const [u16],
                    0x4008_8008 as *mut u16,
                    embassy_rp::pac::dma::vals::TreqSel::SPI1_TX,
                    cfg!(target_endian = "little"),
                )
            };
            transfer.await;
            super::flush_spi1();
            super::drain_spi1_rx();
            super::cs_high();
            spi.cr1().modify(|w| w.set_sse(false));
            spi.cr0().modify(|w| {
                w.set_dss(7);
                w.set_spo(false);
                w.set_sph(false);
            });
            spi.cr1().modify(|w| w.set_sse(true));

            #[cfg(feature = "pico2-queue-profile")]
            {
                let busy_us = busy_started.elapsed().as_micros() as u32;
                let wire_us = (length as u64 * 8_000_000).div_ceil(u64::from(spi_hz)) as u32;
                TRANSPORT[0].fetch_add(1, Ordering::Relaxed);
                TRANSPORT[1].fetch_add(busy_us, Ordering::Relaxed);
                TRANSPORT[2].fetch_max(busy_us.saturating_sub(wire_us), Ordering::Relaxed);
            }

            scan.complete_strip(wave, rect.x);
        }
        #[cfg(feature = "pico2-queue-profile")]
        {
            frame_us += started.elapsed().as_micros();
        }
        if packet.last {
            #[cfg(feature = "pico2-queue-profile")]
            {
                COUNTER_VERSION.fetch_add(1, Ordering::AcqRel);
                TRANSFER_US.fetch_add(frame_us as u32, Ordering::Relaxed);
                TRANSFER_FRAMES.fetch_add(1, Ordering::Relaxed);
                PRESENTED.fetch_add(1, Ordering::Release);
                let current_hash = packet.frame_hash;
                let frame_changed = current_hash != previous_hash;
                if frame_changed {
                    CHANGING.fetch_add(1, Ordering::Release);
                }
                COUNTER_VERSION.fetch_add(1, Ordering::Release);
                DELIVERED_FRAME.store(packet.frame_id, Ordering::Release);
                previous_hash = current_hash;
                frame_us = 0;
            }
            if !visible {
                // ST7789 scans GRAM continuously; one full scan replaces pixels latched before the upload.
                embassy_time::Timer::after_millis(20).await;
                backlight.set_high();
                visible = true;
            }
            super::usb::FRAME_READY.store(true, Ordering::Release);
        }
        drop(packet);
        next_slot = (next_slot + 1) % SLOT_COUNT;
    }
}

#[cfg(feature = "pico2-queue-profile")]
pub fn counters() -> [u32; 4] {
    loop {
        let version = COUNTER_VERSION.load(Ordering::Acquire);
        if version & 1 != 0 {
            core::hint::spin_loop();
            continue;
        }
        let result = [
            PRESENTED.load(Ordering::Relaxed),
            CHANGING.load(Ordering::Relaxed),
            TRANSFER_US.load(Ordering::Relaxed),
            TRANSFER_FRAMES.load(Ordering::Relaxed),
        ];
        core::sync::atomic::fence(Ordering::Acquire);
        if version == COUNTER_VERSION.load(Ordering::Relaxed) {
            return result;
        }
    }
}

#[cfg(feature = "pico2-queue-profile")]
pub async fn wait_delivered(frame: u32) {
    let frame = if frame == 0 { NEXT_FRAME.load(Ordering::Relaxed).wrapping_sub(1) } else { frame };
    while DELIVERED_FRAME.load(Ordering::Acquire).wrapping_sub(frame) > u32::MAX / 2 {
        embassy_time::Timer::after_micros(50).await;
    }
}
