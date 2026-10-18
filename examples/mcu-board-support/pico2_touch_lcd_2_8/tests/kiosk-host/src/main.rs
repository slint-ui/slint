// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

extern crate alloc;
#[cfg(test)]
#[path = "../../../touch_samples.rs"]
mod touch_samples;
#[cfg(test)]
#[path = "../../../touch_packet.rs"]
mod touch_packet;
#[cfg(test)]
#[path = "../../../background_job.rs"]
mod background_job;
#[cfg(test)]
#[path = "../../../orientation.rs"]
mod orientation;
#[cfg(test)]
#[path = "../../../tilt_fusion.rs"]
mod tilt_fusion;
#[cfg(test)]
#[path = "../../../uniform_tiles.rs"]
mod uniform_tiles;
use slint::{
    ComponentHandle, Model,
    platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType, Rgb565BigEndianPixel},
};
use std::{cell::Cell, io::Write, rc::Rc, time::Duration};
slint::include_modules!();
#[path = "../../../../../../demos/printerdemo_mcu/ink_animation.rs"]
mod ink_animation;
#[path = "../../../../../../demos/printerdemo_mcu/ink_particles.rs"]
mod ink_particles;
#[path = "../../../../../../demos/printerdemo_mcu/kiosk.rs"]
mod kiosk;
#[path = "../../../../../../demos/printerdemo_mcu/kiosk_motion.rs"]
mod kiosk_motion;
#[path = "../../../native_buffer.rs"]
mod native_buffer;

#[test]
fn two_core_vials_match_serial_buffers_through_hold_and_release() {
    use ink_particles::{FrameState, Paint, Vial};
    use std::sync::atomic::{AtomicBool, Ordering};
    let queue = background_job::Queue::new();
    let stop = AtomicBool::new(false);
    let mut vials = core::array::from_fn::<_, 4, _>(|index| Vial::new(48, 140, index));
    let mut parallel_vials = core::array::from_fn::<_, 4, _>(|index| Vial::new(48, 140, index));
    for v in &mut parallel_vials {
        v.advance_tilt([0, -1024], 0);
    }
    let mut expected =
        core::array::from_fn::<_, 4, _>(|_| [vec![0u16; 48 * 140], vec![0; 48 * 140]]);
    let mut actual = expected.clone();
    let mut expected_states = core::array::from_fn::<_, 4, _>(|_| {
        core::array::from_fn::<_, 2, _>(|_| FrameState::default())
    });
    let mut actual_states = core::array::from_fn::<_, 4, _>(|_| {
        core::array::from_fn::<_, 2, _>(|_| FrameState::default())
    });
    std::thread::scope(|threads| {
        threads.spawn(|| {
            while !stop.load(Ordering::Acquire) {
                queue.step();
                std::thread::yield_now();
            }
        });
        for tick in 0..900 {
            let back = tick as usize & 1;
            let up = [[0, -1024], [265, -989], [-512, -886], [1024, 0]][tick as usize / 90 % 4];
            for (index, vial) in vials.iter_mut().enumerate() {
                vial.set_level(if (400..440).contains(&tick) {
                    1024
                } else {
                    [409, 665, 512, 819][index]
                });
                vial.set_held((20..300).contains(&tick) || (600..700).contains(&tick), tick);
                let parallel = &mut parallel_vials[index];
                parallel.set_level(if (400..440).contains(&tick) {
                    1024
                } else {
                    [409, 665, 512, 819][index]
                });
                parallel.set_held((20..300).contains(&tick) || (600..700).contains(&tick), tick);
                vial.advance_tilt(up, tick);
                vial.restore_cached(&mut expected[index][back], &mut expected_states[index][back]);
                vial.stamp_cached(
                    tick,
                    &mut expected[index][back],
                    &mut expected_states[index][back],
                );
            }
            let [yv, rv, cv, bv] = &mut parallel_vials;
            let mut next = 0;
            queue.scope(
                &mut || {
                    let v = if next == 0 { &mut *yv } else { &mut *cv };
                    v.advance_tilt(up, tick);
                    next += 1;
                    next == 2
                },
                || {
                    rv.advance_tilt(up, tick);
                    bv.advance_tilt(up, tick);
                },
                std::thread::yield_now,
                || {},
            );
            let [yellow, red, cyan, black] = &mut actual;
            let [ys, rs, cs, bs] = &mut actual_states;
            let mut paints = [
                Paint::new(yv, tick, &mut yellow[back], &mut ys[back]),
                Paint::new(cv, tick, &mut cyan[back], &mut cs[back]),
            ];
            let mut next = 0;
            queue.scope(
                &mut || {
                    next += usize::from(paints[next].step());
                    next == 2
                },
                || {
                    rv.restore_cached(&mut red[back], &mut rs[back]);
                    rv.stamp_cached(tick, &mut red[back], &mut rs[back]);
                    bv.restore_cached(&mut black[back], &mut bs[back]);
                    bv.stamp_cached(tick, &mut black[back], &mut bs[back]);
                },
                std::thread::yield_now,
                || {},
            );
            assert_eq!(actual, expected, "tick {tick}");
        }
        stop.store(true, Ordering::Release);
    });
}

