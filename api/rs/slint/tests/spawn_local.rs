// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

mod fake_backend {
    enum Event {
        Quit,
        Event(Box<dyn FnOnce() + Send>),
    }
    #[derive(Clone)]
    struct Queue(
        std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<Event>>>,
        std::thread::Thread,
    );
    pub struct FakeBackend {
        queue: Queue,
    }

    impl Default for FakeBackend {
        fn default() -> Self {
            Self { queue: Queue(Default::default(), std::thread::current()) }
        }
    }
    impl slint::platform::Platform for FakeBackend {
        fn create_window_adapter(
            &self,
        ) -> Result<std::rc::Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
            unimplemented!()
        }
        fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
            loop {
                let e = self.queue.0.lock().unwrap().pop_front();
                match e {
                    Some(Event::Quit) => break Ok(()),
                    Some(Event::Event(e)) => e(),
                    None => std::thread::park(),
                }
            }
        }
        fn new_event_loop_proxy(&self) -> Option<Box<dyn slint::platform::EventLoopProxy>> {
            Some(Box::new(self.queue.clone()))
        }
    }
    impl slint::platform::EventLoopProxy for Queue {
        fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
            self.0.lock().unwrap().push_back(Event::Quit);
            self.1.unpark();
            Ok(())
        }

        fn invoke_from_event_loop(
            &self,
            event: Box<dyn FnOnce() + Send>,
        ) -> Result<(), slint::EventLoopError> {
            self.0.lock().unwrap().push_back(Event::Event(event));
            self.1.unpark();
            Ok(())
        }
    }
}

/// Code from https://doc.rust-lang.org/std/task/trait.Wake.html#examples
mod executor {
    use std::future::Future;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake};
    use std::thread::{self, Thread};

    /// A waker that wakes up the current thread when called.
    struct ThreadWaker(Thread);

    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    /// Run a future to completion on the current thread.
    pub fn block_on<T>(fut: impl Future<Output = T>) -> T {
        // Pin the future so it can be polled.
        let mut fut = Box::pin(fut);

        // Create a new context to be passed to the future.
        let t = thread::current();
        let waker = Arc::new(ThreadWaker(t)).into();
        let mut cx = Context::from_waker(&waker);

        // Run the future to completion.
        loop {
            match fut.as_mut().poll(&mut cx) {
                Poll::Ready(res) => return res,
                Poll::Pending => thread::park(),
            }
        }
    }
}

#[test]
fn main() {
    slint::platform::set_platform(Box::new(fake_backend::FakeBackend::default())).unwrap();

    slint::invoke_from_event_loop(|| {
        let handle = slint::spawn_local(async { String::from("Hello") }).unwrap();
        slint::spawn_local(async move { panic!("Aborted task") }).unwrap().abort();
        let handle2 = slint::spawn_local(async move { handle.await + ", World" }).unwrap();
        std::thread::spawn(move || {
            let x = executor::block_on(handle2);
            assert_eq!(x, "Hello, World");
            slint::quit_event_loop().unwrap();
        });
    })
    .unwrap();
    slint::run_event_loop().unwrap();
}
