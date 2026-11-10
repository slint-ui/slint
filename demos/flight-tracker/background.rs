// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Startup work that takes a moment, such as decoding imagery or generating the clouds, done
//! without holding up the first frames.

/// Work that runs on a thread of its own, or on the web, which has no threads, in a later frame.
pub struct Background<T> {
    #[cfg(not(target_arch = "wasm32"))]
    receiver: std::sync::mpsc::Receiver<T>,
    #[cfg(target_arch = "wasm32")]
    work: Option<Box<dyn FnOnce() -> T>>,
    /// The frames to wait on the web before running the work.
    #[cfg(target_arch = "wasm32")]
    frames: u32,
    done: bool,
}

impl<T: Send + 'static> Background<T> {
    /// Starts `work`, which on the web runs in the frame `frame` frames from now, so that
    /// several pieces of work spread over several frames.
    pub fn start(frame: u32, work: impl FnOnce() -> T + Send + 'static) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = frame;
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || sender.send(work()));
            Self { receiver, done: false }
        }
        #[cfg(target_arch = "wasm32")]
        Self { work: Some(Box::new(work)), frames: frame, done: false }
    }

    /// The result once it's ready, and only that once.
    pub fn poll(&mut self) -> Option<T> {
        if self.done {
            return None;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let result = self.receiver.try_recv().ok()?;
        #[cfg(target_arch = "wasm32")]
        let result = {
            if self.frames > 0 {
                self.frames -= 1;
                return None;
            }
            (self.work.take()?)()
        };
        self.done = true;
        Some(result)
    }
}
