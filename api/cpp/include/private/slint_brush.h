// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once
#include <string_view>
#include <limits>
#include "private/slint_color.h"
#include "private/slint_brush_internal.h"
#include "private/slint_string.h"

namespace slint {

namespace private_api {

using cbindgen_private::types::GradientStop;

/// \private
/// LinearGradientBrush represents a gradient for a brush that is a linear sequence of color stops,
/// that are aligned at a specific angle.
class LinearGradientBrush
{
public:
    /// Constructs an empty linear gradient with no color stops.
    LinearGradientBrush() = default;
    /// Constructs a new linear gradient with the specified \a angle. The color stops will be
    /// constructed from the stops array pointed to by \a firstStop, with the length \a stopCount.
    LinearGradientBrush(float angle, const GradientStop *firstStop, int stopCount)
        : inner(make_linear_gradient(angle, firstStop, stopCount))
    {
    }

    /// Returns the linear gradient's angle in degrees.
    float angle() const
    {
        // The gradient's first stop is a fake stop to store the angle
        return inner[0].position;
    }

    /// Returns the number of gradient stops.
    int stopCount() const { return int(inner.size()) - 1; }

    /// Returns a pointer to the first gradient stop; undefined if the gradient has not stops.
    const GradientStop *stopsBegin() const { return inner.begin() + 1; }
    /// Returns a pointer past the last gradient stop. The returned pointer cannot be dereferenced,
    /// it can only be used for comparison.
    const GradientStop *stopsEnd() const { return inner.end(); }

private:
    cbindgen_private::types::LinearGradientBrush inner;

    friend class slint::Brush;

    static SharedVector<private_api::GradientStop>
    make_linear_gradient(float angle, const GradientStop *firstStop, int stopCount)
    {
        SharedVector<private_api::GradientStop> gradient;
        gradient.push_back({ Color::from_argb_encoded(0).inner, angle });
        for (int i = 0; i < stopCount; ++i, ++firstStop)
            gradient.push_back(*firstStop);
        return gradient;
    }
};

/// \private
/// RadialGradientBrush represents a circular gradient.
///
/// Internally the brush encodes center-x, center-y, and radius as the first three
/// fake GradientStop entries (positions 0–2), following the same pattern as
/// LinearGradientBrush (which stores the angle as stop 0). Real color stops begin at
/// index 3. NaN center values mean "use bounding-box center"; a negative radius means
/// "use half the bounding-box diagonal".
class RadialGradientBrush
{
public:
    /// Constructs an empty radial gradient with no color stops.
    RadialGradientBrush() = default;
    /// Constructs a new circular radial gradient centered in the bounding box.
    /// The color stops will be
    /// constructed from the stops array pointed to be \a firstStop, with the length \a stopCount.
    RadialGradientBrush(const GradientStop *firstStop, int stopCount)
    {
        // Header: center_x (NaN=bbox), center_y (NaN=bbox), radius (negative=bbox diagonal/2)
        inner.push_back(
                { Color::from_argb_encoded(0).inner, std::numeric_limits<float>::quiet_NaN() });
        inner.push_back(
                { Color::from_argb_encoded(0).inner, std::numeric_limits<float>::quiet_NaN() });
        inner.push_back({ Color::from_argb_encoded(0).inner, -1.0f });
        for (int i = 0; i < stopCount; ++i, ++firstStop)
            inner.push_back(*firstStop);
    }

    /// Constructs a new circular radial gradient with an explicit center and radius.
    RadialGradientBrush(const GradientStop *firstStop, int stopCount, float center_x,
                        float center_y, float radius)
    {
        inner.push_back({ Color::from_argb_encoded(0).inner, center_x });
        inner.push_back({ Color::from_argb_encoded(0).inner, center_y });
        inner.push_back({ Color::from_argb_encoded(0).inner, radius });
        for (int i = 0; i < stopCount; ++i, ++firstStop)
            inner.push_back(*firstStop);
    }

    /// Returns the number of gradient stops.
    int stopCount() const { return int(inner.size()) - 3; }

