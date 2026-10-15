// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

extern crate alloc;
#[cfg(test)]
#[path = "../../../background_job.rs"]
mod background_job;
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

#[test]
fn two_core_vials_match_serial_buffers_through_hold_and_release() {
    use ink_particles::{FrameState, Paint, Vial};
    use std::sync::atomic::{AtomicBool, Ordering};
    let queue = background_job::Queue::new();
    let stop = AtomicBool::new(false);
    let mut vials = core::array::from_fn::<_, 4, _>(|index| Vial::new(48, 140, index));
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
            for (index, vial) in vials.iter_mut().enumerate() {
                vial.set_level(if (400..440).contains(&tick) {
                    1024
                } else {
                    [409, 665, 512, 819][index]
                });
                vial.set_held((20..300).contains(&tick) || (600..700).contains(&tick), tick);
                vial.restore_cached(&mut expected[index][back], &mut expected_states[index][back]);
                vial.stamp_cached(
                    tick,
                    &mut expected[index][back],
                    &mut expected_states[index][back],
                );
            }
            let [yellow, red, cyan, black] = &mut actual;
            let [ys, rs, cs, bs] = &mut actual_states;
            let mut paints = [
                Paint::new(&vials[0], tick, &mut yellow[back], &mut ys[back]),
                Paint::new(&vials[2], tick, &mut cyan[back], &mut cs[back]),
            ];
            let mut next = 0;
            queue.scope(
                &mut || {
                    next += usize::from(paints[next].step());
                    next == 2
                },
                || {
                    vials[1].restore_cached(&mut red[back], &mut rs[back]);
                    vials[1].stamp_cached(tick, &mut red[back], &mut rs[back]);
                    vials[3].restore_cached(&mut black[back], &mut bs[back]);
                    vials[3].stamp_cached(tick, &mut black[back], &mut bs[back]);
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
}
impl Test {
    fn step(&mut self, micros: u64) {
        self.clock.set(self.clock.get() + micros);
        slint::platform::update_timers_and_animations();
        self.window.draw_if_needed(|r| {
            r.render(&mut self.pixels, 320);
        });
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
    let mut t = Test { window, clock, pixels: vec![Rgb565BigEndianPixel::default(); 320 * 240] };
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
    let percentage_pixels = (194..218)
        .flat_map(|y| (64..315).map(move |x| y * 320 + x))
        .filter(|&i| {
            let p = t.pixels[i];
            p.red().max(p.green()).max(p.blue()) > 160
        })
        .count();
    assert!(percentage_pixels > 500, "Ink percentages must remain visible during partial updates");
    assert!(ui.global::<InkParticles>().get_frames().iter().all(|image| image.size().width > 0));
    t.pointer(24., 210., true);
    assert!(!ui.global::<Kiosk>().get_enabled());
    t.pointer(24., 210., false);
    assert_eq!(ui.global::<PrinterState>().get_active_page(), Pages::Ink);
    t.advance(12_000_000);
    assert_eq!(ui.global::<PrinterState>().get_active_page(), Pages::Ink);
    ui.global::<PrinterState>().set_active_page(Pages::Home);
    t.step(20_000);
    assert!(ui.global::<InkParticles>().get_frames().iter().all(|image| image.size().width == 0));
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
