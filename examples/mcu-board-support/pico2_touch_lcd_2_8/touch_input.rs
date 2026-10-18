// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use super::touch_samples::{Batch, Buffer, Kind, Sample};
use alloc::boxed::Box;
use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_rp::{
    gpio::Input,
    i2c::{Blocking, I2c},
    pac,
    peripherals::I2C1,
};
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    channel::Channel,
};

pub struct Devices {
    pub touch: super::cst328::CST328<I2c<'static, I2C1, Blocking>>,
    pub imu: super::imu::Imu,
    pub _pin: Input<'static>,
}

static DEVICES: Channel<CriticalSectionRawMutex, Box<Devices>, 1> = Channel::new();
static SAMPLES: Mutex<CriticalSectionRawMutex, RefCell<Buffer>> =
    Mutex::new(RefCell::new(Buffer::new()));
static FLIPPED: AtomicBool = AtomicBool::new(false);

pub fn configure(devices: Devices) {
    assert!(DEVICES.try_send(Box::new(devices)).is_ok());
}
pub fn flip(flipped: bool) {
    FLIPPED.store(flipped, Ordering::Release);
}
pub fn take() -> Option<Batch> {
    SAMPLES.lock(|samples| samples.borrow_mut().take(super::panel_tick()))
}
pub fn ready() -> bool {
    SAMPLES.lock(|samples| samples.borrow().ready(super::panel_tick()))
}
#[cfg(feature = "pico2-queue-profile")]
pub fn overflows() -> u32 {
    SAMPLES.lock(|samples| samples.borrow().overflows)
}

#[embassy_executor::task]
pub async fn task() {
    let mut devices = DEVICES.receive().await;
    #[cfg(feature = "pico2-queue-profile")]
    let mut replay: Option<Box<super::touch_replay::Replay>> = None;
    let mut applied = false;
    let mut previous: Option<Sample> = None;
    loop {
        #[cfg(feature = "pico2-queue-profile")]
        {
            let requested = super::touch_replay::requested();
            if requested != 0 {
                replay = Some(Box::new(super::touch_replay::Replay::new(requested, super::now_us())));
            }
            if let Some(active) = &mut replay {
                if let Some(sample) = active.poll(super::now_us()) {
                    SAMPLES.lock(|samples| samples.borrow_mut().push(sample));
                    cortex_m::asm::sev();
                }
                if active.finished() { replay = None; }
            }
        }
        let flipped = FLIPPED.load(Ordering::Acquire);
        if applied != flipped {
            devices.imu.apply_flip(flipped);
            applied = flipped;
        }
        let edge = 1 << 10;
        if pac::IO_BANK0.intr(2).read().0 & edge != 0 {
            pac::IO_BANK0.intr(2).write_value(pac::io::regs::Int(edge));
            let result = devices.touch.read();
            let now = super::now_us();
            let sample =
                match result {
                    Ok(super::touch_packet::Report::Contact(x, y)) => {
                        let sample = Sample {
                            time_us: now,
                            x,
                            y,
                            kind: if previous.is_some() { Kind::Move } else { Kind::Press },
                        };
                        previous = Some(sample);
                        Some(sample)
                    }
                    Ok(super::touch_packet::Report::Released) => previous
                        .take()
                        .map(|sample| Sample { time_us: now, kind: Kind::Release, ..sample }),
                    Ok(super::touch_packet::Report::Invalid) => None,
                    Err(_) => {
                        #[cfg(feature = "pico2-queue-profile")]
                        super::touch_profile::error();
                        None
                    }
                };
            if let Some(sample) = sample {
                SAMPLES.lock(|samples| samples.borrow_mut().push(sample));
                cortex_m::asm::sev();
            }
        }
        devices.imu.poll(&mut devices.touch.i2c, super::now_us());
        embassy_time::Timer::after_micros(250).await;
    }
}

fn position(sample: Sample, scale: f32) -> slint::LogicalPosition {
    let (x, y) =
        super::orientation::touch(sample.x as f32, sample.y as f32, super::frame_queue::flipped());
    slint::PhysicalPosition::new(x as i32, y as i32).to_logical(scale)
}

pub fn cancel(window: &slint::Window, previous: &mut Option<slint::LogicalPosition>) {
    if let Some(position) = previous.take() {
        window.dispatch_event(slint::platform::WindowEvent::PointerReleased {
            position,
            button: slint::platform::PointerEventButton::Left,
        });
        window.dispatch_event(slint::platform::WindowEvent::PointerExited);
    }
}

pub fn dispatch(window: &slint::Window, previous: &mut Option<slint::LogicalPosition>) -> bool {
    use i_slint_core::{
        animations::Instant,
        input::{BackendMouseEvent, TouchHistory},
        lengths::LogicalPoint,
    };
    let Some(batch) = take() else {
        return false;
    };
    let sample = batch.samples[batch.len - 1];
    if sample.kind == Kind::Cancel {
        cancel(window, previous);
        return true;
    }
    let position = position(sample, window.scale_factor());
    let point = LogicalPoint::new(position.x, position.y);
    let button = slint::platform::PointerEventButton::Left;
    let (kind, event) = if sample.kind == Kind::Release {
        if previous.take().is_none() {
            return false;
        }
        (
            2,
            BackendMouseEvent::Released {
                position: point,
                button,
                click_count: 0,
                touch_finger_id: 0,
            },
        )
    } else if previous.replace(position).is_none() {
        (
            0,
            BackendMouseEvent::Pressed {
                position: point,
                button,
                click_count: 0,
                touch_finger_id: 0,
            },
        )
    } else {
        (1, BackendMouseEvent::Moved { position: point, touch_finger_id: 0 })
    };
    let history = TouchHistory {
        history: batch.samples[..batch.len - 1]
            .iter()
            .map(|sample| {
                let p = self::position(*sample, window.scale_factor());
                (LogicalPoint::new(p.x, p.y), Instant::from_nanos(sample.time_us * 1000))
            })
            .collect(),
    };
    #[cfg(feature = "pico2-queue-profile")]
    {
        if kind == 0 {
            super::profile::TOUCH_PRESSES.fetch_add(1, Ordering::Release);
            super::profile::RUN.select(2);
        }
        super::touch_profile::event(kind, sample.time_us as u32, history.history.len());
        super::profile::sampled_input(sample.time_us as u32);
        super::profile_phase(super::profile_run(), super::panel_tick());
    }
    window.dispatch_event(slint::platform::WindowEvent::internal(
        i_slint_core::platform::InternalEvent::SampledMouse {
            event,
            event_time: Instant::from_nanos(sample.time_us * 1000),
            history,
        },
    ));
    if kind == 2 {
        window.dispatch_event(slint::platform::WindowEvent::PointerExited);
    }
    true
}
