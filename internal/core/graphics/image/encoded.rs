// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
Images that keep only where their encoded data is.
The [image cache](super::cache) holds what's parsed or decoded from that data.
*/

#[cfg(any(feature = "svg", feature = "image-decoders"))]
use super::EncodedData;
use super::{ImageCacheKey, IntSize};
#[cfg(feature = "std")]
use crate::SlintContext;
use crate::slice::Slice;
#[cfg(feature = "std")]
use alloc::rc::Rc;

/// What an [`EncodedImage`] holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EncodedImageKind {
    /// A still image, in a format of the `image` crate.
    Raster,
    /// An SVG document.
    Svg,
    /// An image with more than one frame.
    Animated,
}

/// An image that's decoded or parsed when it's needed.
///
/// Functions that take a `context` share what they make through its image cache,
/// or don't cache it without one.
#[derive(Clone, Debug, PartialEq)]
#[repr(C)]
pub struct EncodedImage {
    source: ImageCacheKey,
    kind: EncodedImageKind,
    /// The file extension of the format.
    format: Slice<'static, u8>,
    size: IntSize,
}

impl EncodedImage {
    /// Returns where the encoded data is.
    pub fn source(&self) -> &ImageCacheKey {
        &self.source
    }

    /// Returns what the encoded data is.
    pub fn kind(&self) -> EncodedImageKind {
        self.kind
    }

    /// Returns the size in pixels.
    pub fn size(&self) -> IntSize {
        self.size
    }

    /// Renders the image: an SVG at `target_size_for_scalable_source` or its own size,
    /// and an animation at its first frame.
    #[cfg(feature = "std")]
    pub(crate) fn render_to_buffer(
        &self,
        _target_size_for_scalable_source: Option<euclid::Size2D<u32, crate::lengths::PhysicalPx>>,
        context: Option<&SlintContext>,
    ) -> Option<super::SharedImageBuffer> {
        match self.kind {
            EncodedImageKind::Raster => self.decode(),
            #[cfg(feature = "svg")]
            EncodedImageKind::Svg => {
                match self.svg(context)?.render(_target_size_for_scalable_source) {
                    Ok(buffer) => Some(buffer),
                    // A 0x0 image is just empty.
                    Err(resvg::usvg::Error::InvalidSize) => None,
                    Err(err) => {
                        std::eprintln!("Error rendering SVG: {err}");
                        None
                    }
                }
            }
            #[cfg(all(
                feature = "image-decoders",
                any(not(target_arch = "wasm32"), target_os = "emscripten")
            ))]
            EncodedImageKind::Animated => {
                Some(self.animation(context)?.first_frame_buffer().clone())
            }
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// Decodes a still image. The result isn't cached.
    #[cfg(feature = "std")]
    fn decode(&self) -> Option<super::SharedImageBuffer> {
        cfg_select! {
            feature = "image-decoders" => {
                let buffer = decode_data(&self.read()?, image_format(&self.format)?)?;
                if buffer.size() != self.size {
                    crate::debug_log!("Error decoding image: its size changed since it was loaded");
                    return None;
                }
                Some(buffer)
            }
            _ => None,
        }
    }

    /// Decodes a still image into premultiplied RGBA pixels. The result isn't cached.
    #[cfg(feature = "image-decoders")]
    pub(crate) fn decode_rgba_premultiplied(
        &self,
    ) -> Option<super::SharedPixelBuffer<super::Rgba8Pixel>> {
        if self.kind != EncodedImageKind::Raster {
            return None;
        }
        let format = image_format(&self.format)?;
        let decoded = match &self.source {
            #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
            ImageCacheKey::Path(path) => std::fs::File::open(path.path.as_str())
                .map_err(image::ImageError::IoError)
                .and_then(|file| decode_rgba_premultiplied(std::io::BufReader::new(file), format)),
            _ => decode_rgba_premultiplied(std::io::Cursor::new(&*self.read()?), format),
        };
        decoded.map_err(|err| crate::debug_log!("Error decoding image: {err}")).ok()
    }

    /// Returns the decoded still image, shared through the image cache of `context`.
    #[cfg(feature = "std")]
    pub(crate) fn decode_shared(
        &self,
        context: &SlintContext,
    ) -> Option<Rc<super::SharedImageBuffer>> {
        if self.kind != EncodedImageKind::Raster {
            return None;
        }
        super::cache::cached(Some(context), &self.source, || {
            let buffer = self.decode()?;
            let weight = super::cache::buffer_weight(&buffer);
            Some((buffer, weight))
        })
    }

    /// Returns the parsed SVG document.
    #[cfg(feature = "svg")]
    pub fn svg(&self, context: Option<&SlintContext>) -> Option<Rc<super::ParsedSVG>> {
        if self.kind != EncodedImageKind::Svg {
            return None;
        }
        super::cache::cached(context, &self.source, || parse_svg(&self.read()?))
    }

    /// Returns the animation.
    #[cfg(all(
        feature = "image-decoders",
        any(not(target_arch = "wasm32"), target_os = "emscripten")
    ))]
    pub fn animation(&self, context: Option<&SlintContext>) -> Option<Rc<super::AnimatedImage>> {
        if self.kind != EncodedImageKind::Animated {
            return None;
        }
        super::cache::cached(context, &self.source, || {
            let animation = super::animated::load(self.read()?, image_format(&self.format)?)
                .map_err(|err| crate::debug_log!("Error decoding animation: {err}"))
                .ok()??;
            let weight = animation.weight_in_bytes();
            Some((animation, weight))
        })
    }

    /// Reads the encoded data.
    #[cfg(any(feature = "svg", feature = "image-decoders"))]
    fn read(&self) -> Option<EncodedData> {
        match &self.source {
            #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
            ImageCacheKey::Path(path) => std::fs::read(path.path.as_str())
                .map(|data| EncodedData::Shared(crate::SharedVector::from_slice(&data)))
                .map_err(|err| crate::debug_log!("Error reading {}: {err}", path.path))
                .ok(),
            ImageCacheKey::EmbeddedData(data) => Some(EncodedData::Static(data.as_slice())),
            ImageCacheKey::Data(data) => Some(EncodedData::Shared(data.0.clone())),
            #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
            _ => None,
        }
    }
}

