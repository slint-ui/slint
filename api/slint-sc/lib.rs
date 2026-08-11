// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Software-3.0

#![doc = include_str!("README.md")]
#![no_std]
#![forbid(unsafe_code)]
#![forbid(missing_docs)]

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

/// Error returned by the generated render functions.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RenderError {
    /// The frame buffer size doesn't match the requested width and height.
    InvalidFrameBufferSize,
}

impl core::fmt::Display for RenderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidFrameBufferSize => {
                f.write_str("the frame buffer size doesn't match the requested width and height")
            }
        }
    }
}

impl core::error::Error for RenderError {}

#[test]
fn test_render_error_display() {
    use core::fmt::Write;
    // A no_std, no_alloc sink to capture the Display output.
    struct Sink {
        buf: [u8; 80],
        len: usize,
    }
    impl Write for Sink {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let end = self.len + s.len();
            self.buf[self.len..end].copy_from_slice(s.as_bytes());
            self.len = end;
            Ok(())
        }
    }
    let mut sink = Sink { buf: [0; 80], len: 0 };
    write!(sink, "{}", RenderError::InvalidFrameBufferSize).unwrap();
    assert_eq!(
        core::str::from_utf8(&sink.buf[..sink.len]).unwrap(),
        "the frame buffer size doesn't match the requested width and height"
    );
}

/// Module only meant to be used by the code generated by the Slint SC compiler.
#[doc(hidden)]
pub mod private_unstable_api {
    /// Painting into a frame buffer.
    pub mod renderer;
}
