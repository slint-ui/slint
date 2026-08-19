// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Drive `app_main` with a mock backend that captures one frame, to check the
//! scene compiles and renders without a real display.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint_safeui_app::{AppEvent, Platform, app_main, block_on};

/// Renders the scene once into `frame`, then reports a quit.
struct MockPlatform {
    frame: Rc<RefCell<Vec<slint::Rgb8Pixel>>>,
    width: u32,
    height: u32,
    captured: bool,
}

impl Platform for MockPlatform {
    type Pixel = slint::Rgb8Pixel;

    fn now(&self) -> Duration {
        Duration::ZERO
    }

    fn size(&self) -> slint::PhysicalSize {
        slint::PhysicalSize::new(self.width, self.height)
    }

    fn get_input_event(&mut self) -> Option<AppEvent> {
        // Quit once the first frame has been captured.
        self.captured.then_some(AppEvent::Quit)
    }

    async fn wait_for_more_events(&mut self, _timeout: Option<Duration>) {}

    fn with_frame_buffer(&mut self, render: impl FnOnce(&mut [Self::Pixel], usize)) {
        let mut buffer = vec![slint::Rgb8Pixel::default(); (self.width * self.height) as usize];
        render(&mut buffer, self.width as usize);
        *self.frame.borrow_mut() = buffer;
        self.captured = true;
    }
}

#[test]
fn renders_a_non_empty_frame() {
    let frame = Rc::new(RefCell::new(Vec::new()));
    let platform = MockPlatform { frame: frame.clone(), width: 320, height: 240, captured: false };

    block_on(app_main(platform)).unwrap();

    let frame = frame.borrow();
    assert_eq!(frame.len(), 320 * 240);
    // Something was drawn: the frame is not a single flat color.
    assert!(frame.iter().any(|pixel| *pixel != frame[0]), "the scene rendered a uniform frame",);
}
