// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![no_std]
#![forbid(unsafe_code)]
#![forbid(missing_docs)]

/// The size of a window, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Size {
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
}

impl Size {
    /// Construct a size from a width and a height.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[test]
fn test_size() {
    let size = Size::new(320, 240);
    assert_eq!((size.width, size.height), (320, 240));
    assert_eq!(size, Size { width: 320, height: 240 });
    assert_ne!(size, Size::new(240, 320));
    // The default size is empty
    assert_eq!(Size::default(), Size::new(0, 0));
    assert_eq!(Sink::format(format_args!("{size:?}")).as_str(), "Size { width: 320, height: 240 }");
}

/// An RGBA color, as held by properties of the `color` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Color {
    /// Construct a color from its ARGB value, e.g. `0xff123456`.
    pub const fn from_argb_encoded(argb: u32) -> Self {
        let [alpha, red, green, blue] = argb.to_be_bytes();
        Self { red, green, blue, alpha }
    }

    /// Construct a fully opaque color from its red, green, and blue channels.
    pub const fn from_rgb_u8(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue, alpha: 0xff }
    }

    /// The red channel, from 0 to 255.
    pub const fn red(self) -> u8 {
        self.red
    }

    /// The green channel, from 0 to 255.
    pub const fn green(self) -> u8 {
        self.green
    }

    /// The blue channel, from 0 to 255.
    pub const fn blue(self) -> u8 {
        self.blue
    }

    /// The alpha channel: the color's opacity, from 0 for a fully transparent
    /// color to 255 for a fully opaque one.
    pub const fn alpha(self) -> u8 {
        self.alpha
    }

    /// Returns this color composited over `destination`, the Porter-Duff *over*
    /// operation.
    ///
    /// ```
    /// use slint_sc::Color;
    ///
    /// let red = Color::from_rgb_u8(0xff, 0, 0);
    ///
    /// // A fully transparent color leaves the destination as it was
    /// assert_eq!(Color::default().composite_over(red), red);
    ///
    /// // A fully opaque one replaces it
    /// let green = Color::from_rgb_u8(0, 0xff, 0);
    /// assert_eq!(green.composite_over(red), green);
    ///
    /// // Half-transparent blue over opaque red keeps half of each, and the
    /// // two halves round apart
    /// let half_blue = Color::from_argb_encoded(0x800000ff);
    /// assert_eq!(half_blue.composite_over(red), Color::from_rgb_u8(127, 0, 128));
    ///
    /// // Compositing two transparent colors has nothing to show
    /// assert_eq!(Color::default().composite_over(Color::default()), Color::default());
    /// ```
    pub fn composite_over(self, destination: Self) -> Self {
        let alpha = self.alpha as u32;
        // How much each color contributes, both over a denominator of
        // 255 * 255, which keeps the channels and the alpha exact over one
        // common divisor instead of rounding the weights separately
        let src_weight: u32 = alpha * 255;
        let dst_weight: u32 = destination.alpha as u32 * (255 - alpha);
        let total: u32 = src_weight + dst_weight;
        // Neither color contributes anything, so there's nothing to weigh the
        // channels by and the result is transparent
        if total == 0 {
            return Self::default();
        }
        // Both weights are at most 255 * 255 and so is their total, which
        // bounds a channel's numerator by 255 * 255 * 255: well within a u32,
        // and the quotient within a u8. Adding half the divisor first rounds
        // to the nearest integer.
        let channel = |src: u8, dst: u8| {
            ((src as u32 * src_weight + dst as u32 * dst_weight + total / 2) / total) as u8
        };
        Self {
            red: channel(self.red, destination.red),
            green: channel(self.green, destination.green),
            blue: channel(self.blue, destination.blue),
            alpha: ((total + 127) / 255) as u8,
        }
    }
}

#[test]
fn test_color() {
    let c = Color::from_argb_encoded(0x87123456);
    assert_eq!((c.red(), c.green(), c.blue(), c.alpha()), (0x12, 0x34, 0x56, 0x87));
    assert_eq!(Color::from_rgb_u8(0x12, 0x34, 0x56).alpha(), 0xff);
    // The default color is transparent
    assert_eq!(Color::default().alpha(), 0);
}

