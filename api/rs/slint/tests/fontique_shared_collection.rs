// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore donotexist

#![cfg(feature = "unstable-fontique-011")]

use slint::fontique_011::fontique;

// The wasm gallery downloads a CJK font and registers it as a fallback before the first
// component is created, so `shared_collection()` must work without an initialized
// platform.
#[test]
fn register_fonts_before_platform_init() {
    // shared_collection creates the default backend on demand; pick the testing
    // backend so this also works without a display.
    unsafe { std::env::set_var("SLINT_BACKEND", "testing") };

    let font_data = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../internal/common/sharedfontique/Inter-VariableFont.ttf"
    ))
    .unwrap();

    let blob = fontique::Blob::new(std::sync::Arc::new(font_data));
    let mut collection = slint::fontique_011::shared_collection();
    let fonts = collection.register_fonts(blob, None);

    let (family_id, font_infos) = fonts.first().expect("no font was registered");
    assert!(!font_infos.is_empty());
    assert_eq!(collection.family_name(*family_id), Some("Inter"));

    let script = fontique::Script::from_str_unchecked("Hani");
    collection
        .append_fallbacks(fontique::FallbackKey::new(script, None), fonts.iter().map(|x| x.0));
    assert!(
        collection
            .fallback_families(fontique::FallbackKey::new(script, None))
            .any(|id| id == *family_id),
        "the registered font is not in the fallback chain"
    );

    // Data that isn't a font must not register anything.
    let invalid = fontique::Blob::new(std::sync::Arc::new(b"donotexist.ttf".to_vec()));
    assert!(collection.register_fonts(invalid, None).is_empty());
}
