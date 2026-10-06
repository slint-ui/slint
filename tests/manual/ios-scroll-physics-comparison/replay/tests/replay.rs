// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Replays the touches captured on the iPhone through Slint's Flickable on the testing backend.
//! For every `<name>.touches.csv` in a case folder, writes the content offset at each sample time
//! of `<name>.positions.csv` to `<name>.positions.slint.csv`.
//! Like the winit backend, Slint timestamps the touches when it receives them.
//! Set `SLINT_REPLAY_CAPTURE_TIMES=1` to pass UIKit's capture times and coalesced samples instead.

use std::collections::HashMap;
use std::path::Path;

use i_slint_core::animations::{Instant, update_animations};
use i_slint_core::input::{TouchHistory, TouchPhase};
use i_slint_core::lengths::LogicalPoint;
use i_slint_core::platform::InternalEvent;
use slint::ComponentHandle;
use slint::platform::WindowEvent;

slint::slint! {
    // The geometry of the Slint list in the capture app on an iPhone 13 Pro Max.
    export component Replay inherits Window {
        width: 428px;
        height: 926px;
        in-out property <length> content-y <=> list.content-y;
        list := Flickable {
            x: 10px;
            y: 104px;
            width: 408px;
            height: 770px;
            content-width: 400px;
            content-height: 72000px;
        }
    }
}

/// Mock time between two replayed captures, so the animation clock only moves forward.
const CAPTURE_SPACING_S: f64 = 100.;

enum Step {
    Touch {
        id: i32,
        position: LogicalPoint,
        phase: TouchPhase,
        /// Capture time, in seconds on the replay's timeline.
        captured: Option<f64>,
        /// Coalesced samples before the capture time, with their capture times.
        history: Vec<(f64, LogicalPoint)>,
    },
    Sample,
}

fn read_csv(path: &Path) -> Vec<HashMap<String, String>> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or_default().split(',').collect();
    lines
        .filter(|line| !line.is_empty())
        .map(|line| {
            header.iter().map(|h| h.to_string()).zip(line.split(',').map(String::from)).collect()
        })
        .collect()
}

fn number(row: &HashMap<String, String>, column: &str) -> f64 {
    row[column].parse().unwrap_or_else(|e| panic!("{column} = {:?}: {e}", row[column]))
}

fn point(row: &HashMap<String, String>) -> LogicalPoint {
    LogicalPoint::new(number(row, "x") as f32, number(row, "y") as f32)
}

/// The delivered UIKit touch callbacks, as the winit backend turns them into Slint touches.
fn touches(rows: &[HashMap<String, String>], capture_times: bool) -> Vec<(f64, Step)> {
    let is_touch = |row: &&HashMap<String, String>| {
        matches!(row["event_type"].as_str(), "touch_callback" | "coalesced_sample")
    };
    // Maps capture times to the replay's timeline by the touch delivered fastest so far.
    let mut shift = f64::INFINITY;

    let mut steps: Vec<(f64, Step)> = Vec::new();
    for row in rows.iter().filter(is_touch) {
        if row["event_type"] == "touch_callback" {
            shift = shift.min(
                number(row, "callback_seconds_from_release")
                    - number(row, "touch_seconds_from_release"),
            );
        }
        let captured = number(row, "touch_seconds_from_release") + shift;
        if row["event_type"] == "coalesced_sample" {
            // UIKit lists a move's coalesced samples after it, the move itself last.
            if let Some((_, Step::Touch { captured: Some(own), history, .. })) = steps.last_mut()
                && captured < *own
            {
                history.push((captured, point(row)));
            }
            continue;
        }
        // `UITouch.Phase`
        let phase = match row["touch_phase"].as_str() {
            "0" => TouchPhase::Started,
            "1" => TouchPhase::Moved,
            "3" => TouchPhase::Ended,
            "4" => TouchPhase::Cancelled,
            _ => continue,
        };
        steps.push((
            number(row, "callback_seconds_from_release"),
            Step::Touch {
                id: number(row, "touch_index") as i32,
                position: point(row),
                phase,
                captured: Some(captured),
                history: Vec::new(),
            },
        ));
    }
    if !capture_times {
        for (_, step) in &mut steps {
            if let Step::Touch { captured, history, .. } = step {
                *captured = None;
                history.clear();
            }
        }
    }
    steps
}