    /// Returns a pointer to the first gradient stop; undefined if the gradient has not stops.
    const GradientStop *stopsBegin() const { return inner.begin() + 3; }
    /// Returns a pointer past the last gradient stop. The returned pointer cannot be dereferenced,
    /// it can only be used for comparison.
    const GradientStop *stopsEnd() const { return inner.end(); }

private:
    cbindgen_private::types::RadialGradientBrush inner;

    friend class slint::Brush;
};

/// \private
/// ConicGradientBrush represents a conic gradient that rotates around a center point.
///
/// Internally the first three fake GradientStop entries encode the starting angle
/// (index 0), center-x (index 1), and center-y (index 2). Real color stops begin at
/// index 3. NaN center values mean "use bounding-box center".
class ConicGradientBrush
{
public:
    /// Constructs an empty conic gradient with no color stops.
    ConicGradientBrush() = default;
    /// Constructs a new conic gradient with the specified starting \a angle. The color stops will
    /// be constructed from the stops array pointed to be \a firstStop, with the length \a
    /// stopCount.
    ConicGradientBrush(float angle, const GradientStop *firstStop, int stopCount)
    {
        // Header: angle, center_x (NaN=bbox), center_y (NaN=bbox)
        inner.push_back({ Color::from_argb_encoded(0).inner, angle });
        inner.push_back(
                { Color::from_argb_encoded(0).inner, std::numeric_limits<float>::quiet_NaN() });
        inner.push_back(
                { Color::from_argb_encoded(0).inner, std::numeric_limits<float>::quiet_NaN() });
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
    ConicGradientBrush(float angle, const GradientStop *firstStop, int stopCount, float center_x,
                       float center_y)
    {
        // Header: angle, center_x, center_y
        inner.push_back({ Color::from_argb_encoded(0).inner, angle });
        inner.push_back({ Color::from_argb_encoded(0).inner, center_x });
        inner.push_back({ Color::from_argb_encoded(0).inner, center_y });
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
    int stopCount() const { return int(inner.size()) - 3; }

    /// Returns a pointer to the first gradient stop; undefined if the gradient has not stops.
    const GradientStop *stopsBegin() const { return inner.begin() + 3; }
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
    /// \note Radial and conic gradient brushes store center and radius as fake header stops whose
    /// position fields use NaN as a sentinel for "use the bounding box default" and a negative
    /// value for the default radius. Two brushes with default (NaN / negative) metadata compare
    /// equal, matching the Rust \c PartialEq semantics. A plain memory comparison would treat
    /// NaN != NaN and give incorrect results, hence this custom function.
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
        if (data.linear_gradient._0.size() > 1) {
            result.inner = data.linear_gradient._0[1].color;
        }
        break;
    case Tag::RadialGradient:
        // First 3 stops are the fake header (center_x, center_y, radius); real stops start at 3.
        if (data.radial_gradient._0.size() > 3) {
            result.inner = data.radial_gradient._0[3].color;
        }
        break;
    case Tag::ConicGradient:
        // First 3 stops are the fake header (angle, center_x, center_y); real stops start at 3.
        if (data.conic_gradient._0.size() > 3) {
            result.inner = data.conic_gradient._0[3].color;
        }
        break;
    }
    return result;
}

inline Brush Brush::brighter(float factor) const
{
    Brush result = *this;
    switch (data.tag) {
    case Tag::SolidColor:
        cbindgen_private::types::slint_color_brighter(&data.solid_color._0, factor,
                                                      &result.data.solid_color._0);
        break;
    case Tag::LinearGradient:
        for (std::size_t i = 1; i < data.linear_gradient._0.size(); ++i) {
            cbindgen_private::types::slint_color_brighter(&data.linear_gradient._0[i].color, factor,
                                                          &result.data.linear_gradient._0[i].color);
        }
        break;
    case Tag::RadialGradient:
        for (std::size_t i = 3; i < data.radial_gradient._0.size(); ++i) {
            cbindgen_private::types::slint_color_brighter(&data.radial_gradient._0[i].color, factor,
                                                          &result.data.radial_gradient._0[i].color);
        }
        break;
    case Tag::ConicGradient:
        for (std::size_t i = 3; i < data.conic_gradient._0.size(); ++i) {
            cbindgen_private::types::slint_color_brighter(&data.conic_gradient._0[i].color, factor,
                                                          &result.data.conic_gradient._0[i].color);
        }
        break;
    }
    return result;
}

inline Brush Brush::darker(float factor) const
{
    Brush result = *this;
    switch (data.tag) {
    case Tag::SolidColor:
        cbindgen_private::types::slint_color_darker(&data.solid_color._0, factor,
                                                    &result.data.solid_color._0);
        break;
    case Tag::LinearGradient:
        for (std::size_t i = 1; i < data.linear_gradient._0.size(); ++i) {
            cbindgen_private::types::slint_color_darker(&data.linear_gradient._0[i].color, factor,
                                                        &result.data.linear_gradient._0[i].color);
        }
        break;
    case Tag::RadialGradient:
        for (std::size_t i = 3; i < data.radial_gradient._0.size(); ++i) {
            cbindgen_private::types::slint_color_darker(&data.radial_gradient._0[i].color, factor,
                                                        &result.data.radial_gradient._0[i].color);
        }
        break;
    case Tag::ConicGradient:
        for (std::size_t i = 3; i < data.conic_gradient._0.size(); ++i) {
            cbindgen_private::types::slint_color_darker(&data.conic_gradient._0[i].color, factor,
                                                        &result.data.conic_gradient._0[i].color);
        }
        break;
    }
    return result;
}

inline Brush Brush::transparentize(float factor) const
{
    Brush result = *this;
    switch (data.tag) {
    case Tag::SolidColor:
        cbindgen_private::types::slint_color_transparentize(&data.solid_color._0, factor,
                                                            &result.data.solid_color._0);
        break;
    case Tag::LinearGradient:
        for (std::size_t i = 1; i < data.linear_gradient._0.size(); ++i) {
            cbindgen_private::types::slint_color_transparentize(
                    &data.linear_gradient._0[i].color, factor,
                    &result.data.linear_gradient._0[i].color);
        }
        break;
    case Tag::RadialGradient:
        for (std::size_t i = 3; i < data.radial_gradient._0.size(); ++i) {
            cbindgen_private::types::slint_color_transparentize(
                    &data.radial_gradient._0[i].color, factor,
                    &result.data.radial_gradient._0[i].color);
        }
        break;
    case Tag::ConicGradient:
        for (std::size_t i = 3; i < data.conic_gradient._0.size(); ++i) {
            cbindgen_private::types::slint_color_transparentize(
                    &data.conic_gradient._0[i].color, factor,
                    &result.data.conic_gradient._0[i].color);
        }
        break;
    }
    return result;
}

inline Brush Brush::with_alpha(float alpha) const
{
    Brush result = *this;
    switch (data.tag) {
    case Tag::SolidColor:
        cbindgen_private::types::slint_color_with_alpha(&data.solid_color._0, alpha,
                                                        &result.data.solid_color._0);
        break;
    case Tag::LinearGradient:
        for (std::size_t i = 1; i < data.linear_gradient._0.size(); ++i) {
            cbindgen_private::types::slint_color_with_alpha(
                    &data.linear_gradient._0[i].color, alpha,
                    &result.data.linear_gradient._0[i].color);
        }
        break;
    case Tag::RadialGradient:
        for (std::size_t i = 3; i < data.radial_gradient._0.size(); ++i) {
            cbindgen_private::types::slint_color_with_alpha(
                    &data.radial_gradient._0[i].color, alpha,
                    &result.data.radial_gradient._0[i].color);
        }
        break;
    case Tag::ConicGradient:
        for (std::size_t i = 3; i < data.conic_gradient._0.size(); ++i) {
            cbindgen_private::types::slint_color_with_alpha(
                    &data.conic_gradient._0[i].color, alpha,
                    &result.data.conic_gradient._0[i].color);
        }
        break;
    }
    return result;
}

namespace private_api {

template<>
inline void Property<slint::Brush>::set_animated_value(
        const slint::Brush &new_value,
        const cbindgen_private::PropertyAnimation &animation_data) const
{
    cbindgen_private::slint_property_set_animated_value_brush(&inner, &value, &new_value,
                                                              &animation_data);
}

} // namespace private_api

} // namespace slint
