// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// This example demonstrates using the SkCanvas pointer exposed through
// Slint's rendering notifier to draw game content directly via Skia.
//
// The game scene is rendered as an underlay in BeforeRendering, and Slint's
// UI (a HUD panel with controls) renders on top with a transparent background.
// Player movement is handled via Slint's FocusScope (WASD / arrow keys).
// Optional gamepad support via SDL2 (compile with -DENABLE_GAMEPAD=ON).

#include "scene.h"

#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstdlib>

#ifdef HAS_GAMEPAD
#include <SDL.h>
#endif

#include "include/core/SkCanvas.h"
#include "include/core/SkColor.h"
#include "include/core/SkData.h"
#include "include/core/SkImage.h"
#include "include/core/SkPaint.h"
#include "include/core/SkPath.h"
#include "include/core/SkPixmap.h"
#include "include/core/SkRect.h"
#include "include/core/SkSamplingOptions.h"

/// Generate a 32x32 gem sprite as BGRA pixel data.
static std::vector<uint32_t> generate_gem_pixels(uint8_t base_r, uint8_t base_g, uint8_t base_b)
{
    const int sz = 32;
    std::vector<uint32_t> pixels(sz * sz, 0);

    for (int y = 0; y < sz; y++) {
        for (int x = 0; x < sz; x++) {
            float fx = (x - sz / 2.0f) / (sz / 2.0f);
            float fy = (y - sz / 2.0f) / (sz / 2.0f);

            // Diamond shape
            float dist = std::abs(fx) + std::abs(fy);
            if (dist > 1.0f)
                continue;

            float shade = 1.0f - dist * 0.5f;
            float facet = std::max(0.0f, 0.3f - fx * 0.3f - fy * 0.4f);
            float brightness = std::clamp(shade + facet, 0.0f, 1.0f);

            uint8_t r = static_cast<uint8_t>(std::min(255.0f, base_r * brightness + 60 * facet));
            uint8_t g = static_cast<uint8_t>(std::min(255.0f, base_g * brightness + 60 * facet));
            uint8_t b = static_cast<uint8_t>(std::min(255.0f, base_b * brightness + 60 * facet));
            uint8_t a = static_cast<uint8_t>(255 * std::clamp((1.0f - dist) * 2.0f, 0.0f, 1.0f));

            // Premultiplied BGRA
            uint8_t pr = r * a / 255, pg = g * a / 255, pb = b * a / 255;
            pixels[y * sz + x] = (a << 24) | (pr << 16) | (pg << 8) | pb;
        }
    }
    return pixels;
}

class GameRenderer
{
public:
    GameRenderer(slint::ComponentWeakHandle<App> app) : app_weak(app) { }

    void operator()(slint::RenderingState state, slint::GraphicsAPI api)
    {
        if (api != slint::GraphicsAPI::Skia)
            return;

        if (state == slint::RenderingState::BeforeRendering) {
            // Lazily create gem images on first frame
            if (gem_images.empty())
                create_gem_images();
            if (auto app = app_weak.lock()) {
                update(*app);
                render(reinterpret_cast<SkCanvas *>(slint::skia_canvas()), *app);
                (*app)->window().request_redraw();
            }
        } else if (state == slint::RenderingState::RenderingTeardown) {
            gem_images.clear();
        }
    }

private:
    /// Create gem sprite images from procedurally generated pixel data.
    /// When a GrDirectContext is available, the raster images are automatically
    /// promoted to GPU textures by Skia when first drawn to a GPU-backed canvas.
    void create_gem_images()
    {
        const uint8_t gem_colors[][3] = {
            { 255, 50, 50 },   // Ruby
            { 50, 200, 50 },   // Emerald
            { 50, 100, 255 },  // Sapphire
            { 255, 200, 50 },  // Topaz
            { 200, 50, 255 },  // Amethyst
        };

        for (auto &c : gem_colors) {
            auto pixels = generate_gem_pixels(c[0], c[1], c[2]);
            auto info = SkImageInfo::Make(32, 32, kBGRA_8888_SkColorType, kPremul_SkAlphaType);
            size_t row_bytes = 32 * sizeof(uint32_t);
            auto data = SkData::MakeWithCopy(pixels.data(), pixels.size() * sizeof(uint32_t));
            gem_images.push_back(SkImages::RasterFromData(info, std::move(data), row_bytes));
        }
    }

