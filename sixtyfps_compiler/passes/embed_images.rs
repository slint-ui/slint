// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#![cfg_attr(target_arch = "wasm32", allow(unused))]

use crate::diagnostics::BuildDiagnostics;
use crate::embedded_resources::*;
use crate::expression_tree::{Expression, ImageReference};
use crate::object_tree::*;
#[cfg(not(target_arch = "wasm32"))]
use image::GenericImageView;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryInto;
use std::rc::Rc;

pub fn embed_images(
    component: &Rc<Component>,
    embed_files_by_default: bool,
    diag: &mut BuildDiagnostics,
) {
    let global_embedded_resources = &component.embedded_file_resources;

    for component in
        component.used_types.borrow().sub_components.iter().chain(std::iter::once(component))
    {
        visit_all_expressions(component, |e, _| {
            embed_images_from_expression(e, global_embedded_resources, embed_files_by_default, diag)
        });
    }
}

fn embed_images_from_expression(
    e: &mut Expression,
    global_embedded_resources: &RefCell<HashMap<String, EmbeddedResources>>,
    embed_files_by_default: bool,
    diag: &mut BuildDiagnostics,
) {
    if let Expression::ImageReference { ref mut resource_ref, source_location } = e {
        match resource_ref {
            ImageReference::AbsolutePath(path)
                if embed_files_by_default || path.starts_with("builtin:/") =>
            {
                *resource_ref = embed_image(global_embedded_resources, path, diag, source_location);
            }
            _ => {}
        }
    };

    e.visit_mut(|e| {
        embed_images_from_expression(e, global_embedded_resources, embed_files_by_default, diag)
    });
}

fn embed_image(
    global_embedded_resources: &RefCell<HashMap<String, EmbeddedResources>>,
    path: &str,
    diag: &mut BuildDiagnostics,
    source_location: &Option<crate::diagnostics::SourceLocation>,
) -> ImageReference {
    let mut resources = global_embedded_resources.borrow_mut();
    let maybe_id = resources.len();
    let e = match resources.entry(path.into()) {
        std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
        std::collections::hash_map::Entry::Vacant(e) => {
            // Check that the file exists, so that later we can unwrap safely in the generators, etc.
            if let Some(file) = crate::fileaccess::load_file(std::path::Path::new(path)) {
                let mut kind = EmbeddedResourcesKind::RawData;
                #[cfg(not(target_arch = "wasm32"))]
                if std::env::var("SIXTYFPS_PROCESS_IMAGES").is_ok() {
                    match load_image(file) {
                        Ok(img) => kind = EmbeddedResourcesKind::TextureData(generate_texture(img)),
                        Err(err) => {
                            diag.push_error(
                                format!("Cannot load image file {}: {}", path, err),
                                source_location,
                            );
                            return ImageReference::None;
                        }
                    }
                }
                e.insert(EmbeddedResources { id: maybe_id, kind })
            } else {
                diag.push_warning(format!("Cannot find image file {}", path), source_location);
                return ImageReference::None;
            }
        }
    };
    let is_texture = matches!(e.kind, EmbeddedResourcesKind::TextureData { .. });
    if is_texture {
        ImageReference::EmbeddedTexture { resource_id: e.id }
    } else {
        ImageReference::EmbeddedData {
            resource_id: e.id,
            extension: std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .map(|x| x.to_string())
                .unwrap_or_default(),
        }
    }
}

