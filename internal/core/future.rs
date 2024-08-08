// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(target_has_atomic = "ptr")] // Arc is not available. TODO: implement using RawWarker
#![warn(missing_docs)]

//! This module contains the code that runs futures

use crate::api::EventLoopError;
use crate::SlintContext;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
use alloc::task::Wake;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::future::Future;
use core::ops::DerefMut;
use core::pin::Pin;
use core::task::Poll;
use portable_atomic as atomic;

enum FutureState<T> {
    Running(Pin<Box<dyn Future<Output = T>>>),
    Finished(Option<T>),
}

struct FutureRunnerInner<T> {
    fut: FutureState<T>,
    wakers: Vec<core::task::Waker>,
}

struct FutureRunner<T> {
    #[cfg(not(feature = "std"))]
    inner: core::cell::RefCell<FutureRunnerInner<T>>,
    #[cfg(feature = "std")]
    inner: std::sync::Mutex<FutureRunnerInner<T>>,
    aborted: atomic::AtomicBool,
    proxy: Box<dyn crate::platform::EventLoopProxy>,
    #[cfg(feature = "std")]
    thread: std::thread::ThreadId,
}

impl<T> FutureRunner<T> {
    fn inner(&self) -> impl DerefMut<Target = FutureRunnerInner<T>> + '_ {
        #[cfg(feature = "std")]
        return self.inner.lock().unwrap();
        #[cfg(not(feature = "std"))]
        return self.inner.borrow_mut();
    }
}

// # Safety:
// The Future might not be Send, but we only poll the future from the main thread.
// (We even assert that)
// We may access the finished value from another thread only if T is Send
// (because JoinHandle only implement Send if T:Send)
#[allow(unsafe_code)]
unsafe impl<T> Send for FutureRunner<T> {}
#[allow(unsafe_code)]
unsafe impl<T> Sync for FutureRunner<T> {}

impl<T: 'static> Wake for FutureRunner<T> {
    fn wake(self: alloc::sync::Arc<Self>) {
        self.clone().proxy.invoke_from_event_loop(Box::new(move || {
            #[cfg(feature = "std")]
            assert_eq!(self.thread, std::thread::current().id(), "the future was moved to a thread despite we checked it was created in the event loop thread");
            let waker = self.clone().into();
            let mut inner = self.inner();
            let mut cx = core::task::Context::from_waker(&waker);
            if let FutureState::Running(fut) = &mut inner.fut {
                if self.aborted.load(atomic::Ordering::Relaxed) {
                    inner.fut = FutureState::Finished(None);
                } else {
                    match fut.as_mut().poll(&mut cx) {
                        Poll::Ready(val) => {
                            inner.fut = FutureState::Finished(Some(val));
                            for w in core::mem::take(&mut inner.wakers) {
                                w.wake();
                            }
                        }
                        Poll::Pending => {}
                    }
                }
            }
        }))
        .expect("No event loop despite we checked");
    }
}

/// The return value of the [`spawn_local()`] function
///
/// Can be used to abort the future, or to get the value from a different thread with `.await`
///
/// This trait implements future. Polling it after it finished or aborted may result in a panic.
pub struct JoinHandle<T>(alloc::sync::Arc<FutureRunner<T>>);

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.0.inner();
        match &mut inner.fut {
            FutureState::Running(_) => {
                let waker = cx.waker();
                if !inner.wakers.iter().any(|w| w.will_wake(waker)) {
                    inner.wakers.push(waker.clone());
                }
                Poll::Pending
            }
            FutureState::Finished(x) => {
                Poll::Ready(x.take().expect("Polling completed or aborted JoinHandle"))
            }
        }
    }
}

impl<T> JoinHandle<T> {
    /// If the future hasn't completed yet, this will make the event loop stop polling the corresponding future and it will be dropped
    ///
    /// Once this handle has been aborted, it can no longer be polled
    pub fn abort(self) {
        self.0.aborted.store(true, atomic::Ordering::Relaxed);
    }
}

#[cfg(feature = "std")]
#[allow(unsafe_code)]
// Safety: JoinHandle doesn't access the future, only the
unsafe impl<T: Send> Send for JoinHandle<T> {}

