// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once
#include <algorithm>
#include <string_view>
#include <limits>
#include <cstdint>
#include "private/slint_color.h"
#include "private/slint_brush_internal.h"
#include "private/slint_string.h"
#include "private/slint_builtin_functions.h"

namespace slint {

namespace private_api {

using cbindgen_private::types::GradientColorSpace;
using cbindgen_private::types::GradientStop;

/// Encodes \a colorSpace for storage in a fake header GradientStop's position field.
/// Must match `GradientColorSpace::encode` in `internal/core/graphics/brush.rs`.
inline float encode_gradient_color_space(GradientColorSpace colorSpace)
{
    return static_cast<float>(static_cast<uint8_t>(colorSpace));
}

/// \private
/// LinearGradientBrush represents a gradient for a brush that is a linear sequence of color stops,
/// that are aligned at a specific angle.
///
/// Internally the first two fake GradientStop entries encode the angle (index 0) and the
/// color space (index 1). Real color stops begin at index 2.
class LinearGradientBrush
{
public:
    /// Constructs an empty linear gradient with no color stops.
    LinearGradientBrush() = default;
    /// Constructs a new linear gradient with the specified \a angle and \a colorSpace. The color
    /// stops will be constructed from the stops array pointed to by \a firstStop, with the
    /// length \a stopCount.
    LinearGradientBrush(float angle, GradientColorSpace colorSpace, const GradientStop *firstStop,
                        int stopCount)
        : inner(make_linear_gradient(angle, colorSpace, firstStop, stopCount))
    {
    }

    /// Returns the linear gradient's angle in degrees.
    float angle() const
    {
        // The gradient's first stop is a fake stop to store the angle
        return inner[0].position;
    }

    /// Returns the number of gradient stops.
    int stopCount() const { return int(inner.size()) - 2; }

    /// Returns a pointer to the first gradient stop; undefined if the gradient has not stops.
    const GradientStop *stopsBegin() const { return inner.begin() + 2; }
    /// Returns a pointer past the last gradient stop. The returned pointer cannot be dereferenced,
    /// it can only be used for comparison.
    const GradientStop *stopsEnd() const { return inner.end(); }

private:
    cbindgen_private::types::LinearGradientBrush inner;

    friend class slint::Brush;

    static SharedVector<private_api::GradientStop>
    make_linear_gradient(float angle, GradientColorSpace colorSpace, const GradientStop *firstStop,
                         int stopCount)
    {
        SharedVector<private_api::GradientStop> gradient;
        gradient.push_back({ Color::from_argb_encoded(0).inner, angle });
        gradient.push_back(
                { Color::from_argb_encoded(0).inner, encode_gradient_color_space(colorSpace) });
        for (int i = 0; i < stopCount; ++i, ++firstStop)
            gradient.push_back(*firstStop);
        return gradient;
    }
};

/// \private
/// RadialGradientBrush represents a circular or elliptical gradient.
class RadialGradientBrush
{
public:
    /// Constructs an empty radial gradient with no color stops.
    RadialGradientBrush() = default;
    /// Constructs a new radial gradient of the given \a shape and \a colorSpace whose radii
    /// derive from the bounding box.
    RadialGradientBrush(GradientColorSpace colorSpace, const GradientStop *firstStop,
                        int stopCount, float center_x, float center_y, float shape)
    {
        init(colorSpace, firstStop, stopCount, center_x, center_y,
             cbindgen_private::types::RADIAL_GRADIENT_DEFAULT_RADIUS,
             cbindgen_private::types::RADIAL_GRADIENT_DEFAULT_RADIUS, shape);
    }

    /// Constructs a new radial gradient of the given \a shape and \a colorSpace with explicit
    /// radii. Negative radii are treated as 0.
    RadialGradientBrush(GradientColorSpace colorSpace, const GradientStop *firstStop,
                        int stopCount, float center_x, float center_y, float radius_x,
                        float radius_y, float shape)
    {
        init(colorSpace, firstStop, stopCount, center_x, center_y, std::max(0.f, radius_x),
             std::max(0.f, radius_y), shape);
    }

    /// Returns the number of gradient stops.
    int stopCount() const { return int(inner.size()) - 6; }

