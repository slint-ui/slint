// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(feature = "shared-parley")]
use crate::textlayout::sharedparley::FontContext;
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
pub(crate) struct LazyFontContext {
    font_context: core::cell::OnceCell<core::cell::RefCell<FontContext>>,
    #[cfg(feature = "std")]
    prefetched:
        core::cell::Cell<Option<std::thread::JoinHandle<sharedfontique::fontique::Collection>>>,
}

#[cfg(feature = "shared-parley")]
impl LazyFontContext {
    pub(crate) fn new() -> Self {
        Self {
            font_context: Default::default(),
            #[cfg(feature = "std")]
            prefetched: core::cell::Cell::new(PREFETCHED.take()),
        }
    }

    pub(crate) fn get(&self) -> &core::cell::RefCell<FontContext> {
        self.font_context.get_or_init(|| {
            #[cfg(feature = "std")]
            if let Some(worker) = self.prefetched.take() {
                let collection =
                    worker.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic));
                return font_context(sharedfontique::init_collection(collection, true));
            }
            font_context(sharedfontique::create_collection(true))
        })
    }

    pub(crate) fn set_collection(
        &self,
        mut collection: sharedfontique::fontique::Collection,
    ) -> Result<(), crate::api::PlatformError> {
        if self.font_context.get().is_some() {
            return Err(crate::api::PlatformError::Other(
                "The font collection is already in use".into(),
            ));
        }
        collection.make_shared();
        let font_context = core::cell::RefCell::new(FontContext::new(parley::FontContext {
            collection,
            source_cache: sharedfontique::fontique::SourceCache::new_shared(),
        }));
        let _ = self.font_context.set(font_context);
        #[cfg(feature = "std")]
        drop(self.prefetched.take());
        Ok(())
    }
}

#[cfg(feature = "shared-parley")]
fn font_context(collection: sharedfontique::Collection) -> core::cell::RefCell<FontContext> {
    core::cell::RefCell::new(FontContext::new(parley::FontContext {
        collection: collection.inner,
        source_cache: collection.source_cache,
    }))
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
