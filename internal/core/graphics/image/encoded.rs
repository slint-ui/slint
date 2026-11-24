// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/*!
Images that keep only where their encoded data is.
The [image cache](super::cache) holds what's parsed or decoded from that data.
*/

#[cfg(any(feature = "svg", feature = "image-decoders"))]
use super::EncodedData;
use super::{ImageCacheKey, IntSize};
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
    /// An image that the browser decodes.
    Html,
}

/// An image that's decoded, parsed, or handed to the browser when it's needed.
#[derive(Clone, Debug, PartialEq)]
#[repr(C)]
pub struct EncodedImage {
    source: ImageCacheKey,
    kind: EncodedImageKind,
    /// The file extension of the format, or the MIME type for the browser.
    format: Slice<'static, u8>,
    /// The size read when loading, unknown for [`EncodedImageKind::Html`].
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

    /// Returns the size in pixels, or zero while the browser hasn't loaded the image.
    pub fn size(&self) -> IntSize {
        #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std"))]
        if self.kind == EncodedImageKind::Html {
            return self.html_image().and_then(|image| image.size()).unwrap_or_default();
        }
        self.size
    }

    /// Returns true for an SVG, including one that the browser renders.
    pub fn is_svg(&self) -> bool {
        match self.kind {
            EncodedImageKind::Svg => true,
            EncodedImageKind::Html => self.format.as_slice() == b"image/svg+xml",
            _ => false,
        }
    }