    /// Returns a pointer to the first gradient stop; undefined if the gradient has not stops.
    const GradientStop *stopsBegin() const { return inner.begin() + 6; }
    /// Returns a pointer past the last gradient stop. The returned pointer cannot be dereferenced,
    /// it can only be used for comparison.
    const GradientStop *stopsEnd() const { return inner.end(); }

private:
    void init(GradientColorSpace colorSpace, const GradientStop *firstStop, int stopCount,
              float center_x, float center_y, float radius_x, float radius_y, float shape)
    {
        inner.push_back({ Color::from_argb_encoded(0).inner, center_x });
        inner.push_back({ Color::from_argb_encoded(0).inner, center_y });
        inner.push_back({ Color::from_argb_encoded(0).inner, radius_x });
        inner.push_back({ Color::from_argb_encoded(0).inner, radius_y });
        inner.push_back({ Color::from_argb_encoded(0).inner, shape });
        inner.push_back(
                { Color::from_argb_encoded(0).inner, encode_gradient_color_space(colorSpace) });
        for (int i = 0; i < stopCount; ++i, ++firstStop)
            inner.push_back(*firstStop);
    }

    cbindgen_private::types::RadialGradientBrush inner;

    friend class slint::Brush;
};

/// \private
/// ConicGradientBrush represents a conic gradient that rotates around a center point.
///
/// Internally the first four fake GradientStop entries encode the starting angle
/// (index 0), center-x (index 1), center-y (index 2), and color space (index 3). Real
/// color stops begin at index 4. NaN center values mean "use bounding-box center".
class ConicGradientBrush
{
public:
    /// Constructs an empty conic gradient with no color stops.
    ConicGradientBrush() = default;
    /// Constructs a new conic gradient with the specified starting \a angle and \a colorSpace.
    /// The color stops will be constructed from the stops array pointed to be \a firstStop, with
    /// the length \a stopCount.
    ConicGradientBrush(float angle, GradientColorSpace colorSpace, const GradientStop *firstStop,
                       int stopCount)
    {
        // Header: angle, center_x (NaN=bbox), center_y (NaN=bbox), color space
        inner.push_back({ Color::from_argb_encoded(0).inner, angle });
        inner.push_back(
                { Color::from_argb_encoded(0).inner, std::numeric_limits<float>::quiet_NaN() });
        inner.push_back(
                { Color::from_argb_encoded(0).inner, std::numeric_limits<float>::quiet_NaN() });
        inner.push_back(
                { Color::from_argb_encoded(0).inner, encode_gradient_color_space(colorSpace) });
        for (int i = 0; i < stopCount; ++i, ++firstStop)
            inner.push_back(*firstStop);

        // Normalize stops to [0, 1] range with proper boundary stops
        cbindgen_private::types::slint_conic_gradient_normalize_stops(&inner);

        // Apply rotation if angle is non-zero
        if (angle != 0.0f) {
            cbindgen_private::types::slint_conic_gradient_apply_rotation(&inner, angle);
        }
    }

    /// Constructs a new conic gradient with an explicit center.
    ConicGradientBrush(float angle, GradientColorSpace colorSpace, const GradientStop *firstStop,
                       int stopCount, float center_x, float center_y)
    {
        // Header: angle, center_x, center_y, color space
        inner.push_back({ Color::from_argb_encoded(0).inner, angle });
        inner.push_back({ Color::from_argb_encoded(0).inner, center_x });
        inner.push_back({ Color::from_argb_encoded(0).inner, center_y });
        inner.push_back(
                { Color::from_argb_encoded(0).inner, encode_gradient_color_space(colorSpace) });
        for (int i = 0; i < stopCount; ++i, ++firstStop)
            inner.push_back(*firstStop);

        // Normalize stops to [0, 1] range with proper boundary stops
        cbindgen_private::types::slint_conic_gradient_normalize_stops(&inner);

        // Apply rotation if angle is non-zero
        if (angle != 0.0f) {
            cbindgen_private::types::slint_conic_gradient_apply_rotation(&inner, angle);
        }
    }

    /// Returns the conic gradient's starting angle (rotation) in degrees.
    float angle() const { return inner[0].position; }

    /// Returns the number of gradient stops.
    int stopCount() const { return int(inner.size()) - 4; }

    /// Returns a pointer to the first gradient stop; undefined if the gradient has not stops.
    const GradientStop *stopsBegin() const { return inner.begin() + 4; }
    /// Returns a pointer past the last gradient stop. The returned pointer cannot be dereferenced,
    /// it can only be used for comparison.
    const GradientStop *stopsEnd() const { return inner.end(); }

private:
    cbindgen_private::types::ConicGradientBrush inner;

    friend class slint::Brush;
};

}

/// Brush is used to declare how to fill or outline shapes, such as rectangles, paths or text. A
/// brush is either a solid color or a linear gradient.
///
/// Copies of a Brush share the same underlying data, so copying is cheap.
class Brush
{
public:
    /// Constructs a new brush that is a transparent color.
    Brush() : Brush(Color {}) { }
    /// Constructs a new brush that is of color \a color.
    Brush(const Color &color) : data(Inner::SolidColor(color.inner)) { }
    /// \private
    /// Constructs a new brush that is the gradient \a gradient.
    Brush(const private_api::LinearGradientBrush &gradient)
        : data(Inner::LinearGradient(gradient.inner))
    {
    }

