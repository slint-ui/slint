// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell:ignore flashhash flashinfo rcmd rfmt

#[cfg(feature = "pico2-queue-profile")]
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_rp::multicore::Stack;
use embassy_rp::peripherals::{CORE1, USB};
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::{Peri, bind_interrupts};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::control::{OutResponse, Recipient, Request, RequestType};
use embassy_usb::{Builder, Config, Handler};
use static_cell::StaticCell;

#[cfg(feature = "pico2-queue-profile")]
pub static CAPTURE_STATE: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
pub static CAPTURE_OFFSET: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "pico2-queue-profile")]
pub static CAPTURE_PIXELS: core::sync::atomic::AtomicPtr<u8> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());

#[cfg(feature = "pico2-queue-profile")]
pub static CHECK_PANEL: AtomicBool = AtomicBool::new(false);
pub static FRAME_READY: AtomicBool = AtomicBool::new(false);

bind_interrupts!(struct UsbInterrupts {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

pub fn start(
    usb: Peri<'static, USB>,
    core1: Peri<'static, CORE1>,
    dma: Peri<'static, embassy_rp::peripherals::DMA_CH0>,
    backlight: embassy_rp::gpio::Output<'static>,
) {
    #[unsafe(link_section = ".core1_stack")]
    static mut STACK: core::mem::MaybeUninit<Stack<8192>> = core::mem::MaybeUninit::uninit();
    // The CORE1 token gives this call exclusive ownership of the dedicated stack.
    let stack = unsafe {
        let stack = core::ptr::addr_of_mut!(STACK).cast::<Stack<8192>>();
        stack.write_bytes(0, 1);
        &mut *stack
    };
    super::display_executor::route_core0_wakes();
    embassy_rp::multicore::spawn_core1(core1, stack, move || {
        super::display_executor::start(dma, backlight);
        static EXECUTOR: StaticCell<embassy_executor::Executor> = StaticCell::new();
        EXECUTOR.init(embassy_executor::Executor::new()).run(|spawner| {
            spawner.spawn(usb_task(usb).unwrap());
            spawner.spawn(super::boot_guard::watchdog_task().unwrap());

            spawner.spawn(super::background::task().unwrap());
        });
    });
}

struct ResetInterface(u16);

impl Handler for ResetInterface {
    fn control_out(&mut self, req: Request, data: &[u8]) -> Option<OutResponse> {
        if req.request_type != RequestType::Class
            || req.recipient != Recipient::Interface
            || req.index != self.0
        {
            return None;
        }
        if req.length != 0 || !data.is_empty() {
            return Some(OutResponse::Rejected);
        }
        // Raspberry Pi's USB reset protocol uses requests 1 and 2, with the interface mask in bits 0–1.
        // https://github.com/raspberrypi/pico-sdk/blob/master/src/rp2_common/pico_usb_reset/usb_reset.c
        match req.request {
            1 => {
                super::boot_guard::permit_next_boot();
                embassy_rp::rom_data::reset_to_usb_boot(0, u32::from(req.value & 3));
                Some(OutResponse::Accepted)
            }
            2 => {
                super::boot_guard::permit_next_boot();
                embassy_rp::rom_data::reboot(0, 100, 0, 0);
                Some(OutResponse::Accepted)
            }
            _ => Some(OutResponse::Rejected),
        }
    }
}

#[embassy_executor::task]
async fn usb_task(usb: Peri<'static, USB>) {
    static SERIAL: StaticCell<[u8; 16]> = StaticCell::new();
    let serial = SERIAL.init([0; 16]);
    let chip_id = embassy_rp::otp::get_chipid().unwrap();
    for (i, byte) in serial.iter_mut().enumerate() {
        *byte = b"0123456789ABCDEF"[((chip_id >> (60 - i * 4)) & 15) as usize];
    }

    let mut config = Config::new(0x2e8a, 0x0009);
    config.manufacturer = Some("Slint");
    config.product = Some("Pico 2 MCU Printer");
    config.serial_number = Some(core::str::from_utf8(serial).unwrap());
    config.max_power = 250;

    static CONFIG: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL: StaticCell<[u8; 64]> = StaticCell::new();
    let driver = Driver::new(usb, UsbInterrupts);
    let mut builder = Builder::new(
        driver,
        config,
        CONFIG.init([0; 256]),
        BOS.init([0; 256]),
        &mut [],
        CONTROL.init([0; 64]),
    );
    static CDC_STATE: StaticCell<State> = StaticCell::new();
    let mut serial = CdcAcmClass::new(&mut builder, CDC_STATE.init(State::new()), 64);
    let interface_number = {
        let mut function = builder.function(0xff, 0, 1);
        let mut interface = function.interface();
        let number = interface.interface_number().0;
        interface.alt_setting(0xff, 0, 1, None);
        number
    };
    static RESET: StaticCell<ResetInterface> = StaticCell::new();
    builder.handler(RESET.init(ResetInterface(u16::from(interface_number))));

    let mut device = builder.build();
    embassy_futures::join::join(device.run(), async {
        let mut buffer = [0; 64];
        loop {
            serial.wait_connection().await;
            loop {
                let Ok(count) = serial.read_packet(&mut buffer).await else { break };
                #[cfg(all(feature = "pico2-queue-profile", not(feature = "pico2-clock-240")))]
                if buffer[..count].starts_with(b"watchdog-test ") {
                    let core = match core::str::from_utf8(&buffer[14..count]).unwrap_or("").trim() {
                        "main" => 1,
                        "display" => 2,
                        _ => continue,
                    };
                    let _ = serial.write_packet(b"PICO_WATCHDOG_TEST_OK\n").await;
                    super::boot_guard::TEST_STALL.store(core, Ordering::Release);
                    continue;
                }
                #[cfg(feature = "pico2-queue-profile")]
                if buffer[..count].starts_with(b"mode ") {
                    let mode = match core::str::from_utf8(&buffer[5..count]).unwrap_or("").trim() {
                        "auto" => 0, "pan" => 1, "manual" => 2, "pan-fast" => 3, "ink" => 4, "settings-auto" => 5, "settings-pan" => 6, "poll-pan" => 7, "settings-poll-pan" => 8, "ink-boost" => 9, "ink-touch" => 10, "kiosk" => 11, "ink-tilt" => 12, "ink-tilt-boost" => 13,
                        _ => continue,
                    };
                    super::profile::RUN.select(mode);
                    let _ = serial.write_packet(b"PICO_MODE_OK\n").await;
                    continue;
                }
                #[cfg(feature = "pico2-queue-profile")]
                if buffer[..count].starts_with(b"rotation ") {
                    let mode = match core::str::from_utf8(&buffer[9..count]).unwrap_or("").trim() { "0" => 0, "1" => 1, "auto" => 2, _ => continue };
                    super::imu::force_rotation(mode);
                    let _ = serial.write_packet(b"PICO_ROTATION_OK\n").await;
                    continue;
                }
                #[cfg(feature = "pico2-queue-profile")]
                if buffer[..count].starts_with(b"tilt ") {
                    let text = core::str::from_utf8(&buffer[5..count]).unwrap_or("").trim();
                    if text == "live" { super::imu::force_up(None); }
                    else {
                        let mut parts = text.split_whitespace();
                        let (Some(x), Some(y)) = (parts.next().and_then(|s| s.parse::<i16>().ok()), parts.next().and_then(|s| s.parse::<i16>().ok())) else { continue };
                        let length = (i32::from(x) * i32::from(x) + i32::from(y) * i32::from(y)) as u32;
                        if !(900_000..=1_100_000).contains(&length) || parts.next().is_some() { continue; }
                        super::imu::force_up(Some([x, y]));
                    }
                    let _ = serial.write_packet(b"PICO_TILT_OK\n").await;
                    continue;
                }
                #[cfg(feature = "pico2-queue-profile")]
                if buffer[..count].starts_with(b"pointer ") {
                    let mut parts = core::str::from_utf8(&buffer[8..count]).unwrap_or("").split_whitespace();
                    let kind = match parts.next() {
                        Some("down") => 1, Some("move") => 2, Some("up") => 3, _ => continue,
                    };
                    let (Some(x), Some(y)) = (parts.next().and_then(|s| s.parse::<f32>().ok()), parts.next().and_then(|s| s.parse::<f32>().ok())) else { continue };
                    if !x.is_finite() || !y.is_finite() || parts.next().is_some() { continue; }
                    super::profile::schedule_pointer_kind(x, y, kind);
                    let _ = serial.write_packet(b"PICO_POINTER_OK\n").await;
                    continue;
                }
                #[cfg(feature = "pico2-queue-profile")]
                if buffer[..count].starts_with(b"capture ") || buffer[..count].starts_with(b"panelcheck ") {
                    let panel = buffer[..count].starts_with(b"panelcheck ");
                    let start = if panel { 11 } else { 8 };
                    let offset = core::str::from_utf8(&buffer[start..count]).ok().and_then(|s| s.trim().parse::<u32>().ok());
                    let Some(offset) = offset else { continue };
                    if CAPTURE_STATE.load(Ordering::Acquire) != 0 { continue; }
                    CHECK_PANEL.store(panel, Ordering::Relaxed);
                    CAPTURE_OFFSET.store(offset, Ordering::Relaxed);
                    CAPTURE_STATE.store(1, Ordering::Release);
                    while CAPTURE_STATE.load(Ordering::Acquire) != 2 {
                        embassy_time::Timer::after_millis(1).await;
                    }
                    if panel {
                        let pixels = unsafe { core::slice::from_raw_parts(CAPTURE_PIXELS.load(Ordering::Acquire), 153600) };
                        let result = super::frame_queue::panel_check(pixels);
                        CAPTURE_STATE.store(3, Ordering::Release);
                        let mut line = ReportLine { bytes: [0; 2048], length: 0 };
                        let _ = writeln!(&mut line, "PICO_PANEL_CHECK,76800,{},{},{},{}", result[0], result[1], result[2], result[3]);
                        if serial.write_packet(&line.bytes[..line.length]).await.is_err() { break; }
                        continue;
                    }
                    let mut header = ReportLine { bytes: [0; 2048], length: 0 };
                    #[cfg(feature = "pico2-queue-profile")]
                    let _ = writeln!(&mut header, "PICO_FRAME,320,240,153600,offset={offset},format=be,crc32={}",super::frame_crc::LAST.load(Ordering::Acquire));
                    let mut disconnected = serial.write_packet(&header.bytes[..header.length]).await.is_err();
                    // The main core owns this snapshot and releases it only after state 3.
                    let pixels = unsafe { core::slice::from_raw_parts(CAPTURE_PIXELS.load(Ordering::Acquire), 153600) };
                    if !disconnected {
                        for packet in pixels.chunks(64) {
                            if serial.write_packet(packet).await.is_err() { disconnected = true; break; }
                        }
                    }
                    if !disconnected {
                        disconnected = serial.write_packet(&[]).await.is_err();
                    }
                    CAPTURE_STATE.store(3, Ordering::Release);
                    if disconnected { break; }
                    continue;
                }
                #[cfg(feature = "pico2-queue-profile")]
                if buffer[..count].starts_with(b"flashhash ") {
                    let mut parts = core::str::from_utf8(&buffer[10..count]).unwrap_or("").split_whitespace();
                    let (Some(offset), Some(length)) = (parts.next().and_then(|v| u32::from_str_radix(v, 16).ok()), parts.next().and_then(|v| u32::from_str_radix(v, 16).ok())) else { continue };
                    if offset & 3 != 0 || length & 3 != 0 || length == 0 || offset.checked_add(length).is_none_or(|end| end > 16 * 1024 * 1024) || parts.next().is_some() { continue; }
                    let mut hash = 0x811c9dc5u32;
                    for at in (offset..offset + length).step_by(4) {
                        let word = unsafe { core::ptr::read_volatile((0x14000000 + at) as *const u32) };
                        for byte in word.to_le_bytes() { hash = (hash ^ u32::from(byte)).wrapping_mul(0x01000193); }
                        if (at - offset) & 1023 == 1020 { embassy_time::Timer::after_micros(1).await; }
                    }
                    let mut line = ReportLine { bytes: [0; 2048], length: 0 };
                    let _ = writeln!(&mut line, "PICO_FLASH_HASH,offset={offset},bytes={length},fnv32={hash}");
                    for packet in line.bytes[..line.length].chunks(64) {
                        if serial.write_packet(packet).await.is_err() { break; }
                    }
                    continue;
                }
                #[cfg(feature = "pico2-queue-profile")]
                if buffer[..count].starts_with(b"flashinfo") {
                    let memory = embassy_rp::pac::QMI.mem(0);
                    let timing = memory.timing().read();
                    let divisor = match timing.clkdiv() { 0 => 256, value => u32::from(value) };
                    let mut line = ReportLine { bytes: [0; 2048], length: 0 };
                    let _ = writeln!(&mut line, "PICO_FLASH,sys_hz={},flash_hz={},clkdiv={},timing={},rfmt={},rcmd={}", embassy_rp::clocks::clk_sys_freq(), embassy_rp::clocks::clk_sys_freq() / divisor, divisor, timing.0, memory.rfmt().read().0, memory.rcmd().read().0);
                    for packet in line.bytes[..line.length].chunks(64) {
                        if serial.write_packet(packet).await.is_err() { break; }
                    }
                    continue;
                }
                #[cfg(feature = "pico2-queue-profile")]
                if buffer[..count].starts_with(b"imu") {
                    let (raw, samples, errors, us) = super::imu::telemetry();
                    let up = super::ink_up();
                    let mut line = ReportLine { bytes: [0; 2048], length: 0 };
                    let _ = writeln!(&mut line, "PICO_IMU,ax={},ay={},az={},gx={},gy={},gz={},up_x={},up_y={},samples={samples},errors={errors},read_us={us}", raw[0],raw[1],raw[2],raw[3],raw[4],raw[5],up[0],up[1]);
                    for packet in line.bytes[..line.length].chunks(64) {
                        if serial.write_packet(packet).await.is_err() { break; }
                    }
                    continue;
                }
                #[cfg(feature = "pico2-queue-profile")]
                if buffer[..count].starts_with(b"stats") {
                    let format_started = super::profile::now_us();
                    let (sequence, v) = super::profile::snapshot();
                    let spi = embassy_rp::pac::SPI1;
                    let divisor = u32::from(spi.cpsr().read().cpsdvsr()) * (u32::from(spi.cr0().read().scr()) + 1);
                    let mut line = ReportLine { bytes: [0; 2048], length: 0 };

                    let extra = super::frame_queue::counters();
                    let _ = write!(&mut line,
                        "PICO_QUEUE,pipeline={},presented_total={},changing_total={},transfer_us={},transfer_frames={},renderer=framebuffer,window={},cpu_hz={},spi_hz={},elapsed_us={},frames={},draw_us={},raster_us={},wait_us={},window_us={},pixels={},lines={},draw_max_us={},interval_max_us={},skipped_phases={},presented_frames={},changing_frames={},touch_us={},dispatch_us={},touch_calls={},dispatch_calls={},heap_free={}",
                        "parallel", extra[0],extra[1],extra[2],extra[3],
                        sequence, embassy_rp::clocks::clk_sys_freq(), embassy_rp::clocks::clk_peri_freq() / divisor,
                        v[0],v[1],v[2],v[3],v[4],v[5],v[6],v[7],v[8],v[9],v[10],v[11],v[12],v[25],v[26],v[27],v[28],super::ALLOCATOR.free());
                    let _ = write!(&mut line, ",display_flipped={},display_flips={}", u32::from(super::frame_queue::flipped()), super::frame_queue::flips());
                    let _ = write!(&mut line, ",peri_hz={},usb_hz={},adc_hz={}", embassy_rp::clocks::clk_peri_freq(), embassy_rp::clocks::clk_usb_freq(), embassy_rp::clocks::clk_adc_freq());
                    #[cfg(not(feature = "pico2-renderer-profile"))]
                    let _ = write!(&mut line, ",draw_max_raster_us={},draw_max_crc_us={},draw_max_plan_us={},draw_max_pack_us={},draw_max_wait_us={}", v[13], v[14], v[15], v[16], v[17]);
                    #[cfg(feature = "pico2-renderer-profile")]
                    for (index, name) in i_slint_renderer_software::embedded_profile::NAMES.iter().enumerate() {
                        let _ = write!(&mut line, ",{name}_cpu_us={},{}_calls={}", v[13 + index], name, v[19 + index]);
                    }
                    #[cfg(feature = "pico2-renderer-profile")]
                    let _ = write!(&mut line, ",text_cache_hits={},text_cache_misses={},text_cache_evictions={},text_cache_entries={},text_cache_glyphs={}", v[43], v[44], v[45], v[46], v[47]);
                    #[cfg(feature = "pico2-renderer-profile")]
                    for (index, name) in i_slint_renderer_software::embedded_profile::TEXT_STAGES.iter().enumerate() {
                        let _ = write!(&mut line, ",{name}_us={},{name}_calls={}", v[48 + index], v[52 + index]);
                    }

                    { let panel = super::panel_sync::counters();
                        let _ = write!(&mut line, ",panel_ticks={},panel_period_us={},late_strips={},scan_errors={},window_panel_ticks={},window_late_strips={},window_scan_errors={}",panel[0],panel[1],panel[2],panel[3],v[29],v[30],v[31]);
                    }
                    #[cfg(feature = "pico2-queue-profile")]
                    { let _ = write!(&mut line, ",crc_us={},crc_known={},last_crc={}",v[32],super::frame_crc::KNOWN.load(Ordering::Acquire),super::frame_crc::LAST.load(Ordering::Acquire)); }
                    let _ = write!(&mut line, ",mode={},animation_us={},profile_run={},current_run={}",v[33],v[34],v[42],super::profile::RUN.token());
                    let kiosk = super::profile::KIOSK_STATE.load(Ordering::Acquire);
                    let _ = write!(&mut line, ",kiosk_enabled={},kiosk_stage={}", kiosk >> 8, kiosk & 255);
                    let _ = write!(&mut line, ",window_kiosk_stage={}", v[56]);
                    let _ = write!(&mut line, ",plan_us={},transport_rectangles={},transport_busy_us={},transport_max_overhead_us={}", v[57], v[58], v[59], v[60]);
                    let late = super::panel_sync::last_late();
                    let _ = write!(&mut line, ",late_x={},late_scan={},late_wave_delta={},late_frame_us={}",late[0],late[1],late[2],late[3]);
                    let _ = write!(&mut line, ",stack_free={}", super::profile::stack_free());
                    let _ = write!(&mut line, ",input_to_raster_us={},input_to_raster_max_us={},input_to_raster_calls={}", v[35], v[36], v[37]);
                    if matches!(v[33], 4 | 9 | 10 | 12 | 13) {
                        let state = super::profile::INK_STATE.load(Ordering::Acquire);
                        let counts: [u32; 4] = core::array::from_fn(|i| (state >> (i * 8)) & 127);
                        let mask = (0..4).fold(0, |mask, i| mask | (((state >> (i * 8 + 7)) & 1) << i));
                        let _ = write!(&mut line, ",ink_held_mask={},ink_yellow_particles={},ink_red_particles={},ink_cyan_particles={},ink_black_particles={}", mask, counts[0], counts[1], counts[2], counts[3]);
                        let _ = write!(&mut line, ",ink_buffer_us={},ink_restore_us={},ink_stamp_us={},ink_publish_us={}", v[38], v[39], v[40], v[41]);
                    }
                    let format_us = (super::profile::now_us() - format_started) as u32;
                    super::profile::STATS_MAX_US.fetch_max(format_us, Ordering::Relaxed);
                    let _ = write!(&mut line, ",stats_cpu_us={},stats_max_cpu_us={}", format_us, super::profile::STATS_MAX_US.load(Ordering::Relaxed));
                    let _ = writeln!(&mut line);
                    let mut disconnected = false;
                    for packet in line.bytes[..line.length].chunks(64) {
                        if serial.write_packet(packet).await.is_err() {
                            disconnected = true;
                            break;
                        }
                    }
                    if !disconnected && line.length.is_multiple_of(64) {
                        disconnected = serial.write_packet(&[]).await.is_err();
                    }
                    if disconnected { break; }
                    continue;
                }
                let response = if buffer[..count].starts_with(b"status") {
                    if FRAME_READY.load(Ordering::Acquire) {
                        b"PICO_PRINTER_FRAME_READY\r\n".as_slice()
                    } else {
                        b"PICO_PRINTER_INITIALIZING\r\n".as_slice()
                    }
                } else {
                    &buffer[..count]
                };
                if serial.write_packet(response).await.is_err() {
                    break;
                }
            }
        }
    })
    .await;
}

#[cfg(feature = "pico2-queue-profile")]
struct ReportLine {
    bytes: [u8; 2048],
    length: usize,
}

#[cfg(feature = "pico2-queue-profile")]
impl core::fmt::Write for ReportLine {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        let end = self.length + text.len();
        let target = self.bytes.get_mut(self.length..end).ok_or(core::fmt::Error)?;
        target.copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
