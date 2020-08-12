//! Functions usefull for testing

use crate::input::{MouseEvent, MouseEventType};

/// SixtyFPS animations do not use real time, but use a mocked time.
/// Normally, the event loop update the time of the animation using
/// real time, but in tests, it is more convinient to use the fake time.
/// This function will add some milliseconds to the fake time
#[no_mangle]
pub extern "C" fn sixtyfps_mock_elapsed_time(time_in_ms: u64) {
    crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
        let mut tick = driver.current_tick();
        tick += instant::Duration::from_millis(time_in_ms);
        driver.update_animations(tick)
    })
}

/// Simulate a click on a position within the component.
#[no_mangle]
pub extern "C" fn sixtyfps_send_mouse_click(
    component: core::pin::Pin<crate::ComponentRef>,
    x: f32,
    y: f32,
) {
    let pos = euclid::point2(x, y);
    component.as_ref().input_event(MouseEvent { pos, what: MouseEventType::MouseMoved });
    component.as_ref().input_event(MouseEvent { pos, what: MouseEventType::MousePressed });
    sixtyfps_mock_elapsed_time(50);
    component.as_ref().input_event(MouseEvent { pos, what: MouseEventType::MouseReleased });
}
