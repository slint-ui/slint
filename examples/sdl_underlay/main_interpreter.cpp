// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Interpreter-based version of the SDL underlay example.
// Edit scene.slint while this is running and see changes live!
//
// Build with:
//   cmake -DSLINT_FEATURE_BACKEND_SDL=ON -DSLINT_FEATURE_INTERPRETER=ON ...

#include "slint-interpreter.h"

#include <SDL3/SDL.h>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <chrono>
#include <filesystem>
#include <string>

// ---------------------------------------------------------------------------
// C FFI declarations for the Slint SDL backend
// ---------------------------------------------------------------------------
extern "C" {
void slint_sdl_set_pre_render_callback(void (*callback)(void *renderer, void *user_data),
                                       void *user_data, void (*drop_user_data)(void *));
}

// ---------------------------------------------------------------------------
// Game rendering
// ---------------------------------------------------------------------------

struct GameState
{
    std::chrono::steady_clock::time_point start_time = std::chrono::steady_clock::now();
    bool animation_enabled = true;
    float speed = 1.0f;
};

static void render_game(SDL_Renderer *renderer, GameState *state)
{
    auto now = std::chrono::steady_clock::now();
    float elapsed = std::chrono::duration<float>(now - state->start_time).count();
    float t = state->animation_enabled ? elapsed * state->speed : 0.0f;

    SDL_SetRenderDrawBlendMode(renderer, SDL_BLENDMODE_BLEND);
    SDL_SetRenderDrawColor(renderer, 20, 20, 40, 255);
    SDL_RenderFillRect(renderer, nullptr);

    int w = 0, h = 0;
    SDL_GetCurrentRenderOutputSize(renderer, &w, &h);

    const int cols = 12, rows = 8;
    float cell_w = (float)w / cols, cell_h = (float)h / rows;

    for (int row = 0; row < rows; ++row) {
        for (int col = 0; col < cols; ++col) {
            float cx = col * cell_w + cell_w / 2.0f;
            float cy = row * cell_h + cell_h / 2.0f;
            float phase = t * 2.0f + col * 0.4f + row * 0.3f;
            float size = (cell_w < cell_h ? cell_w : cell_h) * 0.3f;
            size *= 0.5f + 0.5f * sinf(phase);

            uint8_t r = (uint8_t)(128 + 127 * sinf(t * 0.7f + col * 0.5f));
            uint8_t g = (uint8_t)(128 + 127 * sinf(t * 0.5f + row * 0.4f));
            uint8_t b = (uint8_t)(128 + 127 * sinf(t * 0.3f + (col + row) * 0.3f));
            SDL_SetRenderDrawColor(renderer, r, g, b, 180);
            SDL_FRect rect = { cx - size / 2, cy - size / 2, size, size };
            SDL_RenderFillRect(renderer, &rect);
        }
    }

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

static void pre_render(void *renderer_ptr, void *user_data)
{
    render_game(reinterpret_cast<SDL_Renderer *>(renderer_ptr),
                reinterpret_cast<GameState *>(user_data));
}

static void drop_game_state(void *user_data)
{
    delete reinterpret_cast<GameState *>(user_data);
}

// ---------------------------------------------------------------------------
// Load/reload the .slint component via the interpreter
// ---------------------------------------------------------------------------

struct AppState
{
    slint::ComponentHandle<slint::interpreter::ComponentInstance> instance;
    GameState *game;
};

static std::optional<slint::ComponentHandle<slint::interpreter::ComponentInstance>>
load_component(const std::string &path)
{
    slint::interpreter::ComponentCompiler compiler;
    auto def = compiler.build_from_path(path);

    for (auto &diag : compiler.diagnostics()) {
        fprintf(stderr, "%s:%d: %s\n", std::string(diag.source_file).c_str(), diag.line,
                std::string(diag.message).c_str());
    }

    if (!def) {
        fprintf(stderr, "Failed to compile %s\n", path.c_str());
        return std::nullopt;
    }

    return def->create();
}

static void setup_instance(slint::ComponentHandle<slint::interpreter::ComponentInstance> &instance,
                           GameState *state)
{
    instance->set_callback("quit", [](auto) {
        slint::quit_event_loop();
        return slint::interpreter::Value();
    });
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

int main(int argc, char *argv[])
{
    std::string slint_path = "examples/sdl_underlay/scene.slint";
    if (argc > 1)
        slint_path = argv[1];

    // Initial load
    auto instance = load_component(slint_path);
    if (!instance)
        return 1;

    // Set up game rendering
    auto *game_state = new GameState();
    slint_sdl_set_pre_render_callback(pre_render, game_state, drop_game_state);

    setup_instance(*instance, game_state);

    // File watcher: poll the .slint file's modification time and reload on change
    auto last_mtime = std::filesystem::last_write_time(slint_path);

    slint::Timer reload_timer;
    reload_timer.start(slint::TimerMode::Repeated, std::chrono::milliseconds(500),
                       [&slint_path, &instance, game_state, &last_mtime, &reload_timer] {
                           std::error_code ec;
                           auto mtime = std::filesystem::last_write_time(slint_path, ec);
                           if (ec || mtime == last_mtime)
                               return;
                           last_mtime = mtime;

                           fprintf(stderr, "[sdl_underlay] Reloading %s...\n", slint_path.c_str());
                           auto new_instance = load_component(slint_path);
                           if (!new_instance)
                               return;

                           // Show new before hiding old, so the window count never drops to
                           // zero (which would quit the event loop).
                           setup_instance(*new_instance, game_state);
                           (*new_instance)->show();
                           (*instance)->hide();
                           *instance = *new_instance;
                       });

    // Animation timer
    slint::Timer anim_timer;
    anim_timer.start(slint::TimerMode::Repeated, std::chrono::milliseconds(16),
                     [&instance, game_state] {
                         auto val = (*instance)->get_property("animation-enabled");
                         if (val && val->to_bool())
                             game_state->animation_enabled = *val->to_bool();
                         auto spd = (*instance)->get_property("speed");
                         if (spd && spd->to_number())
                             game_state->speed = static_cast<float>(*spd->to_number());
                         (*instance)->window().request_redraw();
                     });

    (*instance)->show();
    slint::run_event_loop();
    return 0;
}