    /// Renders the image: an SVG at `target_size_for_scalable_source` or its own size,
    /// and an animation at its first frame.
    #[cfg(feature = "std")]
    pub(crate) fn render_to_buffer(
        &self,
        _target_size_for_scalable_source: Option<euclid::Size2D<u32, crate::lengths::PhysicalPx>>,
    ) -> Option<super::SharedImageBuffer> {
        match self.kind {
            EncodedImageKind::Raster => self.decode_shared().map(|buffer| (*buffer).clone()),
            #[cfg(feature = "svg")]
            EncodedImageKind::Svg => match self.svg()?.render(_target_size_for_scalable_source) {
                Ok(buffer) => Some(buffer),
                // A 0x0 image is just empty.
                Err(resvg::usvg::Error::InvalidSize) => None,
                Err(err) => {
                    std::eprintln!("Error rendering SVG: {err}");
                    None
                }
            },
            #[cfg(all(
                feature = "image-decoders",
                any(not(target_arch = "wasm32"), target_os = "emscripten")
            ))]
            EncodedImageKind::Animated => Some(self.animation()?.first_frame_buffer().clone()),
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

    /// Returns the decoded still image, shared through the image cache.
    #[cfg(feature = "std")]
    pub(crate) fn decode_shared(&self) -> Option<Rc<super::SharedImageBuffer>> {
        if self.kind != EncodedImageKind::Raster {
            return None;
        }
        super::cache::cached(&self.source, || {
            let buffer = self.decode()?;
            let weight = super::cache::buffer_weight(&buffer);
            Some((buffer, weight))
        })
    }

    /// Returns the parsed SVG document, shared through the image cache.
    #[cfg(feature = "svg")]
    pub fn svg(&self) -> Option<Rc<super::ParsedSVG>> {
        if self.kind != EncodedImageKind::Svg {
            return None;
        }
        super::cache::cached(&self.source, || parse_svg(&self.read()?))
    }

    /// Returns the animation, shared through the image cache.
    #[cfg(all(
        feature = "image-decoders",
        any(not(target_arch = "wasm32"), target_os = "emscripten")
    ))]
    pub fn animation(&self) -> Option<Rc<super::AnimatedImage>> {
        if self.kind != EncodedImageKind::Animated {
            return None;
        }
        super::cache::cached(&self.source, || {
            let animation = super::animated::load(self.read()?, image_format(&self.format)?)
                .map_err(|err| crate::debug_log!("Error decoding animation: {err}"))
                .ok()??;
            let weight = animation.weight_in_bytes();
            Some((animation, weight))
        })
    }

    /// Returns the `<img>` element that the browser loads the image with,
    /// shared through the image cache.
    #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std"))]
    pub fn html_image(&self) -> Option<Rc<super::HTMLImage>> {
        if self.kind != EncodedImageKind::Html {
            return None;
        }
        // The browser keeps its own cache; this only bounds the number of DOM elements.
        const WEIGHT: usize = 512;
        super::cache::cached(&self.source, || {
            let mime_type = core::str::from_utf8(self.format.as_slice()).ok()?;
            let image = match &self.source {
                ImageCacheKey::URL(url) => super::HTMLImage::new(url),
                ImageCacheKey::EmbeddedData(data) => {
                    super::HTMLImage::new_from_data(data.as_slice(), mime_type)?
                }
                ImageCacheKey::Data(data) => super::HTMLImage::new_from_data(data, mime_type)?,
                _ => return None,
            };
            Some((image, WEIGHT))
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
#[cfg(all(
    feature = "std",
    any(feature = "image-decoders", all(target_arch = "wasm32", not(target_os = "emscripten")))
))]
fn load_cached(
    key: ImageCacheKey,
    load: impl FnOnce() -> Option<EncodedImage>,
) -> Option<super::ImageInner> {
    let encoded =
        super::cache::cached(&key, || Some((load()?, core::mem::size_of::<EncodedImage>())))?;
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
fn load_svg(source: ImageCacheKey, data: &[u8]) -> Option<EncodedImage> {
    let size = super::cache::cached(&source, || parse_svg(data))?.size();
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
    load_svg(source, data).map(super::ImageInner::Encoded)
}

/// Loads `data`, the encoded data of `source`, given the file extension of its format, if known.
/// The cache keeps what's parsed or decoded to tell an SVG or animation's size.
#[cfg(feature = "image-decoders")]
fn load(source: ImageCacheKey, data: EncodedData, extension: &[u8]) -> Option<EncodedImage> {
    if is_svg(&data, extension) {
        return cfg_select! {
            feature = "svg" => load_svg(source, &data),
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
            super::cache::cached(&source, || Some((animation, weight)));
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

/// Loads the image at `path`, sharing it with other loads of the same file.
#[cfg(all(feature = "image-decoders", any(not(target_arch = "wasm32"), target_os = "emscripten")))]
pub(crate) fn load_from_path(path: &crate::SharedString) -> Option<super::ImageInner> {
    if path.is_empty() {
        return None;
    }
    let source = ImageCacheKey::Path(super::CachedPath::new(path.as_str()));
    load_cached(source.clone(), || {
        let extension = std::path::Path::new(path.as_str())
            .extension()
            .map_or(&b""[..], |extension| extension.as_encoded_bytes());
        let format = image_format(extension);
        let needs_data = is_svg_extension(extension) || format.is_none_or(may_animate);
        let loaded = if needs_data {
            std::fs::read(path.as_str()).map(|data| {
                load(source, EncodedData::Shared(crate::SharedVector::from_slice(&data)), extension)
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
    data: &'static [u8],
    extension: &[u8],
) -> Option<super::ImageInner> {
    let source = ImageCacheKey::EmbeddedData(by_address::ByAddress(Slice::from_slice(data)));
    load_cached(source.clone(), || {
        cfg_select! {
            feature = "image-decoders" => load(source, EncodedData::Static(data), extension),
            _ => load_html(source, data, extension),
        }
    })
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
                ImageCacheKey::Data(by_address::ByAddress(shared.clone())),
                EncodedData::Shared(shared),
                extension,
            )?;
            if encoded.kind == EncodedImageKind::Raster {
                return encoded.decode().map(super::ImageInner::EmbeddedImage);
            }
            Some(super::ImageInner::Encoded(encoded))
        }
        _ => load_html(
            ImageCacheKey::Data(by_address::ByAddress(crate::SharedVector::from_slice(data))),
            data,
            extension,
        )
        .map(super::ImageInner::Encoded),
    }
}

/// Loads the payload of a data URI. The browser loads asynchronously, so all the loads
/// of the same URI share one image and its loading state.
#[cfg(any(
    feature = "image-decoders",
    all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
))]
pub(crate) fn load_from_data_uri(
    uri: &str,
    data: &[u8],
    extension: &[u8],
) -> Option<super::ImageInner> {
    cfg_select! {
        feature = "image-decoders" => {
            let _ = uri;
            load_from_data(data, extension)
        }
        _ => load_cached(ImageCacheKey::URL(uri.into()), || {
            load_html(
                ImageCacheKey::Data(by_address::ByAddress(crate::SharedVector::from_slice(data))),
                data,
                extension,
            )
        }),
    }
}

/// Loads an image that the browser fetches from `url`.
#[cfg(all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std"))]
pub(crate) fn load_as_html_image(url: &str) -> Option<super::ImageInner> {
    if url.is_empty() {
        return None;
    }
    let mime_type: &'static [u8] =
        if url.ends_with(".svg") || url.ends_with(".svgz") { b"image/svg+xml" } else { b"" };
    let source = ImageCacheKey::URL(url.into());
    load_cached(source.clone(), || {
        Some(EncodedImage {
            source,
            kind: EncodedImageKind::Html,
            format: Slice::from_slice(mime_type),
            size: IntSize::default(),
        })
    })
}

/// Hands `data`, the encoded data of `source`, to the browser to decode.
#[cfg(all(
    target_arch = "wasm32",
    not(target_os = "emscripten"),
    feature = "std",
    not(feature = "image-decoders")
))]
fn load_html(source: ImageCacheKey, data: &[u8], extension: &[u8]) -> Option<EncodedImage> {
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
    let encoded = EncodedImage {
        source,
        kind: EncodedImageKind::Html,
        format: Slice::from_slice(mime_type.as_bytes()),
        size: IntSize::default(),
    };
    encoded.html_image()?;
    Some(encoded)
}
