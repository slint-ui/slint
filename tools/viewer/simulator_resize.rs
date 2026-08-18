// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::winit_030::WinitWindowAccessor;
use slint::winit_030::winit;
use std::cell::RefCell;
use std::rc::Rc;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::window::ResizeDirection;

const PHONE_WIDTH: f64 = 388.;
const PHONE_HEIGHT: f64 = 826.;
const CONTROL_AREA_HEIGHT: f64 = 68.;
const MIN_PHONE_SCALE: f64 = 1.;
const RESIZE_BORDER_WIDTH: f64 = 8.;

pub fn install(window: &slint::Window, frame_enabled: impl Fn() -> bool + 'static) {
    let state = Rc::new(RefCell::new(ResizeState::default()));
    window.on_winit_window_event(move |window, event| {
        let mut state = state.borrow_mut();

        match event {
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_position = *position;
            }
            WindowEvent::MouseInput { state: button_state, button: MouseButton::Left, .. } => {
                if *button_state == ElementState::Pressed && frame_enabled() {
                    state.session = window
                        .with_winit_window(|winit_window| {
                            ResizeSession::start(winit_window, state.cursor_position)
                        })
                        .flatten();
                } else if *button_state == ElementState::Released {
                    state.session = None;
                    state.requested_size = None;
                }
            }
            WindowEvent::Focused(false) => {
                state.session = None;
                state.requested_size = None;
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
                let constrained_logical = constrain_size(proposed_logical, session.direction);
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

#[derive(Default)]
struct ResizeState {
    cursor_position: PhysicalPosition<f64>,
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
        let direction = resize_direction(
            inner_size,
            cursor_position,
            RESIZE_BORDER_WIDTH * window.scale_factor(),
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

fn constrain_size(proposed_size: (f64, f64), direction: ResizeDirection) -> (f64, f64) {
    let width_scale = proposed_size.0 / PHONE_WIDTH;
    let height_scale = (proposed_size.1 - CONTROL_AREA_HEIGHT) / PHONE_HEIGHT;
    let scale = match direction {
        ResizeDirection::East | ResizeDirection::West => width_scale,
        ResizeDirection::North | ResizeDirection::South => height_scale,
        _ => {
            (PHONE_WIDTH * proposed_size.0 + PHONE_HEIGHT * (proposed_size.1 - CONTROL_AREA_HEIGHT))
                / (PHONE_WIDTH.powi(2) + PHONE_HEIGHT.powi(2))
        }
    }
    .max(MIN_PHONE_SCALE);

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

fn resize_direction(
    size: PhysicalSize<u32>,
    position: PhysicalPosition<f64>,
    border_width: f64,
) -> Option<ResizeDirection> {
    let west = position.x < border_width;
    let east = position.x > f64::from(size.width) - border_width;
    let north = position.y < border_width;
    let south = position.y > f64::from(size.height) - border_width;

    match (west, east, north, south) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::West),
        (_, true, _, _) => Some(ResizeDirection::East),
        (_, _, true, _) => Some(ResizeDirection::North),
        (_, _, _, true) => Some(ResizeDirection::South),
        _ => None,
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
    fn horizontal_resize_scales_only_the_phone() {
        assert_eq!(constrain_size((776., 1200.), ResizeDirection::East), (776., 1720.));
    }

    #[test]
    fn vertical_resize_scales_only_the_phone() {
        assert_eq!(constrain_size((500., 1720.), ResizeDirection::South), (776., 1720.));
    }

    #[test]
    fn resize_does_not_shrink_below_the_control_area() {
        assert_eq!(constrain_size((200., 400.), ResizeDirection::SouthEast), (388., 894.));
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
    fn resize_direction_uses_only_the_native_border() {
        let size = PhysicalSize::new(388, 894);
        assert_eq!(
            resize_direction(size, PhysicalPosition::new(4., 4.), 8.),
            Some(ResizeDirection::NorthWest)
        );
        assert_eq!(resize_direction(size, PhysicalPosition::new(12., 12.), 8.), None);
    }
}
