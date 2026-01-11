// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(target_has_atomic = "ptr")] // Arc is not available. TODO: implement using RawWarker
#![warn(missing_docs)]

//! This module contains the code that runs futures

use crate::api::EventLoopError;
use crate::SlintContext;
use alloc::boxed::Box;
use alloc::task::Wake;
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

/// The return value of the `spawn_local()` function
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
    /// Checks if the task associated with this `JoinHandle` has finished.
    pub fn is_finished(&self) -> bool {
        matches!(self.0.inner().fut, FutureState::Finished(_))
    }
}

#[cfg(feature = "std")]
#[allow(unsafe_code)]
// Safety: JoinHandle doesn't access the future, only the
unsafe impl<T: Send> Send for JoinHandle<T> {}

/// Implementation for [`SlintContext::spawn_local`]
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

/// This function spawns a new std::thread and executes the provided closure `action` in that thread.
/// It returns a handle that can be awaited as standard [`Future`](core::future::Future) (hence it's executor-agnostic).
#[cfg(feature = "std")]
pub fn spawn_blocking<T: Send + 'static, F: FnMut() -> T + Send + 'static>(
    mut action: F,
) -> SpawnBlockingJoinHandle<T> {
    let shared_thread_info: std::sync::Arc<std::sync::Mutex<SpawnBlockingThreadInfo<T>>> =
        std::sync::Arc::new(std::sync::Mutex::new(SpawnBlockingThreadInfo::default()));
    let shared_clone = shared_thread_info.clone();
    let join_handle = SpawnBlockingJoinHandle::new(shared_thread_info);

    // Keep the `info` locked to be able to safely update the thread handle
    let mut info = shared_clone.lock().expect("Nobody waiting here");

    let shared_clone = shared_clone.clone();
    let handle = std::thread::spawn(move || {
        let ret = action();
        {
            let mut info =
                shared_clone.lock().expect("Something bad happened in another thread...");

            info.action_result = Some(ret);
            if let Some(waker) = info.waker.take() {
                waker.wake();
            }
        }
    });
    info.handle = Some(handle);
    join_handle
}

/// The return value of the `spawn_blocking()` function
///
/// Can be used to await the thread executing the blocking action.
///
/// This trait implements future. Polling it after it finished or aborted may result in a panic.
#[cfg(feature = "std")]
pub struct SpawnBlockingJoinHandle<T> {
    thread_info: std::sync::Arc<std::sync::Mutex<SpawnBlockingThreadInfo<T>>>,
}

#[cfg(feature = "std")]
impl<T> SpawnBlockingJoinHandle<T> {
    fn new(shared: std::sync::Arc<std::sync::Mutex<SpawnBlockingThreadInfo<T>>>) -> Self {
        Self { thread_info: shared }
    }
}

/// Implementation of an executor-agnostic [`Future`](core::future::Future), that waits until the `JoinHandle` related to the action spawned in another thread terminates, and returns it's result.
#[cfg(feature = "std")]
impl<T> core::future::Future for SpawnBlockingJoinHandle<T> {
    type Output = Result<T, Box<dyn core::any::Any + Send>>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let mut thread_info =
            self.thread_info.lock().expect("Something bad happened in another thread...");

        if thread_info.waker.is_none() {
            // store waker to wake this future later on, from the spawned std::thread
            thread_info.waker = Some(cx.waker().clone());
        }

        // This is done to cover the error-path caused by one of the threads crashing.
        // With this in place a panic will be propagated
        if let Some(handle) = thread_info.handle.take() {
            if handle.is_finished() {
                let thread_result = handle.join();

                // Here we care only to propagate the errors in the thread, the happy path is handled with the `action_result` below
                if let Err(e) = thread_result {
                    return core::task::Poll::Ready(Err(e));
                }
            } else {
                thread_info.handle = Some(handle)
            }
        }
        // Happy path, when the action was executed and terminated smoothly
        if let Some(action_result) = thread_info.action_result.take() {
            return core::task::Poll::Ready(Ok(action_result));
        }
        core::task::Poll::Pending
    }
}

/// Struct holding the information required to be passed between the future-polling and the std::thread
#[cfg(feature = "std")]
struct SpawnBlockingThreadInfo<T> {
    /// Holds the action's result, as soon the action is terminated. This is used to propagate the action's result out of the std::thread into the future's output.
    action_result: Option<T>,
    /// Holds the future's waker, as soon as the future is polled.
    waker: Option<core::task::Waker>,
    /// Holds the std::thread handle, as soon as the std::thread is spawned. This is used to propagate eventual sever errors (like panic) on the action or std::thread
    handle: Option<std::thread::JoinHandle<()>>,
}

#[cfg(feature = "std")]
impl<T> Default for SpawnBlockingThreadInfo<T> {
    fn default() -> Self {
        Self { action_result: None, waker: None, handle: None }
    }
}
