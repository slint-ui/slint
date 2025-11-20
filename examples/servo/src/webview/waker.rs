// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use servo::EventLoopWaker;
use smol::channel::Sender;

#[derive(Clone)]
pub struct Waker(Sender<()>);

impl Waker {
    pub fn new(sender: Sender<()>) -> Self {
        Self(sender)
    }
}

impl EventLoopWaker for Waker {
    /// Signals the event loop to wake up using the async channel.
    fn wake(&self) {
        self.0.try_send(()).expect("Failed to wake event loop");
    }

    /// Creates a boxed clone for Servo's event loop management.
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(Self(self.0.clone()))
    }
}
