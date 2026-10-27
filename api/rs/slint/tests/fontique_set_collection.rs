// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(feature = "unstable-fontique-012")]

use slint::fontique_012::{fontique, set_shared_collection, shared_collection};

slint::slint! {
    #[slint_edition = "1.19"]
    import "../../../internal/common/sharedfontique/Inter-VariableFont.ttf";

    export component WithFont inherits Window {
        Text {
            font-family: "Inter";
            text: "Hello";
        }
    }
}

#[test]
fn supplied_collection_replaces_system_fonts_and_accepts_embedded_fonts() {
    let collection = fontique::Collection::new(fontique::CollectionOptions {
        shared: false,
        system_fonts: false,
    });
    set_shared_collection(collection).unwrap();
    i_slint_core::with_global_context(
        || Ok(Box::new(i_slint_backend_testing::TestingBackend::new(Default::default()))),
        |_| (),
    )
    .unwrap();
    let mut collection = shared_collection();
    assert_eq!(collection.family_names().count(), 0);

    let _window = WithFont::new().unwrap();
    let family_id = collection.family_id("Inter").expect("embedded font was not registered");

    let replacement = fontique::Collection::new(fontique::CollectionOptions {
        shared: true,
        system_fonts: false,
    });
    assert!(set_shared_collection(replacement).is_err());
    assert_eq!(shared_collection().family_id("Inter"), Some(family_id));
}

#[test]
fn supplied_collection_is_used_with_a_custom_platform() {
    let mut collection = fontique::Collection::new(fontique::CollectionOptions {
        shared: true,
        system_fonts: false,
    });
    let fonts = collection.register_fonts(
        fontique::Blob::new(std::sync::Arc::new(include_bytes!(
            "../../../../internal/common/sharedfontique/Inter-VariableFont.ttf"
        ) as &'static [u8])),
        None,
    );
    let family_id = fonts[0].0;
    collection.set_generic_families(fontique::GenericFamily::SansSerif, [family_id].into_iter());
    set_shared_collection(collection.clone()).unwrap();
    i_slint_backend_testing::init_no_event_loop();
    let mut active_collection = shared_collection();
    assert_eq!(active_collection.family_id("Inter"), Some(family_id));
    assert_eq!(
        active_collection.generic_families(fontique::GenericFamily::SansSerif).collect::<Vec<_>>(),
        vec![family_id],
    );

    let fonts = collection.register_fonts(
        fontique::Blob::new(std::sync::Arc::new(include_bytes!(
            "../../../../tests/screenshots/fonts/NotoSans-Regular.ttf"
        ) as &'static [u8])),
        None,
    );
    assert_eq!(active_collection.family_id("Noto Sans"), Some(fonts[0].0));
}
