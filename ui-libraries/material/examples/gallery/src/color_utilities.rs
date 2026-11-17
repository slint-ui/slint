// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use material_color_utilities_rust::{DynamicScheme, Hct, rgba_from_argb};
use slint::Color;

/// Converts an ARGB u32 integer to Slint's `Color`.
pub fn to_slint_color(argb: u32) -> Color {
    let (r, g, b, a) = rgba_from_argb(argb);
    Color::from_argb_u8(a, r, g, b)
}

/// Maps a `DynamicScheme` from `material-color-utilities-rust` to Slint's `MaterialScheme`.
pub fn dynamic_to_slint_scheme(scheme: &DynamicScheme) -> crate::MaterialScheme {
    crate::MaterialScheme {
        background: to_slint_color(scheme.background()),
        error: to_slint_color(scheme.error()),
        errorContainer: to_slint_color(scheme.error_container()),
        inverseOnSurface: to_slint_color(scheme.inverse_on_surface()),
        inversePrimary: to_slint_color(scheme.inverse_primary()),
        inverseSurface: to_slint_color(scheme.inverse_surface()),
        onBackground: to_slint_color(scheme.on_background()),
        onError: to_slint_color(scheme.on_error()),
        onErrorContainer: to_slint_color(scheme.on_error_container()),
        onPrimary: to_slint_color(scheme.on_primary()),
        onPrimaryContainer: to_slint_color(scheme.on_primary_container()),
        onPrimaryFixed: to_slint_color(scheme.on_primary_fixed()),
        onPrimaryFixedVariant: to_slint_color(scheme.on_primary_fixed_variant()),
        onSecondary: to_slint_color(scheme.on_secondary()),
        onSecondaryContainer: to_slint_color(scheme.on_secondary_container()),
        onSecondaryFixed: to_slint_color(scheme.on_secondary_fixed()),
        onSecondaryFixedVariant: to_slint_color(scheme.on_secondary_fixed_variant()),
        onSurface: to_slint_color(scheme.on_surface()),
        onSurfaceVariant: to_slint_color(scheme.on_surface_variant()),
        onTertiary: to_slint_color(scheme.on_tertiary()),
        onTertiaryContainer: to_slint_color(scheme.on_tertiary_container()),
        onTertiaryFixed: to_slint_color(scheme.on_tertiary_fixed()),
        onTertiaryFixedVariant: to_slint_color(scheme.on_tertiary_fixed_variant()),
        outline: to_slint_color(scheme.outline()),
        outlineVariant: to_slint_color(scheme.outline_variant()),
        primary: to_slint_color(scheme.primary()),
        primaryContainer: to_slint_color(scheme.primary_container()),
        primaryFixed: to_slint_color(scheme.primary_fixed()),
        primaryFixedDim: to_slint_color(scheme.primary_fixed_dim()),
        scrim: to_slint_color(scheme.scrim()),
        secondary: to_slint_color(scheme.secondary()),
        secondaryContainer: to_slint_color(scheme.secondary_container()),
        secondaryFixed: to_slint_color(scheme.secondary_fixed()),
        secondaryFixedDim: to_slint_color(scheme.secondary_fixed_dim()),
        shadow: to_slint_color(scheme.shadow()),
        surface: to_slint_color(scheme.surface()),
        surfaceBright: to_slint_color(scheme.surface_bright()),
        surfaceContainer: to_slint_color(scheme.surface_container()),
        surfaceContainerHigh: to_slint_color(scheme.surface_container_high()),
        surfaceContainerHighest: to_slint_color(scheme.surface_container_highest()),
        surfaceContainerLow: to_slint_color(scheme.surface_container_low()),
        surfaceContainerLowest: to_slint_color(scheme.surface_container_lowest()),
        surfaceDim: to_slint_color(scheme.surface_dim()),
        surfaceTint: to_slint_color(scheme.surface_tint()),
        surfaceVariant: to_slint_color(scheme.surface_variant()),
        tertiary: to_slint_color(scheme.tertiary()),
        tertiaryContainer: to_slint_color(scheme.tertiary_container()),
        tertiaryFixed: to_slint_color(scheme.tertiary_fixed()),
        tertiaryFixedDim: to_slint_color(scheme.tertiary_fixed_dim()),
    }
}

/// Generates light and dark `MaterialSchemes` from an ARGB seed color using M3 DynamicScheme.
pub fn generate_slint_schemes(seed_argb: u32) -> crate::MaterialSchemes {
    let seed = Hct::from_int(seed_argb);
    let light_dynamic = DynamicScheme::tonal_spot(seed, false, 0.0);
    let dark_dynamic = DynamicScheme::tonal_spot(seed, true, 0.0);

    crate::MaterialSchemes {
        light: dynamic_to_slint_scheme(&light_dynamic),
        dark: dynamic_to_slint_scheme(&dark_dynamic),
    }
}
