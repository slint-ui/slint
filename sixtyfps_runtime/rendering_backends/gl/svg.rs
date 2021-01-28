/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#![cfg(feature = "svg")]

#[cfg(not(target_arch = "wasm32"))]
pub fn load_from_path(path: &std::path::Path) -> Result<image::DynamicImage, usvg::Error> {
    load_image(usvg::Tree::from_file(path, &Default::default())?)
}

pub fn load_from_data(slice: &[u8]) -> Result<image::DynamicImage, usvg::Error> {
    load_image(usvg::Tree::from_data(slice, &Default::default())?)
}

fn load_image(tree: usvg::Tree) -> Result<image::DynamicImage, usvg::Error> {
    // FIXME: get the size from the actual image
    let size = tree.svg_node().size.to_screen_size();
    //let mut result = image::DynamicImage::new_rgba8(size.width(), size.height());
    let mut buffer =
        vec![0u8; size.width() as usize * size.height() as usize * tiny_skia::BYTES_PER_PIXEL];
    let skya_buffer =
        tiny_skia::PixmapMut::from_bytes(buffer.as_mut_slice(), size.width(), size.height())
            .ok_or(usvg::Error::InvalidSize)?;
    resvg::render(&tree, usvg::FitTo::Original, skya_buffer);
    Ok(image::DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(size.width(), size.height(), buffer)
            .ok_or(usvg::Error::InvalidSize)?,
    ))
}
