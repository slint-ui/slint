// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::diagnostics::BuildDiagnostics;
use crate::embedded_resources::*;
use crate::expression_tree::{Expression, ImageReference};
use crate::object_tree::*;
use crate::EmbedResourcesKind;
#[cfg(feature = "software-renderer")]
use image::GenericImageView;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

pub async fn embed_images(
    doc: &Document,
    embed_files: EmbedResourcesKind,
    scale_factor: f64,
    resource_url_mapper: &Option<Rc<dyn Fn(&str) -> Pin<Box<dyn Future<Output = Option<String>>>>>>,
    diag: &mut BuildDiagnostics,
) {
    if embed_files == EmbedResourcesKind::Nothing && resource_url_mapper.is_none() {
        return;
    }

    let global_embedded_resources = &doc.embedded_file_resources;

    let mut all_components = Vec::new();
    doc.visit_all_used_components(|c| all_components.push(c.clone()));
    let all_components = all_components;

    let mapped_urls = {
        let mut urls = HashMap::<SmolStr, Option<SmolStr>>::new();

        if let Some(mapper) = resource_url_mapper {
            // Collect URLs (sync!):
            for component in &all_components {
                visit_all_expressions(component, |e, _| {
                    collect_image_urls_from_expression(e, &mut urls)
                });
            }

            // Map URLs (async -- well, not really):
            for i in urls.iter_mut() {
                *i.1 = (*mapper)(i.0).await.map(SmolStr::new);
            }
        }

        urls
    };

    // Use URLs (sync!):
    for component in &all_components {
        visit_all_expressions(component, |e, _| {
            embed_images_from_expression(
                e,
                &mapped_urls,
                global_embedded_resources,
                embed_files,
                scale_factor,
                diag,
            )
        });
    }
}

fn collect_image_urls_from_expression(
    e: &Expression,
    urls: &mut HashMap<SmolStr, Option<SmolStr>>,
) {
    if let Expression::ImageReference { ref resource_ref, .. } = e {
        if let ImageReference::AbsolutePath(path) = resource_ref {
            urls.insert(path.clone(), None);
        }
    };

    e.visit(|e| collect_image_urls_from_expression(e, urls));
}

fn embed_images_from_expression(
    e: &mut Expression,
    urls: &HashMap<SmolStr, Option<SmolStr>>,
    global_embedded_resources: &RefCell<BTreeMap<SmolStr, EmbeddedResources>>,
    embed_files: EmbedResourcesKind,
    scale_factor: f64,
    diag: &mut BuildDiagnostics,
) {
    if let Expression::ImageReference { ref mut resource_ref, source_location, nine_slice: _ } = e {
        if let ImageReference::AbsolutePath(path) = resource_ref {
            // used mapped path:
            let mapped_path =
                urls.get(path).unwrap_or(&Some(path.clone())).clone().unwrap_or(path.clone());
            *path = mapped_path;
            if embed_files != EmbedResourcesKind::Nothing
                && (embed_files != EmbedResourcesKind::OnlyBuiltinResources
                    || path.starts_with("builtin:/"))
            {
                let image_ref = embed_image(
                    global_embedded_resources,
                    embed_files,
                    path,
                    scale_factor,
                    diag,
                    source_location,
                );
                if embed_files != EmbedResourcesKind::ListAllResources {
                    *resource_ref = image_ref;
                }
            }
        }
    };

    e.visit_mut(|e| {
        embed_images_from_expression(
            e,
            urls,
            global_embedded_resources,
            embed_files,
            scale_factor,
            diag,
        )
    });
}