    void update(auto &app)
    {
        auto now = std::chrono::steady_clock::now();
        float dt = std::chrono::duration<float>(now - last_update).count();
        last_update = now;

        // Read key states from Slint
        float speed = 0.4f * app->get_sprite_speed();

        float px = app->get_player_x();
        float py = app->get_player_y();
        float angle = app->get_player_angle();

        // Move player based on WASD / arrow keys
        float dx = 0, dy = 0;
        if (app->get_key_up())
            dy -= 1;
        if (app->get_key_down())
            dy += 1;
        if (app->get_key_left())
            dx -= 1;
        if (app->get_key_right())
            dx += 1;

#ifdef HAS_GAMEPAD
        // Blend in gamepad stick input (overrides keyboard if stick is deflected)
        if (gamepad) {
            SDL_GameControllerUpdate();
            const float deadzone = 0.15f;
            float gx = SDL_GameControllerGetAxis(gamepad, SDL_CONTROLLER_AXIS_LEFTX) / 32768.0f;
            float gy = SDL_GameControllerGetAxis(gamepad, SDL_CONTROLLER_AXIS_LEFTY) / 32768.0f;
            if (std::abs(gx) > deadzone || std::abs(gy) > deadzone) {
                dx = gx;
                dy = gy;
            }
        }
#endif

        // Normalize diagonal movement (only for digital input)
        float len = std::sqrt(dx * dx + dy * dy);
        if (len > 1.0f) {
            dx /= len;
            dy /= len;
        }

        px += dx * speed * dt;
        py += dy * speed * dt;

        // Clamp to window bounds
        px = std::clamp(px, 0.05f, 0.95f);
        py = std::clamp(py, 0.05f, 0.95f);

        // Rotate player to face movement direction
        if (dx != 0 || dy != 0) {
            float target = std::atan2(dy, dx) * 180.0f / 3.14159f + 90.0f;
            // Smooth rotation
            float diff = std::fmod(target - angle + 540.0f, 360.0f) - 180.0f;
            angle += diff * std::min(1.0f, dt * 10.0f);
        }

        app->set_player_x(px);
        app->set_player_y(py);
        app->set_player_angle(angle);
    }