/// Parses the SVG document `data`, and returns it with its size in bytes.
#[cfg(feature = "svg")]
fn parse_svg(data: &[u8]) -> Option<(super::ParsedSVG, usize)> {
    let svg = super::svg::load_from_data(data)
        .map_err(|err| crate::debug_log!("Error loading SVG: {err}"))
        .ok()?;
    let weight = svg.weight_in_bytes();
    Some((svg, weight))
}

/// Decodes the still image `data`.
#[cfg(feature = "image-decoders")]
fn decode_data(data: &[u8], format: image::ImageFormat) -> Option<super::SharedImageBuffer> {
    image::load_from_memory_with_format(data, format)
        .map(super::dynamic_image_to_shared_image_buffer)
        .map_err(|err| crate::debug_log!("Error decoding image: {err}"))
        .ok()
}

/// Decodes the still image `data` into premultiplied RGBA pixels.
#[cfg(feature = "image-decoders")]
fn decode_rgba_premultiplied(
    data: impl std::io::BufRead + std::io::Seek,
    format: image::ImageFormat,
) -> image::ImageResult<super::SharedPixelBuffer<super::Rgba8Pixel>> {
    use image::{ColorType, ImageDecoder};
    let decoder = image::ImageReader::with_format(data, format).into_decoder()?;
    #[cfg_attr(slint_nightly_test, allow(non_exhaustive_omitted_patterns))]
    let channels = match decoder.color_type() {
        ColorType::L8 => 1,
        ColorType::La8 => 2,
        ColorType::Rgb8 => 3,
        ColorType::Rgba8 => 4,
        _ => {
            let image = image::DynamicImage::from_decoder(decoder)?.into_rgba8();
            return Ok(super::rgba_image_to_premultiplied(image));
        }
    };
    let (width, height) = decoder.dimensions();
    let mut buffer = super::SharedPixelBuffer::<super::Rgba8Pixel>::new(width, height);
    let bytes = buffer.make_mut_bytes();
    let pixels = bytes.len() / 4;
    decoder.read_image(&mut bytes[..pixels * channels])?;
    // From the end, each RGBA pixel only overwrites decoded pixels that are already expanded.
    for i in (0..pixels).rev() {
        let [r, g, b, a] = match bytes[i * channels..(i + 1) * channels] {
            [l] => [l, l, l, 255],
            [l, a] => [l, l, l, a],
            [r, g, b] => [r, g, b, 255],
            [r, g, b, a] => [r, g, b, a],
            _ => unreachable!(),
        };
        let pixel = super::Image::rgba_to_premultiplied_rgba(super::Rgba8Pixel { r, g, b, a });
        bytes[i * 4..(i + 1) * 4].copy_from_slice(&[pixel.r, pixel.g, pixel.b, pixel.a]);
    }
    Ok(buffer)
}