fn embed_image(
    global_embedded_resources: &RefCell<BTreeMap<SmolStr, EmbeddedResources>>,
    embed_files: EmbedResourcesKind,
    path: &str,
    _scale_factor: f64,
    diag: &mut BuildDiagnostics,
    source_location: &Option<crate::diagnostics::SourceLocation>,
) -> ImageReference {
    let mut resources = global_embedded_resources.borrow_mut();
    let maybe_id = resources.len();
    let e = match resources.entry(path.into()) {
        std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
        std::collections::btree_map::Entry::Vacant(e) => {
            // Check that the file exists, so that later we can unwrap safely in the generators, etc.
            if embed_files == EmbedResourcesKind::ListAllResources {
                // Really do nothing with the image!
                e.insert(EmbeddedResources { id: maybe_id, kind: EmbeddedResourcesKind::ListOnly });
                return ImageReference::None;
            } else if let Some(_file) = crate::fileaccess::load_file(std::path::Path::new(path)) {
                #[allow(unused_mut)]
                let mut kind = EmbeddedResourcesKind::RawData;
                #[cfg(feature = "software-renderer")]
                if embed_files == EmbedResourcesKind::EmbedTextures {
                    match load_image(_file, _scale_factor) {
                        Ok((img, source_format, original_size)) => {
                            kind = EmbeddedResourcesKind::TextureData(generate_texture(
                                img,
                                source_format,
                                original_size,
                            ))
                        }
                        Err(err) => {
                            diag.push_error(
                                format!("Cannot load image file {path}: {err}"),
                                source_location,
                            );
                            return ImageReference::None;
                        }
                    }
                }
                e.insert(EmbeddedResources { id: maybe_id, kind })
            } else {
                diag.push_error(format!("Cannot find image file {path}"), source_location);
                return ImageReference::None;
            }
        }
    };

    match e.kind {
        #[cfg(feature = "software-renderer")]
        EmbeddedResourcesKind::TextureData { .. } => {
            ImageReference::EmbeddedTexture { resource_id: e.id }
        }
        _ => ImageReference::EmbeddedData {
            resource_id: e.id,
            extension: std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .map(|x| x.to_string())
                .unwrap_or_default(),
        },
    }
}

#[cfg(feature = "software-renderer")]
trait Pixel {
    //fn alpha(&self) -> f32;
    //fn rgb(&self) -> (u8, u8, u8);
    fn is_transparent(&self) -> bool;
}
#[cfg(feature = "software-renderer")]
impl Pixel for image::Rgba<u8> {
    /*fn alpha(&self) -> f32 { self[3] as f32 / 255. }
    fn rgb(&self) -> (u8, u8, u8) { (self[0], self[1], self[2]) }*/
    fn is_transparent(&self) -> bool {
        self[3] <= 1
    }
}

#[cfg(feature = "software-renderer")]
fn generate_texture(
    image: image::RgbaImage,
    source_format: SourceFormat,
    original_size: Size,
) -> Texture {
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
        Rgb([u8; 3]),
    }
    let mut color = ColorState::Unset;
    'outer: for y in top..=bottom {
        for x in left..=right {
            let p = image.get_pixel(x, y);
            let alpha = p[3];
            if alpha != 255 {
                is_opaque = false;
            }
            if alpha == 0 {
                continue;
            }
            let get_pixel = || match source_format {
                SourceFormat::RgbaPremultiplied => <[u8; 3]>::try_from(&p.0[0..3])
                    .unwrap()
                    .map(|v| (v as u16 * 255 / alpha as u16) as u8),
                SourceFormat::Rgba => p.0[0..3].try_into().unwrap(),
            };
            match color {
                ColorState::Unset => {
                    color = ColorState::Rgb(get_pixel());
                }
                ColorState::Different => {
                    if !is_opaque {
                        break 'outer;
                    }
                }
                ColorState::Rgb([a, b, c]) => {
                    let abs_diff = |t, u| {
                        if t < u {
                            u - t
                        } else {
                            t - u
                        }
                    };
                    let px = get_pixel();
                    if abs_diff(a, px[0]) > 2 || abs_diff(b, px[1]) > 2 || abs_diff(c, px[2]) > 2 {
                        color = ColorState::Different
                    }
                }
            }
        }
    }

    let format = if let ColorState::Rgb(c) = color {
        PixelFormat::AlphaMap(c)
    } else if is_opaque {
        PixelFormat::Rgb
    } else {
        PixelFormat::RgbaPremultiplied
    };

    let rect = Rect::from_ltrb(left as _, top as _, (right + 1) as _, (bottom + 1) as _).unwrap();
    Texture {
        total_size: Size { width: image.width(), height: image.height() },
        original_size,
        rect,
        data: convert_image(image, source_format, format, rect),
        format,
    }
}