trait Pixel {
    //fn alpha(&self) -> f32;
    //fn rgb(&self) -> (u8, u8, u8);
    fn is_transparent(&self) -> bool;
}
#[cfg(not(target_arch = "wasm32"))]
impl Pixel for image::Rgba<u8> {
    /*fn alpha(&self) -> f32 { self[3] as f32 / 255. }
    fn rgb(&self) -> (u8, u8, u8) { (self[0], self[1], self[2]) }*/
    fn is_transparent(&self) -> bool {
        self[3] <= 1
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn generate_texture(image: image::RgbaImage) -> Texture {
    // Analyze each pixels
    let mut top = 0;
    let is_line_transparent = |y| {
        for x in 0..image.width() {
            if !image.get_pixel(x, y).is_transparent() {
                return false;
            }
        }
        true
    };
    while top < image.height() && is_line_transparent(top) {
        top += 1;
    }
    if top == image.height() {
        return Texture::new_empty();
    }
    let mut bottom = image.height() - 1;
    while is_line_transparent(bottom) {
        bottom -= 1;
        assert!(bottom > top); // otherwise we would have a transparent image
    }
    let is_column_transparent = |x| {
        for y in top..=bottom {
            if !image.get_pixel(x, y).is_transparent() {
                return false;
            }
        }
        true
    };
    let mut left = 0;
    while is_column_transparent(left) {
        left += 1;
        assert!(left < image.width()); // otherwise we would have a transparent image
    }
    let mut right = image.width() - 1;
    while is_column_transparent(right) {
        right -= 1;
        assert!(right > left); // otherwise we would have a transparent image
    }
    let mut is_opaque = true;
    enum ColorState {
        Unset,
        Different,
        RGB([u8; 3]),
    }
    let mut color = ColorState::Unset;
    'outer: for y in top..=bottom {
        for x in left..=right {
            let p = image.get_pixel(x, y);
            let alpha = p[3];
            if alpha != 255 {
                is_opaque = false;
            }
            match color {
                ColorState::Unset => {
                    color = ColorState::RGB(p.0[0..3].try_into().unwrap());
                }
                ColorState::Different => {
                    if !is_opaque {
                        break 'outer;
                    }
                }
                ColorState::RGB([a, b, c]) => {
                    let abs_diff = |t, u| {
                        if t < u {
                            u - t
                        } else {
                            t - u
                        }
                    };
                    if abs_diff(a, p[0]) > 2 || abs_diff(b, p[1]) > 2 || abs_diff(c, p[2]) > 2 {
                        color = ColorState::Different
                    }
                }
            }
        }
    }

    let format = if let ColorState::RGB(c) = color {
        PixelFormat::AlphaMap(c)
    } else if is_opaque {
        PixelFormat::Rgb
    } else {
        PixelFormat::Rgba
    };

    let rect = Rect::from_ltrb(left as _, top as _, (right + 1) as _, (bottom + 1) as _).unwrap();
    Texture {
        total_size: Size { width: image.width(), height: image.height() },
        rect,
        data: convert_image(image, format, rect),
        format,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn convert_image(image: image::RgbaImage, format: PixelFormat, rect: Rect) -> Vec<u8> {
    let i = image::SubImage::new(&image, rect.x() as _, rect.y() as _, rect.width(), rect.height());
    match format {
        PixelFormat::Rgb => {
            i.pixels().flat_map(|(_, _, p)| IntoIterator::into_iter(p.0).take(3)).collect()
        }
        PixelFormat::Rgba => {
            i.pixels().flat_map(|(_, _, p)| IntoIterator::into_iter(p.0)).collect()
        }
        PixelFormat::AlphaMap(_) => i.pixels().map(|(_, _, p)| p[3]).collect(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_image(file: crate::fileaccess::VirtualFile) -> image::ImageResult<image::RgbaImage> {
    if file.path.ends_with(".svg") || file.path.ends_with(".svgz") {
        let options = usvg::Options::default();
        let tree = match file.builtin_contents {
            Some(data) => usvg::Tree::from_data(data, &options.to_ref()),
            None => usvg::Tree::from_data(
                std::fs::read(file.path.as_ref()).map_err(image::ImageError::IoError)?.as_slice(),
                &options.to_ref(),
            ),
        }
        .map_err(|e| {
            image::ImageError::Decoding(image::error::DecodingError::new(
                image::error::ImageFormatHint::Name("svg".into()),
                e,
            ))
        })?;
        // TODO: ideally we should find the size used for that `Image`
        let size = tree.svg_node().size;
        let mut buffer = vec![0u8; size.width() as usize * size.height() as usize * 4];
        let size_error = || {
            image::ImageError::Limits(image::error::LimitError::from_kind(
                image::error::LimitErrorKind::DimensionError,
            ))
        };
        let skia_buffer = tiny_skia::PixmapMut::from_bytes(
            buffer.as_mut_slice(),
            size.width() as u32,
            size.height() as u32,
        )
        .ok_or_else(size_error)?;
        resvg::render(&tree, usvg::FitTo::Original, skia_buffer).ok_or_else(size_error)?;
        return image::RgbaImage::from_raw(size.width() as u32, size.height() as u32, buffer)
            .ok_or_else(size_error);
    }
    if let Some(buffer) = file.builtin_contents {
        image::load_from_memory(buffer)
    } else {
        image::open(file.path.as_ref())
    }
    .map(|i| i.to_rgba8())
}
