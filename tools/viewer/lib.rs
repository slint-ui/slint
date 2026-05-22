// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(feature = "remote")]
pub mod remote;

/// Poll a future that is expected to resolve immediately (e.g. the interpreter's
/// `build_from_path` when no async file loader is installed).
pub fn poll_ready<F: std::future::Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::future::Future::poll(future.as_mut(), &mut cx) {
        std::task::Poll::Ready(result) => result,
        std::task::Poll::Pending => unreachable!("Compiler returned Pending"),
    }
}

#[cfg(all(target_os = "android", feature = "remote"))]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    slint::android::init(app).unwrap();
    remote::run(None, true).unwrap();
}