#[cfg(feature = "software-renderer")]
fn convert_image(
    image: image::RgbaImage,
    source_format: SourceFormat,
    format: PixelFormat,
    rect: Rect,
) -> Vec<u8> {
    let i = image::SubImage::new(&image, rect.x() as _, rect.y() as _, rect.width(), rect.height());
    match (source_format, format) {
        (_, PixelFormat::Rgb) => {
            i.pixels().flat_map(|(_, _, p)| IntoIterator::into_iter(p.0).take(3)).collect()
        }
        (SourceFormat::RgbaPremultiplied, PixelFormat::RgbaPremultiplied)
        | (SourceFormat::Rgba, PixelFormat::Rgba) => {
            i.pixels().flat_map(|(_, _, p)| IntoIterator::into_iter(p.0)).collect()
        }
        (SourceFormat::Rgba, PixelFormat::RgbaPremultiplied) => i
            .pixels()
            .flat_map(|(_, _, p)| {
                let a = p.0[3] as u32;
                IntoIterator::into_iter(p.0)
                    .take(3)
                    .map(move |x| (x as u32 * a / 255) as u8)
                    .chain(std::iter::once(a as u8))
            })
            .collect(),
        (SourceFormat::RgbaPremultiplied, PixelFormat::Rgba) => i
            .pixels()
            .flat_map(|(_, _, p)| {
                let a = p.0[3] as u32;
                IntoIterator::into_iter(p.0)
                    .take(3)
                    .map(move |x| (x as u32 * 255 / a) as u8)
                    .chain(std::iter::once(a as u8))
            })
            .collect(),
        (_, PixelFormat::AlphaMap(_)) => i.pixels().map(|(_, _, p)| p[3]).collect(),
    }
}

#[cfg(feature = "software-renderer")]
enum SourceFormat {
    RgbaPremultiplied,
    Rgba,
}

#[cfg(feature = "software-renderer")]
fn load_image(
    file: crate::fileaccess::VirtualFile,
    scale_factor: f64,
) -> image::ImageResult<(image::RgbaImage, SourceFormat, Size)> {
    use resvg::{tiny_skia, usvg};
    use std::ffi::OsStr;
    if file.canon_path.extension() == Some(OsStr::new("svg"))
        || file.canon_path.extension() == Some(OsStr::new("svgz"))
    {
        let tree = {
            let option = usvg::Options::default();
            match file.builtin_contents {
                Some(data) => usvg::Tree::from_data(data, &option),
                None => usvg::Tree::from_data(
                    std::fs::read(&file.canon_path).map_err(image::ImageError::IoError)?.as_slice(),
                    &option,
                ),
            }
            .map_err(|e| {
                image::ImageError::Decoding(image::error::DecodingError::new(
                    image::error::ImageFormatHint::Name("svg".into()),
                    e,
                ))
            })
        }?;
        let scale_factor = scale_factor as f32;
        // TODO: ideally we should find the size used for that `Image`
        let original_size = tree.size();
        let width = original_size.width() * scale_factor;
        let height = original_size.height() * scale_factor;

        let mut buffer = vec![0u8; width as usize * height as usize * 4];
        let size_error = || {
            image::ImageError::Limits(image::error::LimitError::from_kind(
                image::error::LimitErrorKind::DimensionError,
            ))
        };
        let mut skia_buffer =
            tiny_skia::PixmapMut::from_bytes(buffer.as_mut_slice(), width as u32, height as u32)
                .ok_or_else(size_error)?;
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(scale_factor as _, scale_factor as _),
            &mut skia_buffer,
        );
        return image::RgbaImage::from_raw(width as u32, height as u32, buffer)
            .ok_or_else(size_error)
            .map(|img| {
                (
                    img,
                    SourceFormat::RgbaPremultiplied,
                    Size { width: original_size.width() as _, height: original_size.height() as _ },
                )
            });
    }
    if let Some(buffer) = file.builtin_contents {
        image::load_from_memory(buffer)
    } else {
        image::open(file.canon_path)
    }
    .map(|mut image| {
        let (original_width, original_height) = image.dimensions();

        if scale_factor < 1. {
            image = image.resize_exact(
                (original_width as f64 * scale_factor) as u32,
                (original_height as f64 * scale_factor) as u32,
                image::imageops::FilterType::Gaussian,
            );
        }

        (
            image.to_rgba8(),
            SourceFormat::Rgba,
            Size { width: original_width, height: original_height },
        )
    })
}
