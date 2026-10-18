// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use super::touch_samples::{Kind, Sample};
use core::sync::atomic::{AtomicU32, Ordering};

static REQUEST: AtomicU32 = AtomicU32::new(0);
const TRACE: &str = include_str!("results/stock-150/touch-panning/panel-history/reports.csv");

pub fn request(page: u32) {
    REQUEST.store(page, Ordering::Release);
}

pub fn requested() -> u32 {
    REQUEST.swap(0, Ordering::AcqRel)
}

pub struct Replay {
    lines: core::str::Lines<'static>,
    last: u32,
    origin: u64,
    started: u64,
    next: Option<(u64, [u8; 27])>,
    tracker: super::touch_packet::Tracker,
    previous: Option<Sample>,
}

impl Replay {
    pub fn new(page: u32, now: u64) -> Self {
        let (first, last) = if page == 1 { (11, 668) } else { (684, 1484) };
        let mut lines = TRACE.lines();
        let mut next = None;
        for line in lines.by_ref().skip(1) {
            if line.split(',').next().and_then(|n| n.parse::<u32>().ok()) == Some(first) {
                next = Some(Self::record(line));
                break;
            }
        }
        Self {
            lines,
            last,
            origin: next.unwrap().0,
            started: now,
            next,
            tracker: Default::default(),
            previous: None,
        }
    }

    fn record(line: &str) -> (u64, [u8; 27]) {
        let mut fields = line.split(',');
        fields.next();
        let timestamp = fields.next().unwrap().parse().unwrap();
        let hex = fields.next().unwrap().as_bytes();
        let digit = |byte: u8| if byte <= b'9' { byte - b'0' } else { byte - b'a' + 10 };
        (timestamp, core::array::from_fn(|i| digit(hex[i * 2]) << 4 | digit(hex[i * 2 + 1])))
    }

    pub fn finished(&self) -> bool {
        self.next.is_none()
    }

    pub fn poll(&mut self, now: u64) -> Option<Sample> {
        let (time, data) = self.next?;
        if now < self.started + time - self.origin {
            return None;
        }
        self.next = self.lines.next().and_then(|line| {
            let sequence = line.split(',').next()?.parse::<u32>().ok()?;
            (sequence <= self.last).then(|| Self::record(line))
        });
        let report = self.tracker.read(&data);
        super::touch_profile::report(&data, report);
        match report {
            super::touch_packet::Report::Contact(x, y) => {
                let sample = Sample {
                    time_us: now,
                    x,
                    y,
                    kind: if self.previous.is_some() { Kind::Move } else { Kind::Press },
                };
                self.previous = Some(sample);
                Some(sample)
            }
            super::touch_packet::Report::Released => self.previous.take().map(|sample| Sample {
                time_us: now,
                kind: Kind::Release,
                ..sample
            }),
            super::touch_packet::Report::Invalid => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_drags_keep_all_contacts_and_the_original_duration() {
        request(1);
        assert_eq!(requested(), 1);
        assert_eq!(requested(), 0);
        for (page, coordinates, duration) in [(1, 657, 4_943_477), (2, 800, 6_014_382)] {
            let started = 4_000_000_000;
            let mut replay = Replay::new(page, started);
            let mut now = started;
            let mut samples = Vec::new();
            while !replay.finished() {
                if let Some(sample) = replay.poll(now) {
                    samples.push(sample);
                }
                now += 250;
                assert!(now - started < 7_000_000);
            }
            assert_eq!(samples.len(), coordinates + 1);
            assert_eq!(samples[0].kind, Kind::Press);
            assert_eq!(samples.last().unwrap().kind, Kind::Release);
            assert!(samples[1..coordinates].iter().all(|s| s.kind == Kind::Move));
            let elapsed = samples.last().unwrap().time_us - started;
            assert!((duration..duration + 250).contains(&elapsed), "{page}: {elapsed}");
        }
    }
}
