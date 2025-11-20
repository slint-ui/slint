// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use servo::EventLoopWaker;
use smol::channel::Sender;

/// Event loop waker for integrating Servo's async operations with Slint.
///
/// The `Waker` implements Servo's `EventLoopWaker` trait to signal when the
/// Servo event loop needs to process pending events. It uses an async channel
/// to communicate between Servo's rendering thread and Slint's event loop.
///
/// # Thread Safety
///
/// This type is `Clone` and can be safely shared across threads via the
/// underlying channel sender.
#[derive(Clone)]
pub struct Waker(Sender<()>);

impl Waker {
    /// Creates a new waker with the given channel sender.
    ///
    /// # Arguments
    ///
    /// * `sender` - Channel sender for signaling the event loop
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
