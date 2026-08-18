// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::winit_030::WinitWindowAccessor;
use slint::winit_030::winit;
use std::cell::RefCell;
use std::rc::Rc;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::window::{CursorIcon, ResizeDirection};

const PHONE_WIDTH: f64 = 388.;
const PHONE_HEIGHT: f64 = 826.;
const CONTROL_AREA_HEIGHT: f64 = 68.;
const MIN_PHONE_SCALE: f64 = 0.6;
const RESIZE_CORNER_SIZE: f64 = 16.;

pub fn install(window: &slint::Window, frame_enabled: impl Fn() -> bool + 'static) {
    set_frame_enabled(window, frame_enabled());
    let state = Rc::new(RefCell::new(ResizeState::default()));
    window.on_winit_window_event(move |window, event| {
        let mut state = state.borrow_mut();

        match event {
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_position = *position;
                let frame_enabled = frame_enabled();
                let resize_active = state.session.is_some();
                let direction = window
                    .with_winit_window(|winit_window| {
                        let direction = frame_enabled
                            .then(|| {
                                corner_resize_direction(
                                    winit_window.inner_size(),
                                    *position,
                                    RESIZE_CORNER_SIZE * winit_window.scale_factor(),
                                )
                            })
                            .flatten();
                        if !resize_active {
                            winit_window.set_resizable(!frame_enabled || direction.is_some());
                        }
                        direction
                    })
                    .flatten();
                if state.hover_direction != direction {
                    state.hover_direction = direction;
                    window.with_winit_window(|winit_window| {
                        winit_window.set_cursor(resize_cursor(direction));
                    });
                }
            }
            WindowEvent::MouseInput { state: button_state, button: MouseButton::Left, .. } => {
                if *button_state == ElementState::Pressed && frame_enabled() {
                    let session = window
                        .with_winit_window(|winit_window| {
                            let session =
                                ResizeSession::start(winit_window, state.cursor_position)?;
                            winit_window.set_resizable(true);
                            if winit_window.drag_resize_window(session.direction).is_err() {
                                winit_window.set_resizable(false);
                                return None;
                            }
                            Some(session)
                        })
                        .flatten();
                    if session.is_some() {
                        state.session = session;
                        return slint::winit_030::EventResult::PreventDefault;
                    }
                } else if *button_state == ElementState::Released {
                    state.session = None;
                    state.requested_size = None;
                    set_resizable_for_corner(window, frame_enabled(), state.hover_direction);
                }
            }
            WindowEvent::Focused(false) => {
                state.session = None;
                state.requested_size = None;
                set_resizable_for_corner(window, frame_enabled(), None);
            }
            WindowEvent::Resized(proposed_size) if frame_enabled() => {
                if state
                    .requested_size
                    .is_some_and(|requested| sizes_match(requested, *proposed_size))
                {
                    state.requested_size = None;
                    return slint::winit_030::EventResult::Propagate;
                }

                let Some(session) = state.session else {
                    return slint::winit_030::EventResult::Propagate;
                };
                let scale_factor =
                    window.with_winit_window(winit::window::Window::scale_factor).unwrap_or(1.);
                let proposed_logical = (
                    proposed_size.width as f64 / scale_factor,
                    proposed_size.height as f64 / scale_factor,
                );
                let constrained_logical = constrain_size(proposed_logical);
                let constrained_physical = PhysicalSize::new(
                    (constrained_logical.0 * scale_factor).round() as u32,
                    (constrained_logical.1 * scale_factor).round() as u32,
                );

                if sizes_match(*proposed_size, constrained_physical) {
                    state.requested_size = None;
                    return slint::winit_030::EventResult::Propagate;
                }

                state.requested_size = Some(constrained_physical);
                window.with_winit_window(|winit_window| {
                    let _ = winit_window.request_inner_size(constrained_physical);
                    winit_window.set_outer_position(session.position_for(constrained_physical));
                });
                return slint::winit_030::EventResult::PreventDefault;
            }
            _ => {}
        }

        slint::winit_030::EventResult::Propagate
    });
}

pub fn set_frame_enabled(window: &slint::Window, enabled: bool) {
    set_resizable_for_corner(window, enabled, None);
}

fn set_resizable_for_corner(
    window: &slint::Window,
    frame_enabled: bool,
    corner: Option<ResizeDirection>,
) {
    window.with_winit_window(|winit_window| {
        winit_window.set_resizable(!frame_enabled || corner.is_some());
    });
}

#[derive(Default)]
struct ResizeState {
    cursor_position: PhysicalPosition<f64>,
    hover_direction: Option<ResizeDirection>,
    session: Option<ResizeSession>,
    requested_size: Option<PhysicalSize<u32>>,
}

#[derive(Clone, Copy)]
struct ResizeSession {
    direction: ResizeDirection,
    start_position: PhysicalPosition<i32>,
    start_inner_size: PhysicalSize<u32>,
    frame_size: PhysicalSize<u32>,
}

impl ResizeSession {
    fn start(
        window: &winit::window::Window,
        cursor_position: PhysicalPosition<f64>,
    ) -> Option<Self> {
        let inner_size = window.inner_size();
        let direction = corner_resize_direction(
            inner_size,
            cursor_position,
            RESIZE_CORNER_SIZE * window.scale_factor(),
        )?;
        let outer_size = window.outer_size();
        Some(Self {
            direction,
            start_position: window.outer_position().ok()?,
            start_inner_size: inner_size,
            frame_size: PhysicalSize::new(
                outer_size.width.saturating_sub(inner_size.width),
                outer_size.height.saturating_sub(inner_size.height),
            ),
        })
    }