/// Returns the image format with the file extension `extension`.
#[cfg(feature = "image-decoders")]
fn image_format(extension: &[u8]) -> Option<image::ImageFormat> {
    core::str::from_utf8(extension).ok().and_then(image::ImageFormat::from_extension)
}

/// Returns the file extension that [`image_format`] maps back to `format`.
#[cfg(feature = "image-decoders")]
fn format_extension(format: image::ImageFormat) -> Slice<'static, u8> {
    Slice::from_slice(format.extensions_str()[0].as_bytes())
}

/// Returns true if images in `format` can have more than one frame.
#[cfg(feature = "image-decoders")]
fn may_animate(format: image::ImageFormat) -> bool {
    cfg_select! {
        all(target_arch = "wasm32", not(target_os = "emscripten")) => {
            let _ = format;
            false
        }
        _ => super::animated::can_animate(format),
    }
}

/// Returns true if `data` starts like an SVG document.
#[cfg(any(feature = "image-decoders", all(target_arch = "wasm32", not(target_os = "emscripten"))))]
fn looks_like_svg(data: &[u8]) -> bool {
    data.starts_with(b"<?xml") || data.starts_with(b"<svg")
}

/// Returns the image of `key`, loading it with `load` unless an earlier load is in the cache.
#[cfg(feature = "image-decoders")]
fn load_cached(
    context: Option<&SlintContext>,
    key: ImageCacheKey,
    load: impl FnOnce() -> Option<EncodedImage>,
) -> Option<super::ImageInner> {
    let encoded = super::cache::cached(context, &key, || {
        Some((load()?, core::mem::size_of::<EncodedImage>()))
    })?;
    Some(super::ImageInner::Encoded(EncodedImage::clone(&encoded)))
}

#[cfg(feature = "image-decoders")]
fn is_svg_extension(extension: &[u8]) -> bool {
    core::str::from_utf8(extension).ok().and_then(super::image_mime_type_from_extension)
        == Some("image/svg+xml")
}

#[cfg(feature = "image-decoders")]
fn is_svg(data: &[u8], extension: &[u8]) -> bool {
    is_svg_extension(extension) || (extension.is_empty() && looks_like_svg(data))
}

/// Reads the size of a still image from the header of `input`.
#[cfg(feature = "image-decoders")]
fn load_raster(
    source: ImageCacheKey,
    input: impl std::io::BufRead + std::io::Seek,
    format: image::ImageFormat,
) -> Option<EncodedImage> {
    let (width, height) = image::ImageReader::with_format(input, format)
        .into_dimensions()
        .map_err(|err| crate::debug_log!("Error loading image: {err}"))
        .ok()?;
    Some(EncodedImage {
        source,
        kind: EncodedImageKind::Raster,
        format: format_extension(format),
        size: IntSize::new(width, height),
    })
}