#[test]
fn test_composite_over() {
    let destination = Color::from_rgb_u8(0, 255, 200);
    // A fully opaque color is the result itself, a fully transparent one
    // leaves the destination as it was: the rounding never drifts at either end
    assert_eq!(
        Color::from_argb_encoded(0xffff000a).composite_over(destination),
        Color::from_rgb_u8(255, 0, 10)
    );
    assert_eq!(Color::from_argb_encoded(0x00ff000a).composite_over(destination), destination);
    // Halfway between, each channel rounds to the nearest whole number, and
    // the channels don't mix into one another
    assert_eq!(
        Color::from_argb_encoded(0x80ff000a).composite_over(destination),
        Color::from_rgb_u8(128, 127, 105)
    );
    // The brightest possible result still fits in a u8
    assert_eq!(
        Color::from_argb_encoded(0x80ffffff).composite_over(Color::from_rgb_u8(255, 255, 255)),
        Color::from_rgb_u8(255, 255, 255)
    );
    // Over a translucent destination the result keeps an alpha of its own: half
    // over half leaves three quarters covered
    assert_eq!(
        Color::from_argb_encoded(0x80ff0000).composite_over(Color::from_argb_encoded(0x800000ff)),
        Color::from_argb_encoded(0xc0aa0055)
    );
    // With nothing to composite, the result is transparent
    assert_eq!(Color::default().composite_over(Color::default()), Color::default());
}

#[test]
fn test_composite_over_matches_the_specified_formula() {
    // Over an opaque destination, the case rendering is specified for, every
    // channel comes out as the specified `(src * alpha + dst * (255 - alpha) +
    // 127) / 255`, for every channel value and every alpha rather than only
    // the ones the test cases happen to paint.
    //#sls.paint.blend.formula
    for alpha in 0..=255u32 {
        for src in 0..=255u32 {
            for dst in 0..=255u32 {
                let color = Color::from_argb_encoded((alpha << 24) | (src << 16));
                let got = color.composite_over(Color::from_rgb_u8(dst as u8, 0, 0)).red();
                let weighted = src * alpha + dst * (255 - alpha);
                let expected = ((weighted + 127) / 255) as u8;
                assert_eq!(got, expected, "alpha {alpha}, src {src}, dst {dst}");
                // And that is the nearest whole number, as specified: the
                // remainder is itself a whole number, so it never lands on an
                // exact half that could round either way
                let nearest = (weighted / 255 + u32::from(weighted % 255 >= 128)) as u8;
                assert_eq!(expected, nearest, "alpha {alpha}, src {src}, dst {dst}");
            }
        }
    }
}

/// A position in the window, in pixels: the origin is the top-left corner of
/// the window, x grows to the right, and y grows downwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    /// The distance from the left edge of the window.
    pub x: i32,
    /// The distance from the top edge of the window.
    pub y: i32,
}

impl Point {
    /// Construct a position from its distance to the left and top edges.
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// A touch of the display, delivered to the generated `dispatch_touch_event`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TouchEvent {
    /// A finger touched the display.
    #[non_exhaustive]
    Pressed {
        /// Where the finger touched.
        position: Point,
    },
    /// The finger lifted off the display.
    #[non_exhaustive]
    Released {
        /// Where the finger lifted off.
        position: Point,
    },
}

impl TouchEvent {
    /// A finger touching the display at `position`.
    pub const fn pressed(position: Point) -> Self {
        Self::Pressed { position }
    }

    /// A finger lifting off the display at `position`.
    pub const fn released(position: Point) -> Self {
        Self::Released { position }
    }
}

/// An image, as held by properties of the `image` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Image {
    /// No image, the value of an `image` property without one.
    #[default]
    None,
    /// An image decoded at compile time into a static array of packed bytes.
    /// Bytes rather than [`Color`] values so that the generated code can
    /// carry the image in a byte-string literal, which parses much faster
    /// than an array of per-pixel constructor calls.
    StaticArgb {
        /// Four bytes per pixel, alpha, red, green, and blue, the pixels row
        /// by row from the top-left corner.
        argb: &'static [u8],
        /// The number of pixels in each row.
        width: usize,
    },
}

