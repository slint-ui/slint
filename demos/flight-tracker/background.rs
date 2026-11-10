// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Startup work that takes a moment, such as decoding imagery or generating the clouds,
//! done without holding up the first frames.

/// Work that runs on a thread of its own, or on the web, which has no threads, in a later frame.
pub struct Background<T> {
    #[cfg(not(target_arch = "wasm32"))]
    receiver: std::sync::mpsc::Receiver<T>,
    #[cfg(target_arch = "wasm32")]
    work: Option<Box<dyn FnOnce() -> T>>,
    /// The frames to wait on the web before running the work.
    #[cfg(target_arch = "wasm32")]
    frames: u32,
}

impl<T: Send + 'static> Background<T> {
    /// Starts `work`, which on the web runs in the frame `frame` frames from now,
    /// so that several pieces of work spread over several frames.
    pub fn start(frame: u32, work: impl FnOnce() -> T + Send + 'static) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = frame;
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || sender.send(work()));
            Self { receiver }
        }
        #[cfg(target_arch = "wasm32")]
        Self { work: Some(Box::new(work)), frames: frame }
    }

    /// Whether `poll()` has handed out the result.
    #[cfg(target_arch = "wasm32")]
    pub fn is_done(&self) -> bool {
        self.work.is_none()
    }

    /// The result once it's ready, and only that once.
    pub fn poll(&mut self) -> Option<T> {
        #[cfg(not(target_arch = "wasm32"))]
        return self.receiver.try_recv().ok();
        #[cfg(target_arch = "wasm32")]
        {
            if self.frames > 0 {
                self.frames -= 1;
                return None;
            }
            Some((self.work.take()?)())
        }
    }
}

/// Fills `out`, which holds rows of `row_length` bytes, with `fill`,
/// which gets the index of a row and the rows from there on.
/// Where there are threads, and enough rows to be worth them,
/// the rows are shared out among all CPU cores.
pub fn fill_rows(out: &mut [u8], row_length: usize, fill: impl Fn(usize, &mut [u8]) + Sync) {
    let rows = out.len() / row_length.max(1);
    let threads = if cfg!(target_arch = "wasm32") || rows < 64 {
        1
    } else {
        std::thread::available_parallelism().map_or(1, |n| n.get())
    };
    if threads == 1 {
        return fill(0, out);
    }
    let rows_per_thread = rows.div_ceil(threads);
    std::thread::scope(|scope| {
        for (index, chunk) in out.chunks_mut(rows_per_thread * row_length).enumerate() {
            let fill = &fill;
            scope.spawn(move || fill(index * rows_per_thread, chunk));
        }
    });
}
