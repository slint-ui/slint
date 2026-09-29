// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(feature = "shared-parley")]
use i_slint_common::sharedfontique;

#[cfg(all(feature = "std", feature = "shared-parley"))]
crate::thread_local! {
    static PREFETCHED: core::cell::RefCell<
        Option<std::thread::JoinHandle<sharedfontique::fontique::Collection>>,
    > = const { core::cell::RefCell::new(None) };
}

pub struct PrefetchGuard {
    _not_send: core::marker::PhantomData<*const ()>,
}

impl Drop for PrefetchGuard {
    fn drop(&mut self) {
        #[cfg(all(feature = "std", feature = "shared-parley"))]
        drop(PREFETCHED.take());
    }
}

#[must_use = "the prefetched collection is discarded when the guard is dropped"]
pub fn prefetch() -> Option<PrefetchGuard> {
    #[cfg(all(feature = "std", feature = "shared-parley", not(target_family = "wasm")))]
    if crate::SlintContext::current().is_none() && PREFETCHED.with_borrow(Option::is_none) {
        let worker = std::thread::Builder::new()
            .name("slint-font-collection".into())
            .spawn(|| sharedfontique::system_collection(true))
            .ok()?;
        PREFETCHED.set(Some(worker));
        return Some(PrefetchGuard { _not_send: core::marker::PhantomData });
    }
    None
}

#[cfg(feature = "shared-parley")]
pub(crate) fn take_or_create() -> sharedfontique::Collection {
    #[cfg(feature = "std")]
    if let Some(worker) = PREFETCHED.take() {
        let collection = worker.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic));
        return sharedfontique::init_collection(collection, true);
    }
    sharedfontique::create_collection(true)
}

#[cfg(all(test, feature = "std", feature = "shared-parley", not(target_family = "wasm")))]
mod tests {
    #[test]
    #[cfg_attr(miri, ignore)]
    fn dropping_the_guard_discards_the_collection() {
        let guard = super::prefetch();
        assert!(guard.is_some());
        drop(guard);
        assert!(super::PREFETCHED.with_borrow(Option::is_none));
    }
}