struct Backend {
    window: Rc<MinimalSoftwareWindow>,
    clock: Rc<Cell<u64>>,
}
impl slint::platform::Platform for Backend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> Duration {
        Duration::from_micros(self.clock.get())
    }
}
struct Test {
    window: Rc<MinimalSoftwareWindow>,
    clock: Rc<Cell<u64>>,
    pixels: Vec<Rgb565BigEndianPixel>,
    trace: Option<std::fs::File>,
}
impl Test {
    fn step(&mut self, micros: u64) {
        self.clock.set(self.clock.get() + micros);
        slint::platform::update_timers_and_animations();
        self.window.draw_if_needed(|r| {
            if std::env::var_os("PICO_NATIVE_BUFFER").is_some() {
                r.render_into_buffer(&mut native_buffer::FrameBuffer::new(&mut self.pixels, 320));
            } else {
                r.render(&mut self.pixels, 320);
            }
        });
        if let Some(trace) = &mut self.trace {
            let hash = bytemuck::cast_slice::<_, u8>(&self.pixels).iter().fold(
                0xcbf29ce484222325u64,
                |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3),
            );
            writeln!(trace, "{},{}", self.clock.get(), hash).unwrap();
        }
    }
    fn advance(&mut self, micros: u64) {
        let end = self.clock.get() + micros;
        while self.clock.get() < end {
            self.step((end - self.clock.get()).min(16_667));
        }
    }
    fn pointer(&mut self, x: f32, y: f32, down: bool) {
        use slint::platform::{PointerEventButton, WindowEvent};
        let position = slint::LogicalPosition::new(x, y);
        let button = PointerEventButton::Left;
        self.window.dispatch_event(if down {
            WindowEvent::PointerPressed { position, button }
        } else {
            WindowEvent::PointerReleased { position, button }
        });
        self.step(20_000);
    }
    fn save(&self, name: &str) {
        let Ok(directory) = std::env::var("KIOSK_CAPTURE_DIR") else { return };
        std::fs::create_dir_all(&directory).unwrap();
        let mut f =
            std::fs::File::create(std::path::Path::new(&directory).join(format!("{name}.ppm")))
                .unwrap();
        write!(f, "P6\n320 240\n255\n").unwrap();
        for pixel in &self.pixels {
            f.write_all(&[pixel.red(), pixel.green(), pixel.blue()]).unwrap();
        }
    }
}
fn main() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    let clock = Rc::new(Cell::new(0));
    slint::platform::set_platform(Box::new(Backend {
        window: window.clone(),
        clock: clock.clone(),
    }))
    .unwrap();
    let ui = MainWindow::new().unwrap();
    ui.global::<PrinterState>().set_ink_levels(
        [
            InkLevel { color: slint::Color::from_rgb_u8(0, 255, 255), level: 0.40 },
            InkLevel { color: slint::Color::from_rgb_u8(255, 0, 255), level: 0.65 },
            InkLevel { color: slint::Color::from_rgb_u8(255, 255, 0), level: 0.50 },
            InkLevel { color: slint::Color::from_rgb_u8(0, 0, 0), level: 0.80 },
        ]
        .into(),
    );
    ui.window().set_size(slint::PhysicalSize::new(320, 240));
    ui.global::<QueueMotion>().set_pixel_snap(true);
    let _controller = kiosk::start(&ui);
    let _ink = ink_animation::start(&ui);
    ui.show().unwrap();
    let mut t = Test {
        window,
        clock,
        pixels: vec![Rgb565BigEndianPixel::default(); 320 * 240],
        trace: std::env::var_os("KIOSK_FRAME_TRACE").map(|p| std::fs::File::create(p).unwrap()),
    };
    t.advance(100_000);
    ui.global::<PrinterState>().set_active_page(Pages::Settings);
    t.advance(100_000);
    let max = ui.global::<SettingsMotion>().get_maximum();
    println!("Settings maximum {max}");
    assert!(max > 117.);
    ui.global::<SettingsMotion>().set_raw_y(-max);
    t.advance(100_000);
    t.save("settings-kiosk-off");
    t.pointer(258., 180., true);
    t.pointer(258., 180., false);
    assert!(ui.global::<Kiosk>().get_enabled());
    assert_eq!(ui.global::<PrinterState>().get_active_page(), Pages::Home);
    t.save("kiosk-queue");
    t.advance(10_100_000);
    assert_eq!(ui.global::<PrinterState>().get_active_page(), Pages::Settings);
    t.advance(3_200_000);
    t.save("settings-kiosk-on");
    t.advance(6_900_000);
    assert_eq!(ui.global::<PrinterState>().get_active_page(), Pages::Ink);
    t.advance(1_000_000);
    t.save("kiosk-ink");
    let header_pixels = (12..34)
        .flat_map(|y| (64..155).map(move |x| y * 320 + x))
        .filter(|&i| {
            let p = t.pixels[i];
            p.red() > 170 && p.green() > 170 && p.blue() > 170
        })
        .count();
    assert!(header_pixels > 200, "Ink title must remain visible during partial updates");
    let label_pixels = (194..228)
        .flat_map(|y| (64..315).map(move |x| y * 320 + x))
        .filter(|&i| {
            let p = t.pixels[i];
            p.red().max(p.green()).max(p.blue()) > 160
        })
        .count();
    assert!(label_pixels > 500, "Ink labels must remain visible during partial updates");
    assert!(ui.global::<InkParticles>().get_frames().iter().all(|image| image.size().width > 0));
    t.pointer(24., 210., true);
    assert!(!ui.global::<Kiosk>().get_enabled());
    t.pointer(24., 210., false);
    assert_eq!(ui.global::<PrinterState>().get_active_page(), Pages::Ink);
    t.advance(12_000_000);
    assert_eq!(ui.global::<PrinterState>().get_active_page(), Pages::Ink);
    for (y, page) in [(30., Pages::Home), (210., Pages::Settings)] {
        ui.global::<PrinterState>().set_active_page(Pages::Ink);
        t.advance(100_000);
        assert!(ui.global::<InkParticles>().get_frames().iter().all(|image| image.size().width > 0));
        t.pointer(24., y, true);
        t.window.dispatch_event(slint::platform::WindowEvent::PointerReleased {
            position: slint::LogicalPosition::new(24., y),
            button: slint::platform::PointerEventButton::Left,
        });
        assert_eq!(ui.global::<PrinterState>().get_active_page(), page);
        assert!(ui.global::<InkParticles>().get_frames().iter().all(|image| image.size().width == 0));
        t.step(20_000);
        ui.global::<PrinterState>().set_active_page(Pages::Ink);
        t.clock.set(t.clock.get() + 20_000);
        slint::platform::update_timers_and_animations();
        assert!(
            ui.global::<InkParticles>().get_frames().iter().all(|image| image.size().width == 0),
            "Ink buffers must wait for the new page layout",
        );
        t.step(0);
        t.advance(100_000);
        assert!(ui.global::<InkParticles>().get_frames().iter().all(|image| image.size().width > 0));
    }
    ui.global::<PrinterState>().set_active_page(Pages::Home);
    t.step(20_000);
    for (x, y) in [(0., 0.), (160., 120.), (319., 239.)] {
        ui.global::<Kiosk>().set_enabled(true);
        t.step(20_000);
        assert!(ui.global::<Kiosk>().get_enabled());
        t.pointer(x, y, true);
        assert!(!ui.global::<Kiosk>().get_enabled());
        t.pointer(x, y, false);
        assert_eq!(ui.global::<PrinterState>().get_active_page(), Pages::Home);
    }
    println!(
        "Kiosk checkbox, ten-second sequence, immediate exit, no click-through, and stopped timers pass"
    );
}