/// Replays one capture and returns its sample times with the replayed offsets.
fn replay_capture(folder: &Path, name: &str, origin_s: f64) -> Vec<(f64, f64)> {
    let positions = read_csv(&folder.join(format!("{name}.positions.csv")));
    let capture_times = std::env::var_os("SLINT_REPLAY_CAPTURE_TIMES").is_some();
    let mut steps = touches(&read_csv(&folder.join(format!("{name}.touches.csv"))), capture_times);
    steps.extend(positions.iter().map(|row| (number(row, "seconds_from_release"), Step::Sample)));
    steps.sort_by(|a, b| a.0.total_cmp(&b.0));
    let first = steps.first().map_or(0., |step| step.0);

    let instant =
        |seconds: f64| Instant::from_nanos(((origin_s + seconds - first) * 1e9).round() as u64);
    let set_time = |seconds: f64| {
        update_animations(instant(seconds));
        i_slint_backend_testing::mock_elapsed_time(0);
    };
    set_time(first);
    let replay = Replay::new().unwrap();
    replay.set_content_y(-number(&positions[0], "slint_offset_pt") as f32);

    let mut offsets = Vec::with_capacity(positions.len());
    for (time, step) in steps {
        set_time(time);
        match step {
            Step::Touch { id, position, phase, captured, history } => {
                let history = history.into_iter().map(|(time, point)| (point, instant(time)));
                let event = InternalEvent::Touch {
                    id,
                    position,
                    phase,
                    event_time: captured.map(instant),
                    history: TouchHistory { history: history.collect() },
                };
                replay.window().dispatch_event(WindowEvent::internal(event));
            }
            Step::Sample => offsets.push((time, -replay.get_content_y() as f64)),
        }
    }
    offsets
}

/// Replays every capture in `cases/<case>` and writes its `.positions.slint.csv`.
fn replay_case(case: &str) {
    assert!(
        cfg!(slint_ios_scroll_physics),
        "Run the replay from its folder, so `.cargo/config.toml` enables iOS scroll physics."
    );
    i_slint_backend_testing::init_no_event_loop();
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cases").join(case);
    let mut names: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .filter_map(|entry| {
            let file = entry.unwrap().file_name().into_string().unwrap();
            file.strip_suffix(".touches.csv").map(String::from)
        })
        .collect();
    names.sort();
    assert!(!names.is_empty(), "No captures in {}", folder.display());

    for (index, name) in names.iter().enumerate() {
        let offsets = replay_capture(&folder, name, CAPTURE_SPACING_S * (index + 1) as f64);
        assert!(offsets.iter().all(|(_, offset)| offset.is_finite()), "{case}/{name}");
        let mut csv = String::from("seconds_from_release,slint_offset_pt\n");
        for (time, offset) in offsets {
            csv += &format!("{time:.6},{offset:.3}\n");
        }
        std::fs::write(folder.join(format!("{name}.positions.slint.csv")), csv).unwrap();
    }
}

#[test]
fn case01_fling_inside() {
    replay_case("01-fling-inside");
}

#[test]
fn case02_fling_into_edge() {
    replay_case("02-fling-into-edge");
}

#[test]
fn case03_release_outside_moving_outward() {
    replay_case("03-release-outside-moving-outward");
}

#[test]
fn case04_release_outside_held() {
    replay_case("04-release-outside-held");
}

#[test]
fn case05_release_outside_moving_inward() {
    replay_case("05-release-outside-moving-inward");
}

#[test]
fn case06_fling_near_minimum_speed() {
    replay_case("06-fling-near-minimum-speed");
}

#[test]
fn case07_reversal_in_overscroll() {
    replay_case("07-reversal-in-overscroll");
}

#[test]
fn case08_touch_during_deceleration() {
    replay_case("08-touch-during-deceleration");
}

#[test]
fn case09_touch_during_spring_back() {
    replay_case("09-touch-during-spring-back");
}
