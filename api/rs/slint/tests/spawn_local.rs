// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
    i_slint_backend_testing::init_integration_test_with_mock_time();

    // test_spawn_local_from_thread
    std::thread::spawn(|| {
        assert_eq!(
            slint::spawn_local(async {
                panic!("the future shouldn't be run since we're in a thread")
            })
            .map(drop),
            Err(slint::EventLoopError::NoEventLoopProvider)
        );
    })
    .join()
    .unwrap();

    slint::invoke_from_event_loop(|| {
        let handle = slint::spawn_local(async { String::from("Hello") }).unwrap();
        slint::spawn_local(async move { panic!("Aborted task") }).unwrap().abort();
        let handle2 = slint::spawn_local(async move { handle.await + ", World" }).unwrap();
        std::thread::spawn(move || {
            let x = executor::block_on(handle2);
            assert_eq!(x, "Hello, World");
        });
    })
    .unwrap();

    // test_is_finished
    slint::invoke_from_event_loop(|| {
        let handle_one = slint::spawn_local(async { "Hello, World!" }).unwrap();
        assert!(!handle_one.is_finished());
        let handle_two = slint::spawn_local(async move {
            assert!(handle_one.is_finished());
        })
        .unwrap();
        std::thread::spawn(move || {
            let _ = executor::block_on(handle_two);
        });
        slint::quit_event_loop().unwrap();
    })
    .unwrap();
    slint::run_event_loop().unwrap();
}

#[test]
fn with_context() {
    use i_slint_core::SlintContext;
    let ctx = SlintContext::new(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions { mock_time: true, threading: true },
    )));
    let handle = ctx.spawn_local(async { String::from("Hello") }).unwrap();
    ctx.spawn_local(async move { panic!("Aborted task") }).unwrap().abort();
    let handle2 = ctx.spawn_local(async move { handle.await + ", World" }).unwrap();
    let proxy = ctx.event_loop_proxy().unwrap();
    std::thread::spawn(move || {
        let x = executor::block_on(handle2);
        assert_eq!(x, "Hello, World");
        proxy.quit_event_loop().unwrap();
    });
    ctx.run_event_loop().unwrap()
}
