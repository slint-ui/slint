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
pub fn load_from_path(path: &std::path::Path) -> Result<usvg::Tree, usvg::Error> {
    usvg::Tree::from_file(path, &Default::default())
}

pub fn load_from_data(slice: &[u8]) -> Result<usvg::Tree, usvg::Error> {
    usvg::Tree::from_data(slice, &Default::default())
}

pub fn render(
    tree: &usvg::Tree,
    size: euclid::default::Size2D<u32>,
) -> Result<image::DynamicImage, usvg::Error> {
    let mut buffer =
        vec![0u8; size.width as usize * size.height as usize * tiny_skia::BYTES_PER_PIXEL];
    let skya_buffer =
        tiny_skia::PixmapMut::from_bytes(buffer.as_mut_slice(), size.width, size.height)
            .ok_or(usvg::Error::InvalidSize)?;
    // FIXME: resvg doesn't support scaling to width/height, just fit to width.
    resvg::render(&tree, usvg::FitTo::Width(size.width), skya_buffer);
    Ok(image::DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(size.width, size.height, buffer)
            .ok_or(usvg::Error::InvalidSize)?,
    ))
}