/// Parses `data`, the SVG document of `source`, and keeps the tree in the cache.
#[cfg(all(
    feature = "svg",
    any(not(target_arch = "wasm32"), target_os = "emscripten", feature = "image-decoders")
))]
fn load_svg(
    context: Option<&SlintContext>,
    source: ImageCacheKey,
    data: &[u8],
) -> Option<EncodedImage> {
    let size = super::cache::cached(context, &source, || parse_svg(data))?.size();
    Some(EncodedImage {
        source,
        kind: EncodedImageKind::Svg,
        format: Slice::from_slice(b"svg"),
        size,
    })
}

/// Loads the SVG document `data`, which is copied to be parsed again later.
#[cfg(all(
    feature = "svg",
    any(not(target_arch = "wasm32"), target_os = "emscripten", feature = "image-decoders")
))]
pub(crate) fn load_svg_from_data(data: &[u8]) -> Option<super::ImageInner> {
    let source = ImageCacheKey::Data(by_address::ByAddress(crate::SharedVector::from_slice(data)));
    load_svg(None, source, data).map(super::ImageInner::Encoded)
}

/// Loads `data`, the encoded data of `source`, given the file extension of its format, if known.
/// The cache keeps what's parsed or decoded to tell an SVG or animation's size.
#[cfg(feature = "image-decoders")]
fn load(
    context: Option<&SlintContext>,
    source: ImageCacheKey,
    data: EncodedData,
    extension: &[u8],
) -> Option<EncodedImage> {
    if is_svg(&data, extension) {
        return cfg_select! {
            feature = "svg" => load_svg(context, source, &data),
            _ => None,
        };
    }
    let format = image_format(extension).or_else(|| image::guess_format(&data).ok());
    let Some(format) = format else {
        crate::debug_log!("Error loading image: unknown format");
        return None;
    };
    #[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
    match super::animated::load(data.clone(), format) {
        Ok(Some(animation)) => {
            let size = animation.size();
            let weight = animation.weight_in_bytes();
            super::cache::cached(context, &source, || Some((animation, weight)));
            return Some(EncodedImage {
                source,
                kind: EncodedImageKind::Animated,
                format: format_extension(format),
                size,
            });
        }
        Ok(None) => {}
        Err(err) => crate::debug_log!("Error decoding animation, showing a still image: {err}"),
    }
    load_raster(source, std::io::Cursor::new(&*data), format)
}

/// Returns the raster image at `source`, of `size` and with more than one frame if `animated`,
/// without reading it. `extension` is the file extension of its format.
/// Returns no image if this build can't decode the format.
#[cfg(feature = "image-decoders")]
fn sized(
    source: ImageCacheKey,
    extension: &[u8],
    size: IntSize,
    animated: bool,
) -> super::ImageInner {
    let Some(format) = image_format(extension).filter(image::ImageFormat::reading_enabled) else {
        return super::ImageInner::None;
    };
    let kind = if animated { EncodedImageKind::Animated } else { EncodedImageKind::Raster };
    super::ImageInner::Encoded(EncodedImage {
        source,
        kind,
        format: format_extension(format),
        size,
    })
}

/// Like [`sized`], for an image that the binary embeds.
#[cfg(feature = "image-decoders")]
pub(crate) fn sized_from_embedded_data(
    data: Slice<'static, u8>,
    extension: &[u8],
    size: IntSize,
    animated: bool,
) -> super::ImageInner {
    sized(ImageCacheKey::EmbeddedData(by_address::ByAddress(data)), extension, size, animated)
}

/// Like [`sized`], for the image at `path`.
#[cfg(all(feature = "image-decoders", any(not(target_arch = "wasm32"), target_os = "emscripten")))]
pub(crate) fn sized_from_path(path: &str, size: IntSize, animated: bool) -> super::ImageInner {
    let extension = std::path::Path::new(path).extension().unwrap_or_default();
    let source = ImageCacheKey::Path(super::CachedPath::new(path));
    sized(source, extension.as_encoded_bytes(), size, animated)
}

