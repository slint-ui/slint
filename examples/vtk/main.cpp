// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "app.h"
#include "vtk_view.h"

#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <memory>

int main()
{
    // VTK renders with desktop OpenGL 3.2 or newer.
    // Without this, Slint creates an OpenGL ES context on most platforms.
    using GraphicsAPI = slint::BackendRequirements::GraphicsAPI;
    if (auto error = slint::select_backend(
                { .graphics_api = GraphicsAPI::OpenGL, .min_version = { { 3, 2 } } })) {
        fprintf(stderr, "Error selecting a backend with OpenGL 3.2 support: %s\n", error->data());
        return EXIT_FAILURE;
    }

    auto app = App::create();
    auto view = std::make_shared<VtkView>();
    slint::ComponentWeakHandle<App> weak_app(app);

    auto redraw = [weak_app] {
        if (auto app = weak_app.lock()) {
            (*app)->window().request_redraw();
        }
    };
    app->on_select_surface([=](int index) {
        view->set_surface(index);
        redraw();
    });
    app->on_set_wireframe([=](bool wireframe) {
        view->set_wireframe(wireframe);
        redraw();
    });
    app->on_set_show_edges([=](bool show_edges) {
        view->set_show_edges(show_edges);
        redraw();
    });
    app->on_reset_camera([=] {
        view->reset_camera();
        redraw();
    });
    app->on_pointer_event([=](const slint::language::PointerEvent &event, float x, float y) {
        if (view->pointer_event(event, x, y)) {
            redraw();
        }
    });
    app->on_zoom([=](float delta) {
        view->zoom(delta);
        redraw();
    });

    auto last_frame = std::chrono::steady_clock::now();
    auto notifier = [=](slint::RenderingState state, slint::GraphicsAPI) mutable {
        if (state == slint::RenderingState::RenderingTeardown) {
            view->teardown();
            return;
        }
        auto app = weak_app.lock();
        if (!app) {
            return;
        }
        switch (state) {
        case slint::RenderingState::RenderingSetup:
            view->setup();
            (*app)->set_gl_info(slint::SharedString(view->gl_info()));
            break;
        case slint::RenderingState::BeforeRendering: {
            auto now = std::chrono::steady_clock::now();
            if ((*app)->get_auto_rotate()) {
                // Don't jump after startup or a pause in rendering.
                std::chrono::duration<double> elapsed = now - last_frame;
                view->rotate(20. * std::min(elapsed.count(), 0.1));
                (*app)->window().request_redraw();
            }
            last_frame = now;

            auto width = (*app)->get_view_width();
            auto height = (*app)->get_view_height();
            if (width > 0 && height > 0) {
                (*app)->set_texture(view->render(width, height));
            }
            break;
        }
        default:
            break;
        }
    };
    if (auto error = app->window().set_rendering_notifier(notifier)) {
        if (*error == slint::SetRenderingNotifierError::Unsupported) {
            fprintf(stderr,
                    "This example requires an OpenGL renderer. Run it with the environment "
                    "variable SLINT_BACKEND set to winit-femtovg or winit-skia-opengl.\n");
        } else {
            fprintf(stderr, "Unknown error calling set_rendering_notifier\n");
        }
        return EXIT_FAILURE;
    }

    app->run();
}
