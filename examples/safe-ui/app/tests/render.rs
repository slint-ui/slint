// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Drive `app_main` with a mock backend that captures one frame, to check the
//! scene compiles and renders the expected telltales without a real display.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint_safeui_app::{AppEvent, Platform, app_main, block_on, slint_sc};

/// Renders the scene once into `frame`, then reports a quit.
struct MockPlatform {
    frame: Rc<RefCell<Vec<u8>>>,
    size: slint_sc::Size,
    captured: bool,
}

impl Platform for MockPlatform {
    fn now(&self) -> Duration {
        Duration::ZERO
    }

    fn size(&self) -> slint_sc::Size {
        self.size
    }

    fn get_input_event(&mut self) -> Option<AppEvent> {
        // Quit once the first frame has been captured.
        self.captured.then_some(AppEvent::Quit)
    }

    async fn wait_for_more_events(&mut self, _timeout: Option<Duration>) {}

    fn with_frame_buffer(&mut self, render: impl FnOnce(&mut [u8])) {
        let mut buffer = vec![0u8; (self.size.width * self.size.height * 3) as usize];
        render(&mut buffer);
        *self.frame.borrow_mut() = buffer;
        self.captured = true;
    }
}

#[test]
fn renders_the_first_telltale() {
    let size = slint_sc::Size::new(260, 100);
    let frame = Rc::new(RefCell::new(Vec::new()));
    let platform = MockPlatform { frame: frame.clone(), size, captured: false };

    block_on(app_main(platform));

    let frame = frame.borrow();
    assert_eq!(frame.len(), (size.width * size.height * 3) as usize);

    let pixel = |x: u32, y: u32| {
        let i = ((y * size.width + x) * 3) as usize;
        [frame[i], frame[i + 1], frame[i + 2]]
    };
    // The background is black, and at time zero the first telltale is green
    // (its center is at 20 + 60/2 = 50 on both axes).
    assert_eq!(pixel(5, 5), [0, 0, 0]);
    assert_eq!(pixel(50, 50), [0, 0x80, 0]);
}
