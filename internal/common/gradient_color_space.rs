// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// The color space gradient stops are interpolated in.
///
/// `Display` and `FromStr` use the CSS name, as in a gradient's `in <space>` clause.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GradientColorSpace {
    /// Interpolate in sRGB, the default.
    #[default]
    Srgb,
    /// Interpolate in the OKLCH color space.
    Oklch,
    /// Interpolate in the OKLab color space.
    Oklab,
    /// Interpolate in the HSL color space.
    Hsl,
}

impl GradientColorSpace {
    /// The `in <space> ` clause that selects this color space at the start of a gradient,
    /// or nothing for the default sRGB.
    pub fn css_prefix(self) -> impl core::fmt::Display {
        struct CssPrefix(GradientColorSpace);
        impl core::fmt::Display for CssPrefix {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                match self.0 {
                    GradientColorSpace::Srgb => Ok(()),
                    space => write!(f, "in {space} "),
                }
            }
        }
        CssPrefix(self)
    }
}

impl core::fmt::Display for GradientColorSpace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            GradientColorSpace::Srgb => "srgb",
            GradientColorSpace::Oklch => "oklch",
            GradientColorSpace::Oklab => "oklab",
            GradientColorSpace::Hsl => "hsl",
        })
    }
}

impl core::str::FromStr for GradientColorSpace {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "srgb" => Ok(GradientColorSpace::Srgb),
            "oklch" => Ok(GradientColorSpace::Oklch),
            "oklab" => Ok(GradientColorSpace::Oklab),
            "hsl" => Ok(GradientColorSpace::Hsl),
            _ => Err(()),
        }
    }
}

#[test]
fn display_and_from_str() {
    for (space, name) in [
        (GradientColorSpace::Srgb, "srgb"),
        (GradientColorSpace::Oklch, "oklch"),
        (GradientColorSpace::Oklab, "oklab"),
        (GradientColorSpace::Hsl, "hsl"),
    ] {
        assert_eq!(alloc::format!("{space}"), name);
        assert_eq!(name.parse::<GradientColorSpace>(), Ok(space));
    }
    assert_eq!("lab".parse::<GradientColorSpace>(), Err(()));
}

#[test]
fn css_prefix() {
    assert_eq!(alloc::format!("{}", GradientColorSpace::Srgb.css_prefix()), "");
    assert_eq!(alloc::format!("{}", GradientColorSpace::Oklch.css_prefix()), "in oklch ");
}
