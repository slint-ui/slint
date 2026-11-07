// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(feature = "shared-parley")]
use i_slint_common::sharedfontique;

#[cfg(all(feature = "std", feature = "shared-parley"))]
#[allow(clippy::large_enum_variant)]
enum Pending {
    #[cfg_attr(target_family = "wasm", allow(dead_code))]
    Prefetching(std::thread::JoinHandle<sharedfontique::fontique::Collection>),
    Supplied(sharedfontique::fontique::Collection),
}

#[cfg(all(feature = "std", feature = "shared-parley"))]
crate::thread_local! {
    static PENDING: core::cell::RefCell<Option<Pending>> = const { core::cell::RefCell::new(None) };
}

pub struct PrefetchGuard {
    _not_send: core::marker::PhantomData<*const ()>,
}

impl Drop for PrefetchGuard {
    fn drop(&mut self) {
        #[cfg(all(feature = "std", feature = "shared-parley"))]
        drop(PENDING.take());
    }
}

#[must_use = "the prefetched collection is discarded when the guard is dropped"]
pub fn prefetch() -> Option<PrefetchGuard> {
    #[cfg(all(feature = "std", feature = "shared-parley", not(target_family = "wasm")))]
    if crate::SlintContext::current().is_none() && PENDING.with_borrow(Option::is_none) {
        let worker = std::thread::Builder::new()
            .name("slint-font-collection".into())
            .spawn(|| sharedfontique::system_collection(true))
            .ok()?;
        PENDING.set(Some(Pending::Prefetching(worker)));
        return Some(PrefetchGuard { _not_send: core::marker::PhantomData });
    }
    None
}

#[cfg(all(feature = "std", feature = "shared-parley"))]
pub fn supply(
    collection: sharedfontique::fontique::Collection,
) -> Result<(), crate::api::PlatformError> {
    if crate::SlintContext::current().is_some() {
        return Err(crate::api::PlatformError::SetPlatformError(
            crate::platform::SetPlatformError::AlreadySet,
        ));
    }
    PENDING.set(Some(Pending::Supplied(collection)));
    Ok(())
}

#[cfg(feature = "shared-parley")]
pub(crate) fn take_or_create() -> sharedfontique::Collection {
    #[cfg(feature = "std")]
    match PENDING.take() {
        Some(Pending::Prefetching(worker)) => {
            let collection = worker.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            return sharedfontique::init_collection(collection, true);
        }
        Some(Pending::Supplied(mut collection)) => {
            collection.make_shared();
            return sharedfontique::Collection {
                inner: collection,
                source_cache: sharedfontique::fontique::SourceCache::new_shared(),
                default_fonts: Default::default(),
            };
        }
        None => {}
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
        assert!(super::PENDING.with_borrow(Option::is_none));
    }
}
