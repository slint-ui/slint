// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Replays the scroll-to calls of the iPhone captures through Slint's Flickable on the testing backend.
//! For every capture in `../cases/*/captures.csv` without touches, writes Slint's content offset
//! at each sample of `<name>.positions.csv` to `<name>.positions.slint.csv`.
//!
//! Commands and samples use Slint's animation clock on the phone, `slint_tick_seconds_from_command`,
//! so the replay reproduces the Slint curve of the phone when the model is unchanged.
//! Set `SLINT_REPLAY_CASE=<substring>` to replay only matching case folders.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use i_slint_core::animations::{Instant, update_animations};

slint::slint! {
    export component Replay inherits Window {
        width: 428px;
        height: 926px;
        in property <length> viewport-height;
        in property <length> content-height;
        in-out property <length> content-y <=> list.content-y;
        callback scroll-to-offset(length);
        scroll-to-offset(offset) => { list.scroll-to({ x: 0px, y: offset }, ScrollMode.smooth); }
        list := Flickable {
            width: 400px;
            height: root.viewport-height;
            content-width: 400px;
            content-height: root.content-height;
        }
    }
}

/// Keeps the replay's clock away from zero, which the animation driver treats specially.
const CLOCK_ORIGIN_S: f64 = 100.;
/// Clock distance between two captures, so every capture starts from rest.
const CAPTURE_SPACING_S: f64 = 1000.;

type Row = HashMap<String, String>;

fn read_csv(path: &Path) -> Vec<Row> {
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

fn number(row: &Row, column: &str) -> f64 {
    row[column].parse().unwrap_or_else(|e| panic!("{column} = {:?}: {e}", row[column]))
}

enum Step {
    Sample(usize),
    Command(f64),
}

/// Replays one capture from `clock_origin`; returns the CSV of its replayed samples.
fn replay_capture(folder: &Path, capture: &Row, clock_origin: f64) -> String {
    let name = &capture["file"];
    let positions = read_csv(&folder.join(format!("{name}.positions.csv")));
    let events = read_csv(&folder.join(format!("{name}.events.csv")));

    let mut steps: Vec<(f64, Step)> = positions
        .iter()
        .enumerate()
        .map(|(index, row)| (number(row, "slint_tick_seconds_from_command"), Step::Sample(index)))
        .collect();
    steps.extend(events.iter().filter(|e| e["event"] == "command").map(|e| {
        (number(e, "slint_tick_seconds_from_command"), Step::Command(number(e, "target_offset_pt")))
    }));
    // The phone samples a callback's state before the scroll-to of that callback.
    steps.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| matches!(a.1, Step::Command(_)).cmp(&matches!(b.1, Step::Command(_))))
    });

    let instant = |seconds: f64| Instant::from_nanos(((clock_origin + seconds) * 1e9) as u64);
    let set_time = |seconds: f64| {
        update_animations(instant(seconds));
        i_slint_backend_testing::mock_elapsed_time(0);
    };
    set_time(steps[0].0);
    let replay = Replay::new().unwrap();
    replay.set_viewport_height(number(capture, "viewport_pt") as f32);
    replay.set_content_height(number(capture, "content_pt") as f32);
    let start = number(&positions[0], "slint_offset_pt");
    replay.set_content_y(-start as f32);

    let mut offsets = vec![f64::NAN; positions.len()];
    let mut command_start = None;
    let mut target = f64::NAN;
    let commands = events.iter().filter(|e| e["event"] == "command").count();
    for (time, step) in steps {
        set_time(time);
        match step {
            Step::Sample(index) => offsets[index] = -replay.get_content_y() as f64,
            Step::Command(offset) => {
                command_start.get_or_insert(-replay.get_content_y() as f64);
                target = offset;
                replay.invoke_scroll_to_offset(offset as f32);
            }
        }
    }

    let mut csv = String::from(
        "seconds_from_command,slint_tick_seconds_from_command,slint_offset_pt,slint_progress\n",
    );
    for (row, offset) in positions.iter().zip(offsets) {
        assert!(offset.is_finite(), "{name} at {}", row["seconds_from_command"]);
        let progress = match command_start {
            Some(start) if commands == 1 && target != start => {
                format!("{:.6}", (offset - start) / (target - start))
            }
            _ => String::new(),
        };
        csv += &format!(
            "{},{},{offset:.3},{progress}\n",
            row["seconds_from_command"], row["slint_tick_seconds_from_command"]
        );
    }
    csv
}

fn case_folders() -> Vec<PathBuf> {
    let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cases");
    let filter = std::env::var("SLINT_REPLAY_CASE").unwrap_or_default();
    let mut folders: Vec<PathBuf> = std::fs::read_dir(&cases)
        .unwrap_or_else(|e| panic!("{}: {e}", cases.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("captures.csv").exists())
        .filter(|path| path.file_name().unwrap().to_string_lossy().contains(&filter))
        .collect();
    folders.sort();
    folders
}

#[test]
fn scroll_to_captures() {
    i_slint_backend_testing::init_no_event_loop();
    let mut replayed = 0;
    for folder in case_folders() {
        for capture in read_csv(&folder.join("captures.csv")) {
            let name = &capture["file"];
            // Touches would need the iOS flick physics, which the host doesn't build.
            if folder.join(format!("{name}.touches.csv")).exists() {
                continue;
            }
            if !folder.join(format!("{name}.positions.csv")).exists() {
                continue;
            }
            let origin = CLOCK_ORIGIN_S + replayed as f64 * CAPTURE_SPACING_S;
            let csv = replay_capture(&folder, &capture, origin);
            std::fs::write(folder.join(format!("{name}.positions.slint.csv")), csv).unwrap();
            replayed += 1;
        }
    }
    println!("Replayed {replayed} captures");
}
