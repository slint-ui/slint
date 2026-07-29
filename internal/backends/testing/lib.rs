// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]

mod search_api;
pub use search_api::*;
#[cfg(feature = "internal")]
mod internal_tests;
#[cfg(feature = "internal")]
pub use internal_tests::*;
pub mod testing_backend;
pub use testing_backend::get_mocked_time;
// Exported unconditionally so the backend selector can instantiate the
// headless backend.
pub use testing_backend::{TestingBackend, TestingBackendOptions};
#[cfg(feature = "internal")]
pub use testing_backend::{TestingWindow, mock_elapsed_time};
#[cfg(all(feature = "ffi", not(test)))]
mod ffi;
#[cfg(any(feature = "system-testing", feature = "mcp"))]
pub(crate) mod introspection;
#[cfg(feature = "mcp")]
pub mod mcp_server;
#[cfg(feature = "system-testing")]
pub mod systest;

/// Initialize the testing backend without support for event loop.
/// This means that each test thread can use its own backend, but global functions that needs
/// an event loop such as `slint::invoke_from_event_loop` or `Timer`s won't work.
/// Must be called before any call that would otherwise initialize the rendering backend.
/// Calling it when the rendering backend is already initialized will panic.
///
/// Note that for animations and timers, the changes in the system time will be disregarded.
/// Instead, use [`mock_elapsed_time()`] to advance the simulate (mock) time Slint uses.
pub fn init_no_event_loop() {
    i_slint_core::platform::set_platform(Box::new(testing_backend::TestingBackend::new(
        testing_backend::TestingBackendOptions {
            mock_time: true,
            threading: false,
            ..Default::default()
        },
    )))
    .expect("platform already initialized");
}

/// Initialize the testing backend with support for simple event loop.
/// This function can only be called once per process, so make sure to use integration
/// tests with only one `#[test]` function. (Or in a doc test)
/// Must be called before any call that would otherwise initialize the rendering backend.
/// Calling it when the rendering backend is already initialized will panic.
///
/// Note that for animations and timers, the changes in the system time will be disregarded.
/// Instead, use [`mock_elapsed_time()`] to advance the simulate (mock) time Slint uses.
pub fn init_integration_test_with_mock_time() {
    i_slint_core::platform::set_platform(Box::new(testing_backend::TestingBackend::new(
        testing_backend::TestingBackendOptions {
            mock_time: true,
            threading: true,
            ..Default::default()
        },
    )))
    .expect("platform already initialized");
}

/// Initialize the testing backend with support for simple event loop.
/// This function can only be called once per process, so make sure to use integration
/// tests with only one `#[test]` function. (Or in a doc test)
/// Must be called before any call that would otherwise initialize the rendering backend.
/// Calling it when the rendering backend is already initialized will panic.
pub fn init_integration_test_with_system_time() {
    i_slint_core::platform::set_platform(Box::new(testing_backend::TestingBackend::new(
        testing_backend::TestingBackendOptions {
            mock_time: false,
            threading: true,
            ..Default::default()
        },
    )))
    .expect("platform already initialized");
}

/// Advance the simulated mock time by the specified duration. Use in combination with
/// [`init_integration_test_with_mock_time()`] or [`init_no_event_loop()`].
#[cfg(not(feature = "internal"))]
pub fn mock_elapsed_time(duration: std::time::Duration) {
    testing_backend::mock_elapsed_time(duration.as_millis() as _);
}

/// Set the system accent color, as a platform backend would when the OS theme changes.
/// Must be called after initializing the testing backend (e.g. after [`init_no_event_loop()`]).
pub fn set_system_accent_color(color: i_slint_core::Color) {
    i_slint_core::context::with_global_context(
        || panic!("the testing backend must be initialized first"),
        |ctx| ctx.set_accent_color(color),
    )
    .unwrap();
}

/// Replace the font collection with embedded NotoSans fonts for deterministic test results.
/// Must be called after initializing the testing backend (e.g. after [`init_no_event_loop()`]).
#[cfg(feature = "internal")]
pub fn configure_test_fonts() {
    use i_slint_common::sharedfontique::{FALLBACK_FAMILIES, fontique};
    use include_dir::{Dir, include_dir};

    static FONTS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../../tests/screenshots/fonts");
    // Pin the primary by name so its position in the fallback chain doesn't depend on
    // filesystem iteration order. NotoSans-Regular wins as the default upright face.
    const PRIMARY: &str = "NotoSans-Regular.ttf";

    i_slint_core::with_global_context(
        || panic!("platform not set, initialize the testing backend first"),
        |ctx| {
            let mut font_context = ctx.font_context().borrow_mut();
            font_context.collection = fontique::Collection::new(fontique::CollectionOptions {
                shared: true,
                system_fonts: false,
            });
            font_context.source_cache = fontique::SourceCache::new_shared();
            font_context.clear_registered_static_fonts();

            let primary =
                FONTS_DIR.get_file(PRIMARY).expect("primary test font missing from fonts dir");
            let mut fallback_files: Vec<_> = FONTS_DIR
                .files()
                .filter(|f| f.path().extension().is_some_and(|ext| ext == "ttf"))
                .filter(|f| f.path().file_name().and_then(|n| n.to_str()) != Some(PRIMARY))
                .collect();
            // Sort fallbacks lexicographically so the chain is reproducible across platforms.
            fallback_files.sort_by_key(|f| f.path().to_owned());

            let mut chain_families: Vec<fontique::FamilyId> = Vec::new();
            for file in core::iter::once(primary).chain(fallback_files) {
                let fonts = font_context.collection.register_fonts(
                    fontique::Blob::new(std::sync::Arc::new(file.contents())),
                    None,
                );
                for (family_id, _) in &fonts {
                    if !chain_families.contains(family_id) {
                        chain_families.push(*family_id);
                    }
                }
            }
            // Map the fallback generics plus monospace (used by markdown code spans) to the bundled
            // fonts, so all generic families resolve deterministically with system fonts disabled.
            for generic_family in
                FALLBACK_FAMILIES.into_iter().chain([fontique::GenericFamily::Monospace])
            {
                font_context
                    .collection
                    .set_generic_families(generic_family, chain_families.iter().copied());
            }
        },
    )
    .unwrap();
}

pub use i_slint_core::items::{AccessibleLiveness, AccessibleRole, Orientation};
