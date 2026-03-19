use crate::Coord;
use crate::animations::Instant;
use crate::lengths::LogicalPoint;
use crate::lengths::LogicalPx;
use core::time::Duration;
use euclid::Vector2D;

#[derive(Debug)]
pub(crate) struct MoveDataRingbuffer<const N: usize> {
    curr_index: usize,
    full: bool,
    values: [(Instant, LogicalPoint); N],
}

impl<const N: usize> Default for MoveDataRingbuffer<N> {
    fn default() -> Self {
        Self { curr_index: 0, full: false, values: [(Instant::now(), LogicalPoint::default()); N] }
    }
}

impl<const N: usize> MoveDataRingbuffer<N> {
    pub fn empty(&self) -> bool {
        !(self.full || self.curr_index > 0)
    }

    pub fn push(&mut self, time: Instant, value: LogicalPoint) {
        if self.curr_index < self.values.len() {
            self.values[self.curr_index] = (time, value);
        }
        self.curr_index += 1;
        if self.curr_index >= N {
            self.full = true;
            self.curr_index = 0;
        }
    }

    pub fn diff(&self) -> (Duration, Vector2D<Coord, LogicalPx>) {
        if self.full {
            let oldest = self.values[self.curr_index];
            let newest = if self.curr_index > 0 {
                self.values[self.curr_index - 1]
            } else {
                self.values[self.values.len() - 1]
            };
            (newest.0.duration_since(oldest.0), newest.1 - oldest.1)
        } else {
            let oldest = self.values[0];
            let newest = self.values[usize::max(0, self.curr_index - 1)];
            (newest.0.duration_since(oldest.0), newest.1 - oldest.1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
