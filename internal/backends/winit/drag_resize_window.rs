use winit::window::{CursorIcon, ResizeDirection};

pub fn handle_cursor_move_for_resize(
    window: &winit::window::Window,
    position: winit::dpi::PhysicalPosition<f64>,
    current_direction: Option<ResizeDirection>,
) -> Option<ResizeDirection> {
    if !window.is_decorated() {
        let location = get_resize_direction(window.inner_size(), position, 8_f64);

        if current_direction != location {
            window.set_cursor_icon(resize_direction_cursor_icon(location));
        }

        return location;
    }

    None
}

pub fn handle_resize(window: &winit::window::Window, direction: Option<ResizeDirection>) {
    if let Some(dir) = direction {
        let _ = window.drag_resize_window(dir);
    }
}

/// Get the cursor icon that corresponds to the resize direction.
fn resize_direction_cursor_icon(resize_direction: Option<ResizeDirection>) -> CursorIcon {
    match resize_direction {
        Some(resize_direction) => match resize_direction {
            ResizeDirection::East => CursorIcon::EResize,
            ResizeDirection::North => CursorIcon::NResize,
            ResizeDirection::NorthEast => CursorIcon::NeResize,
            ResizeDirection::NorthWest => CursorIcon::NwResize,
            ResizeDirection::South => CursorIcon::SResize,
            ResizeDirection::SouthEast => CursorIcon::SeResize,
            ResizeDirection::SouthWest => CursorIcon::SwResize,
            ResizeDirection::West => CursorIcon::WResize,
        },
        None => CursorIcon::Default,
    }
}

fn get_resize_direction(
    win_size: winit::dpi::PhysicalSize<u32>,
    position: winit::dpi::PhysicalPosition<f64>,
    border_size: f64,
) -> Option<ResizeDirection> {
    enum X {
        West,
        East,
        Default,
    }

    enum Y {
        North,
        South,
        Default,
    }

    let xdir = if position.x < border_size {
        X::West
    } else if position.x > (win_size.width as f64 - border_size) {
        X::East
    } else {
        X::Default
    };

    let ydir = if position.y < border_size {
        Y::North
    } else if position.y > (win_size.height as f64 - border_size) {
        Y::South
    } else {
        Y::Default
    };

    Some(match xdir {
        X::West => match ydir {
            Y::North => ResizeDirection::NorthWest,
            Y::South => ResizeDirection::SouthWest,
            Y::Default => ResizeDirection::West,
        },

        X::East => match ydir {
            Y::North => ResizeDirection::NorthEast,
            Y::South => ResizeDirection::SouthEast,
            Y::Default => ResizeDirection::East,
        },

        X::Default => match ydir {
            Y::North => ResizeDirection::North,
            Y::South => ResizeDirection::South,
            Y::Default => return None,
        },
    })
}
