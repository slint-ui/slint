// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore ANIMEXTS

/*!
Animated GIF, PNG and WebP images, decoded one frame at a time.
*/

use super::{Image, ImageCacheKey, IntSize, OpaqueImage, Rgba8Pixel, SharedPixelBuffer};

#[cfg(all(
    feature = "image-decoders",
    any(not(target_arch = "wasm32"), target_os = "emscripten")
))]
pub(crate) use decoding::FrameDecoder;
#[cfg(all(
    feature = "image-decoders",
    any(not(target_arch = "wasm32"), target_os = "emscripten")
))]
pub(super) use decoding::load;

/// An image with more than one frame.
pub struct AnimatedImage {
    cache_key: ImageCacheKey,
    first_frame: SharedPixelBuffer<Rgba8Pixel>,
    #[cfg(all(
        feature = "image-decoders",
        any(not(target_arch = "wasm32"), target_os = "emscripten")
    ))]
    source: decoding::Source,
}

impl AnimatedImage {
    pub(crate) fn source_cache_key(&self) -> &ImageCacheKey {
        &self.cache_key
    }

    /// Returns the first frame as a still image.
    pub fn first_frame(&self) -> Image {
        Image::from_rgba8_premultiplied(self.first_frame.clone())
    }

    #[cfg(any(
        feature = "image-decoders",
        all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "std")
    ))]
    pub(crate) fn weight_in_bytes(&self) -> usize {
        let encoded = {
            #[cfg(all(
                feature = "image-decoders",
                any(not(target_arch = "wasm32"), target_os = "emscripten")
            ))]
            {
                self.source.heap_size()
            }
            #[cfg(not(all(
                feature = "image-decoders",
                any(not(target_arch = "wasm32"), target_os = "emscripten")
            )))]
            {
                0
            }
        };
        self.first_frame.as_bytes().len() + encoded
    }
}

impl OpaqueImage for AnimatedImage {
    fn size(&self) -> IntSize {
        self.first_frame.size()
    }
    fn cache_key(&self) -> ImageCacheKey {
        self.source_cache_key().clone()
    }
}

#[cfg(all(feature = "image-decoders", any(not(target_arch = "wasm32"), target_os = "emscripten")))]
mod decoding {
    use super::super::ImageData;
    use super::{AnimatedImage, ImageCacheKey, Rgba8Pixel, SharedPixelBuffer};
    use crate::graphics::ImageInner;
    use core::num::NonZeroU32;
    use core::time::Duration;
    use image::{AnimationDecoder as _, ImageFormat, ImageResult};
    use std::io::Cursor;
    use std::rc::Rc;

    #[derive(Clone)]
    enum EncodedData {
        Static(&'static [u8]),
        Shared(Rc<[u8]>),
    }