#[test]
fn queue_and_settings_follow_pointer_moves_without_drag_easing() {
    use slint::platform::WindowEvent;
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    let clock = Rc::new(Cell::new(0));
    slint::platform::set_platform(Box::new(Backend {
        window: window.clone(),
        clock: clock.clone(),
    }))
    .unwrap();
    let ui = MainWindow::new().unwrap();
    ui.window().set_size(slint::PhysicalSize::new(320, 240));
    ui.global::<QueueMotion>().set_pixel_snap(true);
    ui.show().unwrap();
    let mut t = Test {
        window,
        clock,
        pixels: vec![Rgb565BigEndianPixel::default(); 320 * 240],
        trace: None,
    };
    for (page, x) in [(Pages::Home, 210.), (Pages::Settings, 300.)] {
        ui.global::<PrinterState>().set_active_page(page);
        t.advance(100_000);
        let position = || {
            if page == Pages::Home {
                ui.global::<QueueMotion>().get_raw_y()
            } else {
                ui.global::<SettingsMotion>().get_raw_y()
            }
        };
        if page == Pages::Home {
            ui.global::<QueueMotion>().set_raw_y(-40.);
        } else {
            ui.global::<SettingsMotion>().set_raw_y(-40.);
        }
        t.step(16_667);
        t.pointer(x, 170., true);
        for y in [160., 150.] {
            ui.window().dispatch_event(WindowEvent::PointerMoved {
                position: slint::LogicalPosition::new(x, y),
            });
            t.step(16_667);
        }
        let mut previous_y = 150.;
        for y in [140., 130., 135., 145., 150.] {
            let previous = position();
            ui.window().dispatch_event(WindowEvent::PointerMoved {
                position: slint::LogicalPosition::new(x, y),
            });
            let expected = previous + y - previous_y;
            assert!(
                (position() - expected).abs() < 0.001,
                "{page:?}: pointer delta must apply immediately"
            );
            t.step(16_667);
            assert!(
                (position() - expected).abs() < 0.001,
                "{page:?}: holding must not ease after dispatch"
            );
            previous_y = y;
        }
        t.pointer(x, previous_y, false);
        t.advance(1_000_000);
        if page == Pages::Home {
            ui.global::<QueueMotion>().set_raw_y(0.);
        } else {
            ui.global::<SettingsMotion>().set_raw_y(0.);
        }
        t.advance(100_000);
        t.pointer(x, 225., true);
        for y in (45..=215).rev().step_by(10) {
            ui.window().dispatch_event(WindowEvent::PointerMoved {
                position: slint::LogicalPosition::new(x, y as f32),
            });
            let before = position();
            t.step(16_667);
            let after = position();
            assert!(
                (before - (y as f32 - 225.)).abs() < 0.001,
                "{page:?}: a full held drag must track the finger"
            );
            assert!((before - after).abs() < 0.001, "{page:?}: no settling while finger held");
        }
        let stopped = position();
        t.advance(500_000);
        assert!(
            (position() - stopped).abs() < 0.001,
            "{page:?}: held finger must stop the list immediately"
        );
        t.pointer(x, 45., false);
    }
}
#[test]
fn sampled_pointer_history_preserves_direct_drag_position() {
    use i_slint_core::{
        animations::Instant,
        input::{BackendMouseEvent, TouchHistory},
        lengths::LogicalPoint,
        platform::InternalEvent,
    };
    use slint::platform::{PointerEventButton, WindowEvent};
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    let clock = Rc::new(Cell::new(0));
    slint::platform::set_platform(Box::new(Backend {
        window: window.clone(),
        clock: clock.clone(),
    }))
    .unwrap();
    let ui = MainWindow::new().unwrap();
    ui.window().set_size(slint::PhysicalSize::new(320, 240));
    ui.global::<QueueMotion>().set_pixel_snap(true);
    ui.show().unwrap();
    let mut t = Test {
        window,
        clock: clock.clone(),
        pixels: vec![Rgb565BigEndianPixel::default(); 320 * 240],
        trace: None,
    };
    for (page, x) in [(Pages::Home, 210.), (Pages::Settings, 300.)] {
        ui.global::<PrinterState>().set_active_page(page);
        t.advance(100_000);
        if page == Pages::Home {
            ui.global::<QueueMotion>().set_raw_y(-40.);
        } else {
            ui.global::<SettingsMotion>().set_raw_y(-40.);
        }
        t.step(16_667);
        let point = |y| LogicalPoint::new(x, y);
        let send = |event, history| {
            ui.window()
                .dispatch_event(WindowEvent::internal(InternalEvent::SampledMouse {
                    event,
                    event_time: Instant::from_nanos(clock.get() * 1000),
                    history: TouchHistory { history },
                }))
        };
        let position = || {
            if page == Pages::Home {
                ui.global::<QueueMotion>().get_raw_y()
            } else {
                ui.global::<SettingsMotion>().get_raw_y()
            }
        };
        send(
            BackendMouseEvent::Pressed {
                position: point(180.),
                button: PointerEventButton::Left,
                click_count: 0,
                touch_finger_id: 0,
            },
            vec![],
        );
        t.step(16_667);
        send(
            BackendMouseEvent::Moved {
                position: point(160.),
                touch_finger_id: 0,
            },
            vec![(
                point(170.),
                Instant::from_nanos((clock.get() - 8_000) * 1000),
            )],
        );
        assert_eq!(
            position(),
            -60.,
            "{page:?}: history must not add another movement delta"
        );
        t.step(16_667);
        send(
            BackendMouseEvent::Moved {
                position: point(150.),
                touch_finger_id: 0,
            },
            vec![(
                point(155.),
                Instant::from_nanos((clock.get() - 8_000) * 1000),
            )],
        );
        assert_eq!(
            position(),
            -70.,
            "{page:?}: the latest coordinate applies immediately"
        );
        t.advance(400_000);
        assert_eq!(
            position(),
            -70.,
            "{page:?}: a held pointer must not ease toward its target"
        );
        send(
            BackendMouseEvent::Released {
                position: point(150.),
                button: PointerEventButton::Left,
                click_count: 0,
                touch_finger_id: 0,
            },
            vec![],
        );
        ui.window().dispatch_event(WindowEvent::PointerExited);
        t.advance(400_000);
        assert_eq!(
            position(),
            -70.,
            "{page:?}: an old movement sample must not launch a late flick"
        );
    }
}