impl Image {
    /// The width in pixels. An [`Image::None`] has a width of 0.
    ///
    /// ```
    /// use slint_sc::Image;
    ///
    /// assert_eq!(Image::None.width(), 0);
    ///
    /// let image = Image::StaticArgb { argb: &[0x80; 24], width: 3 };
    /// assert_eq!(image.width(), 3);
    /// ```
    pub const fn width(self) -> usize {
        match self {
            Self::None => 0,
            Self::StaticArgb { width, .. } => width,
        }
    }

    /// The height in pixels, derived from the pixel count and the width: an
    /// incomplete last pixel or row is not counted. An [`Image::None`], and
    /// an [`Image::StaticArgb`] with a width of 0, have a height of 0.
    ///
    /// ```
    /// use slint_sc::Image;
    ///
    /// assert_eq!(Image::None.height(), 0);
    ///
    /// let image = Image::StaticArgb { argb: &[0x80; 24], width: 3 };
    /// assert_eq!(image.height(), 2);
    /// ```
    pub const fn height(self) -> usize {
        match self {
            Self::None => 0,
            Self::StaticArgb { width: 0, .. } => 0,
            Self::StaticArgb { argb, width } => argb.len() / 4 / width,
        }
    }
}

#[test]
fn test_touch_event() {
    let pressed = TouchEvent::pressed(Point::new(3, -4));
    assert_eq!(pressed, TouchEvent::Pressed { position: Point { x: 3, y: -4 } });
    // A press and a release of the same position are different events
    assert_ne!(pressed, TouchEvent::released(Point::new(3, -4)));
    // The default position is the origin
    assert_eq!(Point::default(), Point::new(0, 0));
    assert_eq!(
        Sink::format(format_args!("{pressed:?}")).as_str(),
        "Pressed { position: Point { x: 3, y: -4 } }"
    );
}

#[test]
fn test_image() {
    // The default image is no image
    //#sls.gen.prop.types.image
    assert_eq!(Image::default(), Image::None);
    assert_eq!((Image::None.width(), Image::None.height()), (0, 0));

    // Four bytes per pixel, so six pixels of bytes make a 2x3 image
    let image = Image::StaticArgb { argb: &[0x80; 24], width: 2 };
    assert_eq!((image.width(), image.height()), (2, 3));

    // Incomplete trailing pixels and rows don't count towards the height,
    // and a width of 0 derives a height of 0 rather than dividing by it
    assert_eq!(Image::StaticArgb { argb: &[0x80; 23], width: 2 }.height(), 2);
    assert_eq!(Image::StaticArgb { argb: &[0x80; 24], width: 4 }.height(), 1);
    assert_eq!(Image::StaticArgb { argb: &[0x80; 24], width: 0 }.height(), 0);

    // The image is Copy and compares by its parts
    let copy = image;
    assert_eq!(copy, image);
    assert_ne!(copy, Image::None);
}

/// Error returned by the generated render functions.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RenderError {
    /// The frame buffer size doesn't match the size of the window.
    InvalidFrameBufferSize,
}

impl core::fmt::Display for RenderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidFrameBufferSize => {
                f.write_str("the frame buffer size doesn't match the size of the window")
            }
        }
    }
}

impl core::error::Error for RenderError {}

#[test]
fn test_render_error_display() {
    assert_eq!(
        Sink::format(format_args!("{}", RenderError::InvalidFrameBufferSize)).as_str(),
        "the frame buffer size doesn't match the size of the window"
    );
}

/// Names this crate's version so that generated code built for another version
/// fails to compile against it.
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub struct VersionCheck_1_18_0;

/// Module only meant to be used by the code generated by the Slint SC compiler.
#[doc(hidden)]
pub mod private_unstable_api {
    /// Painting into a frame buffer.
    pub mod renderer;
}

/// A sink that captures formatted output in a fixed buffer, so that a test can
/// assert on a `Debug` or `Display` implementation without an allocator.
#[cfg(test)]
struct Sink {
    buf: [u8; 128],
    len: usize,
}

#[cfg(test)]
impl core::fmt::Write for Sink {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        self.buf[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[cfg(test)]
impl Sink {
    /// The formatted arguments, held by the returned sink.
    fn format(args: core::fmt::Arguments<'_>) -> Self {
        use core::fmt::Write;
        let mut sink = Self { buf: [0; 128], len: 0 };
        sink.write_fmt(args).unwrap();
        sink
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap()
    }
}