    impl AsRef<[u8]> for EncodedData {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::Static(data) => data,
                Self::Shared(data) => data,
            }
        }
    }

    impl From<ImageData<'_>> for EncodedData {
        fn from(data: ImageData<'_>) -> Self {
            match data {
                ImageData::Static(data) => Self::Static(data),
                ImageData::Borrowed(data) => Self::Shared(data.into()),
            }
        }
    }

    #[derive(Clone, Copy)]
    enum Format {
        #[cfg(feature = "image-default-formats")]
        Gif,
        Png,
        #[cfg(feature = "image-default-formats")]
        WebP,
    }

    pub(super) struct Source {
        data: EncodedData,
        format: Format,
        plays: Option<NonZeroU32>,
    }

    impl Source {
        pub(super) fn heap_size(&self) -> usize {
            match &self.data {
                EncodedData::Static(_) => 0,
                EncodedData::Shared(data) => data.len(),
            }
        }
    }

    impl AnimatedImage {
        pub(crate) fn frames(&self) -> Option<FrameDecoder<'static>> {
            frames(self.source.format, Cursor::new(self.source.data.clone()))
                .map(FrameDecoder)
                .map_err(|err| crate::debug_log!("Error decoding animated image: {err}"))
                .ok()
        }

        /// Returns the number of times to play the animation, or `None` to repeat it forever.
        pub(crate) fn plays(&self) -> Option<NonZeroU32> {
            self.source.plays
        }
    }

    pub(crate) struct FrameDecoder<'a>(image::Frames<'a>);

    impl FrameDecoder<'_> {
        /// Returns the next frame and how long to show it, or `None` at the end of the animation.
        pub(crate) fn next_frame(&mut self) -> Option<(SharedPixelBuffer<Rgba8Pixel>, Duration)> {
            self.0
                .next()?
                .map(frame_with_delay)
                .map_err(|err| crate::debug_log!("Error decoding animation frame: {err}"))
                .ok()
        }
    }

    fn frame_with_delay(frame: image::Frame) -> (SharedPixelBuffer<Rgba8Pixel>, Duration) {
        let delay = Duration::from(frame.delay());
        // Browsers show frames with a delay of 10ms or less for 100ms; files rely on that.
        let delay =
            if delay <= Duration::from_millis(10) { Duration::from_millis(100) } else { delay };
        (super::super::rgba_image_to_premultiplied(frame.into_buffer()), delay)
    }

    fn frames<'a, R: std::io::BufRead + std::io::Seek + 'a>(
        format: Format,
        reader: R,
    ) -> ImageResult<image::Frames<'a>> {
        Ok(match format {
            #[cfg(feature = "image-default-formats")]
            Format::Gif => {
                with_default_limits(image::codecs::gif::GifDecoder::new(reader)?)?.into_frames()
            }
            Format::Png => with_default_limits(image::codecs::png::PngDecoder::new(reader)?)?
                .apng()?
                .into_frames(),
            #[cfg(feature = "image-default-formats")]
            Format::WebP => {
                with_default_limits(image::codecs::webp::WebPDecoder::new(reader)?)?.into_frames()
            }
        })
    }

    /// Applies the memory limits that `image::load_from_memory` uses to still images.
    fn with_default_limits<D: image::ImageDecoder>(mut decoder: D) -> ImageResult<D> {
        // Every frame is composited onto a full-size RGBA canvas. The WebP decoder ignores
        // `set_limits` for animations, so check the canvas here.
        let (width, height) = decoder.dimensions();
        image::Limits::default().reserve_buffer(width, height, image::ColorType::Rgba8)?;
        decoder.set_limits(image::Limits::default())?;
        Ok(decoder)
    }

    fn plays_from_loop_count(loop_count: image::metadata::LoopCount) -> Option<NonZeroU32> {
        match loop_count {
            image::metadata::LoopCount::Infinite => None,
            image::metadata::LoopCount::Finite(plays) => Some(plays),
        }
    }

    /// Returns the repeat count of the NETSCAPE2.0 (or ANIMEXTS1.0) application extension,
    /// or `None` if the GIF has none.
    ///
    /// See <https://www.w3.org/Graphics/GIF/spec-gif89a.txt> for the block structure.
    #[cfg(feature = "image-default-formats")]
    pub(super) fn gif_repeat_count(data: &[u8]) -> Option<u16> {
        const EXTENSION: u8 = 0x21;
        const APPLICATION_EXTENSION: u8 = 0xFF;
        const IMAGE_DESCRIPTOR: u8 = 0x2C;
        const LOOP_EXTENSIONS: [&[u8]; 2] = [b"\x0bNETSCAPE2.0", b"\x0bANIMEXTS1.0"];
        let color_table_size = |packed: u8| {
            if packed & 0x80 != 0 { 3 << ((packed & 0x07) + 1) } else { 0 }
        };
        let skip_sub_blocks = |mut pos: usize| -> Option<usize> {
            loop {
                let len = usize::from(*data.get(pos)?);
                pos += 1 + len;
                if len == 0 {
                    return Some(pos);
                }
            }
        };

        let mut pos = 13 + color_table_size(*data.get(10)?);
        loop {
            match *data.get(pos)? {
                EXTENSION => {
                    if *data.get(pos + 1)? == APPLICATION_EXTENSION
                        && LOOP_EXTENSIONS.contains(&data.get(pos + 2..pos + 14)?)
                        && let [3, 1, low, high] = *data.get(pos + 14..pos + 18)?
                    {
                        return Some(u16::from_le_bytes([low, high]));
                    }
                    pos = skip_sub_blocks(pos + 2)?;
                }
                IMAGE_DESCRIPTOR => {
                    let local_color_table = color_table_size(*data.get(pos + 9)?);
                    // Skip the descriptor, the color table, and the LZW minimum code size.
                    pos = skip_sub_blocks(pos + 10 + local_color_table + 1)?;
                }
                _ => return None,
            }
        }
    }

    /// Returns the format and the number of plays if `data` holds an animation.
    fn probe(
        data: &[u8],
        format: ImageFormat,
    ) -> ImageResult<Option<(Format, Option<NonZeroU32>)>> {
        #[cfg_attr(slint_nightly_test, allow(non_exhaustive_omitted_patterns))]
        Ok(match format {
            #[cfg(feature = "image-default-formats")]
            ImageFormat::Gif => {
                // The `image` crate reports a missing NETSCAPE extension as repeating forever.
                // Browsers play such files once, and treat the extension's count as repeats
                // after the first play, with 0 meaning forever.
                let plays = match gif_repeat_count(data) {
                    None => NonZeroU32::new(1),
                    Some(0) => None,
                    Some(repeats) => NonZeroU32::new(u32::from(repeats) + 1),
                };
                Some((Format::Gif, plays))
            }
            ImageFormat::Png => {
                let decoder = image::codecs::png::PngDecoder::new(Cursor::new(data))?;
                if !decoder.is_apng()? {
                    return Ok(None);
                }
                Some((Format::Png, plays_from_loop_count(decoder.apng()?.loop_count())))
            }
            #[cfg(feature = "image-default-formats")]
            ImageFormat::WebP => {
                let decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(data))?;
                if !decoder.has_animation() {
                    return Ok(None);
                }
                Some((Format::WebP, plays_from_loop_count(decoder.loop_count())))
            }
            _ => None,
        })
    }

    /// Loads `data` if it's in a format that supports animation.
    ///
    /// Returns an `AnimatedImage` for files with more than one frame and an `EmbeddedImage` for
    /// files with only one, or `Ok(None)` if the caller should decode `data` as a still image.
    pub(crate) fn load(
        cache_key: ImageCacheKey,
        data: ImageData<'_>,
        image_format: ImageFormat,
    ) -> ImageResult<Option<ImageInner>> {
        let Some((format, plays)) = probe(&data, image_format)? else {
            return Ok(None);
        };
        let mut decoder = FrameDecoder(frames(format, Cursor::new(&*data))?);
        let Some((first_frame, _)) = decoder.next_frame() else {
            return Err(image::ImageError::Decoding(image::error::DecodingError::new(
                image_format.into(),
                "the animation has no frames",
            )));
        };
        if decoder.next_frame().is_none() {
            return Ok(Some(ImageInner::EmbeddedImage {
                cache_key,
                buffer: crate::graphics::SharedImageBuffer::RGBA8Premultiplied(first_frame),
            }));
        }
        Ok(Some(ImageInner::AnimatedImage(vtable::VRc::new(AnimatedImage {
            cache_key,
            first_frame,
            source: Source { data: data.into(), format, plays },
        }))))
    }
}

