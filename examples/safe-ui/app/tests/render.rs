// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Drive `app_main` with a mock backend that captures the frames it renders, to
//! check the scene compiles, renders, and follows a touch without a real display.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use slint_safeui_app::{AppEvent, Platform, app_main, block_on, slint_sc};

/// The center of the ENTER panel, and a point in the leaf of its icon.
const ENTER_PANEL: (u32, u32) = (168, 217);
/// The green of the ENTER panel, and the orange of the wait panel that replaces
/// it, as `assets/action-enter.png` and `assets/action-wait-orange.png` paint
/// that point.
const ENTER_GREEN: [u8; 3] = [35, 149, 61];
const WAIT_ORANGE: [u8; 3] = [236, 124, 0];

/// Renders the ready screen, then delivers a tap on the ENTER panel and
/// renders again, before reporting a quit.
struct MockPlatform {
    frames: Rc<RefCell<Vec<Vec<u8>>>>,
    size: slint_sc::Size,
    events: VecDeque<slint_sc::TouchEvent>,
}

impl Platform for MockPlatform {
    fn now(&self) -> Duration {
        Duration::ZERO
    }

    fn size(&self) -> slint_sc::Size {
        self.size
    }

    fn get_input_event(&mut self) -> Option<AppEvent> {
        let frames = self.frames.borrow().len();
        // Let the ready screen render before the tap, and the screen the tap
        // leads to render before the quit.
        if frames == 0 {
            return None;
        }
        if let Some(touch) = self.events.pop_front() {
            return Some(AppEvent::Touch(touch));
        }
        (frames >= 2).then_some(AppEvent::Quit)
    }

    async fn wait_for_more_events(&mut self, _timeout: Option<Duration>) {}

    fn with_frame_buffer(&mut self, render: impl FnOnce(&mut [u8])) {
        let mut buffer = vec![0u8; (self.size.width * self.size.height * 3) as usize];
        render(&mut buffer);
        self.frames.borrow_mut().push(buffer);
    }
}

#[test]
fn a_tap_on_enter_starts_the_sequence() {
    let size = slint_sc::Size::new(320, 240);
    let position = slint_sc::Point::new(ENTER_PANEL.0 as i32, ENTER_PANEL.1 as i32);
    let frames = Rc::new(RefCell::new(Vec::new()));
    let platform = MockPlatform {
        frames: frames.clone(),
        size,
        events: VecDeque::from([
            slint_sc::TouchEvent::pressed(position),
            slint_sc::TouchEvent::released(position),
        ]),
    };

    block_on(app_main(platform));

    let frames = frames.borrow();
    assert_eq!(frames.len(), 2);
    let pixel = |frame: &Vec<u8>, x: u32, y: u32| {
        let i = ((y * size.width + x) * 3) as usize;
        [frame[i], frame[i + 1], frame[i + 2]]
    };

    // The ready screen shows the background and offers ENTER.
    assert_eq!(frames[0].len(), (size.width * size.height * 3) as usize);
    assert_eq!(pixel(&frames[0], 5, 5), [250, 252, 254]);
    assert_eq!(pixel(&frames[0], ENTER_PANEL.0, ENTER_PANEL.1), ENTER_GREEN);

    // The tap secures the doors, so the panel now tells the occupant to wait.
    assert_eq!(pixel(&frames[1], ENTER_PANEL.0, ENTER_PANEL.1), WAIT_ORANGE);
}
