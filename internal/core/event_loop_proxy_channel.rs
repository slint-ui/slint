// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::api::EventLoopError;
use crate::platform::EventLoopProxy;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender};

enum Message {
    Invoke(Box<dyn FnOnce() + Send>),
    Quit,
}

/// A generic implementation of [`EventLoopProxy`] that uses a [`std::sync::mpsc`] channel.
///
/// This can be used by custom platforms to implement [`crate::platform::Platform::new_event_loop_proxy`]
/// without having to implement the callback queueing and wake-up plumbing manually.
#[derive(Clone)]
pub struct ChannelEventLoopProxy {
    sender: Sender<Message>,
    wakeup: Option<Arc<dyn Fn() + Send + Sync>>,
    quit_requested: Arc<AtomicBool>,
}

impl EventLoopProxy for ChannelEventLoopProxy {
    fn quit_event_loop(&self) -> Result<(), EventLoopError> {
        self.quit_requested.store(true, Ordering::SeqCst);
        self.sender.send(Message::Quit).map_err(|_| EventLoopError::EventLoopTerminated)?;
        if let Some(wakeup) = &self.wakeup {
            wakeup();
        }
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), EventLoopError> {
        self.sender
            .send(Message::Invoke(event))
            .map_err(|_| EventLoopError::EventLoopTerminated)?;
        if let Some(wakeup) = &self.wakeup {
            wakeup();
        }
        Ok(())
    }
}

/// The receiver side of the channel created by [`channel_event_loop_proxy`].
///
/// This should be owned by the host event loop and drained regularly.
pub struct ChannelEventLoopReceiver {
    receiver: Mutex<Receiver<Message>>,
    quit_requested: Arc<AtomicBool>,
}

impl ChannelEventLoopReceiver {
    /// Runs all pending callbacks.
    ///
    /// Returns `core::ops::ControlFlow::Break(())` if `quit_event_loop()` was requested
    /// through a proxy, otherwise `core::ops::ControlFlow::Continue(())`.
    pub fn drain(&self) -> core::ops::ControlFlow<()> {
        let mut quit_seen = false;
        loop {
            let msg = {
                let receiver = self.receiver.lock().unwrap();
                receiver.try_recv()
            };

            let Ok(msg) = msg else { break };
            match msg {
                Message::Invoke(f) => f(),
                Message::Quit => {
                    quit_seen = true;
                }
            }
        }
        if quit_seen {
            self.quit_requested.store(false, Ordering::SeqCst);
            core::ops::ControlFlow::Break(())
        } else {
            core::ops::ControlFlow::Continue(())
        }
    }

    /// Returns `true` if `quit_event_loop()` was requested through a proxy.
    pub fn quit_requested(&self) -> bool {
        self.quit_requested.load(Ordering::SeqCst)
    }
}

/// Creates a pair of [`ChannelEventLoopProxy`] and [`ChannelEventLoopReceiver`].
///
/// The `wakeup` closure is called every time a message is sent to the proxy. It can be used
/// to wake up the host event loop if it's sleeping.
pub fn channel_event_loop_proxy(
    wakeup: Option<Box<dyn Fn() + Send + Sync>>,
) -> (ChannelEventLoopProxy, ChannelEventLoopReceiver) {
    let (sender, receiver) = std::sync::mpsc::channel();
    let quit_requested = Arc::new(AtomicBool::new(false));
    (
        ChannelEventLoopProxy {
            sender,
            wakeup: wakeup.map(Arc::from),
            quit_requested: quit_requested.clone(),
        },
        ChannelEventLoopReceiver { receiver: Mutex::new(receiver), quit_requested },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_channel_proxy() {
        let wakeup_called = Arc::new(AtomicBool::new(false));
        let wakeup_called_clone = wakeup_called.clone();
        let (proxy, receiver) = channel_event_loop_proxy(Some(Box::new(move || {
            wakeup_called_clone.store(true, Ordering::SeqCst);
        })));

        let invoked = Arc::new(AtomicBool::new(false));
        let invoked_clone = invoked.clone();
        proxy
            .invoke_from_event_loop(Box::new(move || {
                invoked_clone.store(true, Ordering::SeqCst);
            }))
            .unwrap();

        assert!(wakeup_called.load(Ordering::SeqCst));
        assert!(!invoked.load(Ordering::SeqCst));

        assert_eq!(receiver.drain(), core::ops::ControlFlow::Continue(()));
        assert!(invoked.load(Ordering::SeqCst));
        assert!(!receiver.quit_requested());

        wakeup_called.store(false, Ordering::SeqCst);
        proxy.quit_event_loop().unwrap();
        assert!(wakeup_called.load(Ordering::SeqCst));
        assert!(receiver.quit_requested());
        assert_eq!(receiver.drain(), core::ops::ControlFlow::Break(()));
        assert!(!receiver.quit_requested());

        let invoked_again = Arc::new(AtomicBool::new(false));
        let invoked_again_clone = invoked_again.clone();
        proxy
            .invoke_from_event_loop(Box::new(move || {
                invoked_again_clone.store(true, Ordering::SeqCst);
            }))
            .unwrap();
        assert_eq!(receiver.drain(), core::ops::ControlFlow::Continue(()));
        assert!(invoked_again.load(Ordering::SeqCst));
    }

    #[test]
    fn test_drain_does_not_hold_lock_while_invoking() {
        let (proxy, receiver) = channel_event_loop_proxy(None);
        let receiver = Arc::new(receiver);
        let invoked = Arc::new(AtomicBool::new(false));
        let receiver_clone = receiver.clone();
        let invoked_clone = invoked.clone();

        proxy
            .invoke_from_event_loop(Box::new(move || {
                assert_eq!(receiver_clone.drain(), core::ops::ControlFlow::Continue(()));
                invoked_clone.store(true, Ordering::SeqCst);
            }))
            .unwrap();

        assert_eq!(receiver.drain(), core::ops::ControlFlow::Continue(()));
        assert!(invoked.load(Ordering::SeqCst));
    }

    #[test]
    fn test_channel_proxy_without_wakeup() {
        let (proxy, receiver) = channel_event_loop_proxy(None);

        let invoked = Arc::new(AtomicBool::new(false));
        let invoked_clone = invoked.clone();
        proxy
            .invoke_from_event_loop(Box::new(move || {
                invoked_clone.store(true, Ordering::SeqCst);
            }))
            .unwrap();

        assert_eq!(receiver.drain(), core::ops::ControlFlow::Continue(()));
        assert!(invoked.load(Ordering::SeqCst));

        proxy.quit_event_loop().unwrap();
        assert!(receiver.quit_requested());
        assert_eq!(receiver.drain(), core::ops::ControlFlow::Break(()));
        assert!(!receiver.quit_requested());
    }
}
