// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The point of compiling a reload on a worker thread is that the event loop
//! keeps running while it happens. Check both halves: the new source is applied,
//! and the loop was never blocked for as long as the compilation took.

#![cfg(feature = "live-component")]

#[path = "live_reload/harness.rs"]
mod harness;

use i_slint_live_preview::live_component::Value;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Short next to a compilation, so a stalled loop shows up as a long gap
/// between ticks rather than as a missed tick.
const TICK_INTERVAL: Duration = Duration::from_millis(5);

/// Imports std-widgets so that compiling takes long enough to be obvious when
/// it happens on the event-loop thread.
fn source(value: i32) -> String {
    format!(
        r#"
        import {{ Button, ComboBox, LineEdit, CheckBox, Slider, SpinBox, GroupBox, TabWidget }}
            from "std-widgets.slint";
        export component App inherits Window {{
            out property <int> value: {value};
            TabWidget {{
                Tab {{
                    title: "one";
                    VerticalLayout {{
                        Button {{ text: "a"; }}
                        ComboBox {{ model: ["x"]; }}
                        LineEdit {{ }}
                        CheckBox {{ }}
                        Slider {{ }}
                        SpinBox {{ }}
                        GroupBox {{ Button {{ text: "b"; }} }}
                    }}
                }}
                Tab {{ title: "two"; VerticalLayout {{ Button {{ text: "c"; }} }} }}
            }}
        }}"#
    )
}

/// Longest stretch the event loop went without running a timer callback.
#[derive(Default)]
struct LoopActivity {
    last_tick: Option<Instant>,
    longest_stall: Duration,
}

impl LoopActivity {
    fn tick(&mut self, now: Instant) {
        if let Some(last) = self.last_tick {
            self.longest_stall = self.longest_stall.max(now - last);
        }
        self.last_tick = Some(now);
    }
}

#[test]
fn compiling_a_reload_does_not_block_the_event_loop() {
    let dir = harness::TestDir::new("threaded");
    let file = dir.write("app.slint", &source(1));

    let component = harness::start(&file);
    assert_eq!(component.borrow().get_property("value"), Value::Number(1.));

    let activity = Rc::new(RefCell::new(LoopActivity::default()));
    let ticker = i_slint_core::timers::Timer::default();
    ticker.start(i_slint_core::timers::TimerMode::Repeated, TICK_INTERVAL, {
        let activity = activity.clone();
        move || activity.borrow_mut().tick(Instant::now())
    });

    let reloaded_after = Rc::new(RefCell::new(None));
    component.borrow_mut().set_post_reload_hook({
        let reloaded_after = reloaded_after.clone();
        let started = Instant::now();
        move |_| {
            *reloaded_after.borrow_mut() = Some(started.elapsed());
            i_slint_core::api::quit_event_loop().unwrap();
        }
    });
    harness::watchdog();

    std::fs::write(&file, source(2)).unwrap();
    slint_interpreter::run_event_loop().unwrap();

    assert_eq!(
        component.borrow().get_property("value"),
        Value::Number(2.),
        "the worker's result should have been applied to the instance"
    );

    let reloaded_after = reloaded_after.borrow().expect("the reload hook should have run");
    let longest_stall = activity.borrow().longest_stall;
    // Compiling on this thread stalls the loop for most of the wait, so half is a wide margin
    assert!(
        longest_stall < reloaded_after / 2,
        "event loop stalled for {longest_stall:?} of the {reloaded_after:?} the reload took, \
         so the compilation was not running on the worker thread"
    );
}
