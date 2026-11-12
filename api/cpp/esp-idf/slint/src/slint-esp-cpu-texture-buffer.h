// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include <algorithm>
#include <bit>
#include <span>
#include "slint-platform.h"

// A frame buffer that blends each unscaled image with one loop over its visible rows, instead of
// the software renderer's per-line texture path. The pixel math matches draw_functions.rs.
class CpuTextureFrameBuffer
    : public slint::platform::SoftwareRenderer::TargetPixelBuffer<slint::platform::Rgb565Pixel>
{
public:
    using Pixel = slint::platform::Rgb565Pixel;
    using PhysicalRegion = slint::platform::SoftwareRenderer::PhysicalRegion;
    using DrawRectangleArgs = slint::platform::SoftwareRenderer::DrawRectangleArgs;
    using DrawTextureArgs = slint::platform::SoftwareRenderer::DrawTextureArgs;

    void set_target(std::span<Pixel> target, std::size_t target_width)
    {
        pixels = target;
        width = target_width;
    }

    std::span<Pixel> line_slice(std::size_t line) override
    {
        return pixels.subspan(line * width, width);
    }

    std::size_t num_lines() override { return pixels.size() / width; }

    bool fill_background(const slint::Brush &, const PhysicalRegion &) override { return false; }

    bool draw_rectangle(const DrawRectangleArgs &, const PhysicalRegion &) override
    {
        return false;
    }

    bool draw_texture(const DrawTextureArgs &texture, const PhysicalRegion &clip) override
    {
        const int bytes_per_pixel = unscaled_bytes_per_pixel(texture);
        if (bytes_per_pixel == 0) {
            return false;
        }
        if (texture.alpha == 0) {
            return true;
        }
        const int dst_width = int(texture.dst_width), dst_height = int(texture.dst_height);
        const int x0 = int(texture.dst_x), y0 = int(texture.dst_y);
        for (auto rect : clip.rectangles()) {
            const int left = std::max(x0, int(rect.origin.x));
            const int top = std::max(y0, int(rect.origin.y));
            const int right = std::min(x0 + dst_width, int(rect.origin.x) + int(rect.size.width));
            const int bottom =
                    std::min(y0 + dst_height, int(rect.origin.y) + int(rect.size.height));
            if (left >= right || top >= bottom) {
                continue;
            }
            for (int y = top; y < bottom; ++y) {
                int step;
                const uint8_t *source =
                        source_pixel(texture, bytes_per_pixel, left - x0, y - y0, step);
                blend_row(&pixels[std::size_t(y) * width + left], right - left, source, step,
                          texture);
            }
        }
        return true;
    }

protected:
    // The bytes per source pixel of an image drawn unscaled and untiled, or 0 for other images.
    static int unscaled_bytes_per_pixel(const DrawTextureArgs &texture)
    {
        using Format = slint::cbindgen_private::TexturePixelFormat;
        const bool transposed = texture.rotation == 90 || texture.rotation == 270;
        if (texture.has_tiling || texture.dst_width != (transposed ? texture.height : texture.width)
            || texture.dst_height != (transposed ? texture.width : texture.height)) {
            return 0;
        }
        switch (texture.pixel_format) {
        case Format::Rgb:
            return 3;
        case Format::Rgba:
        case Format::RgbaPremultiplied:
            return 4;
        case Format::AlphaMap:
            return 1;
        default:
            return 0;
        }
    }

    // The source of the destination pixel (x, y), relative to the image's top left. `step` is
    // the distance to the source of the next pixel to the right. The mapping is the one of
    // RenderingRotation's mirror_width and mirror_height in api.rs.
    static const uint8_t *source_pixel(const DrawTextureArgs &texture, int bytes_per_pixel, int x,
                                       int y, int &step)
    {
        const bool transposed = texture.rotation == 90 || texture.rotation == 270;
        const bool mirror_rows = texture.rotation == 180 || texture.rotation == 270;
        const bool mirror_columns = texture.rotation == 90 || texture.rotation == 180;
        const int stride = int(texture.byte_stride);
        const int row = mirror_rows ? int(texture.dst_height) - 1 - y : y;
        const int column = mirror_columns ? int(texture.dst_width) - 1 - x : x;
        step = transposed ? stride : bytes_per_pixel;
        if (mirror_columns) {
            step = -step;
        }
        return texture.image_data
                + (transposed ? column * stride + row * bytes_per_pixel
                              : row * stride + column * bytes_per_pixel);
    }

    std::span<Pixel> pixels;
    std::size_t width = 0;

    // Rgb565Pixel::from_rgb.
    static uint16_t from_rgb(uint8_t r, uint8_t g, uint8_t b)
    {
        return uint16_t(((r & 0xf8) << 8) | ((g & 0xfc) << 3) | (b >> 3));
    }

private:
    static constexpr uint16_t red_blue_mask = 0xf81f;
    static constexpr uint16_t green_mask = 0x07e0;

    // Rgb565Pixel::blend with a premultiplied color.
    static uint16_t blend(uint16_t pixel, uint8_t a, uint8_t r, uint8_t g, uint8_t b)
    {
        const uint32_t inverse = ((255u - a) + 4) >> 3;
        const uint32_t expanded = (pixel & red_blue_mask) | (uint32_t(pixel & green_mask) << 16);
        const uint32_t color = ((uint32_t(r) << 13) | (uint32_t(g) << 24) | (uint32_t(b) << 2))
                & 0b11111100'00011111'00000011'11100000u;
        const uint32_t result = expanded * inverse + color;
        return uint16_t(((result >> 21) & green_mask) | ((result >> 5) & red_blue_mask));
    }

    static uint16_t blend_unpremultiplied(uint16_t pixel, uint8_t a, uint8_t r, uint8_t g,
                                          uint8_t b)
    {
        return blend(pixel, a, uint8_t(r * a / 255), uint8_t(g * a / 255), uint8_t(b * a / 255));
    }

    template<typename F>
    static void for_each(Pixel *target, int count, const uint8_t *source, int step, F f)
    {
        for (int i = 0; i < count; ++i, source += step) {
            target[i] = std::bit_cast<Pixel>(f(std::bit_cast<uint16_t>(target[i]), source));
        }
    }

    static void blend_row(Pixel *target, int count, const uint8_t *source, int step,
                          const DrawTextureArgs &texture)
    {
        using Format = slint::cbindgen_private::TexturePixelFormat;
        const unsigned alpha = texture.alpha;
        const auto colorize = texture.colorize;
        const bool colorized = colorize.alpha() != 0 || texture.pixel_format == Format::AlphaMap;
        if (colorized) {
            const int alpha_offset = texture.pixel_format == Format::AlphaMap ? 0 : 3;
            const uint8_t cr = colorize.red(), cg = colorize.green(), cb = colorize.blue();
            const uint16_t opaque = from_rgb(cr, cg, cb);
            for_each(target, count, source, step, [&](uint16_t pixel, const uint8_t *p) {
                const uint8_t coverage = uint8_t(p[alpha_offset] * alpha / 255);
                switch (coverage) {
                case 0:
                    return pixel;
                case 255:
                    return opaque;
                default:
                    return blend_unpremultiplied(pixel, coverage, cr, cg, cb);
                }
            });
            return;
        }
        switch (texture.pixel_format) {
        case Format::Rgb:
            for_each(target, count, source, step, [&](uint16_t pixel, const uint8_t *p) {
                return alpha == 255 ? from_rgb(p[0], p[1], p[2])
                                    : blend_unpremultiplied(pixel, alpha, p[0], p[1], p[2]);
            });
            break;
        case Format::Rgba:
            for_each(target, count, source, step, [&](uint16_t pixel, const uint8_t *p) {
                const uint8_t a = uint8_t(p[3] * alpha / 255);
                switch (a) {
                case 0:
                    return pixel;
                case 255:
                    return from_rgb(p[0], p[1], p[2]);
                default:
                    return blend_unpremultiplied(pixel, a, p[0], p[1], p[2]);
                }
            });
            break;
        default: // RgbaPremultiplied
            for_each(target, count, source, step, [&](uint16_t pixel, const uint8_t *p) {
                if (p[3] == 0) {
                    return pixel;
                }
                if (alpha == 255) {
                    return p[3] == 255 ? from_rgb(p[0], p[1], p[2])
                                       : blend(pixel, p[3], p[0], p[1], p[2]);
                }
                return blend(pixel, uint8_t(p[3] * alpha / 255), uint8_t(p[0] * alpha / 255),
                             uint8_t(p[1] * alpha / 255), uint8_t(p[2] * alpha / 255));
            });
        }
    }
};