#[cfg(all(test, feature = "image-decoders", feature = "image-default-formats"))]
mod tests {
    use super::super::{
        ImageCacheKey, ImageData, ImageInner, OpaqueImage, Rgba8Pixel, SharedImageBuffer,
        SharedPixelBuffer,
    };
    use super::AnimatedImage;
    use core::time::Duration;
    use std::vec::Vec;

    fn load(data: &'static [u8], format: image::ImageFormat) -> ImageInner {
        super::load(ImageCacheKey::Invalid, ImageData::Static(data), format).unwrap().unwrap()
    }

    fn load_animated(
        data: &'static [u8],
        format: image::ImageFormat,
    ) -> vtable::VRc<super::super::OpaqueImageVTable, AnimatedImage> {
        match load(data, format) {
            ImageInner::AnimatedImage(animated) => animated,
            _ => panic!("expected an animated image"),
        }
    }

    fn top_left_pixel(buffer: &SharedPixelBuffer<Rgba8Pixel>) -> [u8; 3] {
        let p = buffer.as_slice()[0];
        [p.r, p.g, p.b]
    }

    fn still_top_left_pixel(buffer: &SharedImageBuffer) -> [u8; 3] {
        match buffer {
            SharedImageBuffer::RGB8(pixels) => {
                let p = pixels.as_slice()[0];
                [p.r, p.g, p.b]
            }
            SharedImageBuffer::RGBA8(pixels) | SharedImageBuffer::RGBA8Premultiplied(pixels) => {
                let p = pixels.as_slice()[0];
                [p.r, p.g, p.b]
            }
            #[allow(unreachable_patterns)]
            _ => panic!("unexpected pixel format"),
        }
    }

    fn frames(animated: &AnimatedImage) -> Vec<([u8; 3], Duration)> {
        let mut decoder = animated.frames().unwrap();
        core::iter::from_fn(|| decoder.next_frame())
            .map(|(buffer, delay)| (top_left_pixel(&buffer), delay))
            .collect()
    }