    fn position_for(self, inner_size: PhysicalSize<u32>) -> PhysicalPosition<i32> {
        let start_outer_size = PhysicalSize::new(
            self.start_inner_size.width + self.frame_size.width,
            self.start_inner_size.height + self.frame_size.height,
        );
        let new_outer_size = PhysicalSize::new(
            inner_size.width + self.frame_size.width,
            inner_size.height + self.frame_size.height,
        );
        anchored_position(self.start_position, start_outer_size, new_outer_size, self.direction)
    }
}

fn constrain_size(proposed_size: (f64, f64)) -> (f64, f64) {
    let scale = ((PHONE_WIDTH * proposed_size.0
        + PHONE_HEIGHT * (proposed_size.1 - CONTROL_AREA_HEIGHT))
        / (PHONE_WIDTH.powi(2) + PHONE_HEIGHT.powi(2)))
    .max(MIN_PHONE_SCALE);
    size_for_scale(scale)
}

fn size_for_scale(scale: f64) -> (f64, f64) {
    (PHONE_WIDTH * scale, CONTROL_AREA_HEIGHT + PHONE_HEIGHT * scale)
}

fn anchored_position(
    start_position: PhysicalPosition<i32>,
    start_size: PhysicalSize<u32>,
    new_size: PhysicalSize<u32>,
    direction: ResizeDirection,
) -> PhysicalPosition<i32> {
    let width_delta = i64::from(start_size.width) - i64::from(new_size.width);
    let height_delta = i64::from(start_size.height) - i64::from(new_size.height);

    let x_delta = match direction {
        ResizeDirection::West | ResizeDirection::NorthWest | ResizeDirection::SouthWest => {
            width_delta
        }
        ResizeDirection::North | ResizeDirection::South => width_delta / 2,
        _ => 0,
    };
    let y_delta = match direction {
        ResizeDirection::North | ResizeDirection::NorthEast | ResizeDirection::NorthWest => {
            height_delta
        }
        ResizeDirection::East | ResizeDirection::West => height_delta / 2,
        _ => 0,
    };

    PhysicalPosition::new(
        saturating_i32(i64::from(start_position.x) + x_delta),
        saturating_i32(i64::from(start_position.y) + y_delta),
    )
}

fn corner_resize_direction(
    size: PhysicalSize<u32>,
    position: PhysicalPosition<f64>,
    corner_size: f64,
) -> Option<ResizeDirection> {
    let west = position.x < corner_size;
    let east = position.x > f64::from(size.width) - corner_size;
    let north = position.y < corner_size;
    let south = position.y > f64::from(size.height) - corner_size;

    match (west, east, north, south) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        _ => None,
    }
}

fn resize_cursor(direction: Option<ResizeDirection>) -> CursorIcon {
    match direction {
        Some(ResizeDirection::NorthWest | ResizeDirection::SouthEast) => CursorIcon::NwseResize,
        Some(ResizeDirection::NorthEast | ResizeDirection::SouthWest) => CursorIcon::NeswResize,
        _ => CursorIcon::Default,
    }
}

fn sizes_match(first: PhysicalSize<u32>, second: PhysicalSize<u32>) -> bool {
    first.width.abs_diff(second.width) <= 1 && first.height.abs_diff(second.height) <= 1
}

fn saturating_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corner_resize_scales_the_whole_silhouette() {
        assert_eq!(constrain_size((776., 1720.)), (776., 1720.));
    }

    #[test]
    fn resize_does_not_shrink_below_sixty_percent() {
        assert_size(constrain_size((200., 400.)), (232.8, 563.6));
    }

    #[test]
    fn corner_resize_can_cross_below_full_size_without_extra_width() {
        assert_size(constrain_size((310.4, 728.8)), (310.4, 728.8));
    }

    #[test]
    fn north_west_resize_keeps_the_opposite_corner_fixed() {
        let position = anchored_position(
            PhysicalPosition::new(100, 100),
            PhysicalSize::new(388, 894),
            PhysicalSize::new(776, 1720),
            ResizeDirection::NorthWest,
        );
        assert_eq!(position, PhysicalPosition::new(-288, -726));
        assert_eq!(position.x + 776, 488);
        assert_eq!(position.y + 1720, 994);
    }

    #[test]
    fn south_east_resize_keeps_the_opposite_corner_fixed() {
        let position = anchored_position(
            PhysicalPosition::new(100, 100),
            PhysicalSize::new(388, 894),
            PhysicalSize::new(776, 1720),
            ResizeDirection::SouthEast,
        );
        assert_eq!(position, PhysicalPosition::new(100, 100));
    }

    #[test]
    fn resize_direction_uses_only_the_corners() {
        let size = PhysicalSize::new(388, 894);
        assert_eq!(
            corner_resize_direction(size, PhysicalPosition::new(4., 4.), 16.),
            Some(ResizeDirection::NorthWest)
        );
        assert_eq!(
            corner_resize_direction(size, PhysicalPosition::new(384., 890.), 16.),
            Some(ResizeDirection::SouthEast)
        );
        assert_eq!(corner_resize_direction(size, PhysicalPosition::new(4., 447.), 16.), None);
        assert_eq!(corner_resize_direction(size, PhysicalPosition::new(194., 4.), 16.), None);
        assert_eq!(corner_resize_direction(size, PhysicalPosition::new(20., 20.), 16.), None);
    }

    fn assert_size(actual: (f64, f64), expected: (f64, f64)) {
        assert!((actual.0 - expected.0).abs() < 0.001, "{actual:?} != {expected:?}");
        assert!((actual.1 - expected.1).abs() < 0.001, "{actual:?} != {expected:?}");
    }
}
