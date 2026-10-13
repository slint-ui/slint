// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(all(feature = "std", feature = "shared-parley"))]

use std::rc::Rc;

use i_slint_common::sharedfontique::{self, fontique};
use i_slint_core::graphics::FontRequest;
use i_slint_core::platform::{Platform, PlatformError, WindowAdapter};
use i_slint_core::{InternalToken, SlintContext, SlintContextWeak};

const REGISTERED_FONT: &[u8] =
    include_bytes!("../../../tests/screenshots/fonts/NotoSans-Regular.ttf");

struct TestPlatform {
    on_bind: Option<fn(&SlintContext)>,
}

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        unreachable!()
    }

    fn bind_context(&self, context: SlintContextWeak, _: InternalToken) {
        if let Some(on_bind) = self.on_bind {
            on_bind(&context.upgrade().unwrap());
        }
    }
}

fn on_new_thread<R: Send + 'static>(f: fn() -> R) -> R {
    std::thread::spawn(f).join().unwrap()
}

fn query_font(
    context: &SlintContext,
    family: fontique::QueryFamily<'_>,
    fallback: Option<fontique::FallbackKey>,
    character: char,
) -> Option<fontique::QueryFont> {
    let mut context = context.font_context().borrow_mut();
    let context = &mut context.inner;
    let mut query = context.collection.query(&mut context.source_cache);
    query.set_families([family]);
    if let Some(fallback) = fallback {
        query.set_fallbacks(fallback);
    }
    let mut result = None;
    query.matches_with(|font| {
        if font.charmap().and_then(|charmap| charmap.map(character)).is_some() {
            result = Some(font.clone());
            fontique::QueryStatus::Stop
        } else {
            fontique::QueryStatus::Continue
        }
    });
    result
}

type FontSignature = Option<(String, u32)>;

fn font_signature(context: &SlintContext, font: Option<fontique::QueryFont>) -> FontSignature {
    font.map(|font| {
        let family = context
            .font_context()
            .borrow_mut()
            .collection
            .family_name(font.family.0)
            .unwrap()
            .to_owned();
        (family, font.index)
    })
}

#[derive(Debug, PartialEq)]
struct SystemSelection {
    families: Vec<String>,
    default_font: FontSignature,
    fallbacks: Vec<FontSignature>,
}

fn system_selection(context: &SlintContext) -> SystemSelection {
    let mut families: Vec<_> =
        context.font_context().borrow_mut().collection.family_names().map(str::to_owned).collect();
    families.sort();

    let default_font = {
        let mut font_context = context.font_context().borrow_mut();
        let font_context = &mut font_context.inner;
        FontRequest::default()
            .query_fontique(&mut font_context.collection, &mut font_context.source_cache)
    };

    let mut fallbacks = Vec::new();
    for (script, character) in [("Latn", 'A'), ("Arab", 'ا'), ("Hani", '中')] {
        let key = fontique::FallbackKey::new(script.parse::<fontique::Script>().unwrap(), None);
        for family in [
            fontique::QueryFamily::Named("Slint Missing Font"),
            fontique::QueryFamily::Generic(sharedfontique::FALLBACK_FAMILIES[0]),
        ] {
            fallbacks
                .push(font_signature(context, query_font(context, family, Some(key), character)));
        }
    }

    SystemSelection { families, default_font: font_signature(context, default_font), fallbacks }
}

#[test]
#[cfg_attr(miri, ignore)]
fn prefetched_collection_matches_direct_set_platform() {
    let prefetched = on_new_thread(|| {
        let prefetch = i_slint_core::font_collection::prefetch();
        assert!(prefetch.is_some());
        i_slint_core::platform::set_platform(Box::new(TestPlatform { on_bind: None })).unwrap();
        system_selection(&SlintContext::current().unwrap())
    });
    let direct = on_new_thread(|| {
        i_slint_core::platform::set_platform(Box::new(TestPlatform { on_bind: None })).unwrap();
        system_selection(&SlintContext::current().unwrap())
    });
    assert_eq!(prefetched, direct);
}

#[test]
#[cfg_attr(miri, ignore)]
fn fonts_registered_in_bind_context_stay_registered() {
    on_new_thread(|| {
        let _prefetch = i_slint_core::font_collection::prefetch();
        i_slint_core::platform::set_platform(Box::new(TestPlatform {
            on_bind: Some(|context| {
                context.font_context().borrow_mut().register_static_font(REGISTERED_FONT);
            }),
        }))
        .unwrap();
        let context = SlintContext::current().unwrap();
        let registered = query_font(&context, "Noto Sans".into(), None, 'A').unwrap();
        assert_eq!(registered.blob.as_ref(), REGISTERED_FONT);
    });
}
