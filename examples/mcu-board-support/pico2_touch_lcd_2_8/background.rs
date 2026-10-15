// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use super::background_job::Queue;
use core::task::Poll;
use embassy_sync::waitqueue::AtomicWaker;

static JOB: Queue = Queue::new();
static WAKER: AtomicWaker = AtomicWaker::new();

fn wake() {
    WAKER.wake();
    cortex_m::asm::sev();
}

/// Run bounded work on core 1 while the caller renders independently.
/// Each job call must yield promptly so display transfers keep their scan deadlines.
/// The job finishes before this function returns, including during unwinding.
pub fn with_core1<F: FnMut() -> bool + Send, R>(job: &mut F, local: impl FnOnce() -> R) -> R {
    JOB.scope(job, local, cortex_m::asm::wfe, wake)
}

#[embassy_executor::task]
pub async fn task() {
    core::future::poll_fn(|context| {
        WAKER.register(context.waker());
        if JOB.step() {
            context.waker().wake_by_ref();
        }
        Poll::<()>::Pending
    })
    .await;
}
