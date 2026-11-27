// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! An `Image` whose source changes to another file draws the new file.
//! The software renderer keeps the decoded pixels of each image item,
//! and must decode again when the source changes.

mod common;

use slint::platform::software_renderer::RepaintBufferType;
use slint::{ComponentHandle, Image, Rgb8Pixel};

slint::slint! {
    #[edition = "1.20"]
    export component Scene inherits Window {
        in property <image> picture;
        Image { source: root.picture; width: 100%; height: 100%; }
    }
}

#[test]
fn image_shows_its_new_source() {
    let window = common::setup(1, 1);
    let scene = Scene::new().unwrap();
    scene.show().unwrap();

    let testdata = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/image_source_change");
    for (file, color) in [
        ("red.png", Rgb8Pixel { r: 255, g: 0, b: 0 }),
        ("green.png", Rgb8Pixel { r: 0, g: 255, b: 0 }),
    ] {
        scene.set_picture(
            Image::load_from_path(&std::path::Path::new(testdata).join(file)).unwrap(),
        );
        let mut pixel = [Rgb8Pixel::default()];
        assert!(window.draw_if_needed(|renderer| {
            renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
            renderer.render(&mut pixel, 1);
        }));
        assert_eq!(pixel[0], color, "{file}");
    }
}
