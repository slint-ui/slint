// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

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

/// Spawns a Future to execute in the Slint event loop.
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
/// may not work. To overcome this, these futures should be executed in a thread where the specific
/// runtime is running.
///
/// For Tokio, this can be achieved by awaiting [tokio::spawn](https://docs.rs/tokio/latest/tokio/task/fn.spawn.html)
/// in the future passed to slint::spawn_local.
///
/// ```rust
/// # i_slint_backend_testing::init_integration_test();
/// // In your main function, create a runtime that runs on the other threads
/// let tokio_runtime = tokio::runtime::Runtime::new().unwrap();
///
/// // ...
/// // Within the UI thread (for example in a callback handler)
/// slint::spawn_local(async move {
///     let result = tokio_runtime.spawn(async move {
///         // This code is running on the Tokio runtime's thread, you can await futures that depends on Tokio here.
///         42
///     }).await.unwrap();
///     // now we are back on the UI thread so we can do something with the result on the UI thread
///     assert_eq!(result, 42);
///     # slint::quit_event_loop();
/// }).unwrap();
/// # slint::run_event_loop().unwrap();
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
