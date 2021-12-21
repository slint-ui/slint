// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#![cfg(feature = "svg")]

fn with_svg_options<T>(callback: impl FnOnce(usvg::OptionsRef<'_>) -> T) -> T {
    crate::fonts::FONT_CACHE.with(|cache| {
        let options = usvg::Options::default();
        let mut options_ref = options.to_ref();
        let cache = cache.borrow();
        options_ref.fontdb = &cache.available_fonts;
        callback(options_ref)
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_from_path(path: &std::path::Path) -> Result<usvg::Tree, std::io::Error> {
    let svg_data = std::fs::read(path)?;

    with_svg_options(|options| {
        usvg::Tree::from_data(&svg_data, &options)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    })
}

pub fn load_from_data(slice: &[u8]) -> Result<usvg::Tree, usvg::Error> {
    with_svg_options(|options| usvg::Tree::from_data(slice, &options))
}

pub fn render(
    tree: &usvg::Tree,
    size: euclid::default::Size2D<u32>,
) -> Result<image::DynamicImage, usvg::Error> {
    // resvg doesn't support scaling to width/height, just fit to width.
    // FIXME: the fit should actually depends on the image-fit property
    let fit = usvg::FitTo::Width(size.width);
    let size = fit.fit_to(tree.svg_node().size.to_screen_size()).ok_or(usvg::Error::InvalidSize)?;
    let mut buffer =
        vec![0u8; size.width() as usize * size.height() as usize * tiny_skia::BYTES_PER_PIXEL];
    let skia_buffer =
        tiny_skia::PixmapMut::from_bytes(buffer.as_mut_slice(), size.width(), size.height())
            .ok_or(usvg::Error::InvalidSize)?;
    resvg::render(tree, fit, skia_buffer).ok_or(usvg::Error::InvalidSize)?;
    Ok(image::DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(size.width(), size.height(), buffer)
            .ok_or(usvg::Error::InvalidSize)?,
    ))
}