    /// \private
    /// Constructs a new brush that is the gradient \a gradient.
    Brush(const private_api::RadialGradientBrush &gradient)
        : data(Inner::RadialGradient(gradient.inner))
    {
    }

    /// \private
    /// Constructs a new brush that is the gradient \a gradient.
    Brush(const private_api::ConicGradientBrush &gradient)
        : data(Inner::ConicGradient(gradient.inner))
    {
    }

    /// Returns the color of the brush. If the brush is a gradient, this function returns the color
    /// of the first stop.
    inline Color color() const;

    /// Returns a new version of this brush that has the brightness increased
    /// by the specified factor. This is done by calling Color::brighter on
    /// all the colors of this brush.
    [[nodiscard]] inline Brush brighter(float factor) const;
    /// Returns a new version of this color that has the brightness decreased
    /// by the specified factor. This is done by calling Color::darker on
    /// all the colors of this brush.
    [[nodiscard]] inline Brush darker(float factor) const;

    /// Returns a new version of this brush with the opacity decreased by \a factor.
    ///
    /// This is done by calling Color::transparentize on all the colors of this brush.
    [[nodiscard]] inline Brush transparentize(float factor) const;

    /// Returns a new version of this brush with the related color's opacities
    /// set to \a alpha.
    [[nodiscard]] inline Brush with_alpha(float alpha) const;

    /// Returns true if \a a is equal to \a b. If \a a holds a color, then \a b must also hold a
    /// color that is identical to \a a's color. If it holds a gradient, then the gradients must be
    /// identical. Returns false if the brushes differ in what they hold or their respective color
    /// or gradient are not equal.
    ///
    /// \note Radial and conic gradient brushes store their center as fake header stops whose
    /// position fields use NaN as a sentinel for "use the bounding box center". Two brushes with
    /// a NaN center compare equal, matching the Rust \c PartialEq semantics. A plain memory
    /// comparison would treat NaN != NaN and give incorrect results, hence this custom function.
    friend bool operator==(const Brush &a, const Brush &b)
    {
        return cbindgen_private::types::slint_brush_compare_equal(&a.data, &b.data);
    }
    /// Returns true if \a a is not equal to \a b; false otherwise.
    friend bool operator!=(const Brush &a, const Brush &b) { return !(a == b); }

private:
    using Tag = cbindgen_private::types::Brush::Tag;
    using Inner = cbindgen_private::types::Brush;
    Inner data;
    friend struct private_api::Property<Brush>;
};

Color Brush::color() const
{
    Color result;
    switch (data.tag) {
    case Tag::SolidColor:
        result.inner = data.solid_color._0;
        break;
    case Tag::LinearGradient:
        if (data.linear_gradient._0.size() > 2) {
            result.inner = data.linear_gradient._0[2].color;
        }
        break;
    case Tag::RadialGradient:
        // The first 6 stops are the fake header: center_x, center_y, radius_x, radius_y, shape,
        // color space.
        if (data.radial_gradient._0.size() > 6) {
            result.inner = data.radial_gradient._0[6].color;
        }
        break;
    case Tag::ConicGradient:
        // First 4 stops are the fake header (angle, center_x, center_y, color space); real stops
        // start at 4.
        if (data.conic_gradient._0.size() > 4) {
            result.inner = data.conic_gradient._0[4].color;
        }
        break;
    }
    return result;
}

inline Brush Brush::brighter(float factor) const
{
    return private_api::call_builtin(cbindgen_private::slint_builtin_color_brighter, *this, factor);
}

inline Brush Brush::darker(float factor) const
{
    return private_api::call_builtin(cbindgen_private::slint_builtin_color_darker, *this, factor);
}

inline Brush Brush::transparentize(float factor) const
{
    return private_api::call_builtin(cbindgen_private::slint_builtin_color_transparentize, *this,
                                     factor);
}

inline Brush Brush::with_alpha(float alpha) const
{
    return private_api::call_builtin(cbindgen_private::slint_builtin_color_with_alpha, *this,
                                     alpha);
}

namespace private_api {

template<>
inline void Property<slint::Brush>::set_animated_value(
        const slint::Brush &new_value,
        const cbindgen_private::PropertyAnimation &animation_data) const
{
    cbindgen_private::slint_property_set_animated_value_brush(&inner, &get(), &new_value,
                                                              &animation_data);
}

} // namespace private_api

} // namespace slint