/// Loads the image at `path`, sharing it with other loads of the same file.
#[cfg(all(feature = "image-decoders", any(not(target_arch = "wasm32"), target_os = "emscripten")))]
pub(crate) fn load_from_path(
    context: Option<&SlintContext>,
    path: &crate::SharedString,
) -> Option<super::ImageInner> {
    if path.is_empty() {
        return None;
    }
    let source = ImageCacheKey::Path(super::CachedPath::new(path.as_str()));
    load_cached(context, source.clone(), || {
        let extension = std::path::Path::new(path.as_str())
            .extension()
            .map_or(&b""[..], |extension| extension.as_encoded_bytes());
        let format = image_format(extension);
        let needs_data = is_svg_extension(extension) || format.is_none_or(may_animate);
        let loaded = if needs_data {
            std::fs::read(path.as_str()).map(|data| {
                let data = EncodedData::Shared(crate::SharedVector::from_slice(&data));
                load(context, source, data, extension)
            })
        } else {
            std::fs::File::open(path.as_str())
                .map(|file| load_raster(source, std::io::BufReader::new(file), format?))
        };
        loaded.map_err(|err| crate::debug_log!("Error loading image from {path}: {err}")).ok()?
    })
}

/// Loads `data` that the binary embeds, sharing it with other loads of the same data.
/// `extension` is the file extension of the format, or empty to guess it.
#[cfg(any(
    feature = "image-decoders",
    all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
))]
pub(crate) fn load_from_static_data(
    context: Option<&SlintContext>,
    data: &'static [u8],
    extension: &[u8],
) -> Option<super::ImageInner> {
    let source = ImageCacheKey::EmbeddedData(by_address::ByAddress(Slice::from_slice(data)));
    cfg_select! {
        feature = "image-decoders" => load_cached(context, source.clone(), || {
            load(context, source, EncodedData::Static(data), extension)
        }),
        _ => load_html_cached(context, &source, || load_html(data, extension)),
    }
}

/// Loads `data`, copied only if it's an SVG or an animation, which are parsed again from it.
/// A still image is decoded right away.
#[cfg(any(
    feature = "image-decoders",
    all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
))]
pub(crate) fn load_from_data(data: &[u8], extension: &[u8]) -> Option<super::ImageInner> {
    cfg_select! {
        feature = "image-decoders" => {
            let format = image_format(extension).or_else(|| image::guess_format(data).ok());
            if !is_svg(data, extension) && !format.is_some_and(may_animate) {
                return decode_data(data, format?).map(super::ImageInner::EmbeddedImage);
            }
            let shared = crate::SharedVector::from_slice(data);
            let encoded = load(
                None,
                ImageCacheKey::Data(by_address::ByAddress(shared.clone())),
                EncodedData::Shared(shared),
                extension,
            )?;
            if encoded.kind == EncodedImageKind::Raster {
                return encoded.decode().map(super::ImageInner::EmbeddedImage);
            }
            Some(super::ImageInner::Encoded(encoded))
        }
        _ => load_html(data, extension).map(|image| super::ImageInner::HTMLImage(Rc::new(image))),
    }
}

/// Loads the payload of a data URI. The browser loads asynchronously, so the loads
/// of the same URI share one image and its loading state through the cache of `context`.
#[cfg(any(
    feature = "image-decoders",
    all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
))]
pub(crate) fn load_from_data_uri(
    context: &SlintContext,
    uri: &str,
    data: &[u8],
    extension: &[u8],
) -> Option<super::ImageInner> {
    cfg_select! {
        feature = "image-decoders" => {
            let _ = (context, uri);
            load_from_data(data, extension)
        }
        _ => {
            load_html_cached(Some(context), &ImageCacheKey::URL(uri.into()), || {
                load_html(data, extension)
            })
        }
    }
}