    fn expected_frames() -> Vec<([u8; 3], Duration)> {
        [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]]
            .into_iter()
            .zip([100, 200, 300, 400].map(Duration::from_millis))
            .collect()
    }

    #[test]
    fn gif_frames_and_delays() {
        let animated =
            load_animated(include_bytes!("testdata/frames.gif"), image::ImageFormat::Gif);
        assert_eq!(animated.size(), [8, 8].into());
        assert_eq!(
            still_top_left_pixel(&animated.first_frame().0.render_to_buffer(None).unwrap()),
            [255, 0, 0]
        );
        assert_eq!(frames(&animated), expected_frames());
        assert_eq!(animated.plays(), None);
    }

    #[test]
    fn webp_frames_and_delays() {
        let animated =
            load_animated(include_bytes!("testdata/frames.webp"), image::ImageFormat::WebP);
        // The decoder returns some channels as 254 instead of 255.
        let rounded = frames(&animated)
            .into_iter()
            .map(|(pixel, delay)| (pixel.map(|c| if c >= 254 { 255 } else { c }), delay))
            .collect::<Vec<_>>();
        assert_eq!(rounded, expected_frames());
        assert_eq!(animated.plays(), None);
    }

    #[test]
    fn apng_frames_and_plays() {
        let animated = load_animated(include_bytes!("testdata/twice.png"), image::ImageFormat::Png);
        assert_eq!(frames(&animated), expected_frames());
        assert_eq!(animated.plays().map(|plays| plays.get()), Some(2));
    }

    #[test]
    fn gif_plays_like_browsers() {
        // No NETSCAPE extension: one play.
        let once = load_animated(include_bytes!("testdata/once.gif"), image::ImageFormat::Gif);
        assert_eq!(once.plays().map(|plays| plays.get()), Some(1));
        // A NETSCAPE repeat count of 1: two plays.
        let twice = load_animated(include_bytes!("testdata/twice.gif"), image::ImageFormat::Gif);
        assert_eq!(twice.plays().map(|plays| plays.get()), Some(2));
    }

    #[test]
    fn gif_repeat_count_anywhere_in_the_file() {
        let mut gif = std::vec::Vec::from(*b"GIF89a");
        // Logical screen descriptor without a global color table.
        gif.extend([1, 0, 1, 0, 0, 0, 0]);
        // An image with a two-entry local color table, and its LZW data.
        gif.extend([0x2C, 0, 0, 0, 0, 1, 0, 1, 0, 0x80, 0, 0, 0, 0, 0, 0, 2, 2, 0x4C, 0x01, 0]);
        // A comment extension.
        gif.extend([0x21, 0xFE, 2, b'h', b'i', 0]);
        gif.extend([0x21, 0xFF, 11]);
        gif.extend(b"NETSCAPE2.0");
        gif.extend([3, 1, 5, 0, 0, 0x3B]);
        assert_eq!(super::decoding::gif_repeat_count(&gif), Some(5));
        assert_eq!(super::decoding::gif_repeat_count(&gif[..gif.len() - 6]), None);
        assert_eq!(super::decoding::gif_repeat_count(b"GIF89a"), None);
    }

    #[test]
    fn huge_canvas_is_rejected() {
        // 20000x20000 RGBA is 1.6 GB, above the `image` crate's default 512 MiB limit.
        let mut gif = include_bytes!("testdata/frames.gif").to_vec();
        gif[6..10].copy_from_slice(&[0x20, 0x4E, 0x20, 0x4E]);
        let mut webp = include_bytes!("testdata/frames.webp").to_vec();
        assert_eq!(&webp[12..16], b"VP8X");
        webp[24..30].copy_from_slice(&[0x1F, 0x4E, 0x00, 0x1F, 0x4E, 0x00]);
        for (data, format) in [(gif, image::ImageFormat::Gif), (webp, image::ImageFormat::WebP)] {
            assert!(
                super::load(ImageCacheKey::Invalid, ImageData::Borrowed(&data), format).is_err()
            );
            assert!(super::super::Image::load_from_data(&data, None).is_err());
        }
    }

    #[test]
    fn path_of_animated_image() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("graphics/image/testdata/twice.png");
        let image = super::super::Image::load_from_path(&path).unwrap();
        assert!(matches!(image.0, ImageInner::AnimatedImage(_)));
        assert_eq!(image.path(), Some(path.as_path()));
    }

    #[test]
    fn single_frame_gif_is_a_still_image() {
        let image = load(include_bytes!("testdata/single.gif"), image::ImageFormat::Gif);
        assert!(matches!(image, ImageInner::EmbeddedImage { .. }));
    }

    #[test]
    fn still_png_is_left_to_the_caller() {
        let mut png = Vec::new();
        image::RgbImage::new(2, 2)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let result =
            super::load(ImageCacheKey::Invalid, ImageData::Borrowed(&png), image::ImageFormat::Png);
        assert!(matches!(result, Ok(None)));
    }
}
