// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include <algorithm>
#include <climits>
#include <cmath>
#include <span>
#include "slint-platform.h"
#include "driver/ppa.h"
#include "esp_cache.h"
#include "esp_memory_utils.h"

// A frame buffer that hands opaque solid fills, and blends of unscaled, unrotated RGBA images and
// alpha maps, such as the renderer's prepared images, to the Pixel-Processing Accelerator.
// It reads RGBA images blue first, as PreparedTextures::set_blue_first(true) prepares them, since
// the PPA's channel swap moves alpha too.
// The software renderer draws everything it declines.
class PpaFrameBuffer
    : public slint::platform::SoftwareRenderer::TargetPixelBuffer<slint::platform::Rgb565Pixel>
{
public:
    using Pixel = slint::platform::Rgb565Pixel;
    using PhysicalRegion = slint::platform::SoftwareRenderer::PhysicalRegion;
    using DrawRectangleArgs = slint::platform::SoftwareRenderer::DrawRectangleArgs;
    using DrawTextureArgs = slint::platform::SoftwareRenderer::DrawTextureArgs;

    bool init()
    {
        ppa_client_config_t fill_config = {}, blend_config = {};
        fill_config.oper_type = PPA_OPERATION_FILL;
        blend_config.oper_type = PPA_OPERATION_BLEND;
        return ppa_register_client(&fill_config, &fill_client) == ESP_OK
                && ppa_register_client(&blend_config, &blend_client) == ESP_OK;
    }

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

    bool fill_background(const slint::Brush &brush, const PhysicalRegion &clip) override
    {
        return is_opaque_color(brush)
                && fill_clipped(INT_MIN, INT_MIN, INT_MAX, INT_MAX, brush.color(), clip);
    }

    bool draw_rectangle(const DrawRectangleArgs &args, const PhysicalRegion &clip) override
    {
        const bool square = args.top_left_radius == 0 && args.top_right_radius == 0
                && args.bottom_right_radius == 0 && args.bottom_left_radius == 0;
        const bool borderless = args.border_width == 0
                || (is_solid(args.border) && args.border.color().alpha() == 0);
        if (!square || !borderless || args.alpha != 255 || !is_opaque_color(args.background)) {
            return false;
        }
        // Rounded the same way as the software renderer's own rectangles.
        return fill_clipped(std::lround(args.x), std::lround(args.y),
                            std::lround(args.x + args.width), std::lround(args.y + args.height),
                            args.background.color(), clip);
    }

    bool draw_texture(const DrawTextureArgs &texture, const PhysicalRegion &clip) override
    {
        using Format = slint::cbindgen_private::TexturePixelFormat;
        const bool alpha_map = texture.pixel_format == Format::AlphaMap;
        // The DMA can't read images that stayed in flash.
        if ((texture.pixel_format != Format::Rgba && !alpha_map)
            || alpha_map != (texture.colorize.alpha() != 0) || texture.rotation != 0
            || texture.has_tiling || texture.dst_width != texture.width
            || texture.dst_height != texture.height
            || texture.dst_width * texture.dst_height < min_ppa_blend_pixels
            || !(esp_ptr_dma_capable(texture.image_data)
                 || esp_ptr_dma_ext_capable(texture.image_data))) {
            return false;
        }
        ppa_blend_oper_config_t config = {};
        config.in_fg.buffer = texture.image_data;
        config.in_fg.pic_w = texture.byte_stride / (alpha_map ? 1 : 4);
        config.in_fg.pic_h = texture.height;
        config.in_fg.blend_cm = alpha_map ? PPA_BLEND_COLOR_MODE_A8 : PPA_BLEND_COLOR_MODE_ARGB8888;
        config.fg_fix_rgb_val.val = rgb888(texture.colorize);
        const unsigned alpha =
                alpha_map ? texture.alpha * texture.colorize.alpha() / 255 : texture.alpha;
        if (alpha != 255) {
            config.fg_alpha_update_mode = PPA_ALPHA_SCALE;
            config.fg_alpha_scale_ratio = alpha / 256.f;
        }
        config.in_bg.buffer = pixels.data();
        config.in_bg.pic_w = width;
        config.in_bg.pic_h = num_lines();
        config.in_bg.blend_cm = PPA_BLEND_COLOR_MODE_RGB565;
        config.out.blend_cm = PPA_BLEND_COLOR_MODE_RGB565;
        config.mode = PPA_TRANS_MODE_BLOCKING;
        const int x0 = texture.dst_x, y0 = texture.dst_y;
        // Declining after drawing part of it would make the renderer blend that part twice.
        bool drew_any = false;
        for_each_clipped(x0, y0, x0 + texture.dst_width, y0 + texture.dst_height, clip,
                         [&](int left, int top, int right, int bottom) {
                             config.in_bg.block_w = config.in_fg.block_w = right - left;
                             config.in_bg.block_h = config.in_fg.block_h = bottom - top;
                             config.in_bg.block_offset_x = left;
                             config.in_bg.block_offset_y = top;
                             config.in_fg.block_offset_x = left - x0;
                             config.in_fg.block_offset_y = top - y0;
                             set_output(config.out, left, top, bottom);
                             SLINT_TRACE_FIELD("px", (right - left) * (bottom - top));
                             SLINT_TRACE_SPAN("ppa_blend");
                             drew_any |= ppa_do_blend(blend_client, &config) == ESP_OK;
                             return drew_any;
                         });
        return drew_any;
    }

private:
    // Below these sizes, setting up a PPA operation costs more than the CPU's own drawing:
    // about 44 µs per operation, against the CPU's 39 px/µs fill and 4.7 px/µs blend into PSRAM.
    static constexpr int min_ppa_fill_pixels = 2048;
    static constexpr std::size_t min_ppa_blend_pixels = 1024;

    static bool is_solid(const slint::Brush &brush) { return brush == slint::Brush(brush.color()); }
    static bool is_opaque_color(const slint::Brush &brush)
    {
        return is_solid(brush) && brush.color().alpha() == 255;
    }
    static uint32_t rgb888(slint::Color color)
    {
        return uint32_t(color.red()) << 16 | uint32_t(color.green()) << 8 | color.blue();
    }

    // Calls `f` with each part of the rectangle inside `clip`, until it returns false.
    template<typename F>
    static bool for_each_clipped(int x0, int y0, int x1, int y1, const PhysicalRegion &clip, F f)
    {
        for (auto rect : clip.rectangles()) {
            const int left = std::max(x0, int(rect.origin.x));
            const int top = std::max(y0, int(rect.origin.y));
            const int right = std::min(x1, int(rect.origin.x) + int(rect.size.width));
            const int bottom = std::min(y1, int(rect.origin.y) + int(rect.size.height));
            if (left < right && top < bottom && !f(left, top, right, bottom)) {
                return false;
            }
        }
        return true;
    }

    // Sets `out` to the frame buffer, at the block from (x, top) down to `bottom`. For blend and
    // fill operations, the PPA driver only invalidates the block's rows, rounded out to whole
    // cache lines. That would drop pixels the CPU drew there and, past the ends of the frame
    // buffer, changes to neighboring heap memory. Writing back first, rounded out further than
    // any cache line, leaves only clean lines to invalidate.
    void set_output(ppa_out_pic_blk_config_t &out, int x, int top, int bottom)
    {
        out.buffer = pixels.data();
        out.buffer_size = pixels.size_bytes();
        out.pic_w = width;
        out.pic_h = num_lines();
        out.block_offset_x = x;
        out.block_offset_y = top;
        constexpr std::uintptr_t rounding = 256;
        const auto start = reinterpret_cast<std::uintptr_t>(pixels.data() + top * width);
        const auto end = reinterpret_cast<std::uintptr_t>(pixels.data() + bottom * width);
        const auto aligned_start = start & ~(rounding - 1);
        const auto aligned_end = (end + rounding - 1) & ~(rounding - 1);
        esp_cache_msync(reinterpret_cast<void *>(aligned_start), aligned_end - aligned_start,
                        ESP_CACHE_MSYNC_FLAG_DIR_C2M);
    }

    bool fill_clipped(int x0, int y0, int x1, int y1, slint::Color color,
                      const PhysicalRegion &clip)
    {
        const Pixel pixel(slint::Rgb8Pixel { color.red(), color.green(), color.blue() });
        return for_each_clipped(
                x0, y0, x1, y1, clip, [&](int left, int top, int right, int bottom) {
                    if ((right - left) * (bottom - top) < min_ppa_fill_pixels) {
                        for (int y = top; y < bottom; ++y) {
                            std::fill_n(&pixels[y * width + left], right - left, pixel);
                        }
                        return true;
                    }
                    ppa_fill_oper_config_t config = {};
                    set_output(config.out, left, top, bottom);
                    config.out.fill_cm = PPA_FILL_COLOR_MODE_RGB565;
                    config.fill_block_w = right - left;
                    config.fill_block_h = bottom - top;
                    config.fill_argb_color.val = 0xff000000 | rgb888(color);
                    config.mode = PPA_TRANS_MODE_BLOCKING;
                    SLINT_TRACE_FIELD("px", (right - left) * (bottom - top));
                    SLINT_TRACE_SPAN("ppa_fill");
                    return ppa_do_fill(fill_client, &config) == ESP_OK;
                });
    }

    std::span<Pixel> pixels;
    std::size_t width = 0;
    ppa_client_handle_t fill_client = nullptr;
    ppa_client_handle_t blend_client = nullptr;
};