/// Loads an image that the browser fetches from `url`.
#[cfg(all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std"))]
pub(crate) fn load_as_html_image(context: &SlintContext, url: &str) -> Option<super::ImageInner> {
    if url.is_empty() {
        return None;
    }
    load_html_cached(Some(context), &ImageCacheKey::URL(url.into()), || {
        Some(super::HTMLImage::new(url))
    })
}

/// Returns the browser image of `key`, made with `create` unless it's in the cache.
#[cfg(all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std"))]
fn load_html_cached(
    context: Option<&SlintContext>,
    key: &ImageCacheKey,
    create: impl FnOnce() -> Option<super::HTMLImage>,
) -> Option<super::ImageInner> {
    // The browser keeps its own cache; this only bounds the number of DOM elements.
    const WEIGHT: usize = 512;
    super::cache::cached(context, key, || Some((create()?, WEIGHT)))
        .map(super::ImageInner::HTMLImage)
}

/// Hands `data` to the browser to decode.
#[cfg(all(
    target_arch = "wasm32",
    not(target_os = "emscripten"),
    feature = "std",
    not(feature = "image-decoders")
))]
fn load_html(data: &[u8], extension: &[u8]) -> Option<super::HTMLImage> {
    let mime_type = core::str::from_utf8(extension)
        .ok()
        .and_then(super::image_mime_type_from_extension)
        .unwrap_or_else(|| {
            if looks_like_svg(data) {
                "image/svg+xml"
            } else {
                // An empty type makes the browser sniff the format from the data.
                ""
            }
        });
    if mime_type == "image/svg+xml" && data.starts_with(&[0x1f, 0x8b]) {
        crate::debug_log!("Compressed SVG (.svgz) is not supported on the web");
        return None;
    }
    super::HTMLImage::new_from_data(data, mime_type)
}

#[cfg(all(test, feature = "image-decoders"))]
mod tests {
    // cSpell: ignore Luma
    use super::*;

    /// Returns `buffer` as premultiplied RGBA bytes.
    fn rgba_bytes(buffer: super::super::SharedImageBuffer) -> alloc::vec::Vec<u8> {
        match buffer {
            super::super::SharedImageBuffer::RGB8(pixels) => {
                pixels.as_bytes().chunks(3).flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255]).collect()
            }
            super::super::SharedImageBuffer::RGBA8Premultiplied(pixels) => pixels.as_bytes().into(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn decode_rgba_premultiplied_matches_decode() {
        let pixels =
            |channels: usize| (0..2 * 3 * channels).map(|i| (i * 37 % 256) as u8).collect();
        let images = [
            image::DynamicImage::ImageLuma8(image::ImageBuffer::from_raw(2, 3, pixels(1)).unwrap()),
            image::DynamicImage::ImageLumaA8(
                image::ImageBuffer::from_raw(2, 3, pixels(2)).unwrap(),
            ),
            image::DynamicImage::ImageRgb8(image::ImageBuffer::from_raw(2, 3, pixels(3)).unwrap()),
            image::DynamicImage::ImageRgba8(image::ImageBuffer::from_raw(2, 3, pixels(4)).unwrap()),
            image::DynamicImage::ImageRgba16(
                image::ImageBuffer::from_raw(2, 3, (0..24).map(|i| i * 2731).collect()).unwrap(),
            ),
        ];
        let encoded = images
            .iter()
            .map(|image| (image::ImageFormat::Png, image))
            .chain([(image::ImageFormat::Jpeg, &images[2])])
            .map(|(format, image)| {
                let mut data = std::io::Cursor::new(alloc::vec::Vec::new());
                image.write_to(&mut data, format).unwrap();
                (format, data.into_inner())
            });
        for (format, data) in encoded {
            let rgba = decode_rgba_premultiplied(std::io::Cursor::new(&data), format).unwrap();
            assert_eq!((rgba.width(), rgba.height()), (2, 3), "{format:?}");
            let expected = rgba_bytes(decode_data(&data, format).unwrap());
            assert_eq!(rgba.as_bytes(), expected.as_slice(), "{format:?}");
        }
    }
}