/// Spawns a [`Future`] to execute in the Slint event loop.
///
/// This function is intended to be invoked only from the main Slint thread that runs the event loop.
/// The event loop must be initialized prior to calling this function.
///
/// For spawning a `Send` future from a different thread, this function should be called from a closure
/// passed to [`invoke_from_event_loop()`](crate::api::invoke_from_event_loop).
///
/// This function is typically called from a UI callback.
///
/// # Example
///
/// ```rust,no_run
/// slint::spawn_local(async move {
///     // your async code goes here
/// }).unwrap();
/// ```
///
/// # Compatibility with Tokio and other runtimes
///
/// The runtime used to execute the future on the main thread is platform-dependent,
/// for instance, it could be the winit event loop. Therefore, futures that assume a specific runtime
/// may not work. This may be an issue if you call `.await` on a future created by another
/// runtime, or pass the future directly to `spawn_local`.
///
/// Futures from the [smol](https://docs.rs/smol/latest/smol/) runtime always hand off their work to
/// separate I/O threads that run in parallel to the Slint event loop.
///
/// The [Tokio](https://docs.rs/tokio/latest/tokio/index.html) runtime is to the following constraints:
///
/// * Tokio futures require entering the context of a global Tokio runtime.
/// * Tokio futures aren't guaranteed to hand off their work to separate threads and may therefore not complete, because
/// the Slint runtime can't drive the Tokio runtime.
/// * Tokio futures require regular yielding to the Tokio runtime for fairness, a constraint that also can't be met by Slint.
/// * Tokio's [current-thread schedule](https://docs.rs/tokio/latest/tokio/runtime/index.html#current-thread-scheduler)
/// cannot be used in Slint main thread, because Slint cannot yield to it.
///
/// To addresse these constraints, use [async_compat](https://docs.rs/async-compat/latest/async_compat/index.html)'s [Compat::new()](https://docs.rs/async-compat/latest/async_compat/struct.Compat.html#method.new)
/// to implicitly allocate a shared, multi-threaded Tokio runtime that will be used for Tokio futures.
///
/// The following little example demonstrates the use of Tokio's [`TcpStream`](https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html) to
/// read from a network socket. The entire future passed to `spawn_local()` is wrapped in `Compat::new()` to make it run:
///
/// ```rust,no_run
/// // A dummy TCP server that once reports "Hello World"
/// # i_slint_backend_testing::init_integration_test_with_mock_time();
/// use std::io::Write;
///
/// let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
/// let local_addr = listener.local_addr().unwrap();
/// let server = std::thread::spawn(move || {
///     let mut stream = listener.incoming().next().unwrap().unwrap();
///     stream.write("Hello World".as_bytes()).unwrap();
/// });
///
/// let slint_future = async move {
///     use tokio::io::AsyncReadExt;
///     let mut stream = tokio::net::TcpStream::connect(local_addr).await.unwrap();
///     let mut data = Vec::new();
///     stream.read_to_end(&mut data).await.unwrap();        
///     assert_eq!(data, "Hello World".as_bytes());
///     slint::quit_event_loop().unwrap();
/// };
///
/// // Wrap the future that includes Tokio futures in async_compat's `Compat` to ensure
/// // presence of a Tokio run-time.
/// slint::spawn_local(async_compat::Compat::new(slint_future)).unwrap();
///
/// slint::run_event_loop_until_quit().unwrap();
///
/// server.join().unwrap();
/// ```
///
/// The use of `#[tokio::main]` is **not recommended**. If it's necessary to use though, wrap the call to enter the Slint
/// event loop  in a call to [`tokio::task::block_in_place`](https://docs.rs/tokio/latest/tokio/task/fn.block_in_place.html):
///
/// ```rust, no_run
/// // Wrap the call to run_event_loop to ensure presence of a Tokio run-time.
/// tokio::task::block_in_place(slint::run_event_loop).unwrap();
/// ```
pub fn spawn_local<F: Future + 'static>(fut: F) -> Result<JoinHandle<F::Output>, EventLoopError> {
    // ensure we are in the backend's thread
    crate::context::GLOBAL_CONTEXT.with(|ctx| {
        let ctx = ctx.get().ok_or(EventLoopError::NoEventLoopProvider)?;
        spawn_local_with_ctx(ctx, fut)
    })
}

/// Implementation for [SlintContext::spawn_locale]
pub(crate) fn spawn_local_with_ctx<F: Future + 'static>(
    ctx: &SlintContext,
    fut: F,
) -> Result<JoinHandle<F::Output>, EventLoopError> {
    let arc = alloc::sync::Arc::new(FutureRunner {
        #[cfg(feature = "std")]
        thread: std::thread::current().id(),
        inner: FutureRunnerInner { fut: FutureState::Running(Box::pin(fut)), wakers: Vec::new() }
            .into(),
        aborted: Default::default(),
        proxy: ctx.event_loop_proxy().ok_or(EventLoopError::NoEventLoopProvider)?,
    });
    arc.wake_by_ref();
    Ok(JoinHandle(arc))
}

#[test]
fn test_spawn_local_from_thread() {
    std::thread::spawn(|| {
        assert_eq!(
            spawn_local(async { panic!("the future shouldn't be run since we're in a thread") })
                .map(drop),
            Err(EventLoopError::NoEventLoopProvider)
        );
    })
    .join()
    .unwrap();
}
