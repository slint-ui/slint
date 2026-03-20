// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// This example demonstrates how a C++ game using SDL3 can render its own
// content with SDL_Renderer and overlay a Slint UI on top.
//
// Build with:
//   cmake -DSLINT_FEATURE_BACKEND_SDL=ON -DSLINT_FEATURE_BACKEND_WINIT=OFF ...
//
// Or set SLINT_BACKEND=sdl at runtime if multiple backends are compiled in.

#include "scene.h"

#include <SDL3/SDL.h>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <chrono>

// ---------------------------------------------------------------------------
// C FFI declarations for the Slint SDL backend.
// These functions are exported by the Slint library when compiled with the
// backend-sdl feature.
// ---------------------------------------------------------------------------
extern "C" {
    /// Set a callback that is invoked before each Slint frame.  The callback
    /// receives the SDL_Renderer* and a user-data pointer.
    void slint_sdl_set_pre_render_callback(
        void (*callback)(void *renderer, void *user_data),
        void *user_data,
        void (*drop_user_data)(void *));

    /// Returns the SDL_Renderer* managed by the Slint SDL backend.
    void *slint_sdl_get_renderer();

    /// Returns the SDL_Window* managed by the Slint SDL backend.
    void *slint_sdl_get_window();
}

// ---------------------------------------------------------------------------
// Game rendering — a simple animated scene drawn with SDL_Renderer
// ---------------------------------------------------------------------------

struct GameState {
    std::chrono::steady_clock::time_point start_time = std::chrono::steady_clock::now();
    bool animation_enabled = true;
    float speed = 1.0f;
};

/// Draw a simple animated scene: colored bouncing rectangles and a
/// rotating pattern, all using plain SDL_Renderer calls.
static void render_game(SDL_Renderer *renderer, GameState *state)
{
    auto now = std::chrono::steady_clock::now();
    float elapsed = std::chrono::duration<float>(now - state->start_time).count();
    float t = state->animation_enabled ? elapsed * state->speed : 0.0f;

    // Dark background
    SDL_SetRenderDrawBlendMode(renderer, SDL_BLENDMODE_BLEND);
    SDL_SetRenderDrawColor(renderer, 20, 20, 40, 255);
    SDL_RenderFillRect(renderer, nullptr);

    // Draw a grid of animated diamonds
    int w = 0, h = 0;
    SDL_GetCurrentRenderOutputSize(renderer, &w, &h);

    const int cols = 12;
    const int rows = 8;
    float cell_w = (float)w / cols;
    float cell_h = (float)h / rows;

    for (int row = 0; row < rows; ++row) {
        for (int col = 0; col < cols; ++col) {
            float cx = col * cell_w + cell_w / 2.0f;
            float cy = row * cell_h + cell_h / 2.0f;

            // Animate size with a wave pattern
            float phase = t * 2.0f + col * 0.4f + row * 0.3f;
            float size = (cell_w < cell_h ? cell_w : cell_h) * 0.3f;
            size *= 0.5f + 0.5f * sinf(phase);

            // Color based on position and time
            uint8_t r = (uint8_t)(128 + 127 * sinf(t * 0.7f + col * 0.5f));
            uint8_t g = (uint8_t)(128 + 127 * sinf(t * 0.5f + row * 0.4f));
            uint8_t b = (uint8_t)(128 + 127 * sinf(t * 0.3f + (col + row) * 0.3f));

            SDL_SetRenderDrawColor(renderer, r, g, b, 180);

            SDL_FRect rect = {
                cx - size / 2.0f,
                cy - size / 2.0f,
                size,
                size
            };
            SDL_RenderFillRect(renderer, &rect);
        }
    }

    // Draw a few larger "star" rectangles that float across the screen
    for (int i = 0; i < 5; ++i) {
        float x = fmodf(t * (40.0f + i * 15.0f) + i * 200.0f, (float)(w + 100)) - 50.0f;
        float y = h * (0.15f + 0.15f * i) + 30.0f * sinf(t * 1.5f + i);
        float sz = 30.0f + 20.0f * sinf(t * 2.0f + i * 1.1f);

        uint8_t alpha = (uint8_t)(100 + 80 * sinf(t + i));
        SDL_SetRenderDrawColor(renderer, 255, 255, 255, alpha);

        SDL_FRect star = { x - sz / 2, y - sz / 2, sz, sz };
        SDL_RenderFillRect(renderer, &star);
    }
}

// ---------------------------------------------------------------------------
// Pre-render callback — called by Slint before each UI frame
// ---------------------------------------------------------------------------

static void pre_render(void *renderer_ptr, void *user_data)
{
    auto *renderer = reinterpret_cast<SDL_Renderer *>(renderer_ptr);
    auto *state = reinterpret_cast<GameState *>(user_data);
    render_game(renderer, state);
}

static void drop_game_state(void *user_data)
{
    delete reinterpret_cast<GameState *>(user_data);
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

int main()
{
    // Create the Slint UI. This triggers backend initialization.
    auto app = App::create();

    // Create game state and register the pre-render callback.
    // The callback draws the game scene before Slint renders its UI on top.
    auto *state = new GameState();

    slint_sdl_set_pre_render_callback(pre_render, state, drop_game_state);

    // Wire up the UI controls to the game state.
    // We keep a raw pointer since the GameState lifetime is managed by the
    // callback (drop_game_state frees it when the callback is removed).
    auto *state_ptr = state;

    app->on_quit([] { slint::quit_event_loop(); });

    // Poll the UI properties each frame via a timer to update game state.
    // (In a real game you'd integrate this into your game loop instead.)
    slint::ComponentWeakHandle<App> app_weak(app);
    slint::Timer animation_timer;
    animation_timer.start(slint::TimerMode::Repeated,
                          std::chrono::milliseconds(16),
                          [app_weak, state_ptr] {
        if (auto app = app_weak.lock()) {
            state_ptr->animation_enabled = (*app)->get_animation_enabled();
            state_ptr->speed = (*app)->get_speed();
            (*app)->window().request_redraw();
        }
    });

    app->run();
    return 0;
}
