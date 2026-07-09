// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, LogicalPosition, Timer, TimerMode};

slint::include_modules!();

#[derive(Clone, Copy)]
struct GestureSample {
    at_ms: u64,
    y: f32,
}

const GESTURE_NAME: &str = "medium-flick";
const GESTURE_X: f32 = 160.0;
const GESTURE: &[GestureSample] = &[
    GestureSample { at_ms: 0, y: 360.0 },
    GestureSample { at_ms: 16, y: 320.0 },
    GestureSample { at_ms: 32, y: 280.0 },
    GestureSample { at_ms: 48, y: 240.0 },
];

pub fn app_main() -> Result<(), slint::PlatformError> {
    init_android_logging();

    let app = MainWindow::new()?;
    app.set_gesture_name(GESTURE_NAME.into());
    app.set_phase("waiting".into());
    app.set_viewport_y(0.0);

    start_sampler(&app);
    schedule_gesture(&app);

    app.run()
}

fn start_sampler(app: &MainWindow) {
    let weak = app.as_weak();
    let frame = Rc::new(Cell::new(0_u32));
    let previous_y = Rc::new(Cell::new(0.0_f32));
    let start_time = Instant::now();
    let emit_trace = cfg!(target_os = "android") || std::env::var("SLINT_INERTIA_TRACE").is_ok();

    if emit_trace {
        emit_trace_line("source,gesture,frame,time_ms,y_px,velocity_px_s,phase");
    }

    let sampler = Rc::new(Timer::default());
    let sampler_for_callback = sampler.clone();
    sampler.start(TimerMode::Repeated, Duration::from_millis(16), move || {
        let Some(app) = weak.upgrade() else {
            sampler_for_callback.stop();
            return;
        };

        let frame_number = frame.get();
        let y = -app.get_viewport_y();
        let velocity = if frame_number == 0 {
            0.0
        } else {
            (y - previous_y.get()) * 1000.0 / 16.0
        };
        previous_y.set(y);

        let elapsed_ms = start_time.elapsed().as_millis() as i32;
        let current_phase = app.get_phase();
        let phase = if current_phase.as_str() == "released" {
            if velocity.abs() < 0.01 { "stopped" } else { "inertia" }
        } else {
            current_phase.as_str()
        };

        app.set_frame(frame_number as i32);
        app.set_time_ms(elapsed_ms);
        app.set_velocity_px_s(velocity);
        app.set_phase(phase.into());

        if emit_trace {
            emit_trace_line(&format!(
                "slint,{},{},{},{:.3},{:.3},{}",
                GESTURE_NAME, frame_number, elapsed_ms, y, velocity, phase
            ));
        }

        frame.set(frame_number + 1);
    });
}

fn schedule_gesture(app: &MainWindow) {
    let weak = app.as_weak();
    Timer::single_shot(Duration::from_millis(250), move || {
        let Some(app) = weak.upgrade() else {
            return;
        };
        app.set_phase("dragging".into());
        app.window().dispatch_event(slint::platform::WindowEvent::PointerMoved {
            position: LogicalPosition::new(GESTURE_X, GESTURE[0].y),
        });
        app.window().dispatch_event(slint::platform::WindowEvent::PointerPressed {
            position: LogicalPosition::new(GESTURE_X, GESTURE[0].y),
            button: slint::platform::PointerEventButton::Left,
        });
    });

    for sample in &GESTURE[1..] {
        let weak = app.as_weak();
        Timer::single_shot(Duration::from_millis(250 + sample.at_ms), move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            app.window().dispatch_event(slint::platform::WindowEvent::PointerMoved {
                position: LogicalPosition::new(GESTURE_X, sample.y),
            });
        });
    }

    let weak = app.as_weak();
    let release_y = GESTURE.last().unwrap().y;
    let release_at_ms = 250 + GESTURE.last().unwrap().at_ms;
    Timer::single_shot(Duration::from_millis(release_at_ms), move || {
        let Some(app) = weak.upgrade() else {
            return;
        };
        app.window().dispatch_event(slint::platform::WindowEvent::PointerReleased {
            position: LogicalPosition::new(GESTURE_X, release_y),
            button: slint::platform::PointerEventButton::Left,
        });
        app.set_phase("released".into());
    });
}

fn emit_trace_line(line: &str) {
    #[cfg(target_os = "android")]
    log::info!("{line}");
    #[cfg(not(target_os = "android"))]
    println!("{line}");
}

#[cfg(target_os = "android")]
fn init_android_logging() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("inertia-scroll-probe")
            .with_max_level(log::LevelFilter::Info),
    );
}

#[cfg(not(target_os = "android"))]
fn init_android_logging() {}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) -> Result<(), slint::PlatformError> {
    slint::android::init(app).unwrap();
    app_main()
}