    void render(SkCanvas *canvas, auto &app)
    {
        float speed = app->get_sprite_speed();
        bool show_grid = app->get_show_grid();

        auto elapsed = std::chrono::duration<float>(std::chrono::steady_clock::now() - start_time);
        float t = elapsed.count() * speed;

        auto size = canvas->getBaseLayerSize();
        float w = static_cast<float>(size.width());
        float h = static_cast<float>(size.height());
        if (w <= 0 || h <= 0)
            return;

        canvas->save();

        // Dark background
        canvas->clear(SkColorSetRGB(20, 20, 35));

        SkPaint paint;
        paint.setAntiAlias(true);

        // Tile grid
        if (show_grid) {
            paint.setColor(SkColorSetARGB(60, 100, 100, 140));
            paint.setStyle(SkPaint::kStroke_Style);
            paint.setStrokeWidth(1.0f);

            const float tile_size = 48.0f;
            for (float x = 0; x < w; x += tile_size)
                canvas->drawLine(x, 0, x, h, paint);
            for (float y = 0; y < h; y += tile_size)
                canvas->drawLine(0, y, w, y, paint);
        }

        // Gem sprites — drawn from SkImages created via GrDirectContext
        paint.setStyle(SkPaint::kFill_Style);
        for (size_t i = 0; i < gem_images.size(); i++) {
            if (!gem_images[i])
                continue;
            float phase = t * 0.6f + i * 1.5f;
            float x = w * (0.1f + 0.8f * (0.5f + 0.5f * std::sin(phase * 0.7f)));
            float y = h * (0.1f + 0.8f * (0.5f + 0.5f * std::cos(phase * 0.9f)));
            float bob = 6.0f * std::sin(phase * 3.0f);
            float scale = 2.0f + 0.5f * std::sin(phase * 2.0f);

            canvas->save();
            canvas->translate(x, y + bob);
            canvas->scale(scale, scale);
            canvas->rotate(std::sin(phase) * 15.0f);
            canvas->drawImage(gem_images[i], -16, -16,
                              SkSamplingOptions(SkFilterMode::kLinear), &paint);
            canvas->restore();
        }

        // Rotating "enemy" squares
        for (int i = 0; i < 3; i++) {
            float phase = t * 0.5f + i * 2.0f;
            float cx = w * (0.2f + 0.6f * (0.5f + 0.5f * std::cos(phase * 0.3f)));
            float cy = h * (0.3f + 0.4f * (0.5f + 0.5f * std::sin(phase * 0.4f)));

            canvas->save();
            canvas->translate(cx, cy);
            canvas->rotate(phase * 60.0f);

            paint.setColor(SkColorSetRGB(220, 80, 60));
            paint.setStyle(SkPaint::kFill_Style);
            canvas->drawRect(SkRect::MakeXYWH(-16, -16, 32, 32), paint);

            paint.setColor(SkColorSetRGB(255, 120, 100));
            paint.setStyle(SkPaint::kStroke_Style);
            paint.setStrokeWidth(2.0f);
            canvas->drawRect(SkRect::MakeXYWH(-16, -16, 32, 32), paint);

            canvas->restore();
        }

        // Player triangle — position controlled by WASD/arrow keys
        {
            float px = w * app->get_player_x();
            float py = h * app->get_player_y();
            float angle = app->get_player_angle();

            canvas->save();
            canvas->translate(px, py);
            canvas->rotate(angle);

            SkPath triangle;
            triangle.moveTo(0, -22);
            triangle.lineTo(-16, 16);
            triangle.lineTo(16, 16);
            triangle.close();

            // Glow behind player
            paint.setColor(SkColorSetARGB(30, 80, 200, 255));
            paint.setStyle(SkPaint::kFill_Style);
            canvas->drawCircle(0, 0, 40, paint);

            paint.setColor(SkColorSetRGB(80, 200, 255));
            paint.setStyle(SkPaint::kFill_Style);
            canvas->drawPath(triangle, paint);

            paint.setColor(SkColorSetRGB(150, 230, 255));
            paint.setStyle(SkPaint::kStroke_Style);
            paint.setStrokeWidth(2.0f);
            canvas->drawPath(triangle, paint);

            canvas->restore();
        }

        canvas->restore();
    }

    slint::ComponentWeakHandle<App> app_weak;
    std::vector<sk_sp<SkImage>> gem_images;
    std::chrono::time_point<std::chrono::steady_clock> start_time =
            std::chrono::steady_clock::now();
    std::chrono::time_point<std::chrono::steady_clock> last_update =
            std::chrono::steady_clock::now();

public:
#ifdef HAS_GAMEPAD
    SDL_GameController *gamepad = nullptr;
#endif
};

int main()
{
#ifdef HAS_GAMEPAD
    if (SDL_Init(SDL_INIT_GAMECONTROLLER) < 0) {
        fprintf(stderr, "SDL gamepad init failed: %s\n", SDL_GetError());
    }
#endif

    auto app = App::create();

    GameRenderer renderer(app);

#ifdef HAS_GAMEPAD
    // Open the first available gamepad
    for (int i = 0; i < SDL_NumJoysticks(); i++) {
        if (SDL_IsGameController(i)) {
            renderer.gamepad = SDL_GameControllerOpen(i);
            if (renderer.gamepad) {
                fprintf(stderr, "Gamepad connected: %s\n",
                        SDL_GameControllerName(renderer.gamepad));
                break;
            }
        }
    }
#endif

    if (auto error = app->window().set_rendering_notifier(std::move(renderer))) {
        if (*error == slint::SetRenderingNotifierError::Unsupported) {
            fprintf(stderr,
                    "This example requires the Skia renderer backend. "
                    "Please run with SLINT_BACKEND=winit-skia set.\n");
        } else {
            fprintf(stderr, "Unknown error calling set_rendering_notifier\n");
        }
        return EXIT_FAILURE;
    }

    app->run();

#ifdef HAS_GAMEPAD
    SDL_Quit();
#endif
    return EXIT_SUCCESS;
}
