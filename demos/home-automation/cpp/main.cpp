// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "demo.h"

#include "slint-platform.h"
#include "slint_size.h"

#include <cstdio>
#include <deque>
#include <mutex>
#include <print>

using RepaintBufferType = slint::platform::SoftwareRenderer::RepaintBufferType;

class TestWindowAdapter : public slint::platform::WindowAdapter
{
public:
    slint::platform::SoftwareRenderer m_renderer;
    bool needs_redraw = true;
    const slint::PhysicalSize m_size;

    explicit TestWindowAdapter(RepaintBufferType buffer_type, slint::PhysicalSize size)
        : m_renderer(buffer_type), m_size(size)
    {
    }

    slint::platform::AbstractRenderer &renderer() override { return m_renderer; }

    slint::PhysicalSize size() override { return m_size; }

    void request_redraw() override { needs_redraw = true; }
};

template<typename PixelType>
struct TestPlatform : public slint::platform::Platform
{
    TestPlatform(const slint::PhysicalSize &size) : size(size) { }

    std::unique_ptr<slint::platform::WindowAdapter> create_window_adapter() override;

    std::chrono::milliseconds duration_since_start() override;
    void run_event_loop() override;
    void quit_event_loop() override;
    void run_in_event_loop(Task) override;

private:
    slint::PhysicalSize size;
    int loop_count = 0;
    RepaintBufferType buffer_type = RepaintBufferType::ReusedBuffer;
    class TestWindowAdapter *m_window = nullptr;
    std::mutex queue_mutex;
    std::deque<slint::platform::Platform::Task> queue; // protected by queue_mutex
    bool quit = false; // protected by queue_mutex
};

template<typename PixelType>
std::unique_ptr<slint::platform::WindowAdapter> TestPlatform<PixelType>::create_window_adapter()
{
    auto window = std::make_unique<TestWindowAdapter>(buffer_type, size);
    m_window = window.get();
    return window;
}

template<typename PixelType>
std::chrono::milliseconds TestPlatform<PixelType>::duration_since_start()
{
    return std::chrono::milliseconds(loop_count * 16);
}

template<typename PixelType>
void TestPlatform<PixelType>::run_event_loop()
{
    while (loop_count < 100) {
        loop_count++;
        slint::platform::update_timers_and_animations();

        std::optional<slint::platform::Platform::Task> event;
        {
            std::unique_lock lock(queue_mutex);
            if (queue.empty()) {
                if (quit) {
                    quit = false;
                    break;
                }
            } else {
                event = std::move(queue.front());
                queue.pop_front();
            }
        }
        if (event) {
            std::move(*event).run();
            event.reset();
            continue;
        }

        if (m_window) {

            if (loop_count % 20 == 5) {
                m_window->window().dispatch_pointer_press_event(
                        slint::LogicalPosition({ 150, 150 }), slint::PointerEventButton::Left);
            }
            if (loop_count % 20 == 15) {
                m_window->window().dispatch_pointer_release_event(
                        slint::LogicalPosition({ 150, 450 }), slint::PointerEventButton::Left);
            }

            if (std::exchange(m_window->needs_redraw, false)) {
                std::println("Drawing {}", loop_count);
                using slint::platform::SoftwareRenderer;
                auto stride = size.width;
                std::vector<PixelType> line_buffer(stride);
                m_window->m_renderer.render_by_line<PixelType>(
                        [this, &line_buffer](std::size_t line_y, std::size_t line_start,
                                             std::size_t line_end, auto &&render_fn) {
                            std::span<PixelType> view { line_buffer.data(), line_end - line_start };
                            render_fn(view);
                        });
            }
        }

        if (m_window->window().has_active_animations()) {
            continue;
        }
    }
}

template<typename PixelType>
void TestPlatform<PixelType>::quit_event_loop()
{
    {
        const std::unique_lock lock(queue_mutex);
        quit = true;
    }
}

template<typename PixelType>
void TestPlatform<PixelType>::run_in_event_loop(slint::platform::Platform::Task event)
{

    const std::unique_lock lock(queue_mutex);
    queue.push_back(std::move(event));
}

int main()
{
    slint::platform::set_platform(std::make_unique<TestPlatform<slint::platform::Rgb565Pixel>>(
            slint::PhysicalSize({ 1024, 600 })));
    auto demo = AppWindow::create();
    demo->run();
}
