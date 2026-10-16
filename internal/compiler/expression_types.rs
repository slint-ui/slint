// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The types that both the expression tree and the LLR use:
//! units, operators, layout orientations, easing curves, cursors and image references

use crate::langtype::Type;
use smol_str::SmolStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum OperatorClass {
    ComparisonOp,
    LogicalOp,
    ArithmeticOp,
}

/// the class of for this (binary) operation
pub fn operator_class(op: char) -> OperatorClass {
    match op {
        '=' | '!' | '<' | '>' | '≤' | '≥' => OperatorClass::ComparisonOp,
        '&' | '|' => OperatorClass::LogicalOp,
        '+' | '-' | '/' | '*' => OperatorClass::ArithmeticOp,
        _ => panic!("Invalid operator {op:?}"),
    }
}

macro_rules! declare_units {
    // A unit written without a conversion is already its type's canonical unit.
    (@normalize $value:ident, $ident:ident) => { ($value, Unit::$ident) };
    // Otherwise scale by the factor and switch to the named canonical unit.
    (@normalize $value:ident, $ident:ident, $canon:ident, $factor:expr) => {
        ($value * ($factor as f64), Unit::$canon)
    };
    ($( $(#[$m:meta])* $ident:ident = $string:literal $(-> $canon:ident * $factor:expr)? ,)*) => {
        /// A unit as written after a number in the source (`px`, `cm`, `grad`, ...).
        ///
        /// These are all the units a user can type. A literal is normalized to the
        /// canonical [`Unit`] of its type the moment it enters the expression tree, so
        /// only the parser and tooling ever handle a `WrittenUnit`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter)]
        pub enum WrittenUnit {
            $($(#[$m])* $ident,)*
        }

        impl std::fmt::Display for WrittenUnit {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$ident => write!(f, $string), )*
                }
            }
        }

        impl std::str::FromStr for WrittenUnit {
            type Err = ();
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($string => Ok(Self::$ident), )*
                    _ => Err(())
                }
            }
        }

        impl WrittenUnit {
            /// Scale `value`, written in this surface unit, to the value and canonical
            /// [`Unit`] a `NumberLiteral` stores. The scale and the unit come out
            /// together so they can't drift apart.
            pub fn normalize(self, value: f64) -> (f64, Unit) {
                match self {
                    $(Self::$ident => declare_units!(@normalize value, $ident $(, $canon, $factor)?), )*
                }
            }
        }
    };
}

declare_units! {
    /// No unit was given
    None = "",
    /// Percent value
    Percent = "%",

    // Lengths or Coord

    /// Physical pixels
    Phx = "phx",
    /// Logical pixels
    Px = "px",
    /// Centimeters
    Cm = "cm" -> Px * 37.8,
    /// Millimeters
    Mm = "mm" -> Px * 3.78,
    /// inches
    In = "in" -> Px * 96,
    /// Points
    Pt = "pt" -> Px * 96./72.,
    /// Logical pixels multiplied with the window's default-font-size
    Rem = "rem",

    // durations

    /// Seconds
    S = "s" -> Ms * 1000,
    /// Milliseconds
    Ms = "ms",

    // angles

    /// Degree
    Deg = "deg",
    /// Gradians
    Grad = "grad" -> Deg * 360./180.,
    /// Turns
    Turn = "turn" -> Deg * 360.,
    /// Radians
    Rad = "rad" -> Deg * 360./std::f32::consts::TAU,
}

/// The unit a [`Expression::NumberLiteral`] carries: always the canonical unit of
/// its type, so the stored value is already scaled and needs no further
/// conversion. The units a user can type (`cm`, `pt`, `grad`, ...) are
/// [`WrittenUnit`] and are normalized to one of these on the way in.
///
/// [`Expression::NumberLiteral`]: crate::expression_tree::Expression::NumberLiteral
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Unit {
    /// Dimension-less (`float`, `int`)
    #[default]
    None,
    /// Percent
    Percent,
    /// Physical pixels
    Phx,
    /// Logical pixels
    Px,
    /// Logical pixels multiplied with the window's default-font-size
    Rem,
    /// Milliseconds
    Ms,
    /// Degrees
    Deg,
}

impl Unit {
    pub fn ty(self) -> Type {
        match self {
            Unit::None => Type::Float32,
            Unit::Percent => Type::Percent,
            Unit::Px => Type::LogicalLength,
            Unit::Phx => Type::PhysicalLength,
            Unit::Rem => Type::Rem,
            Unit::Ms => Type::Duration,
            Unit::Deg => Type::Angle,
        }
    }
}

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Unit::None => "",
            Unit::Percent => "%",
            Unit::Px => "px",
            Unit::Phx => "phx",
            Unit::Rem => "rem",
            Unit::Ms => "ms",
            Unit::Deg => "deg",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MinMaxOp {
    Min,
    Max,
}

#[derive(Clone, Debug, Default)]
pub enum EasingCurve {
    #[default]
    Linear,
    CubicBezier(f32, f32, f32, f32),
    EaseInElastic,
    EaseOutElastic,
    EaseInOutElastic,
    EaseInBounce,
    EaseOutBounce,
    EaseInOutBounce,
    Spring(f32),
    // CubicBezierNonConst([Box<Expression>; 4]),
    // Custom(Box<dyn Fn(f32)->f32>),
}

/// The compiled `mouse-cursor` value: either a built-in cursor or a custom one built from an
/// image. Generic over the expression type so both the tree and the LLR reuse the same shape.
#[derive(Clone, Debug)]
pub enum MouseCursorInner<E> {
    BuiltIn(Box<E>),
    CustomMouseCursor { image: Box<E>, hotspot_x: Box<E>, hotspot_y: Box<E> },
}

impl<E: Default> Default for MouseCursorInner<E> {
    fn default() -> Self {
        Self::BuiltIn(Box::default())
    }
}

// The compiler resolves every `@image-url("foo.png")` into a `Source` or
// `DataUri` reference; the resource lowering pass may then replace it with
// `EmbeddedData`/`EmbeddedTexture` if configured.
#[derive(Clone, Debug)]
pub enum ImageReference {
    None,
    Source(crate::source_path::SourcePath),
    /// An inline `data:` URI carrying the image content.
    DataUri(SmolStr),
    EmbeddedData {
        resource_id: crate::embedded_resources::EmbeddedResourcesIdx,
        extension: String,
    },
    EmbeddedTexture {
        resource_id: crate::embedded_resources::EmbeddedResourcesIdx,
    },
}

impl ImageReference {
    /// Classify a URL returned by the resource mapper. It is already a URL, so
    /// the only distinction is a `data:` URI (kept as a string, see
    /// [`Self::DataUri`]) from any other URL.
    pub fn from_mapped_url(url: url::Url) -> Self {
        if url.scheme() == "data" {
            Self::DataUri(url.as_str().into())
        } else {
            Self::Source(url.into())
        }
    }
}

#[derive(Clone, Debug, Copy, Eq, PartialEq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

impl Orientation {
    pub fn orthogonal(self) -> Self {
        match self {
            Orientation::Horizontal => Orientation::Vertical,
            Orientation::Vertical => Orientation::Horizontal,
        }
    }
}
