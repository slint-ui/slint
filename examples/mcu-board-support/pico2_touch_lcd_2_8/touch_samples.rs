// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pub const CAPACITY: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Kind {
    Press,
    #[default]
    Move,
    Release,
    Cancel,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub time_us: u64,
    pub x: u16,
    pub y: u16,
    pub kind: Kind,
}

pub struct Batch {
    pub samples: [Sample; CAPACITY],
    pub len: usize,
}

pub struct Buffer {
    samples: [Sample; CAPACITY],
    len: usize,
    last_move_tick: Option<u32>,
    pub overflows: u32,
}

impl Buffer {
    pub const fn new() -> Self {
        Self {
            samples: [Sample { time_us: 0, x: 0, y: 0, kind: Kind::Move }; CAPACITY],
            len: 0,
            last_move_tick: None,
            overflows: 0,
        }
    }

    pub fn push(&mut self, sample: Sample) {
        if self.len == CAPACITY {
            self.overflows += 1;
            if let Some(index) = self.samples[..self.len].iter().position(|s| s.kind == Kind::Move)
            {
                self.samples.copy_within(index + 1..self.len, index);
                self.len -= 1;
            } else {
                self.samples[0] = Sample { kind: Kind::Cancel, ..sample };
                self.len = 1;
            }
        }
        self.samples[self.len] = sample;
        self.len += 1;
    }

    pub fn ready(&self, tick: u32) -> bool {
        self.len != 0
            && (self.samples[0].kind != Kind::Move
                || self.last_move_tick != Some(tick)
                || self.samples[..self.len].iter().any(|s| s.kind != Kind::Move))
    }

    pub fn take(&mut self, tick: u32) -> Option<Batch> {
        if !self.ready(tick) {
            return None;
        }
        let kind = self.samples[0].kind;
        let len = if kind == Kind::Move {
            self.last_move_tick = Some(tick);
            self.samples[..self.len].iter().take_while(|s| s.kind == Kind::Move).count()
        } else {
            1
        };
        let mut batch = Batch { samples: [Sample::default(); CAPACITY], len };
        batch.samples[..len].copy_from_slice(&self.samples[..len]);
        self.samples.copy_within(len..self.len, 0);
        self.len -= len;
        Some(batch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(kind: Kind, time_us: u64) -> Sample {
        Sample { kind, time_us, ..Sample::default() }
    }

    #[test]
    fn moves_coalesce_with_history_once_per_panel_tick() {
        let mut buffer = Buffer::new();
        buffer.push(sample(Kind::Press, 1));
        assert_eq!(buffer.take(0).unwrap().samples[0].kind, Kind::Press);
        buffer.push(sample(Kind::Move, 2));
        buffer.push(sample(Kind::Move, 3));
        let batch = buffer.take(1).unwrap();
        assert_eq!(batch.len, 2);
        assert_eq!((batch.samples[0].time_us, batch.samples[1].time_us), (2, 3));
        buffer.push(sample(Kind::Move, 4));
        assert!(buffer.take(1).is_none());
        assert_eq!(buffer.take(2).unwrap().samples[0].time_us, 4);
    }

    #[test]
    fn press_release_and_final_movement_survive_coalescing() {
        let mut buffer = Buffer::new();
        for (i, kind) in [Kind::Press, Kind::Move, Kind::Move, Kind::Release, Kind::Press]
            .into_iter()
            .enumerate()
        {
            buffer.push(sample(kind, i as u64));
        }
        assert_eq!(buffer.take(0).unwrap().samples[0].kind, Kind::Press);
        assert_eq!(buffer.take(0).unwrap().len, 2);
        assert_eq!(buffer.take(0).unwrap().samples[0].kind, Kind::Release);
        assert_eq!(buffer.take(0).unwrap().samples[0].kind, Kind::Press);
        assert!(buffer.take(0).is_none());
    }

    #[test]
    fn overflow_discards_motion_before_edges() {
        let mut buffer = Buffer::new();
        buffer.push(sample(Kind::Press, 0));
        for i in 1..CAPACITY as u64 {
            buffer.push(sample(Kind::Move, i));
        }
        buffer.push(sample(Kind::Release, 20));
        assert_eq!(buffer.overflows, 1);
        assert_eq!(buffer.take(0).unwrap().samples[0].kind, Kind::Press);
        assert_eq!(buffer.take(0).unwrap().len, CAPACITY - 2);
        assert_eq!(buffer.take(0).unwrap().samples[0].kind, Kind::Release);
    }

    #[test]
    fn overflow_of_edges_cancels_instead_of_leaving_a_pointer_held() {
        let mut buffer = Buffer::new();
        for i in 0..CAPACITY {
            buffer.push(sample(Kind::Press, i as u64));
        }
        buffer.push(sample(Kind::Release, 20));
        assert_eq!(buffer.take(0).unwrap().samples[0].kind, Kind::Cancel);
        assert_eq!(buffer.take(0).unwrap().samples[0].kind, Kind::Release);
    }
}
