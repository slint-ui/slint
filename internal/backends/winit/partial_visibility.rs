// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Tracks when a partially off-screen Windows window needs a fresh present.

/// A window rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl WindowRect {
    pub(crate) const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    fn area(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }
}

/// A monitor rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MonitorRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl MonitorRect {
    pub(crate) const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }
}

/// The currently visible portion of a native window.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VisibilitySnapshot {
    total_area: u64,
    visible_area: u64,
}

impl VisibilitySnapshot {
    pub(crate) fn from_rects(window: WindowRect, monitors: &[MonitorRect]) -> Self {
        let visible_area = monitors
            .iter()
            .map(|monitor| intersection_area(window, *monitor))
            .sum();

        Self { total_area: window.area(), visible_area }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum RecoveryMode {
    #[default]
    Idle,
    RequestRedrawOnReveal,
    PresentBufferOnVisibilityIncrease,
}

/// The next action the backend should take after a visibility change.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VisibilityRecoveryAction {
    #[default]
    None,
    RequestRedraw,
    PresentExistingBuffer,
}

/// Tracks whether the current frame has been presented while the window was only partially visible.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VisibilityRecoveryController {
    mode: RecoveryMode,
    last_visible_area: u64,
}

impl VisibilityRecoveryController {
    /// Marks the native window contents as unavailable after a zero-sized occlusion.
    pub(crate) fn note_zero_sized_occlusion(&mut self) {
        self.mode = RecoveryMode::RequestRedrawOnReveal;
        self.last_visible_area = 0;
    }

    /// Records that the latest frame was rendered while the window had the given visibility.
    pub(crate) fn note_rendered_frame(&mut self, snapshot: VisibilitySnapshot) {
        self.last_visible_area = snapshot.visible_area;
        self.mode = if snapshot.total_area > 0 && snapshot.visible_area < snapshot.total_area {
            RecoveryMode::PresentBufferOnVisibilityIncrease
        } else {
            RecoveryMode::Idle
        };
    }

    /// Updates the controller after a native move/resize and returns the next required action.
    pub(crate) fn on_visibility_changed(
        &mut self,
        snapshot: VisibilitySnapshot,
    ) -> VisibilityRecoveryAction {
        if snapshot.total_area == 0 {
            self.last_visible_area = 0;
            return VisibilityRecoveryAction::None;
        }

        let grew = snapshot.visible_area > self.last_visible_area;
        self.last_visible_area = snapshot.visible_area;

        match self.mode {
            RecoveryMode::Idle => VisibilityRecoveryAction::None,
            RecoveryMode::RequestRedrawOnReveal => {
                if snapshot.visible_area > 0 {
                    VisibilityRecoveryAction::RequestRedraw
                } else {
                    VisibilityRecoveryAction::None
                }
            }
            RecoveryMode::PresentBufferOnVisibilityIncrease => {
                if grew {
                    if snapshot.visible_area >= snapshot.total_area {
                        self.mode = RecoveryMode::Idle;
                    }
                    VisibilityRecoveryAction::PresentExistingBuffer
                } else {
                    VisibilityRecoveryAction::None
                }
            }
        }
    }
}

pub(crate) fn intersection_area(window: WindowRect, monitor: MonitorRect) -> u64 {
    let left = i64::from(window.x).max(i64::from(monitor.x));
    let top = i64::from(window.y).max(i64::from(monitor.y));
    let right = window.right().min(monitor.right());
    let bottom = window.bottom().min(monitor.bottom());

    if right <= left || bottom <= top {
        return 0;
    }

    let width = u64::try_from(right - left).unwrap_or_default();
    let height = u64::try_from(bottom - top).unwrap_or_default();
    width * height
}

#[cfg(test)]
mod tests {
    use super::{
        MonitorRect, VisibilityRecoveryAction, VisibilityRecoveryController, VisibilitySnapshot,
        WindowRect,
    };

    const MONITOR: MonitorRect = MonitorRect::new(0, 0, 100, 100);

    #[test]
    fn fully_visible_window_stays_idle_after_render() {
        let mut controller = VisibilityRecoveryController::default();
        controller.note_rendered_frame(VisibilitySnapshot::from_rects(
            WindowRect::new(0, 0, 100, 100),
            &[MONITOR],
        ));

        assert_eq!(
            controller.on_visibility_changed(VisibilitySnapshot::from_rects(
                WindowRect::new(0, 0, 100, 100),
                &[MONITOR],
            )),
            VisibilityRecoveryAction::None
        );
    }

    #[test]
    fn partially_visible_render_requests_present_when_visibility_grows() {
        let mut controller = VisibilityRecoveryController::default();
        controller.note_rendered_frame(VisibilitySnapshot::from_rects(
            WindowRect::new(-40, 0, 100, 100),
            &[MONITOR],
        ));

        assert_eq!(
            controller.on_visibility_changed(VisibilitySnapshot::from_rects(
                WindowRect::new(-20, 0, 100, 100),
                &[MONITOR],
            )),
            VisibilityRecoveryAction::PresentExistingBuffer
        );
    }

    #[test]
    fn partially_visible_render_ignores_visibility_shrinks() {
        let mut controller = VisibilityRecoveryController::default();
        controller.note_rendered_frame(VisibilitySnapshot::from_rects(
            WindowRect::new(-20, 0, 100, 100),
            &[MONITOR],
        ));

        assert_eq!(
            controller.on_visibility_changed(VisibilitySnapshot::from_rects(
                WindowRect::new(-40, 0, 100, 100),
                &[MONITOR],
            )),
            VisibilityRecoveryAction::None
        );
    }

    #[test]
    fn zero_sized_occlusion_requests_redraw_on_reveal() {
        let mut controller = VisibilityRecoveryController::default();
        controller.note_zero_sized_occlusion();

        assert_eq!(
            controller.on_visibility_changed(VisibilitySnapshot::from_rects(
                WindowRect::new(0, 0, 100, 100),
                &[MONITOR],
            )),
            VisibilityRecoveryAction::RequestRedraw
        );
    }
}
